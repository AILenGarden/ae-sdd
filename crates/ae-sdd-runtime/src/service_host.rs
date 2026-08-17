use super::*;

use crate::{
    RootSeriesDelegationPayload, grant::semantic_series_grant, model::HostActionAckResult,
};
use ae_sdd_domain::DEFAULT_CHILD_SUMMARY_MAX_BYTES;

const DEFAULT_SERIES_DELEGATION_TTL_MS: u64 = 30 * 60 * 1_000;

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FlowDelegationIntent {
    schema_version: String,
    workspace_id: String,
    work_item_id: String,
    decision_digest: String,
    series_kind: String,
    state_revision: u64,
    input_fingerprint: String,
    required_artifacts: Vec<String>,
    /// Bounded methodology refs frozen with the flow decision.
    ///
    /// `None` means the intent predates frozen refs and must not be combined
    /// with a fresh projection during delegation creation. `Some([])` is a
    /// valid authoritative projection with no refs.
    #[serde(default)]
    asset_refs: Option<Vec<crate::AssetRefWire>>,
    /// Deadline frozen on the first commit of this flow intent.
    #[serde(default)]
    deadline_unix_ms: Option<u64>,
    /// The `seriesRunId` this attempt replaces, when the flow decided a retry.
    ///
    /// `#[serde(default)]` keeps intents committed before this field readable.
    #[serde(default)]
    retry_of_series_run_id: Option<String>,
    /// The Flow Run this attempt belongs to (§4.2 `FR -> Series Run`).
    ///
    /// Taken from the committed flow decision rather than from the caller: a Root
    /// that could name the Flow Run could attach an attempt to a run it does not
    /// belong to, which would corrupt the execution tree that line 767 requires
    /// stay uncontaminated across retries.
    ///
    /// Optional because state written before run identity existed carries none, and
    /// D-03 item 6 forbids substituting a blank for missing data.
    #[serde(default)]
    flow_run_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CurrentFlowDelegationIntent {
    schema_version: String,
    workspace_id: String,
    work_item_id: String,
    decision_digest: String,
    state_revision: u64,
    input_fingerprint: String,
    #[serde(default)]
    decision_digest_version: Option<String>,
}

impl RuntimeService {
    pub(super) fn host_register(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let credential = params.capability_token.as_deref().ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::EndpointAuthFailed,
                "boot-scoped host adapter credential is required",
            )
        })?;
        if !constant_time_equal(credential.as_bytes(), self.endpoint_token.as_bytes()) {
            return Err(RuntimeError::new(
                StableErrorCode::EndpointAuthFailed,
                "host adapter credential is invalid for this daemon boot",
            ));
        }
        let payload: HostRegisterPayload = decode_value(params.payload.clone())?;
        let key = require_idempotency(params)?;
        let scope = format!("host-register\0{}", payload.adapter_id);
        let digest = canonical_digest(&payload)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        self.host.register(&payload.adapter_id)?;
        let value = to_value(payload)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "host.registered",
            None,
            None,
            None,
        )
        .map(|(value, _)| value)
    }

    pub(super) fn host_action_next(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let adapter_id = params
            .payload
            .get("adapterId")
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("adapterId is required"))?;
        to_value(self.host.next(adapter_id)?)
    }

    pub(super) fn host_action_ack(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let adapter_id = params
            .payload
            .get("adapterId")
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("adapterId is required"))?
            .to_owned();
        let ack_value = params
            .payload
            .get("ack")
            .cloned()
            .ok_or_else(|| schema_error("ack is required"))?;
        let ack: HostAckPayload = decode_value(ack_value)?;
        let key = require_idempotency(params)?;
        let scope = format!("host-ack\0{}\0{}", adapter_id, ack.action_id);
        let digest = canonical_digest(&ack)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let action = self.host.acknowledge(&adapter_id, ack.clone())?;
        if action.kind == "create"
            && ack.outcome == "rejected"
            && let Some(delegation_id) = action.delegation_id.as_deref()
        {
            self.delegation.host_rejected(delegation_id)?;
        }
        if action.kind == "compact"
            && ack.outcome == "accepted"
            && let Some(compact_id) = &action.compact_id
        {
            self.update_compact_status(compact_id, "host-acknowledged", None)?;
        }
        let value = to_value(HostActionAckResult {
            action,
            ack_id: ack.ack_id,
            outcome: ack.outcome,
            host_task_id: ack.host_task_id,
            child_session_id: ack.session_id,
        })?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "host.action_acknowledged",
            None,
            None,
            None,
        )
        .map(|(value, _)| value)
    }

    pub(super) fn host_pressure(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let payload: HostPressurePayload = decode_value(params.payload.clone())?;
        self.host.require_registered(&payload.adapter_id)?;
        let session_id = SessionId::from_str(&identity.session_id)
            .map_err(|_| schema_error("sessionId is not a UUID"))?;
        let decision = self.context.observe_pressure(session_id, &payload)?;
        let compact = if decision == PressureDecision::TriggerCompact {
            let projection = self.context.project(&identity.session_id, 0, "")?;
            Some(self.start_compact(
                &identity.session_id,
                payload.context_generation,
                &projection.digest,
                payload.observed_at_unix_ms.saturating_add(30_000),
                &payload.adapter_id,
            )?)
        } else {
            None
        };
        Ok(json!({"decision":format!("{decision:?}"),"compact":compact}))
    }

    pub(super) fn delegation_create(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let root_reference = if identity.role == WireAgentRole::Root {
            Some(decode_value::<RootSeriesDelegationPayload>(
                params.payload.clone(),
            )?)
        } else {
            None
        };
        let payload = if let Some(reference) = root_reference.as_ref() {
            self.root_series_delegation_payload(params, &identity, reference)?
        } else {
            decode_value(params.payload.clone())?
        };
        // Schema bounds are validated before any host capability check so an
        // oversized briefing fails closed as a schema error, never as an
        // unrelated capability denial.
        if payload
            .briefing
            .as_deref()
            .is_some_and(|briefing| briefing.len() > DEFAULT_CHILD_SUMMARY_MAX_BYTES as usize)
        {
            return Err(schema_error(
                "delegation briefing must be within 8192 bytes",
            ));
        }
        // Canonicalize caller-controlled request authority before consulting
        // mutable flow or Host authority. A Root supplies only the decision
        // reference; the derived deadline may be renewed later without turning
        // that same caller request into an idempotency conflict.
        let key = require_idempotency(params)?;
        let scope = format!(
            "delegation-create\0{}\0{}",
            identity.session_id,
            params.work_item_id.as_deref().unwrap_or("")
        );
        let request_payload = if let Some(reference) = root_reference.as_ref() {
            to_value(reference)?
        } else {
            to_value(&payload)?
        };
        let digest = canonical_digest(&json!({
            "workspaceId":identity.workspace_id,
            "parentSessionId":identity.session_id,
            "parentRole":identity.role,
            "workItemId":params.work_item_id,
            "payload":request_payload,
        }))?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        if let Some(reference) = root_reference.as_ref() {
            self.validate_current_root_series_delegation(params, &identity, reference)?;
        }
        let now_unix_ms = self.clock.now_unix_ms();
        if payload.deadline_unix_ms <= now_unix_ms {
            return Err(RuntimeError::new(
                StableErrorCode::DelegationAttestationFailed,
                "committed flow delegation intent has expired; rerun flow.next",
            ));
        }
        let host_binding_session_id = if identity.role == WireAgentRole::Series {
            let parent_delegation_id = {
                let state = self.lock_state()?;
                state
                    .sessions
                    .get(&identity.session_id)
                    .and_then(|session| session.delegation_id.clone())
                    .ok_or_else(|| {
                        RuntimeError::new(
                            StableErrorCode::DelegationAttestationFailed,
                            "Series session is missing its delegation lineage",
                        )
                    })?
            };
            self.delegation.root_session_id(&parent_delegation_id)?
        } else {
            identity.session_id.clone()
        };
        let adapter_id = if let Some(binding) = self
            .persistence
            .load_record("session-host-binding/v1", &host_binding_session_id)?
        {
            let bound_workspace = binding
                .get("workspaceId")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "durable session host binding is malformed",
                    )
                })?;
            if bound_workspace != identity.workspace_id {
                return Err(RuntimeError::new(
                    StableErrorCode::DelegationAttestationFailed,
                    "session Host binding belongs to another workspace",
                ));
            }
            let adapter_id = binding
                .get("adapterId")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "durable session host binding is malformed",
                    )
                })?;
            self.host.require_registered(adapter_id)?;
            adapter_id.to_owned()
        } else {
            self.host.delegation_adapter()?
        };
        let typed_scope_digest = canonical_digest(&json!({
            "domain":"delegation.create/v1",
            "workspaceId":identity.workspace_id,
            "parentSessionId":identity.session_id,
            "workItemId":params.work_item_id,
        }))?;
        let (projection, _) = self.delegation.create(
            &identity.workspace_id,
            require(&params.work_item_id, "workItemId")?,
            &identity.session_id,
            identity.role,
            &identity.grant,
            payload,
            &adapter_id,
            &typed_scope_digest,
            key,
            &digest,
            now_unix_ms,
        )?;
        self.persistence.store_record(
            "delegation-memory/v1",
            &projection.delegation_id,
            &json!({
                "schemaVersion":"delegation-memory/v1",
                "delegationId":projection.delegation_id,
                "workspaceId":identity.workspace_id,
                "workItemId":params.work_item_id,
                "status":"active",
                "payloadPurged":false,
            }),
        )?;
        let value = to_value(projection)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "delegation.created",
            Some(identity.workspace_id),
            Some(identity.session_id),
            params.work_item_id.clone(),
        )
        .map(|(value, _)| value)
    }

    pub(super) fn delegation_status(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let id = payload_string(&params.payload, "delegationId")?;
        let mut value = to_value(self.delegation.status(&identity.session_id, id)?)?;
        if let Some(action) = self.delegation.renewal_action(
            &identity.session_id,
            id,
            self.config.max_delegation_lifetime_ms,
            self.clock.now_unix_ms(),
        )? {
            value
                .as_object_mut()
                .expect("delegation projection is an object")
                .insert("nextAction".to_owned(), action);
        }
        if let Some(prerequisite) = self.delegation.collect_prerequisite(
            &identity.workspace_id,
            &identity.session_id,
            id,
            params.work_item_id.as_deref(),
        )? {
            value
                .as_object_mut()
                .expect("delegation projection is an object")
                .insert("collectPrerequisite".to_owned(), prerequisite);
        }
        Ok(value)
    }

    pub(super) fn delegation_accept(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let payload: DelegationAcceptPayload = decode_value(params.payload.clone())?;
        let workspace_id = require(&params.workspace_id, "workspaceId")?.to_owned();
        let key = require_idempotency(params)?;
        let scope = format!("delegation-accept\0{}", payload.delegation_id);
        let digest = canonical_digest(&json!({
            "workspaceId":workspace_id,
            "workItemId":params.work_item_id,
            "payload":payload,
        }))?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let typed_scope_digest = canonical_digest(&json!({
            "domain":"delegation.accept/v1",
            "workspaceId":workspace_id,
            "delegationId":payload.delegation_id,
        }))?;
        let boot_id = self.boot_id.to_string();
        let (projection, _) = self.delegation.accept(
            &workspace_id,
            params.work_item_id.as_deref(),
            &payload.delegation_id,
            &payload.claim_id,
            &payload.action_id,
            &payload.child_session_id,
            payload.expires_at_unix_ms,
            &boot_id,
            &typed_scope_digest,
            key,
            &digest,
            self.clock.now_unix_ms(),
        )?;
        let value = to_value(projection)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "delegation.accepted",
            params.workspace_id.clone(),
            Some(payload.child_session_id),
            params.work_item_id.clone(),
        )
        .map(|(value, _)| value)
    }

    /// Child self-claim entry point (Plan §2). Semantically a sibling of
    /// `delegation.accept` — same payload shape, same supervisor path, same
    /// idempotency treatment — that differs only in *who* is presenting the
    /// one-time `claim_id`. Splitting it into its own wire method keeps the
    /// host-native A2 path (the child itself) distinct from the A1 path (an
    /// external host adapter ACKing on the child's behalf) without forcing
    /// `delegation.accept` to carry a mode flag.
    pub(super) fn delegation_child_claim(
        &self,
        params: &RequestParams<Value>,
    ) -> RuntimeResult<Value> {
        let payload: DelegationAcceptPayload = decode_value(params.payload.clone())?;
        let workspace_id = require(&params.workspace_id, "workspaceId")?.to_owned();
        let key = require_idempotency(params)?;
        let scope = format!("delegation-child-claim\0{}", payload.delegation_id);
        let digest = canonical_digest(&json!({
            "workspaceId":workspace_id,
            "workItemId":params.work_item_id,
            "payload":payload,
        }))?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let typed_scope_digest = canonical_digest(&json!({
            "domain":"delegation.child_claim/v1",
            "workspaceId":workspace_id,
            "delegationId":payload.delegation_id,
        }))?;
        let boot_id = self.boot_id.to_string();
        let (projection, _) = self.delegation.accept(
            &workspace_id,
            params.work_item_id.as_deref(),
            &payload.delegation_id,
            &payload.claim_id,
            &payload.action_id,
            &payload.child_session_id,
            payload.expires_at_unix_ms,
            &boot_id,
            &typed_scope_digest,
            key,
            &digest,
            self.clock.now_unix_ms(),
        )?;
        let value = to_value(projection)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "delegation.child_claimed",
            params.workspace_id.clone(),
            Some(payload.child_session_id),
            params.work_item_id.clone(),
        )
        .map(|(value, _)| value)
    }

    pub(super) fn delegation_report(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let payload: DelegationReportPayload = decode_value(params.payload.clone())?;
        let key = require_idempotency(params)?;
        let scope = format!("delegation-report\0{}", payload.delegation_id);
        let digest = canonical_digest(&payload)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let memory = self
            .persistence
            .load_record("delegation-memory/v1", &payload.delegation_id)?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ChildResultInvalid,
                    "daemon-owned delegation memory namespace is missing",
                )
            })?;
        if memory.get("workspaceId").and_then(Value::as_str) != Some(identity.workspace_id.as_str())
            || !matches!(
                memory.get("status").and_then(Value::as_str),
                Some("active" | "cleaned")
            )
        {
            return Err(RuntimeError::new(
                StableErrorCode::ChildResultInvalid,
                "delegation memory namespace is outside the trusted workspace or lifecycle",
            ));
        }
        let delegation_id = payload.delegation_id.clone();
        let workspace = self.business_workspace_for(&identity)?;
        if self
            .delegation
            .preflight_report(&identity.session_id, &payload)?
        {
            let receipt = self.business.validate_delegation_artifacts(
                &workspace,
                &delegation_id,
                &payload.result,
            )?;
            self.delegation
                .report_validated(&identity.session_id, payload, receipt)?;
        } else {
            self.delegation.report(&identity.session_id, payload)?;
        }
        let (mut status, result, mut artifact_receipt) =
            self.delegation.completion_material(&delegation_id)?;
        if status == "result-staged" {
            let receipt =
                self.business
                    .validate_delegation_artifacts(&workspace, &delegation_id, &result)?;
            self.delegation
                .artifacts_validated(&delegation_id, receipt.clone())?;
            artifact_receipt = Some(receipt);
            status = "artifacts-validated".to_owned();
        }
        if status == "artifacts-validated" {
            let artifact_receipt = artifact_receipt.ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ChildResultInvalid,
                    "artifact validation receipt is missing after validation",
                )
            })?;
            let receipt = self.business.cleanup_delegation_memory(
                &workspace,
                &delegation_id,
                &result,
                &artifact_receipt,
            )?;
            let cleaned_namespace = self
                .persistence
                .load_record("delegation-memory/v1", &delegation_id)?
                .ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::ChildResultInvalid,
                        "delegation memory namespace disappeared during cleanup",
                    )
                })?;
            if cleaned_namespace.get("workspaceId").and_then(Value::as_str)
                != Some(identity.workspace_id.as_str())
                || cleaned_namespace.get("status").and_then(Value::as_str) != Some("cleaned")
                || cleaned_namespace
                    .get("payloadPurged")
                    .and_then(Value::as_bool)
                    != Some(true)
                || cleaned_namespace
                    .get("memorySnapshotDigest")
                    .and_then(Value::as_str)
                    != receipt.get("memorySnapshotDigest").and_then(Value::as_str)
            {
                return Err(RuntimeError::new(
                    StableErrorCode::ChildResultInvalid,
                    "memory cleanup did not commit the daemon-owned namespace tombstone",
                ));
            }
            self.delegation.memory_cleaned(&delegation_id, receipt)?;
        }
        let value = to_value(
            self.delegation
                .status(&identity.session_id, &delegation_id)?,
        )?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "delegation.result_validated_and_memory_cleaned",
            Some(identity.workspace_id),
            Some(identity.session_id),
            params.work_item_id.clone(),
        )
        .map(|(value, _)| value)
    }

    pub(super) fn delegation_collect(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let payload: DelegationCollectPayload = decode_value(params.payload.clone())?;
        let id = payload.delegation_id.as_str();
        let work_item_id = self.delegation.collect_work_item_authority(
            &identity.workspace_id,
            &identity.session_id,
            id,
            params.work_item_id.as_deref(),
        )?;
        let key = require_idempotency(params)?;
        let scope = format!("delegation-collect\0{}", id);
        let digest = canonical_digest(&payload)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let series_boundary = self.delegation.is_root_series_boundary(id)?;
        let mut value = self.delegation.collect(&identity.session_id, id)?;
        if series_boundary {
            let workspace = self.business_workspace_for(&identity)?;
            // A delegation-stable key, not the per-request idempotency key, makes
            // the boundary enter the flow event stream exactly once even when a
            // collector replays the collect under a fresh idempotency identity.
            let boundary_key = format!("series-boundary\0{id}");
            self.business.record_series_completed(
                &workspace,
                &work_item_id,
                &identity.session_id,
                id,
                &boundary_key,
            )?;
        }
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "compactAdvice".to_owned(),
                if series_boundary {
                    json!({"kind":"suggest-compact","reason":"series-boundary","advisory":true})
                } else {
                    Value::Null
                },
            );
        }
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "delegation.collected",
            Some(identity.workspace_id),
            Some(identity.session_id),
            Some(work_item_id),
        )
        .map(|(value, _)| value)
    }

    pub(super) fn delegation_cancel(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let payload: DelegationCancelPayload = decode_value(params.payload.clone())?;
        if payload.reason.trim().is_empty()
            || payload.reason.len() > 1_024
            || payload.reason.contains(['\0', '\r', '\n'])
        {
            return Err(schema_error("delegation cancellation reason is invalid"));
        }
        let id = payload.delegation_id.as_str();
        let key = require_idempotency(params)?;
        let scope = format!("delegation-cancel\0{}", id);
        let digest = canonical_digest(&payload)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let mut value = to_value(self.delegation.cancel(&identity.session_id, id)?)?;
        value
            .as_object_mut()
            .expect("delegation projection is an object")
            .insert("cancellationReason".to_owned(), json!(payload.reason));
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "delegation.cancelled",
            Some(identity.workspace_id),
            Some(identity.session_id),
            params.work_item_id.clone(),
        )
        .map(|(value, _)| value)
    }

    pub(super) fn delegation_renew(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let payload: DelegationRenewPayload = decode_value(params.payload.clone())?;
        let id = payload.delegation_id.as_str();
        let key = require_idempotency(params)?;
        let scope = format!("delegation-renew\0{}", id);
        let digest = canonical_digest(&payload)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let typed_scope_digest = canonical_digest(&json!({
            "domain":"delegation.renew/v1",
            "workspaceId":identity.workspace_id,
            "delegationId":id,
        }))?;
        let value = to_value(self.delegation.renew(
            &identity.session_id,
            id,
            payload.deadline_unix_ms,
            self.config.max_delegation_lifetime_ms,
            self.clock.now_unix_ms(),
            &typed_scope_digest,
            key,
            &digest,
        )?)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "delegation.renewed",
            Some(identity.workspace_id),
            Some(identity.session_id),
            params.work_item_id.clone(),
        )
        .map(|(value, _)| value)
    }

    pub(super) fn compact_request(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let payload: CompactRequestPayload = decode_value(params.payload.clone())?;
        if payload.adapter_id.is_empty()
            || payload.deadline_unix_ms <= self.clock.now_unix_ms()
            || payload.snapshot_digest.len() != 64
            || payload
                .snapshot_digest
                .bytes()
                .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
        {
            return Err(schema_error(
                "compact request requires a registered adapter, a future deadline, and a lowercase snapshot digest",
            ));
        }
        let key = require_idempotency(params)?;
        let scope = format!("compact-request\0{}", identity.session_id);
        let digest = canonical_digest(&payload)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let result = self.start_compact(
            &identity.session_id,
            payload.previous_generation,
            &payload.snapshot_digest,
            payload.deadline_unix_ms,
            &payload.adapter_id,
        )?;
        let value = to_value(result)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "compact.requested",
            Some(identity.workspace_id),
            Some(identity.session_id),
            params.work_item_id.clone(),
        )
        .map(|(value, _)| value)
    }

    pub(super) fn compact_status(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let compact_id = payload_string(&params.payload, "compactId")?;
        self.persistence
            .load_record("compact/v1", compact_id)?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::CompactAckInvalid,
                    "compact cycle does not exist",
                )
            })
    }

    fn capture_flow_delegation_intent(
        &self,
        params: &RequestParams<Value>,
        result: &Value,
    ) -> RuntimeResult<()> {
        let action_kind = result.pointer("/nextAction/kind").and_then(Value::as_str);
        let workspace_id = require(&params.workspace_id, "workspaceId")?.to_owned();
        let work_item_id = require(&params.work_item_id, "workItemId")?.to_owned();
        let decision_digest = result
            .get("decisionDigest")
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("delegate-series flow decision lacks decisionDigest"))?
            .to_owned();
        let delegation_intent = match action_kind {
            Some("delegate-series") => {
                let series_kind = result
                    .pointer("/nextAction/seriesKind")
                    .and_then(Value::as_str)
                    .ok_or_else(|| schema_error("delegate-series flow decision lacks seriesKind"))?
                    .to_owned();
                let required_artifacts = result
                    .pointer("/nextAction/requiredArtifacts")
                    .and_then(Value::as_array)
                    .ok_or_else(|| {
                        schema_error("delegate-series flow decision lacks requiredArtifacts")
                    })?
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| schema_error("requiredArtifacts must contain strings"))
                    })
                    .collect::<RuntimeResult<Vec<_>>>()?;
                Some((series_kind, required_artifacts))
            }
            Some("await-agent-work") => match result.get("phase").and_then(Value::as_str) {
                Some("coding") => Some(("coding".to_owned(), Vec::new())),
                Some("test-running") => Some(("testing".to_owned(), Vec::new())),
                _ => None,
            },
            Some("execute-approved-slice")
                if result.get("phase").and_then(Value::as_str) == Some("coding") =>
            {
                Some(("coding".to_owned(), Vec::new()))
            }
            Some("collect-review-contributions") => Some(("review".to_owned(), Vec::new())),
            _ => None,
        };
        let carries_delegation_intent = delegation_intent.is_some();
        let state_revision = result
            .get("stateRevision")
            .and_then(Value::as_u64)
            .ok_or_else(|| schema_error("delegate-series flow decision lacks stateRevision"))?;
        let input_fingerprint = result
            .get("inputFingerprint")
            .and_then(Value::as_str)
            .unwrap_or(&decision_digest)
            .to_owned();
        let decision_digest_version = result
            .get("decisionDigestVersion")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let now_unix_ms = self.clock.now_unix_ms();
        let mut pending_intent = None;
        if let Some((series_kind, required_artifacts)) = delegation_intent {
            let intent_key =
                flow_delegation_intent_key(&workspace_id, &work_item_id, &decision_digest);
            let existing_intent = if let Some(value) = self
                .persistence
                .load_record("flow-delegation-intent/v1", &intent_key)?
            {
                let existing: FlowDelegationIntent = decode_value(value)?;
                if existing.workspace_id != workspace_id
                    || existing.work_item_id != work_item_id
                    || existing.decision_digest != decision_digest
                    || existing.series_kind != series_kind
                    || existing.state_revision != state_revision
                    || existing.input_fingerprint != input_fingerprint
                    || existing.required_artifacts != required_artifacts
                {
                    return Err(RuntimeError::new(
                        StableErrorCode::DelegationAttestationFailed,
                        "replayed flow decision does not match its committed delegation intent",
                    ));
                }
                Some(existing)
            } else {
                None
            };
            let intent_is_current = existing_intent.as_ref().is_some_and(|existing| {
                existing.asset_refs.as_ref().is_some_and(|asset_refs| {
                    asset_refs
                        .iter()
                        .all(|asset_ref| asset_ref.byte_length.is_some())
                }) && existing
                    .deadline_unix_ms
                    .is_some_and(|deadline| deadline > now_unix_ms)
            });
            if !intent_is_current {
                let deadline_unix_ms = now_unix_ms
                    .checked_add(DEFAULT_SERIES_DELEGATION_TTL_MS)
                    .ok_or_else(|| schema_error("Series delegation deadline overflow"))?;
                let intent = if let Some(mut existing) = existing_intent {
                    existing.deadline_unix_ms = Some(deadline_unix_ms);
                    existing
                } else {
                    let asset_refs = frozen_projection_asset_refs(result)?;
                    FlowDelegationIntent {
                        schema_version: "flow-delegation-intent/v1".to_owned(),
                        workspace_id: workspace_id.clone(),
                        work_item_id: work_item_id.clone(),
                        input_fingerprint: input_fingerprint.clone(),
                        decision_digest: decision_digest.clone(),
                        series_kind,
                        state_revision,
                        retry_of_series_run_id: result
                            .get("retryOfSeriesRunId")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        flow_run_id: result
                            .get("flowRunId")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        required_artifacts,
                        asset_refs: Some(asset_refs),
                        deadline_unix_ms: Some(deadline_unix_ms),
                    }
                };
                pending_intent = Some((intent_key, intent));
            }
        }

        let current_key = current_flow_delegation_intent_key(&workspace_id, &work_item_id);
        let mut update_current = true;
        if let Some(value) = self
            .persistence
            .load_record("flow-delegation-current/v1", &current_key)?
        {
            let current: CurrentFlowDelegationIntent = decode_value(value)?;
            if current.workspace_id != workspace_id || current.work_item_id != work_item_id {
                return Err(RuntimeError::new(
                    StableErrorCode::DelegationAttestationFailed,
                    "current flow delegation intent has inconsistent authority bindings",
                ));
            }
            if state_revision < current.state_revision {
                return Ok(());
            }
            if state_revision == current.state_revision {
                // A phase transition can legitimately produce a different
                // projection at the same revision. It is not a competing
                // delegation intent, so preserve the last frozen reference.
                if !carries_delegation_intent {
                    return Ok(());
                }
                let legacy_to_v2 = current.decision_digest_version.is_none()
                    && decision_digest_version.as_deref() == Some("v2")
                    && input_fingerprint == current.input_fingerprint;
                let same_series_boundary_refresh = if !legacy_to_v2
                    && decision_digest != current.decision_digest
                    && input_fingerprint != current.input_fingerprint
                {
                    let prior_key = flow_delegation_intent_key(
                        &workspace_id,
                        &work_item_id,
                        &current.decision_digest,
                    );
                    let next_key =
                        flow_delegation_intent_key(&workspace_id, &work_item_id, &decision_digest);
                    let prior: Option<FlowDelegationIntent> = self
                        .persistence
                        .load_record("flow-delegation-intent/v1", &prior_key)?
                        .map(decode_value)
                        .transpose()?;
                    let next: Option<FlowDelegationIntent> =
                        if let Some((candidate_key, candidate)) = pending_intent.as_ref() {
                            (candidate_key == &next_key).then(|| candidate.clone())
                        } else {
                            self.persistence
                                .load_record("flow-delegation-intent/v1", &next_key)?
                                .map(decode_value)
                                .transpose()?
                        };
                    match (prior, next) {
                        (Some(prior), Some(next)) => {
                            prior.state_revision == next.state_revision
                                && prior.series_kind == next.series_kind
                                && prior.required_artifacts == next.required_artifacts
                        }
                        _ => false,
                    }
                } else {
                    false
                };
                if !legacy_to_v2
                    && !same_series_boundary_refresh
                    && (decision_digest != current.decision_digest
                        || input_fingerprint != current.input_fingerprint)
                {
                    return Err(RuntimeError::new(
                        StableErrorCode::DelegationAttestationFailed,
                        "one state revision produced conflicting flow delegation intents",
                    ));
                }
                if !legacy_to_v2 && !same_series_boundary_refresh {
                    update_current = false;
                }
            }
        }
        if let Some((intent_key, intent)) = pending_intent {
            self.persistence.store_record(
                "flow-delegation-intent/v1",
                &intent_key,
                &to_value(intent)?,
            )?;
        }
        if !update_current {
            return Ok(());
        }
        self.persistence.store_record(
            "flow-delegation-current/v1",
            &current_key,
            &to_value(CurrentFlowDelegationIntent {
                schema_version: "flow-delegation-current/v1".to_owned(),
                workspace_id,
                work_item_id,
                decision_digest,
                state_revision,
                input_fingerprint,
                decision_digest_version,
            })?,
        )
    }

    fn root_series_delegation_payload(
        &self,
        params: &RequestParams<Value>,
        identity: &TrustedSession,
        reference: &RootSeriesDelegationPayload,
    ) -> RuntimeResult<DelegationCreatePayload> {
        let work_item_id = require(&params.work_item_id, "workItemId")?;
        let value = self
            .persistence
            .load_record(
                "flow-delegation-intent/v1",
                &flow_delegation_intent_key(
                    &identity.workspace_id,
                    work_item_id,
                    &reference.flow_decision_digest,
                ),
            )?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::DelegationAttestationFailed,
                    "Root delegation reference does not name a committed flow intent",
                )
            })?;
        let intent: FlowDelegationIntent = decode_value(value)?;
        // F-10: the fingerprint is deliberately *not* compared against the
        // decision digest. Requiring equality was the read half of the same
        // conflation — it forced every committed intent to carry a fingerprint
        // that proved nothing beyond the decision it already named, and it would
        // now reject any intent carrying a genuine input fingerprint. The
        // fingerprint is still bound: it is committed with the intent and
        // re-checked by the delegation supervisor against the child record.
        if intent.workspace_id != identity.workspace_id
            || intent.work_item_id != work_item_id
            || intent.decision_digest != reference.flow_decision_digest
            || intent.input_fingerprint.is_empty()
        {
            return Err(RuntimeError::new(
                StableErrorCode::DelegationAttestationFailed,
                "committed flow delegation intent has inconsistent authority bindings",
            ));
        }
        let deadline_unix_ms = intent.deadline_unix_ms.ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::DelegationAttestationFailed,
                "committed flow delegation intent predates frozen deadline; rerun flow.next",
            )
        })?;
        let briefing = if intent.required_artifacts.is_empty() {
            format!("Execute the daemon-committed {} Series", intent.series_kind)
        } else {
            format!(
                "Execute the daemon-committed {} Series and produce {}",
                intent.series_kind,
                intent.required_artifacts.join(", ")
            )
        };
        let frozen_asset_refs = intent.asset_refs.ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::DelegationAttestationFailed,
                "committed flow delegation intent predates frozen assetRefs; rerun flow.next",
            )
        })?;
        if frozen_asset_refs
            .iter()
            .any(|asset_ref| asset_ref.byte_length.is_none())
        {
            return Err(RuntimeError::new(
                StableErrorCode::DelegationAttestationFailed,
                "committed flow delegation intent has legacy assetRefs without byteLength; rerun flow.next",
            ));
        }
        let asset_refs = (!frozen_asset_refs.is_empty()).then_some(frozen_asset_refs);
        Ok(DelegationCreatePayload {
            child_role: WireAgentRole::Series,
            parent_delegation_id: None,
            input_revision: intent.state_revision,
            input_fingerprint: intent.input_fingerprint,
            deadline_unix_ms,
            // Retry lineage is authority, not a root preference. It is read from
            // the committed flow intent for the same reason role, grant, revision
            // and deadline are: a root that could name its own predecessor could
            // forge a retry chain, or attach this attempt to a run in another
            // Work Item. The daemon still mints the new run identity itself.
            retry_of_series_run_id: intent.retry_of_series_run_id.clone(),
            // Same reasoning as the retry lineage: the Flow Run comes from the
            // committed intent, never from the root payload.
            flow_run_id: intent.flow_run_id.clone(),
            series_id: crate::supervisor::series_identity(
                &intent.work_item_id,
                &intent.series_kind,
            ),
            grant: semantic_series_grant(),
            briefing: Some(briefing),
            asset_refs,
        })
    }

    fn validate_current_root_series_delegation(
        &self,
        params: &RequestParams<Value>,
        identity: &TrustedSession,
        reference: &RootSeriesDelegationPayload,
    ) -> RuntimeResult<()> {
        let work_item_id = require(&params.work_item_id, "workItemId")?;
        let intent_value = self
            .persistence
            .load_record(
                "flow-delegation-intent/v1",
                &flow_delegation_intent_key(
                    &identity.workspace_id,
                    work_item_id,
                    &reference.flow_decision_digest,
                ),
            )?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::DelegationAttestationFailed,
                    "Root delegation reference does not name a committed flow intent",
                )
            })?;
        let intent: FlowDelegationIntent = decode_value(intent_value)?;
        let current_value = self
            .persistence
            .load_record(
                "flow-delegation-current/v1",
                &current_flow_delegation_intent_key(&identity.workspace_id, work_item_id),
            )?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::DelegationAttestationFailed,
                    "Root delegation reference predates current flow intent authority; rerun flow.next",
                )
            })?;
        let current: CurrentFlowDelegationIntent = decode_value(current_value)?;
        if current.workspace_id != identity.workspace_id
            || current.work_item_id != work_item_id
            || current.decision_digest != intent.decision_digest
            || current.state_revision != intent.state_revision
            || current.input_fingerprint != intent.input_fingerprint
        {
            return Err(RuntimeError::new(
                StableErrorCode::DelegationAttestationFailed,
                "committed flow delegation intent has inconsistent authority bindings",
            ));
        }
        Ok(())
    }

    pub(super) fn authoritative_business(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
        client_kind: Option<ClientKind>,
    ) -> RuntimeResult<Value> {
        let business_workspace = params
            .workspace_id
            .as_deref()
            .map(|workspace_id| {
                let state = self.lock_state()?;
                let record = state
                    .workspaces
                    .get(workspace_id)
                    .ok_or_else(|| project_mismatch("workspace is not registered"))?;
                let session = params
                    .session_id
                    .as_deref()
                    .and_then(|session_id| state.sessions.get(session_id));
                let agent_role = session.map(|session| AgentRole::from(session.result.role));
                let agent_grant = session
                    .map(|session| session.grant.to_domain())
                    .transpose()?;
                Ok(BusinessWorkspace {
                    workspace_id: record.result.workspace_id.clone(),
                    canonical_root: record.result.canonical_root.clone(),
                    project_key: record.result.project_key.clone(),
                    mode: record.result.mode,
                    agent_role,
                    agent_grant,
                    caller_kind: client_kind,
                    inventory_generation: record.result.inventory_generation,
                })
            })
            .transpose()?;
        self.enforce_writer_mode(method, params, business_workspace.as_ref())?;
        let result = if let (Some(workspace), Some(work_item)) =
            (&params.workspace_id, &params.work_item_id)
        {
            self.actors
                .execute(workspace, work_item, params.deadline_ms, || {
                    let result =
                        self.business
                            .execute(method, params, business_workspace.as_ref())?;
                    if method == RpcMethod::FlowNext {
                        self.capture_flow_delegation_intent(params, &result)?;
                    }
                    Ok(result)
                })
        } else {
            let result = self
                .business
                .execute(method, params, business_workspace.as_ref())?;
            if method == RpcMethod::FlowNext {
                self.capture_flow_delegation_intent(params, &result)?;
            }
            Ok(result)
        }?;
        if method == RpcMethod::OperationExecute
            && !params
                .payload
                .get("dryRun")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && params
                .payload
                .get("operation")
                .and_then(Value::as_str)
                .and_then(|name| ae_sdd_operations::OperationName::from_str(name).ok())
                .is_some_and(|operation| operation.spec().writes)
            && let (Some(workspace_id), Some(work_item_id)) = (
                params.workspace_id.as_deref(),
                params.work_item_id.as_deref(),
            )
        {
            self.refresh_work_item_contexts(workspace_id, work_item_id)?;
        }
        Ok(result)
    }

    fn enforce_writer_mode(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
        workspace: Option<&BusinessWorkspace>,
    ) -> RuntimeResult<()> {
        if method != RpcMethod::OperationExecute {
            return Ok(());
        }
        let Some(operation_name) = params.payload.get("operation").and_then(Value::as_str) else {
            return Ok(());
        };
        let Ok(operation) = ae_sdd_operations::OperationName::from_str(operation_name) else {
            return Ok(());
        };
        if !operation.spec().writes {
            return Ok(());
        }
        let workspace = workspace.ok_or_else(|| project_mismatch("workspace is not registered"))?;
        if !matches!(
            workspace.mode,
            WorkspaceMode::RustCanary | WorkspaceMode::RustSoleWriter
        ) {
            return Err(RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "Rust mutation is forbidden until the daemon owns the workspace writer mode",
            ));
        }
        if operation == ae_sdd_operations::OperationName::LeaseBreak
            && workspace.caller_kind == Some(ClientKind::Admin)
        {
            return Ok(());
        }
        let identity = self.session_identity(params, false)?;
        if !identity.engaged || identity.capability_id != "hook.engaged" {
            return Err(RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "an unengaged Hook capability cannot authorize a Rust mutation",
            ));
        }
        Ok(())
    }

    pub(super) fn start_compact(
        &self,
        session_id: &str,
        previous_generation: u64,
        snapshot_digest: &str,
        deadline_unix_ms: u64,
        adapter_id: &str,
    ) -> RuntimeResult<CompactResult> {
        self.host.require_registered(adapter_id)?;
        {
            let state = self.lock_state()?;
            let session = state.sessions.get(session_id).ok_or_else(session_expired)?;
            if session.result.context_generation != previous_generation {
                return Err(RuntimeError::new(
                    StableErrorCode::CompactAckInvalid,
                    "compact previous generation does not match the trusted session",
                ));
            }
        }
        let compact_id = Uuid::new_v4().to_string();
        let action = self.host.enqueue(
            adapter_id,
            "compact",
            None,
            Some(compact_id.clone()),
            Some(session_id.to_owned()),
            Some(previous_generation),
            deadline_unix_ms,
        )?;
        let result = CompactResult {
            compact_id: compact_id.clone(),
            status: "compact-requested".to_owned(),
            previous_generation,
            next_generation: previous_generation.checked_add(1).ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::CompactAckInvalid,
                    "context generation overflow",
                )
            })?,
            action_id: action.action_id,
            restored_projection_digest: None,
        };
        let mut value = to_value(&result)?;
        value["snapshotDigest"] = Value::String(snapshot_digest.to_owned());
        value["sessionId"] = Value::String(session_id.to_owned());
        self.persistence
            .store_record("compact/v1", &compact_id, &value)?;
        self.persistence.store_record(
            "compact-active/v1",
            session_id,
            &json!({"schemaVersion":"compact-active/v1","compactId":compact_id}),
        )?;
        Ok(result)
    }

    pub(super) fn update_compact_status(
        &self,
        compact_id: &str,
        status: &str,
        restored_digest: Option<&str>,
    ) -> RuntimeResult<()> {
        let mut value = self
            .persistence
            .load_record("compact/v1", compact_id)?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::CompactAckInvalid,
                    "compact cycle does not exist",
                )
            })?;
        value["status"] = Value::String(status.to_owned());
        if let Some(digest) = restored_digest {
            value["restoredProjectionDigest"] = Value::String(digest.to_owned());
        }
        self.persistence
            .store_record("compact/v1", compact_id, &value)
    }

    pub(super) fn complete_compact_after_rehydrate(
        &self,
        session_id: &str,
        restored_digest: &str,
    ) -> RuntimeResult<()> {
        // The active cycle identity is persisted as a session-scoped pointer.
        let Some(pointer) = self
            .persistence
            .load_record("compact-active/v1", session_id)?
        else {
            return Ok(());
        };
        let compact_id = pointer
            .get("compactId")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "active compact pointer is malformed",
                )
            })?;
        let cycle = self
            .persistence
            .load_record("compact/v1", compact_id)?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::CompactAckInvalid,
                    "active compact cycle is missing",
                )
            })?;
        if cycle.get("status").and_then(Value::as_str) == Some("context-restored") {
            return Ok(());
        }
        if cycle.get("status").and_then(Value::as_str) != Some("host-acknowledged") {
            return Err(RuntimeError::new(
                StableErrorCode::CompactAckInvalid,
                "context cannot be restored before correlated host ACK",
            ));
        }
        let previous = cycle
            .get("previousGeneration")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "compact previous generation is malformed",
                )
            })?;
        let next = cycle
            .get("nextGeneration")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "compact next generation is malformed",
                )
            })?;
        {
            let mut state = self.lock_state()?;
            let session = state
                .sessions
                .get_mut(session_id)
                .ok_or_else(session_expired)?;
            if session.result.context_generation != previous {
                return Err(RuntimeError::new(
                    StableErrorCode::CompactAckInvalid,
                    "context generation changed before compact rehydrate CAS",
                ));
            }
            session.result.context_generation = next;
        }
        self.persist_session(session_id)?;
        self.update_compact_status(compact_id, "context-restored", Some(restored_digest))?;
        self.append_runtime_event(
            "compact.context_restored",
            json!({
                "compactId":compact_id,
                "previousGeneration":previous,
                "nextGeneration":next,
                "restoredProjectionDigest":restored_digest,
            }),
            Some(
                self.lock_state()?
                    .sessions
                    .get(session_id)
                    .ok_or_else(session_expired)?
                    .workspace_id
                    .clone(),
            ),
            Some(session_id.to_owned()),
            None,
        )?;
        Ok(())
    }
}

