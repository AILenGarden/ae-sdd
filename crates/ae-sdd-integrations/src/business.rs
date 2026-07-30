use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ae_sdd_contracts::execution_runtime::{ExecutionCapsuleV1, ExecutionSliceStatus};
use ae_sdd_contracts::lifecycle::CompletionMilestoneInput;
use ae_sdd_contracts::series::RouteInput;
use ae_sdd_contracts::{BoundedText, ExecutionId, ExecutionStepId, PrdId, SchemaVersion, WorkerId};
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, ArtifactKind, ArtifactRef, BootId, CompletionDigestSet,
    CompletionMilestone, DesignRoute, EventStoreId, EvidenceDigest, FencingToken, GateOutcome,
    InputFingerprint, LeaseId, OperationId, ProcessPhase, ProjectKey, ProjectRelativePath,
    RequestId, ResultDigest, ScopedGrant, SessionId, StateRevision, StoryId, WorkItemId, WorkScale,
    WorkspaceId,
};
use ae_sdd_execution::{
    CapsuleBuildOutcome, ExecutionSliceEvent, ExecutionStep, RefactorCycleV1,
    VerificationExecutionPlan, transition_slice_status,
};
use ae_sdd_flow::{
    ExecutionCursor, FlowEnvironment, FlowInput, FlowSnapshot, NextAction, RouteEngine,
    RouteSelection,
};
use ae_sdd_gates::{GateInputSelector, GateRegistry};
use ae_sdd_operations::{
    Confirmation, ExecutionIdentity, OPERATION_REGISTRY, OperationBackend, OperationName,
    OperationRequest, OperationRequestError, OperationResponse, OperationService,
    OperationServiceError, ValidatedOperationRequest, validate_operation_payload,
};
use ae_sdd_policy::{RequiredGate, RoleOperation, RolePolicy, TransitionContext, TransitionPolicy};
use ae_sdd_protocol::JobStatus;
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
use serde::{Deserialize, Serialize};
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
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    Operation(String),
}

impl ProcessAbortCommitFault {
    /// Returns whether the environment-selected operation scope matches.
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
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
    /// Long-lived authoritative Gate runtimes, one per
    /// (workspace, Work Item, policy, inventory) scope, so the scheduler key
    /// cache and single-flight survive across operations. Entries bind their
    /// construction scope; a policy or inventory drift simply misses the
    /// cache and builds a fresh runtime.
    gate_runtimes: Arc<std::sync::Mutex<BTreeMap<GateRuntimeScope, AuthoritativeGateRuntime>>>,
}

/// Cache scope of one long-lived authoritative Gate runtime.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct GateRuntimeScope {
    workspace_id: String,
    work_item_id: String,
    policy_digest: String,
    inventory_generation: u64,
}

