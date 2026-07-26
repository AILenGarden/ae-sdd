use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ae_sdd_contracts::execution_runtime::{ExecutionCapsuleV1, ExecutionSliceStatus};
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, BootId, DesignRoute, EventStoreId, FencingToken, GateOutcome,
    InputFingerprint, LeaseId, OperationId, ProcessPhase, ProjectKey, ProjectRelativePath,
    RequestId, ResultDigest, ScopedGrant, SessionId, StateRevision, WorkItemId, WorkScale,
    WorkspaceId,
};
use ae_sdd_execution::CapsuleBuildOutcome;
use ae_sdd_flow::{
    ExecutionCursor, FlowEnvironment, FlowInput, FlowSnapshot, NextAction, RouteSelection,
};
use ae_sdd_gates::GateRegistry;
use ae_sdd_operations::{
    Confirmation, ExecutionIdentity, OPERATION_REGISTRY, OperationBackend, OperationName,
    OperationRequest, OperationResponse, OperationService, OperationServiceError,
    ValidatedOperationRequest,
};
use ae_sdd_policy::{RequiredGate, RoleOperation, RolePolicy};
use ae_sdd_protocol::{ClientKind, RequestParams, RpcMethod, StableErrorCode, WorkspaceMode};
use ae_sdd_runtime::{
    BoundJobIdentity, BusinessOperationPort, BusinessWorkspace, ContextProjectionInput,
    DurableEvent, FlowSupervisor, PersistencePort, RuntimeError, RuntimeResult,
};
use ae_sdd_store::{
    AuthoritySnapshot, CommitFaultPort, CommitPoint, CommittedMutation, IdempotencyKey,
    JournalEvent, LeaseControlAction, LeaseControlRequest, LeaseLedger, LeaseOwner, LeaseProof,
    LeaseRecord, MutationRequest, MutationTarget, ProjectMutationStore, ProjectStorePaths,
    RuntimeEventPayload, SqliteRuntimeRepository, StateAuthority, StdCrossProcessLock,
    StdDurableFileSystem, StoreError, UtcTimestamp,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::operation_semantics::{evidence, governance, verification};
use crate::persistence::{
    ReviewProjectionWrite, rebuild_review_authority_projections, upsert_review_authority_projection,
};
use crate::review_authority::AuthenticatedCaller;
use crate::{
    AuthoritativeGateRuntime, ReviewGateAuthority, execution_authority, gate_result_json,
    lifecycle_authority, review_authority,
};

#[path = "jobs/mod.rs"]
mod jobs;

/// Debug-only commit-abort failpoint used by process crash/restart tests.
///
/// `AE_SDD_TEST_COMMIT_ABORT_AT` selects the commit point (`after_prepared` or
/// `after_replace_0`). An optional `@<operation>` suffix restricts the abort to
/// one operation, so a crash test can kill a specific authoritative mutation
/// without also killing the lease acquisition that must precede it.
///
/// `Disarmed` never aborts, so read-only and non-targeted commit paths keep
/// their previous behaviour.
#[derive(Clone, Debug, Default)]
enum ProcessAbortCommitFault {
    /// Never aborts, whatever the environment requests.
    #[default]
    Disarmed,
    /// Aborts at the requested commit point for any operation.
    AnyOperation,
    /// Aborts at the requested commit point only for this operation.
    Operation(String),
}

impl ProcessAbortCommitFault {
    /// Returns whether the environment-selected operation scope matches.
    fn scope_matches(&self, requested_operation: Option<&str>) -> bool {
        match (self, requested_operation) {
            (Self::Disarmed, _) => false,
            (Self::AnyOperation, _) => true,
            (Self::Operation(current), Some(requested)) => current == requested,
            (Self::Operation(_), None) => true,
        }
    }
}

impl CommitFaultPort for ProcessAbortCommitFault {
    fn reached(&self, point: CommitPoint) -> Result<(), StoreError> {
        #[cfg(debug_assertions)]
        {
            if let Ok(requested) = std::env::var("AE_SDD_TEST_COMMIT_ABORT_AT") {
                let (requested_point, requested_operation) = match requested.split_once('@') {
                    Some((point, operation)) => (point, Some(operation)),
                    None => (requested.as_str(), None),
                };
                let point_matches = matches!(
                    (requested_point, point),
                    ("after_prepared", CommitPoint::AfterPreparedJournal)
                        | ("after_replace_0", CommitPoint::AfterTargetReplace(0))
                );
                if point_matches && self.scope_matches(requested_operation) {
                    std::process::abort();
                }
            }
        }
        #[cfg(not(debug_assertions))]
        let _ = point;
        Ok(())
    }
}

/// Production Rust boundary for authoritative project operations.
#[derive(Clone)]
pub struct NativeBusinessAdapter {
    database: PathBuf,
    event_store_id: EventStoreId,
    boot_id: BootId,
    policy_digest: String,
    persistence: Arc<dyn PersistencePort>,
    flow: Arc<FlowSupervisor>,
}

impl NativeBusinessAdapter {
    /// Creates an adapter that shares the daemon's durable event-store epoch.
    #[must_use]
    pub fn new(
        database: PathBuf,
        event_store_id: EventStoreId,
        boot_id: BootId,
        policy_digest: String,
        persistence: Arc<dyn PersistencePort>,
    ) -> Self {
        let flow = Arc::new(FlowSupervisor::new(Arc::clone(&persistence)));
        Self {
            database,
            event_store_id,
            boot_id,
            policy_digest,
            persistence,
            flow,
        }
    }

    fn commit_toolset_receipt(
        &self,
        workspace: &BusinessWorkspace,
        work_item_id: &str,
        identity: &BoundJobIdentity,
        arguments: &Value,
        base_result: Value,
    ) -> RuntimeResult<Value> {
        let wire: ToolsetReceiptCommitWire = decode(arguments.clone())?;
        let located = read_state(workspace, work_item_id)?;
        let paths = ProjectStorePaths::new(&workspace.canonical_root, located.relative.clone())
            .map_err(store_error)?;
        let repository = SqliteRuntimeRepository::open(
            &self.database,
            self.event_store_id,
            &UtcTimestamp::now(),
        )
        .map_err(store_error)?;
        let store = ProjectMutationStore::with_faults(
            paths,
            StdDurableFileSystem,
            StdCrossProcessLock,
            repository,
            ProcessAbortCommitFault::AnyOperation,
        );
        store.recover(UtcTimestamp::now()).map_err(store_error)?;
        let workspace_id: WorkspaceId = parse(&workspace.workspace_id, "workspaceId")?;
        let work_item_id = WorkItemId::new(work_item_id.to_owned()).map_err(domain_error)?;
        let operation: OperationId = parse("toolset.receipt.record", "operation")?;
        let idempotency = IdempotencyKey::new(format!(
            "toolset-receipt:{}",
            hex::encode(Sha256::digest(identity.job_id.as_bytes()))
        ))
        .map_err(store_error)?;
        let payload_bytes = serde_json::to_vec(&json!({
            "schemaVersion": 1,
            "jobId": identity.job_id,
            "identityDigest": identity.identity_digest,
            "arguments": arguments,
        }))
        .map_err(|_| schema_error("toolset project mutation could not be canonicalized"))?;
        let payload_digest = InputFingerprint::digest(payload_bytes);
        if let Some(committed) = store
            .replay_committed(
                workspace_id,
                &work_item_id,
                &operation,
                &idempotency,
                payload_digest,
            )
            .map_err(store_error)?
        {
            return committed_result_data(&committed);
        }

        let before_bytes = fs::read(&located.absolute).map_err(io_error)?;
        let authority = StateAuthority::inspect(&before_bytes).map_err(store_error)?;
        if authority.revision().get() != wire.source_revision {
            return Err(RuntimeError::new(
                StableErrorCode::StaleGateResult,
                "toolset receipt sourceRevision is stale for project commit",
            ));
        }
        if wire.inventory_generation != workspace.inventory_generation {
            return Err(RuntimeError::new(
                StableErrorCode::StaleGateResult,
                "toolset receipt inventoryGeneration is stale for project commit",
            ));
        }
        let lease_proof = LeaseProof {
            lease_id: parse(&wire.lease_id, "leaseId")?,
            owner: LeaseOwner::new(identity.session_id.clone()).map_err(store_error)?,
            fencing_token: FencingToken::new(wire.fencing_token),
        };
        store
            .validate_lease_proof(&lease_proof, &UtcTimestamp::now())
            .map_err(store_error)?;

        let result = base_result
            .as_object()
            .ok_or_else(|| schema_error("toolset receipt result must be an object"))?;
        let receipt_id = toolset_result_string(result, "receiptId", 128)?.to_owned();
        let receipt_digest = toolset_result_digest(result, "receiptDigest")?.to_owned();
        let plan_digest = toolset_result_digest(result, "planDigest")?.to_owned();
        let methodology_digest = toolset_result_digest(result, "methodologyDigest")?.to_owned();
        let policy_digest = toolset_result_digest(result, "policyDigest")?.to_owned();
        let input_fingerprint = toolset_result_digest(result, "inputFingerprint")?.to_owned();
        let identity_digest = toolset_result_digest(result, "identityDigest")?.to_owned();
        if result.get("outcome").and_then(Value::as_str) != Some("PASS")
            || result.get("validated").and_then(Value::as_bool) != Some(true)
            || result.get("toolsetJobId").and_then(Value::as_str) != Some(identity.job_id.as_str())
            || result.get("workItemId").and_then(Value::as_str) != Some(work_item_id.as_str())
            || result.get("sourceRevision").and_then(Value::as_u64) != Some(wire.source_revision)
            || result.get("inventoryGeneration").and_then(Value::as_u64)
                != Some(wire.inventory_generation)
            || methodology_digest != wire.methodology_digest
            || policy_digest != wire.policy_digest
            || identity_digest != identity.identity_digest
            || wire.receipt.get("status").and_then(Value::as_str) != Some("pass")
        {
            return Err(schema_error(
                "toolset PASS result does not match its trusted project commit input",
            ));
        }

        let revision_after = authority
            .revision()
            .checked_next()
            .map_err(|_| schema_error("state revision overflow"))?;
        let mutation_id = RequestId::from_uuid(Uuid::new_v4());
        let artifact_ref = ProjectRelativePath::new(format!(
            ".auto-engineering/{}/evidence/toolset/{receipt_id}.json",
            work_item_id.as_str()
        ))
        .map_err(domain_error)?;
        let artifact_absolute = Path::new(&workspace.canonical_root).join(artifact_ref.as_str());
        match fs::read(&artifact_absolute) {
            Ok(_) => {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "immutable toolset receipt locator already exists",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(error)),
        }
        let snapshot = json!({
            "schemaVersion": 1,
            "kind": "toolsetReceiptAuthority",
            "toolsetJobId": identity.job_id,
            "workspaceId": workspace.workspace_id,
            "workItemId": work_item_id.as_str(),
            "outcome": "PASS",
            "status": "pass",
            "validated": true,
            "receiptId": receipt_id,
            "receiptDigest": receipt_digest,
            "plan": wire.plan,
            "receipt": wire.receipt,
            "planDigest": plan_digest,
            "methodologyDigest": methodology_digest,
            "policyDigest": policy_digest,
            "inputFingerprint": input_fingerprint,
            "sourceRevision": revision_after.get(),
            "inventoryGeneration": wire.inventory_generation,
            "identityDigest": identity.identity_digest,
            "mutationId": mutation_id.to_string(),
            "recorder": {
                "sessionId": identity.session_id,
                "rootSessionId": identity.root_session_id,
                "delegationId": identity.delegation_id,
                "contextGeneration": identity.context_generation,
            },
        });
        let snapshot_bytes = pretty_json_line(&snapshot)?;
        let project_receipt_digest = ArtifactDigest::digest(&snapshot_bytes).to_string();
        let (manifest_ref, manifest_before, manifest_bytes) = prepare_toolset_manifest(
            Path::new(&workspace.canonical_root),
            work_item_id.as_str(),
            &receipt_id,
            &identity.job_id,
            &receipt_digest,
            &plan_digest,
            &methodology_digest,
            &policy_digest,
            &input_fingerprint,
            revision_after.get(),
            wire.inventory_generation,
            &identity.session_id,
            artifact_ref.as_str(),
            &project_receipt_digest,
            &wire.receipt,
        )?;
        let manifest_digest = ArtifactDigest::digest(&manifest_bytes).to_string();

        let mut after = located.value;
        let state_object = after
            .as_object_mut()
            .ok_or_else(|| schema_error("authoritative state must be an object"))?;
        state_object.insert("revision".to_owned(), Value::from(revision_after.get()));
        state_object.insert(
            "lastFencingToken".to_owned(),
            Value::from(wire.fencing_token),
        );
        state_object.insert(
            "toolsetReceiptRef".to_owned(),
            json!({
                "schemaVersion": 1,
                "toolsetJobId": identity.job_id,
                "receiptId": receipt_id,
                "receiptDigest": receipt_digest,
                "artifactRef": artifact_ref.as_str(),
                "projectReceiptDigest": project_receipt_digest,
                "manifestRef": manifest_ref.as_str(),
                "manifestDigest": manifest_digest,
                "mutationId": mutation_id.to_string(),
                "sourceRevision": revision_after.get(),
            }),
        );
        state_object.insert(
            "lastMutation".to_owned(),
            json!({
                "operation": "toolset.receipt.record",
                "idempotencyKey": idempotency.as_str(),
                "revisionBefore": authority.revision().get(),
                "revisionAfter": revision_after.get(),
            }),
        );
        let state_bytes = serde_json::to_vec_pretty(&after)
            .map_err(|_| schema_error("toolset receipt state could not be serialized"))?;

        let mut committed_result = base_result;
        let result_object = committed_result
            .as_object_mut()
            .ok_or_else(|| schema_error("toolset receipt result must be an object"))?;
        result_object.insert(
            "sourceRevision".to_owned(),
            Value::from(revision_after.get()),
        );
        result_object.insert(
            "revisionBefore".to_owned(),
            Value::from(authority.revision().get()),
        );
        result_object.insert(
            "revisionAfter".to_owned(),
            Value::from(revision_after.get()),
        );
        result_object.insert(
            "mutationId".to_owned(),
            Value::String(mutation_id.to_string()),
        );
        result_object.insert(
            "artifactRef".to_owned(),
            Value::String(artifact_ref.as_str().to_owned()),
        );
        result_object.insert(
            "receiptLocator".to_owned(),
            Value::String(artifact_ref.as_str().to_owned()),
        );
        result_object.insert(
            "projectReceiptDigest".to_owned(),
            Value::String(project_receipt_digest.clone()),
        );
        result_object.insert(
            "manifestRef".to_owned(),
            Value::String(manifest_ref.as_str().to_owned()),
        );
        result_object.insert("manifestDigest".to_owned(), Value::String(manifest_digest));

        let event_bytes = serde_json::to_vec(&json!({
            "operation": "toolset.receipt.record",
            "data": committed_result,
        }))
        .map_err(|_| schema_error("toolset receipt event could not be serialized"))?;
        let result_bytes = serde_json::to_vec(&committed_result)
            .map_err(|_| schema_error("toolset receipt result could not be serialized"))?;
        let mutation = MutationRequest {
            mutation_id,
            workspace_id,
            work_item_id,
            operation,
            idempotency_key: idempotency,
            canonical_payload_digest: payload_digest,
            expected_authority: authority,
            lease_proof,
            targets: vec![
                MutationTarget::new(
                    located.relative,
                    Some(ArtifactDigest::digest(&before_bytes)),
                    state_bytes,
                )
                .map_err(store_error)?,
                MutationTarget::new(artifact_ref, None, snapshot_bytes).map_err(store_error)?,
                MutationTarget::new(manifest_ref, manifest_before, manifest_bytes)
                    .map_err(store_error)?,
            ],
            event: JournalEvent {
                boot_id: self.boot_id,
                session_id: Some(parse(&identity.session_id, "sessionId")?),
                event_type: "toolset.receipt.record".to_owned().into_boxed_str(),
                schema_version: 1,
                payload: RuntimeEventPayload::InlineJson(event_bytes),
            },
            result_digest: ResultDigest::digest(&result_bytes),
            prepared_at: UtcTimestamp::now(),
            committed_at: UtcTimestamp::now(),
        };
        let committed = store.commit(mutation).map_err(store_error)?;
        if committed.replayed {
            return committed_result_data(&committed);
        }
        if committed.receipt.mutation_id != mutation_id
            || committed.receipt.revision_after != revision_after
        {
            return Err(RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "toolset receipt commit proof does not match the prepared mutation",
            ));
        }
        Ok(committed_result)
    }

    fn gate_evaluate(
        &self,
        workspace: Option<&BusinessWorkspace>,
        params: &RequestParams<Value>,
    ) -> RuntimeResult<Value> {
        let workspace = require_workspace(workspace)?;
        if workspace.agent_role != Some(AgentRole::Root) {
            return Err(RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "only a daemon-verified root session may evaluate authoritative Gates",
            ));
        }
        let work_item_id = require_work_item(params)?;
        let wire: GateEvaluateWire = decode(params.payload.clone())?;
        assert_expected_project_root(workspace, wire.expected_project_root.as_deref())?;
        match (wire.gate_id.as_deref(), wire.gate_ids.as_deref()) {
            (Some(gate_id), None) => {
                self.gate_evaluate_one(workspace, work_item_id, params, gate_id, None)
            }
            (None, Some(gate_ids)) if valid_gate_batch(gate_ids) => {
                let mut results = Vec::with_capacity(gate_ids.len());
                for gate_id in gate_ids {
                    let idempotency_key = params
                        .idempotency_key
                        .as_deref()
                        .map(|key| batch_gate_idempotency_key(key, gate_id));
                    results.push(self.gate_evaluate_one(
                        workspace,
                        work_item_id,
                        params,
                        gate_id,
                        idempotency_key.as_deref(),
                    )?);
                }
                let all_pass = results.iter().all(|result| {
                    result.pointer("/outcome/kind").and_then(Value::as_str) == Some("PASS")
                });
                Ok(json!({"allPass":all_pass,"results":results}))
            }
            _ => Err(schema_error(
                "exactly one of gateId or a non-empty bounded gateIds array is required",
            )),
        }
    }

    /// Production Review Gate dependencies: durable projections, runtime
    /// identity lineage, and the current daemon boot identity.
    fn review_gate_authority(&self, workspace: &BusinessWorkspace) -> ReviewGateAuthority {
        ReviewGateAuthority {
            database: self.database.clone(),
            persistence: Arc::clone(&self.persistence),
            boot_id: self.boot_id.to_string(),
            workspace: workspace.clone(),
        }
    }

    fn gate_evaluate_one(
        &self,
        workspace: &BusinessWorkspace,
        work_item_id: &str,
        params: &RequestParams<Value>,
        gate_id: &str,
        idempotency_key: Option<&str>,
    ) -> RuntimeResult<Value> {
        let gates = AuthoritativeGateRuntime::with_review_authority(
            workspace,
            work_item_id,
            &self.policy_digest,
            params.fencing_token,
            self.review_gate_authority(workspace),
        )?;
        let result = gates.evaluate(gate_id, Duration::from_millis(params.deadline_ms))?;
        let mut projection = gate_result_json(&result);

        let Some(required_gate) = required_gate(gate_id) else {
            return Ok(projection);
        };
        let located = read_state(workspace, work_item_id)?;
        let input = flow_input(&located.value, work_item_id, self.event_store_id)?;
        let current = self
            .flow
            .project(&workspace.workspace_id, work_item_id, input)?;
        if current.pending_transition().is_none()
            || !current.required_gates().contains(&required_gate)
        {
            return Ok(projection);
        }
        let idempotency_key = idempotency_key
            .or(params.idempotency_key.as_deref())
            .ok_or_else(|| {
                schema_error("idempotencyKey is required to record a pending transition Gate")
            })?;
        let decision = self.flow.record_gate(
            &self.boot_id.to_string(),
            &workspace.workspace_id,
            params.session_id.as_deref(),
            work_item_id,
            idempotency_key,
            input,
            required_gate,
            result.outcome(),
        )?;
        projection
            .as_object_mut()
            .expect("Gate projection is an object")
            .insert("flow".to_owned(), FlowSupervisor::projection(&decision));
        Ok(projection)
    }
}

