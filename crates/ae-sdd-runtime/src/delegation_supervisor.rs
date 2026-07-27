#![allow(unused_imports)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use ae_sdd_context::{
    PressureDecision, PressurePolicy, PressureSample, PressureSource, PressureTracker,
};
use ae_sdd_domain::{
    AgentRole, ClaimId, CompactId, ContextGeneration, DelegationId, EventStoreId, HostAckId,
    HostActionId, InputFingerprint, SampleSequence, ScopedGrant, SessionId,
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
    RuntimeDelegationAttestationRecord, RuntimeDelegationHostActionRecord, RuntimeDelegationRecord,
    RuntimeError, RuntimeIdentityKind, RuntimeIdentitySnapshot, RuntimeIdentityTransition,
    RuntimeResult, RuntimeSessionRecord, RuntimeWorkspaceRecord, ScopedGrantWire, WireAgentRole,
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
    #[serde(default)]
    workspace_id: String,
    #[serde(default)]
    root_session_id: String,
    parent_session_id: String,
    parent_delegation_id: Option<String>,
    child_role: WireAgentRole,
    #[serde(default)]
    grant: ScopedGrantWire,
    input_revision: u64,
    input_fingerprint: String,
    deadline_unix_ms: u64,
    action_id: String,
    #[serde(default)]
    action_digest: String,
    #[serde(default)]
    created_at_unix_ms: u64,
    status: String,
    child_session_id: Option<String>,
    result_digest: Option<String>,
    summary: Option<String>,
    #[serde(default)]
    report_digest: Option<String>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    artifact_receipt: Option<Value>,
    #[serde(default)]
    cleanup_receipt: Option<Value>,
}

impl DelegationSupervisor {
    /// Creates a supervisor over durable records and Host action coordination.
    #[must_use]
    pub fn new(persistence: Arc<dyn PersistencePort>, host: Arc<HostCoordinator>) -> Self {
        Self { persistence, host }
    }

