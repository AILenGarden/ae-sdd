#![allow(unused_imports)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use ae_sdd_context::{
    PressureDecision, PressurePolicy, PressureSample, PressureSource, PressureTracker,
};
use ae_sdd_domain::{
    AgentRole, ClaimId, CompactId, ContextGeneration, DelegationId, EventStoreId, HostAckId,
    HostActionId, InputFingerprint, SampleSequence, SessionId,
};
use ae_sdd_flow::{FlowDecision, FlowEvent, FlowInput, FlowRuntime};
use ae_sdd_host::{
    ChildClaim, HostAck, HostAckOutcome, HostAction, HostActionKind, HostAdapterId, HostTaskId,
    PhysicalSessionProof,
};
use ae_sdd_protocol::StableErrorCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    ContextProjectResult, ContextProjectionInput, DelegationCreatePayload, DelegationReportPayload,
    DelegationResult, HostAckPayload, HostActionPayload, HostPressurePayload, PersistencePort,
    RuntimeError, RuntimeResult, WireAgentRole,
};

/// Durable Host action queue with exact ACK correlation.
pub struct HostCoordinator {
    persistence: Arc<dyn PersistencePort>,
    registrations: Mutex<BTreeMap<String, BTreeSet<String>>>,
    queues: Mutex<BTreeMap<String, VecDeque<HostActionPayload>>>,
    acknowledgements: Mutex<BTreeMap<String, HostAckPayload>>,
    command_sequences: Mutex<BTreeMap<String, u64>>,
}

impl HostCoordinator {
    /// Creates a Host coordinator over durable records.
    #[must_use]
    pub fn new(persistence: Arc<dyn PersistencePort>) -> Self {
        Self {
            persistence,
            registrations: Mutex::new(BTreeMap::new()),
            queues: Mutex::new(BTreeMap::new()),
            acknowledgements: Mutex::new(BTreeMap::new()),
            command_sequences: Mutex::new(BTreeMap::new()),
        }
    }

