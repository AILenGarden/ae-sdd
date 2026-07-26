//! Daemon-wide Cargo resource arbitration.
//!
//! Two sessions requesting Cargo concurrently are serialized through one fair
//! lease: the first is allowed, the second is deferred with a bounded
//! `retryAfterMs`, and after release or lease TTL expiry the second proceeds.
//! The lease is backed by an explicit lock file under the per-user runtime
//! state dir (never a workspace root or an unresolved environment variable),
//! so a daemon crash releases the OS lock; the in-process fair queue keeps
//! waiters FIFO and bounded.

mod support;

#[path = "../src/execution_resources.rs"]
mod execution_resources;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ae_sdd_contracts::SchemaVersion;
use ae_sdd_contracts::execution_runtime::{
    ExecutionBudgetsV1, ExecutionCapsuleV1, ExecutionQueueRefV1, ExecutionSliceV1,
};
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, ArtifactKind, ArtifactRef, BootId, EventStoreId, ExecutionSliceId,
    InventoryGeneration, PolicyDigest, ProjectRelativePath, StateRevision, StoryId, VerificationId,
    WorkItemId,
};
use ae_sdd_protocol::{
    ClientKind, ConfirmationRef, HandshakeRequest, JsonRpcRequest, PROTOCOL_RANGE_V1,
    RequestParams, RpcMethod, SecretString, WorkspaceMode,
};
use ae_sdd_runtime::{
    BusinessOperationPort, BusinessWorkspace, ConnectionState, ContextProjectionInput,
    MemoryPersistence, RuntimeConfig, RuntimeResult, RuntimeService, SessionResult,
    WorkspaceResult,
};
use serde_json::{Value, json};
use uuid::Uuid;

use execution_resources::{
    CargoAcquireRequest, CargoResourceArbiter, ResourceDecision, ResourceKind,
};
use support::{TestClock, TestResolver, params, parity_transition_payload, result, session_params};

const WORK_ITEM: &str = "WORK-1";
const HEX_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HEX_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const TTL_MS: u64 = 300_000;
const RETRY_AFTER_MS: u64 = 1_000;
const QUEUE_CAPACITY: usize = 8;

struct TempLockDir(PathBuf);

impl TempLockDir {
    fn new(test: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("ae-sdd-cargo-lock-{test}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp lock dir is created");
        Self(dir)
    }

    fn lock_file(&self) -> PathBuf {
        self.0.join("execution-cargo.lock")
    }
}

impl Drop for TempLockDir {
    fn drop(&mut self) {
        let _removed = std::fs::remove_dir_all(&self.0);
    }
}

fn acquire<'a>(session: &'a str, lock_path: Option<&'a Path>, now: u64) -> CargoAcquireRequest<'a> {
    CargoAcquireRequest {
        session_id: session,
        lock_path,
        now_unix_ms: now,
        ttl_ms: TTL_MS,
        retry_after_ms: RETRY_AFTER_MS,
        queue_capacity: QUEUE_CAPACITY,
    }
}

fn os_lock_free(path: &Path) -> bool {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .expect("lock file opens");
    match file.try_lock() {
        Ok(()) => {
            let _released = file.unlock();
            true
        }
        Err(_) => false,
    }
}

#[test]
fn first_session_acquires_second_defers_with_retry_hint() {
    let arbiter = CargoResourceArbiter::new();
    assert_eq!(
        arbiter.acquire(ResourceKind::Cargo, &acquire("session-a", None, 1_000)),
        ResourceDecision::Allow
    );
    assert_eq!(
        arbiter.acquire(ResourceKind::Cargo, &acquire("session-b", None, 1_001)),
        ResourceDecision::Defer {
            retry_after_ms: RETRY_AFTER_MS
        },
        "the concurrent second session defers with the bounded retry hint"
    );
    assert_eq!(arbiter.holder_session().as_deref(), Some("session-a"));
}

