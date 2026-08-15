use std::collections::BTreeMap;
use std::sync::Mutex;

use ae_sdd_domain::{AgentRole, EventStoreId, ScopedGrant};
use ae_sdd_protocol::{RequestParams, RpcMethod, StableErrorCode, WorkspaceMode};
use serde_json::Value;

use crate::{
    ContextProjectionInput, DurableEvent, ExecutionCheckpointRecord, ExecutionCheckpointScope,
    ExecutionResourceLeaseOutcomeV1, ExecutionResourceLeaseRecordV1,
    ExecutionResourceLeaseRequestV1, IdempotencyReceipt, PreparedExecutionHookV1, RuntimeError,
    RuntimeIdentityKind, RuntimeIdentitySnapshot, RuntimeIdentityTransition, RuntimeJobRecord,
    RuntimeJobTransition, RuntimeResult,
};

/// Clock used for deadlines, TTL, and deterministic tests.
pub trait ClockPort: Send + Sync {
    /// Current Unix time in milliseconds.
    fn now_unix_ms(&self) -> u64;
}

/// Canonical workspace identity resolved by the platform boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedWorkspace {
    /// Canonical absolute root used as the alias identity.
    pub canonical_root: String,
    /// True when the root is inside an explicitly allowed parent.
    pub inside_allowed_root: bool,
}

/// Filesystem-backed path resolution port.
pub trait WorkspaceResolverPort: Send + Sync {
    /// Canonicalizes and validates a requested workspace root.
    fn resolve(&self, requested_root: &str) -> RuntimeResult<ResolvedWorkspace>;
}

/// Runtime-derived authoritative workspace context passed to business adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BusinessWorkspace {
    /// Stable registered identity.
    pub workspace_id: String,
    /// Canonical root established by the path resolver.
    pub canonical_root: String,
    /// Exact registered project identity.
    pub project_key: String,
    /// Daemon-owned writer migration mode. Callers cannot supply this value.
    pub mode: WorkspaceMode,
    /// Daemon-verified session role for Agent business calls.
    pub agent_role: Option<AgentRole>,
    /// Daemon-verified operation/path grant for Agent business calls.
    pub agent_grant: Option<ScopedGrant>,
    /// Handshake-authenticated caller kind; absent only for daemon-internal work.
    pub caller_kind: Option<ae_sdd_protocol::ClientKind>,
    /// Current daemon-owned inventory generation.
    pub inventory_generation: u64,
}

/// Daemon-captured session lineage bound to one durable background job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundJobIdentity {
    /// Originating durable runtime job.
    pub job_id: String,
    /// Runtime boot executing the job.
    pub boot_id: String,
    /// Physical session that submitted the job.
    pub session_id: String,
    /// Root orchestration session for the lineage.
    pub root_session_id: String,
    /// Physical delegation for non-root sessions.
    pub delegation_id: Option<String>,
    /// Session context generation captured at submission.
    pub context_generation: u64,
    /// Immutable physical-attestation reference captured at submission.
    pub attestation_ref: String,
    /// Digest of the captured physical attestation.
    pub attestation_digest: String,
    /// Digest of the complete captured identity projection.
    pub identity_digest: String,
    /// Durable job submission idempotency key.
    pub idempotency_key: String,
}