    /// Restores durable adapter registrations, ACKs, command sequences, and pending queues.
    pub fn recover(&self) -> RuntimeResult<()> {
        let mut registrations = BTreeMap::new();
        for (adapter_id, value) in self.persistence.list_records("host-adapter/v1")? {
            let capabilities = value
                .get("capabilities")
                .and_then(Value::as_array)
                .ok_or_else(|| malformed("durable host adapter capabilities are malformed"))?
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| malformed("durable host adapter capability is not a string"))
                })
                .collect::<RuntimeResult<BTreeSet<_>>>()?;
            registrations.insert(adapter_id, capabilities);
        }

        let mut acknowledgements = BTreeMap::new();
        let mut acknowledged_actions = BTreeSet::new();
        for (ack_id, value) in self.persistence.list_records("host-ack/v1")? {
            let ack: HostAckPayload = serde_json::from_value(value)
                .map_err(|_| malformed("durable host ACK is malformed"))?;
            if ack.ack_id != ack_id || !acknowledged_actions.insert(ack.action_id.clone()) {
                return Err(malformed(
                    "durable host ACK identity is inconsistent or duplicated",
                ));
            }
            acknowledgements.insert(ack_id, ack);
        }

        let mut queues: BTreeMap<String, Vec<HostActionPayload>> = BTreeMap::new();
        let mut command_sequences = BTreeMap::new();
        for (action_id, value) in self.persistence.list_records("host-action/v1")? {
            let action: HostActionPayload = serde_json::from_value(value)
                .map_err(|_| malformed("durable host action is malformed"))?;
            if action.action_id != action_id || !registrations.contains_key(&action.adapter_id) {
                return Err(malformed(
                    "durable host action identity or adapter registration is inconsistent",
                ));
            }
            command_sequences
                .entry(action.adapter_id.clone())
                .and_modify(|current: &mut u64| *current = (*current).max(action.command_seq))
                .or_insert(action.command_seq);
            if !acknowledged_actions.contains(&action.action_id) {
                queues
                    .entry(action.adapter_id.clone())
                    .or_default()
                    .push(action);
            }
        }
        let queues = queues
            .into_iter()
            .map(|(adapter_id, mut actions)| {
                actions.sort_by_key(|action| action.command_seq);
                (adapter_id, actions.into_iter().collect::<VecDeque<_>>())
            })
            .collect();

        *self.registrations.lock().map_err(lock_error)? = registrations;
        *self.acknowledgements.lock().map_err(lock_error)? = acknowledgements;
        *self.command_sequences.lock().map_err(lock_error)? = command_sequences;
        *self.queues.lock().map_err(lock_error)? = queues;
        Ok(())
    }

    /// Registers the authenticated capability matrix for an adapter.
    pub fn register(&self, adapter_id: &str, capabilities: &[String]) -> RuntimeResult<()> {
        if adapter_id.is_empty() || capabilities.iter().any(String::is_empty) {
            return Err(RuntimeError::new(
                StableErrorCode::HostCapabilityUnsupported,
                "host adapter identity or capability is empty",
            ));
        }
        self.registrations.lock().map_err(lock_error)?.insert(
            adapter_id.to_owned(),
            capabilities.iter().cloned().collect(),
        );
        self.persistence.store_record(
            "host-adapter/v1",
            adapter_id,
            &json!({"schemaVersion":"host-adapter/v1","capabilities":capabilities}),
        )
    }

    /// Verifies that an authenticated adapter supports every required capability.
    pub fn require_capabilities(&self, adapter_id: &str, required: &[&str]) -> RuntimeResult<()> {
        let registrations = self.registrations.lock().map_err(lock_error)?;
        let capabilities = registrations.get(adapter_id).ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::HostCapabilityUnsupported,
                "host adapter is not registered",
            )
        })?;
        if required.iter().all(|item| capabilities.contains(*item)) {
            Ok(())
        } else {
            Err(RuntimeError::new(
                StableErrorCode::HostCapabilityUnsupported,
                "host adapter lacks a required native capability",
            ))
        }
    }

    /// Creates and durably enqueues a correlated host action.
    #[allow(clippy::too_many_arguments)]
    pub fn enqueue(
        &self,
        adapter_id: &str,
        kind: &str,
        delegation_id: Option<String>,
        compact_id: Option<String>,
        session_id: Option<String>,
        context_generation: Option<u64>,
        deadline_unix_ms: u64,
    ) -> RuntimeResult<HostActionPayload> {
        self.require_capabilities(adapter_id, &[kind])?;
        let command_seq = {
            let mut sequences = self.command_sequences.lock().map_err(lock_error)?;
            let value = sequences.entry(adapter_id.to_owned()).or_insert(0);
            *value = value.checked_add(1).ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "host command sequence overflow",
                )
            })?;
            *value
        };
        let action = HostActionPayload {
            action_id: Uuid::new_v4().to_string(),
            adapter_id: adapter_id.to_owned(),
            command_seq,
            kind: kind.to_owned(),
            delegation_id,
            compact_id,
            session_id,
            context_generation,
            deadline_unix_ms,
        };
        let value = serde_json::to_value(&action).map_err(canonical_error)?;
        self.persistence
            .store_record("host-action/v1", &action.action_id, &value)?;
        self.queues
            .lock()
            .map_err(lock_error)?
            .entry(adapter_id.to_owned())
            .or_default()
            .push_back(action.clone());
        Ok(action)
    }

    /// Returns the oldest unacknowledged action without consuming it.
    pub fn next(&self, adapter_id: &str) -> RuntimeResult<Option<HostActionPayload>> {
        Ok(self
            .queues
            .lock()
            .map_err(lock_error)?
            .get(adapter_id)
            .and_then(|queue| queue.front().cloned()))
    }

    /// Records one ACK idempotently after exact action/adapter/sequence correlation.
    pub fn acknowledge(
        &self,
        adapter_id: &str,
        ack: HostAckPayload,
    ) -> RuntimeResult<HostActionPayload> {
        if let Some(existing) = self
            .acknowledgements
            .lock()
            .map_err(lock_error)?
            .get(&ack.ack_id)
            .cloned()
        {
            if existing == ack {
                return self.action(&ack.action_id);
            }
            return Err(RuntimeError::new(
                StableErrorCode::IdempotencyKeyReused,
                "host ACK identity was replayed with different content",
            ));
        }
        let action = self.action(&ack.action_id)?;
        if action.adapter_id != adapter_id || action.command_seq != ack.command_seq {
            return Err(RuntimeError::new(
                StableErrorCode::DelegationAttestationFailed,
                "host ACK does not correlate to adapter/action/command sequence",
            ));
        }
        let value = serde_json::to_value(&ack).map_err(canonical_error)?;
        self.persistence
            .store_record("host-ack/v1", &ack.ack_id, &value)?;
        self.acknowledgements
            .lock()
            .map_err(lock_error)?
            .insert(ack.ack_id.clone(), ack);
        if let Some(queue) = self.queues.lock().map_err(lock_error)?.get_mut(adapter_id)
            && queue
                .front()
                .is_some_and(|queued| queued.action_id == action.action_id)
        {
            queue.pop_front();
        }
        Ok(action)
    }

    /// Reads a durable host action.
    pub fn action(&self, action_id: &str) -> RuntimeResult<HostActionPayload> {
        let value = self
            .persistence
            .load_record("host-action/v1", action_id)?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::DelegationAttestationFailed,
                    "host action does not exist",
                )
            })?;
        serde_json::from_value(value).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "durable host action is malformed",
            )
        })
    }

    /// Reads an ACK correlated to an action, if present.
    pub fn ack_for_action(&self, action_id: &str) -> RuntimeResult<Option<HostAckPayload>> {
        Ok(self
            .acknowledgements
            .lock()
            .map_err(lock_error)?
            .values()
            .find(|ack| ack.action_id == action_id)
            .cloned())
    }
}

fn canonical_error(_error: serde_json::Error) -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::ExternalStateConflict,
        "runtime record could not be canonicalized",
    )
}

fn malformed(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::ExternalStateConflict, message)
}

fn lock_error<T>(_error: std::sync::PoisonError<T>) -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::ExternalStateConflict,
        "runtime supervisor lock is poisoned",
    )
}
