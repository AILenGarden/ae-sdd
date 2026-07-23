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

use super::host_coordinator::HostCoordinator;

use crate::{
    ContextProjectResult, ContextProjectionInput, DelegationCreatePayload, DelegationReportPayload,
    DelegationResult, HostAckPayload, HostActionPayload, HostPressurePayload, PersistencePort,
    RuntimeError, RuntimeResult, WireAgentRole,
};

/// Durable three-layer delegation lifecycle supervisor.
pub struct DelegationSupervisor {
    persistence: Arc<dyn PersistencePort>,
    host: Arc<HostCoordinator>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DurableDelegation {
    schema_version: String,
    delegation_id: String,
    parent_session_id: String,
    parent_delegation_id: Option<String>,
    child_role: WireAgentRole,
    input_revision: u64,
    input_fingerprint: String,
    deadline_unix_ms: u64,
    action_id: String,
    status: String,
    child_session_id: Option<String>,
    result_digest: Option<String>,
    summary: Option<String>,
}

impl DelegationSupervisor {
    /// Creates a supervisor over durable records and Host action coordination.
    #[must_use]
    pub fn new(persistence: Arc<dyn PersistencePort>, host: Arc<HostCoordinator>) -> Self {
        Self { persistence, host }
    }

    /// Creates a bounded delegation and dispatches a native Host create action.
    pub fn create(
        &self,
        parent_session_id: &str,
        parent_role: WireAgentRole,
        payload: DelegationCreatePayload,
    ) -> RuntimeResult<DelegationResult> {
        if !may_spawn(parent_role, payload.child_role) {
            return Err(RuntimeError::new(
                StableErrorCode::RunDepthExceeded,
                "requested child role exceeds the root-series-task/reviewer lineage",
            ));
        }
        self.host
            .require_capabilities(&payload.adapter_id, &["create", "attest"])?;
        let delegation_id = Uuid::new_v4().to_string();
        let action = self.host.enqueue(
            &payload.adapter_id,
            "create",
            Some(delegation_id.clone()),
            None,
            None,
            None,
            payload.deadline_unix_ms,
        )?;
        let record = DurableDelegation {
            schema_version: "delegation/v1".to_owned(),
            delegation_id: delegation_id.clone(),
            parent_session_id: parent_session_id.to_owned(),
            parent_delegation_id: payload.parent_delegation_id,
            child_role: payload.child_role,
            input_revision: payload.input_revision,
            input_fingerprint: payload.input_fingerprint,
            deadline_unix_ms: payload.deadline_unix_ms,
            action_id: action.action_id.clone(),
            status: "spawning".to_owned(),
            child_session_id: None,
            result_digest: None,
            summary: None,
        };
        self.save(&record)?;
        Ok(project_delegation(&record))
    }

    /// Establishes a physical child only after a correlated ACK and one-time claim.
    pub fn accept(
        &self,
        delegation_id: &str,
        claim_id: &str,
        action_id: &str,
        child_session_id: &str,
        expires_at_unix_ms: u64,
        now_unix_ms: u64,
    ) -> RuntimeResult<DelegationResult> {
        let mut record = self.load(delegation_id)?;
        if record.status == "running"
            && record.child_session_id.as_deref() == Some(child_session_id)
        {
            return Ok(project_delegation(&record));
        }
        if record.status != "spawning" || record.action_id != action_id {
            return Err(attestation_error("delegation is not awaiting this claim"));
        }
        let action_wire = self.host.action(action_id)?;
        let ack_wire = self
            .host
            .ack_for_action(action_id)?
            .ok_or_else(|| attestation_error("host ACK is required before child claim"))?;
        let action = host_action_from_wire(&action_wire)?;
        let ack = host_ack_from_wire(&action_wire.adapter_id, &ack_wire)?;
        let claim = ChildClaim::new(
            ClaimId::from_str(claim_id).map_err(|_| attestation_error("invalid claim identity"))?,
            DelegationId::from_str(delegation_id)
                .map_err(|_| attestation_error("invalid delegation identity"))?,
            HostActionId::from_str(action_id)
                .map_err(|_| attestation_error("invalid action identity"))?,
            SessionId::from_str(child_session_id)
                .map_err(|_| attestation_error("invalid child session identity"))?,
            expires_at_unix_ms,
        )
        .map_err(|_| attestation_error("invalid child claim"))?;
        let proof =
            PhysicalSessionProof::establish(&action, &ack, &claim, now_unix_ms).map_err(|_| {
                attestation_error("ACK and child claim do not establish physical proof")
            })?;
        if proof.child_session_id().to_string() != child_session_id {
            return Err(attestation_error(
                "physical child session identity mismatch",
            ));
        }
        record.child_session_id = Some(child_session_id.to_owned());
        record.status = "running".to_owned();
        self.save(&record)?;
        Ok(project_delegation(&record))
    }

