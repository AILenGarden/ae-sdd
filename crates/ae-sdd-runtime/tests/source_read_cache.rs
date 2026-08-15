//! Bounded source-read cache on the execution Hook fast path.
//!
//! The cache key is `workspace + canonical path + content digest + range`: the
//! same key hits, a different content digest or range misses, and path
//! separators are canonicalized.  The LRU stores only the digest, the range
//! and a <=24 KiB excerpt, returned under per-session visibility; source
//! bodies are never persisted.  A PreTool source read that hits carries
//! `executionDirective.cachedReadRef`; a PostTool source read stores the
//! bounded excerpt from the bounded Hook payload.

mod support;

#[path = "../src/execution_cache.rs"]
mod execution_cache;

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
    MemoryPersistence, PersistencePort, RuntimeConfig, RuntimeResult, RuntimeService,
    SessionResult, WorkspaceResult,
};
use serde_json::{Value, json};
use uuid::Uuid;

use execution_cache::{MAX_SOURCE_READ_EXCERPT_BYTES, SourceReadKey, SourceReadVisibility};
use support::{TestClock, TestResolver, params, parity_transition_payload, result, session_params};

const WORK_ITEM: &str = "WORK-1";
const HEX_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HEX_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const READ_PATH: &str = "crates/ae-sdd-runtime/src/lib.rs";