    /// Rebuilds the operational delegation projection from typed identity rows.
    pub fn recover(&self) -> RuntimeResult<()> {
        for snapshot in self
            .persistence
            .list_identity_snapshots(RuntimeIdentityKind::Delegation)?
        {
            let typed = snapshot.delegation.as_ref().ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "typed delegation snapshot lacks its delegation row",
                )
            })?;
            let binding = snapshot.host_action.as_ref().ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "typed delegation snapshot lacks its Host action binding",
                )
            })?;
            let projection: DelegationResult = serde_json::from_value(snapshot.response.clone())
                .map_err(|_| {
                    RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "typed delegation response is malformed",
                    )
                })?;
            if projection.delegation_id != typed.delegation_id
                || projection.action_id != binding.host_action_id
                || projection.child_role != typed.role
                || projection.child_session_id != typed.child_session_id
                || binding.workspace_id != typed.workspace_id
                || binding.delegation_id != typed.delegation_id
                || binding.parent_session_id != typed.parent_session_id
            {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "typed delegation projection and action binding disagree",
                ));
            }
            let grant = snapshot
                .attestation
                .as_ref()
                .map_or_else(|| projection.grant.clone(), |value| value.grant.clone());
            let existing = self
                .persistence
                .load_record("delegation/v1", &typed.delegation_id)?
                .map(serde_json::from_value::<DurableDelegation>)
                .transpose()
                .map_err(|_| {
                    RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "durable delegation record is malformed",
                    )
                })?;
            let mut record = existing.unwrap_or_else(|| DurableDelegation {
                schema_version: "delegation/v1".to_owned(),
                delegation_id: typed.delegation_id.clone(),
                workspace_id: typed.workspace_id.clone(),
                root_session_id: typed.root_session_id.clone(),
                parent_session_id: typed.parent_session_id.clone(),
                parent_delegation_id: typed.parent_delegation_id.clone(),
                child_role: typed.role,
                grant: grant.clone(),
                input_revision: typed.input_revision,
                input_fingerprint: typed.input_fingerprint.clone(),
                deadline_unix_ms: typed.deadline_unix_ms,
                action_id: binding.host_action_id.clone(),
                action_digest: binding.action_digest.clone(),
                created_at_unix_ms: typed.created_at_unix_ms,
                status: typed.status.clone(),
                child_session_id: typed.child_session_id.clone(),
                result_digest: None,
                summary: None,
                report_digest: None,
                result: None,
                artifact_receipt: None,
                cleanup_receipt: None,
            });
            if record.delegation_id != typed.delegation_id
                || (!record.workspace_id.is_empty() && record.workspace_id != typed.workspace_id)
                || (!record.root_session_id.is_empty()
                    && record.root_session_id != typed.root_session_id)
                || record.parent_session_id != typed.parent_session_id
                || record.parent_delegation_id != typed.parent_delegation_id
                || record.child_role != typed.role
                || record.input_revision != typed.input_revision
                || record.input_fingerprint != typed.input_fingerprint
                || record.deadline_unix_ms != typed.deadline_unix_ms
                || record.action_id != binding.host_action_id
                || (!record.action_digest.is_empty()
                    && record.action_digest != binding.action_digest)
                || record.grant.normalized()? != grant.normalized()?
            {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "operational delegation differs from typed identity authority",
                ));
            }
            if typed.status == "spawning" && record.status == "running" {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "operational delegation advanced without a typed acceptance",
                ));
            }
            record.workspace_id.clone_from(&typed.workspace_id);
            record.root_session_id.clone_from(&typed.root_session_id);
            record.child_session_id.clone_from(&typed.child_session_id);
            record.action_digest.clone_from(&binding.action_digest);
            if record.created_at_unix_ms == 0 {
                record.created_at_unix_ms = typed.created_at_unix_ms;
            }
            if matches!(record.status.as_str(), "spawning" | "running") {
                record.status.clone_from(&typed.status);
            }
            self.save(&record)?;
        }
        Ok(())
    }

    /// Creates a bounded delegation and dispatches a native Host create action.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &self,
        workspace_id: &str,
        parent_session_id: &str,
        parent_role: WireAgentRole,
        parent_grant: &ScopedGrant,
        payload: DelegationCreatePayload,
        scope_digest: &str,
        idempotency_key: &str,
        request_digest: &str,
        now_unix_ms: u64,
    ) -> RuntimeResult<(DelegationResult, bool)> {
        if !may_spawn(parent_role, payload.child_role) {
            return Err(RuntimeError::new(
                StableErrorCode::RunDepthExceeded,
                "requested child role exceeds the root-series-task/reviewer lineage",
            ));
        }
        let grant =
            crate::grant::validate_child_grant(parent_grant, payload.child_role, &payload.grant)?;
        self.host
            .require_capabilities(&payload.adapter_id, &["create", "attest"])?;
        let workspace = self.typed_workspace(workspace_id)?;
        let parent = self.typed_session(parent_session_id)?;
        if parent.workspace_id != workspace_id || parent.role != parent_role {
            return Err(attestation_error(
                "delegation parent does not match its typed session identity",
            ));
        }
        let root_session_id = match parent_role {
            WireAgentRole::Root if payload.parent_delegation_id.is_none() => {
                parent_session_id.to_owned()
            }
            WireAgentRole::Series => {
                let parent_delegation_id =
                    payload.parent_delegation_id.as_deref().ok_or_else(|| {
                        attestation_error("nested delegation requires its parent delegation")
                    })?;
                let parent_delegation = self.typed_delegation(parent_delegation_id)?;
                if parent_delegation.workspace_id != workspace_id
                    || parent_delegation.child_session_id.as_deref() != Some(parent_session_id)
                    || parent_delegation.role != parent_role
                    || parent_delegation.status != "running"
                {
                    return Err(attestation_error(
                        "nested delegation parent is not the attested running child",
                    ));
                }
                parent_delegation.root_session_id
            }
            _ => {
                return Err(attestation_error(
                    "root delegation cannot name a parent delegation",
                ));
            }
        };
        let delegation_id = stable_uuid("delegation", scope_digest, idempotency_key);
        let action_id = stable_uuid("delegation-host-action", scope_digest, idempotency_key);
        let action = self.host.enqueue_with_action_id(
            &action_id,
            &payload.adapter_id,
            "create",
            Some(delegation_id.clone()),
            None,
            None,
            None,
            payload.deadline_unix_ms,
        )?;
        let action_digest =
            canonical_wire_digest(&serde_json::to_value(&action).map_err(canonical_error)?)?;
        let record = DurableDelegation {
            schema_version: "delegation/v1".to_owned(),
            delegation_id: delegation_id.clone(),
            workspace_id: workspace_id.to_owned(),
            root_session_id: root_session_id.clone(),
            parent_session_id: parent_session_id.to_owned(),
            parent_delegation_id: payload.parent_delegation_id,
            child_role: payload.child_role,
            grant,
            input_revision: payload.input_revision,
            input_fingerprint: payload.input_fingerprint,
            deadline_unix_ms: payload.deadline_unix_ms,
            action_id: action.action_id.clone(),
            action_digest: action_digest.clone(),
            created_at_unix_ms: now_unix_ms,
            status: "spawning".to_owned(),
            child_session_id: None,
            result_digest: None,
            summary: None,
            report_digest: None,
            result: None,
            artifact_receipt: None,
            cleanup_receipt: None,
        };
        let projection = project_delegation(&record);
        let response = serde_json::to_value(&projection).map_err(canonical_error)?;
        let receipt_digest = canonical_wire_digest(&response)?;
        let committed = self
            .persistence
            .commit_identity_bundle(RuntimeIdentityTransition {
                operation: "delegation.create".to_owned(),
                scope_digest: scope_digest.to_owned(),
                idempotency_key: idempotency_key.to_owned(),
                request_digest: request_digest.to_owned(),
                expected_workspace_mode: Some(workspace.mode),
                expected_inventory_generation: Some(workspace.inventory_generation),
                expected_session_status: None,
                expected_delegation_status: None,
                expected_context_generation: None,
                snapshot: RuntimeIdentitySnapshot {
                    identity_kind: RuntimeIdentityKind::Delegation,
                    workspace,
                    session: None,
                    delegation: Some(RuntimeDelegationRecord {
                        delegation_id: delegation_id.clone(),
                        workspace_id: workspace_id.to_owned(),
                        root_session_id,
                        parent_session_id: parent_session_id.to_owned(),
                        child_session_id: None,
                        parent_delegation_id: record.parent_delegation_id.clone(),
                        role: record.child_role,
                        input_revision: record.input_revision,
                        input_fingerprint: record.input_fingerprint.clone(),
                        status: record.status.clone(),
                        deadline_unix_ms: record.deadline_unix_ms,
                        receipt_digest,
                        created_at_unix_ms: now_unix_ms,
                        updated_at_unix_ms: now_unix_ms,
                    }),
                    host_action: Some(RuntimeDelegationHostActionRecord {
                        workspace_id: workspace_id.to_owned(),
                        delegation_id,
                        host_action_id: action.action_id,
                        parent_session_id: parent_session_id.to_owned(),
                        action_digest,
                        created_at_unix_ms: now_unix_ms,
                    }),
                    attestation: None,
                    response,
                    replayed: false,
                },
                committed_at_unix_ms: now_unix_ms,
            })?;
        self.save(&record)?;
        let projection = serde_json::from_value(committed.response).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "typed delegation receipt response is malformed",
            )
        })?;
        Ok((projection, committed.replayed))
    }

    /// Establishes a physical child only after a correlated ACK and one-time claim.
    #[allow(clippy::too_many_arguments)]
    pub fn accept(
        &self,
        workspace_id: &str,
        work_item_id: Option<&str>,
        delegation_id: &str,
        claim_id: &str,
        action_id: &str,
        child_session_id: &str,
        expires_at_unix_ms: u64,
        accepted_boot_id: &str,
        scope_digest: &str,
        idempotency_key: &str,
        request_digest: &str,
        now_unix_ms: u64,
    ) -> RuntimeResult<(DelegationResult, bool)> {
        let mut record = self.load(delegation_id)?;
        let replay_candidate = record.status == "running"
            && record.child_session_id.as_deref() == Some(child_session_id);
        if (!replay_candidate && record.status != "spawning") || record.action_id != action_id {
            return Err(attestation_error("delegation is not awaiting this claim"));
        }
        if record.workspace_id != workspace_id {
            return Err(attestation_error(
                "delegation claim belongs to another workspace",
            ));
        }
        if expires_at_unix_ms > record.deadline_unix_ms {
            return Err(attestation_error(
                "delegation claim expiry exceeds the delegation deadline",
            ));
        }
        let prior = self.typed_delegation_snapshot(delegation_id)?;
        let prior_delegation = prior
            .delegation
            .as_ref()
            .ok_or_else(|| attestation_error("typed delegation snapshot lacks its delegation"))?;
        let binding = prior.host_action.as_ref().ok_or_else(|| {
            attestation_error("typed delegation snapshot lacks its Host action binding")
        })?;
        if prior_delegation.workspace_id != workspace_id
            || binding.workspace_id != workspace_id
            || binding.delegation_id != delegation_id
            || binding.host_action_id != action_id
            || binding.parent_session_id != record.parent_session_id
        {
            return Err(attestation_error(
                "typed delegation and Host action binding are inconsistent",
            ));
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
        let action_digest =
            canonical_wire_digest(&serde_json::to_value(&action_wire).map_err(canonical_error)?)?;
        let ack_digest =
            canonical_wire_digest(&serde_json::to_value(&ack_wire).map_err(canonical_error)?)?;
        if binding.action_digest != action_digest || record.action_digest != action_digest {
            return Err(attestation_error(
                "Host action digest differs from the typed delegation binding",
            ));
        }
        let claim_digest = canonical_wire_digest(&json!({
            "domain":"delegation-claim/v1",
            "workspaceId":workspace_id,
            "delegationId":delegation_id,
            "actionId":action_id,
            "childSessionId":child_session_id,
            "claimId":claim_id,
            "expiresAtUnixMs":expires_at_unix_ms,
        }))?;
        let grant = record.grant.normalized()?;
        let attestation_ref =
            format!("runtime-delegation:{workspace_id}:{delegation_id}:{child_session_id}");
        let attestation_digest = canonical_wire_digest(&json!({
            "domain":"runtime-delegation-attestation/v1",
            "workspaceId":workspace_id,
            "delegationId":delegation_id,
            "physicalSessionId":child_session_id,
            "hostActionId":action_id,
            "hostAckId":ack_wire.ack_id,
            "actionDigest":action_digest,
            "ackDigest":ack_digest,
            "claimDigest":claim_digest,
            "grant":grant,
            "attestationRef":attestation_ref,
            "acceptedBootId":accepted_boot_id,
            "acceptedAtUnixMs":now_unix_ms,
            "expiresAtUnixMs":expires_at_unix_ms,
        }))?;
        record.child_session_id = Some(child_session_id.to_owned());
        record.status = "running".to_owned();
        let projection = project_delegation(&record);
        let response = serde_json::to_value(&projection).map_err(canonical_error)?;
        let receipt_digest = canonical_wire_digest(&response)?;
        let placeholder_external_key_hash = canonical_wire_digest(&json!({
            "domain":"pending-delegated-session/v1",
            "workspaceId":workspace_id,
            "delegationId":delegation_id,
            "sessionId":child_session_id,
        }))?;
        let committed = self
            .persistence
            .commit_identity_bundle(RuntimeIdentityTransition {
                operation: "delegation.accept".to_owned(),
                scope_digest: scope_digest.to_owned(),
                idempotency_key: idempotency_key.to_owned(),
                request_digest: request_digest.to_owned(),
                expected_workspace_mode: Some(prior.workspace.mode),
                expected_inventory_generation: Some(prior.workspace.inventory_generation),
                expected_session_status: None,
                expected_delegation_status: Some("spawning".to_owned()),
                expected_context_generation: None,
                snapshot: RuntimeIdentitySnapshot {
                    identity_kind: RuntimeIdentityKind::Delegation,
                    workspace: prior.workspace,
                    session: Some(RuntimeSessionRecord {
                        session_id: child_session_id.to_owned(),
                        agent_id: format!("pending-delegation:{delegation_id}"),
                        workspace_id: workspace_id.to_owned(),
                        external_key_hash: placeholder_external_key_hash,
                        role: record.child_role,
                        root_session_id: record.root_session_id.clone(),
                        parent_session_id: Some(record.parent_session_id.clone()),
                        delegation_id: Some(delegation_id.to_owned()),
                        engaged: false,
                        current_work_item: work_item_id.map(str::to_owned),
                        grant: grant.clone(),
                        context_generation: 0,
                        expires_at_unix_ms,
                        status: "opening".to_owned(),
                        created_at_unix_ms: now_unix_ms,
                        updated_at_unix_ms: now_unix_ms,
                    }),
                    delegation: Some(RuntimeDelegationRecord {
                        delegation_id: delegation_id.to_owned(),
                        workspace_id: workspace_id.to_owned(),
                        root_session_id: record.root_session_id.clone(),
                        parent_session_id: record.parent_session_id.clone(),
                        child_session_id: Some(child_session_id.to_owned()),
                        parent_delegation_id: record.parent_delegation_id.clone(),
                        role: record.child_role,
                        input_revision: record.input_revision,
                        input_fingerprint: record.input_fingerprint.clone(),
                        status: record.status.clone(),
                        deadline_unix_ms: record.deadline_unix_ms,
                        receipt_digest,
                        created_at_unix_ms: prior_delegation.created_at_unix_ms,
                        updated_at_unix_ms: now_unix_ms,
                    }),
                    host_action: Some(binding.clone()),
                    attestation: Some(RuntimeDelegationAttestationRecord {
                        workspace_id: workspace_id.to_owned(),
                        delegation_id: delegation_id.to_owned(),
                        physical_session_id: child_session_id.to_owned(),
                        host_action_id: action_id.to_owned(),
                        host_ack_id: ack_wire.ack_id,
                        action_digest,
                        ack_digest,
                        claim_digest,
                        grant,
                        attestation_ref,
                        attestation_digest,
                        accepted_boot_id: accepted_boot_id.to_owned(),
                        accepted_at_unix_ms: now_unix_ms,
                        expires_at_unix_ms,
                    }),
                    response,
                    replayed: false,
                },
                committed_at_unix_ms: now_unix_ms,
            })?;
        self.save(&record)?;
        let projection = serde_json::from_value(committed.response).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "typed delegation receipt response is malformed",
            )
        })?;
        Ok((projection, committed.replayed))
    }

    /// Stages a bounded child result without pretending artifact or memory validation succeeded.
    pub fn report(
        &self,
        child_session_id: &str,
        payload: DelegationReportPayload,
    ) -> RuntimeResult<DelegationResult> {
        let mut record = self.load(&payload.delegation_id)?;
        if record.child_session_id.as_deref() != Some(child_session_id) {
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
        let report_digest = hex::encode(Sha256::digest(&canonical));
        if record.status != "running" {
            if matches!(
                record.status.as_str(),
                "result-staged" | "artifacts-validated" | "memory-cleaned" | "completed"
            ) && record.report_digest.as_deref() == Some(report_digest.as_str())
            {
                return Ok(project_delegation(&record));
            }
            return Err(RuntimeError::new(
                StableErrorCode::ChildResultInvalid,
                "delegation is not accepting this child result",
            ));
        }
        let memory_snapshot = payload
            .result
            .get("memorySnapshotDigest")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ChildResultInvalid,
                    "child result requires a memorySnapshotDigest",
                )
            })?;
        if !is_lower_hex_digest(memory_snapshot) {
            return Err(RuntimeError::new(
                StableErrorCode::ChildResultInvalid,
                "memorySnapshotDigest must be lowercase sha256",
            ));
        }
        let result_bytes = serde_json::to_vec(&payload.result).map_err(canonical_error)?;
        record.result_digest = Some(hex::encode(Sha256::digest(result_bytes)));
        record.summary = Some(payload.summary);
        record.report_digest = Some(report_digest);
        record.result = Some(payload.result);
        record.status = "result-staged".to_owned();
        self.save(&record)?;
        Ok(project_delegation(&record))
    }

    /// Records successful artifact validation from its dedicated verifier port.
    pub fn artifacts_validated(
        &self,
        delegation_id: &str,
        receipt: Value,
    ) -> RuntimeResult<DelegationResult> {
        let mut record = self.load(delegation_id)?;
        if record.status == "artifacts-validated"
            && record.artifact_receipt.as_ref() == Some(&receipt)
        {
            return Ok(project_delegation(&record));
        }
        if record.status != "result-staged"
            || receipt.get("schemaVersion").and_then(Value::as_str)
                != Some("delegation-artifact-validation/v1")
            || receipt.get("delegationId").and_then(Value::as_str) != Some(delegation_id)
            || receipt.get("resultDigest").and_then(Value::as_str)
                != record.result_digest.as_deref()
            || !receipt.get("artifacts").is_some_and(Value::is_array)
        {
            return Err(RuntimeError::new(
                StableErrorCode::ChildResultInvalid,
                "artifact validation receipt does not bind the staged child result",
            ));
        }
        record.artifact_receipt = Some(receipt);
        record.status = "artifacts-validated".to_owned();
        self.save(&record)?;
        Ok(project_delegation(&record))
    }

    /// Records durable child memory cleanup from its dedicated cleaner port.
    pub fn memory_cleaned(
        &self,
        delegation_id: &str,
        receipt: Value,
    ) -> RuntimeResult<DelegationResult> {
        let mut record = self.load(delegation_id)?;
        if record.status == "memory-cleaned" && record.cleanup_receipt.as_ref() == Some(&receipt) {
            return Ok(project_delegation(&record));
        }
        let snapshot = record
            .result
            .as_ref()
            .and_then(|result| result.get("memorySnapshotDigest"))
            .and_then(Value::as_str);
        if record.status != "artifacts-validated"
            || receipt.get("schemaVersion").and_then(Value::as_str)
                != Some("delegation-memory-cleanup/v1")
            || receipt.get("delegationId").and_then(Value::as_str) != Some(delegation_id)
            || receipt.get("memorySnapshotDigest").and_then(Value::as_str) != snapshot
            || !receipt
                .get("cleanupDigest")
                .and_then(Value::as_str)
                .is_some_and(is_lower_hex_digest)
            || receipt
                .get("cleanedAtUnixMs")
                .and_then(Value::as_u64)
                .is_none_or(|value| value == 0)
        {
            return Err(RuntimeError::new(
                StableErrorCode::ChildResultInvalid,
                "memory cleanup receipt does not bind the validated child result",
            ));
        }
        record.cleanup_receipt = Some(receipt);
        record.status = "memory-cleaned".to_owned();
        self.save(&record)?;
        Ok(project_delegation(&record))
    }

    /// Completes and returns the bounded root projection only after validation and cleanup.
    pub fn collect(&self, parent_session_id: &str, delegation_id: &str) -> RuntimeResult<Value> {
        let mut record = self.load(delegation_id)?;
        if record.parent_session_id != parent_session_id {
            return Err(RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "only the parent session may collect this delegation",
            ));
        }
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
    pub fn status(
        &self,
        requester_session_id: &str,
        delegation_id: &str,
    ) -> RuntimeResult<DelegationResult> {
        let record = self.load(delegation_id)?;
        if record.parent_session_id != requester_session_id
            && record.child_session_id.as_deref() != Some(requester_session_id)
        {
            return Err(RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "session is outside this delegation lineage",
            ));
        }
        Ok(project_delegation(&record))
    }

    /// Moves a non-completed delegation to a terminal cancellation state.
    pub fn cancel(
        &self,
        parent_session_id: &str,
        delegation_id: &str,
    ) -> RuntimeResult<DelegationResult> {
        let mut record = self.load(delegation_id)?;
        if record.parent_session_id != parent_session_id {
            return Err(RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "only the parent session may cancel this delegation",
            ));
        }
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

    /// Returns the staged result and any durable artifact receipt for completion orchestration.
    pub fn completion_material(
        &self,
        delegation_id: &str,
    ) -> RuntimeResult<(String, Value, Option<Value>)> {
        let record = self.load(delegation_id)?;
        let result = record.result.ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::ChildResultInvalid,
                "delegation has no staged child result",
            )
        })?;
        Ok((record.status, result, record.artifact_receipt))
    }

    pub(crate) fn root_session_id(&self, delegation_id: &str) -> RuntimeResult<String> {
        let mut current = delegation_id.to_owned();
        let mut visited = std::collections::BTreeSet::new();
        for _ in 0..3 {
            if !visited.insert(current.clone()) {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "delegation lineage contains a cycle",
                ));
            }
            let record = self.load(&current)?;
            match record.parent_delegation_id {
                Some(parent) => current = parent,
                None => return Ok(record.parent_session_id),
            }
        }
        Err(RuntimeError::new(
            StableErrorCode::RunDepthExceeded,
            "delegation lineage exceeds the supported three-layer model",
        ))
    }

    pub(crate) fn session_lineage(&self, delegation_id: &str) -> RuntimeResult<(String, String)> {
        let record = self.load(delegation_id)?;
        Ok((
            self.root_session_id(delegation_id)?,
            record.parent_session_id,
        ))
    }

    fn typed_workspace(&self, workspace_id: &str) -> RuntimeResult<RuntimeWorkspaceRecord> {
        self.persistence
            .list_identity_snapshots(RuntimeIdentityKind::Workspace)?
            .into_iter()
            .map(|snapshot| snapshot.workspace)
            .find(|workspace| workspace.workspace_id == workspace_id)
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "typed delegation references a missing workspace identity",
                )
            })
    }

    fn typed_session(&self, session_id: &str) -> RuntimeResult<RuntimeSessionRecord> {
        self.persistence
            .list_identity_snapshots(RuntimeIdentityKind::Session)?
            .into_iter()
            .filter_map(|snapshot| snapshot.session)
            .find(|session| session.session_id == session_id)
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::DelegationAttestationFailed,
                    "typed delegation references a missing parent session",
                )
            })
    }

    fn typed_delegation_snapshot(
        &self,
        delegation_id: &str,
    ) -> RuntimeResult<RuntimeIdentitySnapshot> {
        self.persistence
            .list_identity_snapshots(RuntimeIdentityKind::Delegation)?
            .into_iter()
            .find(|snapshot| {
                snapshot
                    .delegation
                    .as_ref()
                    .is_some_and(|delegation| delegation.delegation_id == delegation_id)
            })
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::DelegationAttestationFailed,
                    "typed delegation identity is missing",
                )
            })
    }

    fn typed_delegation(&self, delegation_id: &str) -> RuntimeResult<RuntimeDelegationRecord> {
        self.typed_delegation_snapshot(delegation_id)?
            .delegation
            .ok_or_else(|| attestation_error("typed delegation snapshot is malformed"))
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
        grant: record.grant.clone(),
        child_role: record.child_role,
        action_id: record.action_id.clone(),
        child_session_id: record.child_session_id.clone(),
        result_digest: record.result_digest.clone(),
    }
}