/// Bound on cached Gate runtimes; a full map is cleared rather than grown
/// unboundedly, which simply rebuilds the least-recent scopes on demand.
const GATE_RUNTIME_CACHE_LIMIT: usize = 64;

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
            gate_runtimes: Arc::new(std::sync::Mutex::new(BTreeMap::new())),
        }
    }

    /// Returns the long-lived authoritative Gate runtime for one workspace
    /// and Work Item, building it on first use. The cached runtime carries no
    /// fencing expectation: fencing stays a `GateKey` freshness dimension, so
    /// a lease rotation changes the key and re-evaluates instead of trusting
    /// a stale snapshot.
    fn gate_runtime(
        &self,
        workspace: &BusinessWorkspace,
        work_item_id: &str,
    ) -> RuntimeResult<AuthoritativeGateRuntime> {
        let scope = GateRuntimeScope {
            workspace_id: workspace.workspace_id.clone(),
            work_item_id: work_item_id.to_owned(),
            policy_digest: self.policy_digest.clone(),
            inventory_generation: workspace.inventory_generation,
        };
        let mut runtimes = self
            .gate_runtimes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(runtime) = runtimes.get(&scope) {
            return Ok(runtime.clone());
        }
        let runtime = AuthoritativeGateRuntime::with_review_authority(
            workspace,
            work_item_id,
            &self.policy_digest,
            None,
            self.review_gate_authority(workspace),
        )?;
        if runtimes.len() >= GATE_RUNTIME_CACHE_LIMIT {
            runtimes.clear();
        }
        runtimes.insert(scope, runtime.clone());
        Ok(runtime)
    }

    /// Drops cached Gate outcomes affected by committed mutations, mapped
    /// from the operation's changed selectors. A missing runtime means no
    /// Gate has been evaluated yet for this scope, so there is nothing to
    /// invalidate.
    fn invalidate_gate_selectors(
        &self,
        workspace: &BusinessWorkspace,
        work_item_id: &str,
        selectors: &[GateInputSelector],
    ) {
        let scope = GateRuntimeScope {
            workspace_id: workspace.workspace_id.clone(),
            work_item_id: work_item_id.to_owned(),
            policy_digest: self.policy_digest.clone(),
            inventory_generation: workspace.inventory_generation,
        };
        let runtimes = self
            .gate_runtimes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(runtime) = runtimes.get(&scope) {
            runtime.invalidate_selectors(selectors);
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

        let before_bytes = fs::read(&located.absolute)
            .map_err(|error| io_error("read authoritative state", error))?;
        let authority = StateAuthority::inspect(&before_bytes).map_err(store_error)?;
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
        let source_revision_is_current = authority.revision().get() == wire.source_revision;
        let source_input_is_current = located
            .value
            .get("inputFingerprint")
            .and_then(Value::as_str)
            == Some(input_fingerprint.as_str());
        if !source_revision_is_current && !source_input_is_current {
            return Err(RuntimeError::new(
                StableErrorCode::StaleGateResult,
                "toolset receipt sourceRevision no longer matches the project input",
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
            Err(error) => return Err(io_error("check toolset receipt locator", error)),
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
            "sourceRevision": wire.source_revision,
            "committedRevision": revision_after.get(),
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
        let (manifest_ref, manifest_before, manifest_bytes) = if wire.preserve_evidence_manifest {
            let manifest_ref = ProjectRelativePath::new(format!(
                ".auto-engineering/{}/evidence/manifest.json",
                work_item_id.as_str()
            ))
            .map_err(domain_error)?;
            let manifest_bytes =
                fs::read(Path::new(&workspace.canonical_root).join(manifest_ref.as_str()))
                    .map_err(|error| io_error("read sealed evidence manifest", error))?;
            let manifest: Value = serde_json::from_slice(&manifest_bytes)
                .map_err(|_| schema_error("finalized evidence manifest could not be decoded"))?;
            validate_toolset_manifest(&manifest, work_item_id.as_str())?;
            let before = Some(ArtifactDigest::digest(&manifest_bytes));
            (manifest_ref, before, manifest_bytes)
        } else {
            prepare_toolset_manifest(
                Path::new(&workspace.canonical_root),
                work_item_id.as_str(),
                &receipt_id,
                &identity.job_id,
                &receipt_digest,
                &plan_digest,
                &methodology_digest,
                &policy_digest,
                &input_fingerprint,
                wire.source_revision,
                wire.inventory_generation,
                &identity.session_id,
                artifact_ref.as_str(),
                &project_receipt_digest,
                &wire.receipt,
            )?
        };
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
                "sourceRevision": wire.source_revision,
                "committedRevision": revision_after.get(),
            }),
        );
        let final_binding = wire.finalized_evidence_binding.as_ref().map(|binding| {
            json!({
                "reviewId":binding.review_id,
                "sourceRevision":binding.source_revision,
                "inputFingerprint":binding.input_fingerprint,
                "rulesetFingerprint":binding.ruleset_fingerprint,
                "policyDigest":binding.policy_digest,
                "inventoryGeneration":binding.inventory_generation,
                "toolsetJobId":identity.job_id,
                "receiptId":receipt_id,
                "receiptDigest":receipt_digest,
                "planDigest":plan_digest,
                "methodologyDigest":methodology_digest,
            })
        });
        if let Some(binding) = &final_binding {
            state_object.insert("finalVerificationBinding".to_owned(), binding.clone());
        } else {
            state_object.remove("finalVerificationBinding");
        }
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
            "committedRevision".to_owned(),
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
        if let Some(binding) = final_binding {
            result_object.insert("finalVerificationBinding".to_owned(), binding);
        }

        let event_bytes = serde_json::to_vec(&json!({
            "operation": "toolset.receipt.record",
            "data": committed_result,
        }))
        .map_err(|_| schema_error("toolset receipt event could not be serialized"))?;
        let result_bytes = serde_json::to_vec(&committed_result)
            .map_err(|_| schema_error("toolset receipt result could not be serialized"))?;
        let mut targets = vec![
            MutationTarget::new(
                located.relative,
                Some(ArtifactDigest::digest(&before_bytes)),
                state_bytes,
            )
            .map_err(store_error)?,
            MutationTarget::new(artifact_ref, None, snapshot_bytes).map_err(store_error)?,
        ];
        if !wire.preserve_evidence_manifest {
            targets.push(
                MutationTarget::new(manifest_ref, manifest_before, manifest_bytes)
                    .map_err(store_error)?,
            );
        }
        let mutation = MutationRequest {
            mutation_id,
            workspace_id,
            work_item_id,
            operation,
            idempotency_key: idempotency,
            canonical_payload_digest: payload_digest,
            expected_authority: authority,
            lease_proof,
            targets,
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
        let gates = self.gate_runtime(workspace, work_item_id)?;
        let result = gates.evaluate(gate_id, Duration::from_millis(params.deadline_ms))?;
        if params
            .fencing_token
            .is_some_and(|expected| expected != result.key().fencing_token().get())
        {
            return Err(RuntimeError::new(
                StableErrorCode::StaleFencingToken,
                "Gate snapshot fencing token is no longer authoritative",
            ));
        }
        let mut projection = gate_result_json(&result);
        let stats = gates.stats();
        projection
            .as_object_mut()
            .expect("Gate projection is an object")
            .insert(
                "scheduler".to_owned(),
                json!({
                    "gatesEvaluated": stats.gates_evaluated,
                    "cacheHits": stats.cache_hits,
                    "cacheMisses": stats.cache_misses,
                }),
            );

        let Some(required_gate) = required_gate(gate_id) else {
            return Ok(projection);
        };
        let located = read_state(workspace, work_item_id)?;
        let input = flow_input(workspace, &located.value, work_item_id, self.event_store_id)?;
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

    fn transition_gate_passes(
        &self,
        workspace: &BusinessWorkspace,
        state: &Value,
        work_item_id: &str,
        target: ProcessPhase,
    ) -> RuntimeResult<Vec<RequiredGate>> {
        let input = flow_input(workspace, state, work_item_id, self.event_store_id)?;
        let decision = self
            .flow
            .project(&workspace.workspace_id, work_item_id, input)?;
        if decision.pending_transition() == Some(target)
            && matches!(decision.next_action(), NextAction::ApplyTransition { target: ready } if *ready == target)
        {
            Ok(decision.passed_gates().iter().copied().collect())
        } else {
            Ok(Vec::new())
        }
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
                // Creation runs before its Work Item exists, so it cannot take
                // the ProjectBackend path: that opens on an already-resolvable
                // `state.json`. Every later operation keeps the lease/revision
                // guarantees the backend enforces; this one is guarded instead
                // by exclusive-create on the state file itself.
                if operation == OperationName::WorkItemCreate {
                    return create_work_item(workspace, &request);
                }
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
                        // The preflight authorizes the same terminal transition
                        // as the commit, so it needs the same milestone
                        // projection or completion is denied before the
                        // confirmation binding is ever issued.
                        let completion = completion_projection(
                            workspace,
                            &backend.state.value,
                            work_item_id.as_str(),
                        )?;
                        let target = lifecycle_target(operation, &request.payload)?
                            .ok_or_else(|| schema_error("lifecycle target is required"))?;
                        let passed_gates = self.transition_gate_passes(
                            workspace,
                            &backend.state.value,
                            work_item_id.as_str(),
                            target,
                        )?;
                        let preflight =
                            lifecycle_authority::preflight_lifecycle_confirmation_with_gate_passes(
                                &backend.state.value,
                                work_item_id.as_str(),
                                operation,
                                &request.payload,
                                expected_revision,
                                completion,
                                role,
                                request.session_id,
                                system_time_unix_ms()?,
                                &passed_gates,
                            )?;
                        match preflight.disposition() {
                            lifecycle_authority::LifecycleAuthorityDisposition::Denied => {
                                return match preflight.into_permitted() {
                                    Err(error) => Err(error),
                                    Ok(_) => Err(schema_error(
                                        "denied lifecycle preflight unexpectedly became permitted",
                                    )),
                                };
                            }
                            lifecycle_authority::LifecycleAuthorityDisposition::AwaitingConfirmation => {
                                let binding = preflight.confirmation_binding().ok_or_else(|| {
                                    schema_error(
                                        "lifecycle preflight is missing its confirmation binding",
                                    )
                                })?;
                                return Err(RuntimeError::new(
                                    StableErrorCode::ConfirmationRequired,
                                    "lifecycle authority requires confirmation",
                                )
                                .with_remediation(format!(
                                    "provide lifecycle confirmation for binding {binding}"
                                )));
                            }
                            lifecycle_authority::LifecycleAuthorityDisposition::Permitted => {}
                        }
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
                if let Some(projection) = route_control_projection(&located.value, work_item_id)? {
                    if wire.target_phase.is_some() {
                        return Err(schema_error(
                            "targetPhase is forbidden until route.decide commits a route",
                        ));
                    }
                    return Ok(with_document_tree(projection, &located.value));
                }
                let input =
                    flow_input(workspace, &located.value, work_item_id, self.event_store_id)?;
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
                Ok(with_document_tree(
                    decorate_route_handoff(
                        FlowSupervisor::projection(&decision),
                        &located.value,
                        current_review_input_fingerprint(workspace, &located.value)?,
                    )?,
                    &located.value,
                ))
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
        let flow_projection =
            if let Some(projection) = route_control_projection(&located.value, work_item_id)? {
                projection
            } else {
                let flow = self.flow.project(
                    &workspace.workspace_id,
                    work_item_id,
                    flow_input(workspace, &located.value, work_item_id, self.event_store_id)?,
                )?;
                decorate_route_handoff(
                    FlowSupervisor::projection(&flow),
                    &located.value,
                    current_review_input_fingerprint(workspace, &located.value)?,
                )?
            };
        let flow_projection = with_document_tree(flow_projection, &located.value);
        let next_action = flow_projection
            .get("nextAction")
            .cloned()
            .unwrap_or_else(|| json!({"kind":"await-agent-work"}));
        let asset_refs = projection_asset_refs(
            workspace,
            flow_projection.get("phase").and_then(Value::as_str),
        );
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
                "assetRefs": asset_refs,
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
        if entrypoint == "toolset.receipt.record"
            && arguments.get("finalizedEvidence").is_none()
            && (arguments.get("preserveEvidenceManifest").is_some()
                || arguments.get("finalizedEvidenceBinding").is_some())
        {
            return Err(schema_error(
                "final verification provenance fields are daemon-reserved",
            ));
        }
        let prepared_arguments = if entrypoint == "toolset.receipt.record"
            && arguments.get("finalizedEvidence").is_some()
        {
            prepare_finalized_evidence_receipt(
                &self.policy_digest,
                workspace,
                work_item_id.ok_or_else(|| {
                    schema_error("finalized evidence receipt Work Item is required")
                })?,
                arguments,
            )?
        } else {
            arguments.clone()
        };
        let execution_arguments = toolset_execution_arguments(entrypoint, &prepared_arguments);
        let result = jobs::execute(
            workspace,
            work_item_id,
            &self.database,
            self.persistence.as_ref(),
            identity,
            &self.policy_digest,
            entrypoint,
            &execution_arguments,
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
            &prepared_arguments,
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
        let canonical_root = fs::canonicalize(&workspace.canonical_root)
            .map_err(|error| io_error("canonicalize workspace root", error))?;
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
            let absolute = fs::canonicalize(canonical_root.join(relative.as_str()))
                .map_err(|error| io_error("canonicalize delegation artifact", error))?;
            if !absolute.starts_with(&canonical_root) || !absolute.is_file() {
                return Err(child_result_error(
                    "child deliverable is not a regular file inside the registered workspace",
                ));
            }
            let bytes =
                fs::read(&absolute).map_err(|error| io_error("read delegation artifact", error))?;
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

fn toolset_execution_arguments(entrypoint: &str, arguments: &Value) -> Value {
    let mut executable = arguments.clone();
    if entrypoint == "toolset.receipt.record"
        && let Some(object) = executable.as_object_mut()
    {
        object.remove("preserveEvidenceManifest");
        object.remove("finalizedEvidenceBinding");
    }
    executable
}

/// Builds the terminal receipt from daemon-owned Review bindings and sealed
/// evidence. The Host supplies neither an execution plan nor a receipt.
fn prepare_finalized_evidence_receipt(
    policy_digest: &str,
    workspace: &BusinessWorkspace,
    work_item_id: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let request: FinalizedEvidenceReceiptRequest = decode(arguments.clone())?;
    let located = read_state(workspace, work_item_id)?;
    let session = located
        .value
        .get("reviewSession")
        .and_then(Value::as_object)
        .ok_or_else(|| schema_error("finalized evidence receipt requires a Tier 3 Review"))?;
    if session.get("tier").and_then(Value::as_str) != Some("tier3")
        || session.get("status").and_then(Value::as_str) != Some("running")
        || session.get("reviewId").and_then(Value::as_str)
            != Some(request.finalized_evidence.review_id.as_str())
        || session.get("sourceRevision").and_then(Value::as_u64)
            != Some(request.finalized_evidence.source_revision)
        || session.get("inputFingerprint").and_then(Value::as_str)
            != Some(request.finalized_evidence.input_fingerprint.as_str())
        || session.get("rulesetFingerprint").and_then(Value::as_str)
            != Some(request.finalized_evidence.ruleset_fingerprint.as_str())
        || session.get("policyDigest").and_then(Value::as_str)
            != Some(request.finalized_evidence.policy_digest.as_str())
        || session.get("inventoryGeneration").and_then(Value::as_u64)
            != Some(request.finalized_evidence.inventory_generation)
    {
        return Err(RuntimeError::new(
            StableErrorCode::StaleGateResult,
            "finalized evidence receipt does not match the active Tier 3 Review",
        ));
    }
    if request.finalized_evidence.policy_digest != policy_digest
        || request.finalized_evidence.inventory_generation != workspace.inventory_generation
    {
        return Err(RuntimeError::new(
            StableErrorCode::StaleGateResult,
            "finalized evidence receipt binding is stale for this daemon",
        ));
    }
    let fingerprint = InputFingerprint::from_str(&request.finalized_evidence.input_fingerprint)
        .map_err(|_| schema_error("finalized evidence inputFingerprint is invalid"))?;
    let manifest_bytes = review_authority::validate_finalized_review_evidence(
        workspace,
        &located.value,
        work_item_id,
        fingerprint,
    )?;
    let manifest_ref = format!(".auto-engineering/{work_item_id}/evidence/manifest.json");
    let manifest_digest = ArtifactDigest::digest(&manifest_bytes);
    let execution_id = ExecutionId::new(format!(
        "final-evidence-{}",
        &manifest_digest.to_string()[..24]
    ))
    .map_err(domain_error)?;
    let plan = VerificationExecutionPlan::new(
        SchemaVersion::V1,
        execution_id,
        WorkItemId::new(work_item_id.to_owned()).map_err(domain_error)?,
        fingerprint,
        vec![
            ExecutionStep::new(
                SchemaVersion::V1,
                ExecutionStepId::new("sealed-evidence-verification").map_err(domain_error)?,
                ArtifactRef::new(
                    ArtifactKind::new("sealed-evidence-manifest").map_err(domain_error)?,
                    ProjectRelativePath::new(manifest_ref.clone()).map_err(domain_error)?,
                    manifest_digest,
                    u64::try_from(manifest_bytes.len())
                        .map_err(|_| schema_error("finalized evidence manifest is too large"))?,
                ),
                Vec::<BoundedText<256>>::new(),
                None,
                Vec::new(),
            )
            .map_err(domain_error)?,
        ],
    )
    .map_err(domain_error)?;
    let observed_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| schema_error("system clock is before the Unix epoch"))?
        .as_millis();
    let observed_at = u64::try_from(observed_at)
        .map_err(|_| schema_error("system clock exceeds the receipt range"))?;
    let receipt = plan
        .receipt(
            WorkerId::new("daemon-sealed-evidence-verifier").map_err(domain_error)?,
            JobStatus::Pass,
            Some(0),
            EvidenceDigest::digest(&manifest_bytes),
            EvidenceDigest::digest([]),
            observed_at,
            observed_at,
            false,
            false,
        )
        .map_err(domain_error)?;
    let methodology_digest = ArtifactDigest::digest(b"ae-sdd/finalized-evidence-receipt/v1");
    Ok(json!({
        "plan": plan,
        "receipt": receipt,
        "sourceRevision": request.finalized_evidence.source_revision,
        "policyDigest": request.finalized_evidence.policy_digest,
        "methodologyDigest": methodology_digest.to_string(),
        "inventoryGeneration": request.finalized_evidence.inventory_generation,
        "leaseId": request.lease_id,
        "fencingToken": request.fencing_token,
        "preserveEvidenceManifest": true,
        "finalizedEvidenceBinding": request.finalized_evidence,
    }))
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
        Err(error) => return Err(io_error("read sealed evidence manifest", error)),
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
        OperationName::ReviewRecord | OperationName::ReviewContribute
            if payload.get("reviewedPaths").is_some() =>
        {
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
        OperationName::RouteDecide => RoleOperation::SelectRoute,
        OperationName::DocumentSave => RoleOperation::ModifyAssignedPaths,
        OperationName::EvidenceRecord | OperationName::EvidenceFinalize => {
            RoleOperation::SubmitEvidence
        }
        OperationName::VerificationPlan => RoleOperation::RunAssignedTests,
        OperationName::ReviewContribute | OperationName::ReviewRecord => {
            RoleOperation::SubmitReviewFindings
        }
        OperationName::ReviewFinalize => RoleOperation::RequestGlobalTransition,
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
    #[serde(default)]
    preserve_evidence_manifest: bool,
    #[serde(default)]
    finalized_evidence_binding: Option<FinalizedEvidenceBinding>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FinalizedEvidenceReceiptRequest {
    finalized_evidence: FinalizedEvidenceBinding,
    lease_id: String,
    fencing_token: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FinalizedEvidenceBinding {
    review_id: String,
    source_revision: u64,
    input_fingerprint: String,
    ruleset_fingerprint: String,
    policy_digest: String,
    inventory_generation: u64,
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
    execution_runtime: Option<Value>,
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

/// Bounded writer-lease TTL for the internal atomic append commit of one
/// `review.contribute`; never exposed to callers as a cross-reviewer lease.
const REVIEW_CONTRIBUTE_LEASE_TTL_SECONDS: u64 = 300;

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
    passed_gates: &'a [RequiredGate],
}

impl PreparedSemanticMutation {
    fn plain(data: Value) -> Self {
        Self {
            data,
            targets: Vec::new(),
            execution_runtime: None,
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

    fn execution(
        data: Value,
        targets: Vec<evidence::SemanticTarget>,
        execution_runtime: Value,
    ) -> Self {
        Self {
            data,
            targets,
            execution_runtime: Some(execution_runtime),
            evidence_authority: None,
            review: None,
            review_session: None,
            review_binding: None,
            review_record: None,
            lifecycle: None,
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
            OperationName::ReviewContribute => self.mutate_review_contribution(request),
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
            OperationName::ReviewContribute => self.mutate_review_contribution(request),
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
            self.workspace,
            &self.state.value,
            work_item_id.as_str(),
            self.adapter.event_store_id,
        )?;
        let decision = self.adapter.flow.project(
            &self.workspace.workspace_id,
            work_item_id.as_str(),
            input,
        )?;
        decorate_route_handoff(
            FlowSupervisor::projection(&decision),
            &self.state.value,
            current_review_input_fingerprint(self.workspace, &self.state.value)?,
        )
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
        let gates = self
            .adapter
            .gate_runtime(self.workspace, work_item_id.as_str())?;
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
            1,
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
        let before_bytes = fs::read(&self.state.absolute)
            .map_err(|error| io_error("read authoritative state", error))?;
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

    /// Authoritative `review.contribute` mutation. The registry keeps the
    /// operation lease-free for callers: reviewers serialize through the Work
    /// Item actor plus the revision/idempotency/fingerprint CAS, and the
    /// daemon acquires a short internal writer lease only for its own atomic
    /// append commit. No cross-reviewer writer lease is ever held.
    fn mutate_review_contribution(
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
            return Ok(OperationResponse {
                changed: false,
                revision_before: Some(committed.receipt.revision_before),
                revision_after: Some(committed.receipt.revision_after),
                receipt_digest: Some(committed.receipt.result_digest.into_array()),
                data,
            });
        }
        let before_bytes = fs::read(&self.state.absolute)
            .map_err(|error| io_error("read authoritative state", error))?;
        let authority = StateAuthority::inspect(&before_bytes).map_err(store_error)?;
        let state = authoritative_state_snapshot(&self.state.value, &before_bytes)?;
        if request.request().expected_revision != Some(authority.revision()) {
            return Err(RuntimeError::new(
                StableErrorCode::RevisionConflict,
                "expectedRevision does not match authoritative state",
            ));
        }
        let now = UtcTimestamp::now();
        let owner = LeaseOwner::new(format!(
            "review.contribute:{}",
            self.session_id
                .map_or_else(|| "daemon".to_owned(), |value| value.to_string())
        ))
        .map_err(store_error)?;
        let lease = store
            .acquire_lease(
                LeaseId::from_uuid(Uuid::new_v4()),
                owner,
                now.clone(),
                add_seconds_from(&now, REVIEW_CONTRIBUTE_LEASE_TTL_SECONDS)?,
            )
            .map_err(|error| match error {
                StoreError::LeaseConflict => RuntimeError::new(
                    StableErrorCode::ExecutionResourceBusy,
                    "an active writer lease blocks the review contribution",
                ),
                other => store_error(other),
            })?;
        match self.commit_review_contribution(
            &store,
            request,
            &state,
            authority,
            &before_bytes,
            &lease,
            work_item_id,
        ) {
            Ok(response) => {
                self.release_review_contribute_lease(&store, &lease, work_item_id)?;
                Ok(response)
            }
            Err(commit_error) => {
                // Best-effort release so a failed append does not pin the
                // writer lease; the commit failure stays the primary error.
                match self.release_review_contribute_lease(&store, &lease, work_item_id) {
                    Ok(()) => Err(commit_error),
                    Err(release_error) => Err(commit_error.with_remediation(format!(
                        "internal contribution lease release also failed: {}",
                        release_error.message()
                    ))),
                }
            }
        }
    }

    /// Prepares and commits one pending contribution append through the
    /// journaled mutation path under the internal writer lease.
    #[allow(clippy::too_many_arguments)]
    fn commit_review_contribution<C: CommitFaultPort>(
        &self,
        store: &ProjectMutationStore<
            StdDurableFileSystem,
            StdCrossProcessLock,
            SqliteRuntimeRepository,
            C,
        >,
        request: &ValidatedOperationRequest,
        state: &Value,
        authority: AuthoritySnapshot,
        before_bytes: &[u8],
        lease: &LeaseRecord,
        work_item_id: &WorkItemId,
    ) -> RuntimeResult<OperationResponse> {
        let caller = review_authority::AuthenticatedCaller::new(
            self.agent_id.clone().ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::RoleOperationForbidden,
                    "review.contribute requires a daemon-authenticated agentId",
                )
            })?,
            self.session_id.ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::RoleOperationForbidden,
                    "review.contribute requires a daemon-authenticated sessionId",
                )
            })?,
            self.workspace.agent_role.ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::RoleOperationForbidden,
                    "review.contribute requires a daemon-authenticated role",
                )
            })?,
        );
        let prepared = review_authority::prepare_review_contribution(
            self.workspace,
            state,
            work_item_id.as_str(),
            request,
            &caller,
            self.adapter.persistence.as_ref(),
            &self.adapter.boot_id.to_string(),
            &self.adapter.policy_digest,
            self.workspace.inventory_generation,
            &UtcTimestamp::now(),
        )?;
        let mut after = state.clone();
        let revision_after = authority
            .revision()
            .checked_next()
            .map_err(|_| schema_error("state revision overflow"))?;
        let object = after
            .as_object_mut()
            .ok_or_else(|| schema_error("authoritative state must be an object"))?;
        object.insert("review".to_owned(), prepared.review.clone());
        object.remove("reviewLoop");
        object.insert("reviewSession".to_owned(), prepared.review_session.clone());
        object.insert(
            "inputFingerprint".to_owned(),
            Value::String(prepared.input_fingerprint.clone()),
        );
        object.insert(
            "rulesetFingerprint".to_owned(),
            Value::String(prepared.ruleset_fingerprint.clone()),
        );
        object.insert(
            "policyDigest".to_owned(),
            Value::String(self.adapter.policy_digest.clone()),
        );
        object.insert(
            "inventoryGeneration".to_owned(),
            Value::from(self.workspace.inventory_generation),
        );
        object.insert("revision".to_owned(), Value::from(revision_after.get()));
        object.insert(
            "lastFencingToken".to_owned(),
            Value::from(lease.fencing_token().get()),
        );
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
        let response_data = prepared.review.clone();
        let event_bytes = serde_json::to_vec(&json!({
            "operation": request.operation().as_str(),
            "data": response_data,
        }))
        .map_err(|_| schema_error("event could not be serialized"))?;
        let result_bytes = serde_json::to_vec(&response_data)
            .map_err(|_| schema_error("operation result could not be serialized"))?;
        let mutation = MutationRequest {
            mutation_id: RequestId::from_uuid(Uuid::new_v4()),
            workspace_id: parse(&self.workspace.workspace_id, "workspaceId")?,
            work_item_id: work_item_id.clone(),
            operation: request.operation_id().clone(),
            idempotency_key: IdempotencyKey::new(
                request
                    .request()
                    .idempotency_key
                    .as_deref()
                    .ok_or_else(|| schema_error("idempotencyKey is required"))?
                    .to_owned(),
            )
            .map_err(store_error)?,
            canonical_payload_digest: InputFingerprint::from_array(*request.payload_digest()),
            expected_authority: authority,
            lease_proof: LeaseProof::from(lease),
            targets: vec![
                MutationTarget::new(
                    self.state.relative.clone(),
                    Some(ArtifactDigest::digest(before_bytes)),
                    after_bytes,
                )
                .map_err(store_error)?,
            ],
            event: JournalEvent {
                boot_id: self.adapter.boot_id,
                session_id: self.session_id,
                event_type: request.operation().as_str().to_owned().into_boxed_str(),
                schema_version: 1,
                payload: RuntimeEventPayload::InlineJson(event_bytes),
            },
            result_digest: ResultDigest::digest(&result_bytes),
            prepared_at: UtcTimestamp::now(),
            committed_at: UtcTimestamp::now(),
        };
        if request.request().dry_run {
            store.validate_mutation(&mutation).map_err(store_error)?;
            let target_paths = mutation
                .targets
                .iter()
                .map(|target| target.path().as_str().to_owned())
                .collect::<Vec<_>>();
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
        if !committed.replayed {
            self.adapter.invalidate_gate_selectors(
                self.workspace,
                work_item_id.as_str(),
                &[GateInputSelector::ReviewBatch],
            );
        }
        Ok(OperationResponse {
            changed: !committed.replayed,
            revision_before: Some(committed.receipt.revision_before),
            revision_after: Some(committed.receipt.revision_after),
            receipt_digest: Some(committed.receipt.result_digest.into_array()),
            data: response_data,
        })
    }

    /// Releases the internal contribution lease through the journaled
    /// lease-control path so later writers are never blocked by a finished
    /// append.
    fn release_review_contribute_lease<C: CommitFaultPort>(
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
        .map_err(|_| schema_error("contribution lease release could not be canonicalized"))?;
        let control = LeaseControlRequest {
            mutation_id: RequestId::from_uuid(Uuid::new_v4()),
            workspace_id: parse(&self.workspace.workspace_id, "workspaceId")?,
            work_item_id: work_item_id.clone(),
            operation: parse(OperationName::LeaseRelease.as_str(), "operation")?,
            idempotency_key: IdempotencyKey::new(format!(
                "review.contribute.release:{}",
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
        let input = flow_input(
            self.workspace,
            state,
            work_item_id,
            self.adapter.event_store_id,
        )?;
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
    ) -> RuntimeResult<Option<(FlowInput, ProcessPhase, Vec<RequiredGate>)>> {
        let Some(target) = target else {
            return Ok(None);
        };
        let work_item_id = request
            .request()
            .work_item_id
            .as_ref()
            .ok_or_else(|| schema_error("workItemId is required"))?;
        let input = flow_input(
            self.workspace,
            state,
            work_item_id.as_str(),
            self.adapter.event_store_id,
        )?;
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
        let gates = self
            .adapter
            .gate_runtime(self.workspace, work_item_id.as_str())?;
        let count = u64::try_from(decision.required_gates().len()).unwrap_or(u64::MAX);
        let per_gate_ms = self
            .deadline_ms
            .checked_div(count.max(1))
            .unwrap_or(1)
            .max(1);
        for required in decision.required_gates() {
            let evaluated =
                gates.evaluate(required.as_str(), Duration::from_millis(per_gate_ms))?;
            if evaluated.key().fencing_token().get() != fencing.get() {
                return Err(RuntimeError::new(
                    StableErrorCode::StaleFencingToken,
                    "Gate snapshot fencing token is no longer authoritative",
                ));
            }
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
        Ok(Some((
            input,
            target,
            decision.passed_gates().iter().copied().collect(),
        )))
    }

    /// Advances the recorded completion milestone inside the post-image state
    /// when the committed operation proves the next stage: a green
    /// verification evidence record closes `ImplementationVerified`, evidence
    /// finalization closes `ReviewReady`, and a Review aggregation closes
    /// `GovernanceClosed`. The advance starts from the *effective* milestone
    /// (the recorded value rolled back against currently observed digests),
    /// so a stale input is never carried forward by a later operation. Work
    /// Items without an approved execution runtime never grow a milestone.
    fn advance_completion_milestone(
        &self,
        before: &Value,
        after: &mut Value,
        request: &ValidatedOperationRequest,
        semantic: Option<&PreparedSemanticMutation>,
    ) -> RuntimeResult<()> {
        let operation = request.operation();
        let governed = matches!(
            operation,
            OperationName::EvidenceRecord
                | OperationName::EvidenceFinalize
                | OperationName::ReviewRecord
                | OperationName::ReviewFinalize
        );
        if !governed || before.get("executionRuntime").is_none() {
            return Ok(());
        }
        if operation == OperationName::EvidenceRecord
            && request
                .request()
                .payload
                .get("exitCode")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                != 0
        {
            // A red verification never closes ImplementationVerified.
            return Ok(());
        }
        let work_item_id = request
            .request()
            .work_item_id
            .as_ref()
            .ok_or_else(|| schema_error("workItemId is required"))?;
        let Some((recorded, bound)) = completion_milestone_from_state(before)? else {
            return Ok(());
        };
        let observed = observed_completion_digests(self.workspace, before, work_item_id.as_str())?;
        let effective = recorded.invalidate(&bound, &observed);
        let root = Path::new(&self.workspace.canonical_root);
        let (next, code, verification, evidence, review_input, gate) =
            match (operation, effective) {
                (OperationName::EvidenceRecord, _) => {
                    let verification = semantic
                        .and_then(|prepared| {
                            prepared.targets.iter().find(|target| {
                                target.relative_path.ends_with("evidence/manifest.json")
                            })
                        })
                        .map(|target| verification_digest_from_manifest_bytes(&target.after_bytes))
                        .transpose()?
                        .ok_or_else(|| {
                            schema_error("evidence record did not prepare a sealed manifest")
                        })?;
                    (
                        CompletionMilestone::ImplementationVerified,
                        Some(code_digest(root, after)?),
                        Some(verification),
                        None,
                        None,
                        None,
                    )
                }
                (OperationName::EvidenceFinalize, CompletionMilestone::ImplementationVerified) => (
                    CompletionMilestone::ReviewReady,
                    None,
                    None,
                    Some(evidence_authority_digest(after)),
                    None,
                    None,
                ),
                (
                    OperationName::ReviewRecord | OperationName::ReviewFinalize,
                    CompletionMilestone::ReviewReady,
                ) => {
                    // Governance closes only on a terminal clean Review: an
                    // intermediate non-clean aggregation (for example the first
                    // specialty of a multi-reviewer tier) leaves the milestone at
                    // ReviewReady.
                    let terminal_clean = after
                        .pointer("/review/batch/latestStatus")
                        .and_then(Value::as_str)
                        == Some("VALID_CLEAN")
                        && after
                            .pointer("/reviewSession/status")
                            .and_then(Value::as_str)
                            == Some("completed");
                    if !terminal_clean {
                        return Ok(());
                    }
                    (
                        CompletionMilestone::GovernanceClosed,
                        None,
                        None,
                        None,
                        Some(review_binding_digest(after)),
                        Some(completion_gate_digest(after)),
                    )
                }
                _ => return Ok(()),
            };
        let runtime_object = after
            .get_mut("executionRuntime")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| schema_error("executionRuntime section is malformed"))?;
        runtime_object.insert(
            "completionMilestone".to_owned(),
            Value::String(completion_milestone_wire(next).to_owned()),
        );
        runtime_object.insert(
            "completionBound".to_owned(),
            json!({
                "codeDigest": digest_wire(code.unwrap_or(bound.code_digest())),
                "verificationDigest": digest_wire(verification.unwrap_or(bound.verification_digest())),
                "evidenceDigest": digest_wire(evidence.unwrap_or(bound.evidence_digest())),
                "reviewInputDigest": digest_wire(review_input.unwrap_or(bound.review_input_digest())),
                "gateDigest": digest_wire(gate.unwrap_or(bound.gate_digest())),
            }),
        );
        Ok(())
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

    /// Returns the newest committed review aggregation event sequence for the
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
                let review_aggregation = event.kind == OperationName::ReviewRecord.as_str()
                    || event.kind == OperationName::ReviewFinalize.as_str();
                if review_aggregation
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
            if matches!(
                request.operation(),
                OperationName::ReviewRecord | OperationName::ReviewFinalize
            ) {
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
        if !request.request().dry_run
            && matches!(
                request.operation(),
                OperationName::ReviewRecord | OperationName::ReviewFinalize
            )
        {
            self.repair_review_projections(work_item_id.as_str())?;
        }
        let before_bytes = fs::read(&self.state.absolute)
            .map_err(|error| io_error("read authoritative state", error))?;
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
        let transition = self.prepare_transition_commit(
            &state,
            request,
            lifecycle_target(request.operation(), &request.request().payload)?,
        )?;
        let passed_gates = transition
            .as_ref()
            .map_or(&[][..], |(_, _, gates)| gates.as_slice());
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
                passed_gates,
            },
        )?;
        let mut after = state.clone();
        apply_mutation(&mut after, request, semantic.as_ref())?;
        self.advance_completion_milestone(&state, &mut after, request, semantic.as_ref())?;
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
        // Review aggregation events carry a bounded replay seed so the durable
        // SQLite projection can be rebuilt from committed events alone. `data`
        // already holds the batch/attempt/receipt; only the typed session is
        // additionally required to reconstruct the complete tuple.
        let event_value = match semantic
            .as_ref()
            .and_then(|prepared| prepared.review_session.as_ref())
        {
            Some(session)
                if matches!(
                    request.operation(),
                    OperationName::ReviewRecord | OperationName::ReviewFinalize
                ) =>
            {
                json!({
                    "operation": request.operation().as_str(),
                    "data": response_data,
                    "reviewProjection": {"reviewSession": session},
                })
            }
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
        if !committed.replayed {
            self.adapter.invalidate_gate_selectors(
                self.workspace,
                work_item_id.as_str(),
                mutation_gate_selectors(request.operation()),
            );
        }
        // A review aggregation response must never succeed without its durable
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
        if let Some((input, target, _)) = transition {
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

/// Bounded page size for durable review aggregation event scans.
const REVIEW_EVENT_PAGE: usize = 256;

/// Reconstructs one Review Batch v2 projection write from a committed review
/// aggregation (`review.record` or `review.finalize`) event payload.
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
    let review_aggregation = event.kind == OperationName::ReviewRecord.as_str()
        || event.kind == OperationName::ReviewFinalize.as_str();
    if !review_aggregation
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

/// Entry nodes whose container layout the nested state model defines.
///
/// `BUG` and `CONFIG` deliberately do not appear: they run the micro chain on a
/// flat state, a different shape this function does not build. Accepting them
/// here would write a nested skeleton that no reader expects.
const NESTED_ENTRY_CONTAINERS: [(&str, &[&str]); 4] = [
    ("ROUTE", &[]),
    ("PRD", &["prdState", "drStates", "storyStates"]),
    ("DR", &["drState", "storyStates"]),
    ("STORY", &["storyStates"]),
];

/// Creates one Work Item state, exactly once.
///
/// The guard is exclusive-create on `state.json` rather than a lease: there is
/// no Work Item to lease yet. A retried request with the same caller-supplied
/// `workItemId` therefore fails closed instead of producing a second
/// directory, and a rejected payload leaves nothing behind because the
/// directory is only made after every field validates.
///
/// The business name may come from the caller or be minted here: a bootstrap
/// caller has no Work Item yet, so it cannot know a free name in advance and
/// the registry deliberately does not require `workItemId` for
/// `workitem.create`. A minted name has to satisfy the same `WorkItemId`
/// charset rules and the same existing-directory screen as a chosen one, or
/// the daemon would create a state no later operation could address.
fn create_work_item(
    workspace: &BusinessWorkspace,
    request: &OperationRequest,
) -> RuntimeResult<Value> {
    // This path bypasses the OperationService, so the registry field contract
    // has to be applied here or an unknown/mistyped field would slip through.
    validate_operation_payload(OperationName::WorkItemCreate, &request.payload)
        .map_err(|error| schema_error(&error.to_string()))?;
    let payload = request.payload.clone();
    let entry_node = payload
        .get("entryNode")
        .and_then(Value::as_str)
        .map(str::trim)
        .ok_or_else(|| schema_error("entryNode must be non-empty text"))?
        .to_ascii_uppercase();
    let containers = NESTED_ENTRY_CONTAINERS
        .iter()
        .find(|(node, _)| *node == entry_node)
        .map(|(_, containers)| *containers)
        .ok_or_else(|| {
            schema_error(
                "entryNode must be ROUTE, PRD, DR, or STORY; BUG and CONFIG run the flat micro chain",
            )
        })?;
    let root = Path::new(&workspace.canonical_root).join(".auto-engineering");
    let chosen_work_item = request
        .work_item_id
        .as_ref()
        .map(|value| value.as_str().trim())
        .filter(|value| !value.is_empty());
    let bootstrap_identity = match chosen_work_item {
        Some(_) => None,
        None => Some(anonymous_create_identity(workspace, request, &entry_node)?),
    };
    let work_item_id = chosen_work_item.map_or_else(
        || {
            bootstrap_identity
                .as_ref()
                .expect("anonymous create identity was prepared")
                .work_item_id
                .clone()
        },
        str::to_owned,
    );
    // A path separator or traversal segment in the id would place the directory
    // outside `.auto-engineering/`, so reject it before touching the filesystem.
    if work_item_id.contains('/') || work_item_id.contains('\\') || work_item_id.contains("..") {
        return Err(schema_error(
            "workItemId must not contain a path separator or traversal segment",
        ));
    }
    let story_name = payload
        .get("storyName")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(identity) = bootstrap_identity.as_ref()
        && let Some(existing) = find_anonymous_create(&root, &identity.idempotency_key_digest)?
    {
        return replay_anonymous_create(&existing, identity);
    }

    // A pre-existing Work Item of the same business name must not gain a second
    // directory: `read_state` resolves by name and would then fail ambiguous.
    // Chosen and daemon-derived names share one collision screen. Anonymous
    // retries are the only exception: their deterministic origin metadata
    // proves whether the existing state is the original committed result.
    if existing_state_directory(&root, &work_item_id)? {
        if let Some(identity) = bootstrap_identity.as_ref() {
            let located = read_state(workspace, &work_item_id)?;
            return replay_anonymous_create(&located.value, identity);
        }
        return Err(RuntimeError::new(
            StableErrorCode::ScopeAmbiguous,
            "a Work Item with this name already exists",
        ));
    }

    let state_uuid = bootstrap_identity.as_ref().map_or_else(
        || Uuid::new_v4().to_string(),
        |identity| identity.state_uuid.clone(),
    );
    let state_machine_id = format!("{state_uuid}-{work_item_id}");
    let now = UtcTimestamp::now().to_string();
    let mut state = Map::new();
    state.insert("version".to_owned(), json!("2"));
    state.insert(
        "projectKey".to_owned(),
        json!(workspace.project_key.as_str()),
    );
    state.insert("stateModel".to_owned(), json!("nested"));
    state.insert("processPolicy".to_owned(), json!("compact"));
    state.insert("entryNode".to_owned(), json!(entry_node));
    match entry_node.as_str() {
        "PRD" | "DR" => {
            state.insert("scale".to_owned(), json!("large"));
            state.insert("selectedDesign".to_owned(), json!("DR"));
        }
        "STORY" => {
            state.insert("scale".to_owned(), json!("medium"));
            state.insert("selectedDesign".to_owned(), json!("STORY"));
        }
        "ROUTE" => {}
        _ => unreachable!("entry node was validated above"),
    }
    // The flow authority and the session context projection both derive their
    // snapshot from the Work Item phase, and both read a created state on the
    // very next call — the bootstrap Hook right after `workitem.create`. A
    // fresh item sits at the start of its lifecycle, so the state opens at
    // `initialized`, the same phase the prd/dr containers below already use;
    // without it the create would succeed and every later read would fail.
    state.insert("phase".to_owned(), json!("initialized"));
    state.insert("currentPhase".to_owned(), json!("initialized"));
    state.insert("stateMachineId".to_owned(), json!(state_machine_id));
    state.insert("stateMachineName".to_owned(), json!(work_item_id));
    state.insert("stateUuid".to_owned(), json!(state_uuid));
    state.insert("parentPrdId".to_owned(), Value::Null);
    state.insert("parentDrId".to_owned(), Value::Null);
    state.insert("activeStory".to_owned(), Value::Null);
    state.insert("activeTask".to_owned(), Value::Null);
    state.insert("routeDocuments".to_owned(), json!({}));
    state.insert(
        "documentPaths".to_owned(),
        json!({
            "RA":format!("ae-sdd-doc/RA/{work_item_id}.md"),
            "DR":format!("ae-sdd-doc/DR/{work_item_id}.md"),
            "STORY":format!("ae-sdd-doc/Story/{work_item_id}.md"),
            "CODING_PLAN":format!("ae-sdd-doc/Coding/{work_item_id}/{work_item_id}-CodingPlan.md"),
        }),
    );
    state.insert("history".to_owned(), json!([]));
    state.insert("events".to_owned(), json!([]));
    state.insert("createdAt".to_owned(), json!(now));
    state.insert("lastUpdated".to_owned(), json!(now));
    if let Some(identity) = bootstrap_identity.as_ref() {
        state.insert(
            "bootstrapCreate".to_owned(),
            json!({
                "idempotencyKeyDigest":identity.idempotency_key_digest,
                "requestDigest":identity.request_digest,
            }),
        );
    }
    // `StateAuthority::inspect` reads both on every open and rejects the file if
    // either is absent, so a freshly created state has to carry them at zero.
    // The Python creator derived `revision` instead of storing it, which is why
    // they are absent from the constructor this shape was taken from.
    state.insert("revision".to_owned(), json!(0));
    state.insert("lastFencingToken".to_owned(), json!(0));
    state.insert(
        "executionPlan".to_owned(),
        json!({"goal":"","changedPaths":[],"verification":[],"risks":[],
               "sourceReads":[],"approved":false,"approvedAt":null,"approvedBy":null}),
    );
    state.insert(
        "review".to_owned(),
        json!({"status":"pending","findings":[],"reviewedPaths":[],
               "evidenceIds":[],"updatedAt":null}),
    );
    for container in containers {
        let value = match *container {
            "prdState" => json!({"prdId":work_item_id,"phase":"initialized",
                                 "completedSteps":[],"lastUpdated":now}),
            "drState" => json!({"drId":work_item_id,"phase":"initialized","docPath":null,
                                "completedSteps":[],"lastUpdated":now}),
            _ => json!({}),
        };
        state.insert((*container).to_owned(), value);
    }
    if let Some(story_name) = story_name {
        state.insert("storyName".to_owned(), json!(story_name));
    }
    let provided = provided_documents(workspace, &payload)?;
    adopt_provided_documents(&mut state, &entry_node, &provided, &now)?;

    let directory = root.join(&state_machine_id);
    fs::create_dir_all(&directory).map_err(|error| io_error("create state directory", error))?;
    let path = directory.join("state.json");
    let body = serde_json::to_vec_pretty(&Value::Object(state.clone()))
        .map_err(|_| schema_error("initial state could not be encoded"))?;
    // `create_new` is the exclusive-create guard: a racing caller that already
    // wrote this path loses instead of silently overwriting a live Work Item.
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(&body)
                .map_err(|error| io_error("write authoritative state", error))?;
            file.sync_all()
                .map_err(|error| io_error("write authoritative state", error))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if let Some(identity) = bootstrap_identity.as_ref() {
                let bytes =
                    fs::read(&path).map_err(|error| io_error("read authoritative state", error))?;
                let value = serde_json::from_slice::<Value>(&bytes)
                    .map_err(|_| schema_error("authoritative state JSON is malformed"))?;
                return replay_anonymous_create(&value, identity);
            }
            return Err(RuntimeError::new(
                StableErrorCode::ScopeAmbiguous,
                "state.json already exists and exclusive create will not overwrite it",
            ));
        }
        Err(error) => return Err(io_error("write authoritative state", error)),
    }
    // `workItemId` reports the business name, not the directory identity:
    // `read_state` and every later operation resolve by `stateMachineName`,
    // so the uuid-prefixed id would be a key no caller could use downstream.
    // The directory identity stays available as `stateMachineId`.
    Ok(json!({
        "changed": true,
        "revisionBefore": Value::Null,
        "revisionAfter": Value::Null,
        "receiptDigest": Value::Null,
        "data": {"workItemId": work_item_id, "stateMachineId": state_machine_id,
                 "stateMachineName": work_item_id,
                 "stateUuid": state_uuid, "entryNode": entry_node,
                 "statePath": format!(".auto-engineering/{state_machine_id}/state.json")},
    }))
}

/// A caller-owned document a `workitem.create` payload registers for adoption.
///
/// Adoption only records the mapping in the new state: the file is never
/// opened for content, copied, or written — the canonicalize below is the
/// metadata-only containment proof the path contract requires.
struct ProvidedDocument {
    intent: ProvidedDocumentIntent,
    doc_id: String,
    path: ProjectRelativePath,
    parent_doc_id: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProvidedDocumentIntent {
    Prd,
    Dr,
    Story,
}

impl ProvidedDocumentIntent {
    const fn document_paths_key(self) -> &'static str {
        match self {
            Self::Prd => "PRD",
            Self::Dr => "DR",
            Self::Story => "STORY",
        }
    }
}

/// Parses and screens the optional `providedDocuments` adoption tree.
///
/// The registry request layer already rejected malformed entries; what remains
/// needs the workspace root: docId charset for the container that will key it,
/// project-relative path form (the same traversal screen document.save uses),
/// and proof that the path names an existing file inside this workspace.
fn provided_documents(
    workspace: &BusinessWorkspace,
    payload: &Value,
) -> RuntimeResult<Vec<ProvidedDocument>> {
    let Some(documents) = payload.get("providedDocuments") else {
        return Ok(Vec::new());
    };
    let entries = documents
        .as_array()
        .ok_or_else(|| schema_error("providedDocuments must be an array"))?;
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    let root = Path::new(&workspace.canonical_root);
    let canonical_root = root
        .canonicalize()
        .map_err(|error| io_error("resolve workspace root", error))?;
    let mut provided = Vec::with_capacity(entries.len());
    for entry in entries {
        let intent = match entry.get("intent").and_then(Value::as_str) {
            Some("PRD") => ProvidedDocumentIntent::Prd,
            Some("DR") => ProvidedDocumentIntent::Dr,
            Some("STORY") => ProvidedDocumentIntent::Story,
            _ => {
                return Err(schema_error(
                    "providedDocuments.intent must be PRD, DR, or STORY",
                ));
            }
        };
        let doc_id = entry
            .get("docId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        // The docId becomes a container key (`prdState.prdId`, `drStates` key
        // or `storyStates` key), and the lifecycle authority fail-closes on an
        // id it cannot parse, so screen it before it reaches the state file.
        let id_is_valid = match intent {
            ProvidedDocumentIntent::Prd => PrdId::new(doc_id.clone()).is_ok(),
            ProvidedDocumentIntent::Dr => WorkItemId::new(doc_id.clone()).is_ok(),
            ProvidedDocumentIntent::Story => StoryId::new(doc_id.clone()).is_ok(),
        };
        if !id_is_valid {
            return Err(schema_error(
                "providedDocuments.docId is not a valid document identifier",
            ));
        }
        let path = entry
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("providedDocuments.path must be non-empty text"))?;
        let path = ProjectRelativePath::new(path.to_owned()).map_err(domain_error)?;
        let candidate = root.join(path.as_str());
        let canonical = candidate.canonicalize().map_err(|_| {
            schema_error("providedDocuments.path must name an existing workspace file")
        })?;
        if !canonical.starts_with(&canonical_root) || !canonical.is_file() {
            return Err(schema_error(
                "providedDocuments.path must stay inside the workspace and name a file",
            ));
        }
        let parent_doc_id = entry
            .get("parentDocId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        provided.push(ProvidedDocument {
            intent,
            doc_id,
            path,
            parent_doc_id,
        });
    }
    Ok(provided)
}

/// Registers caller-provided documents in the freshly built state.
///
/// Adoption only records mappings: `documentPaths` takes the first provided
/// path per intent (unprovided intents keep their minted defaults),
/// `routeDocuments` marks each adopted series committed so a ROUTE handoff
/// skips its generation, and the prd/dr/story containers gain their entries
/// with generation-complete phases. The root phase jumps directly to the
/// deepest adopted document's post-generation phase because the
/// TransitionPolicy only ever advances one step; this write is the create-time
/// initial value, not a transition.
fn adopt_provided_documents(
    state: &mut Map<String, Value>,
    entry_node: &str,
    provided: &[ProvidedDocument],
    now: &str,
) -> RuntimeResult<()> {
    if provided.is_empty() {
        return Ok(());
    }
    let has = |intent| provided.iter().any(|doc| doc.intent == intent);
    // Deepest adopted document decides the initial phase. `dr-generated` is
    // not a member of the STORY route chain, so a STORY-entry item that only
    // adopts a DR falls back to the chain's deepest legal pre-Story phase.
    let root_phase = if has(ProvidedDocumentIntent::Story) {
        if entry_node == "STORY" {
            "story-generated"
        } else {
            "dr-generated"
        }
    } else if has(ProvidedDocumentIntent::Dr) {
        if entry_node == "STORY" {
            "requirement-analyzed"
        } else {
            "dr-generated"
        }
    } else {
        "initialized"
    };

    let paths = state
        .get_mut("documentPaths")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| schema_error("documentPaths must be an object"))?;
    for intent in [
        ProvidedDocumentIntent::Prd,
        ProvidedDocumentIntent::Dr,
        ProvidedDocumentIntent::Story,
    ] {
        if let Some(doc) = provided.iter().find(|doc| doc.intent == intent) {
            paths.insert(
                intent.document_paths_key().to_owned(),
                json!(doc.path.as_str()),
            );
        }
    }
    let routes = state
        .get_mut("routeDocuments")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| schema_error("routeDocuments must be an object"))?;
    for doc in provided {
        routes.insert(
            doc.intent.document_paths_key().to_owned(),
            Value::Bool(true),
        );
        // The requirement-analysis series exists to produce the PRD, so an
        // adopted PRD also commits the series key the handoff actually reads.
        if doc.intent == ProvidedDocumentIntent::Prd {
            routes.insert("RA".to_owned(), Value::Bool(true));
        }
    }

    let singular_dr_id = state
        .get("drState")
        .and_then(|dr| dr.get("drId"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    if let Some(prd) = provided
        .iter()
        .find(|doc| doc.intent == ProvidedDocumentIntent::Prd)
    {
        match state.get_mut("prdState") {
            Some(existing) => {
                // The existing container belongs to the Work Item itself:
                // `prdId` must stay the business name or the lifecycle
                // authority can no longer address the item, so the adopted
                // document identity rides in `docId` alongside it.
                let existing = existing
                    .as_object_mut()
                    .ok_or_else(|| schema_error("prdState must be an object"))?;
                existing.insert("docId".to_owned(), json!(prd.doc_id));
                existing.insert("docPath".to_owned(), json!(prd.path.as_str()));
            }
            None => {
                state.insert(
                    "prdState".to_owned(),
                    json!({
                        "prdId": prd.doc_id,
                        "docId": prd.doc_id,
                        "phase": root_phase,
                        "docPath": prd.path.as_str(),
                        "completedSteps": [],
                        "lastUpdated": now,
                    }),
                );
            }
        }
    }
    for doc in provided
        .iter()
        .filter(|doc| doc.intent == ProvidedDocumentIntent::Dr)
    {
        if entry_node == "DR" && singular_dr_id.as_deref() == Some(doc.doc_id.as_str()) {
            let dr = state
                .get_mut("drState")
                .and_then(Value::as_object_mut)
                .ok_or_else(|| schema_error("drState must be an object"))?;
            dr.insert("docPath".to_owned(), json!(doc.path.as_str()));
        } else {
            let dr_states = state
                .entry("drStates")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or_else(|| schema_error("drStates must be an object"))?;
            dr_states.insert(
                doc.doc_id.clone(),
                json!({
                    "drId": doc.doc_id,
                    "phase": "dr-generated",
                    "docPath": doc.path.as_str(),
                    "completedSteps": [],
                    "lastUpdated": now,
                    "storyStates": {},
                }),
            );
        }
    }
    for doc in provided
        .iter()
        .filter(|doc| doc.intent == ProvidedDocumentIntent::Story)
    {
        let story = json!({
            "phase": "story-generated",
            "currentPhase": "story-generated",
            "docPath": doc.path.as_str(),
        });
        let nested_in_singular = entry_node == "DR"
            && doc.parent_doc_id.as_deref() == singular_dr_id.as_deref()
            && doc.parent_doc_id.is_some();
        if nested_in_singular {
            let dr = state
                .get_mut("drState")
                .and_then(Value::as_object_mut)
                .ok_or_else(|| schema_error("drState must be an object"))?;
            dr.entry("storyStates")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or_else(|| schema_error("drState.storyStates must be an object"))?
                .insert(doc.doc_id.clone(), story);
        } else if let Some(parent) = doc.parent_doc_id.as_deref() {
            // The request layer proved the parent is a provided DR, and the DR
            // loop above registered every provided DR, so this entry exists.
            state
                .get_mut("drStates")
                .and_then(Value::as_object_mut)
                .and_then(|states| states.get_mut(parent))
                .and_then(Value::as_object_mut)
                .ok_or_else(|| {
                    schema_error("providedDocuments.parentDocId has no registered DR entry")
                })?
                .get_mut("storyStates")
                .and_then(Value::as_object_mut)
                .ok_or_else(|| schema_error("drStates.storyStates must be an object"))?
                .insert(doc.doc_id.clone(), story);
        } else {
            state
                .entry("storyStates")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or_else(|| schema_error("storyStates must be an object"))?
                .insert(doc.doc_id.clone(), story);
        }
    }

    if entry_node == "DR"
        && let Some(prd) = provided
            .iter()
            .find(|doc| doc.intent == ProvidedDocumentIntent::Prd)
    {
        state.insert("parentPrdId".to_owned(), json!(prd.doc_id));
    }
    if entry_node == "STORY"
        && let Some(dr) = provided
            .iter()
            .find(|doc| doc.intent == ProvidedDocumentIntent::Dr)
    {
        state.insert("parentDrId".to_owned(), json!(dr.doc_id));
    }

    state.insert("phase".to_owned(), json!(root_phase));
    state.insert("currentPhase".to_owned(), json!(root_phase));
    // Container phases mirror the root: the lifecycle authority fail-closes
    // when prdState disagrees with the top-level phase, and the singular
    // drState is kept on the same rule so the two views cannot diverge.
    if let Some(prd) = state.get_mut("prdState").and_then(Value::as_object_mut) {
        prd.insert("phase".to_owned(), json!(root_phase));
    }
    if let Some(dr) = state.get_mut("drState").and_then(Value::as_object_mut) {
        dr.insert("phase".to_owned(), json!(root_phase));
    }
    Ok(())
}

/// Derives the PRD → DR → Story document tree from the authoritative
/// containers. This is a read-time projection: it is never persisted, so the
/// state file stays the single owner of the mapping.
fn derive_document_tree(state: &Value) -> Value {
    let document_paths = state.get("documentPaths").and_then(Value::as_object);
    let bound_path = |key: &str| {
        document_paths
            .and_then(|paths| paths.get(key))
            .and_then(Value::as_str)
    };
    let story_node = |story_id: &str, story: &Value| {
        json!({
            "storyId": story_id,
            "docPath": story.get("docPath").cloned().unwrap_or(Value::Null),
            "phase": story.get("phase").cloned().unwrap_or(Value::Null),
        })
    };
    let nested_stories = |dr: Option<&Value>| {
        dr.and_then(|dr| dr.get("storyStates"))
            .and_then(Value::as_object)
            .map(|stories| {
                stories
                    .iter()
                    .map(|(story_id, story)| story_node(story_id, story))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let prd = state
        .get("prdState")
        .and_then(Value::as_object)
        .map(|prd| {
            let doc_path = prd
                .get("docPath")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| bound_path("PRD").map(str::to_owned))
                .or_else(|| bound_path("RA").map(str::to_owned));
            json!({
                "docId": prd.get("docId").and_then(Value::as_str)
                    .or_else(|| prd.get("prdId").and_then(Value::as_str)),
                "docPath": doc_path,
                "phase": prd.get("phase").cloned().unwrap_or(Value::Null),
            })
        });
    let mut drs = Vec::new();
    if let Some(dr) = state.get("drState").and_then(Value::as_object) {
        let doc_path = dr
            .get("docPath")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| bound_path("DR").map(str::to_owned));
        drs.push(json!({
            "drId": dr.get("drId").cloned().unwrap_or(Value::Null),
            "docPath": doc_path,
            "phase": dr.get("phase").cloned().unwrap_or(Value::Null),
            "stories": nested_stories(state.get("drState")),
        }));
    }
    if let Some(states) = state.get("drStates").and_then(Value::as_object) {
        for (dr_id, dr) in states {
            drs.push(json!({
                "drId": dr_id,
                "docPath": dr.get("docPath").cloned().unwrap_or(Value::Null),
                "phase": dr.get("phase").cloned().unwrap_or(Value::Null),
                "stories": nested_stories(Some(dr)),
            }));
        }
    }
    drs.sort_by(|left, right| {
        left.get("drId")
            .and_then(Value::as_str)
            .cmp(&right.get("drId").and_then(Value::as_str))
    });
    let stories = state
        .get("storyStates")
        .and_then(Value::as_object)
        .map(|stories| {
            stories
                .iter()
                .map(|(story_id, story)| story_node(story_id, story))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "prd": prd,
        "drs": drs,
        "stories": stories,
    })
}

/// Attaches the derived document tree to a flow projection value.
fn with_document_tree(mut projection: Value, state: &Value) -> Value {
    if let Some(object) = projection.as_object_mut() {
        object.insert("documentTree".to_owned(), derive_document_tree(state));
    }
    projection
}

struct AnonymousCreateIdentity {
    work_item_id: String,
    state_uuid: String,
    idempotency_key_digest: String,
    request_digest: String,
}

fn anonymous_create_identity(
    workspace: &BusinessWorkspace,
    request: &OperationRequest,
    entry_node: &str,
) -> RuntimeResult<AnonymousCreateIdentity> {
    let idempotency_key = request
        .idempotency_key
        .as_deref()
        .ok_or_else(|| schema_error("idempotencyKey is required for anonymous workitem.create"))?;
    let identity =
        ArtifactDigest::digest(format!("{}\0{idempotency_key}", workspace.workspace_id).as_bytes());
    let identity_hex = identity.to_string();
    let mut uuid_bytes = [0_u8; 16];
    uuid_bytes.copy_from_slice(&identity.as_bytes()[..16]);
    Ok(AnonymousCreateIdentity {
        work_item_id: format!("{entry_node}-{}", &identity_hex[..8]),
        state_uuid: Uuid::from_bytes(uuid_bytes).to_string(),
        idempotency_key_digest: ArtifactDigest::digest(idempotency_key.as_bytes()).to_string(),
        request_digest: ArtifactDigest::digest(
            serde_json::to_vec(&canonical_value(&json!({
                "operation":"workitem.create",
                "payload":request.payload,
            })))
            .map_err(|_| schema_error("anonymous create request could not be canonicalized"))?,
        )
        .to_string(),
    })
}

fn replay_anonymous_create(
    state: &Value,
    identity: &AnonymousCreateIdentity,
) -> RuntimeResult<Value> {
    let bootstrap = state.get("bootstrapCreate").and_then(Value::as_object);
    let key_matches = bootstrap
        .and_then(|value| value.get("idempotencyKeyDigest"))
        .and_then(Value::as_str)
        == Some(identity.idempotency_key_digest.as_str());
    if !key_matches {
        return Err(RuntimeError::new(
            StableErrorCode::ScopeAmbiguous,
            "the deterministic bootstrap Work Item is owned by another origin",
        ));
    }
    let request_matches = bootstrap
        .and_then(|value| value.get("requestDigest"))
        .and_then(Value::as_str)
        == Some(identity.request_digest.as_str());
    if !request_matches {
        return Err(RuntimeError::new(
            StableErrorCode::IdempotencyKeyReused,
            "anonymous workitem.create idempotency key is bound to another payload",
        ));
    }
    created_work_item_response(state)
}

fn find_anonymous_create(root: &Path, key_digest: &str) -> RuntimeResult<Option<Value>> {
    if !root.is_dir() {
        return Ok(None);
    }
    let mut matched = None;
    for entry in fs::read_dir(root).map_err(|error| io_error("list state directories", error))? {
        let path = entry
            .map_err(|error| io_error("list state directories", error))?
            .path()
            .join("state.json");
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(path).map_err(|error| io_error("read state file", error))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| schema_error("authoritative state JSON is malformed"))?;
        let current = value
            .get("bootstrapCreate")
            .and_then(|bootstrap| bootstrap.get("idempotencyKeyDigest"))
            .and_then(Value::as_str);
        if current != Some(key_digest) {
            continue;
        }
        if matched.is_some() {
            return Err(RuntimeError::new(
                StableErrorCode::ScopeAmbiguous,
                "anonymous create idempotency key matched multiple Work Items",
            ));
        }
        matched = Some(value);
    }
    Ok(matched)
}

fn created_work_item_response(state: &Value) -> RuntimeResult<Value> {
    let string = |field: &str| {
        state
            .get(field)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| schema_error("created Work Item replay state is malformed"))
    };
    let work_item_id = string("stateMachineName")?;
    let state_machine_id = string("stateMachineId")?;
    let state_uuid = string("stateUuid")?;
    let entry_node = string("entryNode")?;
    Ok(json!({
        "changed": true,
        "revisionBefore": Value::Null,
        "revisionAfter": Value::Null,
        "receiptDigest": Value::Null,
        "data": {"workItemId": work_item_id, "stateMachineId": state_machine_id,
                 "stateMachineName": work_item_id,
                 "stateUuid": state_uuid, "entryNode": entry_node,
                 "statePath": format!(".auto-engineering/{state_machine_id}/state.json")},
    }))
}

/// Whether any existing `state.json` already answers to this business name.
fn existing_state_directory(root: &Path, work_item: &str) -> RuntimeResult<bool> {
    if !root.is_dir() {
        return Ok(false);
    }
    for entry in fs::read_dir(root).map_err(|error| io_error("list state directories", error))? {
        let path = entry
            .map_err(|error| io_error("list state directories", error))?
            .path()
            .join("state.json");
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| io_error("read state file", error))?;
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        if state_matches(&value, work_item) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn read_state(workspace: &BusinessWorkspace, work_item: &str) -> RuntimeResult<LocatedState> {
    let root = Path::new(&workspace.canonical_root);
    let directory = root.join(".auto-engineering");
    let mut matches = Vec::new();
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(RuntimeError::new(
                StableErrorCode::ProjectMismatch,
                "Work Item key did not match any state directory",
            ));
        }
        Err(error) => return Err(io_error("list state directories", error)),
    };
    for entry in entries {
        let path = entry
            .map_err(|error| io_error("list state directories", error))?
            .path()
            .join("state.json");
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| io_error("read state file", error))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| schema_error("authoritative state JSON is malformed"))?;
        if state_matches(&value, work_item) {
            matches.push((path, value));
        }
    }
    if matches.len() != 1 {
        let (code, message) = if matches.is_empty() {
            (
                StableErrorCode::ProjectMismatch,
                "Work Item key did not match any state directory",
            )
        } else {
            (
                StableErrorCode::ScopeAmbiguous,
                "Work Item key matched multiple state directories",
            )
        };
        return Err(RuntimeError::new(code, message));
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
    workspace: &BusinessWorkspace,
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
    // The completion milestone recorded by evidence/review mutations enters
    // the snapshot rolled back against the digests observed right now, so a
    // stale input never keeps `GovernanceClosed` on any flow decision.
    if let Some((milestone, bound)) = completion_milestone_from_state(state)? {
        let observed = observed_completion_digests(workspace, state, work_item_id)?;
        snapshot =
            snapshot.with_completion_milestone(milestone.invalidate(&bound, &observed), bound);
    }
    let scale = parse_scale(
        state
            .get("scale")
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("authoritative route scale is missing"))?,
    )?;
    let route = state
        .get("routeDecision")
        .and_then(|value| {
            value
                .get("designRoute")
                .or_else(|| value.get("selectedDesign"))
        })
        .or_else(|| state.get("selectedDesign"))
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error("authoritative selectedDesign is missing"))?;
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