/// Durable runtime metadata and event port.
///
/// Implementations must commit event sequence allocation and record insertion
/// atomically. Records are versioned JSON values; authoritative project state
/// remains outside this metadata store.
pub trait PersistencePort: Send + Sync {
    /// Durable event-store epoch.
    fn event_store_id(&self) -> RuntimeResult<EventStoreId>;
    /// Latest committed global event sequence.
    fn latest_event_sequence(&self) -> RuntimeResult<u64>;
    /// Appends one bounded event and allocates the next global sequence.
    fn append_event(&self, event: DurableEvent) -> RuntimeResult<DurableEvent>;
    /// Atomically appends an event and stores its idempotency receipt.
    ///
    /// Implementations allocate one sequence, apply it to both values, and
    /// commit both records in one transaction.
    fn commit_event_and_receipt(
        &self,
        event: DurableEvent,
        receipt: IdempotencyReceipt,
    ) -> RuntimeResult<(DurableEvent, IdempotencyReceipt)>;
    /// Atomically commits one supervised execution event, its idempotency
    /// receipt, and the per-session prepared Hook barrier record.
    fn commit_prepared_execution_hook(
        &self,
        event: DurableEvent,
        receipt: IdempotencyReceipt,
        record: PreparedExecutionHookV1,
    ) -> RuntimeResult<(DurableEvent, IdempotencyReceipt, PreparedExecutionHookV1)>;
    /// Reads an ordered bounded event page after a cursor.
    fn events_after(&self, after: u64, limit: usize) -> RuntimeResult<Vec<DurableEvent>>;
    /// Oldest available event sequence, or zero for an empty store.
    fn oldest_event_sequence(&self) -> RuntimeResult<u64>;
    /// Reads an idempotency receipt by namespaced key.
    fn load_receipt(&self, scope: &str, key: &str) -> RuntimeResult<Option<IdempotencyReceipt>>;
    /// Atomically stores a receipt, rejecting a conflicting existing payload.
    fn store_receipt(&self, receipt: &IdempotencyReceipt) -> RuntimeResult<()>;
    /// Reads one durable versioned aggregate projection.
    fn load_record(&self, namespace: &str, key: &str) -> RuntimeResult<Option<Value>>;
    /// Lists all records in one bounded runtime namespace in key order.
    fn list_records(&self, namespace: &str) -> RuntimeResult<Vec<(String, Value)>>;
    /// Atomically upserts one durable versioned aggregate projection.
    fn store_record(&self, namespace: &str, key: &str, value: &Value) -> RuntimeResult<()>;
    /// Deletes one rebuildable runtime projection. Missing records are a
    /// successful no-op.
    fn delete_record(&self, namespace: &str, key: &str) -> RuntimeResult<()>;
    /// Atomically acquires or re-enters one durable execution resource lease.
    fn acquire_execution_resource_lease(
        &self,
        request: &ExecutionResourceLeaseRequestV1,
    ) -> RuntimeResult<ExecutionResourceLeaseOutcomeV1>;
    /// Releases a durable lease only when boot and session still match.
    fn release_execution_resource_lease(
        &self,
        resource: &str,
        boot_id: &str,
        session_id: &str,
    ) -> RuntimeResult<()>;
    /// Atomically commits a typed identity bundle and its durable receipt.
    fn commit_identity_bundle(
        &self,
        transition: RuntimeIdentityTransition,
    ) -> RuntimeResult<RuntimeIdentitySnapshot>;
    /// Lists the latest typed identity snapshots for one aggregate family.
    fn list_identity_snapshots(
        &self,
        kind: RuntimeIdentityKind,
    ) -> RuntimeResult<Vec<RuntimeIdentitySnapshot>>;
    /// Atomically commits one typed job CAS transition and its event.
    fn commit_job_transition(
        &self,
        transition: RuntimeJobTransition,
    ) -> RuntimeResult<RuntimeJobRecord>;
    /// Loads one typed runtime job.
    fn load_job(&self, job_id: &str) -> RuntimeResult<Option<RuntimeJobRecord>>;
    /// Lists typed runtime jobs in stable identity order.
    fn list_jobs(&self) -> RuntimeResult<Vec<RuntimeJobRecord>>;
    /// Loads the rebuildable execution-supervisor checkpoint for one scope.
    ///
    /// The row is rebuildable runtime metadata only: it accelerates a daemon
    /// restart and never outranks the project authority.  Callers validate it
    /// against the authority cursor with [`ExecutionCheckpointRecord::recover`].
    fn load_execution_checkpoint(
        &self,
        scope: &ExecutionCheckpointScope,
    ) -> RuntimeResult<Option<ExecutionCheckpointRecord>>;
    /// Atomically upserts the rebuildable execution-supervisor checkpoint.
    fn store_execution_checkpoint(&self, record: &ExecutionCheckpointRecord) -> RuntimeResult<()>;
    /// Discards the rebuildable execution-supervisor checkpoint for one scope.
    ///
    /// Used when the project authority disagrees with the cached row; the
    /// discard must never touch project state.
    fn discard_execution_checkpoint(&self, scope: &ExecutionCheckpointScope) -> RuntimeResult<()>;
}

/// Business-operation boundary for authoritative state, Gates, and jobs.
///
/// The runtime never substitutes a local Gate/state implementation when this
/// port rejects or is unavailable.
pub trait BusinessOperationPort: Send + Sync {
    /// Executes a typed post-handshake method at the authoritative boundary.
    fn execute(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
        workspace: Option<&BusinessWorkspace>,
    ) -> RuntimeResult<Value>;

    /// Builds one authoritative role-aware Hook/context projection off the fast path.
    fn project_context(
        &self,
        workspace: &BusinessWorkspace,
        work_item_id: &str,
        session_id: &str,
        role: AgentRole,
    ) -> RuntimeResult<ContextProjectionInput>;

    /// Executes one already-admitted bounded background job.
    fn execute_job(
        &self,
        workspace: &BusinessWorkspace,
        entrypoint: &str,
        arguments: &Value,
    ) -> RuntimeResult<Value>;

