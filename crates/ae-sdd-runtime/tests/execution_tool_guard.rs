//! Execution supervisor wiring through the daemon session and Hook fast path.
//!
//! `hostPayload.executionEvent` must strict-decode (malformed input fails
//! closed), an old host without the field is only recorded as `unclassified`
//! shadow, and a successful `execution.resume` binds the capsule digest plus
//! the supervisor checkpoint to the authenticated session so PreTool events
//! are adjudicated: a broad verification before the focused GREEN is denied
//! with `EXECUTION_PROGRESS_REQUIRED`, while admissible events carry an
//! `executionDirective` with the frozen output budget.  PostTool appends a
//! bounded execution event (classification, byte count and digests only —
//! never the tool output body).

mod support;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

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
    DurableEvent, MemoryPersistence, PersistencePort, RuntimeConfig, RuntimeResult, RuntimeService,
    SessionResult, WorkspaceResult,
};
use serde_json::{Value, json};
use uuid::Uuid;

use support::{
    TestClock, TestResolver, params, parity_transition_payload, result, session_params,
    stable_error,
};

const WORK_ITEM: &str = "WORK-1";
const HEX_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct ExecutionBusiness {
    resume_response: Mutex<Value>,
    operation_calls: AtomicUsize,
}

impl BusinessOperationPort for ExecutionBusiness {
    fn execute(
        &self,
        _method: RpcMethod,
        params: &RequestParams<Value>,
        _workspace: Option<&BusinessWorkspace>,
    ) -> RuntimeResult<Value> {
        self.operation_calls.fetch_add(1, Ordering::AcqRel);
        if params.payload.get("operation").and_then(Value::as_str) == Some("execution.resume") {
            return Ok(self
                .resume_response
                .lock()
                .expect("resume response lock")
                .clone());
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
            "cleanupDigest":HEX_A,
            "cleanedAtUnixMs":1_000,
        }))
    }
}

struct ExecutionHarness {
    runtime: Arc<RuntimeService>,
    business: Arc<ExecutionBusiness>,
    persistence: Arc<MemoryPersistence>,
    token: String,
}