fn route_selection_missing(state: &Value) -> bool {
    state.get("scale").and_then(Value::as_str).is_none()
        || state
            .get("routeDecision")
            .and_then(|value| {
                value
                    .get("designRoute")
                    .or_else(|| value.get("selectedDesign"))
            })
            .or_else(|| state.get("selectedDesign"))
            .and_then(Value::as_str)
            .is_none()
}

fn route_pending_projection(state: &Value, work_item_id: &str) -> RuntimeResult<Value> {
    if state.get("entryNode").and_then(Value::as_str) != Some("ROUTE") {
        return Err(schema_error(
            "authoritative route selection is missing outside a ROUTE intake",
        ));
    }
    let revision = state
        .get("revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| schema_error("authoritative state revision is missing"))?;
    Ok(json!({
        "phase":"initialized",
        "stateRevision":revision,
        "nextAction":{
            "kind":"analyze-route",
            "workItemId":work_item_id,
            "requiredFacts":["requestedIntent","impactFacts","classificationConfidenceBps"],
            "submit":{
                "method":"operation.execute",
                "operation":"route.decide",
                "requiresExpectedRevision":true,
                "requiresIdempotencyKey":true
            }
        }
    }))
}

fn route_control_projection(state: &Value, work_item_id: &str) -> RuntimeResult<Option<Value>> {
    if route_selection_missing(state) {
        return route_pending_projection(state, work_item_id).map(Some);
    }
    if state.get("routeApproved").and_then(Value::as_bool) == Some(false) {
        let confirmation_id = state
            .get("routeApprovalConfirmationId")
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("pending route approval binding is missing"))?;
        return Ok(Some(json!({
            "phase":"initialized",
            "stateRevision":state.get("revision"),
            "routeDecision":state.get("routeDecision"),
            "nextAction":{
                "kind":"approve-route",
                "confirmationId":confirmation_id,
                "submit":{
                    "method":"operation.execute",
                    "operation":"route.decide",
                    "requiresSameFacts":true,
                    "requiresUserApprovalRef":true
                }
            }
        })));
    }
    Ok(None)
}

