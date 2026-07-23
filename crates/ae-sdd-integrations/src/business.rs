use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ae_sdd_domain::{
    AgentRole, ArtifactDigest, BootId, DesignRoute, EventStoreId, FencingToken, GateOutcome,
    InputFingerprint, LeaseId, ProcessPhase, ProjectKey, ProjectRelativePath, RequestId,
    ResultDigest, ScopedGrant, SessionId, StateRevision, WorkItemId, WorkScale,
};
use ae_sdd_flow::{FlowEnvironment, FlowInput, FlowSnapshot, NextAction, RouteSelection};
use ae_sdd_gates::GateRegistry;
use ae_sdd_operations::{
    Confirmation, ExecutionIdentity, OPERATION_REGISTRY, OperationBackend, OperationName,
    OperationRequest, OperationResponse, OperationService, OperationServiceError,
    ValidatedOperationRequest,
};
use ae_sdd_policy::{RequiredGate, RoleOperation, RolePolicy, TransitionContext, TransitionPolicy};
use ae_sdd_protocol::{RequestParams, RpcMethod, StableErrorCode, WorkspaceMode};
use ae_sdd_runtime::{
    BusinessOperationPort, BusinessWorkspace, ContextProjectionInput, FlowSupervisor,
    PersistencePort, RuntimeError, RuntimeResult,
};
use ae_sdd_store::{
    IdempotencyKey, JournalEvent, LeaseLedger, LeaseOwner, LeaseProof, MutationRequest,
    MutationTarget, ProjectMutationStore, ProjectStorePaths, RuntimeEventPayload,
    SqliteRuntimeRepository, StateAuthority, StdCrossProcessLock, StdDurableFileSystem, StoreError,
    UtcTimestamp,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::{AuthoritativeGateRuntime, SqliteRuntimePersistence, gate_result_json};

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

    fn gate_evaluate(
        &self,
        workspace: Option<&BusinessWorkspace>,
        params: &RequestParams<Value>,
    ) -> RuntimeResult<Value> {
        let workspace = require_workspace(workspace)?;
        let work_item_id = require_work_item(params)?;
        let gate_id = params.payload["gateId"]
            .as_str()
            .ok_or_else(|| schema_error("gateId is required"))?;
        let gates = AuthoritativeGateRuntime::new(
            workspace,
            work_item_id,
            &self.policy_digest,
            params.fencing_token,
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
        let idempotency_key = params.idempotency_key.as_deref().ok_or_else(|| {
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
            RpcMethod::OperationDescribe => Ok(operation_registry()),
            RpcMethod::OperationExecute => {
                let workspace = require_workspace(workspace)?;
                let wire: ExecuteWire = decode(params.payload.clone())?;
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
                let request = operation_request(operation, params, workspace, wire.payload)?;
                let backend = ProjectBackend::open(self, workspace, params)?;
                let role = workspace.agent_role.ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::RoleOperationForbidden,
                        "typed operations require a daemon-verified Agent role",
                    )
                })?;
                RolePolicy::authorize(role, semantic_operation(role, operation)).map_err(|_| {
                    RuntimeError::new(
                        StableErrorCode::RoleOperationForbidden,
                        "daemon-verified role forbids the typed operation",
                    )
                })?;
                let operation_id =
                    ae_sdd_domain::OperationId::new(operation.as_str()).map_err(domain_error)?;
                let grant = ScopedGrant::new([operation_id], [], []);
                let response = OperationService::execute(
                    ExecutionIdentity::Agent {
                        role,
                        grant: &grant,
                    },
                    request,
                    &backend,
                )
                .map_err(operation_error)?;
                Ok(response_value(response))
            }
            RpcMethod::FlowSnapshot | RpcMethod::FlowNext => {
                let workspace = require_workspace(workspace)?;
                let work_item_id = require_work_item(params)?;
                let located = read_state(workspace, work_item_id)?;
                let input = flow_input(&located.value, work_item_id, self.event_store_id)?;
                let decision = if method == RpcMethod::FlowNext {
                    if let Some(target) = params.payload.get("targetPhase").and_then(Value::as_str)
                    {
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
        match entrypoint {
            "assets.read" => {
                let path = job_project_path(workspace, arguments, "path")?;
                let bytes = fs::read(path).map_err(io_error)?;
                if bytes.len() > 1_048_576 {
                    return Err(schema_error("asset read exceeds the 1 MiB job bound"));
                }
                Ok(json!({
                    "outcome":"PASS",
                    "content":String::from_utf8_lossy(&bytes),
                    "byteLength":bytes.len(),
                    "digest":ArtifactDigest::digest(&bytes).to_string(),
                }))
            }
            "assets.check" => {
                let paths = arguments
                    .get("paths")
                    .and_then(Value::as_array)
                    .ok_or_else(|| schema_error("assets.check requires paths"))?;
                let missing = paths
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|path| {
                        ProjectRelativePath::new((*path).to_owned()).map_or(true, |relative| {
                            !Path::new(&workspace.canonical_root)
                                .join(relative.as_str())
                                .is_file()
                        })
                    })
                    .collect::<Vec<_>>();
                Ok(json!({
                    "outcome": if missing.is_empty() {"PASS"} else {"FAIL"},
                    "missing": missing,
                }))
            }
            "assets.stats" => asset_stats(workspace),
            "git.status" | "git.diff" | "git.log" | "git.impact" | "git.blame" => {
                git_job(workspace, entrypoint, arguments)
            }
            "db.audit" => {
                SqliteRuntimePersistence::open(&self.database)?.integrity_check()?;
                Ok(json!({"outcome":"PASS","integrity":"ok"}))
            }
            _ => Err(RuntimeError::new(
                StableErrorCode::OperationNotRegistered,
                "background job entrypoint is not implemented by a Rust adapter",
            )),
        }
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

fn job_project_path(
    workspace: &BusinessWorkspace,
    arguments: &Value,
    field: &str,
) -> RuntimeResult<PathBuf> {
    let value = arguments
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error("job path argument is required"))?;
    let relative = ProjectRelativePath::new(value.to_owned()).map_err(domain_error)?;
    let root = Path::new(&workspace.canonical_root);
    let path = root.join(relative.as_str());
    let canonical = path.canonicalize().map_err(io_error)?;
    if !canonical.starts_with(root) {
        return Err(RuntimeError::new(
            StableErrorCode::WorkspaceOutsideAllowedRoot,
            "job path escaped the registered workspace",
        ));
    }
    Ok(canonical)
}

fn asset_stats(workspace: &BusinessWorkspace) -> RuntimeResult<Value> {
    let root = Path::new(&workspace.canonical_root);
    let mut pending = vec![root.to_path_buf()];
    let mut files = 0_u64;
    let mut bytes = 0_u64;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let metadata = entry.metadata().map_err(io_error)?;
            if metadata.is_dir() {
                if pending.len() >= 4_096 {
                    return Err(schema_error("asset traversal directory bound exceeded"));
                }
                pending.push(entry.path());
            } else if metadata.is_file() {
                files = files.saturating_add(1);
                bytes = bytes.saturating_add(metadata.len());
                if files > 100_000 {
                    return Err(schema_error("asset traversal file bound exceeded"));
                }
            }
        }
    }
    Ok(json!({"outcome":"PASS","files":files,"bytes":bytes}))
}

fn git_job(
    workspace: &BusinessWorkspace,
    entrypoint: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let mut command = match entrypoint {
        "git.status" => vec!["status".to_owned(), "--short".to_owned()],
        "git.diff" => vec!["diff".to_owned(), "--stat".to_owned()],
        "git.log" => vec![
            "log".to_owned(),
            "--oneline".to_owned(),
            "-n".to_owned(),
            "50".to_owned(),
        ],
        "git.impact" => vec![
            "diff".to_owned(),
            "--name-only".to_owned(),
            "HEAD~1".to_owned(),
        ],
        "git.blame" => {
            let relative = arguments
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| schema_error("git.blame requires path"))?;
            ProjectRelativePath::new(relative.to_owned()).map_err(domain_error)?;
            vec!["blame".to_owned(), "--".to_owned(), relative.to_owned()]
        }
        _ => return Err(schema_error("unsupported Git job")),
    };
    command.insert(0, "--no-pager".to_owned());
    let output = crate::BoundedCommandRunner::new(1_048_576)
        .run(
            Path::new("git"),
            &command,
            Some(Path::new(&workspace.canonical_root)),
            std::time::Duration::from_secs(30),
        )
        .map_err(|_| RuntimeError::new(StableErrorCode::GateError, "Git job failed"))?;
    Ok(json!({
        "outcome": if output.exit_code == Some(0) {"PASS"} else {"ERROR"},
        "exitCode":output.exit_code,
        "stdout":String::from_utf8_lossy(&output.stdout),
        "stderr":String::from_utf8_lossy(&output.stderr),
    }))
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
            RoleOperation::RequestGlobalTransition
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
    #[serde(default = "empty_object")]
    payload: Value,
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

fn operation_request(
    operation: OperationName,
    params: &RequestParams<Value>,
    workspace: &BusinessWorkspace,
    payload: Value,
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
        payload,
    })
}