#[test]
fn release_lets_the_waiting_session_proceed() {
    let arbiter = CargoResourceArbiter::new();
    assert_eq!(
        arbiter.acquire(ResourceKind::Cargo, &acquire("session-a", None, 1_000)),
        ResourceDecision::Allow
    );
    assert!(matches!(
        arbiter.acquire(ResourceKind::Cargo, &acquire("session-b", None, 1_001)),
        ResourceDecision::Defer { .. }
    ));

    arbiter.release(ResourceKind::Cargo, "session-a");
    assert_eq!(arbiter.holder_session(), None);
    assert_eq!(
        arbiter.acquire(ResourceKind::Cargo, &acquire("session-b", None, 1_002)),
        ResourceDecision::Allow,
        "after release the queued session proceeds"
    );
    assert_eq!(arbiter.holder_session().as_deref(), Some("session-b"));
}

#[test]
fn ttl_expiry_releases_a_forgotten_lease() {
    let arbiter = CargoResourceArbiter::new();
    assert_eq!(
        arbiter.acquire(ResourceKind::Cargo, &acquire("session-a", None, 1_000)),
        ResourceDecision::Allow
    );
    assert_eq!(
        arbiter.acquire(
            ResourceKind::Cargo,
            &acquire("session-b", None, 1_000 + TTL_MS)
        ),
        ResourceDecision::Allow,
        "a lease at or beyond its TTL is released for the next waiter"
    );
    assert_eq!(arbiter.holder_session().as_deref(), Some("session-b"));

    assert_eq!(
        arbiter.acquire(
            ResourceKind::Cargo,
            &acquire("session-a", None, 1_000 + TTL_MS + 1)
        ),
        ResourceDecision::Defer {
            retry_after_ms: RETRY_AFTER_MS
        },
        "the former holder lost the lease and must wait"
    );
}

#[test]
fn reentrant_acquire_keeps_the_original_lease_clock() {
    let arbiter = CargoResourceArbiter::new();
    assert_eq!(
        arbiter.acquire(ResourceKind::Cargo, &acquire("session-a", None, 1_000)),
        ResourceDecision::Allow
    );
    assert_eq!(
        arbiter.acquire(
            ResourceKind::Cargo,
            &acquire("session-a", None, 1_000 + TTL_MS - 1)
        ),
        ResourceDecision::Allow,
        "the holder re-entering within the TTL keeps its lease"
    );
    assert_eq!(
        arbiter.acquire(
            ResourceKind::Cargo,
            &acquire("session-b", None, 1_000 + TTL_MS)
        ),
        ResourceDecision::Allow,
        "re-entry never refreshes the lease clock"
    );
}

#[test]
fn waiters_are_granted_in_fifo_order() {
    let arbiter = CargoResourceArbiter::new();
    assert_eq!(
        arbiter.acquire(ResourceKind::Cargo, &acquire("session-a", None, 1_000)),
        ResourceDecision::Allow
    );
    assert!(matches!(
        arbiter.acquire(ResourceKind::Cargo, &acquire("session-b", None, 1_001)),
        ResourceDecision::Defer { .. }
    ));
    assert!(matches!(
        arbiter.acquire(ResourceKind::Cargo, &acquire("session-c", None, 1_002)),
        ResourceDecision::Defer { .. }
    ));
    assert_eq!(arbiter.queue_position("session-b"), Some(0));
    assert_eq!(arbiter.queue_position("session-c"), Some(1));

    arbiter.release(ResourceKind::Cargo, "session-a");
    assert_eq!(
        arbiter.acquire(ResourceKind::Cargo, &acquire("session-c", None, 1_003)),
        ResourceDecision::Defer {
            retry_after_ms: RETRY_AFTER_MS
        },
        "only the queue front may take a freed lease"
    );
    assert_eq!(
        arbiter.acquire(ResourceKind::Cargo, &acquire("session-b", None, 1_004)),
        ResourceDecision::Allow
    );
    arbiter.release(ResourceKind::Cargo, "session-b");
    assert_eq!(
        arbiter.acquire(ResourceKind::Cargo, &acquire("session-c", None, 1_005)),
        ResourceDecision::Allow
    );
}