    /// Executes a job with the trusted work-item identity captured by the
    /// scheduler. The default preserves adapters whose jobs are workspace-only;
    /// adapters with work-item diagnostics override this method explicitly.
    fn execute_bound_job(
        &self,
        workspace: &BusinessWorkspace,
        work_item_id: Option<&str>,
        entrypoint: &str,
        arguments: &Value,
    ) -> RuntimeResult<Value> {
        let _ = work_item_id;
        self.execute_job(workspace, entrypoint, arguments)
    }

    /// Executes a job with daemon-captured session lineage and scoped identity.
    fn execute_trusted_job(
        &self,
        workspace: &BusinessWorkspace,
        work_item_id: Option<&str>,
        _identity: Option<&BoundJobIdentity>,
        entrypoint: &str,
        arguments: &Value,
    ) -> RuntimeResult<Value> {
        self.execute_bound_job(workspace, work_item_id, entrypoint, arguments)
    }

    /// Records the collected Root-to-Series delegation boundary as a durable
    /// flow event. Adapters without project-state authority cannot rebuild the
    /// flow input, so the default is a no-op and the boundary stays advisory.
    fn record_series_completed(
        &self,
        workspace: &BusinessWorkspace,
        work_item_id: &str,
        session_id: &str,
        delegation_id: &str,
        idempotency_key: &str,
    ) -> RuntimeResult<()> {
        let _ = (
            workspace,
            work_item_id,
            session_id,
            delegation_id,
            idempotency_key,
        );
        Ok(())
    }

    /// Validates bounded child artifact references against authoritative files.
    fn validate_delegation_artifacts(
        &self,
        workspace: &BusinessWorkspace,
        delegation_id: &str,
        result: &Value,
    ) -> RuntimeResult<Value>;

    /// Cleans the daemon-owned child memory namespace and returns a durable receipt.
    fn cleanup_delegation_memory(
        &self,
        workspace: &BusinessWorkspace,
        delegation_id: &str,
        result: &Value,
        artifact_receipt: &Value,
    ) -> RuntimeResult<Value>;
}

/// Fail-closed default for installations missing authoritative business ports.
#[derive(Clone, Copy, Debug, Default)]
pub struct RejectingBusinessPort;

impl BusinessOperationPort for RejectingBusinessPort {
    fn execute(
        &self,
        method: RpcMethod,
        _params: &RequestParams<Value>,
        _workspace: Option<&BusinessWorkspace>,
    ) -> RuntimeResult<Value> {
        let code = if method == RpcMethod::GateEvaluate {
            StableErrorCode::GateError
        } else {
            StableErrorCode::OperationNotRegistered
        };
        Err(RuntimeError::new(
            code,
            "authoritative business operation port is unavailable",
        ))
    }

    fn project_context(
        &self,
        _workspace: &BusinessWorkspace,
        _work_item_id: &str,
        _session_id: &str,
        _role: AgentRole,
    ) -> RuntimeResult<ContextProjectionInput> {
        Err(RuntimeError::new(
            StableErrorCode::ContextRevisionStale,
            "authoritative context projection port is unavailable",
        ))
    }

    fn execute_job(
        &self,
        _workspace: &BusinessWorkspace,
        _entrypoint: &str,
        _arguments: &Value,
    ) -> RuntimeResult<Value> {
        Err(RuntimeError::new(
            StableErrorCode::OperationNotRegistered,
            "background job adapter is unavailable",
        ))
    }

    fn validate_delegation_artifacts(
        &self,
        _workspace: &BusinessWorkspace,
        _delegation_id: &str,
        _result: &Value,
    ) -> RuntimeResult<Value> {
        Err(RuntimeError::new(
            StableErrorCode::ChildResultInvalid,
            "authoritative delegation artifact validator is unavailable",
        ))
    }

    fn cleanup_delegation_memory(
        &self,
        _workspace: &BusinessWorkspace,
        _delegation_id: &str,
        _result: &Value,
        _artifact_receipt: &Value,
    ) -> RuntimeResult<Value> {
        Err(RuntimeError::new(
            StableErrorCode::ChildResultInvalid,
            "authoritative delegation memory cleaner is unavailable",
        ))
    }
}

/// Deterministic in-memory persistence used by contract tests.
#[derive(Debug)]
pub struct MemoryPersistence {
    event_store_id: EventStoreId,
    inner: Mutex<MemoryState>,
    commit_failure_plan: Mutex<CommitFailurePlan>,
    record_store_failure: Mutex<Option<RecordStoreFailure>>,
}

#[derive(Debug, Default)]
struct CommitFailurePlan {
    calls: usize,
    fail_at: Option<usize>,
}

#[derive(Debug)]
struct RecordStoreFailure {
    namespace: String,
    remaining_calls: usize,
}