impl ExecutionHarness {
    fn new(resume_response: Value) -> Self {
        let config = RuntimeConfig::default();
        let token = "endpoint-execution-test".to_owned();
        let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
            Uuid::from_u128(71),
        )));
        let clock = Arc::new(TestClock::new(1_000));
        let business = Arc::new(ExecutionBusiness {
            resume_response: Mutex::new(resume_response),
            operation_calls: AtomicUsize::new(0),
        });
        let runtime = Arc::new(RuntimeService::new(
            config,
            BootId::from_uuid(Uuid::from_u128(72)),
            token.clone(),
            persistence.clone(),
            clock,
            Arc::new(TestResolver),
            business.clone(),
        ));
        Self {
            runtime,
            business,
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
                client_build: "test/execution-guard".to_owned(),
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

    fn execution_events(&self) -> Vec<DurableEvent> {
        self.persistence
            .events_after(0, 256)
            .expect("event page")
            .into_iter()
            .filter(|event| event.kind == "execution.tool")
            .collect()
    }

    fn set_resume_response(&self, response: Value) {
        *self.business.resume_response.lock().expect("resume lock") = response;
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
        ExecutionSliceId::new("slice-guard").expect("slice id"),
        1,
        "supervise the active slice",
        Vec::new(),
        vec![ProjectRelativePath::new("crates/ae-sdd-runtime").expect("path scope")],
        Vec::new(),
        VerificationId::new("V-EFF-005").expect("verification id"),
        Vec::new(),
        "execution-capsule/slice-guard".to_owned(),
    )
    .expect("valid slice");
    ExecutionCapsuleV1::new(
        SchemaVersion::V1,
        WorkItemId::new(WORK_ITEM).expect("work item id"),
        StoryId::new("STORY-AE-SDD-SLICE-SUPERVISOR-001").expect("story id"),
        StateRevision::new(12),
        ArtifactDigest::digest(b"approved-plan"),
        PolicyDigest::digest(b"guard policy"),
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

fn capsule_digest() -> String {
    format!(
        "sha256:{}",
        ArtifactDigest::digest(b"execution-capsule/v1").to_hex()
    )
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
            "capsuleDigest": capsule_digest(),
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

fn resume_response_no_change() -> Value {
    json!({
        "changed": false,
        "revisionBefore": null,
        "revisionAfter": null,
        "receiptDigest": null,
        "data": {
            "projectionKind": "no-change",
            "contextRevision": 12,
            "capsuleDigest": capsule_digest(),
            "capsule": Value::Null,
            "nextAction": {
                "kind":"execute-approved-slice",
                "activeOrdinal":1,
                "queueDigest":format!("sha256:{}", "b".repeat(64)),
            },
            "authorityRefreshCount": 1,
        },
    })
}

fn register(
    harness: &ExecutionHarness,
    connection: &mut ConnectionState,
    suffix: &str,
) -> WorkspaceResult {
    let mut request = params(
        json!({
            "projectRoot": format!("C:/ae-sdd-tests/{suffix}"),
            "projectKey": format!("project-{suffix}"),
        }),
        1_000,
    );
    request.idempotency_key = Some(format!("workspace-{suffix}"));
    serde_json::from_value(result(&harness.call(
        connection,
        RpcMethod::WorkspaceRegister,
        request,
    )))
    .expect("workspace result decodes")
}

fn confirmation() -> ConfirmationRef {
    ConfirmationRef {
        confirmation_id: "confirmation".to_owned(),
        approved_by: "user".to_owned(),
        approved_at: "2026-01-01T00:00:00Z".to_owned(),
    }
}

fn make_canary(harness: &ExecutionHarness, workspace: &WorkspaceResult) -> WorkspaceResult {
    let mut admin = harness.connection(ClientKind::Admin);
    let mut drain = params(json!({"stop":false}), 1_000);
    drain.idempotency_key = Some("drain-execution".to_owned());
    drain.confirmation = Some(confirmation());
    let _ = result(&harness.call(&mut admin, RpcMethod::RuntimeDrain, drain));
    let mut transition = params(
        parity_transition_payload(WorkspaceMode::RustCanary, 1_000),
        1_000,
    );
    transition.workspace_id = Some(workspace.workspace_id.clone());
    transition.idempotency_key = Some("mode-execution".to_owned());
    transition.confirmation = Some(confirmation());
    serde_json::from_value(result(&harness.call(
        &mut admin,
        RpcMethod::WorkspaceModeTransition,
        transition,
    )))
    .expect("canary workspace")
}

fn open_engaged_root(
    harness: &ExecutionHarness,
    connection: &mut ConnectionState,
    workspace: &WorkspaceResult,
    external_key: &str,
) -> SessionResult {
    let mut request = params(
        json!({
            "externalKey": external_key,
            "role": "root",
            "engaged": true,
        }),
        1_000,
    );
    request.workspace_id = Some(workspace.workspace_id.clone());
    request.agent_id = Some("agent".to_owned());
    request.work_item_id = Some(WORK_ITEM.to_owned());
    request.idempotency_key = Some(format!("session-open-{external_key}"));
    serde_json::from_value(result(&harness.call(
        connection,
        RpcMethod::SessionOpen,
        request,
    )))
    .expect("session result decodes")
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
    harness: &ExecutionHarness,
    connection: &mut ConnectionState,
    workspace: &WorkspaceResult,
    session: &SessionResult,
) -> Value {
    let mut request = session_params(
        workspace,
        session,
        "agent",
        json!({"operation":"execution.resume","payload":{}}),
        1_000,
    );
    request.work_item_id = Some(WORK_ITEM.to_owned());
    result(&harness.call(connection, RpcMethod::OperationExecute, request))
}

fn engaged_session(
    harness: &ExecutionHarness,
    connection: &mut ConnectionState,
    suffix: &str,
) -> (WorkspaceResult, SessionResult) {
    let workspace = register(harness, connection, suffix);
    let canary = make_canary(harness, &workspace);
    let session = open_engaged_root(harness, connection, &canary, "external");
    (canary, session)
}

#[test]
fn malformed_execution_event_fails_closed_before_any_decision() {
    let harness = ExecutionHarness::new(resume_response_full(&test_capsule()));
    let mut connection = harness.connection(ClientKind::Hook);
    let (workspace, session) = engaged_session(&harness, &mut connection, "guard-malformed");

    let unknown_field = harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &workspace,
            &session,
            "event-unknown-field",
            json!({"executionEvent":{"class":"broad-test","mystery":true}}),
        ),
    );
    assert_eq!(stable_error(&unknown_field), "OPERATION_SCHEMA_INVALID");

    let wrong_shape = harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &workspace,
            &session,
            "event-wrong-shape",
            json!({"executionEvent":{"class":"broad-test","outputBytes":"lots"}}),
        ),
    );
    assert_eq!(stable_error(&wrong_shape), "OPERATION_SCHEMA_INVALID");
}