fn frozen_projection_asset_refs(projection: &Value) -> RuntimeResult<Vec<crate::AssetRefWire>> {
    match projection.get("assetRefs") {
        Some(Value::Array(values)) => {
            let asset_refs: Vec<crate::AssetRefWire> = decode_value(Value::Array(values.clone()))?;
            if asset_refs
                .iter()
                .any(|asset_ref| asset_ref.byte_length.is_none())
            {
                return Err(schema_error(
                    "fresh authoritative context assetRefs require byteLength",
                ));
            }
            Ok(asset_refs)
        }
        Some(Value::Null) | None => Ok(Vec::new()),
        Some(_) => Err(schema_error(
            "authoritative context assetRefs must be an array or null",
        )),
    }
}

fn flow_delegation_intent_key(
    workspace_id: &str,
    work_item_id: &str,
    decision_digest: &str,
) -> String {
    format!("{workspace_id}\0{work_item_id}\0{decision_digest}")
}

fn current_flow_delegation_intent_key(workspace_id: &str, work_item_id: &str) -> String {
    format!("{workspace_id}\0{work_item_id}")
}

#[derive(Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DelegationCollectPayload {
    delegation_id: String,
}

#[derive(Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DelegationCancelPayload {
    delegation_id: String,
    #[serde(default = "default_cancellation_reason")]
    reason: String,
}

#[derive(Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DelegationRenewPayload {
    delegation_id: String,
    deadline_unix_ms: u64,
}

fn default_cancellation_reason() -> String {
    "user-abort".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_projection_asset_refs_require_byte_length() {
        let error = frozen_projection_asset_refs(&json!({
            "assetRefs":[{
                "kind":"requirements-analysis",
                "path":"ae-sdd-doc/RA/WORK.md",
                "sha256":"a".repeat(64)
            }]
        }))
        .expect_err("fresh projection refs must carry byteLength");

        assert!(error.message().contains("byteLength"), "{error}");
    }
}