impl BusinessOperationPort for NativeBusinessAdapter {
    fn execute(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
        workspace: Option<&BusinessWorkspace>,
    ) -> RuntimeResult<Value> {
        match method {
            RpcMethod::OperationDescribe => {
                let wire: DescribeWire = decode(params.payload.clone())?;
                operation_registry(wire.operation.as_deref())
            }
            RpcMethod::OperationExecute => {
                let workspace = require_workspace(workspace)?;
                let wire: ExecuteWire = decode(params.payload.clone())?;
                assert_expected_project_root(workspace, wire.expected_project_root.as_deref())?;
                assert_expected_project_key(workspace, wire.expected_project_key.as_deref())?;
                let operation = OperationName::from_str(&wire.operation).map_err(|_| {
                    RuntimeError::new(
                        StableErrorCode::OperationNotRegistered,
                        "typed operation is not registered",
                    )
                })?;
                if operation.spec().writes
                    && !matches!(
                        workspace.mode,
                        WorkspaceMode::RustCanary | WorkspaceMode::RustSoleWriter
                    )
                {
                    return Err(RuntimeError::new(
                        StableErrorCode::RoleOperationForbidden,
                        "project mutation is forbidden until Rust owns the workspace writer mode",
                    ));
                }
                let authorization_payload = wire.payload.clone();
                let request =
                    operation_request(operation, params, workspace, wire.payload, wire.dry_run)?;
                let backend = ProjectBackend::open(self, workspace, params)?;
                assert_story_scope(
                    &backend.state.value,
                    require_work_item(params)?,
                    wire.story.as_deref(),
                )?;
                let response = if operation == OperationName::LeaseBreak {
                    if workspace.caller_kind != Some(ClientKind::Admin) {
                        return Err(RuntimeError::new(
                            StableErrorCode::RoleOperationForbidden,
                            "lease.break requires an authenticated Admin client",
                        ));
                    }
                    OperationService::execute(ExecutionIdentity::Admin, request, &backend)
                } else {
                    let role = workspace.agent_role.ok_or_else(|| {
                        RuntimeError::new(
                            StableErrorCode::RoleOperationForbidden,
                            "typed operations require a daemon-verified Agent role",
                        )
                    })?;
                    RolePolicy::authorize(role, semantic_operation(role, operation)).map_err(
                        |_| {
                            RuntimeError::new(
                                StableErrorCode::RoleOperationForbidden,
                                "daemon-verified role forbids the typed operation",
                            )
                        },
                    )?;
                    let grant = workspace.agent_grant.as_ref().ok_or_else(|| {
                        RuntimeError::new(
                            StableErrorCode::RoleOperationForbidden,
                            "typed operations require a daemon-verified scoped grant",
                        )
                    })?;
                    authorize_operation_paths(
                        grant,
                        operation,
                        &authorization_payload,
                        &backend.state,
                        workspace,
                    )?;
                    if matches!(
                        operation,
                        OperationName::StateTransition | OperationName::WorkItemComplete
                    ) && request.confirmation.is_none()
                    {
                        let expected_revision = request.expected_revision.ok_or_else(|| {
                            schema_error("expectedRevision is required for lifecycle preflight")
                        })?;
                        let work_item_id = request.work_item_id.as_ref().ok_or_else(|| {
                            schema_error("workItemId is required for lifecycle preflight")
                        })?;
                        let preflight = lifecycle_authority::preflight_lifecycle_confirmation(
                            &backend.state.value,
                            work_item_id.as_str(),
                            operation,
                            &request.payload,
                            expected_revision,
                            role,
                            request.session_id,
                            system_time_unix_ms()?,
                        )?;
                        if preflight.disposition()
                            == lifecycle_authority::LifecycleAuthorityDisposition::Denied
                        {
                            return match preflight.into_permitted() {
                                Err(error) => Err(error),
                                Ok(_) => Err(schema_error(
                                    "denied lifecycle preflight unexpectedly became permitted",
                                )),
                            };
                        }
                        let binding = preflight.confirmation_binding().ok_or_else(|| {
                            schema_error("lifecycle preflight is missing its confirmation binding")
                        })?;
                        return Err(RuntimeError::new(
                            StableErrorCode::ConfirmationRequired,
                            "lifecycle authority requires confirmation",
                        )
                        .with_remediation(format!(
                            "provide lifecycle confirmation for binding {binding}"
                        )));
                    }
                    OperationService::execute(
                        ExecutionIdentity::Agent { role, grant },
                        request,
                        &backend,
                    )
                }
                .map_err(operation_error)?;
                Ok(response_value(response))
            }
            RpcMethod::FlowSnapshot | RpcMethod::FlowNext => {
                let workspace = require_workspace(workspace)?;
                let work_item_id = require_work_item(params)?;
                let wire: FlowWire = decode(params.payload.clone())?;
                assert_expected_project_root(workspace, wire.expected_project_root.as_deref())?;
                let located = read_state(workspace, work_item_id)?;
                assert_story_scope(&located.value, work_item_id, wire.story.as_deref())?;
                let input = flow_input(&located.value, work_item_id, self.event_store_id)?;
                let decision = if method == RpcMethod::FlowNext {
                    if let Some(target) = wire.target_phase.as_deref() {
                        let role = workspace.agent_role.ok_or_else(|| {
                            RuntimeError::new(
                                StableErrorCode::RoleOperationForbidden,
                                "a daemon-verified root session is required to request a transition",
                            )
                        })?;
                        if role != AgentRole::Root {
                            return Err(RuntimeError::new(
                                StableErrorCode::RoleOperationForbidden,
                                "only the root Agent may request a flow transition",
                            ));
                        }
                        let idempotency_key =
                            params.idempotency_key.as_deref().ok_or_else(|| {
                                schema_error("idempotencyKey is required for a transition request")
                            })?;
                        self.flow.request_transition(
                            &self.boot_id.to_string(),
                            &workspace.workspace_id,
                            params.session_id.as_deref(),
                            work_item_id,
                            idempotency_key,
                            input,
                            role,
                            parse_phase(target)?,
                        )?
                    } else {
                        self.flow
                            .project(&workspace.workspace_id, work_item_id, input)?
                    }
                } else {
                    if wire.target_phase.is_some() {
                        return Err(schema_error("flow.snapshot does not accept targetPhase"));
                    }
                    self.flow
                        .project(&workspace.workspace_id, work_item_id, input)?
                };
                Ok(FlowSupervisor::projection(&decision))
            }
            RpcMethod::GateEvaluate => self.gate_evaluate(workspace, params),
            RpcMethod::JobSubmit | RpcMethod::JobStatus | RpcMethod::JobCancel => {
                Err(RuntimeError::new(
                    StableErrorCode::OperationSchemaInvalid,
                    "job methods are owned by the daemon scheduler",
                ))
            }
            _ => Err(RuntimeError::new(
                StableErrorCode::OperationNotRegistered,
                "method is not owned by the business adapter",
            )),
        }
    }

    fn project_context(
        &self,
        workspace: &BusinessWorkspace,
        work_item_id: &str,
        session_id: &str,
        role: AgentRole,
    ) -> RuntimeResult<ContextProjectionInput> {
        let located = read_state(workspace, work_item_id)?;
        let source_revision = located
            .value
            .get("revision")
            .and_then(Value::as_u64)
            .ok_or_else(|| schema_error("authoritative state revision is missing"))?;
        let input_fingerprint = authoritative_input_fingerprint(&located.value)?.to_string();
        let view = work_item_view(&located.value, work_item_id);
        let hook_guard = view
            .get("hookGuard")
            .or_else(|| located.value.get("hookGuard"))
            .cloned()
            .filter(|guard| {
                guard.get("stateRevision").and_then(Value::as_u64) == Some(source_revision)
                    && guard.get("policyDigest").and_then(Value::as_str)
                        == Some(self.policy_digest.as_str())
                    && guard.get("inventoryGeneration").and_then(Value::as_u64)
                        == Some(workspace.inventory_generation)
                    && guard.get("inputFingerprint").and_then(Value::as_str)
                        == Some(input_fingerprint.as_str())
            });
        let flow = self.flow.project(
            &workspace.workspace_id,
            work_item_id,
            flow_input(&located.value, work_item_id, self.event_store_id)?,
        )?;
        let flow_projection = FlowSupervisor::projection(&flow);
        let next_action = flow_projection
            .get("nextAction")
            .cloned()
            .unwrap_or_else(|| json!({"kind":"await-agent-work"}));
        Ok(ContextProjectionInput {
            session_id: session_id.to_owned(),
            source_revision,
            projection: json!({
                "workspaceId": workspace.workspace_id,
                "workItemId": work_item_id,
                "role": role_name(role),
                "phase": flow_projection.get("phase"),
                "nextAction": next_action,
                "flow": flow_projection,
                "stateRevision": source_revision,
                "policyDigest": self.policy_digest,
                "inventoryGeneration": workspace.inventory_generation,
                "inputFingerprint": input_fingerprint,
                "hookGuard": hook_guard,
            }),
        })
    }

    fn execute_job(
        &self,
        workspace: &BusinessWorkspace,
        entrypoint: &str,
        arguments: &Value,
    ) -> RuntimeResult<Value> {
        jobs::execute(
            workspace,
            None,
            &self.database,
            self.persistence.as_ref(),
            None,
            &self.policy_digest,
            entrypoint,
            arguments,
        )
    }

    fn execute_bound_job(
        &self,
        workspace: &BusinessWorkspace,
        work_item_id: Option<&str>,
        entrypoint: &str,
        arguments: &Value,
    ) -> RuntimeResult<Value> {
        jobs::execute(
            workspace,
            work_item_id,
            &self.database,
            self.persistence.as_ref(),
            None,
            &self.policy_digest,
            entrypoint,
            arguments,
        )
    }

    fn execute_trusted_job(
        &self,
        workspace: &BusinessWorkspace,
        work_item_id: Option<&str>,
        identity: Option<&BoundJobIdentity>,
        entrypoint: &str,
        arguments: &Value,
    ) -> RuntimeResult<Value> {
        let result = jobs::execute(
            workspace,
            work_item_id,
            &self.database,
            self.persistence.as_ref(),
            identity,
            &self.policy_digest,
            entrypoint,
            arguments,
        )?;
        if entrypoint != "toolset.receipt.record"
            || result.get("outcome").and_then(Value::as_str) != Some("PASS")
        {
            return Ok(result);
        }
        self.commit_toolset_receipt(
            workspace,
            work_item_id.ok_or_else(|| schema_error("toolset receipt Work Item is required"))?,
            identity.ok_or_else(|| schema_error("toolset receipt identity is required"))?,
            arguments,
            result,
        )
    }