#[test]
fn unknown_execution_event_class_fails_closed() {
    let harness = ExecutionHarness::new(resume_response_full(&test_capsule()));
    let mut connection = harness.connection(ClientKind::Hook);
    let (workspace, session) = engaged_session(&harness, &mut connection, "guard-class");

    let response = harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &workspace,
            &session,
            "event-unknown-class",
            json!({"executionEvent":{"class":"warp-drive"}}),
        ),
    );
    assert_eq!(stable_error(&response), "OPERATION_SCHEMA_INVALID");
}

#[test]
fn missing_execution_event_records_shadow_unclassified_without_blocking() {
    let harness = ExecutionHarness::new(resume_response_full(&test_capsule()));
    let mut connection = harness.connection(ClientKind::Hook);
    let (workspace, session) = engaged_session(&harness, &mut connection, "guard-shadow");
    resume_execution(&harness, &mut connection, &workspace, &session);

    let decision = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(&workspace, &session, "event-shadow", json!({"tool":"read"})),
    ));
    assert_eq!(decision["decision"], "allow");
    assert!(
        decision.get("executionDirective").is_none(),
        "shadow events carry no directive: {decision}"
    );
    let events = harness.execution_events();
    assert_eq!(events.len(), 1, "one bounded shadow record: {events:?}");
    assert_eq!(events[0].payload["class"], "unclassified");
}

#[test]
fn broad_test_before_focused_green_is_denied_with_progress_directive() {
    let harness = ExecutionHarness::new(resume_response_full(&test_capsule()));
    let mut connection = harness.connection(ClientKind::Hook);
    let (workspace, session) = engaged_session(&harness, &mut connection, "guard-broad");
    resume_execution(&harness, &mut connection, &workspace, &session);

    let focused = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &workspace,
            &session,
            "event-focused-red",
            json!({"executionEvent":{"class":"focused-test","outputBytes":128}}),
        ),
    ));
    assert_eq!(focused["decision"], "allow");

    let broad = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &workspace,
            &session,
            "event-broad",
            json!({"executionEvent":{"class":"broad-test","outputBytes":64}}),
        ),
    ));
    assert_eq!(broad["decision"], "deny");
    let directive = &broad["executionDirective"];
    assert_eq!(directive["decision"], "require-progress");
    assert_eq!(directive["reasonCode"], "EXECUTION_PROGRESS_REQUIRED");

    let events = harness.execution_events();
    assert_eq!(
        events.len(),
        2,
        "both classified events recorded: {events:?}"
    );
    assert_eq!(events[1].payload["class"], "broad-test");
    assert_eq!(events[1].payload["decision"], "require-progress");
}