fn decorate_route_handoff(
    mut projection: Value,
    state: &Value,
    current_review_input: Option<InputFingerprint>,
) -> RuntimeResult<Value> {
    let next_action = if projection
        .pointer("/nextAction/kind")
        .and_then(Value::as_str)
        == Some("await-agent-work")
    {
        let final_action = final_verification_handoff_action(state, current_review_input)?;
        if final_action.is_some() {
            final_action
        } else {
            route_handoff_action(state)?
        }
    } else {
        None
    };
    if let Some(next_action) = next_action
        && let Some(object) = projection.as_object_mut()
    {
        object.insert("nextAction".to_owned(), next_action);
    }
    Ok(projection)
}

/// Tier 3 Review cannot finalize until a daemon-committed verification receipt
/// exists. Surface that missing authority as a typed host action instead of
/// falling through to the ambiguous `await-agent-work` projection.
fn final_verification_handoff_action(
    state: &Value,
    current_review_input: Option<InputFingerprint>,
) -> RuntimeResult<Option<Value>> {
    if state.pointer("/reviewSession/tier").and_then(Value::as_str) != Some("tier3")
        || state
            .pointer("/reviewSession/status")
            .and_then(Value::as_str)
            != Some("running")
    {
        return Ok(None);
    }
    let session = state
        .get("reviewSession")
        .and_then(Value::as_object)
        .ok_or_else(|| schema_error("Tier 3 Review session is malformed"))?;
    let required = |field: &str| {
        session
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("Tier 3 Review session lacks final verification binding"))
    };
    let source_revision = session
        .get("sourceRevision")
        .and_then(Value::as_u64)
        .ok_or_else(|| schema_error("Tier 3 Review session lacks sourceRevision"))?;
    let inventory_generation = session
        .get("inventoryGeneration")
        .and_then(Value::as_u64)
        .ok_or_else(|| schema_error("Tier 3 Review session lacks inventoryGeneration"))?;
    let session_input = InputFingerprint::from_str(required("inputFingerprint")?)
        .map_err(|_| schema_error("Tier 3 Review inputFingerprint is malformed"))?;
    if let Some(current) = current_review_input
        && current != session_input
    {
        return Ok(Some(json!({
            "kind":"refresh-verification-evidence",
            "inputFingerprint":current.to_string(),
            "submit":{
                "method":"operation.execute",
                "operation":"evidence.record",
                "arguments":{"inputFingerprint":current.to_string()},
                "followUp":{"method":"operation.execute","operation":"evidence.finalize"},
                "requires":["active-lease","verification-artifact"]
            }
        })));
    }
    let session_input_text = session_input.to_string();
    let receipt_is_current = state
        .get("finalVerificationBinding")
        .and_then(Value::as_object)
        .is_some_and(|receipt| {
            let active_receipt = state.get("toolsetReceiptRef").and_then(Value::as_object);
            receipt.get("reviewId").and_then(Value::as_str)
                == session.get("reviewId").and_then(Value::as_str)
                && receipt.get("inputFingerprint").and_then(Value::as_str)
                    == Some(session_input_text.as_str())
                && receipt.get("rulesetFingerprint").and_then(Value::as_str)
                    == session.get("rulesetFingerprint").and_then(Value::as_str)
                && receipt.get("policyDigest").and_then(Value::as_str)
                    == session.get("policyDigest").and_then(Value::as_str)
                && receipt.get("inventoryGeneration").and_then(Value::as_u64)
                    == Some(inventory_generation)
                && receipt.get("sourceRevision").and_then(Value::as_u64) == Some(source_revision)
                && receipt.get("toolsetJobId").and_then(Value::as_str)
                    == active_receipt
                        .and_then(|value| value.get("toolsetJobId"))
                        .and_then(Value::as_str)
                && receipt.get("receiptId").and_then(Value::as_str)
                    == active_receipt
                        .and_then(|value| value.get("receiptId"))
                        .and_then(Value::as_str)
        });
    if receipt_is_current {
        return Ok(None);
    }
    Ok(Some(json!({
        "kind":"record-final-verification",
        "reviewId":required("reviewId")?,
        "sourceRevision":source_revision,
        "inputFingerprint":required("inputFingerprint")?,
        "rulesetFingerprint":required("rulesetFingerprint")?,
        "policyDigest":required("policyDigest")?,
        "inventoryGeneration":inventory_generation,
        "submit":{
            "method":"job.submit",
            "entrypoint":"toolset.receipt.record",
            "arguments":{
                "finalizedEvidence":{
                    "reviewId":required("reviewId")?,
                    "sourceRevision":source_revision,
                    "inputFingerprint":required("inputFingerprint")?,
                    "rulesetFingerprint":required("rulesetFingerprint")?,
                    "policyDigest":required("policyDigest")?,
                    "inventoryGeneration":inventory_generation
                }
            },
            "requires":["active-lease","finalized-verification-evidence"]
        }
    })))
}

fn current_review_input_fingerprint(
    workspace: &BusinessWorkspace,
    state: &Value,
) -> RuntimeResult<Option<InputFingerprint>> {
    if state.pointer("/reviewSession/tier").and_then(Value::as_str) == Some("tier3")
        && state
            .pointer("/reviewSession/status")
            .and_then(Value::as_str)
            == Some("running")
    {
        review_authority::authoritative_review_workspace_input_fingerprint(workspace, state)
            .map(Some)
    } else {
        Ok(None)
    }
}