    /// Stages a bounded child result without pretending artifact or memory validation succeeded.
    pub fn report(
        &self,
        child_session_id: &str,
        payload: DelegationReportPayload,
    ) -> RuntimeResult<DelegationResult> {
        let mut record = self.load(&payload.delegation_id)?;
        if record.status != "running"
            || record.child_session_id.as_deref() != Some(child_session_id)
        {
            return Err(RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "only the attested running child may report this delegation",
            ));
        }
        if record.input_revision != payload.input_revision
            || record.input_fingerprint != payload.input_fingerprint
        {
            return Err(RuntimeError::new(
                StableErrorCode::ChildResultInvalid,
                "child result input revision or fingerprint is stale",
            ));
        }
        if payload.summary.is_empty() || payload.summary.len() > 8_192 {
            return Err(RuntimeError::new(
                StableErrorCode::ChildResultTooLarge,
                "child result summary must be within 1..=8192 bytes",
            ));
        }
        reject_transcript_fields(&payload.result)?;
        let canonical = serde_json::to_vec(&payload).map_err(canonical_error)?;
        if canonical.len() > 65_536 {
            return Err(RuntimeError::new(
                StableErrorCode::ChildResultTooLarge,
                "canonical child result exceeds 65536 bytes",
            ));
        }
        record.result_digest = Some(hex::encode(Sha256::digest(&canonical)));
        record.summary = Some(payload.summary);
        record.status = "result-staged".to_owned();
        self.save(&record)?;
        Ok(project_delegation(&record))
    }

    /// Records successful artifact validation from its dedicated verifier port.
    pub fn artifacts_validated(&self, delegation_id: &str) -> RuntimeResult<DelegationResult> {
        self.advance(delegation_id, "result-staged", "artifacts-validated")
    }

    /// Records durable child memory cleanup from its dedicated cleaner port.
    pub fn memory_cleaned(&self, delegation_id: &str) -> RuntimeResult<DelegationResult> {
        self.advance(delegation_id, "artifacts-validated", "memory-cleaned")
    }

    /// Completes and returns the bounded root projection only after validation and cleanup.
    pub fn collect(&self, delegation_id: &str) -> RuntimeResult<Value> {
        let mut record = self.load(delegation_id)?;
        if record.status == "completed" {
            return Ok(collect_projection(&record));
        }
        if record.status != "memory-cleaned" {
            return Err(RuntimeError::new(
                StableErrorCode::ChildResultInvalid,
                "child result is not artifact-validated and memory-cleaned",
            ));
        }
        record.status = "completed".to_owned();
        self.save(&record)?;
        Ok(collect_projection(&record))
    }

    /// Reads the durable delegation lifecycle projection.
    pub fn status(&self, delegation_id: &str) -> RuntimeResult<DelegationResult> {
        self.load(delegation_id)
            .map(|record| project_delegation(&record))
    }

    /// Moves a non-completed delegation to a terminal cancellation state.
    pub fn cancel(&self, delegation_id: &str) -> RuntimeResult<DelegationResult> {
        let mut record = self.load(delegation_id)?;
        if record.status == "completed" {
            return Err(RuntimeError::new(
                StableErrorCode::JobNotCancellable,
                "completed delegation cannot be cancelled",
            ));
        }
        record.status = "cancelled".to_owned();
        self.save(&record)?;
        Ok(project_delegation(&record))
    }

    fn advance(&self, id: &str, from: &str, to: &str) -> RuntimeResult<DelegationResult> {
        let mut record = self.load(id)?;
        if record.status != from {
            return Err(RuntimeError::new(
                StableErrorCode::ChildResultInvalid,
                format!("delegation must be {from} before {to}"),
            ));
        }
        record.status = to.to_owned();
        self.save(&record)?;
        Ok(project_delegation(&record))
    }

    fn load(&self, id: &str) -> RuntimeResult<DurableDelegation> {
        let value = self
            .persistence
            .load_record("delegation/v1", id)?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ChildResultInvalid,
                    "delegation does not exist",
                )
            })?;
        serde_json::from_value(value).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "durable delegation record is malformed",
            )
        })
    }

    fn save(&self, record: &DurableDelegation) -> RuntimeResult<()> {
        let value = serde_json::to_value(record).map_err(canonical_error)?;
        self.persistence
            .store_record("delegation/v1", &record.delegation_id, &value)
    }
}

