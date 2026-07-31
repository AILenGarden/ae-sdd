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
    /// Adapter IDs the daemon knows how to address. Membership answers "is
    /// there such a recipient", not "is that recipient permitted": the daemon
    /// posts errands and learns what a host could actually do from the ACK
    /// outcome.
    registrations: Mutex<BTreeSet<String>>,
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
            registrations: Mutex::new(BTreeSet::new()),
            queues: Mutex::new(BTreeMap::new()),
            acknowledgements: Mutex::new(BTreeMap::new()),
            command_sequences: Mutex::new(BTreeMap::new()),
        }
    }

    /// Restores durable adapter registrations, ACKs, command sequences, and pending queues.
    ///
    /// Records written before capabilities were dropped still carry the field.
    /// It is ignored rather than rejected: the row's only remaining purpose is
    /// to name a reachable adapter, so an old row is a perfectly good one.
    pub fn recover(&self) -> RuntimeResult<()> {
        let mut registrations = BTreeSet::new();
        for (adapter_id, _) in self.persistence.list_records("host-adapter/v1")? {
            registrations.insert(adapter_id);
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
            if action.action_id != action_id || !registrations.contains(&action.adapter_id) {
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

    /// Records an adapter as addressable.
    ///
    /// This is bookkeeping for delivery, not a grant. Nothing here is checked
    /// against what the host can really do, because nothing could be: the host
    /// runs the errand in its own process and reports back.
    pub fn register(&self, adapter_id: &str) -> RuntimeResult<()> {
        if adapter_id.is_empty() {
            return Err(RuntimeError::new(
                StableErrorCode::HostCapabilityUnsupported,
                "host adapter identity is empty",
            ));
        }
        self.registrations
            .lock()
            .map_err(lock_error)?
            .insert(adapter_id.to_owned());
        self.persistence.store_record(
            "host-adapter/v1",
            adapter_id,
            &json!({"schemaVersion":"host-adapter/v1"}),
        )
    }

    /// Ensures the daemon has somewhere to deliver an errand for `adapter_id`.
    ///
    /// A failure here means the recipient is unknown, so the errand could not
    /// be delivered at all. The ID is named in the message because with several
    /// hosts attached, "not registered" alone does not say which one is missing.
    pub fn require_registered(&self, adapter_id: &str) -> RuntimeResult<()> {
        if self
            .registrations
            .lock()
            .map_err(lock_error)?
            .contains(adapter_id)
        {
            Ok(())
        } else {
            Err(RuntimeError::new(
                StableErrorCode::HostCapabilityUnsupported,
                format!("host adapter {adapter_id} is not registered"),
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
        self.enqueue_with_action_id(
            &Uuid::new_v4().to_string(),
            adapter_id,
            kind,
            delegation_id,
            compact_id,
            session_id,
            context_generation,
            deadline_unix_ms,
        )
    }

    /// Creates a Host action with a caller-derived stable identity.
    ///
    /// Delegation creation uses this path so a crash between action persistence
    /// and the typed identity commit cannot allocate a second Host command.
    #[allow(clippy::too_many_arguments)]
    pub fn enqueue_with_action_id(
        &self,
        action_id: &str,
        adapter_id: &str,
        kind: &str,
        delegation_id: Option<String>,
        compact_id: Option<String>,
        session_id: Option<String>,
        context_generation: Option<u64>,
        deadline_unix_ms: u64,
    ) -> RuntimeResult<HostActionPayload> {
        let action = self.stage_with_action_id(
            action_id,
            adapter_id,
            kind,
            delegation_id,
            compact_id,
            session_id,
            context_generation,
            deadline_unix_ms,
        )?;
        self.publish(&action)?;
        Ok(action)
    }

    /// Durably stages an action without publishing it to its adapter queue.
    ///
    /// Callers that must commit other authoritative state before the host may
    /// observe an action stage it first and publish only after that commit
    /// succeeds. A staged-but-unpublished action is invisible to `next`, so a
    /// failed commit cannot leave behind an action the host can acknowledge but
    /// never claim.
    #[allow(clippy::too_many_arguments)]
    pub fn stage_with_action_id(
        &self,
        action_id: &str,
        adapter_id: &str,
        kind: &str,
        delegation_id: Option<String>,
        compact_id: Option<String>,
        session_id: Option<String>,
        context_generation: Option<u64>,
        deadline_unix_ms: u64,
    ) -> RuntimeResult<HostActionPayload> {
        self.require_registered(adapter_id)?;
        if let Some(value) = self.persistence.load_record("host-action/v1", action_id)? {
            let existing: HostActionPayload = serde_json::from_value(value)
                .map_err(|_| malformed("durable host action is malformed"))?;
            if existing.action_id != action_id
                || existing.adapter_id != adapter_id
                || existing.kind != kind
                || existing.delegation_id != delegation_id
                || existing.compact_id != compact_id
                || existing.session_id != session_id
                || existing.context_generation != context_generation
                || existing.deadline_unix_ms != deadline_unix_ms
            {
                return Err(RuntimeError::new(
                    StableErrorCode::IdempotencyKeyReused,
                    "stable host action identity was reused with different content",
                ));
            }
            return Ok(existing);
        }
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
            action_id: action_id.to_owned(),
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
        Ok(action)
    }

    /// Publishes a durably staged action to its adapter queue.
    ///
    /// Publishing is idempotent: an action already queued, or already
    /// acknowledged, is not enqueued a second time.
    pub fn publish(&self, action: &HostActionPayload) -> RuntimeResult<()> {
        if self.ack_for_action(&action.action_id)?.is_some() {
            return Ok(());
        }
        let mut queues = self.queues.lock().map_err(lock_error)?;
        let queue = queues.entry(action.adapter_id.clone()).or_default();
        if queue
            .iter()
            .any(|queued| queued.action_id == action.action_id)
        {
            return Ok(());
        }
        queue.push_back(action.clone());
        queue.make_contiguous().sort_by_key(|queued| queued.command_seq);
        Ok(())
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
        // One action carries at most one ACK. `recover` enforces this over the
        // durable records, so accepting a second ACK under a fresh identity
        // writes state the daemon refuses to load: the next restart fails
        // permanently. Rejecting the write is what keeps the two paths agreeing.
        if let Some(existing) = self.ack_for_action(&ack.action_id)?
            && existing.ack_id != ack.ack_id
        {
            return Err(RuntimeError::new(
                StableErrorCode::IdempotencyKeyReused,
                "host action is already acknowledged under a different ACK identity",
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
        // Selection must not depend on `ack_id` ordering. The map is keyed by
        // `ack_id`, so a plain `find` would return whichever identity sorts
        // first, letting a caller decide which of several ACKs wins by choosing
        // its identifier. An accepted ACK carrying the host task binding is the
        // only one that can establish physical proof, so prefer it.
        let acknowledgements = self.acknowledgements.lock().map_err(lock_error)?;
        let mut candidates = acknowledgements
            .values()
            .filter(|ack| ack.action_id == action_id)
            .peekable();
        let mut fallback = None;
        for ack in candidates.by_ref() {
            let usable = ack.outcome == "accepted"
                && ack.host_task_id.is_some()
                && ack.session_id.is_some();
            if usable {
                return Ok(Some(ack.clone()));
            }
            if fallback.is_none() {
                fallback = Some(ack.clone());
            }
        }
        Ok(fallback)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::MemoryPersistence;
    use ae_sdd_domain::EventStoreId;
    use uuid::Uuid;

    fn coordinator() -> HostCoordinator {
        let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
            Uuid::from_u128(7),
        )));
        let coordinator = HostCoordinator::new(persistence);
        coordinator
            .register("host-a")
            .expect("adapter registers");
        coordinator
    }

    /// A row written before capabilities were dropped still carries them. All
    /// the row is for is naming a reachable adapter, so an old one is a
    /// perfectly good one -- rejecting it would strand a daemon on restart over
    /// a field nothing reads.
    #[test]
    fn a_record_still_carrying_capabilities_recovers_as_addressable() {
        let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
            Uuid::from_u128(11),
        )));
        persistence
            .store_record(
                "host-adapter/v1",
                "host-legacy",
                &json!({
                    "schemaVersion":"host-adapter/v1",
                    "capabilities":["create","attest","ack"]
                }),
            )
            .expect("legacy record stores");

        let coordinator = HostCoordinator::new(persistence);
        coordinator.recover().expect("legacy record recovers");

        coordinator
            .require_registered("host-legacy")
            .expect("a recovered adapter is addressable");
    }

    /// Staging is what makes the create path atomic from the host's point of
    /// view: the action is durable, so its command sequence is fixed and its
    /// digest can be committed alongside the delegation, but the host cannot see
    /// it yet. If the commit then fails, nothing was ever offered.
    #[test]
    fn staging_an_action_does_not_offer_it_to_the_host() {
        let coordinator = coordinator();
        let action = coordinator
            .stage_with_action_id(
                "action-1",
                "host-a",
                "create",
                Some("delegation-1".to_owned()),
                None,
                None,
                None,
                2_000,
            )
            .expect("action stages");

        assert_eq!(action.command_seq, 1, "staging assigns the command sequence");
        assert!(
            coordinator.next("host-a").expect("queue reads").is_none(),
            "a staged action must not be visible before its commit succeeds"
        );
        assert_eq!(
            coordinator.action("action-1").expect("record reads").kind,
            "create",
            "a staged action must still be durable"
        );
    }

    #[test]
    fn publishing_a_staged_action_offers_it_exactly_once() {
        let coordinator = coordinator();
        let action = coordinator
            .stage_with_action_id(
                "action-1",
                "host-a",
                "create",
                Some("delegation-1".to_owned()),
                None,
                None,
                None,
                2_000,
            )
            .expect("action stages");

        coordinator.publish(&action).expect("first publish");
        coordinator.publish(&action).expect("republish is tolerated");

        let offered = coordinator
            .next("host-a")
            .expect("queue reads")
            .expect("the published action is offered");
        assert_eq!(offered.action_id, "action-1");

        coordinator
            .acknowledge(
                "host-a",
                HostAckPayload {
                    ack_id: "ack-1".to_owned(),
                    action_id: "action-1".to_owned(),
                    command_seq: action.command_seq,
                    outcome: "accepted".to_owned(),
                    host_task_id: Some("task-1".to_owned()),
                    session_id: Some("session-1".to_owned()),
                },
            )
            .expect("ack consumes the queued action");

        assert!(
            coordinator.next("host-a").expect("queue reads").is_none(),
            "a republished action must not be offered a second time"
        );
    }

    /// `recover` admits at most one durable ACK per action, so the write path
    /// must refuse a second one. Accepting it produced state the daemon could
    /// not load: every subsequent start failed on the duplicate, which any host
    /// could trigger by re-acknowledging under a fresh identity.
    #[test]
    fn a_second_ack_under_a_new_identity_is_refused() {
        let coordinator = coordinator();
        let action = coordinator
            .enqueue_with_action_id(
                "action-1",
                "host-a",
                "create",
                Some("delegation-1".to_owned()),
                None,
                None,
                None,
                2_000,
            )
            .expect("action enqueues");
        let ack = |ack_id: &str, host_task: Option<&str>| HostAckPayload {
            ack_id: ack_id.to_owned(),
            action_id: "action-1".to_owned(),
            command_seq: action.command_seq,
            outcome: "accepted".to_owned(),
            host_task_id: host_task.map(str::to_owned),
            session_id: Some("session-1".to_owned()),
        };

        coordinator
            .acknowledge("host-a", ack("ack-first", None))
            .expect("the first ack is recorded");
        let error = coordinator
            .acknowledge("host-a", ack("ack-second", Some("task-1")))
            .expect_err("a competing ack identity must be refused");
        assert_eq!(error.code(), StableErrorCode::IdempotencyKeyReused);

        coordinator
            .acknowledge("host-a", ack("ack-first", None))
            .expect("replaying the same ack stays idempotent");
        assert_eq!(
            coordinator
                .ack_for_action("action-1")
                .expect("selection succeeds")
                .expect("an ack exists")
                .ack_id,
            "ack-first",
            "the recorded ack must remain the only one"
        );
    }

    /// Durable records written before the write path was constrained can still
    /// hold several ACKs for one action. Selection must not depend on `ack_id`
    /// ordering there either: only an accepted ACK carrying the host task and
    /// session bindings can establish physical proof.
    #[test]
    fn ack_selection_prefers_the_usable_ack_over_identity_order() {
        let coordinator = coordinator();
        let action = coordinator
            .enqueue_with_action_id(
                "action-1",
                "host-a",
                "create",
                Some("delegation-1".to_owned()),
                None,
                None,
                None,
                2_000,
            )
            .expect("action enqueues");

        // Seed the in-memory map directly: the write path now refuses a second
        // ACK, but stores written before that constraint still carry pairs like
        // this one, and `ack_for_action` has to pick correctly over them.
        {
            let mut acknowledgements = coordinator
                .acknowledgements
                .lock()
                .expect("ack map is available");
            for (ack_id, host_task) in [("aaaa-first", None), ("zzzz-last", Some("task-1"))] {
                acknowledgements.insert(
                    ack_id.to_owned(),
                    HostAckPayload {
                        ack_id: ack_id.to_owned(),
                        action_id: "action-1".to_owned(),
                        command_seq: action.command_seq,
                        outcome: "accepted".to_owned(),
                        host_task_id: host_task.map(str::to_owned),
                        session_id: Some("session-1".to_owned()),
                    },
                );
            }
        }

        let selected = coordinator
            .ack_for_action("action-1")
            .expect("selection succeeds")
            .expect("an ack exists");
        assert_eq!(
            selected.ack_id, "zzzz-last",
            "the usable ack must win even though its identity sorts last"
        );
    }
}