fn route_handoff_action(state: &Value) -> RuntimeResult<Option<Value>> {
    if state.get("entryNode").and_then(Value::as_str) != Some("ROUTE")
        || state.get("routeApproved").and_then(Value::as_bool) != Some(true)
    {
        return Ok(None);
    }
    let decision = state
        .get("routeDecision")
        .and_then(Value::as_object)
        .ok_or_else(|| schema_error("approved ROUTE intake is missing routeDecision"))?;
    let required = decision
        .get("requiredSeries")
        .and_then(Value::as_array)
        .ok_or_else(|| schema_error("approved routeDecision is missing requiredSeries"))?;
    let requires = |series: &str| required.iter().any(|value| value.as_str() == Some(series));
    let committed = |intent: &str| {
        state
            .get("routeDocuments")
            .and_then(Value::as_object)
            .and_then(|documents| documents.get(intent))
            .and_then(Value::as_bool)
            == Some(true)
    };
    let delegate = |series_kind: &str, required_artifacts: &[&str]| {
        json!({
            "kind":"delegate-series",
            "seriesKind":series_kind,
            "requiredArtifacts":required_artifacts,
            "routeDecision":state.get("routeDecision"),
            "submit":{
                "method":"delegation.create",
                "role":"series",
                "taskKind":series_kind
            }
        })
    };

    if requires("requirement-analysis") && !committed("RA") {
        return Ok(Some(delegate("requirement-analysis", &["RA"])));
    }
    if requires("design-review") && !committed("DR") {
        return Ok(Some(delegate("design-review", &["DR"])));
    }
    if requires("story") && (!committed("STORY") || !committed("CODING_PLAN")) {
        return Ok(Some(delegate("story", &["STORY", "CODING_PLAN"])));
    }
    if !requires("story") && !committed("CODING_PLAN") {
        return Ok(Some(delegate("coding-plan", &["CODING_PLAN"])));
    }

    let plan = state
        .get("executionPlan")
        .and_then(Value::as_object)
        .ok_or_else(|| schema_error("ROUTE intake is missing executionPlan"))?;
    if plan
        .get("goal")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .is_empty()
    {
        return Ok(Some(json!({
            "kind":"prepare-execution-plan",
            "submit":{"method":"operation.execute","operation":"execution.plan.set"}
        })));
    }
    if plan.get("approved").and_then(Value::as_bool) != Some(true) {
        return Ok(Some(json!({
            "kind":"approve-execution-plan",
            "requiresUserApproval":true,
            "submit":{"method":"operation.execute","operation":"execution.plan.approve"}
        })));
    }
    let phase = parse_phase(
        state
            .get("currentPhase")
            .or_else(|| state.get("phase"))
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("approved ROUTE intake is missing its current phase"))?,
    )?;
    if phase < ProcessPhase::Coding {
        let scale = parse_scale(
            state
                .get("scale")
                .or_else(|| decision.get("scale"))
                .and_then(Value::as_str)
                .ok_or_else(|| schema_error("approved ROUTE intake is missing its scale"))?,
        )?;
        let design_route = parse_route(
            decision
                .get("designRoute")
                .or_else(|| decision.get("selectedDesign"))
                .or_else(|| state.get("selectedDesign"))
                .and_then(Value::as_str)
                .ok_or_else(|| schema_error("approved ROUTE intake is missing its design route"))?,
        )?;
        let target_phase = next_route_phase(phase, scale, design_route)?;
        return Ok(Some(json!({
            "kind":"advance-route-phase",
            "targetPhase":target_phase,
            "submit":{
                "method":"flow.next",
                "arguments":{"targetPhase":target_phase},
                "requiresIdempotencyKey":true
            }
        })));
    }
    if phase == ProcessPhase::Coding && state.get("executionRuntime").is_none() {
        return Ok(Some(json!({
            "kind":"resume-approved-execution",
            "submit":{"method":"operation.execute","operation":"execution.resume"}
        })));
    }
    Ok(None)
}

fn next_route_phase(
    current: ProcessPhase,
    scale: WorkScale,
    design_route: DesignRoute,
) -> RuntimeResult<&'static str> {
    [
        "route-selected",
        "requirement-analyzed",
        "dr-generated",
        "story-generated",
        "testcase-generated",
        "coding-process",
        "coding",
    ]
    .into_iter()
    .find(|target| {
        let Ok(target) = parse_phase(target) else {
            return false;
        };
        TransitionPolicy::authorize(TransitionContext {
            actor_role: AgentRole::Root,
            current,
            target,
            scale,
            design_route,
            paused_from: None,
        })
        .is_ok()
    })
    .ok_or_else(|| schema_error("approved ROUTE intake has no legal phase handoff before Coding"))
}

/// Reads the completion milestone and bound digest set recorded in the
/// `executionRuntime` state section. Work Items without an approved execution
/// runtime carry no milestone and stay on the plain phase wire, which keeps
/// the completion chain strictly opt-in for execution-governed items.
/// Projects the completion milestone recorded in state together with the
/// freshly observed digests that decide whether it is still valid.
///
/// Completion is authorized from this projection, so every lifecycle path that
/// can authorize a terminal transition must supply it; omitting it denies the
/// transition as milestone-required.
fn completion_projection(
    workspace: &BusinessWorkspace,
    state: &Value,
    work_item_id: &str,
) -> RuntimeResult<Option<CompletionMilestoneInput>> {
    match completion_milestone_from_state(state)? {
        Some((milestone, bound)) => Ok(Some(CompletionMilestoneInput::new(
            milestone,
            bound,
            observed_completion_digests(workspace, state, work_item_id)?,
        ))),
        None => Ok(None),
    }
}

fn completion_milestone_from_state(
    state: &Value,
) -> RuntimeResult<Option<(CompletionMilestone, CompletionDigestSet)>> {
    let Some(runtime) = state.get("executionRuntime") else {
        return Ok(None);
    };
    let milestone = match runtime.get("completionMilestone").and_then(Value::as_str) {
        None | Some("none") => CompletionMilestone::None,
        Some("implementation-verified") => CompletionMilestone::ImplementationVerified,
        Some("review-ready") => CompletionMilestone::ReviewReady,
        Some("governance-closed") => CompletionMilestone::GovernanceClosed,
        Some(_) => {
            return Err(schema_error(
                "executionRuntime completionMilestone is unsupported",
            ));
        }
    };
    let bound = runtime
        .get("completionBound")
        .map(completion_bound_from_value)
        .transpose()?
        .unwrap_or(CompletionDigestSet::ZERO);
    Ok(Some((milestone, bound)))
}

fn completion_bound_from_value(value: &Value) -> RuntimeResult<CompletionDigestSet> {
    let digest = |field: &str| -> RuntimeResult<ArtifactDigest> {
        let raw = value
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("executionRuntime completionBound is incomplete"))?;
        ArtifactDigest::from_str(raw.strip_prefix("sha256:").unwrap_or(raw))
            .map_err(|_| schema_error("executionRuntime completionBound digest is malformed"))
    };
    Ok(CompletionDigestSet::new(
        digest("codeDigest")?,
        digest("verificationDigest")?,
        digest("evidenceDigest")?,
        digest("reviewInputDigest")?,
        digest("gateDigest")?,
    ))
}

/// Computes the completion input digests observed right now.
///
/// Every dimension is revision- and fencing-independent so unrelated
/// committed mutations never roll the milestone: `code` binds the approved
/// changed-path contents from the workspace, `verification` binds the active
/// entries of the sealed evidence manifest, `evidence` binds the recorded
/// evidence authority projection, and `reviewInput`/`gate` bind the Review
/// and policy inputs written by the Review authority. The FlowRuntime rolls
/// the recorded milestone back to the earliest point whose bound digest no
/// longer matches, so a stale changed path can never keep `GovernanceClosed`.
fn observed_completion_digests(
    workspace: &BusinessWorkspace,
    state: &Value,
    work_item_id: &str,
) -> RuntimeResult<CompletionDigestSet> {
    let root = Path::new(&workspace.canonical_root);
    Ok(CompletionDigestSet::new(
        code_digest(root, state)?,
        verification_digest(root, state, work_item_id)?,
        evidence_authority_digest(state),
        review_binding_digest(state),
        completion_gate_digest(state),
    ))
}

/// Digest of the approved changed-path list plus each path's current
/// content. A missing path hashes an explicit `<missing>` marker instead of
/// vanishing, a directory hashes `<directory>`, and a path that cannot be
/// read hashes `<unreadable>`.
///
/// The approved plan is the authority and the filesystem is only observed:
/// this digest feeds every projection-time authority load (session.open,
/// gate.evaluate, flow.snapshot/next, execution.resume), so one unreadable
/// changed path must never wedge the Work Item. Each marker differs from any
/// content hash, so a milestone recorded against real content still rolls
/// back while its bound path is unreadable — the fail-safe direction.
fn code_digest(root: &Path, state: &Value) -> RuntimeResult<ArtifactDigest> {
    let mut paths: Vec<String> = state
        .pointer("/executionPlan/changedPaths")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths.dedup();
    let mut hasher = Sha256::new();
    hasher.update(b"ae-sdd-completion-code/v1\0");
    for relative in paths {
        hash_part(&mut hasher, relative.as_bytes());
        let path = root.join(&relative);
        match fs::metadata(&path) {
            Ok(metadata) if metadata.is_dir() => hash_part(&mut hasher, b"<directory>"),
            Ok(_) => match fs::read(&path) {
                Ok(bytes) => hash_part(&mut hasher, &bytes),
                Err(_) => hash_part(&mut hasher, b"<unreadable>"),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                hash_part(&mut hasher, b"<missing>");
            }
            Err(_) => hash_part(&mut hasher, b"<unreadable>"),
        }
    }
    Ok(ArtifactDigest::from_array(hasher.finalize().into()))
}

/// Digest of the active entries in the Story's sealed evidence manifest.
fn verification_digest(
    root: &Path,
    state: &Value,
    work_item_id: &str,
) -> RuntimeResult<ArtifactDigest> {
    let story = operation_story_id(state, work_item_id)?;
    let path = root
        .join(".auto-engineering")
        .join(story)
        .join("evidence/manifest.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ArtifactDigest::digest(
                b"ae-sdd-completion-verification/missing\0",
            ));
        }
        Err(_) => {
            return Ok(ArtifactDigest::digest(
                b"ae-sdd-completion-verification/unreadable\0",
            ));
        }
    };
    verification_digest_from_manifest_bytes(&bytes)
}

/// Digest of the active entries in one sealed evidence manifest payload.
///
/// Each entry's `inputFingerprint` review-binding is excluded: it is re-sealed
/// against the current Review input whenever a Review starts, which is exactly
/// what the separate `reviewInput` milestone dimension tracks, so it must not
/// also roll the verification dimension.
fn verification_digest_from_manifest_bytes(bytes: &[u8]) -> RuntimeResult<ArtifactDigest> {
    let manifest: Value = serde_json::from_slice(bytes)
        .map_err(|_| schema_error("sealed evidence manifest is malformed"))?;
    let active: Vec<Value> = manifest
        .get("entries")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| {
                    entry
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("active")
                        == "active"
                })
                .map(|entry| {
                    let mut entry = entry.clone();
                    if let Some(object) = entry.as_object_mut() {
                        object.remove("inputFingerprint");
                    }
                    entry
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(ArtifactDigest::digest(
        serde_json::to_vec(&canonical_value(&Value::Array(active)))
            .map_err(|_| schema_error("active evidence entries are not canonicalizable"))?,
    ))
}

/// Stable wire value of one completion milestone inside the
/// `executionRuntime` state section.
fn completion_milestone_wire(milestone: CompletionMilestone) -> &'static str {
    match milestone {
        CompletionMilestone::None => "none",
        CompletionMilestone::ImplementationVerified => "implementation-verified",
        CompletionMilestone::ReviewReady => "review-ready",
        CompletionMilestone::GovernanceClosed => "governance-closed",
    }
}

fn digest_wire(digest: ArtifactDigest) -> String {
    format!("sha256:{digest}")
}

/// Gate input selectors whose dependent Gates must re-evaluate after one
/// committed mutation of this operation.
fn mutation_gate_selectors(operation: OperationName) -> &'static [GateInputSelector] {
    match operation {
        OperationName::DocumentSave
        | OperationName::ExecutionSliceStart
        | OperationName::ExecutionSliceRecord => &[GateInputSelector::ChangedPaths],
        OperationName::EvidenceRecord | OperationName::EvidenceFinalize => {
            &[GateInputSelector::EvidenceLedger]
        }
        OperationName::ReviewContribute
        | OperationName::ReviewRecord
        | OperationName::ReviewFinalize => &[GateInputSelector::ReviewBatch],
        OperationName::ExecutionPlanSet | OperationName::ExecutionPlanApprove => {
            &[GateInputSelector::ExecutionPlan]
        }
        _ => &[],
    }
}

/// Digest of the recorded evidence authority projection (`None` when no
/// evidence was ever recorded), stable across unrelated state mutations.
fn evidence_authority_digest(state: &Value) -> ArtifactDigest {
    digest_optional_section(state, "evidenceAuthority")
}

/// Digest of the Review input binding the Review authority wrote into state.
fn review_binding_digest(state: &Value) -> ArtifactDigest {
    digest_fields(
        state,
        &[
            "inputFingerprint",
            "rulesetFingerprint",
            "policyDigest",
            "inventoryGeneration",
        ],
        b"ae-sdd-completion-review-input/v1\0",
    )
}

/// Digest of the final completion-Gate inputs: the Review binding plus the
/// sealed evidence authority those Gates join at the terminal transition.
fn completion_gate_digest(state: &Value) -> ArtifactDigest {
    let mut hasher = Sha256::new();
    hasher.update(b"ae-sdd-completion-gates/v1\0");
    hash_part(&mut hasher, review_binding_digest(state).as_bytes());
    hash_part(&mut hasher, evidence_authority_digest(state).as_bytes());
    ArtifactDigest::from_array(hasher.finalize().into())
}

fn digest_optional_section(state: &Value, field: &str) -> ArtifactDigest {
    match state.get(field) {
        Some(value) => ArtifactDigest::digest(
            serde_json::to_vec(&canonical_value(value))
                .unwrap_or_else(|_| b"<uncanonicalizable>".to_vec()),
        ),
        None => ArtifactDigest::digest(format!("ae-sdd-completion-absent/{field}\0")),
    }
}

fn digest_fields(state: &Value, fields: &[&str], domain: &[u8]) -> ArtifactDigest {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for field in fields {
        hash_part(&mut hasher, field.as_bytes());
        match state.get(*field) {
            Some(value) => hash_part(
                &mut hasher,
                &serde_json::to_vec(&canonical_value(value))
                    .unwrap_or_else(|_| b"<uncanonicalizable>".to_vec()),
            ),
            None => hash_part(&mut hasher, b"<absent>"),
        }
    }
    ArtifactDigest::from_array(hasher.finalize().into())
}

fn hash_part(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn canonical_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_value(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect::<Map<_, _>>(),
        ),
        Value::Array(array) => Value::Array(array.iter().map(canonical_value).collect()),
        _ => value.clone(),
    }
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
        runtime
            .get("activeSliceStatus")
            .and_then(Value::as_str)
            .map(parse_execution_status)
            .transpose()?
            .unwrap_or(ExecutionSliceStatus::Pending),
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

fn prepare_route_decision(
    state: &Value,
    request: &ValidatedOperationRequest,
) -> RuntimeResult<Value> {
    if state.get("entryNode").and_then(Value::as_str) != Some("ROUTE") {
        return Err(schema_error(
            "route.decide is only valid for a ROUTE intake Work Item",
        ));
    }
    if state.get("routeApproved").and_then(Value::as_bool) == Some(true) {
        return Err(schema_error(
            "route.decide cannot replace an already committed route decision",
        ));
    }
    let work_item_id = request
        .request()
        .work_item_id
        .as_ref()
        .ok_or_else(|| schema_error("workItemId is required"))?;
    let payload = &request.request().payload;
    let fingerprint_payload = json!({
        "requestedIntent":payload.get("requestedIntent"),
        "availableArtifacts":payload.get("availableArtifacts").cloned().unwrap_or_else(|| json!([])),
        "impactFacts":payload.get("impactFacts"),
        "classificationConfidenceBps":payload.get("classificationConfidenceBps"),
    });
    let fingerprint = InputFingerprint::digest(
        serde_json::to_vec(&json!({
            "workItemId":work_item_id.as_str(),
            "routeFacts":fingerprint_payload,
        }))
        .map_err(|_| schema_error("route input fingerprint could not be computed"))?,
    );
    let input: RouteInput = serde_json::from_value(json!({
        "schemaVersion":SchemaVersion::V1,
        "workItemId":work_item_id.as_str(),
        "entryNode":"entry.route",
        "requestedIntent":payload.get("requestedIntent").cloned().unwrap_or(Value::Null),
        "availableArtifacts":payload.get("availableArtifacts").cloned().unwrap_or_else(|| json!([])),
        "impactFacts":payload.get("impactFacts").cloned().unwrap_or(Value::Null),
        "classificationConfidenceBps":payload.get("classificationConfidenceBps").cloned().unwrap_or(Value::Null),
        "inputFingerprint":fingerprint.to_string(),
        "userApprovalRef":payload.get("userApprovalRef").cloned().unwrap_or(Value::Null),
    }))
    .map_err(|error| schema_error(&format!("route input is invalid: {error}")))?;
    let engine = RouteEngine::default();
    let approval_confirmation_id = format!(
        "route:{}",
        engine
            .approval_binding(&input)
            .map_err(|error| schema_error(&format!("route approval binding failed: {error}")))?
    );
    let decision = engine
        .decide(&input)
        .map_err(|error| schema_error(&format!("route decision failed: {error}")))?;
    let scale = match decision.scale() {
        WorkScale::Large => "large",
        WorkScale::Medium => "medium",
        WorkScale::Small => "small",
        WorkScale::Micro => "micro",
    };
    let selected_design = match decision.design_route() {
        DesignRoute::Dr => "DR",
        DesignRoute::Story => "STORY",
        DesignRoute::CodingPlan => "CODING_PLAN",
    };
    let approved = decision.is_approved();
    Ok(json!({
        "decision":decision,
        "scale":scale,
        "selectedDesign":selected_design,
        "approved":approved,
        "approvalConfirmationId":approval_confirmation_id,
    }))
}