#[test]
fn the_waiter_queue_is_bounded() {
    let arbiter = CargoResourceArbiter::new();
    assert_eq!(
        arbiter.acquire(ResourceKind::Cargo, &acquire("session-a", None, 1_000)),
        ResourceDecision::Allow
    );
    let tight = CargoAcquireRequest {
        queue_capacity: 1,
        ..acquire("session-b", None, 1_001)
    };
    assert!(matches!(
        arbiter.acquire(ResourceKind::Cargo, &tight),
        ResourceDecision::Defer { .. }
    ));
    let overflow = CargoAcquireRequest {
        queue_capacity: 1,
        ..acquire("session-c", None, 1_002)
    };
    assert!(matches!(
        arbiter.acquire(ResourceKind::Cargo, &overflow),
        ResourceDecision::Defer { .. }
    ));
    assert_eq!(
        arbiter.queue_position("session-c"),
        None,
        "a waiter beyond the queue capacity is not queued"
    );
}

#[test]
fn os_lock_file_stays_held_until_release() {
    let temp = TempLockDir::new("os-lock");
    let lock_path = temp.lock_file();
    let arbiter = CargoResourceArbiter::new();

    assert_eq!(
        arbiter.acquire(
            ResourceKind::Cargo,
            &acquire("session-a", Some(lock_path.as_path()), 1_000)
        ),
        ResourceDecision::Allow
    );
    assert!(
        !os_lock_free(lock_path.as_path()),
        "the OS lock is held while the lease is active"
    );

    arbiter.release(ResourceKind::Cargo, "session-a");
    assert!(
        os_lock_free(lock_path.as_path()),
        "the OS lock is released with the lease, so a crashed holder never blocks recovery"
    );
}

#[test]
fn os_lock_contended_by_another_handle_defers() {
    let temp = TempLockDir::new("os-contention");
    let lock_path = temp.lock_file();
    let foreign = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .expect("foreign handle opens");
    assert!(
        foreign.try_lock().is_ok(),
        "foreign handle takes the OS lock"
    );

    let arbiter = CargoResourceArbiter::new();
    assert_eq!(
        arbiter.acquire(
            ResourceKind::Cargo,
            &acquire("session-a", Some(lock_path.as_path()), 1_000)
        ),
        ResourceDecision::Defer {
            retry_after_ms: RETRY_AFTER_MS
        },
        "OS-level contention defers fail-closed"
    );
    assert_eq!(arbiter.holder_session(), None);

    let _released = foreign.unlock();
    assert_eq!(
        arbiter.acquire(
            ResourceKind::Cargo,
            &acquire("session-a", Some(lock_path.as_path()), 1_001)
        ),
        ResourceDecision::Allow,
        "once the foreign handle releases, the lease is granted"
    );
}

#[test]
fn an_unwritable_lock_path_defers_fail_closed() {
    let temp = TempLockDir::new("os-error");
    let missing_parent = temp.0.join("missing").join("execution-cargo.lock");
    let arbiter = CargoResourceArbiter::new();
    assert_eq!(
        arbiter.acquire(
            ResourceKind::Cargo,
            &acquire("session-a", Some(missing_parent.as_path()), 1_000)
        ),
        ResourceDecision::Defer {
            retry_after_ms: RETRY_AFTER_MS
        },
        "a lock file that cannot be opened never allows parallel Cargo"
    );
}

struct ResumeBusiness {
    operation_calls: AtomicUsize,
}

impl BusinessOperationPort for ResumeBusiness {
    fn execute(
        &self,
        _method: RpcMethod,
        params: &RequestParams<Value>,
        _workspace: Option<&BusinessWorkspace>,
    ) -> RuntimeResult<Value> {
        self.operation_calls.fetch_add(1, Ordering::AcqRel);
        if params.payload.get("operation").and_then(Value::as_str) == Some("execution.resume") {
            return Ok(resume_response_full(&test_capsule()));
        }
        Ok(json!({"ok":true}))
    }