    fn validate_delegation_artifacts(
        &self,
        workspace: &BusinessWorkspace,
        delegation_id: &str,
        result: &Value,
    ) -> RuntimeResult<Value> {
        let deliverables = result
            .get("deliverables")
            .and_then(Value::as_array)
            .ok_or_else(|| child_result_error("child result deliverables must be an array"))?;
        if deliverables.len() > 256 {
            return Err(child_result_error(
                "child result exceeds the 256 deliverable validation bound",
            ));
        }
        let canonical_root = fs::canonicalize(&workspace.canonical_root).map_err(io_error)?;
        let mut validated = Vec::with_capacity(deliverables.len());
        let mut paths = std::collections::BTreeSet::new();
        for item in deliverables {
            let object = item
                .as_object()
                .ok_or_else(|| child_result_error("child deliverable must be an object"))?;
            let id = bounded_child_string(object, "id", 128)?;
            let kind = bounded_child_string(object, "kind", 128)?;
            let path = bounded_child_string(object, "path", 1_024)?;
            let expected_digest = lowercase_digest(object, "digest")?;
            let expected_bytes = object
                .get("byteLength")
                .and_then(Value::as_u64)
                .ok_or_else(|| child_result_error("child deliverable byteLength is required"))?;
            let relative = ProjectRelativePath::new(path.to_owned()).map_err(domain_error)?;
            if !paths.insert(relative.clone()) {
                return Err(child_result_error("child deliverable path is duplicated"));
            }
            let absolute =
                fs::canonicalize(canonical_root.join(relative.as_str())).map_err(io_error)?;
            if !absolute.starts_with(&canonical_root) || !absolute.is_file() {
                return Err(child_result_error(
                    "child deliverable is not a regular file inside the registered workspace",
                ));
            }
            let bytes = fs::read(&absolute).map_err(io_error)?;
            if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != expected_bytes
                || ArtifactDigest::digest(&bytes).to_string() != expected_digest
            {
                return Err(child_result_error(
                    "child deliverable digest or byteLength does not match the authoritative file",
                ));
            }
            validated.push(json!({
                "id":id,
                "kind":kind,
                "path":relative.as_str(),
                "digest":expected_digest,
                "byteLength":expected_bytes,
            }));
        }
        let result_bytes = serde_json::to_vec(result)
            .map_err(|_| child_result_error("child result could not be canonicalized"))?;
        Ok(json!({
            "schemaVersion":"delegation-artifact-validation/v1",
            "delegationId":delegation_id,
            "resultDigest":ResultDigest::digest(result_bytes).to_string(),
            "artifacts":validated,
        }))
    }

    fn cleanup_delegation_memory(
        &self,
        workspace: &BusinessWorkspace,
        delegation_id: &str,
        result: &Value,
        artifact_receipt: &Value,
    ) -> RuntimeResult<Value> {
        if artifact_receipt.get("delegationId").and_then(Value::as_str) != Some(delegation_id)
            || artifact_receipt
                .get("resultDigest")
                .and_then(Value::as_str)
                .is_none()
        {
            return Err(child_result_error(
                "artifact receipt is not bound to the delegation",
            ));
        }
        let snapshot = result
            .get("memorySnapshotDigest")
            .and_then(Value::as_str)
            .filter(|value| is_lowercase_digest(value))
            .ok_or_else(|| {
                child_result_error("memorySnapshotDigest must be a lowercase sha256 digest")
            })?;
        let existing = self
            .persistence
            .load_record("delegation-memory/v1", delegation_id)?
            .ok_or_else(|| {
                child_result_error("daemon-owned delegation memory namespace is missing")
            })?;
        if existing.get("workspaceId").and_then(Value::as_str)
            != Some(workspace.workspace_id.as_str())
        {
            return Err(child_result_error(
                "delegation memory namespace belongs to a different workspace",
            ));
        }
        if existing.get("status").and_then(Value::as_str) == Some("cleaned") {
            if existing.get("memorySnapshotDigest").and_then(Value::as_str) == Some(snapshot) {
                return existing
                    .get("cleanupReceipt")
                    .cloned()
                    .ok_or_else(|| child_result_error("cleaned namespace receipt is missing"));
            }
            return Err(child_result_error(
                "delegation memory namespace was already cleaned for another snapshot",
            ));
        }
        if existing.get("status").and_then(Value::as_str) != Some("active") {
            return Err(child_result_error(
                "delegation memory namespace is not active",
            ));
        }
        let cleanup_binding = json!({
            "delegationId":delegation_id,
            "workspaceId":workspace.workspace_id,
            "memorySnapshotDigest":snapshot,
            "artifactResultDigest":artifact_receipt.get("resultDigest"),
        });
        let cleanup_digest = ResultDigest::digest(
            serde_json::to_vec(&cleanup_binding)
                .map_err(|_| child_result_error("memory cleanup binding is invalid"))?,
        )
        .to_string();
        let cleaned_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| child_result_error("system clock is before the Unix epoch"))?
            .as_millis();
        let cleaned_at = u64::try_from(cleaned_at)
            .map_err(|_| child_result_error("system clock exceeds the receipt range"))?;
        let receipt = json!({
            "schemaVersion":"delegation-memory-cleanup/v1",
            "delegationId":delegation_id,
            "memorySnapshotDigest":snapshot,
            "cleanupDigest":cleanup_digest,
            "cleanedAtUnixMs":cleaned_at,
        });
        self.persistence.store_record(
            "delegation-memory/v1",
            delegation_id,
            &json!({
                "schemaVersion":"delegation-memory/v1",
                "workspaceId":workspace.workspace_id,
                "delegationId":delegation_id,
                "status":"cleaned",
                "memorySnapshotDigest":snapshot,
                "payloadPurged":true,
                "cleanupReceipt":receipt,
            }),
        )?;
        Ok(receipt)
    }
}

const fn role_name(role: AgentRole) -> &'static str {
    match role {
        AgentRole::Root => "root",
        AgentRole::Series => "series",
        AgentRole::Task => "task",
        AgentRole::Reviewer => "reviewer",
    }
}

fn bounded_child_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    maximum: usize,
) -> RuntimeResult<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= maximum)
        .ok_or_else(|| child_result_error(&format!("child deliverable {field} is invalid")))
}

fn lowercase_digest<'a>(object: &'a Map<String, Value>, field: &str) -> RuntimeResult<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_lowercase_digest(value))
        .ok_or_else(|| {
            child_result_error(&format!(
                "child deliverable {field} must be a lowercase sha256 digest"
            ))
        })
}

fn is_lowercase_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn child_result_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::ChildResultInvalid, message)
}

fn toolset_result_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    maximum: usize,
) -> RuntimeResult<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= maximum)
        .ok_or_else(|| schema_error(&format!("toolset receipt result {field} is invalid")))
}

fn toolset_result_digest<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> RuntimeResult<&'a str> {
    let value = toolset_result_string(object, field, 64)?;
    if is_lowercase_digest(value) {
        Ok(value)
    } else {
        Err(schema_error(&format!(
            "toolset receipt result {field} must be a lowercase sha256 digest"
        )))
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_toolset_manifest(
    workspace_root: &Path,
    work_item_id: &str,
    receipt_id: &str,
    toolset_job_id: &str,
    receipt_digest: &str,
    plan_digest: &str,
    methodology_digest: &str,
    policy_digest: &str,
    input_fingerprint: &str,
    source_revision: u64,
    inventory_generation: u64,
    recorder_session_id: &str,
    artifact_ref: &str,
    project_receipt_digest: &str,
    receipt: &Value,
) -> RuntimeResult<(ProjectRelativePath, Option<ArtifactDigest>, Vec<u8>)> {
    let manifest_ref = ProjectRelativePath::new(format!(
        ".auto-engineering/{work_item_id}/evidence/manifest.json"
    ))
    .map_err(domain_error)?;
    let manifest_absolute = workspace_root.join(manifest_ref.as_str());
    let (mut manifest, before_digest) = match fs::read(&manifest_absolute) {
        Ok(bytes) => {
            if bytes.is_empty() || bytes.len() > 1_048_576 {
                return Err(toolset_authority_conflict(
                    "evidence manifest exceeds the durable byte bound",
                ));
            }
            let before_digest = Some(ArtifactDigest::digest(&bytes));
            let manifest: Value = serde_json::from_slice(&bytes)
                .map_err(|_| toolset_authority_conflict("evidence manifest is malformed"))?;
            validate_toolset_manifest(&manifest, work_item_id)?;
            (manifest, before_digest)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (
            json!({"schemaVersion":1,"storyId":work_item_id,"entries":[]}),
            None,
        ),
        Err(error) => return Err(io_error(error)),
    };
    let entries = manifest
        .get_mut("entries")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| toolset_authority_conflict("evidence manifest entries are missing"))?;
    if entries
        .iter()
        .any(|entry| entry.get("evidenceId").and_then(Value::as_str) == Some(receipt_id))
    {
        return Err(toolset_authority_conflict(
            "immutable toolset receipt already exists in the evidence manifest",
        ));
    }
    let logical_key = format!("toolset/{work_item_id}");
    for previous in entries.iter_mut().filter_map(Value::as_object_mut) {
        if previous.get("status").and_then(Value::as_str) == Some("active")
            && previous.get("logicalKey").and_then(Value::as_str) == Some(logical_key.as_str())
        {
            previous.insert("status".to_owned(), Value::String("superseded".to_owned()));
            previous.insert(
                "supersededBy".to_owned(),
                Value::String(receipt_id.to_owned()),
            );
        }
    }
    entries.push(json!({
        "evidenceId": receipt_id,
        "kind": "toolset-receipt",
        "logicalKey": logical_key,
        "status": "active",
        "toolsetJobId": toolset_job_id,
        "workItemId": work_item_id,
        "inputFingerprint": input_fingerprint,
        "exitCode": receipt.get("exitCode"),
        "startedAtUnixMs": receipt.get("startedAtUnixMs"),
        "finishedAtUnixMs": receipt.get("finishedAtUnixMs"),
        "receiptDigest": receipt_digest,
        "planDigest": plan_digest,
        "policyDigest": policy_digest,
        "methodologyDigest": methodology_digest,
        "sourceRevision": source_revision,
        "inventoryGeneration": inventory_generation,
        "recorderSessionId": recorder_session_id,
        "reusable": true,
        "artifacts": [{
            "path": artifact_ref,
            "snapshotPath": artifact_ref,
            "sha256": format!("sha256:{project_receipt_digest}"),
        }],
    }));
    seal_toolset_manifest(&mut manifest)?;
    Ok((manifest_ref, before_digest, pretty_json_line(&manifest)?))
}

fn validate_toolset_manifest(manifest: &Value, work_item_id: &str) -> RuntimeResult<()> {
    let object = manifest
        .as_object()
        .ok_or_else(|| toolset_authority_conflict("evidence manifest root must be an object"))?;
    if object.get("schemaVersion").and_then(Value::as_u64) != Some(1)
        || object.get("storyId").and_then(Value::as_str) != Some(work_item_id)
        || !object.get("entries").is_some_and(Value::is_array)
    {
        return Err(toolset_authority_conflict(
            "evidence manifest schema or Work Item binding is invalid",
        ));
    }
    if let Some(expected) = object.get("contentHash").and_then(Value::as_str)
        && expected != toolset_manifest_content_hash(manifest)?
    {
        return Err(toolset_authority_conflict(
            "evidence manifest contentHash does not match its contents",
        ));
    }
    Ok(())
}

fn seal_toolset_manifest(manifest: &mut Value) -> RuntimeResult<()> {
    let content_hash = toolset_manifest_content_hash(manifest)?;
    manifest
        .as_object_mut()
        .ok_or_else(|| toolset_authority_conflict("evidence manifest root must be an object"))?
        .insert("contentHash".to_owned(), Value::String(content_hash));
    Ok(())
}

fn toolset_manifest_content_hash(manifest: &Value) -> RuntimeResult<String> {
    let mut payload = manifest.clone();
    payload
        .as_object_mut()
        .ok_or_else(|| toolset_authority_conflict("evidence manifest root must be an object"))?
        .retain(|key, _| key != "contentHash" && !key.starts_with('_'));
    let bytes = serde_json::to_vec(&payload)
        .map_err(|_| schema_error("evidence manifest could not be canonicalized"))?;
    Ok(format!("sha256:{}", ArtifactDigest::digest(bytes)))
}

fn pretty_json_line(value: &Value) -> RuntimeResult<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|_| schema_error("project authority could not be serialized"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn toolset_authority_conflict(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::ExternalStateConflict, message)
}

fn authorize_operation_paths(
    grant: &ScopedGrant,
    operation: OperationName,
    payload: &Value,
    state: &LocatedState,
    workspace: &BusinessWorkspace,
) -> RuntimeResult<()> {
    let mut required = Vec::new();
    match operation {
        OperationName::DocumentResolve | OperationName::DocumentSave => {
            let intent = payload
                .get("intent")
                .and_then(Value::as_str)
                .ok_or_else(|| schema_error("intent is required"))?;
            required.push(document_path(&state.value, intent)?);
            if operation == OperationName::DocumentSave {
                let content = payload
                    .get("contentFile")
                    .and_then(Value::as_str)
                    .ok_or_else(|| schema_error("contentFile is required"))?;
                required.push(scoped_relative_path(workspace, content)?);
            }
        }
        OperationName::EvidenceRecord => {
            let artifact = payload
                .get("artifactPath")
                .and_then(Value::as_str)
                .ok_or_else(|| schema_error("artifactPath is required"))?;
            required.push(scoped_relative_path(workspace, artifact)?);
        }
        OperationName::VerificationPlan => {
            required.extend(scoped_path_array(workspace, payload, "changedPaths")?);
        }
        OperationName::ReviewRecord if payload.get("reviewedPaths").is_some() => {
            required.extend(scoped_path_array(workspace, payload, "reviewedPaths")?);
        }
        _ => {}
    }
    if required.iter().all(|path| grant.permits_path(path)) {
        Ok(())
    } else {
        Err(RuntimeError::new(
            StableErrorCode::RoleOperationForbidden,
            "daemon-verified scoped grant forbids an operation path",
        ))
    }
}

fn scoped_path_array(
    workspace: &BusinessWorkspace,
    payload: &Value,
    field: &str,
) -> RuntimeResult<Vec<ProjectRelativePath>> {
    payload
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| schema_error("scoped path list must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| schema_error("scoped path list entries must be strings"))
                .and_then(|path| scoped_relative_path(workspace, path))
        })
        .collect()
}

fn scoped_relative_path(
    workspace: &BusinessWorkspace,
    value: &str,
) -> RuntimeResult<ProjectRelativePath> {
    let path = Path::new(value);
    if !path.is_absolute() {
        return ProjectRelativePath::new(value.to_owned()).map_err(domain_error);
    }
    let root = fs::canonicalize(&workspace.canonical_root)
        .map_err(|_| schema_error("registered workspace root cannot be canonicalized"))?;
    let path = fs::canonicalize(path)
        .map_err(|_| schema_error("operation path cannot be canonicalized"))?;
    let relative = path.strip_prefix(root).map_err(|_| {
        RuntimeError::new(
            StableErrorCode::RoleOperationForbidden,
            "operation path is outside the registered workspace",
        )
    })?;
    ProjectRelativePath::new(relative.to_string_lossy().replace('\\', "/")).map_err(domain_error)
}