fn prepare_execution_slice_mutation(
    workspace: &BusinessWorkspace,
    state: &Value,
    request: &ValidatedOperationRequest,
    authority: &SemanticAuthorityContext<'_>,
) -> RuntimeResult<PreparedSemanticMutation> {
    let work_item_id = request
        .request()
        .work_item_id
        .as_ref()
        .ok_or_else(|| schema_error("workItemId is required"))?;
    let plan = execution_authority::approved_plan_authority(state)?;
    let bundle = execution_authority::load_required_context_bundle(
        Path::new(&workspace.canonical_root),
        state,
        work_item_id.as_str(),
        plan.plan(),
    )?;
    let committed = execution_authority::verify_committed_capsule(
        Path::new(&workspace.canonical_root),
        state,
        &plan,
        &bundle,
        authority.policy_digest,
    )?;
    let capsule = committed.capsule();
    let runtime = state
        .get("executionRuntime")
        .and_then(Value::as_object)
        .ok_or_else(|| execution_slice_error("executionRuntime is missing or malformed"))?;
    let active_ordinal = runtime
        .get("activeSliceOrdinal")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| execution_slice_error("active slice ordinal is malformed"))?;
    let current_status = execution_status_from_runtime(runtime)?;
    let refactor_cycle = refactor_cycle_from_runtime(runtime)?;

    let (next_status, progress_digest) = match request.operation() {
        OperationName::ExecutionSliceStart => {
            let requested_ordinal = request.request().payload["activeOrdinal"]
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| execution_slice_error("activeOrdinal is malformed"))?;
            if requested_ordinal != active_ordinal {
                return Err(execution_slice_error(
                    "activeOrdinal does not match the authoritative execution cursor",
                ));
            }
            let requested_digest = request.request().payload["queueDigest"]
                .as_str()
                .ok_or_else(|| capsule_stale_error("queueDigest is malformed"))?;
            if parse_execution_digest(requested_digest, "queueDigest")?
                != capsule.queue().queue_digest()
            {
                return Err(capsule_stale_error(
                    "queueDigest does not match the approved execution queue",
                ));
            }
            let (status, _) = transition_slice_status(
                current_status,
                refactor_cycle,
                ExecutionSliceEvent::Claimed,
            )
            .map_err(|_| execution_slice_error("the active slice cannot be started"))?;
            (status, None)
        }
        OperationName::ExecutionSliceRecord => {
            let requested_slice_id = request.request().payload["sliceId"]
                .as_str()
                .ok_or_else(|| execution_slice_error("sliceId is malformed"))?;
            if requested_slice_id != capsule.active_slice().slice_id().as_str() {
                return Err(execution_slice_error(
                    "sliceId does not match the authoritative active slice",
                ));
            }
            let target_status = parse_execution_status(
                request.request().payload["status"]
                    .as_str()
                    .ok_or_else(|| execution_slice_error("status is malformed"))?,
            )?;
            let progress_digest = request.request().payload["progressDigest"]
                .as_str()
                .ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::ExecutionProgressRequired,
                        "execution.slice.record requires a progressDigest",
                    )
                })?;
            let progress_digest = parse_execution_digest(progress_digest, "progressDigest")?;
            if runtime
                .get("lastProgressDigest")
                .and_then(Value::as_str)
                .is_some_and(|last| last == progress_digest.to_string())
            {
                return Err(RuntimeError::new(
                    StableErrorCode::ExecutionProgressRequired,
                    "execution slice progress must be new",
                ));
            }
            let event = execution_event_for_target(current_status, target_status)?;
            let (status, _) = transition_slice_status(current_status, refactor_cycle, event)
                .map_err(|_| {
                    execution_slice_error(
                        "requested status is not the next legal slice lifecycle state",
                    )
                })?;
            (status, Some(progress_digest))
        }
        _ => return Err(schema_error("operation is not an execution slice mutation")),
    };

    let mut next_runtime = Value::Object(runtime.clone());
    next_runtime["activeSliceStatus"] = execution_status_value(next_status)?;
    next_runtime["refactorCycle"] = Value::String("idle".to_owned());
    if let Some(digest) = progress_digest {
        next_runtime["lastProgressDigest"] = Value::String(digest.to_string());
    }

    let locators = execution_authority::execution_artifact_locators(work_item_id.as_str())?;
    let ledger_path = Path::new(&workspace.canonical_root).join(locators.ledger().as_str());
    let mut ledger_bytes =
        fs::read(&ledger_path).map_err(|error| io_error("read execution ledger", error))?;
    let ledger_before = ArtifactDigest::digest(&ledger_bytes);
    let expected_ledger = runtime
        .get("ledgerDigest")
        .and_then(Value::as_str)
        .ok_or_else(|| capsule_stale_error("execution ledger digest is missing"))?;
    if parse_execution_digest(expected_ledger, "executionRuntime.ledgerDigest")? != ledger_before {
        return Err(capsule_stale_error("execution ledger digest drifted"));
    }

    let mut targets = Vec::new();
    let mut resulting_status = next_status;
    let mut resulting_ordinal = active_ordinal;
    if next_status == ExecutionSliceStatus::Completed
        && active_ordinal < capsule.queue().total_slices()
    {
        let source_revision = request
            .request()
            .expected_revision
            .ok_or_else(|| schema_error("expectedRevision is required"))?
            .checked_next()
            .map_err(|_| schema_error("state revision overflow"))?;
        let next_outcome = execution_authority::build_capsule_from_authority(
            state,
            work_item_id.as_str(),
            source_revision,
            active_ordinal + 1,
            &plan,
            &bundle,
            authority.policy_digest,
            authority.inventory_generation,
        )?;
        targets.push(evidence::SemanticTarget {
            relative_path: locators.capsule().as_str().to_owned(),
            before_digest: Some(committed.capsule_digest()),
            after_bytes: next_outcome.capsule_bytes().to_vec(),
        });
        resulting_status = ExecutionSliceStatus::Pending;
        resulting_ordinal += 1;
        next_runtime["capsuleDigest"] =
            Value::String(format!("sha256:{}", next_outcome.capsule_digest()));
        next_runtime["activeSliceOrdinal"] = Value::from(resulting_ordinal);
        next_runtime["activeSliceStatus"] = execution_status_value(resulting_status)?;
        next_runtime["refactorCycle"] = Value::String("idle".to_owned());
        next_runtime
            .as_object_mut()
            .expect("execution runtime stays an object")
            .remove("lastProgressDigest");
    }

    let ledger_event = json!({
        "schemaVersion":1,
        "kind":request.operation().as_str(),
        "sliceId":capsule.active_slice().slice_id().as_str(),
        "activeOrdinal":active_ordinal,
        "fromStatus":execution_status_value(current_status)?,
        "status":execution_status_value(next_status)?,
        "progressDigest":progress_digest.map(|digest| digest.to_string()),
        "queueDigest":capsule.queue().queue_digest().to_string(),
        "capsuleDigest":committed.capsule_digest().to_string(),
    });
    ledger_bytes.extend_from_slice(
        &serde_json::to_vec(&ledger_event)
            .map_err(|_| schema_error("execution ledger event could not be serialized"))?,
    );
    ledger_bytes.push(b'\n');
    let ledger_after = ArtifactDigest::digest(&ledger_bytes);
    next_runtime["ledgerDigest"] = Value::String(format!("sha256:{ledger_after}"));
    targets.push(evidence::SemanticTarget {
        relative_path: locators.ledger().as_str().to_owned(),
        before_digest: Some(ledger_before),
        after_bytes: ledger_bytes,
    });

    Ok(PreparedSemanticMutation::execution(
        json!({
            "sliceId":capsule.active_slice().slice_id().as_str(),
            "activeOrdinal":active_ordinal,
            "status":execution_status_value(next_status)?,
            "nextActiveOrdinal":resulting_ordinal,
            "nextStatus":execution_status_value(resulting_status)?,
            "queueDigest":capsule.queue().queue_digest().to_string(),
        }),
        targets,
        next_runtime,
    ))
}

fn execution_status_from_runtime(
    runtime: &Map<String, Value>,
) -> RuntimeResult<ExecutionSliceStatus> {
    runtime
        .get("activeSliceStatus")
        .and_then(Value::as_str)
        .map(parse_execution_status)
        .transpose()
        .map(|status| status.unwrap_or(ExecutionSliceStatus::Pending))
}

fn parse_execution_status(value: &str) -> RuntimeResult<ExecutionSliceStatus> {
    serde_json::from_value(Value::String(value.to_owned()))
        .map_err(|_| execution_slice_error("execution slice status is invalid"))
}

fn execution_status_value(status: ExecutionSliceStatus) -> RuntimeResult<Value> {
    serde_json::to_value(status)
        .map_err(|_| schema_error("execution slice status could not be serialized"))
}

fn refactor_cycle_from_runtime(runtime: &Map<String, Value>) -> RuntimeResult<RefactorCycleV1> {
    match runtime
        .get("refactorCycle")
        .and_then(Value::as_str)
        .unwrap_or("idle")
    {
        "idle" => Ok(RefactorCycleV1::Idle),
        "open" => Ok(RefactorCycleV1::Open),
        _ => Err(execution_slice_error("execution refactor cycle is invalid")),
    }
}

fn execution_event_for_target(
    current: ExecutionSliceStatus,
    target: ExecutionSliceStatus,
) -> RuntimeResult<ExecutionSliceEvent> {
    use ExecutionSliceEvent as Event;
    use ExecutionSliceStatus as Status;
    match (current, target) {
        (Status::Running, Status::RedObserved) => Ok(Event::RedObserved),
        (Status::RedObserved, Status::Patched) => Ok(Event::PatchApplied),
        (Status::Patched, Status::FocusedGreen) => Ok(Event::FocusedTestGreen),
        (Status::FocusedGreen, Status::EvidenceBound) => Ok(Event::EvidenceBound),
        (Status::EvidenceBound, Status::Completed) => Ok(Event::Completed),
        (Status::Blocked, Status::Running) => Ok(Event::Resumed),
        (status, Status::Blocked) if status != Status::Completed => Ok(Event::Blocked),
        _ => Err(execution_slice_error(
            "requested status is not adjacent to the authoritative slice status",
        )),
    }
}

fn parse_execution_digest(value: &str, field: &str) -> RuntimeResult<ArtifactDigest> {
    ArtifactDigest::from_str(value.strip_prefix("sha256:").unwrap_or(value)).map_err(|_| {
        RuntimeError::new(
            StableErrorCode::ExecutionCapsuleStale,
            format!("{field} is not a canonical SHA-256 digest"),
        )
    })
}

fn execution_slice_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::ExecutionSliceInvalid, message)
}

fn capsule_stale_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::ExecutionCapsuleStale, message)
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
        OperationName::RouteDecide => Ok(Some(PreparedSemanticMutation::plain(
            prepare_route_decision(state, request)?,
        ))),
        OperationName::ExecutionSliceStart | OperationName::ExecutionSliceRecord => {
            prepare_execution_slice_mutation(workspace, state, request, authority).map(Some)
        }
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
            prepared_review_mutation(prepared, authority)
        }
        OperationName::ReviewFinalize => {
            let caller = AuthenticatedCaller::new(
                authority.agent_id.ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::RoleOperationForbidden,
                        "review.finalize requires a daemon-authenticated agentId",
                    )
                })?,
                authority.session_id.ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::RoleOperationForbidden,
                        "review.finalize requires a daemon-authenticated sessionId",
                    )
                })?,
                authority.actor_role.ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::RoleOperationForbidden,
                        "review.finalize requires a daemon-authenticated role",
                    )
                })?,
            );
            let prepared = review_authority::prepare_review_finalize(
                workspace,
                state,
                work_item_id.as_str(),
                request,
                &caller,
                authority.persistence,
                authority.policy_digest,
                authority.inventory_generation,
                &UtcTimestamp::now(),
            )?;
            prepared_review_mutation(prepared, authority)
        }
        OperationName::StateTransition | OperationName::WorkItemComplete => {
            let expected_revision = request
                .request()
                .expected_revision
                .ok_or_else(|| schema_error("expectedRevision is required"))?;
            // Completion is authorized from the milestone recorded in state,
            // rolled back against freshly observed digests. Without this
            // projection the lifecycle input carries no milestone at all and
            // every `Completed` transition is denied as milestone-required.
            let completion = completion_projection(workspace, state, work_item_id.as_str())?;
            let outcome = lifecycle_authority::prepare_lifecycle_mutation_with_gate_passes(
                state,
                work_item_id.as_str(),
                request.operation(),
                &request.request().payload,
                expected_revision,
                completion,
                request.request().confirmation.as_ref(),
                authority.actor_role.ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::RoleOperationForbidden,
                        "lifecycle mutation requires a daemon-authenticated role",
                    )
                })?,
                authority.session_id,
                system_time_unix_ms()?,
                authority.passed_gates,
            )?;
            let permitted = outcome.into_permitted()?;
            Ok(Some(PreparedSemanticMutation::lifecycle(permitted)))
        }
        _ => Ok(None),
    }
}