    fn project_context(
        &self,
        workspace: &BusinessWorkspace,
        work_item_id: &str,
        session_id: &str,
        role: AgentRole,
    ) -> RuntimeResult<ContextProjectionInput> {
        let policy = ae_sdd_policy::policy_digest().to_hex();
        Ok(ContextProjectionInput {
            session_id: session_id.to_owned(),
            source_revision: 1,
            projection: json!({
                "workspaceId":workspace.workspace_id,
                "workItemId":work_item_id,
                "role":format!("{role:?}").to_lowercase(),
                "stateRevision":1,
                "policyDigest":policy,
                "inventoryGeneration":workspace.inventory_generation,
                "inputFingerprint":HEX_A,
                "hookGuard":{
                    "outcome":"PASS",
                    "stateRevision":1,
                    "policyDigest":policy,
                    "inventoryGeneration":workspace.inventory_generation,
                    "inputFingerprint":HEX_A,
                },
            }),
        })
    }

    fn execute_job(
        &self,
        _workspace: &BusinessWorkspace,
        _entrypoint: &str,
        _arguments: &Value,
    ) -> RuntimeResult<Value> {
        Ok(json!({"outcome":"PASS"}))
    }

    fn validate_delegation_artifacts(
        &self,
        _workspace: &BusinessWorkspace,
        delegation_id: &str,
        _result: &Value,
    ) -> RuntimeResult<Value> {
        Ok(json!({
            "schemaVersion":"delegation-artifact-validation/v1",
            "delegationId":delegation_id,
            "resultDigest":HEX_A,
            "artifacts":[],
        }))
    }

    fn cleanup_delegation_memory(
        &self,
        _workspace: &BusinessWorkspace,
        delegation_id: &str,
        _result: &Value,
        _artifact_receipt: &Value,
    ) -> RuntimeResult<Value> {
        Ok(json!({
            "schemaVersion":"delegation-memory-cleanup/v1",
            "delegationId":delegation_id,
            "memorySnapshotDigest":HEX_A,
            "cleanupDigest":HEX_B,
            "cleanedAtUnixMs":1_000,
        }))
    }
}

struct LockHarness {
    runtime: Arc<RuntimeService>,
    clock: Arc<TestClock>,
    token: String,
}

impl LockHarness {
    fn new(configure: impl FnOnce(&mut RuntimeConfig)) -> Self {
        let mut config = RuntimeConfig::default();
        configure(&mut config);
        let token = "endpoint-cargo-lock-test".to_owned();
        let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
            Uuid::from_u128(91),
        )));
        let clock = Arc::new(TestClock::new(1_000));
        let business = Arc::new(ResumeBusiness {
            operation_calls: AtomicUsize::new(0),
        });
        let runtime = Arc::new(RuntimeService::new(
            config,
            BootId::from_uuid(Uuid::from_u128(92)),
            token.clone(),
            persistence,
            clock.clone(),
            Arc::new(TestResolver),
            business,
        ));
        Self {
            runtime,
            clock,
            token,
        }
    }

    fn connection(&self, kind: ClientKind) -> ConnectionState {
        let mut connection = ConnectionState::default();
        let response = self.raw(
            &mut connection,
            RpcMethod::RuntimeHandshake,
            serde_json::to_value(HandshakeRequest {
                protocol_range: PROTOCOL_RANGE_V1.to_owned(),
                client_build: "test/cargo-resource-lock".to_owned(),
                client_kind: kind,
                endpoint_token: SecretString::new(self.token.clone()),
                expected_boot_id: self.runtime.boot_id().to_string(),
                expected_policy_digest: self.runtime.policy_digest().to_owned(),
            })
            .expect("handshake serializes"),
        );
        assert!(response.get("result").is_some(), "{response}");
        connection
    }

    fn call(
        &self,
        connection: &mut ConnectionState,
        method: RpcMethod,
        params: RequestParams<Value>,
    ) -> Value {
        self.raw(
            connection,
            method,
            serde_json::to_value(params).expect("params serialize"),
        )
    }

    fn raw(&self, connection: &mut ConnectionState, method: RpcMethod, params: Value) -> Value {
        let request = JsonRpcRequest::new(format!("{}-test", method.as_str()), method, params);
        let bytes = serde_json::to_vec(&request).expect("request serializes");
        serde_json::from_slice(&self.runtime.handle_payload(connection, &bytes))
            .expect("response is JSON")
    }
}