#[test]
fn focused_green_then_broad_test_is_allowed_with_output_budget() {
    let harness = ExecutionHarness::new(resume_response_full(&test_capsule()));
    let mut connection = harness.connection(ClientKind::Hook);
    let (workspace, session) = engaged_session(&harness, &mut connection, "guard-green");
    resume_execution(&harness, &mut connection, &workspace, &session);

    let post = result(&harness.call(
        &mut connection,
        RpcMethod::HookPostTool,
        hook_request(
            &workspace,
            &session,
            "event-focused-green",
            json!({"executionEvent":{"class":"focused-test","outcome":"pass","outputBytes":128}}),
        ),
    ));
    assert_eq!(post["decision"], "allow");

    let broad = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &workspace,
            &session,
            "event-broad-green",
            json!({"executionEvent":{"class":"broad-test","outputBytes":64}}),
        ),
    ));
    assert_eq!(broad["decision"], "allow");
    let directive = &broad["executionDirective"];
    assert_eq!(directive["decision"], "allow");
    assert_eq!(directive["outputBudgetBytes"], 65_536);
}

#[test]
fn classified_event_without_a_bound_session_stays_shadow() {
    let harness = ExecutionHarness::new(resume_response_full(&test_capsule()));
    let mut connection = harness.connection(ClientKind::Hook);
    let (workspace, session) = engaged_session(&harness, &mut connection, "guard-unbound");

    let broad = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &workspace,
            &session,
            "event-unbound-broad",
            json!({"executionEvent":{"class":"broad-test","outputBytes":64}}),
        ),
    ));
    assert_eq!(broad["decision"], "allow");
    assert!(broad.get("executionDirective").is_none());
    assert!(harness.execution_events().is_empty());
}

#[test]
fn no_change_resume_keeps_the_existing_binding() {
    let harness = ExecutionHarness::new(resume_response_full(&test_capsule()));
    let mut connection = harness.connection(ClientKind::Hook);
    let (workspace, session) = engaged_session(&harness, &mut connection, "guard-no-change");
    resume_execution(&harness, &mut connection, &workspace, &session);

    harness.set_resume_response(resume_response_no_change());
    resume_execution(&harness, &mut connection, &workspace, &session);

    let broad = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(
            &workspace,
            &session,
            "event-no-change-broad",
            json!({"executionEvent":{"class":"broad-test","outputBytes":64}}),
        ),
    ));
    assert_eq!(broad["decision"], "deny");
    assert_eq!(
        broad["executionDirective"]["reasonCode"],
        "EXECUTION_PROGRESS_REQUIRED"
    );
}

#[test]
fn post_tool_appends_a_bounded_execution_event_without_tool_output() {
    let harness = ExecutionHarness::new(resume_response_full(&test_capsule()));
    let mut connection = harness.connection(ClientKind::Hook);
    let (workspace, session) = engaged_session(&harness, &mut connection, "guard-bounded");
    resume_execution(&harness, &mut connection, &workspace, &session);

    let post = result(&harness.call(
        &mut connection,
        RpcMethod::HookPostTool,
        hook_request(
            &workspace,
            &session,
            "event-source-read",
            json!({
                "executionEvent":{
                    "class":"source-read",
                    "path":"crates/ae-sdd-runtime/src/lib.rs",
                    "contentDigest":HEX_A,
                    "startLine":1,
                    "endLine":40,
                    "outputBytes":2_048,
                    "outputDigest":HEX_A,
                },
                "toolOutput":"x".repeat(4_096),
            }),
        ),
    ));
    assert_eq!(post["decision"], "allow");

    let events = harness.execution_events();
    assert_eq!(events.len(), 1, "one bounded execution event: {events:?}");
    let payload = &events[0].payload;
    assert_eq!(payload["class"], "source-read");
    assert_eq!(payload["outputBytes"], 2_048);
    let encoded = serde_json::to_vec(payload).expect("payload serializes");
    assert!(
        encoded.len() <= 512,
        "execution event stays bounded, got {} bytes",
        encoded.len()
    );
    let text = String::from_utf8(encoded).expect("payload is UTF-8");
    assert!(
        !text.contains(&"x".repeat(4_096)),
        "tool output body never enters the execution event"
    );
}