/// Projects one prepared review aggregation into the semantic mutation shared
/// by `review.record` and `review.finalize`: response data, state binding and
/// the typed records the post-commit path persists as the SQLite projection.
fn prepared_review_mutation(
    prepared: review_authority::PreparedReviewRecord,
    authority: &SemanticAuthorityContext<'_>,
) -> RuntimeResult<Option<PreparedSemanticMutation>> {
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
    } else if review.get("receipt").is_some() {
        return Err(schema_error(
            "review authority projected a receipt for a nonterminal batch",
        ));
    }
    Ok(Some(PreparedSemanticMutation {
        data: review.clone(),
        targets: Vec::new(),
        execution_runtime: None,
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
        OperationName::RouteDecide => {
            let prepared = prepared_data(semantic, "route decision was not prepared")?;
            let object = root_state_object_mut(state)?;
            object.insert("routeDecision".to_owned(), prepared["decision"].clone());
            object.insert("scale".to_owned(), prepared["scale"].clone());
            object.insert(
                "selectedDesign".to_owned(),
                prepared["selectedDesign"].clone(),
            );
            object.insert("routeApproved".to_owned(), prepared["approved"].clone());
            object.insert(
                "routeApprovalConfirmationId".to_owned(),
                prepared["approvalConfirmationId"].clone(),
            );
        }
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
        OperationName::ExecutionSliceStart | OperationName::ExecutionSliceRecord => {
            let prepared = semantic
                .ok_or_else(|| schema_error("execution slice mutation was not prepared"))?;
            root_state_object_mut(state)?.insert(
                "executionRuntime".to_owned(),
                prepared
                    .execution_runtime
                    .clone()
                    .ok_or_else(|| schema_error("execution runtime projection was not prepared"))?,
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
        OperationName::ReviewRecord | OperationName::ReviewFinalize => {
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
        OperationName::DocumentSave => {
            if state.get("entryNode").and_then(Value::as_str) == Some("ROUTE") {
                let intent = request.request().payload["intent"]
                    .as_str()
                    .ok_or_else(|| schema_error("intent is required"))?
                    .to_owned();
                // The Story identity comes from the caller-supplied `docId`,
                // never from the ROUTE state machine identity — binding
                // `activeStory` to the state machine name made every
                // Story-scoped gate resolve the ROUTE machine as its Story.
                let story_binding = if intent == "STORY" {
                    request.request().payload["docId"]
                        .as_str()
                        .filter(|doc_id| doc_id.starts_with("STORY-"))
                        .and_then(|doc_id| {
                            StoryId::new(doc_id).ok().map(|_| {
                                // Best-effort: without an authoritative STORY
                                // destination mapping the docPath binding is
                                // skipped instead of failing the mutation.
                                (doc_id.to_owned(), document_path(state, "STORY").ok())
                            })
                        })
                } else {
                    None
                };
                let route_phase = story_binding
                    .as_ref()
                    .map(|_| {
                        let phase =
                            state.get("phase").and_then(Value::as_str).ok_or_else(|| {
                                schema_error("ROUTE phase is required when binding Story authority")
                            })?;
                        let parsed_phase = parse_phase(phase)?;
                        if let Some(current_phase) = state.get("currentPhase") {
                            let current_phase = current_phase.as_str().ok_or_else(|| {
                                schema_error("ROUTE currentPhase must be a string")
                            })?;
                            if parse_phase(current_phase)? != parsed_phase {
                                return Err(schema_error(
                                    "ROUTE currentPhase must match authoritative phase",
                                ));
                            }
                        }
                        Ok::<_, RuntimeError>(phase.to_owned())
                    })
                    .transpose()?;
                let object = root_state_object_mut(state)?;
                let documents = object
                    .entry("routeDocuments")
                    .or_insert_with(|| json!({}))
                    .as_object_mut()
                    .ok_or_else(|| schema_error("routeDocuments must be an object"))?;
                documents.insert(intent.clone(), Value::Bool(true));
                if let Some((story_id, doc_path)) = story_binding {
                    object.insert("activeStory".to_owned(), Value::String(story_id.clone()));
                    if let Some(doc_path) = doc_path {
                        let stories = object
                            .entry("storyStates")
                            .or_insert_with(|| json!({}))
                            .as_object_mut()
                            .ok_or_else(|| schema_error("storyStates must be an object"))?;
                        let entry = stories
                            .entry(story_id)
                            .or_insert_with(|| json!({}))
                            .as_object_mut()
                            .ok_or_else(|| schema_error("storyStates entry must be an object"))?;
                        let route_phase = route_phase
                            .as_deref()
                            .expect("Story binding always captures the ROUTE phase");
                        let story_phase = entry
                            .entry("phase")
                            .or_insert_with(|| Value::String(route_phase.to_owned()))
                            .as_str()
                            .ok_or_else(|| schema_error("Story phase must be a string"))?
                            .to_owned();
                        parse_phase(&story_phase)?;
                        let current_phase = entry
                            .entry("currentPhase")
                            .or_insert_with(|| Value::String(story_phase.clone()))
                            .as_str()
                            .ok_or_else(|| schema_error("Story currentPhase must be a string"))?;
                        if current_phase != story_phase {
                            return Err(schema_error(
                                "Story currentPhase must match authoritative phase",
                            ));
                        }
                        entry.insert(
                            "docPath".to_owned(),
                            Value::String(doc_path.as_str().to_owned()),
                        );
                    }
                }
            }
        }
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
    match fs::read(&absolute) {
        Ok(bytes) => Ok(json!({
            "path": relative.as_str(),
            "exists":true,
            "digest": ArtifactDigest::digest(&bytes).to_string(),
            "byteLength": bytes.len(),
        })),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(json!({
            "path":relative.as_str(),
            "exists":false,
            "digest":Value::Null,
            "byteLength":0,
        })),
        Err(error) => Err(io_error("read document content", error)),
    }
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
    let bytes = fs::read(root.join(content.as_str()))
        .map_err(|error| io_error("read document content", error))?;
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
        Err(error) => Err(io_error("read lease ledger", error)),
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

/// Maximum number of asset references attached to one context projection.
const MAX_PROJECTION_ASSET_REFS: usize = 8;
/// Only files small enough to hash deterministically are referenced.
const MAX_ASSET_REF_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// Maps the authoritative lifecycle phase to its methodology skill path.
fn phase_skill_path(phase: &str) -> Option<&'static str> {
    match phase {
        "requirement-analyzed" => Some("source/skills/phase1-design/requirement-analysis-skill.md"),
        "dr-generated" => Some("source/skills/phase1-design/dr-generate-skill.md"),
        "story-generated" => Some("source/skills/phase1-design/story-generate-skill.md"),
        "testcase-generated" => Some("source/skills/phase1-design/testcase-generate-skill.md"),
        "coding-process" => Some("source/skills/phase2-coding/coding-process-skill.md"),
        "coding" => Some("source/skills/phase2-coding/coding-skill.md"),
        "test-running" => Some("source/skills/phase3-review/test-generate-skill.md"),
        "code-reviewed" => Some("source/skills/phase3-review/code-review-skill.md"),
        _ => None,
    }
}

/// Builds one bounded path+sha256+kind reference when the asset exists inside
/// the workspace; unreadable or oversized assets are omitted, never invented.
fn asset_reference(root: &Path, relative: &str, kind: &str) -> Option<Value> {
    let absolute = root.join(relative);
    let metadata = fs::metadata(&absolute).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_ASSET_REF_FILE_BYTES {
        return None;
    }
    let bytes = fs::read(&absolute).ok()?;
    Some(json!({
        "kind": kind,
        "path": relative,
        "sha256": hex::encode(Sha256::digest(&bytes)),
    }))
}

/// Bounded asset reference region for one projection: the constraints index
/// plus the methodology skill of the current phase.  References carry no
/// bodies; the region is deterministic for a given filesystem state, so the
/// projection digest moves exactly when a referenced asset changes, and the
/// whole region stays inside the projection byte budget enforced on `put`.
fn projection_asset_refs(workspace: &BusinessWorkspace, phase: Option<&str>) -> Vec<Value> {
    let root = Path::new(&workspace.canonical_root);
    let mut refs = Vec::new();
    if let Some(reference) = asset_reference(root, "constraints/README.md", "constraints-index") {
        refs.push(reference);
    }
    if let Some(skill) = phase.and_then(phase_skill_path)
        && let Some(reference) = asset_reference(root, skill, "methodology-skill")
    {
        refs.push(reference);
    }
    refs.truncate(MAX_PROJECTION_ASSET_REFS);
    refs
}

fn lifecycle_target(
    operation: OperationName,
    payload: &Value,
) -> RuntimeResult<Option<ProcessPhase>> {
    match operation {
        OperationName::StateTransition => payload
            .get("targetPhase")
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("targetPhase is required"))
            .and_then(parse_phase)
            .map(Some),
        OperationName::WorkItemComplete => Ok(Some(ProcessPhase::Completed)),
        _ => Ok(None),
    }
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
    match normalize_route(value).as_str() {
        "dr" => Ok(DesignRoute::Dr),
        "story" => Ok(DesignRoute::Story),
        "codingplan" => Ok(DesignRoute::CodingPlan),
        _ => Err(schema_error("design route is invalid")),
    }
}

/// Folds separator spellings so one persisted route value parses identically
/// here and in the lifecycle authority. `classify` recommends `CODING_PLAN`,
/// so dropping `-`, `_` and spaces is required to read back what it writes.
fn normalize_route(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|character| !matches!(character, '-' | '_' | ' '))
        .flat_map(char::to_lowercase)
        .collect()
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
        // The validator already knows which field is wrong; collapsing that into
        // one opaque sentence forces every caller to read the registry source to
        // fix a typo. Each variant below renders field names and static text
        // only. `CanonicalizePayload` is the exception: it wraps a serde error
        // whose message can echo a payload value, and `RpcError` is a redacted
        // surface, so it stays collapsed.
        OperationServiceError::InvalidRequest(
            error @ (OperationRequestError::RequiredPrecondition(_)
            | OperationRequestError::InvalidIdempotencyKey
            | OperationRequestError::InvalidConfirmation
            | OperationRequestError::PayloadMustBeObject
            | OperationRequestError::UnknownPayloadField(_)
            | OperationRequestError::RequiredPayloadField(_)
            | OperationRequestError::PayloadFieldType(_)
            | OperationRequestError::EmptyString(_)
            | OperationRequestError::EmptyArray(_)
            | OperationRequestError::InvalidLeaseTtl),
        ) => schema_error(&error.to_string()),
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

fn io_error(context: &'static str, error: std::io::Error) -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::ExternalStateConflict,
        format!("authoritative project file I/O failed: {context}: {error}"),
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
    fn parse_route_reads_back_every_spelling_classify_can_write() {
        // `jobs::misc` recommends `CODING_PLAN`; flow.next must parse it.
        for spelling in ["CODING_PLAN", "coding-plan", "codingplan", "CodingPlan"] {
            assert_eq!(
                parse_route(spelling).expect("classify spelling parses"),
                DesignRoute::CodingPlan,
                "{spelling} must resolve to the CodingPlan route"
            );
        }
        assert_eq!(parse_route("DR").expect("DR parses"), DesignRoute::Dr);
        assert_eq!(
            parse_route("STORY").expect("STORY parses"),
            DesignRoute::Story
        );
        assert_eq!(
            parse_route("nonsense")
                .expect_err("an unknown route must fail closed")
                .code(),
            StableErrorCode::OperationSchemaInvalid
        );
    }

    #[test]
    fn route_handoff_advances_only_from_committed_document_markers() {
        let mut state = json!({
            "entryNode":"ROUTE",
            "routeApproved":true,
            "phase":"initialized",
            "scale":"large",
            "selectedDesign":"DR",
            "routeDecision":{
                "designRoute":"dr",
                "requiredSeries":["requirement-analysis","design-review","story"]
            },
            "routeDocuments":{},
            "executionPlan":{"goal":"","approved":false}
        });
        assert_eq!(
            route_handoff_action(&state)
                .expect("valid route state")
                .expect("fresh route action")["seriesKind"],
            "requirement-analysis"
        );

        state["routeDocuments"]["RA"] = Value::Bool(true);
        assert_eq!(
            route_handoff_action(&state)
                .expect("valid route state")
                .expect("DR action")["seriesKind"],
            "design-review"
        );

        state["routeDocuments"]["DR"] = Value::Bool(true);
        assert_eq!(
            route_handoff_action(&state)
                .expect("valid route state")
                .expect("Story action")["seriesKind"],
            "story"
        );

        state["routeDocuments"]["STORY"] = Value::Bool(true);
        state["routeDocuments"]["CODING_PLAN"] = Value::Bool(true);
        assert_eq!(
            route_handoff_action(&state)
                .expect("valid route state")
                .expect("plan preparation")["kind"],
            "prepare-execution-plan"
        );

        state["executionPlan"]["goal"] = Value::String("implement repair".to_owned());
        assert_eq!(
            route_handoff_action(&state)
                .expect("valid route state")
                .expect("plan approval")["kind"],
            "approve-execution-plan"
        );

        state["executionPlan"]["approved"] = Value::Bool(true);
        let advance = route_handoff_action(&state)
            .expect("valid route state")
            .expect("approved route phase handoff");
        assert_eq!(advance["kind"], "advance-route-phase");
        assert_eq!(advance["targetPhase"], "route-selected");
        assert_eq!(
            advance["submit"],
            json!({
                "method":"flow.next",
                "arguments":{"targetPhase":"route-selected"},
                "requiresIdempotencyKey":true
            })
        );

        for (current, expected) in [
            ("route-selected", "requirement-analyzed"),
            ("requirement-analyzed", "dr-generated"),
            ("dr-generated", "testcase-generated"),
            ("testcase-generated", "coding-process"),
            ("coding-process", "coding"),
        ] {
            state["phase"] = Value::String(current.to_owned());
            let advance = route_handoff_action(&state)
                .expect("valid route state")
                .expect("legal route phase handoff");
            assert_eq!(advance["kind"], "advance-route-phase", "{current}");
            assert_eq!(advance["targetPhase"], expected, "{current}");
        }

        state["phase"] = Value::String("coding".to_owned());
        let resume = route_handoff_action(&state)
            .expect("valid route state")
            .expect("approved execution handoff");
        assert_eq!(resume["kind"], "resume-approved-execution");
        assert_eq!(
            resume["submit"],
            json!({"method":"operation.execute","operation":"execution.resume"})
        );
    }

    #[test]
    fn route_handoff_does_not_override_a_flow_owned_transition_action() {
        let state = json!({
            "entryNode":"ROUTE",
            "routeApproved":true,
            "phase":"route-selected",
            "scale":"large",
            "selectedDesign":"DR",
            "routeDecision":{
                "designRoute":"dr",
                "requiredSeries":["requirement-analysis","design-review","story"]
            },
            "routeDocuments":{
                "RA":true,
                "DR":true,
                "STORY":true,
                "CODING_PLAN":true
            },
            "executionPlan":{"goal":"implement repair","approved":true}
        });
        let projection = json!({
            "phase":"route-selected",
            "nextAction":{
                "kind":"evaluate-gates",
                "targetPhase":"requirement-analyzed",
                "requiredGates":["G-00"]
            }
        });

        let decorated = decorate_route_handoff(projection.clone(), &state, None)
            .expect("pending transition projection decorates");

        assert_eq!(decorated, projection);
    }

    #[test]
    fn final_verification_handoff_does_not_override_flow_owned_actions() {
        let state = json!({
            "reviewSession":{
                "tier":"tier3",
                "status":"running",
                "reviewId":"review-001",
                "sourceRevision":113,
                "inputFingerprint":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "rulesetFingerprint":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "policyDigest":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                "inventoryGeneration":2
            }
        });

        for kind in ["evaluate-gates", "apply-transition"] {
            let projection = json!({
                "phase":"code-reviewed",
                "nextAction":{"kind":kind,"targetPhase":"completed"}
            });
            let decorated = decorate_route_handoff(projection.clone(), &state, None)
                .expect("flow-owned action decorates");
            assert_eq!(decorated, projection, "{kind}");
        }
    }

    #[test]
    fn tier3_review_projects_final_verification_instead_of_awaiting_agent_work() {
        let mut state = json!({
            "reviewSession":{
                "tier":"tier3",
                "status":"running",
                "reviewId":"review-001",
                "sourceRevision":113,
                "inputFingerprint":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "rulesetFingerprint":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "policyDigest":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                "inventoryGeneration":2
            }
        });
        let projection = decorate_route_handoff(
            json!({"phase":"initialized","nextAction":{"kind":"await-agent-work"}}),
            &state,
            None,
        )
        .expect("Tier 3 handoff projects");
        assert_eq!(
            projection["nextAction"]["kind"],
            "record-final-verification"
        );
        assert_eq!(projection["nextAction"]["sourceRevision"], 113);
        assert_eq!(
            projection["nextAction"]["submit"]["entrypoint"],
            "toolset.receipt.record"
        );
        assert_eq!(
            projection["nextAction"]["submit"]["arguments"]["finalizedEvidence"]["reviewId"],
            "review-001"
        );
        assert_eq!(
            projection["nextAction"]["submit"]["requires"],
            json!(["active-lease", "finalized-verification-evidence"])
        );

        let current = InputFingerprint::from_str(
            "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        )
        .expect("current Review input");
        let refresh = decorate_route_handoff(
            json!({"phase":"initialized","nextAction":{"kind":"await-agent-work"}}),
            &state,
            Some(current),
        )
        .expect("drifted Review input projects evidence refresh");
        assert_eq!(
            refresh["nextAction"]["kind"],
            "refresh-verification-evidence"
        );
        assert_eq!(
            refresh["nextAction"]["inputFingerprint"],
            current.to_string()
        );

        state["finalVerificationBinding"] = json!({
            "reviewId":"review-001",
            "inputFingerprint":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "rulesetFingerprint":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "policyDigest":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "inventoryGeneration":2,
            "toolsetJobId":"job-final",
            "receiptId":"receipt-final",
            "sourceRevision":113
        });
        state["toolsetReceiptRef"] = json!({
            "toolsetJobId":"job-final",
            "receiptId":"receipt-final"
        });
        let current_receipt = decorate_route_handoff(
            json!({"phase":"review-running","nextAction":{"kind":"await-agent-work"}}),
            &state,
            Some(
                InputFingerprint::from_str(
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                )
                .expect("session Review input"),
            ),
        )
        .expect("current receipt suppresses another final verification job");
        assert_eq!(current_receipt["nextAction"]["kind"], "await-agent-work");

        state["toolsetReceiptRef"]["toolsetJobId"] = json!("job-later-regular");
        let overwritten_receipt = decorate_route_handoff(
            json!({"phase":"review-running","nextAction":{"kind":"await-agent-work"}}),
            &state,
            Some(
                InputFingerprint::from_str(
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                )
                .expect("session Review input"),
            ),
        )
        .expect("a later regular receipt invalidates terminal provenance");
        assert_eq!(
            overwritten_receipt["nextAction"]["kind"],
            "record-final-verification"
        );
        state["toolsetReceiptRef"]["toolsetJobId"] = json!("job-final");

        state["finalVerificationBinding"]["reviewId"] = json!("review-stale");
        let stale_receipt = decorate_route_handoff(
            json!({"phase":"review-running","nextAction":{"kind":"await-agent-work"}}),
            &state,
            Some(
                InputFingerprint::from_str(
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                )
                .expect("session Review input"),
            ),
        )
        .expect("stale provenance projects another final verification job");
        assert_eq!(
            stale_receipt["nextAction"]["kind"],
            "record-final-verification"
        );
    }

    #[test]
    fn terminal_provenance_is_hidden_from_the_strict_toolset_validator() {
        let prepared = json!({
            "plan":{},
            "receipt":{},
            "preserveEvidenceManifest":true,
            "finalizedEvidenceBinding":{"reviewId":"review-001"}
        });

        let executable = toolset_execution_arguments("toolset.receipt.record", &prepared);

        assert!(executable.get("preserveEvidenceManifest").is_none());
        assert!(executable.get("finalizedEvidenceBinding").is_none());
        assert!(prepared.get("preserveEvidenceManifest").is_some());
        assert!(prepared.get("finalizedEvidenceBinding").is_some());
    }

    #[test]
    fn story_route_skips_the_dr_series() {
        let state = json!({
            "entryNode":"ROUTE",
            "routeApproved":true,
            "selectedDesign":"STORY",
            "routeDecision":{"requiredSeries":["requirement-analysis","story"]},
            "routeDocuments":{"RA":true},
            "executionPlan":{"goal":"","approved":false}
        });

        assert_eq!(
            route_handoff_action(&state)
                .expect("valid route state")
                .expect("Story action")["seriesKind"],
            "story"
        );
    }

    #[test]
    fn route_document_save_records_the_committed_series_artifact() {
        let request = OperationRequest {
            operation: OperationName::DocumentSave,
            workspace_id: Some(WorkspaceId::from_uuid(uuid::Uuid::from_u128(1))),
            project_key: Some(ProjectKey::new("ae-sdd").expect("project key")),
            work_item_id: Some(WorkItemId::new("ROUTE-TEST-001").expect("work item")),
            session_id: Some(SessionId::from_uuid(uuid::Uuid::from_u128(2))),
            lease_id: Some(LeaseId::from_uuid(uuid::Uuid::from_u128(3))),
            fencing_token: Some(FencingToken::new(1)),
            expected_revision: Some(StateRevision::new(2)),
            idempotency_key: Some("route-save-ra".into()),
            confirmation: None,
            dry_run: false,
            payload: json!({"intent":"RA","contentFile":"ra-input.md"}),
        };
        let request = ValidatedOperationRequest::validate(request).expect("valid document save");
        let mut state = json!({
            "entryNode":"ROUTE",
            "stateMachineName":"ROUTE-TEST-001",
            "activeStory":null,
            "routeDocuments":{}
        });

        apply_mutation(&mut state, &request, None).expect("route document marker commits");

        assert_eq!(state["routeDocuments"]["RA"], true);

        let story_request = OperationRequest {
            operation: OperationName::DocumentSave,
            workspace_id: Some(WorkspaceId::from_uuid(uuid::Uuid::from_u128(1))),
            project_key: Some(ProjectKey::new("ae-sdd").expect("project key")),
            work_item_id: Some(WorkItemId::new("ROUTE-TEST-001").expect("work item")),
            session_id: Some(SessionId::from_uuid(uuid::Uuid::from_u128(2))),
            lease_id: Some(LeaseId::from_uuid(uuid::Uuid::from_u128(3))),
            fencing_token: Some(FencingToken::new(1)),
            expected_revision: Some(StateRevision::new(3)),
            idempotency_key: Some("route-save-story".into()),
            confirmation: None,
            dry_run: false,
            payload: json!({"intent":"STORY","contentFile":"story-input.md"}),
        };
        let story_request =
            ValidatedOperationRequest::validate(story_request).expect("valid Story save");
        apply_mutation(&mut state, &story_request, None).expect("Story scope commits");
        assert_eq!(state["routeDocuments"]["STORY"], true);
        // Without a `docId` the save must not invent a Story identity — in
        // particular it must never bind the ROUTE state machine name.
        assert_eq!(state["activeStory"], Value::Null);
    }

    #[test]
    fn route_story_save_binds_active_story_from_the_doc_id() {
        let request = OperationRequest {
            operation: OperationName::DocumentSave,
            workspace_id: Some(WorkspaceId::from_uuid(uuid::Uuid::from_u128(1))),
            project_key: Some(ProjectKey::new("ae-sdd").expect("project key")),
            work_item_id: Some(WorkItemId::new("ROUTE-TEST-001").expect("work item")),
            session_id: Some(SessionId::from_uuid(uuid::Uuid::from_u128(2))),
            lease_id: Some(LeaseId::from_uuid(uuid::Uuid::from_u128(3))),
            fencing_token: Some(FencingToken::new(1)),
            expected_revision: Some(StateRevision::new(2)),
            idempotency_key: Some("route-save-story-doc".into()),
            confirmation: None,
            dry_run: false,
            payload: json!({
                "intent":"STORY",
                "contentFile":"story-input.md",
                "docId":"STORY-ROUTE-TEST-001",
            }),
        };
        let request = ValidatedOperationRequest::validate(request).expect("valid Story save");
        let mut state = json!({
            "entryNode":"ROUTE",
            "stateMachineName":"ROUTE-TEST-001",
            "activeStory":null,
            "phase":"testcase-generated",
            "currentPhase":"testcase-generated",
            "routeDocuments":{},
            "documentPaths":{"STORY":"ae-sdd-doc/Story/ROUTE-TEST-001.md"}
        });

        let mut mismatched = state.clone();
        mismatched["currentPhase"] = Value::String("coding".to_owned());
        let before = mismatched.clone();
        let error = apply_mutation(&mut mismatched, &request, None)
            .expect_err("a mismatched ROUTE phase mirror must fail closed");
        assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
        assert_eq!(mismatched, before);

        apply_mutation(&mut state, &request, None).expect("Story scope commits");

        assert_eq!(state["routeDocuments"]["STORY"], true);
        assert_eq!(state["activeStory"], "STORY-ROUTE-TEST-001");
        assert_eq!(
            state["storyStates"]["STORY-ROUTE-TEST-001"]["docPath"],
            "ae-sdd-doc/Story/ROUTE-TEST-001.md"
        );
        assert_eq!(
            state["storyStates"]["STORY-ROUTE-TEST-001"]["phase"],
            "testcase-generated"
        );
        assert_eq!(
            state["storyStates"]["STORY-ROUTE-TEST-001"]["currentPhase"],
            "testcase-generated"
        );
    }

    #[test]
    fn authoritative_state_snapshot_rejects_same_revision_drift() {
        let cached = json!({"revision":7,"phase":"coding"});
        let fresh = serde_json::to_vec(&json!({"revision":7,"phase":"test-running"}))
            .expect("fresh state serializes");

        let error = authoritative_state_snapshot(&cached, &fresh)
            .expect_err("same revision with different content must fail closed");

        assert_eq!(error.code(), StableErrorCode::ExternalStateConflict);
    }

    /// A caller cannot fix a rejected payload from a message that names neither
    /// the field nor the expected shape. The validator already produced that
    /// detail; only the wildcard mapping threw it away.
    #[test]
    fn a_rejected_payload_names_the_offending_field() {
        let missing = operation_error(OperationServiceError::InvalidRequest(
            OperationRequestError::RequiredPayloadField("owner"),
        ));
        assert_eq!(missing.code(), StableErrorCode::OperationSchemaInvalid);
        assert!(
            missing.message().contains("owner"),
            "a missing required field must be named: {}",
            missing.message()
        );

        let unknown = operation_error(OperationServiceError::InvalidRequest(
            OperationRequestError::UnknownPayloadField("parameters".to_owned()),
        ));
        assert!(
            unknown.message().contains("parameters"),
            "an unknown field must be named: {}",
            unknown.message()
        );

        let wrong_type = operation_error(OperationServiceError::InvalidRequest(
            OperationRequestError::PayloadFieldType("ttlSeconds"),
        ));
        assert!(
            wrong_type.message().contains("ttlSeconds"),
            "a wrong-typed field must be named: {}",
            wrong_type.message()
        );
    }

    fn creation_workspace(root: &std::path::Path) -> BusinessWorkspace {
        BusinessWorkspace {
            workspace_id: "ws-create-test".to_owned(),
            canonical_root: root.to_string_lossy().into_owned(),
            project_key: "ae-sdd".to_owned(),
            mode: WorkspaceMode::RustSoleWriter,
            agent_role: None,
            agent_grant: None,
            caller_kind: None,
            inventory_generation: 1,
        }
    }

    fn creation_request(work_item: &str, payload: Value) -> OperationRequest {
        OperationRequest {
            operation: OperationName::WorkItemCreate,
            workspace_id: None,
            project_key: None,
            work_item_id: Some(
                ae_sdd_domain::WorkItemId::new(work_item.to_owned()).expect("work item id"),
            ),
            session_id: None,
            lease_id: None,
            fencing_token: None,
            expected_revision: None,
            idempotency_key: None,
            confirmation: None,
            dry_run: false,
            payload,
        }
    }

    fn anonymous_creation_request(entry_node: &str, key: &str) -> OperationRequest {
        OperationRequest {
            work_item_id: None,
            idempotency_key: Some(key.to_owned().into_boxed_str()),
            ..creation_request("ignored", json!({"entryNode":entry_node}))
        }
    }

    /// A created state has to be openable by the store on the very next call, so
    /// it must carry both authority fields the Python creator derived instead of
    /// storing.
    #[test]
    fn a_created_state_carries_the_fields_the_store_requires_to_open_it() {
        let root = tempfile::TempDir::new().expect("workspace root");
        let workspace = creation_workspace(root.path());
        let response = create_work_item(
            &workspace,
            &creation_request("Story-STORY-CREATE-UNIT-001", json!({"entryNode":"STORY"})),
        )
        .expect("creation succeeds");

        let relative = response["data"]["statePath"]
            .as_str()
            .expect("statePath is reported");
        let bytes = std::fs::read(root.path().join(relative)).expect("state is on disk");
        ae_sdd_store::StateAuthority::inspect(&bytes)
            .expect("the store must accept a freshly created state");

        let state: Value = serde_json::from_slice(&bytes).expect("state is JSON");
        assert_eq!(state["revision"], 0);
        assert_eq!(state["lastFencingToken"], 0);
        assert_eq!(state["stateMachineName"], "Story-STORY-CREATE-UNIT-001");
        assert_eq!(
            state["stateMachineId"],
            format!(
                "{}-Story-STORY-CREATE-UNIT-001",
                state["stateUuid"].as_str().expect("stateUuid")
            ),
            "the directory identity must be the uuid-prefixed business name"
        );
        assert!(
            state.get("storyStates").is_some(),
            "entryNode STORY must open the storyStates container"
        );
    }

    /// The flow authority and the session context projection both read a
    /// created state on the very next call — the bootstrap Hook right after
    /// `workitem.create` — and both derive the snapshot from the Work Item
    /// phase. A state without it creates fine and then fails every read, the
    /// exact first-turn deadlock the create is meant to unblock.
    #[test]
    fn a_route_intake_exposes_analysis_instead_of_guessing_a_route() {
        let root = tempfile::TempDir::new().expect("workspace root");
        let workspace = creation_workspace(root.path());
        let response = create_work_item(
            &workspace,
            &creation_request("ROUTE-CREATE-UNIT-002", json!({"entryNode":"ROUTE"})),
        )
        .expect("creation succeeds");
        let work_item = response["data"]["workItemId"]
            .as_str()
            .expect("business key is reported");
        let state_path = response["data"]["statePath"]
            .as_str()
            .expect("statePath is reported");
        let bytes = std::fs::read(root.path().join(state_path)).expect("state is on disk");
        let state: Value = serde_json::from_slice(&bytes).expect("state is JSON");
        assert_eq!(
            state["documentPaths"]["STORY"],
            "ae-sdd-doc/Story/ROUTE-CREATE-UNIT-002.md"
        );
        assert_eq!(
            state["documentPaths"]["CODING_PLAN"],
            "ae-sdd-doc/Coding/ROUTE-CREATE-UNIT-002/ROUTE-CREATE-UNIT-002-CodingPlan.md"
        );

        let missing = flow_input(
            &workspace,
            &state,
            work_item,
            EventStoreId::from_uuid(Uuid::from_u128(41)),
        )
        .expect_err("route selection must not default to large/DR");
        assert_eq!(missing.code(), StableErrorCode::OperationSchemaInvalid);

        let projection = route_pending_projection(&state, work_item)
            .expect("a fresh ROUTE item must expose a typed analysis action");
        assert_eq!(projection["nextAction"]["kind"], "analyze-route");
        assert_eq!(
            projection["nextAction"]["submit"]["method"],
            "operation.execute"
        );
        assert_eq!(
            projection["nextAction"]["submit"]["operation"],
            "route.decide"
        );
    }

    /// A bootstrap caller has no Work Item yet and therefore cannot invent a
    /// business name for it; with `workItemId` absent the daemon has to mint
    /// one that the rest of the system can resolve on the very next call.
    #[test]
    fn an_anonymous_create_mints_a_resolvable_business_name() {
        let root = tempfile::TempDir::new().expect("workspace root");
        let workspace = creation_workspace(root.path());
        let request = anonymous_creation_request("STORY", "anonymous-create-resolvable");

        let response = create_work_item(&workspace, &request).expect("creation succeeds");

        let minted = response["data"]["workItemId"]
            .as_str()
            .expect("the minted business name is reported as workItemId");
        assert!(
            minted.starts_with("STORY-"),
            "the minted name derives from the entry node: {minted}"
        );
        let suffix = &minted["STORY-".len()..];
        assert_eq!(suffix.len(), 8, "eight hex chars follow: {minted}");
        assert!(
            suffix
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "the suffix is lowercase hex: {minted}"
        );
        assert!(
            ae_sdd_domain::WorkItemId::new(minted.to_owned()).is_ok(),
            "the minted name must satisfy the shared WorkItemId rules: {minted}"
        );
        assert_eq!(
            response["data"]["stateMachineName"].as_str(),
            Some(minted),
            "the business name and stateMachineName are one key"
        );
        let state_machine_id = response["data"]["stateMachineId"]
            .as_str()
            .expect("the uuid-prefixed directory identity is reported");
        assert!(
            state_machine_id.ends_with(minted),
            "the state machine id keeps its uuid-prefixed shape: {state_machine_id}"
        );
        let located = read_state(&workspace, minted).expect("the minted name must resolve");
        assert_eq!(
            located.value["stateMachineName"].as_str(),
            Some(minted),
            "read_state resolves a freshly minted name on the next call"
        );
    }

    /// Two anonymous creates in one workspace must not collide: a collision
    /// would hand two Work Items one business name and `read_state` would fail
    /// ambiguous for both.
    #[test]
    fn minted_business_names_do_not_collide() {
        let root = tempfile::TempDir::new().expect("workspace root");
        let workspace = creation_workspace(root.path());
        let first = create_work_item(
            &workspace,
            &anonymous_creation_request("PRD", "anonymous-create-first"),
        )
        .expect("first creation succeeds");
        let second = create_work_item(
            &workspace,
            &anonymous_creation_request("PRD", "anonymous-create-second"),
        )
        .expect("second creation succeeds");

        assert_ne!(
            first["data"]["workItemId"], second["data"]["workItemId"],
            "each anonymous create mints a fresh business name"
        );
    }

    #[test]
    fn anonymous_create_replays_the_original_item_before_session_binding() {
        let root = tempfile::TempDir::new().expect("workspace root");
        let workspace = creation_workspace(root.path());
        let request = anonymous_creation_request("ROUTE", "bootstrap-crash-window");

        let first = create_work_item(&workspace, &request).expect("first create commits");
        let replay = create_work_item(&workspace, &request).expect("retry replays create");

        assert_eq!(replay, first, "retry returns the original result envelope");
        let states = fs::read_dir(root.path().join(".auto-engineering"))
            .expect("authority directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.path().join("state.json").is_file())
            .count();
        assert_eq!(states, 1, "crash-window replay cannot mint a second ROUTE");

        let conflict = anonymous_creation_request("STORY", "bootstrap-crash-window");
        let error = create_work_item(&workspace, &conflict)
            .expect_err("one key cannot authorize a different bootstrap payload");
        assert_eq!(error.code(), StableErrorCode::IdempotencyKeyReused);
    }

    /// A caller that does choose the business name keeps today's contract
    /// exactly: the response reports that name as the key to use downstream.
    #[test]
    fn a_caller_supplied_business_name_is_reported_as_the_work_item_id() {
        let root = tempfile::TempDir::new().expect("workspace root");
        let workspace = creation_workspace(root.path());

        let response = create_work_item(
            &workspace,
            &creation_request("Story-STORY-NAMED-001", json!({"entryNode":"STORY"})),
        )
        .expect("creation succeeds");

        assert_eq!(
            response["data"]["workItemId"].as_str(),
            Some("Story-STORY-NAMED-001"),
            "workItemId is the business key agents use downstream, not the directory id"
        );
        let state_machine_id = response["data"]["stateMachineId"]
            .as_str()
            .expect("stateMachineId is reported");
        assert!(
            state_machine_id.ends_with("-Story-STORY-NAMED-001"),
            "the directory identity stays uuid-prefixed: {state_machine_id}"
        );
        let located = read_state(&workspace, "Story-STORY-NAMED-001")
            .expect("the short business key resolves without the directory UUID");
        assert_eq!(located.value["stateMachineName"], "Story-STORY-NAMED-001");
    }

    #[test]
    fn duplicate_directories_for_one_short_key_return_an_explicit_ambiguity() {
        let root = tempfile::TempDir::new().expect("workspace root");
        let workspace = creation_workspace(root.path());
        let response = create_work_item(
            &workspace,
            &creation_request("Story-STORY-AMBIGUOUS", json!({"entryNode":"STORY"})),
        )
        .expect("creation succeeds");
        let original = root
            .path()
            .join(response["data"]["statePath"].as_str().expect("state path"));
        let duplicate = root
            .path()
            .join(".auto-engineering/duplicate-Story-STORY-AMBIGUOUS/state.json");
        fs::create_dir_all(duplicate.parent().expect("duplicate parent"))
            .expect("duplicate directory");
        fs::copy(original, duplicate).expect("duplicate state fixture");

        let error = match read_state(&workspace, "Story-STORY-AMBIGUOUS") {
            Ok(_) => panic!("duplicate business keys must fail closed"),
            Err(error) => error,
        };
        assert_eq!(error.code(), StableErrorCode::ScopeAmbiguous);
        assert_eq!(
            error.message(),
            "Work Item key matched multiple state directories"
        );
    }

    #[test]
    fn a_short_key_with_no_state_directory_returns_project_mismatch() {
        let root = tempfile::TempDir::new().expect("workspace root");
        let workspace = creation_workspace(root.path());

        let absent = match read_state(&workspace, "STORY-MISSING") {
            Ok(_) => panic!("an absent authority directory must be a zero match"),
            Err(error) => error,
        };
        assert_eq!(absent.code(), StableErrorCode::ProjectMismatch);

        fs::create_dir(root.path().join(".auto-engineering")).expect("empty authority directory");
        let empty = match read_state(&workspace, "STORY-MISSING") {
            Ok(_) => panic!("an empty authority directory must be a zero match"),
            Err(error) => error,
        };
        assert_eq!(empty.code(), StableErrorCode::ProjectMismatch);
    }

    /// Two Work Items answering to one business name would make `read_state`
    /// fail ambiguous forever, so the second attempt has to lose.
    #[test]
    fn creating_the_same_business_name_twice_is_refused() {
        let root = tempfile::TempDir::new().expect("workspace root");
        let workspace = creation_workspace(root.path());
        let request = || creation_request("Story-STORY-DUP-001", json!({"entryNode":"STORY"}));
        create_work_item(&workspace, &request()).expect("first creation succeeds");

        let error = create_work_item(&workspace, &request()).expect_err("second must be refused");

        assert_eq!(error.code(), StableErrorCode::ScopeAmbiguous);
    }

    /// `BUG` and `CONFIG` run the flat micro chain. Building a nested skeleton
    /// for them would hand every later reader a shape it does not expect.
    #[test]
    fn a_flat_chain_entry_node_is_refused_rather_than_given_a_nested_skeleton() {
        let root = tempfile::TempDir::new().expect("workspace root");
        let workspace = creation_workspace(root.path());

        let error = create_work_item(
            &workspace,
            &creation_request("Bug-BUG-FLAT-001", json!({"entryNode":"BUG"})),
        )
        .expect_err("BUG must be refused");

        assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
        assert!(
            std::fs::read_dir(root.path().join(".auto-engineering")).is_err(),
            "a refused creation must leave no directory behind"
        );
    }

    /// A traversal segment would place the directory outside
    /// `.auto-engineering/`. The guard that actually stops it is the domain
    /// type, upstream of creation, so that is where this asserts; the check
    /// inside `create_work_item` is defence in depth for any future caller that
    /// does not go through `WorkItemId`.
    #[test]
    fn a_traversing_work_item_name_cannot_even_be_named() {
        for candidate in ["../escaped", "a/b", "a\\b", "..", "x/../y"] {
            assert!(
                ae_sdd_domain::WorkItemId::new(candidate.to_owned()).is_err(),
                "{candidate} must not be constructible as a WorkItemId"
            );
        }
    }

    /// `RpcError` is a contractually redacted surface. Serde's message can echo
    /// payload values, so the one variant wrapping it stays collapsed even
    /// though its siblings are now surfaced.
    #[test]
    fn a_payload_value_never_reaches_the_redacted_error_surface() {
        let serde_error = serde_json::from_str::<u32>("\"super-secret-token\"")
            .expect_err("a string is not a u32");
        let error = operation_error(OperationServiceError::InvalidRequest(
            OperationRequestError::CanonicalizePayload(serde_error),
        ));

        assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
        assert!(
            !error.message().contains("super-secret-token"),
            "a payload value must never reach the redacted surface: {}",
            error.message()
        );
    }

    /// The projection asset region hands the Agent references, never bodies:
    /// path, kind and the exact content digest, bounded and deterministic.
    #[test]
    fn projection_asset_refs_reference_existing_assets_without_bodies() {
        let root = tempfile::TempDir::new().expect("workspace root");
        let workspace = creation_workspace(root.path());
        std::fs::create_dir_all(root.path().join("constraints")).expect("constraints dir");
        std::fs::write(
            root.path().join("constraints/README.md"),
            b"# constraints\n",
        )
        .expect("constraints index");
        std::fs::create_dir_all(root.path().join("source/skills/phase2-coding"))
            .expect("skill dir");
        std::fs::write(
            root.path()
                .join("source/skills/phase2-coding/coding-skill.md"),
            b"# coding\n",
        )
        .expect("skill");

        let refs = projection_asset_refs(&workspace, Some("coding"));

        assert_eq!(refs.len(), 2, "constraints index plus the phase skill");
        assert_eq!(refs[0]["kind"], "constraints-index");
        assert_eq!(refs[0]["path"], "constraints/README.md");
        assert_eq!(
            refs[0]["sha256"],
            hex::encode(sha2::Sha256::digest(b"# constraints\n"))
        );
        assert_eq!(refs[1]["kind"], "methodology-skill");
        assert_eq!(
            refs[1]["path"],
            "source/skills/phase2-coding/coding-skill.md"
        );
        assert_eq!(
            refs[1]["sha256"],
            hex::encode(sha2::Sha256::digest(b"# coding\n"))
        );
        for reference in &refs {
            let keys: Vec<&String> = reference
                .as_object()
                .expect("reference is an object")
                .keys()
                .collect();
            assert_eq!(
                keys,
                ["kind", "path", "sha256"],
                "a reference carries no body fields"
            );
        }
    }

    /// Missing files, unknown phases and oversized assets shrink the region;
    /// they never fail the projection or invent a digest.
    #[test]
    fn projection_asset_refs_omit_unavailable_assets() {
        let root = tempfile::TempDir::new().expect("workspace root");
        let workspace = creation_workspace(root.path());
        assert!(
            projection_asset_refs(&workspace, Some("coding")).is_empty(),
            "an empty workspace projects no references"
        );

        std::fs::create_dir_all(root.path().join("constraints")).expect("constraints dir");
        std::fs::write(
            root.path().join("constraints/README.md"),
            b"# constraints\n",
        )
        .expect("constraints index");
        let refs = projection_asset_refs(&workspace, Some("paused"));
        assert_eq!(refs.len(), 1, "an unmapped phase contributes no skill ref");
        assert_eq!(refs[0]["kind"], "constraints-index");
        let refs = projection_asset_refs(&workspace, None);
        assert_eq!(refs.len(), 1, "an absent phase contributes no skill ref");
    }

    /// Every lifecycle phase that names a skill maps onto a real repository
    /// path shape so the reference is resolvable by the receiving Agent.
    #[test]
    fn every_phase_skill_mapping_stays_inside_the_source_tree() {
        for (phase, path) in [
            (
                "requirement-analyzed",
                "source/skills/phase1-design/requirement-analysis-skill.md",
            ),
            (
                "dr-generated",
                "source/skills/phase1-design/dr-generate-skill.md",
            ),
            (
                "story-generated",
                "source/skills/phase1-design/story-generate-skill.md",
            ),
            (
                "testcase-generated",
                "source/skills/phase1-design/testcase-generate-skill.md",
            ),
            (
                "coding-process",
                "source/skills/phase2-coding/coding-process-skill.md",
            ),
            ("coding", "source/skills/phase2-coding/coding-skill.md"),
            (
                "test-running",
                "source/skills/phase3-review/test-generate-skill.md",
            ),
            (
                "code-reviewed",
                "source/skills/phase3-review/code-review-skill.md",
            ),
        ] {
            assert_eq!(phase_skill_path(phase), Some(path), "{phase} mapping");
        }
        for unmapped in [
            "initialized",
            "route-selected",
            "completed",
            "paused",
            "unknown",
        ] {
            assert_eq!(phase_skill_path(unmapped), None, "{unmapped} mapping");
        }
    }

    /// Once `execution.resume` seeds the `executionRuntime` capsule, every
    /// projection-time authority load (session.open, gates, flow, resume
    /// itself) observes the completion digests. An approved plan may
    /// legitimately name a directory among its changed paths, and the
    /// filesystem is only observed there, so the directory must hash a marker
    /// instead of failing the load and bricking the Work Item.
    #[test]
    fn a_seeded_execution_runtime_tolerates_a_directory_changed_path() {
        let root = tempfile::TempDir::new().expect("workspace root");
        let workspace = creation_workspace(root.path());
        std::fs::create_dir_all(root.path().join("src/generated")).expect("changed dir");
        let state = json!({
            "revision": 7,
            "scale": "large",
            "routeDecision": {"designRoute": "story"},
            "storyStates": {"STORY-DIR-1": {"currentPhase": "coding"}},
            "executionPlan": {
                "goal": "implement",
                "approved": true,
                "changedPaths": ["src/generated"],
            },
            "executionRuntime": {
                "queueDigest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                "activeSliceOrdinal": 1,
            },
        });

        flow_input(
            &workspace,
            &state,
            "STORY-DIR-1",
            EventStoreId::from_uuid(Uuid::from_u128(42)),
        )
        .expect("a directory changed path must not wedge the authority load");
    }

    /// Real content still binds the code digest: files hash their bytes,
    /// directories and missing paths hash stable markers, and the observation
    /// is deterministic across calls.
    #[test]
    fn a_code_digest_hashes_content_and_stable_markers() {
        let root = tempfile::TempDir::new().expect("workspace root");
        std::fs::create_dir_all(root.path().join("src/generated")).expect("changed dir");
        std::fs::write(root.path().join("src/lib.rs"), b"pub fn one() {}\n").expect("file");
        let state = json!({
            "executionPlan": {
                "changedPaths": ["src/generated", "src/lib.rs", "src/missing.rs"],
            },
        });

        let first = code_digest(root.path(), &state).expect("digest is observation-tolerant");
        let second = code_digest(root.path(), &state).expect("digest is deterministic");
        assert_eq!(first, second, "the same observation hashes identically");

        std::fs::write(root.path().join("src/lib.rs"), b"pub fn two() {}\n")
            .expect("rewritten file");
        let changed = code_digest(root.path(), &state).expect("digest after rewrite");
        assert_ne!(
            first, changed,
            "real content still binds: rewriting a file rolls the digest"
        );
    }

    /// A sealed evidence manifest that cannot be read (a directory where the
    /// file should be) hashes an explicit unreadable sentinel instead of
    /// failing, and stays distinct from the missing-manifest sentinel.
    #[test]
    fn a_verification_digest_marks_an_unreadable_manifest() {
        let root = tempfile::TempDir::new().expect("workspace root");
        let state = json!({"storyStates": {"STORY-1": {}}});
        let missing = verification_digest(root.path(), &state, "STORY-1")
            .expect("a missing manifest hashes the missing sentinel");

        std::fs::create_dir_all(
            root.path()
                .join(".auto-engineering/STORY-1/evidence/manifest.json"),
        )
        .expect("manifest directory");
        let unreadable = verification_digest(root.path(), &state, "STORY-1")
            .expect("an unreadable manifest hashes the unreadable sentinel");

        assert_ne!(
            missing, unreadable,
            "an unreadable manifest must not collapse into the missing sentinel"
        );
    }

    /// The file-I/O error surface names the artifact class and keeps the OS
    /// error, so a caller can tell which authoritative read failed and why.
    #[test]
    fn an_io_error_keeps_the_os_error_and_artifact_class() {
        let error = io_error(
            "read changed path",
            std::io::Error::new(std::io::ErrorKind::IsADirectory, "is a directory"),
        );

        assert_eq!(error.code(), StableErrorCode::ExternalStateConflict);
        assert!(
            error.message().contains("read changed path"),
            "the artifact class is named: {}",
            error.message()
        );
        assert!(
            error.message().contains("is a directory"),
            "the OS error survives: {}",
            error.message()
        );
    }
}