fn artifact(path: &str, contents: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new("execution-fixture").expect("artifact kind"),
        ProjectRelativePath::new(path).expect("project-relative path"),
        ArtifactDigest::digest(contents),
        u64::try_from(contents.len()).expect("fixture length fits u64"),
    )
}

fn test_capsule() -> ExecutionCapsuleV1 {
    let queue_contents = b"execution-queue/v1";
    let queue_ref = ExecutionQueueRefV1::new(
        artifact(
            ".auto-engineering/WORK-1/execution/queue.json",
            queue_contents,
        ),
        ArtifactDigest::digest(queue_contents),
        1,
        0,
        1,
    )
    .expect("queue ref");
    let active_slice = ExecutionSliceV1::new(
        ExecutionSliceId::new("slice-lock").expect("slice id"),
        1,
        "serialize cargo for the active slice",
        Vec::new(),
        vec![ProjectRelativePath::new("crates/ae-sdd-runtime").expect("path scope")],
        Vec::new(),
        VerificationId::new("V-EFF-007").expect("verification id"),
        Vec::new(),
        "execution-capsule/slice-lock".to_owned(),
    )
    .expect("valid slice");
    ExecutionCapsuleV1::new(
        SchemaVersion::V1,
        WorkItemId::new(WORK_ITEM).expect("work item id"),
        StoryId::new("STORY-AE-SDD-SLICE-SUPERVISOR-001").expect("story id"),
        StateRevision::new(12),
        ArtifactDigest::digest(b"approved-plan"),
        PolicyDigest::digest(b"lock policy"),
        InventoryGeneration::new(1),
        artifact(
            "ae-sdd-doc/Story/STORY-AE-SDD-SLICE-SUPERVISOR-001.md",
            b"story body",
        ),
        artifact("constraints/README.md", b"constraints body"),
        artifact(
            "source/standards/thinking/be-coding-thinking-engine.md",
            b"thinking body",
        ),
        artifact(
            "ae-sdd-doc/Story/slice-supervisor.verification.json",
            b"verification body",
        ),
        queue_ref,
        active_slice,
        ExecutionBudgetsV1::default(),
    )
    .expect("valid capsule")
}

fn resume_response_full(capsule: &ExecutionCapsuleV1) -> Value {
    json!({
        "changed": false,
        "revisionBefore": null,
        "revisionAfter": null,
        "receiptDigest": null,
        "data": {
            "projectionKind": "full",
            "contextRevision": capsule.source_revision().get(),
            "capsuleDigest": format!("sha256:{}", ArtifactDigest::digest(b"execution-capsule/v1").to_hex()),
            "capsule": serde_json::to_value(capsule).expect("capsule serializes"),
            "nextAction": {
                "kind":"execute-approved-slice",
                "activeOrdinal":1,
                "queueDigest":format!("sha256:{}", "b".repeat(64)),
            },
            "authorityRefreshCount": 1,
        },
    })
}

fn confirmation() -> ConfirmationRef {
    ConfirmationRef {
        confirmation_id: "confirmation".to_owned(),
        approved_by: "user".to_owned(),
        approved_at: "2026-01-01T00:00:00Z".to_owned(),
    }
}