struct ResumeBusiness {
    resume_response: Value,
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
            return Ok(self.resume_response.clone());
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

struct CacheHarness {
    runtime: Arc<RuntimeService>,
    persistence: Arc<MemoryPersistence>,
    token: String,
}

impl CacheHarness {
    fn new(configure: impl FnOnce(&mut RuntimeConfig)) -> Self {
        let mut config = RuntimeConfig::default();
        configure(&mut config);
        let token = "endpoint-source-cache-test".to_owned();
        let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
            Uuid::from_u128(81),
        )));
        let clock = Arc::new(TestClock::new(1_000));
        let business = Arc::new(ResumeBusiness {
            resume_response: resume_response_full(&test_capsule()),
            operation_calls: AtomicUsize::new(0),
        });
        let runtime = Arc::new(RuntimeService::new(
            config,
            BootId::from_uuid(Uuid::from_u128(82)),
            token.clone(),
            persistence.clone(),
            clock,
            Arc::new(TestResolver),
            business,
        ));
        Self {
            runtime,
            persistence,
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
                client_build: "test/source-read-cache".to_owned(),
                client_kind: kind,
                endpoint_token: SecretString::new(self.token.clone()),
                expected_boot_id: self.runtime.boot_id().to_string(),
                expected_policy_digest: self.runtime.policy_digest().to_owned(),
                adapter_id: None,
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

    fn durable_events_json(&self) -> String {
        let events = self.persistence.events_after(0, 512).expect("event page");
        serde_json::to_string(&events).expect("events serialize")
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
        ExecutionSliceId::new("slice-cache").expect("slice id"),
        1,
        "cache the active slice reads",
        Vec::new(),
        vec![ProjectRelativePath::new("crates/ae-sdd-runtime").expect("path scope")],
        Vec::new(),
        VerificationId::new("V-EFF-006").expect("verification id"),
        Vec::new(),
        "execution-capsule/slice-cache".to_owned(),
    )
    .expect("valid slice");
    ExecutionCapsuleV1::new(
        SchemaVersion::V1,
        WorkItemId::new(WORK_ITEM).expect("work item id"),
        StoryId::new("STORY-AE-SDD-SLICE-SUPERVISOR-001").expect("story id"),
        StateRevision::new(12),
        ArtifactDigest::digest(b"approved-plan"),
        PolicyDigest::digest(b"cache policy"),
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
    harness: &CacheHarness,
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
    harness: &CacheHarness,
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

fn source_read_event(digest: &str, start_line: u32, end_line: u32) -> Value {
    json!({
        "class":"source-read",
        "path":READ_PATH,
        "contentDigest":digest,
        "startLine":start_line,
        "endLine":end_line,
        "outputBytes":2_048,
        "outputDigest":digest,
    })
}

fn visibility(workspace: &'static str, session: &'static str) -> SourceReadVisibility<'static> {
    SourceReadVisibility::new(workspace, session)
}

fn read_key(
    workspace: &str,
    digest: &str,
    start_line: Option<u32>,
    end_line: Option<u32>,
) -> SourceReadKey {
    SourceReadKey::new(workspace, READ_PATH, digest, start_line, end_line)
}

#[test]
fn same_key_hits_and_returns_the_stored_excerpt() {
    let cache = execution_cache::SourceReadCache::new();
    let scope = visibility("workspace", "session-a");
    let key = read_key("workspace", HEX_A, Some(1), Some(40));

    let stored_ref = cache.put(&scope, &key, "fn main() {}", 8);
    assert!(stored_ref.starts_with("source-read:"), "{stored_ref}");

    let hit = cache.get(&scope, &key).expect("same key hits");
    assert_eq!(hit, stored_ref);
    assert_eq!(
        cache.excerpt(&scope, &key).as_deref(),
        Some("fn main() {}"),
        "the bounded excerpt is returned for the same key"
    );
}

#[test]
fn different_content_digest_misses() {
    let cache = execution_cache::SourceReadCache::new();
    let scope = visibility("workspace", "session-a");
    cache.put(
        &scope,
        &read_key("workspace", HEX_A, Some(1), Some(40)),
        "old body",
        8,
    );

    assert_eq!(
        cache.get(&scope, &read_key("workspace", HEX_B, Some(1), Some(40))),
        None,
        "a changed content digest invalidates the entry"
    );
}

#[test]
fn different_range_misses() {
    let cache = execution_cache::SourceReadCache::new();
    let scope = visibility("workspace", "session-a");
    cache.put(
        &scope,
        &read_key("workspace", HEX_A, Some(1), Some(40)),
        "ranged body",
        8,
    );

    assert_eq!(
        cache.get(&scope, &read_key("workspace", HEX_A, Some(41), Some(80))),
        None,
        "a shifted range is a different key"
    );
    assert_eq!(
        cache.get(&scope, &read_key("workspace", HEX_A, None, None)),
        None,
        "a whole-file read is a different key than a ranged read"
    );
}

#[test]
fn path_separators_are_canonicalized() {
    let cache = execution_cache::SourceReadCache::new();
    let scope = visibility("workspace", "session-a");
    let windows_key = SourceReadKey::new(
        "workspace",
        "crates\\ae-sdd-runtime\\src\\lib.rs",
        HEX_A,
        None,
        None,
    );
    cache.put(&scope, &windows_key, "body", 8);

    let unix_key = SourceReadKey::new("workspace", READ_PATH, HEX_A, None, None);
    assert!(
        cache.get(&scope, &unix_key).is_some(),
        "backslash and slash spellings resolve to the same canonical key"
    );
}

#[test]
fn excerpt_is_truncated_to_24_kib_at_a_char_boundary() {
    let cache = execution_cache::SourceReadCache::new();
    let scope = visibility("workspace", "session-a");
    let key = read_key("workspace", HEX_A, None, None);
    // 9_000 three-byte characters = 27_000 bytes, not aligned to the cap.
    let body = "界".repeat(9_000);
    cache.put(&scope, &key, &body, 8);

    let excerpt = cache.excerpt(&scope, &key).expect("excerpt stored");
    assert!(
        excerpt.len() <= MAX_SOURCE_READ_EXCERPT_BYTES,
        "excerpt is bounded, got {} bytes",
        excerpt.len()
    );
    assert!(
        excerpt.len() > MAX_SOURCE_READ_EXCERPT_BYTES - 3,
        "truncation stops at the last full character before the cap"
    );
}

#[test]
fn entries_are_visible_only_to_the_storing_session() {
    let cache = execution_cache::SourceReadCache::new();
    let key = read_key("workspace", HEX_A, Some(1), Some(40));
    cache.put(&visibility("workspace", "session-a"), &key, "body", 8);

    assert_eq!(
        cache.get(&visibility("workspace", "session-b"), &key),
        None,
        "another session in the same workspace cannot see the entry"
    );
    assert_eq!(
        cache.get(&visibility("other-workspace", "session-a"), &key),
        None,
        "another workspace cannot see the entry"
    );
}

#[test]
fn lru_evicts_the_least_recently_used_entry() {
    let cache = execution_cache::SourceReadCache::new();
    let scope = visibility("workspace", "session-a");
    let key_a = SourceReadKey::new("workspace", "src/a.rs", HEX_A, None, None);
    let key_b = SourceReadKey::new("workspace", "src/b.rs", HEX_A, None, None);
    let key_c = SourceReadKey::new("workspace", "src/c.rs", HEX_A, None, None);

    cache.put(&scope, &key_a, "a", 2);
    cache.put(&scope, &key_b, "b", 2);
    assert!(cache.get(&scope, &key_a).is_some(), "touch a");
    cache.put(&scope, &key_c, "c", 2);

    assert_eq!(cache.len(), 2, "capacity is bounded");
    assert_eq!(cache.get(&scope, &key_b), None, "b was least recently used");
    assert!(cache.get(&scope, &key_a).is_some(), "a survives");
    assert!(cache.get(&scope, &key_c).is_some(), "c survives");
    let stats = cache.stats();
    assert_eq!(stats.stores, 3);
    assert_eq!(stats.evictions, 1);
    assert!(stats.hits >= 3);
    assert_eq!(stats.misses, 1);
}

#[test]
fn post_tool_stores_and_pre_tool_returns_the_cached_read_ref() {
    let harness = CacheHarness::new(|_| {});
    let mut connection = harness.connection(ClientKind::Hook);
    let (workspace, session) = engaged_session(&harness, &mut connection, "cache-hit");
    resume_execution(&harness, &mut connection, &workspace, &session);

    let post = result(&harness.call(
        &mut connection,
        RpcMethod::HookPostTool,
        hook_request(
            &workspace,
            &session,
            "cache-store",
            json!({
                "executionEvent": source_read_event(HEX_A, 1, 40),
                "toolOutput": "pub const RUNTIME_BUILD: &str = ...;",
            }),
        ),
    ));
    assert_eq!(post["decision"], "allow");

    let pre = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &workspace,
            &session,
            "cache-read",
            json!({"executionEvent": source_read_event(HEX_A, 1, 40)}),
        ),
    ));
    assert_eq!(pre["decision"], "allow");
    let cached_ref = pre["executionDirective"]["cachedReadRef"]
        .as_str()
        .unwrap_or_else(|| panic!("cached read hit must carry cachedReadRef: {pre}"));
    assert!(cached_ref.starts_with("source-read:"), "{cached_ref}");
    assert_eq!(pre["executionDirective"]["decision"], "allow");
}

