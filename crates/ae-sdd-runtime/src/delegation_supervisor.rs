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
use crate::host_execution_binding::{HOST_EXECUTION_BINDING_V1, HostExecutionBindingLedger};

use crate::{
    AssetRefWire, ClockPort, ContextProjectResult, ContextProjectionInput, DelegationCreatePayload,
    DelegationReportPayload, DelegationResult, DurableEvent, HostAckPayload, HostActionPayload,
    HostPressurePayload, PersistencePort, RuntimeDelegationAttestationRecord,
    RuntimeDelegationHostActionRecord, RuntimeDelegationRecord, RuntimeError, RuntimeIdentityKind,
    RuntimeIdentitySnapshot, RuntimeIdentityTransition, RuntimeResult, RuntimeSessionRecord,
    RuntimeWorkspaceRecord, ScopedGrantWire, WireAgentRole,
};

/// Durable three-layer delegation lifecycle supervisor.
pub struct DelegationSupervisor {
    persistence: Arc<dyn PersistencePort>,
    host: Arc<HostCoordinator>,
    /// UUID host-execution binding ledger (§9.4). Shared with `RuntimeService`
    /// so the Hook fast path and session-close can refresh/release the same
    /// in-memory state without an extra durability hop.
    bindings: Arc<HostExecutionBindingLedger>,
    /// Injected clock so collect/cancel (which have no `now` parameter today)
    /// can stamp `released_at_unix_ms` without changing their call signatures.
    clock: Arc<dyn ClockPort>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DurableDelegation {
    schema_version: String,
    delegation_id: String,
    #[serde(default)]
    workspace_id: String,
    #[serde(default)]
    work_item_id: String,
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
    /// `ae-sdd-daemon-design.md` §4.1 / audit F-06: the identity of *this
    /// attempt* at the logical Series. A `DelegationId` cannot stand in for it —
    /// a retry produces a new delegation but stays the same logical Series, so
    /// without a separate run identity the two attempts are only distinguishable
    /// as unrelated delegations.
    ///
    /// `#[serde(default)]` so records written before this field remain readable;
    /// D-03 forbids treating missing data as an empty rebuild, and an older
    /// record legitimately has no attempt identity rather than a blank one.
    #[serde(default)]
    series_run_id: String,
    /// The stable logical Series every attempt of it shares.
    ///
    /// Separate from `series_run_id` so "all attempts of this Series" is answerable
    /// from a stored field rather than a walk of `retry_of` edges. The lookup itself
    /// lives in the `series_run/v1` projection, which is keyed per attempt.
    #[serde(default)]
    series_id: String,
    /// The Flow Run this attempt belongs to (§4.2 `FR -> Series Run`).
    ///
    /// `#[serde(default)]` so delegations written before run identity remain
    /// readable; absent means "this delegation predates Flow Run identity", which
    /// D-03 item 6 requires stay distinct from an empty identity.
    #[serde(default)]
    flow_run_id: Option<String>,
    /// The `seriesRunId` this attempt replaces, absent on a first attempt.
    ///
    /// F-06 requires this to survive so "this Series was retried twice" is still
    /// answerable after a restart.
    #[serde(default)]
    retry_of: Option<String>,
    /// Optional bounded briefing the child series was created with.
    #[serde(default)]
    briefing: Option<String>,
    /// Optional bounded asset references the child series was created with.
    #[serde(default)]
    asset_refs: Option<Vec<AssetRefWire>>,
    action_id: String,
    #[serde(default)]
    action_digest: String,
    /// Digest of the daemon-issued, boot-local claim and its authority binding.
    #[serde(default)]
    claim_digest: String,
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

enum ReportAdmission {
    New {
        report_digest: String,
        result_digest: String,
    },
    Replay,
}

/// Derives the *stable* logical Series identity.
///
/// D-03 item 3 separates this from the per-attempt run: every retry of one Series
/// must resolve to the same value here, so it is derived from the facts that
/// identify the Series rather than from anything attempt-specific.
///
/// Scope caveat: the flow decision currently names only a `seriesKind`, so this
/// derives from Work Item plus kind. `ae-sdd-daemon-design.md` §7 runs Story and
/// TestCase Series once *per Story*, which this cannot yet distinguish — those
/// Series would collide on one identity. The extra dimension is deliberately not
/// invented here, because the flow does not emit a Story target yet and a
/// fabricated one would be wrong in a way no test could catch. When the target
/// arrives it joins this derivation.
pub fn series_identity(work_item_id: &str, series_kind: &str) -> String {
    let digest =
        Sha256::digest(format!("ae-sdd/series/v1:{work_item_id}:{series_kind}").as_bytes());
    Uuid::from_slice(&digest[..16])
        .expect("sha256 yields at least 16 bytes")
        .to_string()
}

/// Derives the attempt identity from the daemon-issued delegation id.
///
/// Deterministic so an idempotent replay of one `delegation.create` yields the
/// same attempt rather than a second phantom run, and namespaced so it can never
/// collide with a delegation id in a query.
fn series_run_identity(delegation_id: &str) -> String {
    let digest = Sha256::digest(format!("ae-sdd/series-run/v1:{delegation_id}").as_bytes());
    Uuid::from_slice(&digest[..16])
        .expect("sha256 yields at least 16 bytes")
        .to_string()
}

impl DelegationSupervisor {
    /// Creates a supervisor over durable records and Host action coordination.
    #[must_use]
    pub fn new(
        persistence: Arc<dyn PersistencePort>,
        host: Arc<HostCoordinator>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        let bindings = Arc::new(HostExecutionBindingLedger::empty());
        bindings.attach_persistence(Arc::clone(&persistence));
        Self {
            persistence,
            host,
            bindings,
            clock,
        }
    }

    /// Read-only access to the binding ledger, for the Hook fast path and
    /// session-close to refresh/release on the same in-memory map.
    #[must_use]
    pub(crate) fn bindings(&self) -> &HostExecutionBindingLedger {
        &self.bindings
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
                work_item_id: typed.work_item_id.clone().unwrap_or_default(),
                root_session_id: typed.root_session_id.clone(),
                parent_session_id: typed.parent_session_id.clone(),
                parent_delegation_id: typed.parent_delegation_id.clone(),
                // Reconstructed from a typed projection, so it derives the same
                // attempt identity the create path would have. The projection
                // carries no retry edge, and inventing one would assert a
                // predecessor that may not exist — D-03 forbids filling missing
                // data with a fabricated value.
                series_run_id: series_run_identity(&typed.delegation_id),
                // A typed projection carries no Series kind, so the stable Series
                // is genuinely unknown on this path. Left empty rather than
                // derived from a guess: D-03 forbids substituting fabricated data
                // for missing data, and a wrong Series id would silently merge two
                // unrelated Series into one query result.
                series_id: String::new(),
                retry_of: None,
                // Absent for the same reason `series_id` is empty: a typed
                // projection carries no Flow Run, and attaching this attempt to a
                // guessed run would put it in the wrong branch of §4.2's execution
                // tree. `None` records "unknown", which is the truth here.
                flow_run_id: None,
                child_role: typed.role,
                grant: grant.clone(),
                input_revision: typed.input_revision,
                input_fingerprint: typed.input_fingerprint.clone(),
                deadline_unix_ms: typed.deadline_unix_ms,
                briefing: projection.briefing.clone(),
                asset_refs: projection.asset_refs.clone(),
                action_id: binding.host_action_id.clone(),
                action_digest: binding.action_digest.clone(),
                claim_digest: String::new(),
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
            // A renewal in the pre-fix release advanced only the rebuildable
            // projection. Promote that value only when the committed event
            // proves it came from the daemon's renewal path; otherwise keep
            // recovery fail-closed on an unproven external mutation.
            let mut authoritative_deadline = typed.deadline_unix_ms;
            if record.deadline_unix_ms > authoritative_deadline {
                let event = self.committed_renewal_event(&record)?.ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "operational delegation differs from typed identity authority",
                    )
                })?;
                self.promote_recovered_renewal(&snapshot, &record, &event)?;
                authoritative_deadline = record.deadline_unix_ms;
            }
            if record.delegation_id != typed.delegation_id
                || (!record.workspace_id.is_empty() && record.workspace_id != typed.workspace_id)
                || typed.work_item_id.as_ref().is_some_and(|work_item_id| {
                    !record.work_item_id.is_empty() && record.work_item_id != *work_item_id
                })
                || (!record.root_session_id.is_empty()
                    && record.root_session_id != typed.root_session_id)
                || record.parent_session_id != typed.parent_session_id
                || record.parent_delegation_id != typed.parent_delegation_id
                || record.child_role != typed.role
                || record.input_revision != typed.input_revision
                || record.input_fingerprint != typed.input_fingerprint
                || record.deadline_unix_ms > authoritative_deadline
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
            if record.work_item_id.is_empty() {
                record.work_item_id = typed.work_item_id.clone().unwrap_or_default();
            }
            record.root_session_id.clone_from(&typed.root_session_id);
            record.child_session_id.clone_from(&typed.child_session_id);
            record.action_digest.clone_from(&binding.action_digest);
            record.deadline_unix_ms = authoritative_deadline;
            if record.created_at_unix_ms == 0 {
                record.created_at_unix_ms = typed.created_at_unix_ms;
            }
            if matches!(record.status.as_str(), "spawning" | "running") {
                record.status.clone_from(&typed.status);
            }
            if record.status == "completed" {
                self.save(&record)?;
                self.bindings.release_by_delegation(
                    &record.delegation_id,
                    "collected",
                    self.clock.now_unix_ms(),
                )?;
                continue;
            }
            let recovered_ack = self.host.ack_for_action(&record.action_id)?;
            if matches!(record.status.as_str(), "spawning" | "cancelled")
                && recovered_ack
                    .as_ref()
                    .is_some_and(|ack| ack.outcome == "rejected")
            {
                if record.status == "spawning" {
                    record.status = "cancelled".to_owned();
                }
                self.save(&record)?;
                self.bindings.release_by_delegation(
                    &record.delegation_id,
                    "cancelled",
                    self.clock.now_unix_ms(),
                )?;
                continue;
            }
            let recovered_claim = if record.status == "spawning" && recovered_ack.is_none() {
                let claim_id = Uuid::new_v4().to_string();
                record.claim_digest = delegation_claim_digest(
                    &claim_id,
                    &record.workspace_id,
                    &record.delegation_id,
                    &record.action_id,
                    record.child_role,
                    &record.parent_session_id,
                    record.deadline_unix_ms,
                )?;
                Some(claim_id)
            } else {
                None
            };
            self.save(&record)?;
            if let Some(claim_id) = recovered_claim {
                self.host.attach_claim(&record.action_id, claim_id)?;
            }
        }
        Ok(())
    }

    fn committed_renewal_event(
        &self,
        record: &DurableDelegation,
    ) -> RuntimeResult<Option<DurableEvent>> {
        const EVENT_PAGE_SIZE: usize = 4_096;
        let expected_scope = format!("delegation-renew\0{}", record.delegation_id);
        let mut after = 0;
        let mut found = None;
        loop {
            let page = self.persistence.events_after(after, EVENT_PAGE_SIZE)?;
            if page.is_empty() {
                break;
            }
            for event in &page {
                if event.kind != "delegation.renewed"
                    || event.payload.get("scope").and_then(Value::as_str)
                        != Some(expected_scope.as_str())
                {
                    continue;
                }
                if event.workspace_id.as_deref() != Some(record.workspace_id.as_str())
                    || event.session_id.as_deref() != Some(record.parent_session_id.as_str())
                {
                    return Err(RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "renewal event identity differs from the operational delegation",
                    ));
                }
                let key = event
                    .payload
                    .get("key")
                    .and_then(Value::as_str)
                    .filter(|key| !key.is_empty())
                    .ok_or_else(|| {
                        RuntimeError::new(
                            StableErrorCode::ExternalStateConflict,
                            "renewal event lacks its receipt key",
                        )
                    })?;
                let receipt = self
                    .persistence
                    .load_receipt(&expected_scope, key)?
                    .ok_or_else(|| {
                        RuntimeError::new(
                            StableErrorCode::ExternalStateConflict,
                            "renewal event lacks its atomic receipt",
                        )
                    })?;
                if receipt.event_seq != event.event_seq {
                    return Err(RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "renewal receipt points to a different durable event",
                    ));
                }
                let response: Value =
                    serde_json::from_str(&receipt.response_json).map_err(|_| {
                        RuntimeError::new(
                            StableErrorCode::ExternalStateConflict,
                            "renewal receipt response is malformed",
                        )
                    })?;
                if response.get("delegationId").and_then(Value::as_str)
                    != Some(record.delegation_id.as_str())
                {
                    return Err(RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "renewal receipt names a different delegation",
                    ));
                }
                if response.get("deadlineUnixMs").and_then(Value::as_u64)
                    == Some(record.deadline_unix_ms)
                {
                    found = Some(event.clone());
                }
            }
            after = page.last().map_or(after, |event| event.event_seq);
            if page.len() < EVENT_PAGE_SIZE {
                break;
            }
        }
        Ok(found)
    }

    fn promote_recovered_renewal(
        &self,
        snapshot: &RuntimeIdentitySnapshot,
        record: &DurableDelegation,
        event: &DurableEvent,
    ) -> RuntimeResult<()> {
        let mut delegation = snapshot.delegation.clone().ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "typed delegation snapshot lacks its delegation row",
            )
        })?;
        delegation.deadline_unix_ms = record.deadline_unix_ms;
        delegation.updated_at_unix_ms = self.clock.now_unix_ms();
        let session = self.current_delegation_session(&delegation, snapshot.session.as_ref())?;
        let mut response = snapshot.response.clone();
        response
            .as_object_mut()
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "typed delegation response is malformed",
                )
            })?
            .insert("deadlineUnixMs".to_owned(), json!(record.deadline_unix_ms));
        let scope_digest = hex::encode(Sha256::digest(
            format!("ae-sdd/recovery-renew/v1\0{}", record.delegation_id).as_bytes(),
        ));
        self.persistence
            .commit_identity_bundle(RuntimeIdentityTransition {
                operation: "delegation.renew.recover".to_owned(),
                scope_digest,
                idempotency_key: format!("recovery-renew-{}", event.event_seq),
                request_digest: event.payload_digest.clone(),
                expected_workspace_mode: None,
                expected_inventory_generation: None,
                expected_session_status: None,
                expected_delegation_status: Some("running".to_owned()),
                expected_context_generation: None,
                snapshot: RuntimeIdentitySnapshot {
                    identity_kind: RuntimeIdentityKind::Delegation,
                    workspace: snapshot.workspace.clone(),
                    session,
                    delegation: Some(delegation),
                    host_action: snapshot.host_action.clone(),
                    attestation: snapshot.attestation.clone(),
                    current_boot_receipt: None,
                    response,
                    replayed: false,
                },
                committed_at_unix_ms: self.clock.now_unix_ms(),
            })?;
        Ok(())
    }

    /// Creates a bounded delegation and dispatches a native Host create action.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &self,
        workspace_id: &str,
        work_item_id: &str,
        parent_session_id: &str,
        parent_role: WireAgentRole,
        parent_grant: &ScopedGrant,
        payload: DelegationCreatePayload,
        adapter_id: &str,
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
        self.host.require_registered(adapter_id)?;
        let workspace = self.typed_workspace(workspace_id)?;
        let parent = self.typed_session(parent_session_id)?;
        if parent.workspace_id != workspace_id || parent.role != parent_role {
            return Err(attestation_error(
                "delegation parent does not match its typed session identity",
            ));
        }
        // §2.4: the per-root-session "at most one spawning delegation" guard
        // (`reject_concurrent_pending_create`) is removed. With Child Self-Claim
        // (Plan §2) the child presents a precise `claim_id`, so FIFO queue
        // disambiguation is no longer the concurrency model — each delegation
        // is keyed by its own id and claimed independently. ROUTE-A's binding
        // ledger is the new liveness authority; `claim_or_preempt` replaces
        // the queue-position heuristic.
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
        // §9.4: the daemon-minted binding identity is derived from the same
        // (scope, idempotency) material as the delegation/action ids, so an
        // idempotent replay of create recovers the same binding id rather than
        // orphaning a duplicate row.
        let binding_id = stable_uuid("host-execution-binding", scope_digest, idempotency_key);
        // Stage, do not publish. The delegation record and this action must
        // become visible together: publishing first lets a failed commit leave a
        // queued action whose delegation does not exist, which the host can
        // acknowledge but never claim.
        let action = self.host.stage_with_action_id(
            &action_id,
            adapter_id,
            "create",
            Some(delegation_id.clone()),
            None,
            None,
            None,
            payload.deadline_unix_ms,
        )?;
        let action_digest =
            canonical_wire_digest(&serde_json::to_value(&action).map_err(canonical_error)?)?;
        let claim_id = Uuid::new_v4().to_string();
        let claim_digest = delegation_claim_digest(
            &claim_id,
            workspace_id,
            &delegation_id,
            &action.action_id,
            payload.child_role,
            parent_session_id,
            payload.deadline_unix_ms,
        )?;
        let published_action = action.clone();
        let record = DurableDelegation {
            schema_version: "delegation/v1".to_owned(),
            delegation_id: delegation_id.clone(),
            workspace_id: workspace_id.to_owned(),
            work_item_id: work_item_id.to_owned(),
            root_session_id: root_session_id.clone(),
            parent_session_id: parent_session_id.to_owned(),
            parent_delegation_id: payload.parent_delegation_id,
            child_role: payload.child_role,
            grant,
            input_revision: payload.input_revision,
            input_fingerprint: payload.input_fingerprint,
            deadline_unix_ms: payload.deadline_unix_ms,
            // Each delegation *is* one physical attempt, so the run identity is
            // minted here rather than supplied: a caller that could choose it
            // could make a retry masquerade as the attempt it replaces. Derived
            // from the daemon-issued delegation id so it is deterministic under
            // idempotent replay of the same create.
            series_run_id: series_run_identity(&delegation_id),
            series_id: payload.series_id,
            retry_of: payload.retry_of_series_run_id,
            flow_run_id: payload.flow_run_id,
            briefing: payload.briefing,
            asset_refs: payload.asset_refs,
            action_id: action.action_id.clone(),
            action_digest: action_digest.clone(),
            claim_digest,
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
                        work_item_id: Some(work_item_id.to_owned()),
                        root_session_id: root_session_id.clone(),
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
                        delegation_id: delegation_id.clone(),
                        host_action_id: action.action_id,
                        parent_session_id: parent_session_id.to_owned(),
                        action_digest,
                        created_at_unix_ms: now_unix_ms,
                    }),
                    attestation: None,
                    current_boot_receipt: None,
                    response,
                    replayed: false,
                },
                committed_at_unix_ms: now_unix_ms,
            })?;
        if committed.replayed {
            let current = self.load(&delegation_id)?;
            if current.workspace_id != workspace_id
                || current.parent_session_id != parent_session_id
                || current.action_id != published_action.action_id
            {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "replayed delegation create differs from durable operational state",
                ));
            }
            if self
                .host
                .ack_for_action(&current.action_id)?
                .is_some_and(|ack| ack.outcome == "rejected")
            {
                let projection = self.host_rejected(&delegation_id)?;
                return Ok((projection, true));
            }
            return Ok((project_delegation(&current), true));
        }
        self.save(&record)?;
        // §9.4 attach point 1: the delegation row is durable, so the binding
        // ledger may now record the spawning binding. Persisting after the
        // delegation save keeps a failed binding write from orphaning an action
        // whose delegation is missing; persisting before `attach_claim`/
        // `publish` keeps the binding durable before either becomes visible.
        self.bindings.spawn(
            &binding_id,
            workspace_id,
            &root_session_id,
            &delegation_id,
            now_unix_ms,
        )?;
        self.host
            .attach_claim(&published_action.action_id, claim_id)?;
        // The authoritative commit succeeded, so the action may now become
        // visible to its adapter. `publish` is idempotent, which keeps a replayed
        // create from queueing the same action twice.
        self.host.publish(&published_action)?;
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
        let authoritative_work_item = if record.work_item_id.is_empty() {
            return Err(attestation_error(
                "legacy delegation lacks a frozen Work Item authority",
            ));
        } else {
            if work_item_id.is_some_and(|supplied| supplied != record.work_item_id) {
                return Err(attestation_error(
                    "caller Work Item differs from the delegation's frozen authority",
                ));
            }
            record.work_item_id.as_str()
        };
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
        let supplied_claim_digest = delegation_claim_digest(
            claim_id,
            workspace_id,
            delegation_id,
            action_id,
            record.child_role,
            &record.parent_session_id,
            record.deadline_unix_ms,
        )?;
        if record.claim_digest.is_empty() || record.claim_digest != supplied_claim_digest {
            return Err(attestation_error(
                "child claim was not issued by the daemon for this delegation",
            ));
        }
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
                        current_work_item: Some(authoritative_work_item.to_owned()),
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
                        work_item_id: Some(authoritative_work_item.to_owned()),
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
                    current_boot_receipt: None,
                    response,
                    replayed: false,
                },
                committed_at_unix_ms: now_unix_ms,
            })?;
        self.save(&record)?;
        // §9.4 attach point 2: the binding transitions spawning → active in
        // lockstep with the delegation's spawning → running. Idempotent under
        // accept replay: a binding already `active` is left untouched.
        self.bindings.activate(delegation_id, now_unix_ms)?;
        let projection = serde_json::from_value(committed.response).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "typed delegation receipt response is malformed",
            )
        })?;
        Ok((projection, committed.replayed))
    }

    /// Checks report identity and envelope invariants without changing delegation state.
    pub(crate) fn preflight_report(
        &self,
        child_session_id: &str,
        payload: &DelegationReportPayload,
    ) -> RuntimeResult<bool> {
        let record = self.load(&payload.delegation_id)?;
        Ok(matches!(
            validate_report_admission(&record, child_session_id, payload)?,
            ReportAdmission::New { .. }
        ))
    }

    /// Stages a bounded child result without pretending artifact or memory validation succeeded.
    pub fn report(
        &self,
        child_session_id: &str,
        payload: DelegationReportPayload,
    ) -> RuntimeResult<DelegationResult> {
        let mut record = self.load(&payload.delegation_id)?;
        let ReportAdmission::New {
            report_digest,
            result_digest,
        } = validate_report_admission(&record, child_session_id, &payload)?
        else {
            return Ok(project_delegation(&record));
        };
        record.result_digest = Some(result_digest);
        record.summary = Some(payload.summary);
        record.report_digest = Some(report_digest);
        record.result = Some(payload.result);
        record.status = "result-staged".to_owned();
        self.save(&record)?;
        Ok(project_delegation(&record))
    }

    /// Commits a preflighted result and its artifact receipt in one delegation write.
    pub(crate) fn report_validated(
        &self,
        child_session_id: &str,
        payload: DelegationReportPayload,
        receipt: Value,
    ) -> RuntimeResult<DelegationResult> {
        let mut record = self.load(&payload.delegation_id)?;
        let ReportAdmission::New {
            report_digest,
            result_digest,
        } = validate_report_admission(&record, child_session_id, &payload)?
        else {
            return Ok(project_delegation(&record));
        };
        if !artifact_receipt_binds(&receipt, &payload.delegation_id, &result_digest) {
            return Err(RuntimeError::new(
                StableErrorCode::ChildResultInvalid,
                "artifact validation receipt does not bind the child result",
            ));
        }
        record.result_digest = Some(result_digest);
        record.summary = Some(payload.summary);
        record.report_digest = Some(report_digest);
        record.result = Some(payload.result);
        record.artifact_receipt = Some(receipt);
        record.status = "artifacts-validated".to_owned();
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
            || !record
                .result_digest
                .as_deref()
                .is_some_and(|result_digest| {
                    artifact_receipt_binds(&receipt, delegation_id, result_digest)
                })
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

    /// Resolves immutable collect authority before receipt replay or mutation.
    pub(crate) fn collect_work_item_authority(
        &self,
        workspace_id: &str,
        parent_session_id: &str,
        delegation_id: &str,
        requested_work_item_id: Option<&str>,
    ) -> RuntimeResult<String> {
        let record = self.load(delegation_id)?;
        if record.workspace_id != workspace_id || record.parent_session_id != parent_session_id {
            return Err(attestation_error(
                "collect caller does not match the delegation's durable authority",
            ));
        }
        if record.work_item_id.is_empty() {
            return Err(attestation_error(
                "legacy delegation lacks a frozen Work Item authority",
            ));
        }
        if requested_work_item_id.is_some_and(|requested| requested != record.work_item_id) {
            return Err(attestation_error(
                "caller Work Item differs from the delegation's frozen authority",
            ));
        }
        Ok(record.work_item_id)
    }

    pub(crate) fn delegated_session_work_item_authority(
        &self,
        workspace_id: &str,
        child_session_id: &str,
        delegation_id: &str,
        requested_work_item_id: Option<&str>,
    ) -> RuntimeResult<String> {
        let value = self
            .persistence
            .load_record("delegation/v1", delegation_id)?
            .ok_or_else(|| {
                attestation_error("child session does not match a durable delegation")
            })?;
        let record: DurableDelegation = serde_json::from_value(value).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "durable delegation record is malformed",
            )
        })?;
        if record.workspace_id != workspace_id
            || record.child_session_id.as_deref() != Some(child_session_id)
            || record.status != "running"
        {
            return Err(attestation_error(
                "child session does not match the delegation's durable authority",
            ));
        }
        if record.work_item_id.is_empty() {
            return Err(attestation_error(
                "legacy delegation lacks a frozen Work Item authority",
            ));
        }
        if requested_work_item_id.is_some_and(|requested| requested != record.work_item_id) {
            return Err(attestation_error(
                "caller Work Item differs from the delegation's frozen authority",
            ));
        }
        Ok(record.work_item_id)
    }

    pub(crate) fn is_root_series_boundary(&self, delegation_id: &str) -> RuntimeResult<bool> {
        let record = self
            .persistence
            .load_record("delegation/v1", delegation_id)?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ChildResultInvalid,
                    "delegation record is missing",
                )
            })?;
        Ok(is_root_series_boundary(&record))
    }

    pub(crate) fn collect_prerequisite(
        &self,
        workspace_id: &str,
        requester_session_id: &str,
        delegation_id: &str,
        requested_work_item_id: Option<&str>,
    ) -> RuntimeResult<Option<Value>> {
        let wire = self
            .persistence
            .load_record("delegation/v1", delegation_id)?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ChildResultInvalid,
                    "delegation record is missing",
                )
            })?;
        let record: DurableDelegation = serde_json::from_value(wire.clone()).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "delegation record is malformed",
            )
        })?;
        if record.parent_session_id != requester_session_id || record.status != "memory-cleaned" {
            return Ok(None);
        }
        let work_item_id = self.collect_work_item_authority(
            workspace_id,
            requester_session_id,
            delegation_id,
            requested_work_item_id,
        )?;
        let root_series_boundary = is_root_series_boundary(&wire);
        let root_project_lease_submit = root_series_boundary.then(|| {
            json!({
                "method":"operation.execute",
                "payload":{
                    "operation":"lease.acquire",
                    "payload":{
                        "owner":{"purpose":"delegation-collect"},
                        "ttlSeconds":300
                    }
                },
                "requestContext":{
                    "workspaceId":workspace_id,
                    "workItemId":work_item_id
                }
            })
        });
        let mut collect_request_context = json!({
            "workspaceId":workspace_id,
            "workItemId":work_item_id
        });
        if root_series_boundary {
            let context = collect_request_context
                .as_object_mut()
                .expect("collect request context is an object");
            context.insert(
                "leaseIdFrom".to_owned(),
                json!("rootProjectLeaseSubmit.result.data.leaseId"),
            );
            context.insert(
                "fencingTokenFrom".to_owned(),
                json!("rootProjectLeaseSubmit.result.data.fencingToken"),
            );
        }
        Ok(Some(json!({
            "requiresRootProjectLease":root_series_boundary,
            "rootProjectLeaseRemediation":root_series_boundary.then_some(
                "call operation.execute with operation=lease.acquire as the Root session before delegation.collect"
            ),
            "rootProjectLeaseSubmit":root_project_lease_submit,
            "collectSubmit":{
                "method":"delegation.collect",
                "payload":{"delegationId":delegation_id},
                "requestContext":collect_request_context
            }
        })))
    }

    /// Completes and returns the bounded root projection only after validation and cleanup.
    pub fn collect(&self, parent_session_id: &str, delegation_id: &str) -> RuntimeResult<Value> {
        let root_series_boundary = self.is_root_series_boundary(delegation_id)?;
        let mut record = self.load(delegation_id)?;
        if record.parent_session_id != parent_session_id {
            return Err(RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "only the parent session may collect this delegation",
            ));
        }
        if record.status == "completed" {
            self.bindings.release_by_delegation(
                delegation_id,
                "collected",
                self.clock.now_unix_ms(),
            )?;
            return Ok(collect_projection(&record, root_series_boundary));
        }
        if record.status != "memory-cleaned" {
            return Err(RuntimeError::new(
                StableErrorCode::ChildResultInvalid,
                "child result is not artifact-validated and memory-cleaned",
            ));
        }
        record.status = "completed".to_owned();
        self.save(&record)?;
        // §9.4 attach point 3a: the happy terminal. A completed delegation never
        // resumes, so its host-execution binding is released for reuse. The
        // `collected` reason distinguishes this path from cancellation/close.
        self.bindings.release_by_delegation(
            delegation_id,
            "collected",
            self.clock.now_unix_ms(),
        )?;
        Ok(collect_projection(&record, root_series_boundary))
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

    /// Projects a parent-only renewal action when a running delegation is near expiry.
    pub fn renewal_action(
        &self,
        requester_session_id: &str,
        delegation_id: &str,
        max_lifetime_ms: u64,
        now_unix_ms: u64,
    ) -> RuntimeResult<Option<Value>> {
        let record = self.load(delegation_id)?;
        if record.parent_session_id != requester_session_id {
            return Ok(None);
        }
        if record.status != "running" || now_unix_ms >= record.deadline_unix_ms {
            return Ok(None);
        }
        let initial_lifetime = record
            .deadline_unix_ms
            .saturating_sub(record.created_at_unix_ms);
        let threshold_ms = (initial_lifetime / 5).min(120_000);
        if record.deadline_unix_ms.saturating_sub(now_unix_ms) > threshold_ms {
            return Ok(None);
        }
        let maximum_deadline = record.created_at_unix_ms.saturating_add(max_lifetime_ms);
        let proposed_deadline = now_unix_ms
            .saturating_add(initial_lifetime)
            .min(maximum_deadline);
        if proposed_deadline <= record.deadline_unix_ms {
            return Ok(None);
        }
        Ok(Some(json!({
            "kind":"renew-delegation",
            "delegationId":record.delegation_id,
            "currentDeadlineUnixMs":record.deadline_unix_ms,
            "deadlineUnixMs":proposed_deadline,
            "maximumDeadlineUnixMs":maximum_deadline,
        })))
    }

    /// Reads the delegation's absolute deadline, gated by lineage access control.
    ///
    /// The `deadline_unix_ms` is the authoritative liveness upper bound for a
    /// running delegation. Unlike the physical attestation's `expires_at_unix_ms`
    /// (which is an immutable, accept-time TTL snapshot frozen into the
    /// attestation digest), the deadline represents the delegation's own
    /// activity window. Live callers (session open, job submission, review
    /// contribution) must check the deadline to admit renewal of the same
    /// accepted delegation after the ancestor session TTL has been refreshed.
    pub fn deadline_unix_ms(
        &self,
        requester_session_id: &str,
        delegation_id: &str,
    ) -> RuntimeResult<u64> {
        let record = self.load(delegation_id)?;
        if record.parent_session_id != requester_session_id
            && record.child_session_id.as_deref() != Some(requester_session_id)
        {
            return Err(RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "session is outside this delegation lineage",
            ));
        }
        Ok(record.deadline_unix_ms)
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
        // §9.4 attach point 3b: the unhappy terminal. Cancellation is terminal
        // for the binding just as collect is, so the binding is released with
        // the `cancelled` reason to keep the audit trail honest.
        self.bindings.release_by_delegation(
            delegation_id,
            "cancelled",
            self.clock.now_unix_ms(),
        )?;
        Ok(project_delegation(&record))
    }

    /// Terminates a spawning delegation after its bound Host rejects create.
    pub fn host_rejected(&self, delegation_id: &str) -> RuntimeResult<DelegationResult> {
        let mut record = self.load(delegation_id)?;
        if record.status == "cancelled" {
            // A prior attempt may have persisted the delegation row and failed
            // before its Series projection. Replaying both writes is required
            // before an ACK retry may be acknowledged as complete.
            self.save(&record)?;
            self.bindings.release_by_delegation(
                delegation_id,
                "cancelled",
                self.clock.now_unix_ms(),
            )?;
            return Ok(project_delegation(&record));
        }
        if record.status != "spawning" {
            return Err(RuntimeError::new(
                StableErrorCode::DelegationAttestationFailed,
                "Host rejection applies only to a spawning delegation",
            ));
        }
        record.status = "cancelled".to_owned();
        self.save(&record)?;
        self.bindings.release_by_delegation(
            delegation_id,
            "cancelled",
            self.clock.now_unix_ms(),
        )?;
        Ok(project_delegation(&record))
    }

    /// Extends a running delegation's deadline within the configured bound.
    ///
    /// The liveness judgement itself is correct and stays untouched; what was
    /// missing is a way to express a legitimate extension. Without it a long
    /// series that outlives its original deadline can only be cancelled and
    /// rebuilt, discarding work that was already done.
    ///
    /// Only the parent may renew: a child that could extend its own deadline
    /// would hold its grant for as long as it liked. The bound is a total
    /// lifetime measured from creation, so repeated renewals cannot walk the
    /// deadline forward without limit.
    #[allow(clippy::too_many_arguments)]
    pub fn renew(
        &self,
        parent_session_id: &str,
        delegation_id: &str,
        deadline_unix_ms: u64,
        max_lifetime_ms: u64,
        now_unix_ms: u64,
        scope_digest: &str,
        idempotency_key: &str,
        request_digest: &str,
    ) -> RuntimeResult<DelegationResult> {
        let mut record = self.load(delegation_id)?;
        if record.parent_session_id != parent_session_id {
            return Err(RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "only the parent session may renew this delegation",
            ));
        }
        if record.status != "running" {
            return Err(RuntimeError::new(
                StableErrorCode::DelegationAttestationFailed,
                "only a running delegation can be renewed",
            ));
        }
        if deadline_unix_ms <= now_unix_ms {
            return Err(RuntimeError::new(
                StableErrorCode::OperationSchemaInvalid,
                "renewed delegation deadline must be in the future",
            ));
        }
        // The bound is measured from creation, not from the current deadline.
        // A deadline-relative bound would move every time it was granted, so
        // repeated renewals could walk the deadline forward without limit;
        // `created_at_unix_ms` never changes and survives rehydration.
        let ceiling = record.created_at_unix_ms.saturating_add(max_lifetime_ms);
        if deadline_unix_ms > ceiling {
            return Err(RuntimeError::new(
                StableErrorCode::OperationSchemaInvalid,
                "renewed delegation deadline exceeds the permitted lifetime",
            ));
        }
        if deadline_unix_ms < record.deadline_unix_ms {
            return Err(RuntimeError::new(
                StableErrorCode::OperationSchemaInvalid,
                "renewal cannot shorten a delegation deadline",
            ));
        }
        let prior = self.typed_delegation_snapshot(delegation_id)?;
        let prior_delegation = prior.delegation.as_ref().ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "typed delegation snapshot lacks its delegation row",
            )
        })?;
        if prior_delegation.deadline_unix_ms != record.deadline_unix_ms {
            if prior_delegation.deadline_unix_ms == deadline_unix_ms {
                record.deadline_unix_ms = deadline_unix_ms;
                self.save(&record)?;
                return Ok(project_delegation(&record));
            }
            return Err(RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "operational delegation differs from typed identity authority",
            ));
        }
        record.deadline_unix_ms = deadline_unix_ms;
        let projection = project_delegation(&record);
        let response = serde_json::to_value(&projection).map_err(canonical_error)?;
        let mut delegation = prior_delegation.clone();
        delegation.deadline_unix_ms = deadline_unix_ms;
        delegation.updated_at_unix_ms = now_unix_ms;
        let session = self.current_delegation_session(&delegation, prior.session.as_ref())?;
        self.persistence
            .commit_identity_bundle(RuntimeIdentityTransition {
                operation: "delegation.renew".to_owned(),
                scope_digest: scope_digest.to_owned(),
                idempotency_key: idempotency_key.to_owned(),
                request_digest: request_digest.to_owned(),
                expected_workspace_mode: None,
                expected_inventory_generation: None,
                expected_session_status: None,
                expected_delegation_status: Some("running".to_owned()),
                expected_context_generation: None,
                snapshot: RuntimeIdentitySnapshot {
                    identity_kind: RuntimeIdentityKind::Delegation,
                    workspace: prior.workspace,
                    session,
                    delegation: Some(delegation),
                    host_action: prior.host_action,
                    attestation: prior.attestation,
                    current_boot_receipt: None,
                    response,
                    replayed: false,
                },
                committed_at_unix_ms: now_unix_ms,
            })?;
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

    fn current_delegation_session(
        &self,
        delegation: &RuntimeDelegationRecord,
        embedded: Option<&RuntimeSessionRecord>,
    ) -> RuntimeResult<Option<RuntimeSessionRecord>> {
        let Some(child_session_id) = delegation.child_session_id.as_deref() else {
            return Ok(embedded.cloned());
        };
        let session = self
            .persistence
            .list_identity_snapshots(RuntimeIdentityKind::Session)?
            .into_iter()
            .filter_map(|snapshot| snapshot.session)
            .find(|session| session.session_id == child_session_id)
            .or_else(|| {
                embedded
                    .filter(|session| session.session_id == child_session_id)
                    .cloned()
            })
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::DelegationAttestationFailed,
                    "typed delegation references a missing child session",
                )
            })?;
        if session.workspace_id != delegation.workspace_id
            || session.role != delegation.role
            || session.root_session_id != delegation.root_session_id
            || session.parent_session_id.as_deref() != Some(delegation.parent_session_id.as_str())
            || session.delegation_id.as_deref() != Some(delegation.delegation_id.as_str())
        {
            return Err(RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "current child session differs from its delegation lineage",
            ));
        }
        Ok(Some(session))
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
            .store_record("delegation/v1", &record.delegation_id, &value)?;
        self.save_series_run_projection(record)
    }

    /// Mirrors the delegation into the `series_run/v1` projection.
    ///
    /// D-03 item 5 and §4.2 require the execution flow tree be queryable apart from
    /// the delegation tree, and line 767 requires Series retries not pollute it.
    /// The delegation record cannot serve that query itself: it is keyed by
    /// `delegationId`, so "every attempt of this Series" means listing all
    /// delegations and filtering. This projection is keyed by `seriesRunId` and
    /// carries `seriesId`, which is what makes the query direct.
    ///
    /// Written from `save` rather than only at create so the projection tracks the
    /// attempt as it advances. §7 rule 18 requires a restart not resurrect an
    /// unproven `running` Series as `completed`, which is only checkable if the
    /// in-progress state is durable rather than inferred at the end.
    ///
    /// Skipped entirely when the delegation carries no Series identity — the typed
    /// reconstruction path has neither a Series kind nor a Flow Run, and writing a
    /// projection keyed on an empty identity would merge unrelated Series into one
    /// row. Absent stays absent (D-03 item 6).
    fn save_series_run_projection(&self, record: &DurableDelegation) -> RuntimeResult<()> {
        if record.series_run_id.is_empty() || record.series_id.is_empty() {
            return Ok(());
        }
        // `attemptOrdinal` is deliberately absent rather than guessed. The record
        // knows only its immediate predecessor, so the ordinal would require walking
        // the chain, and a fabricated 1 would claim a first attempt for every retry.
        let projection = json!({
            "schemaVersion": "series_run/v1",
            "workspaceId": record.workspace_id,
            "seriesRunId": record.series_run_id,
            "seriesId": record.series_id,
            "flowRunId": record.flow_run_id,
            "retryOf": record.retry_of,
            "delegationId": record.delegation_id,
            "lifecycleState": series_lifecycle_state(&record.status),
            "childRole": format!("{:?}", record.child_role).to_lowercase(),
            "inputRevision": record.input_revision,
            "inputFingerprint": record.input_fingerprint,
            "createdAtUnixMs": record.created_at_unix_ms,
        });
        self.persistence.store_record(
            "series_run/v1",
            &format!("{}\0{}", record.workspace_id, record.series_run_id),
            &projection,
        )
    }
}