fn semantic_operation(role: AgentRole, operation: OperationName) -> RoleOperation {
    match operation {
        OperationName::StateTransition | OperationName::WorkItemComplete => {
            RoleOperation::RequestGlobalTransition
        }
        OperationName::ExecutionPlanApprove => RoleOperation::ApproveExecutionPlan,
        OperationName::ExecutionPlanSet => RoleOperation::SelectRoute,
        OperationName::DocumentSave => RoleOperation::ModifyAssignedPaths,
        OperationName::EvidenceRecord | OperationName::EvidenceFinalize => {
            RoleOperation::SubmitEvidence
        }
        OperationName::VerificationPlan => RoleOperation::RunAssignedTests,
        OperationName::ReviewRecord => RoleOperation::SubmitReviewFindings,
        OperationName::LeaseBreak => RoleOperation::BreakLease,
        OperationName::LeaseAcquire | OperationName::LeaseRenew | OperationName::LeaseRelease => {
            RoleOperation::ManageOwnLease
        }
        _ if matches!(role, AgentRole::Root | AgentRole::Series) => {
            RoleOperation::ReadBoundedProjection
        }
        _ => RoleOperation::ReadAuthorizedArtifacts,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExecuteWire {
    operation: String,
    #[serde(default)]
    dry_run: bool,
    #[serde(default = "empty_object")]
    payload: Value,
    #[serde(default)]
    expected_project_root: Option<String>,
    #[serde(default)]
    expected_project_key: Option<String>,
    #[serde(default)]
    story: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DescribeWire {
    #[serde(default)]
    operation: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FlowWire {
    #[serde(default)]
    target_phase: Option<String>,
    #[serde(default)]
    story: Option<String>,
    #[serde(default)]
    expected_project_root: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GateEvaluateWire {
    #[serde(default)]
    gate_id: Option<String>,
    #[serde(default)]
    gate_ids: Option<Vec<String>>,
    #[serde(default)]
    expected_project_root: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ToolsetReceiptCommitWire {
    plan: Value,
    receipt: Value,
    source_revision: u64,
    policy_digest: String,
    methodology_digest: String,
    inventory_generation: u64,
    lease_id: String,
    fencing_token: u64,
}

fn empty_object() -> Value {
    json!({})
}

fn decode<T: serde::de::DeserializeOwned>(value: Value) -> RuntimeResult<T> {
    serde_json::from_value(value).map_err(|_| {
        RuntimeError::new(
            StableErrorCode::OperationSchemaInvalid,
            "business payload violates its strict schema",
        )
    })
}

fn require_workspace(value: Option<&BusinessWorkspace>) -> RuntimeResult<&BusinessWorkspace> {
    value.ok_or_else(|| {
        RuntimeError::new(
            StableErrorCode::ProjectMismatch,
            "registered workspace context is required",
        )
    })
}

fn require_work_item(params: &RequestParams<Value>) -> RuntimeResult<&str> {
    params.work_item_id.as_deref().ok_or_else(|| {
        RuntimeError::new(
            StableErrorCode::OperationSchemaInvalid,
            "workItemId is required",
        )
    })
}

fn assert_expected_project_root(
    workspace: &BusinessWorkspace,
    expected: Option<&str>,
) -> RuntimeResult<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let expected = fs::canonicalize(expected)
        .map_err(|_| schema_error("expectedProjectRoot cannot be canonicalized"))?;
    let actual = fs::canonicalize(&workspace.canonical_root)
        .map_err(|_| schema_error("registered workspace root cannot be canonicalized"))?;
    if expected == actual {
        Ok(())
    } else {
        Err(RuntimeError::new(
            StableErrorCode::ProjectMismatch,
            "expectedProjectRoot does not match the registered workspace",
        ))
    }
}

fn assert_expected_project_key(
    workspace: &BusinessWorkspace,
    expected: Option<&str>,
) -> RuntimeResult<()> {
    match expected {
        Some(expected) if expected != workspace.project_key => Err(RuntimeError::new(
            StableErrorCode::ProjectMismatch,
            "expectedProjectKey does not match the registered workspace",
        )),
        _ => Ok(()),
    }
}

fn assert_story_scope(
    state: &Value,
    work_item_id: &str,
    expected: Option<&str>,
) -> RuntimeResult<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let matches = expected == work_item_id
        || state.get("activeStory").and_then(Value::as_str) == Some(expected)
        || state.get("currentStory").and_then(Value::as_str) == Some(expected);
    if matches {
        Ok(())
    } else {
        Err(RuntimeError::new(
            StableErrorCode::ProjectMismatch,
            "story assertion does not match the authoritative Work Item",
        ))
    }
}

fn batch_gate_idempotency_key(base: &str, gate_id: &str) -> String {
    hex::encode(Sha256::digest(format!("{base}\0{gate_id}").as_bytes()))
}

fn valid_gate_batch(gate_ids: &[String]) -> bool {
    !gate_ids.is_empty()
        && gate_ids.len() <= GateRegistry::all().len()
        && gate_ids.iter().all(|gate_id| !gate_id.is_empty())
        && gate_ids.iter().collect::<BTreeSet<_>>().len() == gate_ids.len()
}

fn system_time_unix_ms() -> RuntimeResult<u64> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "system clock is before the Unix epoch",
            )
        })?
        .as_millis();
    u64::try_from(millis).map_err(|_| {
        RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "system clock exceeds the lifecycle timestamp range",
        )
    })
}

fn operation_request(
    operation: OperationName,
    params: &RequestParams<Value>,
    workspace: &BusinessWorkspace,
    payload: Value,
    dry_run: bool,
) -> RuntimeResult<OperationRequest> {
    Ok(OperationRequest {
        operation,
        workspace_id: Some(parse(&workspace.workspace_id, "workspaceId")?),
        project_key: Some(ProjectKey::new(workspace.project_key.clone()).map_err(domain_error)?),
        work_item_id: params
            .work_item_id
            .as_deref()
            .map(|value| WorkItemId::new(value.to_owned()).map_err(domain_error))
            .transpose()?,
        session_id: params
            .session_id
            .as_deref()
            .map(|value| parse(value, "sessionId"))
            .transpose()?,
        lease_id: params
            .lease_id
            .as_deref()
            .map(|value| parse(value, "leaseId"))
            .transpose()?,
        fencing_token: params.fencing_token.map(FencingToken::new),
        expected_revision: params.expected_revision.map(StateRevision::new),
        idempotency_key: params.idempotency_key.clone().map(String::into_boxed_str),
        confirmation: params
            .confirmation
            .as_ref()
            .map(|value| {
                Confirmation::new(
                    value.confirmation_id.clone(),
                    value.approved_by.clone(),
                    value.approved_at.clone(),
                )
                .map_err(|_| schema_error("confirmation is invalid"))
            })
            .transpose()?,
        dry_run,
        payload,
    })
}

struct ProjectBackend<'a> {
    adapter: &'a NativeBusinessAdapter,
    workspace: &'a BusinessWorkspace,
    state: LocatedState,
    agent_id: Option<Box<str>>,
    session_id: Option<SessionId>,
    deadline_ms: u64,
}

struct PreparedSemanticMutation {
    data: Value,
    targets: Vec<evidence::SemanticTarget>,
    /// `evidenceAuthority` state projection (ledger/manifest locator + digest)
    /// produced by evidence mutations.
    evidence_authority: Option<Value>,
    review: Option<Value>,
    review_session: Option<Value>,
    review_binding: Option<PreparedReviewBinding>,
    /// Typed Review Batch v2 tuple retained so the post-commit path can write
    /// the durable SQLite projection without re-deriving review authority.
    review_record: Option<review_authority::PreparedReviewRecord>,
    lifecycle: Option<lifecycle_authority::PermittedLifecycleMutation>,
}

struct PreparedReviewBinding {
    input_fingerprint: String,
    ruleset_fingerprint: String,
    policy_digest: String,
    inventory_generation: u64,
}

/// Bounded writer-lease TTL for the internal first-generation seed commit.
const EXECUTION_SEED_LEASE_TTL_SECONDS: u64 = 300;

/// Typed inputs for one first-generation execution seed commit.
struct ExecutionSeedCommit<'a> {
    work_item_id: &'a WorkItemId,
    operation: &'a OperationId,
    outcome: &'a CapsuleBuildOutcome,
    plan: &'a execution_authority::ApprovedPlanAuthority,
    state: &'a Value,
    authority: AuthoritySnapshot,
    before_bytes: &'a [u8],
    lease: &'a LeaseRecord,
}

struct SemanticAuthorityContext<'a> {
    agent_id: Option<&'a str>,
    actor_role: Option<AgentRole>,
    session_id: Option<SessionId>,
    persistence: &'a dyn PersistencePort,
    boot_id: &'a BootId,
    policy_digest: &'a str,
    inventory_generation: u64,
}

impl PreparedSemanticMutation {
    fn plain(data: Value) -> Self {
        Self {
            data,
            targets: Vec::new(),
            evidence_authority: None,
            review: None,
            review_session: None,
            review_binding: None,
            review_record: None,
            lifecycle: None,
        }
    }

    fn with_targets(data: Value, targets: Vec<evidence::SemanticTarget>, authority: Value) -> Self {
        Self {
            targets,
            evidence_authority: Some(authority),
            ..Self::plain(data)
        }
    }

    fn lifecycle(permitted: lifecycle_authority::PermittedLifecycleMutation) -> Self {
        let data = permitted.data().clone();
        Self {
            data,
            lifecycle: Some(permitted),
            ..Self::plain(Value::Null)
        }
    }
}

impl<'a> ProjectBackend<'a> {
    fn open(
        adapter: &'a NativeBusinessAdapter,
        workspace: &'a BusinessWorkspace,
        params: &RequestParams<Value>,
    ) -> RuntimeResult<Self> {
        Ok(Self {
            adapter,
            workspace,
            state: read_state(workspace, require_work_item(params)?)?,
            agent_id: params.agent_id.clone().map(String::into_boxed_str),
            session_id: params
                .session_id
                .as_deref()
                .map(|value| parse(value, "sessionId"))
                .transpose()?,
            deadline_ms: params.deadline_ms,
        })
    }

    fn store(
        &self,
    ) -> RuntimeResult<
        ProjectMutationStore<
            StdDurableFileSystem,
            StdCrossProcessLock,
            SqliteRuntimeRepository,
            ProcessAbortCommitFault,
        >,
    > {
        self.store_for(None)
    }

    /// Opens the authoritative project store, recovering any interrupted
    /// mutation first. `operation` scopes the debug commit-abort failpoint so a
    /// crash test can target one authoritative mutation.
    fn store_for(
        &self,
        operation: Option<&str>,
    ) -> RuntimeResult<
        ProjectMutationStore<
            StdDurableFileSystem,
            StdCrossProcessLock,
            SqliteRuntimeRepository,
            ProcessAbortCommitFault,
        >,
    > {
        let paths =
            ProjectStorePaths::new(&self.workspace.canonical_root, self.state.relative.clone())
                .map_err(store_error)?;
        let repository = SqliteRuntimeRepository::open(
            &self.adapter.database,
            self.adapter.event_store_id,
            &UtcTimestamp::now(),
        )
        .map_err(store_error)?;
        let store = ProjectMutationStore::with_faults(
            paths,
            StdDurableFileSystem,
            StdCrossProcessLock,
            repository,
            operation.map_or(ProcessAbortCommitFault::Disarmed, |operation| {
                ProcessAbortCommitFault::Operation(operation.to_owned())
            }),
        );
        store.recover(UtcTimestamp::now()).map_err(store_error)?;
        Ok(store)
    }
}

impl OperationBackend for ProjectBackend<'_> {
    type Error = RuntimeError;

    fn read(&self, request: &ValidatedOperationRequest) -> Result<OperationResponse, Self::Error> {
        let state = &self.state.value;
        let data = match request.operation() {
            OperationName::WorkItemGet => state.clone(),
            OperationName::StateNextActions => self.state_next_actions(request)?,
            OperationName::GateCheck => self.gate_check(request)?,
            OperationName::DocumentResolve => resolve_document(self.workspace, state, request)?,
            OperationName::LeaseStatus => lease_status(&self.store()?)?,
            OperationName::ExecutionResume => self.execution_resume(request)?,
            _ => {
                return Err(schema_error(
                    "read operation has no authoritative projection",
                ));
            }
        };
        Ok(OperationResponse {
            changed: false,
            revision_before: None,
            revision_after: None,
            receipt_digest: None,
            data,
        })
    }

    fn mutate(
        &self,
        request: &ValidatedOperationRequest,
    ) -> Result<OperationResponse, Self::Error> {
        if request.request().dry_run {
            return Err(schema_error(
                "dry-run request reached the committing backend path",
            ));
        }
        match request.operation() {
            OperationName::LeaseAcquire
            | OperationName::LeaseRenew
            | OperationName::LeaseRelease
            | OperationName::LeaseBreak => mutate_lease(
                &self.store()?,
                request,
                self.session_id,
                self.adapter.boot_id,
            ),
            _ => self.mutate_state(request),
        }
    }

    fn dry_run(
        &self,
        request: &ValidatedOperationRequest,
    ) -> Result<OperationResponse, Self::Error> {
        if !request.request().dry_run {
            return Err(schema_error(
                "non-dry-run request reached the validation-only backend path",
            ));
        }
        match request.operation() {
            OperationName::LeaseAcquire
            | OperationName::LeaseRenew
            | OperationName::LeaseRelease
            | OperationName::LeaseBreak => mutate_lease(
                &self.store()?,
                request,
                self.session_id,
                self.adapter.boot_id,
            ),
            _ => self.mutate_state(request),
        }
    }
}