#[derive(Debug, Default)]
struct MemoryState {
    events: Vec<DurableEvent>,
    receipts: BTreeMap<(String, String), IdempotencyReceipt>,
    records: BTreeMap<(String, String), Value>,
    identity_receipts: BTreeMap<(String, String, String), (String, RuntimeIdentitySnapshot)>,
    identity_snapshots: Vec<RuntimeIdentitySnapshot>,
    jobs: BTreeMap<String, RuntimeJobRecord>,
    job_submissions: BTreeMap<(String, String), String>,
    execution_checkpoints: BTreeMap<(String, String, String), ExecutionCheckpointRecord>,
    execution_resource_leases: BTreeMap<String, ExecutionResourceLeaseRecordV1>,
}

impl MemoryPersistence {
    /// Creates an empty store with an explicit epoch identity.
    #[must_use]
    pub fn new(event_store_id: EventStoreId) -> Self {
        Self {
            event_store_id,
            inner: Mutex::new(MemoryState::default()),
            commit_failure_plan: Mutex::new(CommitFailurePlan::default()),
            record_store_failure: Mutex::new(None),
        }
    }

    /// Injects one deterministic failure into a future event+receipt commit.
    ///
    /// This is a test seam for crash-consistency contracts; production SQLite
    /// persistence has its own transaction fault tests.
    pub fn fail_commit_event_and_receipt_after(&self, additional_calls: usize) {
        let mut plan = self
            .commit_failure_plan
            .lock()
            .expect("commit failure plan lock");
        plan.fail_at = Some(plan.calls.saturating_add(additional_calls.max(1)));
    }

    /// Injects one deterministic failure into a future record write for a namespace.
    pub fn fail_store_record_after(&self, namespace: &str, additional_calls: usize) {
        *self
            .record_store_failure
            .lock()
            .expect("record failure plan lock") = Some(RecordStoreFailure {
            namespace: namespace.to_owned(),
            remaining_calls: additional_calls.max(1),
        });
    }

    fn lock(&self) -> RuntimeResult<std::sync::MutexGuard<'_, MemoryState>> {
        self.inner.lock().map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "runtime metadata lock is poisoned",
            )
        })
    }

    fn consume_commit_failure(&self) -> bool {
        let mut plan = self
            .commit_failure_plan
            .lock()
            .expect("commit failure plan lock");
        plan.calls = plan.calls.saturating_add(1);
        if plan.fail_at == Some(plan.calls) {
            plan.fail_at = None;
            true
        } else {
            false
        }
    }
}

impl PersistencePort for MemoryPersistence {
    fn event_store_id(&self) -> RuntimeResult<EventStoreId> {
        Ok(self.event_store_id)
    }

    fn latest_event_sequence(&self) -> RuntimeResult<u64> {
        Ok(self
            .lock()?
            .events
            .last()
            .map_or(0, |event| event.event_seq))
    }