/// Projects a delegation status onto the §11.2 conceptual lifecycle vocabulary.
///
/// The two vocabularies exist because they answer different questions: a delegation
/// status describes the *Agent handoff*, while §11.2 describes the *Series*. Writing
/// the mapping down once keeps them from drifting, the same reason
/// `SeriesLifecycleState::to_receipt_status` exists.
///
/// `memory-cleaned` maps to `completed` rather than a state of its own: cleanup
/// happens after the Series already reached a terminal outcome, so it says nothing
/// about the Series. `opening` and `spawning` both precede a child claim, which
/// §11.2 calls `spawn_requested` — and §7 rule 13 requires that a Series not show
/// as `running` before the child claims it, so neither may map to `running`.
fn series_lifecycle_state(status: &str) -> &'static str {
    match status {
        "opening" | "spawning" => "spawn_requested",
        "running" => "running",
        "result-staged" => "result_staged",
        "artifacts-validated" => "validated",
        "completed" | "memory-cleaned" => "completed",
        "cancelled" => "cancelled",
        // An unrecognised status is reported as unknown rather than coerced into a
        // plausible neighbour: a new delegation status silently reading as `running`
        // would let a supervisor believe an attempt is live when it is not.
        _ => "unknown",
    }
}

fn project_delegation(record: &DurableDelegation) -> DelegationResult {
    DelegationResult {
        delegation_id: record.delegation_id.clone(),
        status: record.status.clone(),
        deadline_unix_ms: record.deadline_unix_ms,
        grant: record.grant.clone(),
        child_role: record.child_role,
        action_id: record.action_id.clone(),
        child_session_id: record.child_session_id.clone(),
        result_digest: record.result_digest.clone(),
        briefing: record.briefing.clone(),
        asset_refs: record.asset_refs.clone(),
    }
}