impl ProjectBackend<'_> {
    fn state_next_actions(&self, request: &ValidatedOperationRequest) -> RuntimeResult<Value> {
        let work_item_id = request
            .request()
            .work_item_id
            .as_ref()
            .ok_or_else(|| schema_error("workItemId is required"))?;
        let input = flow_input(
            &self.state.value,
            work_item_id.as_str(),
            self.adapter.event_store_id,
        )?;
        let decision = self.adapter.flow.project(
            &self.workspace.workspace_id,
            work_item_id.as_str(),
            input,
        )?;
        Ok(FlowSupervisor::projection(&decision))
    }

    fn gate_check(&self, request: &ValidatedOperationRequest) -> RuntimeResult<Value> {
        let work_item_id = request
            .request()
            .work_item_id
            .as_ref()
            .ok_or_else(|| schema_error("workItemId is required"))?;
        let requested = request
            .request()
            .payload
            .get("gateIds")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| schema_error("gateIds must contain strings"))
                    })
                    .collect::<RuntimeResult<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_else(|| {
                GateRegistry::all()
                    .iter()
                    .map(|gate| gate.id.to_owned())
                    .collect()
            });
        if requested.len() > ae_sdd_gates::GATE_COUNT {
            return Err(schema_error("gateIds exceeds the registered Gate count"));
        }
        let gates = AuthoritativeGateRuntime::with_review_authority(
            self.workspace,
            work_item_id.as_str(),
            &self.adapter.policy_digest,
            request.request().fencing_token.map(FencingToken::get),
            self.adapter.review_gate_authority(self.workspace),
        )?;
        let count = u64::try_from(requested.len()).unwrap_or(u64::MAX);
        let budget = self
            .deadline_ms
            .checked_div(count.max(1))
            .unwrap_or(1)
            .max(1);
        requested
            .iter()
            .map(|gate_id| {
                gates
                    .evaluate(gate_id, Duration::from_millis(budget))
                    .map(|result| gate_result_json(&result))
            })
            .collect::<RuntimeResult<Vec<_>>>()
            .map(Value::Array)
    }

    /// Authoritative `execution.resume`: resolves the committed capsule or
    /// seeds a first-generation one from a single authority snapshot and a
    /// single required-context bundle load.
    fn execution_resume(&self, request: &ValidatedOperationRequest) -> RuntimeResult<Value> {
        let work_item_id = request
            .request()
            .work_item_id
            .as_ref()
            .ok_or_else(|| schema_error("workItemId is required"))?;
        let resume =
            execution_authority::decode_execution_resume_payload(&request.request().payload)?;
        // The backend opened with exactly one state snapshot; the approved
        // plan and the four required contexts resolve from that snapshot plus
        // one bundle load, and every later check reuses these in-call values,
        // so the authority refresh count stays at one.
        let plan = execution_authority::approved_plan_authority(&self.state.value)?;
        let bundle = execution_authority::load_required_context_bundle(
            Path::new(&self.workspace.canonical_root),
            &self.state.value,
            work_item_id.as_str(),
            plan.plan(),
        )?;
        if self.state.value.get("executionRuntime").is_some() {
            let committed = execution_authority::verify_committed_capsule(
                Path::new(&self.workspace.canonical_root),
                &self.state.value,
                &plan,
                &bundle,
                &self.adapter.policy_digest,
            )?;
            return self.execution_resume_response(
                work_item_id.as_str(),
                &self.state.value,
                committed.capsule(),
                committed.capsule_digest(),
                &resume,
            );
        }
        let source_revision = StateRevision::new(
            self.state
                .value
                .get("revision")
                .and_then(Value::as_u64)
                .ok_or_else(|| schema_error("authoritative state revision is missing"))?,
        );
        let outcome = execution_authority::build_capsule_from_authority(
            &self.state.value,
            work_item_id.as_str(),
            source_revision,
            &plan,
            &bundle,
            &self.adapter.policy_digest,
            self.workspace.inventory_generation,
        )?;
        let after =
            self.seed_execution_capsule(work_item_id, request.operation_id(), &outcome, &plan)?;
        self.execution_resume_response(
            work_item_id.as_str(),
            &after,
            outcome.capsule(),
            outcome.capsule_digest(),
            &resume,
        )
    }

    /// Atomically commits the first-generation queue, ledger seed, capsule
    /// and state locator/digest section through the project mutation store.
    /// The registry keeps `execution.resume` lease-free for callers, so the
    /// seed write runs under a short-lived internal writer lease that is
    /// released before the operation returns; a conflicting active writer
    /// fails closed with `EXECUTION_RESOURCE_BUSY`.
    fn seed_execution_capsule(
        &self,
        work_item_id: &WorkItemId,
        operation: &OperationId,
        outcome: &CapsuleBuildOutcome,
        plan: &execution_authority::ApprovedPlanAuthority,
    ) -> RuntimeResult<Value> {
        let store = self.store_for(Some(operation.as_str()))?;
        let before_bytes = fs::read(&self.state.absolute).map_err(io_error)?;
        let authority = StateAuthority::inspect(&before_bytes).map_err(store_error)?;
        let state = authoritative_state_snapshot(&self.state.value, &before_bytes)?;
        let now = UtcTimestamp::now();
        let owner = LeaseOwner::new(format!(
            "execution.resume:{}",
            self.session_id
                .map_or_else(|| "daemon".to_owned(), |value| value.to_string())
        ))
        .map_err(store_error)?;
        let lease = store
            .acquire_lease(
                LeaseId::from_uuid(Uuid::new_v4()),
                owner,
                now.clone(),
                add_seconds_from(&now, EXECUTION_SEED_LEASE_TTL_SECONDS)?,
            )
            .map_err(|error| match error {
                StoreError::LeaseConflict => RuntimeError::new(
                    StableErrorCode::ExecutionResourceBusy,
                    "an active writer lease blocks execution capsule seeding",
                ),
                other => store_error(other),
            })?;
        let committed = match self.commit_execution_seed(
            &store,
            ExecutionSeedCommit {
                work_item_id,
                operation,
                outcome,
                plan,
                state: &state,
                authority,
                before_bytes: &before_bytes,
                lease: &lease,
            },
        ) {
            Ok(after) => after,
            Err(commit_error) => {
                // Best-effort release so a failed seed does not pin the
                // writer lease; the commit failure stays the primary error.
                return match self.release_execution_seed_lease(&store, &lease, work_item_id) {
                    Ok(()) => Err(commit_error),
                    Err(release_error) => Err(commit_error.with_remediation(format!(
                        "internal seed lease release also failed: {}",
                        release_error.message()
                    ))),
                };
            }
        };
        self.release_execution_seed_lease(&store, &lease, work_item_id)?;
        Ok(committed)
    }

    /// Commits the prepared seed mutation: state locator/digest section plus
    /// the queue, capsule and ledger artifacts in one journaled atomic write.
    /// The idempotency key binds the capsule digest, so a retried first
    /// generation replays without a second side effect.
    fn commit_execution_seed<C: CommitFaultPort>(
        &self,
        store: &ProjectMutationStore<
            StdDurableFileSystem,
            StdCrossProcessLock,
            SqliteRuntimeRepository,
            C,
        >,
        seed: ExecutionSeedCommit<'_>,
    ) -> RuntimeResult<Value> {
        let locators =
            execution_authority::execution_artifact_locators(seed.work_item_id.as_str())?;
        let ledger_bytes =
            execution_authority::ledger_seed_bytes(seed.outcome, seed.plan.plan_digest())?;
        let ledger_digest = ArtifactDigest::digest(&ledger_bytes);
        let revision_after = seed
            .authority
            .revision()
            .checked_next()
            .map_err(|_| schema_error("state revision overflow"))?;
        let idempotency_key = format!("execution.resume:{}", seed.outcome.capsule_digest());
        let mut after = seed.state.clone();
        let object = after
            .as_object_mut()
            .ok_or_else(|| schema_error("authoritative state must be an object"))?;
        object.insert("revision".to_owned(), Value::from(revision_after.get()));
        object.insert(
            "lastFencingToken".to_owned(),
            Value::from(seed.lease.fencing_token().get()),
        );
        object.insert(
            "executionRuntime".to_owned(),
            execution_authority::execution_runtime_state_section(
                seed.outcome,
                &locators,
                ledger_digest,
            ),
        );
        object.insert(
            "lastMutation".to_owned(),
            json!({
                "operation": OperationName::ExecutionResume.as_str(),
                "idempotencyKey": idempotency_key.as_str(),
                "revisionBefore": seed.authority.revision().get(),
                "revisionAfter": revision_after.get(),
            }),
        );
        let after_bytes = serde_json::to_vec_pretty(&after)
            .map_err(|_| schema_error("state could not be serialized"))?;
        let result_data = json!({
            "capsuleDigest": format!("sha256:{}", seed.outcome.capsule_digest()),
            "queueDigest": format!("sha256:{}", seed.outcome.queue_digest()),
            "revisionAfter": revision_after.get(),
        });
        let result_bytes = serde_json::to_vec(&result_data)
            .map_err(|_| schema_error("operation result could not be serialized"))?;
        let payload_bytes = serde_json::to_vec(&json!({
            "operation": OperationName::ExecutionResume.as_str(),
            "approvedPlanDigest": seed.plan.plan_digest().to_string(),
            "capsuleDigest": seed.outcome.capsule_digest().to_string(),
            "queueDigest": seed.outcome.queue_digest().to_string(),
        }))
        .map_err(|_| schema_error("execution seed payload could not be canonicalized"))?;
        let event_bytes = serde_json::to_vec(&json!({
            "operation": OperationName::ExecutionResume.as_str(),
            "data": result_data,
        }))
        .map_err(|_| schema_error("execution seed event could not be serialized"))?;
        let mutation = MutationRequest {
            mutation_id: RequestId::from_uuid(Uuid::new_v4()),
            workspace_id: parse(&self.workspace.workspace_id, "workspaceId")?,
            work_item_id: seed.work_item_id.clone(),
            operation: seed.operation.clone(),
            idempotency_key: IdempotencyKey::new(idempotency_key).map_err(store_error)?,
            canonical_payload_digest: InputFingerprint::digest(payload_bytes),
            expected_authority: seed.authority,
            lease_proof: LeaseProof::from(seed.lease),
            targets: vec![
                MutationTarget::new(
                    self.state.relative.clone(),
                    Some(ArtifactDigest::digest(seed.before_bytes)),
                    after_bytes,
                )
                .map_err(store_error)?,
                MutationTarget::new(
                    locators.queue().clone(),
                    None,
                    seed.outcome.queue_bytes().to_vec(),
                )
                .map_err(store_error)?,
                MutationTarget::new(
                    locators.capsule().clone(),
                    None,
                    seed.outcome.capsule_bytes().to_vec(),
                )
                .map_err(store_error)?,
                MutationTarget::new(locators.ledger().clone(), None, ledger_bytes)
                    .map_err(store_error)?,
            ],
            event: JournalEvent {
                boot_id: self.adapter.boot_id,
                session_id: self.session_id,
                event_type: OperationName::ExecutionResume
                    .as_str()
                    .to_owned()
                    .into_boxed_str(),
                schema_version: 1,
                payload: RuntimeEventPayload::InlineJson(event_bytes),
            },
            result_digest: ResultDigest::digest(&result_bytes),
            prepared_at: UtcTimestamp::now(),
            committed_at: UtcTimestamp::now(),
        };
        store.commit(mutation).map_err(store_error)?;
        Ok(after)
    }

    /// Releases the internal seed lease through the journaled lease-control
    /// path so later writers are never blocked by a finished resume.
    fn release_execution_seed_lease<C: CommitFaultPort>(
        &self,
        store: &ProjectMutationStore<
            StdDurableFileSystem,
            StdCrossProcessLock,
            SqliteRuntimeRepository,
            C,
        >,
        lease: &LeaseRecord,
        work_item_id: &WorkItemId,
    ) -> RuntimeResult<()> {
        let now = UtcTimestamp::now();
        let payload_bytes = serde_json::to_vec(&json!({
            "leaseId": lease.lease_id().to_string(),
            "fencingToken": lease.fencing_token().get(),
        }))
        .map_err(|_| schema_error("seed lease release could not be canonicalized"))?;
        let control = LeaseControlRequest {
            mutation_id: RequestId::from_uuid(Uuid::new_v4()),
            workspace_id: parse(&self.workspace.workspace_id, "workspaceId")?,
            work_item_id: work_item_id.clone(),
            operation: parse(OperationName::LeaseRelease.as_str(), "operation")?,
            idempotency_key: IdempotencyKey::new(format!(
                "execution.resume.release:{}",
                lease.lease_id()
            ))
            .map_err(store_error)?,
            canonical_payload_digest: InputFingerprint::digest(payload_bytes),
            action: LeaseControlAction::Release {
                proof: LeaseProof::from(lease),
                now: now.clone(),
            },
            boot_id: self.adapter.boot_id,
            session_id: self.session_id,
            committed_at: now,
        };
        store.commit_lease_control(control).map_err(store_error)?;
        Ok(())
    }

    /// Assembles the `execution.resume` projection: `no-change` when the
    /// caller already knows the current capsule digest and context revision,
    /// otherwise the full capsule with its FlowRuntime-owned next action.
    fn execution_resume_response(
        &self,
        work_item_id: &str,
        state: &Value,
        capsule: &ExecutionCapsuleV1,
        capsule_digest: ArtifactDigest,
        request: &execution_authority::ExecutionResumeRequest,
    ) -> RuntimeResult<Value> {
        let context_revision = capsule.source_revision().get();
        let no_change = request.known_capsule_digest() == Some(capsule_digest)
            && request
                .known_context_revision()
                .is_none_or(|revision| revision == context_revision);
        let input = flow_input(state, work_item_id, self.adapter.event_store_id)?;
        let decision =
            self.adapter
                .flow
                .project(&self.workspace.workspace_id, work_item_id, input)?;
        let projection = FlowSupervisor::projection(&decision);
        let next_action = projection
            .get("nextAction")
            .cloned()
            .unwrap_or_else(|| json!({"kind":"await-agent-work"}));
        let capsule_value = if no_change {
            Value::Null
        } else {
            serde_json::to_value(capsule)
                .map_err(|_| schema_error("execution capsule could not be serialized"))?
        };
        Ok(json!({
            "projectionKind": if no_change { "no-change" } else { "full" },
            "contextRevision": context_revision,
            "capsuleDigest": format!("sha256:{capsule_digest}"),
            "capsule": capsule_value,
            "nextAction": next_action,
            "authorityRefreshCount": 1,
        }))
    }

    fn prepare_transition_commit(
        &self,
        state: &Value,
        request: &ValidatedOperationRequest,
        target: Option<ProcessPhase>,
    ) -> RuntimeResult<Option<(FlowInput, ProcessPhase)>> {
        let Some(target) = target else {
            return Ok(None);
        };
        let work_item_id = request
            .request()
            .work_item_id
            .as_ref()
            .ok_or_else(|| schema_error("workItemId is required"))?;
        let input = flow_input(state, work_item_id.as_str(), self.adapter.event_store_id)?;
        let decision = self.adapter.flow.project(
            &self.workspace.workspace_id,
            work_item_id.as_str(),
            input,
        )?;
        if decision.pending_transition() != Some(target)
            || !matches!(decision.next_action(), NextAction::ApplyTransition { target: ready } if *ready == target)
        {
            return Err(RuntimeError::new(
                StableErrorCode::GateBlocked,
                "transition has no matching root intent with all required Gates recorded as PASS",
            ));
        }

        let fencing = request.request().fencing_token.ok_or_else(|| {
            RuntimeError::new(StableErrorCode::LeaseRequired, "fencingToken is required")
        })?;
        let gates = AuthoritativeGateRuntime::with_review_authority(
            self.workspace,
            work_item_id.as_str(),
            &self.adapter.policy_digest,
            Some(fencing.get()),
            self.adapter.review_gate_authority(self.workspace),
        )?;
        let count = u64::try_from(decision.required_gates().len()).unwrap_or(u64::MAX);
        let per_gate_ms = self
            .deadline_ms
            .checked_div(count.max(1))
            .unwrap_or(1)
            .max(1);
        for required in decision.required_gates() {
            let evaluated =
                gates.evaluate(required.as_str(), Duration::from_millis(per_gate_ms))?;
            if !matches!(evaluated.outcome(), GateOutcome::Pass) {
                return Err(RuntimeError::new(
                    StableErrorCode::GateBlocked,
                    format!(
                        "required Gate {} is not a fresh PASS at transition commit",
                        required.as_str()
                    ),
                ));
            }
        }
        Ok(Some((input, target)))
    }

    /// Rebuilds this Work Item's Review projections from committed events.
    ///
    /// A seedless historical event is tolerated only when the current project
    /// state still joins that same review/batch/attempt, which keeps pre-seed
    /// deployments recoverable without fabricating authority.
    fn repair_review_projections(&self, work_item_id: &str) -> RuntimeResult<()> {
        let repaired = repair_review_projections_from_events(
            &self.adapter.database,
            self.adapter.persistence.as_ref(),
            &self.workspace.workspace_id,
            work_item_id,
        )?;
        if repaired > 0 {
            return Ok(());
        }
        let latest = self.latest_review_event_sequence(work_item_id)?;
        if latest == 0 {
            return Ok(());
        }
        let Some(write) = review_authority::review_projection_write_from_state(
            &self.state.value,
            &self.workspace.workspace_id,
            work_item_id,
            latest,
        )?
        else {
            return Ok(());
        };
        rebuild_review_authority_projections(&self.adapter.database, &[write])
    }

    /// Returns the newest committed `review.record` event sequence for the
    /// bounded state fallback, or `0` when no such event exists.
    fn latest_review_event_sequence(&self, work_item_id: &str) -> RuntimeResult<u64> {
        let mut cursor = 0_u64;
        let mut latest = 0_u64;
        loop {
            let page = self
                .adapter
                .persistence
                .events_after(cursor, REVIEW_EVENT_PAGE)?;
            if page.is_empty() {
                break;
            }
            for event in &page {
                cursor = cursor.max(event.event_seq);
                if event.kind == OperationName::ReviewRecord.as_str()
                    && event.workspace_id.as_deref() == Some(self.workspace.workspace_id.as_str())
                    && event.work_item_id.as_deref() == Some(work_item_id)
                {
                    latest = latest.max(event.event_seq);
                }
            }
            if page.len() < REVIEW_EVENT_PAGE {
                break;
            }
        }
        Ok(latest)
    }

    fn mutate_state(
        &self,
        request: &ValidatedOperationRequest,
    ) -> RuntimeResult<OperationResponse> {
        let idempotency_key = request
            .request()
            .idempotency_key
            .as_deref()
            .ok_or_else(|| schema_error("idempotencyKey is required"))?;
        let idempotency = IdempotencyKey::new(idempotency_key.to_owned()).map_err(store_error)?;
        let workspace_id = parse(&self.workspace.workspace_id, "workspaceId")?;
        let payload_digest = InputFingerprint::from_array(*request.payload_digest());
        let work_item_id = request
            .request()
            .work_item_id
            .as_ref()
            .ok_or_else(|| schema_error("workItemId is required"))?;
        let store = self.store_for(Some(request.operation().as_str()))?;
        if !request.request().dry_run
            && let Some(committed) = store
                .replay_committed(
                    workspace_id,
                    work_item_id,
                    request.operation_id(),
                    &idempotency,
                    payload_digest,
                )
                .map_err(store_error)?
        {
            let data = committed_result_data(&committed)?;
            // Same-key replay must repair a missing or partially written
            // projection before reporting `changed=false`.
            if request.operation() == OperationName::ReviewRecord {
                self.repair_review_projections(work_item_id.as_str())?;
            }
            return Ok(OperationResponse {
                changed: false,
                revision_before: Some(committed.receipt.revision_before),
                revision_after: Some(committed.receipt.revision_after),
                receipt_digest: Some(committed.receipt.result_digest.into_array()),
                data,
            });
        }
        // A new Review attempt must not silently bypass an earlier projection
        // failure; repair committed history first.
        if !request.request().dry_run && request.operation() == OperationName::ReviewRecord {
            self.repair_review_projections(work_item_id.as_str())?;
        }
        let before_bytes = fs::read(&self.state.absolute).map_err(io_error)?;
        let authority = StateAuthority::inspect(&before_bytes).map_err(store_error)?;
        let state = authoritative_state_snapshot(&self.state.value, &before_bytes)?;
        let fencing = request.request().fencing_token.ok_or_else(|| {
            RuntimeError::new(StableErrorCode::LeaseRequired, "fencingToken is required")
        })?;
        let lease_id = request.request().lease_id.ok_or_else(|| {
            RuntimeError::new(StableErrorCode::LeaseRequired, "leaseId is required")
        })?;
        let owner = LeaseOwner::new(
            self.session_id
                .map_or_else(|| "admin".to_owned(), |value| value.to_string()),
        )
        .map_err(store_error)?;
        let lease_proof = LeaseProof {
            lease_id,
            owner,
            fencing_token: fencing,
        };
        store
            .validate_lease_proof(&lease_proof, &UtcTimestamp::now())
            .map_err(store_error)?;
        if request.request().expected_revision != Some(authority.revision()) {
            return Err(RuntimeError::new(
                StableErrorCode::RevisionConflict,
                "expectedRevision does not match authoritative state",
            ));
        }
        let semantic = prepare_semantic_mutation(
            self.workspace,
            &state,
            request,
            &SemanticAuthorityContext {
                agent_id: self.agent_id.as_deref(),
                actor_role: self.workspace.agent_role,
                session_id: self.session_id,
                persistence: self.adapter.persistence.as_ref(),
                boot_id: &self.adapter.boot_id,
                policy_digest: &self.adapter.policy_digest,
                inventory_generation: self.workspace.inventory_generation,
            },
        )?;
        let mut after = state.clone();
        let transition = self.prepare_transition_commit(
            &after,
            request,
            semantic
                .as_ref()
                .and_then(|prepared| prepared.lifecycle.as_ref())
                .and_then(lifecycle_authority::PermittedLifecycleMutation::target_phase),
        )?;
        apply_mutation(&mut after, request, semantic.as_ref())?;
        let revision_after = authority
            .revision()
            .checked_next()
            .map_err(|_| schema_error("state revision overflow"))?;
        let object = after
            .as_object_mut()
            .ok_or_else(|| schema_error("authoritative state must be an object"))?;
        object.insert("revision".to_owned(), Value::from(revision_after.get()));
        object.insert("lastFencingToken".to_owned(), Value::from(fencing.get()));
        object.insert(
            "lastMutation".to_owned(),
            json!({
                "operation": request.operation().as_str(),
                "idempotencyKey": request.request().idempotency_key.as_deref(),
                "revisionBefore": authority.revision().get(),
                "revisionAfter": revision_after.get(),
            }),
        );
        let after_bytes = serde_json::to_vec_pretty(&after)
            .map_err(|_| schema_error("state could not be serialized"))?;
        let mut targets = vec![
            MutationTarget::new(
                self.state.relative.clone(),
                Some(ArtifactDigest::digest(&before_bytes)),
                after_bytes,
            )
            .map_err(store_error)?,
        ];
        if request.operation() == OperationName::DocumentSave {
            targets.push(document_target(self.workspace, &state, request)?);
        }
        let response_data = semantic
            .as_ref()
            .map(|prepared| prepared.data.clone())
            .unwrap_or_else(|| json!({"replayed":false}));
        if let Some(prepared) = semantic.as_ref() {
            for target in prepared.targets.clone() {
                targets.push(
                    MutationTarget::new(
                        ProjectRelativePath::new(target.relative_path).map_err(domain_error)?,
                        target.before_digest,
                        target.after_bytes,
                    )
                    .map_err(store_error)?,
                );
            }
        }
        // `review.record` events carry a bounded replay seed so the durable
        // SQLite projection can be rebuilt from committed events alone. `data`
        // already holds the batch/attempt/receipt; only the typed session is
        // additionally required to reconstruct the complete tuple.
        let event_value = match semantic
            .as_ref()
            .and_then(|prepared| prepared.review_session.as_ref())
        {
            Some(session) if request.operation() == OperationName::ReviewRecord => json!({
                "operation": request.operation().as_str(),
                "data": response_data,
                "reviewProjection": {"reviewSession": session},
            }),
            _ => json!({
                "operation": request.operation().as_str(),
                "data": response_data,
            }),
        };
        let event_bytes = serde_json::to_vec(&event_value)
            .map_err(|_| schema_error("event could not be serialized"))?;
        let result_bytes = serde_json::to_vec(&response_data)
            .map_err(|_| schema_error("operation result could not be serialized"))?;
        let result_digest = ResultDigest::digest(&result_bytes);
        let target_paths = targets
            .iter()
            .map(|target| target.path().as_str().to_owned())
            .collect::<Vec<_>>();
        let mutation = MutationRequest {
            mutation_id: RequestId::from_uuid(Uuid::new_v4()),
            workspace_id,
            work_item_id: work_item_id.clone(),
            operation: request.operation_id().clone(),
            idempotency_key: idempotency,
            canonical_payload_digest: payload_digest,
            expected_authority: authority,
            lease_proof,
            targets,
            event: JournalEvent {
                boot_id: self.adapter.boot_id,
                session_id: self.session_id,
                event_type: request.operation().as_str().to_owned().into_boxed_str(),
                schema_version: 1,
                payload: RuntimeEventPayload::InlineJson(event_bytes),
            },
            result_digest,
            prepared_at: UtcTimestamp::now(),
            committed_at: UtcTimestamp::now(),
        };
        if request.request().dry_run {
            store.validate_mutation(&mutation).map_err(store_error)?;
            return Ok(OperationResponse {
                changed: false,
                revision_before: Some(authority.revision()),
                revision_after: Some(authority.revision()),
                receipt_digest: None,
                data: json!({
                    "dryRun":true,
                    "wouldChange":true,
                    "targetPaths":target_paths,
                    "result":response_data,
                }),
            });
        }
        let committed = store.commit(mutation).map_err(store_error)?;
        // A `review.record` response must never succeed without its durable
        // projection. The project mutation is already committed, so a
        // projection failure stays retryable under the same idempotency key.
        if let Some(prepared) = semantic
            .as_ref()
            .and_then(|prepared| prepared.review_record.as_ref())
        {
            let write = prepared.projection_write(
                &self.workspace.workspace_id,
                work_item_id.as_str(),
                committed.event.event_sequence.get(),
            )?;
            upsert_review_authority_projection(&self.adapter.database, &write).map_err(
                |error| {
                    RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        format!(
                            "review projection is not durable; retry the same idempotencyKey: {}",
                            error.message()
                        ),
                    )
                },
            )?;
        }
        if let Some((input, target)) = transition {
            let commit_key = format!(
                "flow-commit-{}",
                ResultDigest::digest(idempotency_key.as_bytes())
            );
            self.adapter.flow.record_transition_committed(
                &self.adapter.boot_id.to_string(),
                &self.workspace.workspace_id,
                self.session_id.as_ref().map(ToString::to_string).as_deref(),
                request
                    .request()
                    .work_item_id
                    .as_ref()
                    .expect("transition workItemId was validated")
                    .as_str(),
                &commit_key,
                input,
                target,
                committed.receipt.revision_after,
            )?;
        }
        Ok(OperationResponse {
            changed: !committed.replayed,
            revision_before: Some(committed.receipt.revision_before),
            revision_after: Some(committed.receipt.revision_after),
            receipt_digest: Some(committed.receipt.result_digest.into_array()),
            data: response_data,
        })
    }
}