fn engaged_session(
    harness: &LockHarness,
    connection: &mut ConnectionState,
    suffix: &str,
) -> (WorkspaceResult, SessionResult) {
    let mut request = params(
        json!({
            "projectRoot": format!("C:/ae-sdd-tests/{suffix}"),
            "projectKey": format!("project-{suffix}"),
        }),
        1_000,
    );
    request.idempotency_key = Some(format!("workspace-{suffix}"));
    let workspace: WorkspaceResult = serde_json::from_value(result(&harness.call(
        connection,
        RpcMethod::WorkspaceRegister,
        request,
    )))
    .expect("workspace result decodes");

    let mut admin = harness.connection(ClientKind::Admin);
    let mut drain = params(json!({"stop":false}), 1_000);
    drain.idempotency_key = Some(format!("drain-{suffix}"));
    drain.confirmation = Some(confirmation());
    let _ = result(&harness.call(&mut admin, RpcMethod::RuntimeDrain, drain));
    let mut transition = params(
        parity_transition_payload(WorkspaceMode::RustCanary, 1_000),
        1_000,
    );
    transition.workspace_id = Some(workspace.workspace_id.clone());
    transition.idempotency_key = Some(format!("mode-{suffix}"));
    transition.confirmation = Some(confirmation());
    let canary: WorkspaceResult = serde_json::from_value(result(&harness.call(
        &mut admin,
        RpcMethod::WorkspaceModeTransition,
        transition,
    )))
    .expect("canary workspace");

    let mut open = params(
        json!({
            "externalKey": format!("external-{suffix}"),
            "role": "root",
            "engaged": true,
        }),
        1_000,
    );
    open.workspace_id = Some(canary.workspace_id.clone());
    open.agent_id = Some("agent".to_owned());
    open.work_item_id = Some(WORK_ITEM.to_owned());
    open.idempotency_key = Some(format!("session-open-{suffix}"));
    let session: SessionResult = serde_json::from_value(result(&harness.call(
        connection,
        RpcMethod::SessionOpen,
        open,
    )))
    .expect("session result decodes");
    (canary, session)
}

fn hook_request(
    workspace: &WorkspaceResult,
    session: &SessionResult,
    event_id: &str,
    host_payload: Value,
) -> RequestParams<Value> {
    let mut request = session_params(
        workspace,
        session,
        "agent",
        json!({"hookEventId":event_id,"turnSeq":1,"hostPayload":host_payload}),
        100,
    );
    request.turn_id = Some("turn".to_owned());
    request.work_item_id = Some(WORK_ITEM.to_owned());
    request.idempotency_key = Some(format!("request-{event_id}"));
    request
}

fn resume_execution(
    harness: &LockHarness,
    connection: &mut ConnectionState,
    workspace: &WorkspaceResult,
    session: &SessionResult,
) {
    let mut request = session_params(
        workspace,
        session,
        "agent",
        json!({"operation":"execution.resume","payload":{}}),
        1_000,
    );
    request.work_item_id = Some(WORK_ITEM.to_owned());
    result(&harness.call(connection, RpcMethod::OperationExecute, request));
}

fn focused_test_event() -> Value {
    json!({"executionEvent":{"class":"focused-test","outputBytes":128}})
}