fn collect_projection(record: &DurableDelegation) -> Value {
    let result = record.result.as_ref().unwrap_or(&Value::Null);
    json!({
        "delegationId": record.delegation_id,
        "status": record.status,
        "summary": record.summary,
        "resultDigest": record.result_digest,
        "outcome":result.get("outcome"),
        "findings":result.get("findings").cloned().unwrap_or_else(|| json!([])),
        "requestedAction":result.get("requestedAction"),
        "artifacts":record.artifact_receipt.as_ref().and_then(|receipt| receipt.get("artifacts")).cloned().unwrap_or_else(|| json!([])),
        "artifactValidationReceipt":record.artifact_receipt,
        "memoryCleanupReceipt":record.cleanup_receipt,
    })
}

fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn canonical_wire_digest(value: &impl Serialize) -> RuntimeResult<String> {
    let bytes = serde_json::to_vec(value).map_err(canonical_error)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn stable_uuid(domain: &str, scope_digest: &str, idempotency_key: &str) -> String {
    let mut material =
        Vec::with_capacity(domain.len() + scope_digest.len() + idempotency_key.len() + 2);
    material.extend_from_slice(domain.as_bytes());
    material.push(0);
    material.extend_from_slice(scope_digest.as_bytes());
    material.push(0);
    material.extend_from_slice(idempotency_key.as_bytes());
    let digest = Sha256::digest(material);
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes).to_string()
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