    fn append_event(&self, mut event: DurableEvent) -> RuntimeResult<DurableEvent> {
        let mut state = self.lock()?;
        let next = state
            .events
            .last()
            .map_or(1, |previous| previous.event_seq.saturating_add(1));
        if next == 0 {
            return Err(RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "global event sequence overflow",
            ));
        }
        event.event_store_id = self.event_store_id.to_string();
        event.event_seq = next;
        state.events.push(event.clone());
        Ok(event)
    }

    fn commit_event_and_receipt(
        &self,
        mut event: DurableEvent,
        mut receipt: IdempotencyReceipt,
    ) -> RuntimeResult<(DurableEvent, IdempotencyReceipt)> {
        if self.consume_commit_failure() {
            return Err(RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "injected event+receipt persistence failure",
            ));
        }
        let mut state = self.lock()?;
        let key = (receipt.scope.clone(), receipt.key.clone());
        if let Some(existing) = state.receipts.get(&key) {
            if existing.request_digest != receipt.request_digest {
                return Err(RuntimeError::new(
                    StableErrorCode::IdempotencyKeyReused,
                    "idempotency key was reused with a different payload",
                ));
            }
            let existing_event = state
                .events
                .iter()
                .find(|item| item.event_seq == existing.event_seq)
                .cloned()
                .ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "receipt points to a missing durable event",
                    )
                })?;
            return Ok((existing_event, existing.clone()));
        }
        let next = state
            .events
            .last()
            .map_or(1, |previous| previous.event_seq.saturating_add(1));
        if next == 0 {
            return Err(RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "global event sequence overflow",
            ));
        }
        event.event_store_id = self.event_store_id.to_string();
        event.event_seq = next;
        receipt.event_seq = next;
        state.events.push(event.clone());
        state.receipts.insert(key, receipt.clone());
        Ok((event, receipt))
    }

    fn commit_prepared_execution_hook(
        &self,
        mut event: DurableEvent,
        mut receipt: IdempotencyReceipt,
        record: PreparedExecutionHookV1,
    ) -> RuntimeResult<(DurableEvent, IdempotencyReceipt, PreparedExecutionHookV1)> {
        if self.consume_commit_failure() {
            return Err(RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "injected prepared execution persistence failure",
            ));
        }
        let mut state = self.lock()?;
        let record_key = (
            "prepared-execution-hook/v1".to_owned(),
            record.session_id.clone(),
        );
        if let Some(existing_value) = state.records.get(&record_key) {
            let existing: PreparedExecutionHookV1 = serde_json::from_value(existing_value.clone())
                .map_err(|_| {
                    RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "prepared execution Hook record is malformed",
                    )
                })?;
            if !existing.completed
                && (existing.hook_event_id != record.hook_event_id
                    || existing.request_digest != record.request_digest)
            {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "another execution Hook transition is still pending for this session",
                ));
            }
            if !existing.completed {
                let existing_receipt = state
                    .receipts
                    .get(&(receipt.scope.clone(), receipt.key.clone()))
                    .cloned()
                    .ok_or_else(|| {
                        RuntimeError::new(
                            StableErrorCode::ExternalStateConflict,
                            "prepared execution Hook record lacks its receipt",
                        )
                    })?;
                if existing_receipt.request_digest != receipt.request_digest {
                    return Err(RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "prepared execution Hook receipt conflicts with its record",
                    ));
                }
                let existing_event = state
                    .events
                    .iter()
                    .find(|item| item.event_seq == existing_receipt.event_seq)
                    .cloned()
                    .ok_or_else(|| {
                        RuntimeError::new(
                            StableErrorCode::ExternalStateConflict,
                            "prepared execution Hook receipt lacks its event",
                        )
                    })?;
                return Ok((existing_event, existing_receipt, existing));
            }
        }
        let receipt_key = (receipt.scope.clone(), receipt.key.clone());
        if let Some(existing) = state.receipts.get(&receipt_key) {
            if existing.request_digest != receipt.request_digest {
                return Err(RuntimeError::new(
                    StableErrorCode::IdempotencyKeyReused,
                    "idempotency key was reused with a different payload",
                ));
            }
            return Err(RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "execution receipt exists without an active prepared Hook record",
            ));
        }
        let next = state
            .events
            .last()
            .map_or(1, |previous| previous.event_seq.saturating_add(1));
        if next == 0 {
            return Err(RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "global event sequence overflow",
            ));
        }
        event.event_store_id = self.event_store_id.to_string();
        event.event_seq = next;
        receipt.event_seq = next;
        let record_value = serde_json::to_value(&record).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "prepared execution Hook record cannot be encoded",
            )
        })?;
        state.events.push(event.clone());
        state.receipts.insert(receipt_key, receipt.clone());
        state.records.insert(record_key, record_value);
        Ok((event, receipt, record))
    }

    fn events_after(&self, after: u64, limit: usize) -> RuntimeResult<Vec<DurableEvent>> {
        Ok(self
            .lock()?
            .events
            .iter()
            .filter(|event| event.event_seq > after)
            .take(limit)
            .cloned()
            .collect())
    }

    fn oldest_event_sequence(&self) -> RuntimeResult<u64> {
        Ok(self
            .lock()?
            .events
            .first()
            .map_or(0, |event| event.event_seq))
    }

    fn load_receipt(&self, scope: &str, key: &str) -> RuntimeResult<Option<IdempotencyReceipt>> {
        Ok(self
            .lock()?
            .receipts
            .get(&(scope.to_owned(), key.to_owned()))
            .cloned())
    }

    fn store_receipt(&self, receipt: &IdempotencyReceipt) -> RuntimeResult<()> {
        let mut state = self.lock()?;
        let key = (receipt.scope.clone(), receipt.key.clone());
        if let Some(existing) = state.receipts.get(&key) {
            if existing.request_digest != receipt.request_digest {
                return Err(RuntimeError::new(
                    StableErrorCode::IdempotencyKeyReused,
                    "idempotency key was reused with a different payload",
                ));
            }
            return Ok(());
        }
        state.receipts.insert(key, receipt.clone());
        Ok(())
    }

    fn load_record(&self, namespace: &str, key: &str) -> RuntimeResult<Option<Value>> {
        Ok(self
            .lock()?
            .records
            .get(&(namespace.to_owned(), key.to_owned()))
            .cloned())
    }

    fn list_records(&self, namespace: &str) -> RuntimeResult<Vec<(String, Value)>> {
        Ok(self
            .lock()?
            .records
            .iter()
            .filter(|((record_namespace, _), _)| record_namespace == namespace)
            .map(|((_, key), value)| (key.clone(), value.clone()))
            .collect())
    }

    fn store_record(&self, namespace: &str, key: &str, value: &Value) -> RuntimeResult<()> {
        let mut failure = self.record_store_failure.lock().map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "record failure plan lock is poisoned",
            )
        })?;
        if let Some(plan) = failure.as_mut()
            && plan.namespace == namespace
        {
            plan.remaining_calls -= 1;
            if plan.remaining_calls == 0 {
                *failure = None;
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "injected record persistence failure",
                ));
            }
        }
        drop(failure);
        self.lock()?
            .records
            .insert((namespace.to_owned(), key.to_owned()), value.clone());
        Ok(())
    }

    fn delete_record(&self, namespace: &str, key: &str) -> RuntimeResult<()> {
        self.lock()?
            .records
            .remove(&(namespace.to_owned(), key.to_owned()));
        Ok(())
    }

    fn acquire_execution_resource_lease(
        &self,
        request: &ExecutionResourceLeaseRequestV1,
    ) -> RuntimeResult<ExecutionResourceLeaseOutcomeV1> {
        let mut state = self.lock()?;
        if let Some(existing) = state.execution_resource_leases.get(&request.resource)
            && existing.expires_at_unix_ms > request.now_unix_ms
        {
            if existing.boot_id == request.boot_id && existing.session_id == request.session_id {
                return Ok(ExecutionResourceLeaseOutcomeV1::Reentered);
            }
            return Ok(ExecutionResourceLeaseOutcomeV1::Deferred {
                retry_after_ms: request.retry_after_ms,
            });
        }
        let record = ExecutionResourceLeaseRecordV1 {
            schema_version: "execution-resource-lease/v1".to_owned(),
            resource: request.resource.clone(),
            boot_id: request.boot_id.clone(),
            session_id: request.session_id.clone(),
            acquired_at_unix_ms: request.now_unix_ms,
            expires_at_unix_ms: request.now_unix_ms.saturating_add(request.ttl_ms),
        };
        state
            .execution_resource_leases
            .insert(request.resource.clone(), record);
        Ok(ExecutionResourceLeaseOutcomeV1::Granted)
    }

    fn release_execution_resource_lease(
        &self,
        resource: &str,
        boot_id: &str,
        session_id: &str,
    ) -> RuntimeResult<()> {
        let mut state = self.lock()?;
        if state
            .execution_resource_leases
            .get(resource)
            .is_some_and(|lease| lease.boot_id == boot_id && lease.session_id == session_id)
        {
            state.execution_resource_leases.remove(resource);
        }
        Ok(())
    }

    fn commit_identity_bundle(
        &self,
        transition: RuntimeIdentityTransition,
    ) -> RuntimeResult<RuntimeIdentitySnapshot> {
        validate_identity_transition(&transition)?;
        let mut state = self.lock()?;
        let receipt_key = (
            transition.snapshot.workspace.workspace_id.clone(),
            transition.scope_digest.clone(),
            transition.idempotency_key.clone(),
        );
        if let Some((request_digest, snapshot)) = state.identity_receipts.get(&receipt_key) {
            if request_digest != &transition.request_digest {
                return Err(RuntimeError::new(
                    StableErrorCode::IdempotencyKeyReused,
                    "identity idempotency key was reused with a different trusted request",
                ));
            }
            let mut replayed = snapshot.clone();
            replayed.replayed = true;
            return Ok(replayed);
        }
        validate_identity_expected_values(&state.identity_snapshots, &transition)?;
        let mut snapshot = transition.snapshot;
        snapshot.replayed = false;
        state.identity_snapshots.push(snapshot.clone());
        state
            .identity_receipts
            .insert(receipt_key, (transition.request_digest, snapshot.clone()));
        Ok(snapshot)
    }

    fn list_identity_snapshots(
        &self,
        kind: RuntimeIdentityKind,
    ) -> RuntimeResult<Vec<RuntimeIdentitySnapshot>> {
        let state = self.lock()?;
        Ok(latest_identity_snapshots(&state.identity_snapshots, kind))
    }

    fn commit_job_transition(
        &self,
        mut transition: RuntimeJobTransition,
    ) -> RuntimeResult<RuntimeJobRecord> {
        validate_job_record(&transition.record)?;
        let mut state = self.lock()?;
        let submission_key = (
            transition.record.submission_scope_digest.clone(),
            transition.record.submission_idempotency_key.clone(),
        );
        if transition.expected_status.is_none()
            && let Some(job_id) = state.job_submissions.get(&submission_key)
        {
            let existing = state.jobs.get(job_id).ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "typed job submission index points to a missing job",
                )
            })?;
            if existing.request_digest != transition.record.request_digest {
                return Err(RuntimeError::new(
                    StableErrorCode::IdempotencyKeyReused,
                    "job submission key was reused with a different trusted request",
                ));
            }
            return Ok(existing.clone());
        }
        match state.jobs.get(&transition.record.job_id) {
            Some(existing) => {
                if Some(existing.status) != transition.expected_status
                    || Some(existing.row_version) != transition.expected_row_version
                {
                    return Err(RuntimeError::new(
                        StableErrorCode::RevisionConflict,
                        "typed job expected status or row version is stale",
                    ));
                }
            }
            None if transition.expected_status.is_some()
                || transition.expected_row_version.is_some() =>
            {
                return Err(RuntimeError::new(
                    StableErrorCode::RevisionConflict,
                    "typed job expected row does not exist",
                ));
            }
            None => {}
        }
        let next = state
            .events
            .last()
            .map_or(1, |previous| previous.event_seq.saturating_add(1));
        transition.event.event_store_id = self.event_store_id.to_string();
        transition.event.event_seq = next;
        transition.record.last_event_seq = next;
        if transition.expected_status.is_none() {
            transition.record.submitted_event_seq = next;
            state
                .job_submissions
                .insert(submission_key, transition.record.job_id.clone());
        }
        state.events.push(transition.event);
        state
            .jobs
            .insert(transition.record.job_id.clone(), transition.record.clone());
        Ok(transition.record)
    }

    fn load_job(&self, job_id: &str) -> RuntimeResult<Option<RuntimeJobRecord>> {
        Ok(self.lock()?.jobs.get(job_id).cloned())
    }

    fn list_jobs(&self) -> RuntimeResult<Vec<RuntimeJobRecord>> {
        Ok(self.lock()?.jobs.values().cloned().collect())
    }

    fn load_execution_checkpoint(
        &self,
        scope: &ExecutionCheckpointScope,
    ) -> RuntimeResult<Option<ExecutionCheckpointRecord>> {
        Ok(self
            .lock()?
            .execution_checkpoints
            .get(&execution_checkpoint_key(scope))
            .cloned())
    }

    fn store_execution_checkpoint(&self, record: &ExecutionCheckpointRecord) -> RuntimeResult<()> {
        validate_execution_checkpoint(record)?;
        self.lock()?.execution_checkpoints.insert(
            (
                record.workspace_id.clone(),
                record.work_item_id.clone(),
                record.session_id.clone(),
            ),
            record.clone(),
        );
        Ok(())
    }

    fn discard_execution_checkpoint(&self, scope: &ExecutionCheckpointScope) -> RuntimeResult<()> {
        self.lock()?
            .execution_checkpoints
            .remove(&execution_checkpoint_key(scope));
        Ok(())
    }
}