fn collect_projection(record: &DurableDelegation, root_series_boundary: bool) -> Value {
    let result = record.result.as_ref().unwrap_or(&Value::Null);
    let root_project_lease_remediation = root_series_boundary.then_some(
        "call operation.execute with operation=lease.acquire as the Root session before delegation.collect",
    );
    let root_project_lease_submit = root_series_boundary.then(|| {
        json!({
            "method":"operation.execute",
            "operation":"lease.acquire",
            "arguments":{
                "owner":{"purpose":"delegation-collect"},
                "ttlSeconds":300
            }
        })
    });
    let lease_binding = root_series_boundary.then(|| {
        json!({
            "leaseIdFrom":"rootProjectLeaseSubmit.result.data.leaseId",
            "fencingTokenFrom":"rootProjectLeaseSubmit.result.data.fencingToken"
        })
    });
    json!({
        "delegationId": record.delegation_id,
        "status": record.status,
        "summary": record.summary,
        "briefing": record.briefing,
        "assetRefs": record.asset_refs,
        "resultDigest": record.result_digest,
        "outcome":result.get("outcome"),
        "findings":result.get("findings").cloned().unwrap_or_else(|| json!([])),
        "requestedAction":result.get("requestedAction"),
        "artifacts":record.artifact_receipt.as_ref().and_then(|receipt| receipt.get("artifacts")).cloned().unwrap_or_else(|| json!([])),
        "artifactValidationReceipt":record.artifact_receipt,
        "memoryCleanupReceipt":record.cleanup_receipt,
        "requiresRootProjectLease":root_series_boundary,
        "rootProjectLeaseRemediation":root_project_lease_remediation,
        "rootProjectLeaseSubmit":root_project_lease_submit,
        "collectSubmit":{
            "method":"delegation.collect",
            "arguments":{"delegationId":record.delegation_id},
            "leaseBinding":lease_binding
        },
    })
}