struct ProjectBackend<'a> {
    adapter: &'a NativeBusinessAdapter,
    workspace: &'a BusinessWorkspace,
    state: LocatedState,
    session_id: Option<SessionId>,
    deadline_ms: u64,
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
        ProjectMutationStore<StdDurableFileSystem, StdCrossProcessLock, SqliteRuntimeRepository>,
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
        Ok(ProjectMutationStore::new(
            paths,
            StdDurableFileSystem,
            StdCrossProcessLock,
            repository,
        ))
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
        match request.operation() {
            OperationName::LeaseAcquire
            | OperationName::LeaseRenew
            | OperationName::LeaseRelease
            | OperationName::LeaseBreak => mutate_lease(&self.store()?, request, self.session_id),
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
        let gates = AuthoritativeGateRuntime::new(
            self.workspace,
            work_item_id.as_str(),
            &self.adapter.policy_digest,
            request.request().fencing_token.map(FencingToken::get),
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

    fn authorize_transition_if_needed(
        &self,
        state: &Value,
        request: &ValidatedOperationRequest,
    ) -> RuntimeResult<Option<(FlowInput, ProcessPhase)>> {
        if !matches!(
            request.operation(),
            OperationName::StateTransition | OperationName::WorkItemComplete
        ) {
            return Ok(None);
        }
        let work_item_id = request
            .request()
            .work_item_id
            .as_ref()
            .ok_or_else(|| schema_error("workItemId is required"))?;
        let view = work_item_view(state, work_item_id.as_str());
        let current = parse_phase(
            view.get("currentPhase")
                .or_else(|| view.get("phase"))
                .and_then(Value::as_str)
                .ok_or_else(|| schema_error("current phase is missing"))?,
        )?;
        let target = if request.operation() == OperationName::WorkItemComplete {
            ProcessPhase::Completed
        } else {
            parse_phase(
                request
                    .request()
                    .payload
                    .get("targetPhase")
                    .and_then(Value::as_str)
                    .ok_or_else(|| schema_error("targetPhase is missing"))?,
            )?
        };
        let route = state
            .get("routeDecision")
            .and_then(|value| value.get("selectedDesign"))
            .or_else(|| state.get("selectedDesign"))
            .and_then(Value::as_str)
            .unwrap_or("DR");
        let permit = TransitionPolicy::authorize(TransitionContext {
            actor_role: AgentRole::Root,
            current,
            target,
            scale: parse_scale(
                state
                    .get("scale")
                    .and_then(Value::as_str)
                    .unwrap_or("large"),
            )?,
            design_route: parse_route(route)?,
            paused_from: view
                .get("pausedFrom")
                .and_then(Value::as_str)
                .map(parse_phase)
                .transpose()?,
        })
        .map_err(|_| {
            RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "transition is illegal for the authoritative route and phase",
            )
        })?;
        let input = flow_input(state, work_item_id.as_str(), self.adapter.event_store_id)?;
        let decision = self.adapter.flow.project(
            &self.workspace.workspace_id,
            work_item_id.as_str(),
            input,
        )?;
        if decision.pending_transition() != Some(target)
            || decision.required_gates() != permit.required_gates()
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
        let gates = AuthoritativeGateRuntime::new(
            self.workspace,
            work_item_id.as_str(),
            &self.adapter.policy_digest,
            Some(fencing.get()),
        )?;
        let count = u64::try_from(permit.required_gates().len()).unwrap_or(u64::MAX);
        let per_gate_ms = self
            .deadline_ms
            .checked_div(count.max(1))
            .unwrap_or(1)
            .max(1);
        for required in permit.required_gates() {
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
        let store = self.store()?;
        if let Some(committed) = store
            .replay_committed(workspace_id, &idempotency, payload_digest)
            .map_err(store_error)?
        {
            return Ok(OperationResponse {
                changed: false,
                revision_before: Some(committed.receipt.revision_before),
                revision_after: Some(committed.receipt.revision_after),
                receipt_digest: Some(committed.receipt.result_digest.into_array()),
                data: json!({"replayed":true}),
            });
        }
        let before_bytes = fs::read(&self.state.absolute).map_err(io_error)?;
        let authority = StateAuthority::inspect(&before_bytes).map_err(store_error)?;
        if request.request().expected_revision != Some(authority.revision()) {
            return Err(RuntimeError::new(
                StableErrorCode::RevisionConflict,
                "expectedRevision does not match authoritative state",
            ));
        }
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
        let mut after = self.state.value.clone();
        let transition = self.authorize_transition_if_needed(&after, request)?;
        apply_mutation(&mut after, request)?;
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
            targets.push(document_target(self.workspace, &self.state.value, request)?);
        }
        let event_value = json!({"operation": request.operation().as_str()});
        let event_bytes = serde_json::to_vec(&event_value)
            .map_err(|_| schema_error("event could not be serialized"))?;
        let result_digest = ResultDigest::digest(&event_bytes);
        let committed = store
            .commit(MutationRequest {
                mutation_id: RequestId::from_uuid(Uuid::new_v4()),
                workspace_id,
                work_item_id: request
                    .request()
                    .work_item_id
                    .clone()
                    .ok_or_else(|| schema_error("workItemId is required"))?,
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
            })
            .map_err(store_error)?;
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
            data: json!({"replayed": committed.replayed}),
        })
    }
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
    Ok(FlowInput::new(
        snapshot,
        FlowEnvironment::new(
            event_store_id,
            fingerprint,
            RouteSelection::new(scale, parse_route(route)?),
        ),
    ))
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