fn validate_identity_transition(transition: &RuntimeIdentityTransition) -> RuntimeResult<()> {
    if transition.idempotency_key.is_empty()
        || transition.idempotency_key.len() > 128
        || !is_digest(&transition.scope_digest)
        || !is_digest(&transition.request_digest)
        || contains_secret(&transition.snapshot.response)
    {
        return Err(RuntimeError::new(
            StableErrorCode::OperationSchemaInvalid,
            "typed identity transition is unbounded, malformed, or contains secret material",
        ));
    }
    Ok(())
}

fn validate_identity_expected_values(
    snapshots: &[RuntimeIdentitySnapshot],
    transition: &RuntimeIdentityTransition,
) -> RuntimeResult<()> {
    let workspace_current = snapshots.iter().rev().find(|snapshot| {
        snapshot.workspace.workspace_id == transition.snapshot.workspace.workspace_id
    });
    let current = snapshots.iter().rev().find(|snapshot| {
        snapshot.workspace.workspace_id == transition.snapshot.workspace.workspace_id
            && match transition.snapshot.identity_kind {
                RuntimeIdentityKind::Workspace => true,
                RuntimeIdentityKind::Session => {
                    snapshot
                        .session
                        .as_ref()
                        .map(|record| record.session_id.as_str())
                        == transition
                            .snapshot
                            .session
                            .as_ref()
                            .map(|record| record.session_id.as_str())
                }
                RuntimeIdentityKind::Delegation => {
                    snapshot
                        .delegation
                        .as_ref()
                        .map(|record| record.delegation_id.as_str())
                        == transition
                            .snapshot
                            .delegation
                            .as_ref()
                            .map(|record| record.delegation_id.as_str())
                }
            }
    });
    if let Some(expected) = transition.expected_workspace_mode
        && workspace_current.map(|value| value.workspace.mode) != Some(expected)
    {
        return revision_conflict();
    }
    if let Some(expected) = transition.expected_inventory_generation
        && workspace_current.map(|value| value.workspace.inventory_generation) != Some(expected)
    {
        return revision_conflict();
    }
    if let Some(expected) = transition.expected_session_status.as_deref()
        && current
            .and_then(|value| value.session.as_ref())
            .map(|value| value.status.as_str())
            != Some(expected)
    {
        return revision_conflict();
    }
    if let Some(expected) = transition.expected_delegation_status.as_deref()
        && current
            .and_then(|value| value.delegation.as_ref())
            .map(|value| value.status.as_str())
            != Some(expected)
    {
        return revision_conflict();
    }
    if let Some(expected) = transition.expected_context_generation
        && current
            .and_then(|value| value.session.as_ref())
            .map(|value| value.context_generation)
            != Some(expected)
    {
        return revision_conflict();
    }
    Ok(())
}