fn is_root_series_boundary(record: &Value) -> bool {
    record.get("childRole").and_then(Value::as_str) == Some("series")
        && record.get("parentDelegationId").is_some_and(Value::is_null)
}

fn validate_report_admission(
    record: &DurableDelegation,
    child_session_id: &str,
    payload: &DelegationReportPayload,
) -> RuntimeResult<ReportAdmission> {
    if record.child_session_id.as_deref() != Some(child_session_id) {
        return Err(RuntimeError::new(
            StableErrorCode::RoleOperationForbidden,
            "only the attested running child may report this delegation",
        ));
    }
    if record.input_revision != payload.input_revision {
        return Err(RuntimeError::new(
            StableErrorCode::ChildResultInvalid,
            "child result inputRevision is stale",
        )
        .with_remediation(
            "re-read the delegation's frozen inputRevision from the projection and \
             retry delegation.report",
        ));
    }
    if record.input_fingerprint != payload.input_fingerprint {
        return Err(RuntimeError::new(
            StableErrorCode::ChildResultInvalid,
            "child result inputFingerprint is stale",
        )
        .with_remediation(
            "delegation.report requires the frozen inputFingerprint (not decisionDigest); \
             re-read it from the projection and retry",
        ));
    }
    if payload.summary.is_empty() || payload.summary.len() > 8_192 {
        return Err(RuntimeError::new(
            StableErrorCode::ChildResultTooLarge,
            "child result summary must be within 1..=8192 bytes",
        ));
    }
    reject_transcript_fields(&payload.result)?;
    let canonical = serde_json::to_vec(payload).map_err(canonical_error)?;
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
            return Ok(ReportAdmission::Replay);
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
    Ok(ReportAdmission::New {
        report_digest,
        result_digest: hex::encode(Sha256::digest(result_bytes)),
    })
}

fn artifact_receipt_binds(receipt: &Value, delegation_id: &str, result_digest: &str) -> bool {
    receipt.get("schemaVersion").and_then(Value::as_str)
        == Some("delegation-artifact-validation/v1")
        && receipt.get("delegationId").and_then(Value::as_str) == Some(delegation_id)
        && receipt.get("resultDigest").and_then(Value::as_str) == Some(result_digest)
        && receipt.get("artifacts").is_some_and(Value::is_array)
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

#[allow(clippy::too_many_arguments)]
fn delegation_claim_digest(
    claim_id: &str,
    workspace_id: &str,
    delegation_id: &str,
    action_id: &str,
    child_role: WireAgentRole,
    parent_session_id: &str,
    deadline_unix_ms: u64,
) -> RuntimeResult<String> {
    canonical_wire_digest(&json!({
        "domain":"delegation-issued-claim/v1",
        "claimId":claim_id,
        "workspaceId":workspace_id,
        "delegationId":delegation_id,
        "actionId":action_id,
        "childRole":child_role,
        "parentSessionId":parent_session_id,
        "deadlineUnixMs":deadline_unix_ms,
    }))
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