/// Bounded page size for durable `review.record` event scans.
const REVIEW_EVENT_PAGE: usize = 256;

/// Reconstructs one Review Batch v2 projection write from a committed
/// `review.record` event payload.
///
/// The event carries `data` (batch/attempt/optional receipt) plus the
/// `reviewProjection.reviewSession` replay seed. Historical events without the
/// seed have no reconstructable authority and fail closed; callers decide
/// whether a bounded latest-event state fallback applies.
fn review_projection_write_from_event(
    event: &DurableEvent,
    workspace_id: &str,
    work_item_id: &str,
) -> RuntimeResult<Option<ReviewProjectionWrite>> {
    if event.kind != OperationName::ReviewRecord.as_str()
        || event.workspace_id.as_deref() != Some(workspace_id)
        || event.work_item_id.as_deref() != Some(work_item_id)
    {
        return Ok(None);
    }
    let Some(session) = event.payload.pointer("/reviewProjection/reviewSession") else {
        return Ok(None);
    };
    let data = event.payload.get("data").ok_or_else(|| {
        RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "committed review event lacks its result data",
        )
    })?;
    let seed = json!({"reviewSession": session.clone(), "review": data.clone()});
    review_authority::review_projection_write_from_state(
        &seed,
        workspace_id,
        work_item_id,
        event.event_seq,
    )
}

/// Repairs Review Batch v2 projections from the committed event history for one
/// workspace and Work Item.
///
/// Events are scanned in bounded pages and replayed in event order. Exact rows
/// are accepted, missing rows are restored, and drift fails closed. Seedless
/// historical events are skipped here and handled by the bounded state
/// fallback in the caller.
fn repair_review_projections_from_events(
    database: &Path,
    persistence: &dyn PersistencePort,
    workspace_id: &str,
    work_item_id: &str,
) -> RuntimeResult<usize> {
    let mut writes = Vec::new();
    let mut cursor = 0_u64;
    loop {
        let page = persistence.events_after(cursor, REVIEW_EVENT_PAGE)?;
        if page.is_empty() {
            break;
        }
        for event in &page {
            cursor = cursor.max(event.event_seq);
            if let Some(write) =
                review_projection_write_from_event(event, workspace_id, work_item_id)?
            {
                writes.push(write);
            }
        }
        if page.len() < REVIEW_EVENT_PAGE {
            break;
        }
    }
    if writes.is_empty() {
        return Ok(0);
    }
    let repaired = writes.len();
    rebuild_review_authority_projections(database, &writes)?;
    Ok(repaired)
}

#[derive(Clone)]
struct LocatedState {
    absolute: PathBuf,
    relative: ProjectRelativePath,
    value: Value,
}

fn read_state(workspace: &BusinessWorkspace, work_item: &str) -> RuntimeResult<LocatedState> {
    let root = Path::new(&workspace.canonical_root);
    let directory = root.join(".auto-engineering");
    let mut matches = Vec::new();
    for entry in fs::read_dir(&directory).map_err(io_error)? {
        let path = entry.map_err(io_error)?.path().join("state.json");
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path).map_err(io_error)?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| schema_error("authoritative state JSON is malformed"))?;
        if state_matches(&value, work_item) {
            matches.push((path, value));
        }
    }
    if matches.len() != 1 {
        return Err(RuntimeError::new(
            if matches.is_empty() {
                StableErrorCode::ProjectMismatch
            } else {
                StableErrorCode::ScopeAmbiguous
            },
            "Work Item state could not be resolved unambiguously",
        ));
    }
    let (absolute, value) = matches.pop().expect("one match was checked");
    let relative = absolute
        .strip_prefix(root)
        .map_err(|_| schema_error("state path escaped workspace"))?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(LocatedState {
        absolute,
        relative: ProjectRelativePath::new(relative).map_err(domain_error)?,
        value,
    })
}

fn state_matches(value: &Value, work_item: &str) -> bool {
    [
        "stateMachineName",
        "currentWorkItem",
        "activeStory",
        "activeTask",
    ]
    .iter()
    .any(|key| value.get(*key).and_then(Value::as_str) == Some(work_item))
        || value
            .get("storyStates")
            .and_then(Value::as_object)
            .is_some_and(|stories| stories.contains_key(work_item))
}

fn flow_input(
    state: &Value,
    work_item_id: &str,
    event_store_id: EventStoreId,
) -> RuntimeResult<FlowInput> {
    let view = work_item_view(state, work_item_id);
    let phase = parse_phase(
        view.get("currentPhase")
            .or_else(|| view.get("phase"))
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("authoritative Work Item phase is missing"))?,
    )?;
    let revision = StateRevision::new(
        state
            .get("revision")
            .and_then(Value::as_u64)
            .ok_or_else(|| schema_error("authoritative state revision is missing"))?,
    );
    let correction_count = view
        .get("correctionCount")
        .or_else(|| state.get("correctionCount"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let mut snapshot = FlowSnapshot::new(phase, revision, correction_count);
    if phase == ProcessPhase::Paused {
        let paused_from = view
            .get("pausedFrom")
            .or_else(|| state.get("pausedFrom"))
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("paused Work Item is missing pausedFrom"))?;
        snapshot = snapshot.with_paused_from(parse_phase(paused_from)?);
    }
    let scale = parse_scale(
        state
            .get("scale")
            .and_then(Value::as_str)
            .unwrap_or("large"),
    )?;
    let route = state
        .get("routeDecision")
        .and_then(|value| value.get("selectedDesign"))
        .or_else(|| state.get("selectedDesign"))
        .and_then(Value::as_str)
        .unwrap_or("DR");
    let fingerprint = authoritative_input_fingerprint(state)?;
    let mut environment = FlowEnvironment::new(
        event_store_id,
        fingerprint,
        RouteSelection::new(scale, parse_route(route)?),
    );
    if let Some(cursor) = execution_cursor_from_state(state)? {
        environment = environment.with_execution_cursor(cursor);
    }
    Ok(FlowInput::new(snapshot, environment))
}