fn latest_identity_snapshots(
    snapshots: &[RuntimeIdentitySnapshot],
    kind: RuntimeIdentityKind,
) -> Vec<RuntimeIdentitySnapshot> {
    let mut latest = BTreeMap::new();
    for snapshot in snapshots.iter().filter(|value| value.identity_kind == kind) {
        let key = match kind {
            RuntimeIdentityKind::Workspace => snapshot.workspace.workspace_id.clone(),
            RuntimeIdentityKind::Session => snapshot
                .session
                .as_ref()
                .map(|record| record.session_id.clone())
                .unwrap_or_default(),
            RuntimeIdentityKind::Delegation => snapshot
                .delegation
                .as_ref()
                .map(|record| record.delegation_id.clone())
                .unwrap_or_default(),
        };
        latest.insert(key, snapshot.clone());
    }
    latest.into_values().collect()
}

fn validate_job_record(record: &RuntimeJobRecord) -> RuntimeResult<()> {
    let arguments = serde_json::to_vec(&record.arguments).map_err(|_| {
        RuntimeError::new(
            StableErrorCode::OperationSchemaInvalid,
            "typed job arguments are not canonical JSON",
        )
    })?;
    if !record.arguments.is_object()
        || arguments.len() > 65_536
        || !is_digest(&record.submission_scope_digest)
        || !is_digest(&record.submission_idempotency_key_digest)
        || !is_digest(&record.request_digest)
        || record.submission_idempotency_key.is_empty()
        || record.submission_idempotency_key.len() > 256
        || record.result.as_ref().is_some_and(contains_secret)
    {
        return Err(RuntimeError::new(
            StableErrorCode::OperationSchemaInvalid,
            "typed runtime job record is malformed or unbounded",
        ));
    }
    Ok(())
}