fn apply_mutation(state: &mut Value, request: &ValidatedOperationRequest) -> RuntimeResult<()> {
    let payload = request.request().payload.clone();
    match request.operation() {
        OperationName::ExecutionPlanSet => {
            let object = root_state_object_mut(state)?;
            object.insert("executionPlan".to_owned(), payload);
        }
        OperationName::ExecutionPlanApprove => {
            let object = root_state_object_mut(state)?;
            let plan = object
                .get_mut("executionPlan")
                .and_then(Value::as_object_mut)
                .ok_or_else(|| schema_error("executionPlan does not exist"))?;
            plan.insert("approved".to_owned(), Value::Bool(true));
            plan.insert("approval".to_owned(), payload);
        }
        OperationName::EvidenceRecord => {
            push_array(root_state_object_mut(state)?, "evidence", payload)?;
        }
        OperationName::EvidenceFinalize => {
            root_state_object_mut(state)?.insert("evidenceFinalized".to_owned(), Value::Bool(true));
        }
        OperationName::ReviewRecord => {
            root_state_object_mut(state)?.insert("review".to_owned(), payload);
        }
        OperationName::VerificationPlan => {
            root_state_object_mut(state)?.insert("verificationPlan".to_owned(), payload);
        }
        OperationName::StateTransition => {
            let target = payload
                .get("targetPhase")
                .and_then(Value::as_str)
                .ok_or_else(|| schema_error("targetPhase is required"))?;
            let work_item_id = request
                .request()
                .work_item_id
                .as_ref()
                .ok_or_else(|| schema_error("workItemId is required"))?;
            let object = work_item_object_mut(state, work_item_id.as_str())?;
            object.insert("phase".to_owned(), Value::String(target.to_owned()));
            object.insert("currentPhase".to_owned(), Value::String(target.to_owned()));
        }
        OperationName::WorkItemComplete => {
            let work_item_id = request
                .request()
                .work_item_id
                .as_ref()
                .ok_or_else(|| schema_error("workItemId is required"))?;
            let object = work_item_object_mut(state, work_item_id.as_str())?;
            object.insert("phase".to_owned(), Value::String("completed".to_owned()));
            object.insert(
                "currentPhase".to_owned(),
                Value::String("completed".to_owned()),
            );
        }
        OperationName::DocumentSave => {}
        _ => return Err(schema_error("mutation operation is not implemented")),
    }
    Ok(())
}