/// Reads the approved execution cursor observed in authoritative state.  The
/// cursor carries only the active ordinal, queue digest and slice status; the
/// capsule body never enters a flow decision.
fn execution_cursor_from_state(state: &Value) -> RuntimeResult<Option<ExecutionCursor>> {
    let Some(runtime) = state.get("executionRuntime") else {
        return Ok(None);
    };
    let queue_digest = runtime
        .get("queueDigest")
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error("executionRuntime queueDigest is missing"))?;
    let queue_digest =
        ArtifactDigest::from_str(queue_digest.strip_prefix("sha256:").unwrap_or(queue_digest))
            .map_err(|_| schema_error("executionRuntime queueDigest is malformed"))?;
    let active_ordinal = runtime
        .get("activeSliceOrdinal")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| schema_error("executionRuntime activeSliceOrdinal is missing"))?;
    Ok(Some(ExecutionCursor::new(
        active_ordinal,
        queue_digest,
        ExecutionSliceStatus::Pending,
    )))
}

fn work_item_view<'a>(state: &'a Value, work_item_id: &str) -> &'a Value {
    ["storyStates", "taskStates", "drStates"]
        .into_iter()
        .find_map(|collection| {
            state
                .get(collection)
                .and_then(Value::as_object)
                .and_then(|items| items.get(work_item_id))
        })
        .unwrap_or(state)
}

fn authoritative_input_fingerprint(state: &Value) -> RuntimeResult<InputFingerprint> {
    let mut authority = state.clone();
    strip_derived_runtime_fields(&mut authority);
    let bytes = serde_json::to_vec(&authority)
        .map_err(|_| schema_error("authoritative input fingerprint could not be computed"))?;
    Ok(InputFingerprint::digest(bytes))
}

fn strip_derived_runtime_fields(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("gateResults");
            object.remove("hookGuard");
            object.remove("nextActions");
            for child in object.values_mut() {
                strip_derived_runtime_fields(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                strip_derived_runtime_fields(child);
            }
        }
        _ => {}
    }
}

fn prepare_semantic_mutation(
    workspace: &BusinessWorkspace,
    state: &Value,
    request: &ValidatedOperationRequest,
    authority: &SemanticAuthorityContext<'_>,
) -> RuntimeResult<Option<PreparedSemanticMutation>> {
    let work_item_id = request
        .request()
        .work_item_id
        .as_ref()
        .ok_or_else(|| schema_error("workItemId is required"))?;
    match request.operation() {
        OperationName::EvidenceRecord => {
            let story_id = operation_story_id(state, work_item_id.as_str())?;
            let prepared = evidence::prepare_record(
                Path::new(&workspace.canonical_root),
                story_id,
                &request.request().payload,
                &UtcTimestamp::now().to_string(),
            )
            .map_err(evidence_error)?;
            Ok(Some(PreparedSemanticMutation::with_targets(
                prepared.result,
                prepared.targets,
                prepared.authority,
            )))
        }
        OperationName::EvidenceFinalize => {
            let story_id = operation_story_id(state, work_item_id.as_str())?;
            let prepared =
                evidence::prepare_finalize(Path::new(&workspace.canonical_root), story_id)
                    .map_err(evidence_error)?;
            Ok(Some(PreparedSemanticMutation::with_targets(
                prepared.result,
                prepared.targets,
                prepared.authority,
            )))
        }
        OperationName::VerificationPlan => {
            if request
                .request()
                .payload
                .get("persist")
                .and_then(Value::as_bool)
                == Some(false)
            {
                return Err(schema_error(
                    "verification.plan persist=false is not an authoritative mutation",
                ));
            }
            let expected_revision = request
                .request()
                .expected_revision
                .ok_or_else(|| schema_error("expectedRevision is required"))?;
            let toolset_job_id = request
                .request()
                .payload
                .get("toolsetJobId")
                .and_then(Value::as_str)
                .ok_or_else(|| schema_error("toolsetJobId is required"))?;
            let job = authority
                .persistence
                .load_job(toolset_job_id)?
                .ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::GateBlocked,
                        "toolset receipt job does not exist",
                    )
                })?;
            let authority = execution_authority::prepare_execution_plan_from_authority(
                Path::new(&workspace.canonical_root),
                state,
                &request.request().payload,
                &job,
                &workspace.workspace_id,
                work_item_id.as_str(),
                expected_revision,
                authority.policy_digest,
                authority.inventory_generation,
            )?;
            let story_id = operation_story_id(state, work_item_id.as_str())?;
            let mut plan = verification::build_verification_plan(
                Path::new(&workspace.canonical_root),
                story_id,
                work_item_id.as_str(),
                authority.changed_paths(),
                authority.since_fingerprint(),
            )
            .map_err(|error| schema_error(&error.to_string()))?;
            let object = plan
                .as_object_mut()
                .ok_or_else(|| schema_error("verificationPlan must be an object"))?;
            object.insert(
                "toolsetJobId".to_owned(),
                Value::String(authority.toolset_job_id().to_owned()),
            );
            object.insert(
                "sourceRevision".to_owned(),
                Value::from(authority.source_revision()),
            );
            object.insert(
                "planDigest".to_owned(),
                Value::String(authority.plan_digest().to_owned()),
            );
            object.insert(
                "inputFingerprint".to_owned(),
                Value::String(authority.input_fingerprint().to_owned()),
            );
            object.insert(
                "evidenceInputFingerprint".to_owned(),
                Value::String(authority.input_fingerprint().to_owned()),
            );
            execution_authority::validate_verification_input_binding(authority.plan(), &plan)?;
            Ok(Some(PreparedSemanticMutation::plain(plan)))
        }
        OperationName::ExecutionPlanSet => Ok(Some(PreparedSemanticMutation::plain(
            governance::execution_plan(&request.request().payload).map_err(schema_error)?,
        ))),
        OperationName::ExecutionPlanApprove => {
            let confirmation = request
                .request()
                .confirmation
                .as_ref()
                .ok_or_else(|| schema_error("execution plan approval requires confirmation"))?;
            Ok(Some(PreparedSemanticMutation::plain(
                governance::approved_execution_plan(state, confirmation).map_err(schema_error)?,
            )))
        }
        OperationName::ReviewRecord => {
            let caller = AuthenticatedCaller::new(
                authority.agent_id.ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::RoleOperationForbidden,
                        "review.record requires a daemon-authenticated agentId",
                    )
                })?,
                authority.session_id.ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::RoleOperationForbidden,
                        "review.record requires a daemon-authenticated sessionId",
                    )
                })?,
                authority.actor_role.ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::RoleOperationForbidden,
                        "review.record requires a daemon-authenticated role",
                    )
                })?,
            );
            let prepared = review_authority::prepare_review_record(
                workspace,
                state,
                work_item_id.as_str(),
                request,
                &caller,
                authority.persistence,
                &authority.boot_id.to_string(),
                authority.policy_digest,
                authority.inventory_generation,
                &UtcTimestamp::now(),
            )?;
            let review = prepared
                .review
                .clone()
                .ok_or_else(|| schema_error("review authority did not project a v2 batch"))?;
            if let Some(receipt) = prepared.receipt.as_ref() {
                if review.get("receipt") != Some(receipt) {
                    return Err(schema_error(
                        "review authority returned an inconsistent terminal receipt",
                    ));
                }
            } else {
                if review.get("receipt").is_some() {
                    return Err(schema_error(
                        "review authority projected a receipt for a nonterminal batch",
                    ));
                }
            }
            Ok(Some(PreparedSemanticMutation {
                data: review.clone(),
                targets: Vec::new(),
                evidence_authority: None,
                review: Some(review),
                review_session: Some(prepared.review_session.clone()),
                review_binding: Some(PreparedReviewBinding {
                    input_fingerprint: prepared.input_fingerprint.clone(),
                    ruleset_fingerprint: prepared.ruleset_fingerprint.clone(),
                    policy_digest: authority.policy_digest.to_owned(),
                    inventory_generation: authority.inventory_generation,
                }),
                review_record: Some(prepared),
                lifecycle: None,
            }))
        }
        OperationName::StateTransition | OperationName::WorkItemComplete => {
            let expected_revision = request
                .request()
                .expected_revision
                .ok_or_else(|| schema_error("expectedRevision is required"))?;
            let outcome = lifecycle_authority::prepare_lifecycle_mutation(
                state,
                work_item_id.as_str(),
                request.operation(),
                &request.request().payload,
                expected_revision,
                request.request().confirmation.as_ref(),
                authority.actor_role.ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::RoleOperationForbidden,
                        "lifecycle mutation requires a daemon-authenticated role",
                    )
                })?,
                authority.session_id,
                system_time_unix_ms()?,
            )?;
            let permitted = outcome.into_permitted()?;
            Ok(Some(PreparedSemanticMutation::lifecycle(permitted)))
        }
        _ => Ok(None),
    }
}

fn operation_story_id<'a>(state: &'a Value, work_item_id: &'a str) -> RuntimeResult<&'a str> {
    if state
        .get("storyStates")
        .and_then(Value::as_object)
        .is_some_and(|stories| stories.contains_key(work_item_id))
    {
        return Ok(work_item_id);
    }
    ["activeStory", "currentStory"]
        .into_iter()
        .find_map(|field| state.get(field).and_then(Value::as_str))
        .filter(|story| !story.is_empty())
        .ok_or_else(|| schema_error("operation requires an authoritative Story scope"))
}

fn evidence_error(error: evidence::EvidenceError) -> RuntimeError {
    let conflict = matches!(
        error,
        evidence::EvidenceError::InvalidManifest(_)
            | evidence::EvidenceError::ManifestTampered
            | evidence::EvidenceError::LedgerTampered(_)
            | evidence::EvidenceError::SnapshotInvalid(_)
            | evidence::EvidenceError::Io(_)
    );
    RuntimeError::new(
        if conflict {
            StableErrorCode::ExternalStateConflict
        } else {
            StableErrorCode::OperationSchemaInvalid
        },
        error.to_string(),
    )
}

fn committed_result_data(committed: &CommittedMutation) -> RuntimeResult<Value> {
    let RuntimeEventPayload::InlineJson(bytes) = &committed.event.draft.payload else {
        return Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "operation receipt references a non-inline result event",
        ));
    };
    let event: Value = serde_json::from_slice(bytes).map_err(|_| {
        RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "operation result event is malformed",
        )
    })?;
    let data = event.get("data").cloned().ok_or_else(|| {
        RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "operation result event is missing data",
        )
    })?;
    let data_bytes = serde_json::to_vec(&data)
        .map_err(|_| schema_error("operation result could not be canonicalized"))?;
    if ResultDigest::digest(&data_bytes) != committed.receipt.result_digest {
        return Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "operation result digest does not match its durable event",
        ));
    }
    Ok(data)
}

fn apply_mutation(
    state: &mut Value,
    request: &ValidatedOperationRequest,
    semantic: Option<&PreparedSemanticMutation>,
) -> RuntimeResult<()> {
    match request.operation() {
        OperationName::ExecutionPlanSet => {
            let object = root_state_object_mut(state)?;
            object.insert(
                "executionPlan".to_owned(),
                prepared_data(semantic, "execution plan was not prepared")?.clone(),
            );
        }
        OperationName::ExecutionPlanApprove => {
            root_state_object_mut(state)?.insert(
                "executionPlan".to_owned(),
                prepared_data(semantic, "execution plan approval was not prepared")?.clone(),
            );
        }
        OperationName::EvidenceRecord | OperationName::EvidenceFinalize => {
            let prepared = semantic.ok_or_else(|| schema_error("evidence was not prepared"))?;
            let authority = prepared
                .evidence_authority
                .clone()
                .ok_or_else(|| schema_error("evidence authority projection was not prepared"))?;
            root_state_object_mut(state)?.insert("evidenceAuthority".to_owned(), authority);
        }
        OperationName::ReviewRecord => {
            let prepared = semantic.ok_or_else(|| schema_error("review was not prepared"))?;
            let review_session = prepared
                .review_session
                .clone()
                .ok_or_else(|| schema_error("reviewSession was not prepared"))?;
            let binding = prepared
                .review_binding
                .as_ref()
                .ok_or_else(|| schema_error("review authority binding was not prepared"))?;
            let object = root_state_object_mut(state)?;
            let review = prepared
                .review
                .clone()
                .ok_or_else(|| schema_error("review batch was not prepared"))?;
            object.insert("review".to_owned(), review);
            object.remove("reviewLoop");
            object.insert("reviewSession".to_owned(), review_session);
            object.insert(
                "inputFingerprint".to_owned(),
                Value::String(binding.input_fingerprint.clone()),
            );
            object.insert(
                "rulesetFingerprint".to_owned(),
                Value::String(binding.ruleset_fingerprint.clone()),
            );
            object.insert(
                "policyDigest".to_owned(),
                Value::String(binding.policy_digest.clone()),
            );
            object.insert(
                "inventoryGeneration".to_owned(),
                Value::from(binding.inventory_generation),
            );
        }
        OperationName::VerificationPlan => {
            let plan = prepared_data(semantic, "verification plan was not prepared")?.clone();
            root_state_object_mut(state)?.insert("verificationPlan".to_owned(), plan);
        }
        OperationName::StateTransition | OperationName::WorkItemComplete => {
            apply_lifecycle_intents(
                state,
                request,
                semantic.ok_or_else(|| schema_error("lifecycle plan was not prepared"))?,
            )?;
        }
        OperationName::DocumentSave => {}
        _ => return Err(schema_error("mutation operation is not implemented")),
    }
    Ok(())
}

fn prepared_data<'a>(
    semantic: Option<&'a PreparedSemanticMutation>,
    error: &str,
) -> RuntimeResult<&'a Value> {
    semantic
        .map(|prepared| &prepared.data)
        .ok_or_else(|| schema_error(error))
}

fn apply_lifecycle_intents(
    state: &mut Value,
    request: &ValidatedOperationRequest,
    prepared: &PreparedSemanticMutation,
) -> RuntimeResult<()> {
    let work_item_id = request
        .request()
        .work_item_id
        .as_ref()
        .ok_or_else(|| schema_error("workItemId is required"))?;
    let permitted = prepared
        .lifecycle
        .as_ref()
        .ok_or_else(|| schema_error("permitted lifecycle mutation is missing"))?;
    let after =
        lifecycle_authority::apply_exact_after_image(state, work_item_id.as_str(), permitted)?;
    *state = after;
    Ok(())
}

fn root_state_object_mut(state: &mut Value) -> RuntimeResult<&mut Map<String, Value>> {
    state
        .as_object_mut()
        .ok_or_else(|| schema_error("authoritative state must be an object"))
}

fn resolve_document(
    workspace: &BusinessWorkspace,
    state: &Value,
    request: &ValidatedOperationRequest,
) -> RuntimeResult<Value> {
    let intent = request.request().payload["intent"]
        .as_str()
        .ok_or_else(|| schema_error("intent is required"))?;
    let relative = document_path(state, intent)?;
    let absolute = Path::new(&workspace.canonical_root).join(relative.as_str());
    let bytes = fs::read(&absolute).map_err(io_error)?;
    Ok(json!({
        "path": relative.as_str(),
        "digest": ArtifactDigest::digest(&bytes).to_string(),
        "byteLength": bytes.len(),
    }))
}

fn document_target(
    workspace: &BusinessWorkspace,
    state: &Value,
    request: &ValidatedOperationRequest,
) -> RuntimeResult<MutationTarget> {
    let intent = request.request().payload["intent"]
        .as_str()
        .ok_or_else(|| schema_error("intent is required"))?;
    let relative = document_path(state, intent)?;
    let content = request.request().payload["contentFile"]
        .as_str()
        .ok_or_else(|| schema_error("contentFile is required"))?;
    let content = ProjectRelativePath::new(content.to_owned()).map_err(domain_error)?;
    let root = Path::new(&workspace.canonical_root);
    let bytes = fs::read(root.join(content.as_str())).map_err(io_error)?;
    let destination = root.join(relative.as_str());
    let before = fs::read(destination)
        .ok()
        .map(|bytes| ArtifactDigest::digest(&bytes));
    MutationTarget::new(relative, before, bytes).map_err(store_error)
}