fn execution_checkpoint_key(scope: &ExecutionCheckpointScope) -> (String, String, String) {
    (
        scope.workspace_id.clone(),
        scope.work_item_id.clone(),
        scope.session_id.clone(),
    )
}

fn validate_execution_checkpoint(record: &ExecutionCheckpointRecord) -> RuntimeResult<()> {
    if record.workspace_id.is_empty()
        || record.workspace_id.len() > 128
        || record.work_item_id.is_empty()
        || record.work_item_id.len() > 128
        || record.session_id.is_empty()
        || record.session_id.len() > 128
        || !is_digest(&record.capsule_digest)
        || !is_digest(&record.queue_digest)
    {
        return Err(RuntimeError::new(
            StableErrorCode::OperationSchemaInvalid,
            "execution checkpoint record is unbounded or malformed",
        ));
    }
    Ok(())
}

fn contains_secret(value: &Value) -> bool {
    match value {
        Value::Object(values) => values.iter().any(|(key, value)| {
            matches!(
                key.as_str(),
                "capabilityToken" | "claimId" | "credential" | "endpointToken" | "secret" | "token"
            ) || contains_secret(value)
        }),
        Value::Array(values) => values.iter().any(contains_secret),
        _ => false,
    }
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn revision_conflict<T>() -> RuntimeResult<T> {
    Err(RuntimeError::new(
        StableErrorCode::RevisionConflict,
        "typed persistence expected value is stale",
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, mpsc};

    use ae_sdd_domain::EventStoreId;
    use uuid::Uuid;

    use super::MemoryPersistence;

    #[test]
    fn concurrent_commit_cannot_pass_between_failure_plan_installation_steps() {
        let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(Uuid::nil())));
        let mut plan = persistence
            .commit_failure_plan
            .lock()
            .expect("commit failure plan lock");
        let (started_tx, started_rx) = mpsc::channel();
        let installer_persistence = Arc::clone(&persistence);
        let installer = std::thread::spawn(move || {
            started_tx.send(()).expect("signal installer start");
            installer_persistence.fail_commit_event_and_receipt_after(1);
        });

        started_rx.recv().expect("installer started");
        plan.calls += 1;
        drop(plan);
        installer.join().expect("installer joined");

        assert!(persistence.consume_commit_failure());
        assert!(!persistence.consume_commit_failure());
    }
}