fn project_delegation(record: &DurableDelegation) -> DelegationResult {
    DelegationResult {
        delegation_id: record.delegation_id.clone(),
        status: record.status.clone(),
        child_role: record.child_role,
        action_id: record.action_id.clone(),
        child_session_id: record.child_session_id.clone(),
        result_digest: record.result_digest.clone(),
    }
}

fn collect_projection(record: &DurableDelegation) -> Value {
    json!({
        "delegationId": record.delegation_id,
        "status": record.status,
        "summary": record.summary,
        "resultDigest": record.result_digest,
    })
}

fn may_spawn(parent: WireAgentRole, child: WireAgentRole) -> bool {
    matches!(
        (parent, child),
        (WireAgentRole::Root, WireAgentRole::Series)
            | (
                WireAgentRole::Series,
                WireAgentRole::Task | WireAgentRole::Reviewer
            )
    )
}

fn host_action_from_wire(value: &HostActionPayload) -> RuntimeResult<HostAction> {
    let kind = match value.kind.as_str() {
        "create" => HostActionKind::Create,
        "send" => HostActionKind::Send,
        "wait" => HostActionKind::Wait,
        "cancel" => HostActionKind::Cancel,
        "attest" => HostActionKind::Attest,
        "compact" => HostActionKind::Compact,
        _ => return Err(attestation_error("unknown host action kind")),
    };
    HostAction::new(
        HostActionId::from_str(&value.action_id)
            .map_err(|_| attestation_error("invalid host action identity"))?,
        HostAdapterId::new(value.adapter_id.clone().into_boxed_str())
            .map_err(|_| attestation_error("invalid host adapter identity"))?,
        value.command_seq,
        kind,
        value
            .delegation_id
            .as_deref()
            .map(DelegationId::from_str)
            .transpose()
            .map_err(|_| attestation_error("invalid delegation binding"))?,
        value
            .compact_id
            .as_deref()
            .map(CompactId::from_str)
            .transpose()
            .map_err(|_| attestation_error("invalid compact binding"))?,
        value
            .session_id
            .as_deref()
            .map(SessionId::from_str)
            .transpose()
            .map_err(|_| attestation_error("invalid session binding"))?,
        value.context_generation.map(ContextGeneration::new),
        value.deadline_unix_ms,
        Sha256::digest(serde_json::to_vec(value).map_err(canonical_error)?).into(),
    )
    .map_err(|_| attestation_error("invalid durable host action"))
}

fn host_ack_from_wire(adapter_id: &str, value: &HostAckPayload) -> RuntimeResult<HostAck> {
    let outcome = match value.outcome.as_str() {
        "accepted" => HostAckOutcome::Accepted,
        "rejected" => HostAckOutcome::Rejected {
            error_code: "HOST_REJECTED".into(),
        },
        _ => return Err(attestation_error("unknown host ACK outcome")),
    };
    HostAck::new(
        HostAckId::from_str(&value.ack_id)
            .map_err(|_| attestation_error("invalid ACK identity"))?,
        HostActionId::from_str(&value.action_id)
            .map_err(|_| attestation_error("invalid action identity"))?,
        HostAdapterId::new(adapter_id.to_owned().into_boxed_str())
            .map_err(|_| attestation_error("invalid adapter identity"))?,
        value.command_seq,
        outcome,
        value
            .host_task_id
            .as_ref()
            .map(|item| HostTaskId::new(item.clone().into_boxed_str()))
            .transpose()
            .map_err(|_| attestation_error("invalid host task identity"))?,
        value
            .session_id
            .as_deref()
            .map(SessionId::from_str)
            .transpose()
            .map_err(|_| attestation_error("invalid ACK session identity"))?,
    )
    .map_err(|_| attestation_error("invalid host ACK"))
}

fn reject_transcript_fields(value: &Value) -> RuntimeResult<()> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if matches!(
                    key.as_str(),
                    "transcript" | "source" | "sourceCode" | "fullStdout" | "fullStderr"
                ) {
                    return Err(RuntimeError::new(
                        StableErrorCode::ChildResultInvalid,
                        "child result contains forbidden unbounded content",
                    ));
                }
                reject_transcript_fields(child)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                reject_transcript_fields(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn canonical_error(_error: serde_json::Error) -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::ExternalStateConflict,
        "runtime record could not be canonicalized",
    )
}

fn attestation_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::DelegationAttestationFailed, message)
}