fn root_state_object_mut(state: &mut Value) -> RuntimeResult<&mut Map<String, Value>> {
    state
        .as_object_mut()
        .ok_or_else(|| schema_error("authoritative state must be an object"))
}

fn work_item_object_mut<'a>(
    state: &'a mut Value,
    work_item_id: &str,
) -> RuntimeResult<&'a mut Map<String, Value>> {
    let collection = ["storyStates", "taskStates", "drStates"]
        .into_iter()
        .find(|collection| {
            state
                .get(*collection)
                .and_then(Value::as_object)
                .is_some_and(|items| items.contains_key(work_item_id))
        });
    if let Some(collection) = collection {
        return state
            .get_mut(collection)
            .and_then(Value::as_object_mut)
            .and_then(|items| items.get_mut(work_item_id))
            .and_then(Value::as_object_mut)
            .ok_or_else(|| schema_error("nested Work Item state is malformed"));
    }
    root_state_object_mut(state)
}

fn push_array(object: &mut Map<String, Value>, key: &str, value: Value) -> RuntimeResult<()> {
    let array = object
        .entry(key.to_owned())
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| schema_error("authoritative collection has the wrong type"))?;
    array.push(value);
    Ok(())
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

fn mutate_lease(
    store: &ProjectMutationStore<
        StdDurableFileSystem,
        StdCrossProcessLock,
        SqliteRuntimeRepository,
    >,
    request: &ValidatedOperationRequest,
    session_id: Option<SessionId>,
) -> RuntimeResult<OperationResponse> {
    let state_bytes = fs::read(store.paths().state_path()).map_err(io_error)?;
    let state_authority = StateAuthority::inspect(&state_bytes).map_err(store_error)?;
    let now = UtcTimestamp::now();
    let owner = LeaseOwner::new(session_id.map_or_else(
        || serde_json::to_string(&request.request().payload["owner"]).unwrap_or_default(),
        |value| value.to_string(),
    ))
    .map_err(store_error)?;
    let data = match request.operation() {
        OperationName::LeaseAcquire => {
            let ttl = request.request().payload["ttlSeconds"]
                .as_u64()
                .ok_or_else(|| schema_error("ttlSeconds is required"))?;
            let expires = add_seconds(ttl)?;
            let lease_id = request
                .request()
                .idempotency_key
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok())
                .map_or_else(|| LeaseId::from_uuid(Uuid::new_v4()), LeaseId::from_uuid);
            let record = store
                .acquire_lease(lease_id, owner, now, expires)
                .map_err(store_error)?;
            json!({
                "leaseId": record.lease_id().to_string(),
                "fencingToken": record.fencing_token().get(),
                "expiresAt": record.expires_at().to_string(),
            })
        }
        OperationName::LeaseRenew => {
            let proof = lease_proof(request, owner)?;
            let ttl = request.request().payload["ttlSeconds"]
                .as_u64()
                .ok_or_else(|| schema_error("ttlSeconds is required"))?;
            let record = store
                .renew_lease(&proof, &now, add_seconds(ttl)?)
                .map_err(store_error)?;
            json!({"leaseId":record.lease_id().to_string(),"expiresAt":record.expires_at().to_string()})
        }
        OperationName::LeaseRelease => {
            let tombstone = store
                .release_lease(&lease_proof(request, owner)?, now)
                .map_err(store_error)?;
            json!({"leaseId":tombstone.lease_id.to_string(),"status":"released"})
        }
        OperationName::LeaseBreak => {
            let reason = request.request().payload["reason"]
                .as_str()
                .ok_or_else(|| schema_error("reason is required"))?;
            let tombstone = store.break_lease(owner, reason, now).map_err(store_error)?;
            json!({"broken":tombstone.is_some()})
        }
        _ => return Err(schema_error("unsupported lease mutation")),
    };
    let digest = ResultDigest::digest(serde_json::to_vec(&data).unwrap_or_default());
    Ok(OperationResponse {
        changed: true,
        revision_before: Some(state_authority.revision()),
        revision_after: Some(state_authority.revision()),
        receipt_digest: Some(digest.into_array()),
        data,
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

fn lease_status(
    store: &ProjectMutationStore<
        StdDurableFileSystem,
        StdCrossProcessLock,
        SqliteRuntimeRepository,
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

fn operation_registry() -> Value {
    Value::Array(
        OPERATION_REGISTRY
            .iter()
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
    )
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

fn add_seconds(seconds: u64) -> RuntimeResult<UtcTimestamp> {
    let seconds = i64::try_from(seconds).map_err(|_| schema_error("ttlSeconds is too large"))?;
    let timestamp = jiff::Timestamp::now()
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
    RuntimeError::new(code, "authoritative project store rejected the operation")
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