fn document_path(state: &Value, intent: &str) -> RuntimeResult<ProjectRelativePath> {
    let value = state
        .get("documentPaths")
        .and_then(Value::as_object)
        .and_then(|paths| paths.get(intent))
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error("document intent has no authoritative destination mapping"))?;
    ProjectRelativePath::new(value.to_owned()).map_err(domain_error)
}

fn mutate_lease<C: CommitFaultPort>(
    store: &ProjectMutationStore<
        StdDurableFileSystem,
        StdCrossProcessLock,
        SqliteRuntimeRepository,
        C,
    >,
    request: &ValidatedOperationRequest,
    session_id: Option<SessionId>,
    boot_id: BootId,
) -> RuntimeResult<OperationResponse> {
    let now = UtcTimestamp::now();
    let owner = if request.operation() == OperationName::LeaseBreak {
        LeaseOwner::new("admin:authenticated-local-client")
    } else {
        LeaseOwner::new(session_id.as_ref().map_or_else(
            || serde_json::to_string(&request.request().payload["owner"]).unwrap_or_default(),
            ToString::to_string,
        ))
    }
    .map_err(store_error)?;

    let idempotency_key = request
        .request()
        .idempotency_key
        .as_deref()
        .ok_or_else(|| schema_error("idempotencyKey is required"))?;
    let canonical_payload_digest = lease_control_digest(request, session_id.as_ref())?;
    let action = match request.operation() {
        OperationName::LeaseAcquire => {
            let ttl = request.request().payload["ttlSeconds"]
                .as_u64()
                .ok_or_else(|| schema_error("ttlSeconds is required"))?;
            let lease_id = deterministic_lease_id(idempotency_key, canonical_payload_digest);
            LeaseControlAction::Acquire {
                lease_id,
                owner,
                expires_at: add_seconds_from(&now, ttl)?,
                now,
            }
        }
        OperationName::LeaseRenew => {
            let proof = lease_proof(request, owner)?;
            let ttl = request.request().payload["ttlSeconds"]
                .as_u64()
                .ok_or_else(|| schema_error("ttlSeconds is required"))?;
            LeaseControlAction::Renew {
                proof,
                expires_at: add_seconds_from(&now, ttl)?,
                now,
            }
        }
        OperationName::LeaseRelease => LeaseControlAction::Release {
            proof: lease_proof(request, owner)?,
            now,
        },
        OperationName::LeaseBreak => LeaseControlAction::Break {
            actor: owner,
            reason: request.request().payload["reason"]
                .as_str()
                .ok_or_else(|| schema_error("reason is required"))?
                .to_owned()
                .into_boxed_str(),
            now,
        },
        _ => return Err(schema_error("unsupported lease mutation")),
    };
    let control = LeaseControlRequest {
        mutation_id: RequestId::from_uuid(Uuid::new_v4()),
        workspace_id: request
            .request()
            .workspace_id
            .ok_or_else(|| schema_error("workspaceId is required"))?,
        work_item_id: request
            .request()
            .work_item_id
            .clone()
            .ok_or_else(|| schema_error("workItemId is required"))?,
        operation: request.operation_id().clone(),
        idempotency_key: IdempotencyKey::new(idempotency_key.to_owned()).map_err(store_error)?,
        canonical_payload_digest,
        action,
        boot_id,
        session_id,
        committed_at: UtcTimestamp::now(),
    };
    if request.request().dry_run {
        let preview = store.preview_lease_control(&control).map_err(store_error)?;
        return Ok(OperationResponse {
            changed: false,
            revision_before: Some(preview.revision),
            revision_after: Some(preview.revision),
            receipt_digest: None,
            data: json!({"dryRun":true,"wouldChange":true,"result":preview.data}),
        });
    }
    let committed = store.commit_lease_control(control).map_err(store_error)?;
    Ok(OperationResponse {
        changed: !committed.mutation.replayed,
        revision_before: Some(committed.mutation.receipt.revision_before),
        revision_after: Some(committed.mutation.receipt.revision_after),
        receipt_digest: Some(committed.mutation.receipt.result_digest.into_array()),
        data: committed.data,
    })
}

fn lease_proof(
    request: &ValidatedOperationRequest,
    owner: LeaseOwner,
) -> RuntimeResult<LeaseProof> {
    Ok(LeaseProof {
        lease_id: request.request().lease_id.ok_or_else(|| {
            RuntimeError::new(StableErrorCode::LeaseRequired, "leaseId is required")
        })?,
        owner,
        fencing_token: request.request().fencing_token.ok_or_else(|| {
            RuntimeError::new(StableErrorCode::LeaseRequired, "fencingToken is required")
        })?,
    })
}

fn lease_control_digest(
    request: &ValidatedOperationRequest,
    session_id: Option<&SessionId>,
) -> RuntimeResult<InputFingerprint> {
    let binding = json!({
        "payload":request.request().payload.clone(),
        "sessionId":session_id.map(ToString::to_string),
        "leaseId":request.request().lease_id.map(|value| value.to_string()),
        "fencingToken":request.request().fencing_token.map(FencingToken::get),
    });
    let bytes = serde_json::to_vec(&binding)
        .map_err(|_| schema_error("lease control binding could not be canonicalized"))?;
    Ok(InputFingerprint::digest(bytes))
}

fn deterministic_lease_id(key: &str, binding: InputFingerprint) -> LeaseId {
    let mut digest = Sha256::new();
    digest.update(b"ae-sdd-lease-id/v1\0");
    digest.update(key.as_bytes());
    digest.update([0]);
    digest.update(binding.to_string().as_bytes());
    let digest: [u8; 32] = digest.finalize().into();
    let mut uuid = [0_u8; 16];
    uuid.copy_from_slice(&digest[..16]);
    LeaseId::from_uuid(Uuid::from_bytes(uuid))
}

fn lease_status<C: CommitFaultPort>(
    store: &ProjectMutationStore<
        StdDurableFileSystem,
        StdCrossProcessLock,
        SqliteRuntimeRepository,
        C,
    >,
) -> RuntimeResult<Value> {
    match fs::read(store.paths().lease_path()) {
        Ok(bytes) => {
            let ledger = LeaseLedger::from_json(&bytes).map_err(store_error)?;
            Ok(ledger.active().map_or_else(
                || json!({"active":false,"lastFencingToken":ledger.last_fencing_token().get()}),
                |record| {
                    json!({
                        "active":true,
                        "leaseId":record.lease_id().to_string(),
                        "owner":record.owner().as_str(),
                        "fencingToken":record.fencing_token().get(),
                        "expiresAt":record.expires_at().to_string(),
                    })
                },
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(json!({"active":false})),
        Err(error) => Err(io_error(error)),
    }
}

fn operation_registry(operation: Option<&str>) -> RuntimeResult<Value> {
    let specs = OPERATION_REGISTRY
        .iter()
        .filter(|spec| operation.is_none_or(|name| spec.operation.as_str() == name))
        .collect::<Vec<_>>();
    if operation.is_some() && specs.is_empty() {
        return Err(RuntimeError::new(
            StableErrorCode::OperationNotRegistered,
            "typed operation is not registered",
        ));
    }
    Ok(Value::Array(
        specs
            .into_iter()
            .map(|spec| {
                json!({
                    "operation": spec.operation.as_str(),
                    "scope": format!("{:?}", spec.scope).to_lowercase(),
                    "writes": spec.writes,
                    "requiresLease": spec.requires_lease,
                    "requiresRevision": spec.requires_revision,
                    "requiresIdempotency": spec.requires_idempotency,
                    "requiresConfirmation": spec.requires_confirmation,
                    "fields": spec.fields.iter().map(|field| json!({
                        "name":field.name,"kind":format!("{:?}",field.kind),"required":field.required
                    })).collect::<Vec<_>>(),
                })
            })
            .collect(),
    ))
}

fn response_value(response: OperationResponse) -> Value {
    json!({
        "changed": response.changed,
        "revisionBefore": response.revision_before.map(StateRevision::get),
        "revisionAfter": response.revision_after.map(StateRevision::get),
        "receiptDigest": response.receipt_digest.map(hex::encode),
        "data": response.data,
    })
}

fn add_seconds_from(now: &UtcTimestamp, seconds: u64) -> RuntimeResult<UtcTimestamp> {
    let seconds = i64::try_from(seconds).map_err(|_| schema_error("ttlSeconds is too large"))?;
    let timestamp = now
        .as_timestamp()
        .checked_add(jiff::SignedDuration::from_secs(seconds))
        .map_err(|_| schema_error("lease expiry overflow"))?;
    UtcTimestamp::from_str(&timestamp.to_string()).map_err(store_error)
}

fn parse_phase(value: &str) -> RuntimeResult<ProcessPhase> {
    match value {
        "initialized" => Ok(ProcessPhase::Initialized),
        "route-selected" => Ok(ProcessPhase::RouteSelected),
        "requirement-analyzed" => Ok(ProcessPhase::RequirementAnalyzed),
        "dr-generated" => Ok(ProcessPhase::DrGenerated),
        "story-generated" => Ok(ProcessPhase::StoryGenerated),
        "testcase-generated" => Ok(ProcessPhase::TestcaseGenerated),
        "coding-process" => Ok(ProcessPhase::CodingProcess),
        "coding" => Ok(ProcessPhase::Coding),
        "test-running" => Ok(ProcessPhase::TestRunning),
        "code-reviewed" => Ok(ProcessPhase::CodeReviewed),
        "completed" => Ok(ProcessPhase::Completed),
        "paused" => Ok(ProcessPhase::Paused),
        _ => Err(schema_error(
            "phase is not in the Rust lifecycle vocabulary",
        )),
    }
}

fn required_gate(value: &str) -> Option<RequiredGate> {
    match value {
        "G-00" => Some(RequiredGate::G00),
        "G-01" => Some(RequiredGate::G01),
        "G-02" => Some(RequiredGate::G02),
        "G-03" => Some(RequiredGate::G03),
        "G-04" => Some(RequiredGate::G04),
        "G-07" => Some(RequiredGate::G07),
        "G-08" => Some(RequiredGate::G08),
        "G-09" => Some(RequiredGate::G09),
        "G-10" => Some(RequiredGate::G10),
        "G-11" => Some(RequiredGate::G11),
        "G-12" => Some(RequiredGate::G12),
        "G-13" => Some(RequiredGate::G13),
        "G-14" => Some(RequiredGate::G14),
        "G-CODE-1" => Some(RequiredGate::GCode1),
        "G-CODEPLAN-SRC" => Some(RequiredGate::GCodePlanSource),
        "G-DR-CTX" => Some(RequiredGate::GDrContext),
        "G-HTTP-1" => Some(RequiredGate::GHttp1),
        "G-RA-1" => Some(RequiredGate::GRa1),
        "G-RA-2" => Some(RequiredGate::GRa2),
        "G-RA-3" => Some(RequiredGate::GRa3),
        "G-RA-4" => Some(RequiredGate::GRa4),
        "G-RA-5" => Some(RequiredGate::GRa5),
        "G-RA-6" => Some(RequiredGate::GRa6),
        "G-RA-FLOW-VIOLATION" => Some(RequiredGate::GRaFlowViolation),
        "G-REVIEW-DEPTH" => Some(RequiredGate::GReviewDepth),
        "G-STORY-CTX" => Some(RequiredGate::GStoryContext),
        _ => None,
    }
}

fn parse_scale(value: &str) -> RuntimeResult<WorkScale> {
    match value.trim().to_ascii_lowercase().as_str() {
        "large" | "大" | "大型" => Ok(WorkScale::Large),
        "medium" | "中" | "中型" => Ok(WorkScale::Medium),
        "small" | "小" | "小型" => Ok(WorkScale::Small),
        "micro" | "微" | "微型" => Ok(WorkScale::Micro),
        _ => Err(schema_error("scale is invalid")),
    }
}

fn parse_route(value: &str) -> RuntimeResult<DesignRoute> {
    match value.to_ascii_lowercase().as_str() {
        "dr" => Ok(DesignRoute::Dr),
        "story" => Ok(DesignRoute::Story),
        "codingplan" | "coding-plan" => Ok(DesignRoute::CodingPlan),
        _ => Err(schema_error("design route is invalid")),
    }
}

fn parse<T: FromStr>(value: &str, field: &str) -> RuntimeResult<T> {
    T::from_str(value).map_err(|_| schema_error(&format!("{field} is invalid")))
}

fn operation_error(error: OperationServiceError) -> RuntimeError {
    match error {
        OperationServiceError::RoleOperationForbidden => RuntimeError::new(
            StableErrorCode::RoleOperationForbidden,
            "trusted role/grant forbids the operation",
        ),
        OperationServiceError::Backend(error) => error
            .downcast_ref::<RuntimeError>()
            .cloned()
            .unwrap_or_else(|| schema_error("authoritative operation backend failed")),
        _ => schema_error("typed operation request or response is invalid"),
    }
}

fn store_error(error: StoreError) -> RuntimeError {
    let code = match error {
        StoreError::RevisionConflict { .. } => StableErrorCode::RevisionConflict,
        StoreError::ExternalStateConflict { .. } | StoreError::JournalConflict { .. } => {
            StableErrorCode::ExternalStateConflict
        }
        StoreError::LeaseConflict => StableErrorCode::LeaseConflict,
        StoreError::LeaseRequired | StoreError::LeaseMismatch { .. } => {
            StableErrorCode::LeaseRequired
        }
        StoreError::LeaseExpired => StableErrorCode::LeaseExpired,
        StoreError::StaleFencingToken { .. } => StableErrorCode::StaleFencingToken,
        StoreError::IdempotencyKeyReused { .. } => StableErrorCode::IdempotencyKeyReused,
        _ => StableErrorCode::OperationSchemaInvalid,
    };
    RuntimeError::new(
        code,
        format!("authoritative project store rejected the operation: {error}"),
    )
}

fn io_error(_error: std::io::Error) -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::ExternalStateConflict,
        "authoritative project file I/O failed",
    )
}

fn domain_error<E>(_error: E) -> RuntimeError {
    schema_error("typed domain identity or path is invalid")
}

fn schema_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::OperationSchemaInvalid, message)
}

fn authoritative_state_snapshot(cached: &Value, fresh_bytes: &[u8]) -> RuntimeResult<Value> {
    let fresh: Value = serde_json::from_slice(fresh_bytes).map_err(|_| {
        RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "authoritative state JSON is malformed",
        )
    })?;
    let cached_revision = cached
        .get("revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "cached authoritative state revision is missing",
            )
        })?;
    let fresh_revision = fresh
        .get("revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "authoritative state revision is missing",
            )
        })?;
    if cached_revision == fresh_revision && cached != &fresh {
        return Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "authoritative state changed without advancing revision",
        ));
    }
    Ok(fresh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoritative_state_snapshot_rejects_same_revision_drift() {
        let cached = json!({"revision":7,"phase":"coding"});
        let fresh = serde_json::to_vec(&json!({"revision":7,"phase":"test-running"}))
            .expect("fresh state serializes");

        let error = authoritative_state_snapshot(&cached, &fresh)
            .expect_err("same revision with different content must fail closed");

        assert_eq!(error.code(), StableErrorCode::ExternalStateConflict);
    }
}