#[test]
fn pre_tool_with_a_changed_digest_carries_no_cached_read_ref() {
    let harness = CacheHarness::new(|_| {});
    let mut connection = harness.connection(ClientKind::Hook);
    let (workspace, session) = engaged_session(&harness, &mut connection, "cache-miss");
    resume_execution(&harness, &mut connection, &workspace, &session);

    let _ = result(&harness.call(
        &mut connection,
        RpcMethod::HookPostTool,
        hook_request(
            &workspace,
            &session,
            "cache-store-old",
            json!({
                "executionEvent": source_read_event(HEX_A, 1, 40),
                "toolOutput": "old body",
            }),
        ),
    ));

    let pre = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &workspace,
            &session,
            "cache-read-new-digest",
            json!({"executionEvent": source_read_event(HEX_B, 1, 40)}),
        ),
    ));
    assert_eq!(pre["decision"], "allow");
    assert!(
        pre["executionDirective"].get("cachedReadRef").is_none(),
        "a changed digest must miss: {pre}"
    );

    let other_range = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &workspace,
            &session,
            "cache-read-other-range",
            json!({"executionEvent": source_read_event(HEX_A, 41, 80)}),
        ),
    ));
    assert!(
        other_range["executionDirective"]
            .get("cachedReadRef")
            .is_none(),
        "a shifted range must miss: {other_range}"
    );
}

#[test]
fn another_session_cannot_see_the_cached_read() {
    let harness = CacheHarness::new(|_| {});
    let mut connection = harness.connection(ClientKind::Hook);
    let (workspace, first) = engaged_session(&harness, &mut connection, "cache-owner");
    resume_execution(&harness, &mut connection, &workspace, &first);

    let mut open = params(
        json!({
            "externalKey": "external-cache-visitor",
            "role": "root",
            "engaged": true,
        }),
        1_000,
    );
    open.workspace_id = Some(workspace.workspace_id.clone());
    open.agent_id = Some("agent".to_owned());
    open.work_item_id = Some(WORK_ITEM.to_owned());
    open.idempotency_key = Some("session-open-cache-visitor".to_owned());
    let second: SessionResult = serde_json::from_value(result(&harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        open,
    )))
    .expect("second session decodes");
    resume_execution(&harness, &mut connection, &workspace, &second);

    let _ = result(&harness.call(
        &mut connection,
        RpcMethod::HookPostTool,
        hook_request(
            &workspace,
            &first,
            "cache-store-owner",
            json!({
                "executionEvent": source_read_event(HEX_A, 1, 40),
                "toolOutput": "session body",
            }),
        ),
    ));

    let pre = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &workspace,
            &second,
            "cache-read-visitor",
            json!({"executionEvent": source_read_event(HEX_A, 1, 40)}),
        ),
    ));
    assert_eq!(pre["decision"], "allow");
    assert!(
        pre["executionDirective"].get("cachedReadRef").is_none(),
        "per-session visibility: another session must miss: {pre}"
    );
}

#[test]
fn durable_events_never_carry_the_source_body() {
    let harness = CacheHarness::new(|_| {});
    let mut connection = harness.connection(ClientKind::Hook);
    let (workspace, session) = engaged_session(&harness, &mut connection, "cache-body");
    resume_execution(&harness, &mut connection, &workspace, &session);

    let body = "secret-source-body-".repeat(512);
    let post = result(&harness.call(
        &mut connection,
        RpcMethod::HookPostTool,
        hook_request(
            &workspace,
            &session,
            "cache-store-body",
            json!({
                "executionEvent": source_read_event(HEX_A, 1, 40),
                "toolOutput": body,
            }),
        ),
    ));
    assert_eq!(post["decision"], "allow");

    let events = harness.durable_events_json();
    assert!(
        !events.contains("secret-source-body-"),
        "source bodies are never persisted in durable events: {} bytes",
        events.len()
    );
}