#[test]
fn concurrent_cargo_pretool_defers_the_second_session_with_retry_after_ms() {
    let temp = TempLockDir::new("hook-defer");
    let lock_path = temp.lock_file();
    let harness = LockHarness::new(|config| {
        config.cargo_lock_path = Some(lock_path.clone());
    });
    let mut connection = harness.connection(ClientKind::Hook);
    let (first_workspace, first) = engaged_session(&harness, &mut connection, "lock-first");
    let (second_workspace, second) = engaged_session(&harness, &mut connection, "lock-second");
    resume_execution(&harness, &mut connection, &first_workspace, &first);
    resume_execution(&harness, &mut connection, &second_workspace, &second);

    let first_pre = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &first_workspace,
            &first,
            "cargo-first",
            focused_test_event(),
        ),
    ));
    assert_eq!(first_pre["decision"], "allow");
    assert!(
        first_pre["executionDirective"]
            .get("retryAfterMs")
            .is_none(),
        "the lease holder carries no retry hint: {first_pre}"
    );

    let second_pre = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &second_workspace,
            &second,
            "cargo-second",
            focused_test_event(),
        ),
    ));
    assert_eq!(second_pre["decision"], "deny");
    let directive = &second_pre["executionDirective"];
    assert_eq!(directive["decision"], "require-progress");
    assert_eq!(directive["reasonCode"], "EXECUTION_RESOURCE_BUSY");
    assert_eq!(directive["retryAfterMs"], 1_000);

    let _ = result(&harness.call(
        &mut connection,
        RpcMethod::HookPostTool,
        hook_request(
            &first_workspace,
            &first,
            "cargo-first-done",
            json!({"executionEvent":{"class":"focused-test","outcome":"pass","outputBytes":128}}),
        ),
    ));

    let retry = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &second_workspace,
            &second,
            "cargo-second-retry",
            focused_test_event(),
        ),
    ));
    assert_eq!(
        retry["decision"], "allow",
        "after release the deferred session proceeds: {retry}"
    );
    assert!(
        retry["executionDirective"].get("retryAfterMs").is_none(),
        "the granted retry carries no retry hint: {retry}"
    );
}

#[test]
fn ttl_expiry_lets_the_second_session_proceed_without_post_tool() {
    let temp = TempLockDir::new("hook-ttl");
    let lock_path = temp.lock_file();
    let harness = LockHarness::new(|config| {
        config.cargo_lock_path = Some(lock_path.clone());
        config.cargo_lock_ttl_ms = 500;
    });
    let mut connection = harness.connection(ClientKind::Hook);
    let (first_workspace, first) = engaged_session(&harness, &mut connection, "ttl-first");
    let (second_workspace, second) = engaged_session(&harness, &mut connection, "ttl-second");
    resume_execution(&harness, &mut connection, &first_workspace, &first);
    resume_execution(&harness, &mut connection, &second_workspace, &second);

    let first_pre = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &first_workspace,
            &first,
            "cargo-ttl-first",
            focused_test_event(),
        ),
    ));
    assert_eq!(first_pre["decision"], "allow");

    let blocked = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &second_workspace,
            &second,
            "cargo-ttl-second",
            focused_test_event(),
        ),
    ));
    assert_eq!(blocked["decision"], "deny");
    assert_eq!(
        blocked["executionDirective"]["reasonCode"],
        "EXECUTION_RESOURCE_BUSY"
    );

    harness.clock.set(1_000 + 500);
    let after_ttl = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &second_workspace,
            &second,
            "cargo-ttl-second-retry",
            focused_test_event(),
        ),
    ));
    assert_eq!(
        after_ttl["decision"], "allow",
        "the forgotten lease is released at its TTL: {after_ttl}"
    );
}

#[test]
fn unbound_shadow_sessions_are_never_deferred() {
    let temp = TempLockDir::new("hook-shadow");
    let lock_path = temp.lock_file();
    let harness = LockHarness::new(|config| {
        config.cargo_lock_path = Some(lock_path.clone());
    });
    let mut connection = harness.connection(ClientKind::Hook);
    let (first_workspace, first) = engaged_session(&harness, &mut connection, "shadow-first");
    let (second_workspace, second) = engaged_session(&harness, &mut connection, "shadow-second");
    resume_execution(&harness, &mut connection, &first_workspace, &first);

    let first_pre = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &first_workspace,
            &first,
            "cargo-shadow-first",
            focused_test_event(),
        ),
    ));
    assert_eq!(first_pre["decision"], "allow");

    // The second session never resumed: it is unbound shadow and must not be
    // blocked by the resource arbiter during the rollout shadow stage.
    let shadow_pre = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &second_workspace,
            &second,
            "cargo-shadow-second",
            focused_test_event(),
        ),
    ));
    assert_eq!(shadow_pre["decision"], "allow");
    assert!(
        shadow_pre.get("executionDirective").is_none(),
        "shadow events carry no directive: {shadow_pre}"
    );
}
