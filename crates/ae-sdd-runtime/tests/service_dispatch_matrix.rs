mod support;

use std::str::FromStr;
use std::sync::{Arc, Mutex};

use ae_sdd_domain::{
    AgentRole, BootId, DesignRoute, EventStoreId, InputFingerprint, ProcessPhase, StateRevision,
    WorkScale,
};
use ae_sdd_flow::{FlowEnvironment, FlowInput, FlowSnapshot, RouteSelection};
use ae_sdd_protocol::{
    ClientKind, HandshakeRequest, JsonRpcRequest, PROTOCOL_RANGE_V1, RequestParams, RpcMethod,
    SecretString, StableErrorCode, WorkspaceMode,
};
use ae_sdd_runtime::{
    BoundJobIdentity, BusinessOperationPort, BusinessWorkspace, ConnectionState,
    ContextProjectionInput, DelegationReportPayload, DurableEvent, FlowSupervisor,
    MemoryPersistence, PersistencePort, RejectingBusinessPort, RuntimeConfig, RuntimeError,
    RuntimeJobRecord, RuntimeJobStatus, RuntimeResult, RuntimeService, SessionResult,
    WorkspaceResult,
};
use serde_json::{Value, json};
use uuid::Uuid;

use support::{
    Harness, TestClock, TestResolver, open_root_session, params, register_workspace, result,
    session_params, stable_error,
};

#[derive(Default)]
struct ScriptedBusiness {
    identities: Mutex<Vec<Option<BoundJobIdentity>>>,
}

impl BusinessOperationPort for ScriptedBusiness {
    fn execute(
        &self,
        _method: RpcMethod,
        _params: &RequestParams<Value>,
        _workspace: Option<&BusinessWorkspace>,
    ) -> RuntimeResult<Value> {
        Ok(json!({"ok":true}))
    }

    fn project_context(
        &self,
        _workspace: &BusinessWorkspace,
        work_item_id: &str,
        session_id: &str,
        role: AgentRole,
    ) -> RuntimeResult<ContextProjectionInput> {
        Ok(ContextProjectionInput {
            session_id: session_id.to_owned(),
            source_revision: 1,
            projection: json!({
                "workItemId":work_item_id,
                "role":format!("{role:?}").to_lowercase(),
            }),
        })
    }

    fn execute_job(
        &self,
        _workspace: &BusinessWorkspace,
        _entrypoint: &str,
        arguments: &Value,
    ) -> RuntimeResult<Value> {
        if arguments
            .get("adapterError")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(RuntimeError::new(
                StableErrorCode::OperationNotRegistered,
                "scripted job adapter failure",
            ));
        }
        Ok(arguments
            .get("businessResult")
            .cloned()
            .unwrap_or_else(|| arguments.clone()))
    }

    fn execute_trusted_job(
        &self,
        workspace: &BusinessWorkspace,
        _work_item_id: Option<&str>,
        identity: Option<&BoundJobIdentity>,
        entrypoint: &str,
        arguments: &Value,
    ) -> RuntimeResult<Value> {
        self.identities
            .lock()
            .expect("scripted identity lock")
            .push(identity.cloned());
        self.execute_job(workspace, entrypoint, arguments)
    }

    fn validate_delegation_artifacts(
        &self,
        _workspace: &BusinessWorkspace,
        _delegation_id: &str,
        _result: &Value,
    ) -> RuntimeResult<Value> {
        Err(RuntimeError::new(
            StableErrorCode::ChildResultInvalid,
            "not used by scheduler matrix",
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
            "not used by scheduler matrix",
        ))
    }
}

struct ScriptHarness {
    runtime: Arc<RuntimeService>,
    clock: Arc<TestClock>,
    persistence: Arc<MemoryPersistence>,
    business: Arc<ScriptedBusiness>,
    token: String,
}

struct JobSubmission<'a> {
    session: Option<&'a SessionResult>,
    entrypoint: &'a str,
    arguments: Value,
    key: &'a str,
    work_item_id: Option<&'a str>,
    expected_revision: Option<u64>,
    deadline_unix_ms: u64,
}

struct AcceptedDelegation {
    harness: Harness,
    connection: ConnectionState,
    workspace: WorkspaceResult,
    root: SessionResult,
    delegation_id: String,
    child_session_id: String,
}

fn accepted_delegation(suffix: &str) -> AcceptedDelegation {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection(ClientKind::HostAdapter);
    let mut register = params(
        json!({"adapterId":"host-delegation","capabilities":["create","attest"]}),
        1_000,
    );
    register.capability_token = Some(harness.host_credential());
    register.idempotency_key = Some(format!("host-register-{suffix}"));
    let registered = result(&harness.call(&mut host, RpcMethod::HostRegister, register));
    assert_eq!(registered["adapterId"], "host-delegation");

    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, suffix);
    let root = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "delegation-root",
        &format!("delegation-root-{suffix}"),
        Some("WORK"),
    );
    let mut create = session_params(
        &workspace,
        &root,
        "delegation-root",
        json!({
            "childRole":"series",
            "parentDelegationId":null,
            "inputRevision":1,
            "inputFingerprint":"a".repeat(64),
            "deadlineUnixMs":2_000,
            "adapterId":"host-delegation",
            "grant":{"operations":[],"capabilities":[],"paths":[]}
        }),
        1_000,
    );
    create.work_item_id = Some("WORK".to_owned());
    create.idempotency_key = Some(format!("delegation-create-{suffix}"));
    let created = result(&harness.call(&mut connection, RpcMethod::DelegationCreate, create));
    let delegation_id = created["delegationId"]
        .as_str()
        .expect("delegation ID")
        .to_owned();

    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":"host-delegation"}), 1_000),
    ));
    let action_id = action["actionId"]
        .as_str()
        .expect("Host action ID")
        .to_owned();
    let child_session_id = "00000000-0000-0000-0000-000000009301".to_owned();
    let mut ack = params(
        json!({
            "adapterId":"host-delegation",
            "ack":{
                "ackId":"00000000-0000-0000-0000-000000009302",
                "actionId":action_id,
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-delegation",
                "sessionId":child_session_id,
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some(format!("host-ack-{suffix}"));
    let acknowledged = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));
    assert_eq!(acknowledged["actionId"], action_id);

    let mut accept = params(
        json!({
            "delegationId":delegation_id,
            "claimId":"00000000-0000-0000-0000-000000009303",
            "actionId":action_id,
            "childSessionId":child_session_id,
            "expiresAtUnixMs":1_900,
        }),
        1_000,
    );
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.work_item_id = Some("WORK".to_owned());
    accept.idempotency_key = Some(format!("delegation-accept-{suffix}"));
    let accepted = result(&harness.call(&mut connection, RpcMethod::DelegationAccept, accept));
    assert_eq!(accepted["status"], "running");

    AcceptedDelegation {
        harness,
        connection,
        workspace,
        root,
        delegation_id,
        child_session_id,
    }
}

fn child_report(
    fixture: &AcceptedDelegation,
    summary: impl Into<String>,
    result: Value,
) -> DelegationReportPayload {
    DelegationReportPayload {
        delegation_id: fixture.delegation_id.clone(),
        input_revision: 1,
        input_fingerprint: "a".repeat(64),
        summary: summary.into(),
        result,
    }
}

impl ScriptHarness {
    fn new(mut config: RuntimeConfig) -> Self {
        config.policy_digest = ae_sdd_policy::policy_digest().to_hex();
        let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
            Uuid::from_u128(9_200),
        )));
        let clock = Arc::new(TestClock::new(1_000));
        let business = Arc::new(ScriptedBusiness::default());
        let token = "scripted-runtime-token".to_owned();
        let runtime = Arc::new(RuntimeService::new(
            config,
            BootId::from_uuid(Uuid::from_u128(9_201)),
            token.clone(),
            persistence.clone(),
            clock.clone(),
            Arc::new(TestResolver),
            business.clone(),
        ));
        Self {
            runtime,
            clock,
            persistence,
            business,
            token,
        }
    }

    fn connection(&self, client_kind: ClientKind) -> ConnectionState {
        let mut connection = ConnectionState::default();
        let response = self.raw(
            &mut connection,
            RpcMethod::RuntimeHandshake,
            serde_json::to_value(HandshakeRequest {
                protocol_range: PROTOCOL_RANGE_V1.to_owned(),
                client_build: "test/scripted".to_owned(),
                client_kind,
                endpoint_token: SecretString::new(self.token.clone()),
                expected_boot_id: self.runtime.boot_id().to_string(),
                expected_policy_digest: self.runtime.policy_digest().to_owned(),
                adapter_id: None,
            })
            .expect("scripted handshake serializes"),
        );
        assert!(response.get("result").is_some(), "{response}");
        connection
    }

    fn call(
        &self,
        connection: &mut ConnectionState,
        method: RpcMethod,
        request: RequestParams<Value>,
    ) -> Value {
        self.raw(
            connection,
            method,
            serde_json::to_value(request).expect("scripted params serialize"),
        )
    }

    fn raw(&self, connection: &mut ConnectionState, method: RpcMethod, request: Value) -> Value {
        let envelope =
            JsonRpcRequest::new(format!("{}-scripted", method.as_str()), method, request);
        serde_json::from_slice(&self.runtime.handle_payload(
            connection,
            &serde_json::to_vec(&envelope).expect("scripted request serializes"),
        ))
        .expect("scripted response is JSON")
    }

    fn register_workspace(
        &self,
        connection: &mut ConnectionState,
        suffix: &str,
    ) -> WorkspaceResult {
        let mut request = params(
            json!({
                "projectRoot":format!("C:/ae-sdd-tests/scripted-{suffix}"),
                "projectKey":format!("scripted-{suffix}"),
            }),
            1_000,
        );
        request.idempotency_key = Some(format!("scripted-workspace-{suffix}"));
        serde_json::from_value(result(&self.call(
            connection,
            RpcMethod::WorkspaceRegister,
            request,
        )))
        .expect("scripted workspace decodes")
    }

    fn open_root_session(
        &self,
        connection: &mut ConnectionState,
        workspace: &WorkspaceResult,
        work_item_id: &str,
    ) -> SessionResult {
        let mut request = params(
            json!({"externalKey":"scripted-root","role":"root","engaged":false}),
            1_000,
        );
        request.workspace_id = Some(workspace.workspace_id.clone());
        request.agent_id = Some("scripted-agent".to_owned());
        request.work_item_id = Some(work_item_id.to_owned());
        request.idempotency_key = Some("scripted-session-open".to_owned());
        serde_json::from_value(result(&self.call(
            connection,
            RpcMethod::SessionOpen,
            request,
        )))
        .expect("scripted root session decodes")
    }

    fn submit_job(
        &self,
        connection: &mut ConnectionState,
        workspace: &WorkspaceResult,
        submission: JobSubmission<'_>,
    ) -> Value {
        let JobSubmission {
            session,
            entrypoint,
            arguments,
            key,
            work_item_id,
            expected_revision,
            deadline_unix_ms,
        } = submission;
        let mut request = params(
            json!({
                "entrypoint":entrypoint,
                "arguments":arguments,
                "deadlineUnixMs":deadline_unix_ms,
            }),
            1_000,
        );
        request.workspace_id = Some(workspace.workspace_id.clone());
        request.idempotency_key = Some(key.to_owned());
        request.work_item_id = work_item_id.map(str::to_owned);
        request.expected_revision = expected_revision;
        if let Some(session) = session {
            request.agent_id = Some("scripted-agent".to_owned());
            request.session_id = Some(session.session_id.clone());
            request.capability_token = Some(session.capability_token.clone());
        }
        self.call(connection, RpcMethod::JobSubmit, request)
    }
}

fn flow_input(store_id: EventStoreId) -> FlowInput {
    FlowInput::new(
        FlowSnapshot::new(ProcessPhase::Initialized, StateRevision::new(1), 0),
        FlowEnvironment::new(
            store_id,
            InputFingerprint::digest(b"service-dispatch-matrix"),
            RouteSelection::new(WorkScale::Large, DesignRoute::Dr),
        ),
    )
}

fn append_flow_event(
    persistence: &MemoryPersistence,
    input: FlowInput,
    work_item_id: &str,
    kind: &str,
    detail: Value,
) {
    let mut payload = json!({
        "schemaVersion":"flow-test/v1",
        "idempotencyKey":format!("event-{work_item_id}"),
        "policyDigest":input.environment().policy_digest().to_string(),
        "inputFingerprint":input.environment().input_fingerprint().to_string(),
    });
    payload
        .as_object_mut()
        .expect("flow payload is an object")
        .extend(
            detail
                .as_object()
                .expect("flow detail is an object")
                .clone(),
        );
    persistence
        .append_event(DurableEvent {
            event_store_id: String::new(),
            event_seq: 0,
            boot_id: "boot-flow-matrix".to_owned(),
            kind: kind.to_owned(),
            workspace_id: Some("workspace-flow-matrix".to_owned()),
            session_id: Some("session-flow-matrix".to_owned()),
            work_item_id: Some(work_item_id.to_owned()),
            payload_digest: InputFingerprint::digest(work_item_id.as_bytes()).to_string(),
            payload,
        })
        .expect("flow event persists");
}

#[test]
fn missing_business_adapter_fails_closed_for_every_boundary() {
    let port = RejectingBusinessPort;
    let workspace = BusinessWorkspace {
        workspace_id: "workspace".to_owned(),
        canonical_root: "C:/workspace".to_owned(),
        project_key: "project".to_owned(),
        mode: WorkspaceMode::Legacy,
        agent_role: None,
        agent_grant: None,
        caller_kind: None,
        inventory_generation: 1,
    };
    let request = params(json!({}), 1_000);

    let gate = port
        .execute(RpcMethod::GateEvaluate, &request, Some(&workspace))
        .expect_err("missing Gate authority must fail closed");
    assert_eq!(gate.code(), StableErrorCode::GateError);

    let operation = port
        .execute(RpcMethod::OperationDescribe, &request, Some(&workspace))
        .expect_err("missing operation authority must fail closed");
    assert_eq!(operation.code(), StableErrorCode::OperationNotRegistered);

    let context = port
        .project_context(&workspace, "WORK", "session", AgentRole::Root)
        .expect_err("missing context authority must fail closed");
    assert_eq!(context.code(), StableErrorCode::ContextRevisionStale);

    let job = port
        .execute_job(&workspace, "gate.evaluate", &json!({}))
        .expect_err("missing job authority must fail closed");
    assert_eq!(job.code(), StableErrorCode::OperationNotRegistered);

    let artifacts = port
        .validate_delegation_artifacts(&workspace, "delegation", &json!({}))
        .expect_err("missing artifact authority must fail closed");
    assert_eq!(artifacts.code(), StableErrorCode::ChildResultInvalid);

    let cleanup = port
        .cleanup_delegation_memory(&workspace, "delegation", &json!({}), &json!({}))
        .expect_err("missing cleanup authority must fail closed");
    assert_eq!(cleanup.code(), StableErrorCode::ChildResultInvalid);
}

#[test]
fn flow_wire_projection_decodes_every_role_phase_gate_outcome_and_fault() {
    let store_id = EventStoreId::from_uuid(Uuid::from_u128(9_001));
    let persistence = Arc::new(MemoryPersistence::new(store_id));
    let supervisor = FlowSupervisor::new(persistence.clone());
    let input = flow_input(store_id);

    for (role_index, role) in ["root", "series", "task", "reviewer"]
        .into_iter()
        .enumerate()
    {
        for (phase_index, phase) in [
            "initialized",
            "route-selected",
            "requirement-analyzed",
            "dr-generated",
            "story-generated",
            "testcase-generated",
            "coding-process",
            "coding",
            "test-running",
            "code-reviewed",
            "completed",
            "paused",
        ]
        .into_iter()
        .enumerate()
        {
            let work_item_id = format!("ROLE-{role_index}-PHASE-{phase_index}");
            append_flow_event(
                persistence.as_ref(),
                input,
                &work_item_id,
                "flow.transition_requested",
                json!({"actorRole":role,"targetPhase":phase}),
            );
            let decision = supervisor
                .project("workspace-flow-matrix", &work_item_id, input)
                .expect("a well-formed transition request has a deterministic projection");
            assert!(
                FlowSupervisor::projection(&decision)["lastEventSeq"]
                    .as_u64()
                    .is_some_and(|sequence| sequence > 0)
            );
        }
    }

    for (index, gate) in [
        "G-00",
        "G-01",
        "G-02",
        "G-03",
        "G-04",
        "G-07",
        "G-08",
        "G-09",
        "G-10",
        "G-11",
        "G-12",
        "G-13",
        "G-14",
        "G-CODE-1",
        "G-CODEPLAN-SRC",
        "G-DR-CTX",
        "G-HTTP-1",
        "G-RA-1",
        "G-RA-2",
        "G-RA-3",
        "G-RA-4",
        "G-RA-5",
        "G-RA-6",
        "G-RA-FLOW-VIOLATION",
        "G-REVIEW-DEPTH",
        "G-STORY-CTX",
    ]
    .into_iter()
    .enumerate()
    {
        let work_item_id = format!("GATE-{index}");
        append_flow_event(
            persistence.as_ref(),
            input,
            &work_item_id,
            "flow.gate_completed",
            json!({"gateId":gate,"outcome":{"kind":"PASS"}}),
        );
        let error = supervisor
            .project("workspace-flow-matrix", &work_item_id, input)
            .expect_err("a Gate result without a pending transition is rejected");
        assert_eq!(error.code(), StableErrorCode::ExternalStateConflict);
    }

    let outcomes = [
        json!({"kind":"FAIL","findings":["FINDING"]}),
        json!({"kind":"ERROR","code":"GATE_ERROR","retryable":true}),
        json!({"kind":"TIMEOUT","deadlineMs":250}),
        json!({"kind":"CANCELLED","reason":"USER_ABORT"}),
        json!({
            "kind":"STALE",
            "changed":[
                "gate-id",
                "gate-implementation",
                "policy",
                "workspace",
                "work-item",
                "story",
                "state-revision",
                "fencing-token",
                "inventory-generation",
                "toolchain",
                "configuration",
                "input"
            ]
        }),
    ];
    for (index, outcome) in outcomes.into_iter().enumerate() {
        let work_item_id = format!("OUTCOME-{index}");
        append_flow_event(
            persistence.as_ref(),
            input,
            &work_item_id,
            "flow.gate_completed",
            json!({"gateId":"G-00","outcome":outcome}),
        );
        let error = supervisor
            .project("workspace-flow-matrix", &work_item_id, input)
            .expect_err("an out-of-order Gate result is rejected after decoding");
        assert_eq!(error.code(), StableErrorCode::ExternalStateConflict);
    }

    for (index, fault) in [
        "gate-worker",
        "event-store",
        "artifact-projection",
        "host-adapter",
    ]
    .into_iter()
    .enumerate()
    {
        let work_item_id = format!("FAULT-{index}");
        append_flow_event(
            persistence.as_ref(),
            input,
            &work_item_id,
            "flow.background_fault",
            json!({"fault":fault}),
        );
        let decision = supervisor
            .project("workspace-flow-matrix", &work_item_id, input)
            .expect("a typed background fault degrades the supervisor deterministically");
        assert_eq!(
            FlowSupervisor::projection(&decision)["health"]["status"],
            "degraded"
        );
    }
}

#[test]
fn flow_wire_rejects_malformed_events_and_bounded_history_overflow() {
    let store_id = EventStoreId::from_uuid(Uuid::from_u128(9_002));
    let persistence = Arc::new(MemoryPersistence::new(store_id));
    let supervisor = FlowSupervisor::new(persistence.clone());
    let input = flow_input(store_id);

    for (index, (kind, detail)) in [
        (
            "flow.transition_requested",
            json!({"actorRole":"invalid","targetPhase":"coding"}),
        ),
        (
            "flow.transition_requested",
            json!({"actorRole":"root","targetPhase":"invalid"}),
        ),
        (
            "flow.gate_completed",
            json!({"gateId":"G-UNKNOWN","outcome":{"kind":"PASS"}}),
        ),
        (
            "flow.gate_completed",
            json!({"gateId":"G-00","outcome":{"kind":"UNKNOWN"}}),
        ),
        (
            "flow.gate_completed",
            json!({"gateId":"G-00","outcome":{"kind":"FAIL","findings":[]}}),
        ),
        (
            "flow.gate_completed",
            json!({"gateId":"G-00","outcome":{"kind":"TIMEOUT","deadlineMs":0}}),
        ),
        (
            "flow.gate_completed",
            json!({"gateId":"G-00","outcome":{"kind":"STALE","changed":[]}}),
        ),
        ("flow.background_fault", json!({"fault":"invalid"})),
        ("flow.unknown", json!({})),
    ]
    .into_iter()
    .enumerate()
    {
        let work_item_id = format!("MALFORMED-{index}");
        append_flow_event(persistence.as_ref(), input, &work_item_id, kind, detail);
        let error = supervisor
            .project("workspace-flow-matrix", &work_item_id, input)
            .expect_err("malformed typed flow history must fail closed");
        assert_eq!(error.code(), StableErrorCode::ExternalStateConflict);
    }

    let empty_key = supervisor
        .request_transition(
            "boot",
            "workspace-flow-matrix",
            None,
            "EMPTY-KEY",
            "",
            input,
            AgentRole::Root,
            ProcessPhase::Paused,
        )
        .expect_err("an empty idempotency key is rejected");
    assert_eq!(empty_key.code(), StableErrorCode::OperationSchemaInvalid);

    for index in 0..=4_096 {
        append_flow_event(
            persistence.as_ref(),
            input,
            "OVERFLOW",
            "flow.prompt_accepted",
            json!({"ordinal":index}),
        );
    }
    let overflow = supervisor
        .project("workspace-flow-matrix", "OVERFLOW", input)
        .expect_err("flow replay history is bounded");
    assert_eq!(overflow.code(), StableErrorCode::SubscriberBackpressure);
}

#[test]
fn session_admission_rejects_untrusted_shapes_and_closes_durably() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut connection, "session-matrix");

    let invalid_cases = [
        (
            json!({
                "externalKey":"context-injection",
                "role":"root",
                "engaged":false,
                "context":{"forged":true}
            }),
            None,
            StableErrorCode::RoleOperationForbidden,
        ),
        (
            json!({"externalKey":"engaged","role":"root","engaged":true}),
            None,
            StableErrorCode::RoleOperationForbidden,
        ),
        (
            json!({"externalKey":"root-id","role":"root","engaged":false}),
            Some("00000000-0000-0000-0000-000000009101"),
            StableErrorCode::RoleOperationForbidden,
        ),
        (
            json!({"externalKey":"child-no-id","role":"series","engaged":false}),
            None,
            StableErrorCode::DelegationAttestationFailed,
        ),
        (
            json!({"externalKey":"child-no-delegation","role":"series","engaged":false}),
            Some("00000000-0000-0000-0000-000000009102"),
            StableErrorCode::DelegationAttestationFailed,
        ),
    ];
    for (index, (payload, session_id, expected)) in invalid_cases.into_iter().enumerate() {
        let mut request = params(payload, 1_000);
        request.workspace_id = Some(workspace.workspace_id.clone());
        request.agent_id = Some(format!("invalid-agent-{index}"));
        request.session_id = session_id.map(str::to_owned);
        request.idempotency_key = Some(format!("invalid-session-{index}"));
        let response = harness.call(&mut connection, RpcMethod::SessionOpen, request);
        assert_eq!(stable_error(&response), expected.as_str());
    }

    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "root-agent",
        "stable-external",
        Some("WORK"),
    );

    let mut rebound = params(
        json!({"externalKey":"stable-external","role":"root","engaged":false}),
        1_000,
    );
    rebound.workspace_id = Some(workspace.workspace_id.clone());
    rebound.agent_id = Some("different-agent".to_owned());
    rebound.work_item_id = Some("WORK".to_owned());
    rebound.idempotency_key = Some("rebind-different-agent".to_owned());
    assert_eq!(
        stable_error(&harness.call(&mut connection, RpcMethod::SessionOpen, rebound)),
        StableErrorCode::TurnIdentityMismatch.as_str()
    );

    let mut missing_capability =
        session_params(&workspace, &session, "root-agent", json!({}), 1_000);
    missing_capability.capability_token = None;
    missing_capability.idempotency_key = Some("heartbeat-no-capability".to_owned());
    assert_eq!(
        stable_error(&harness.call(
            &mut connection,
            RpcMethod::SessionHeartbeat,
            missing_capability,
        )),
        StableErrorCode::SessionExpired.as_str()
    );

    let mut wrong_agent = session_params(&workspace, &session, "different-agent", json!({}), 1_000);
    wrong_agent.idempotency_key = Some("heartbeat-wrong-agent".to_owned());
    assert_eq!(
        stable_error(&harness.call(&mut connection, RpcMethod::SessionHeartbeat, wrong_agent,)),
        StableErrorCode::TurnIdentityMismatch.as_str()
    );

    let mut malformed_capability =
        session_params(&workspace, &session, "root-agent", json!({}), 1_000);
    malformed_capability.capability_token = Some("not-a-capability".to_owned());
    malformed_capability.idempotency_key = Some("heartbeat-malformed-capability".to_owned());
    assert_eq!(
        stable_error(&harness.call(
            &mut connection,
            RpcMethod::SessionHeartbeat,
            malformed_capability,
        )),
        StableErrorCode::SessionExpired.as_str()
    );

    let mut close = session_params(
        &workspace,
        &session,
        "root-agent",
        json!({"reason":"matrix-complete"}),
        1_000,
    );
    close.idempotency_key = Some("session-close-matrix".to_owned());
    let closed: SessionResult = serde_json::from_value(result(&harness.call(
        &mut connection,
        RpcMethod::SessionClose,
        close,
    )))
    .expect("closed session projection decodes");
    assert_eq!(closed.session_id, session.session_id);

    let mut after_close = session_params(&workspace, &session, "root-agent", json!({}), 1_000);
    after_close.idempotency_key = Some("heartbeat-after-close".to_owned());
    assert_eq!(
        stable_error(&harness.call(&mut connection, RpcMethod::SessionHeartbeat, after_close,)),
        StableErrorCode::SessionExpired.as_str()
    );
}

#[test]
fn active_session_capacity_is_enforced_without_eviction() {
    let harness = Harness::new(RuntimeConfig {
        max_sessions: 1,
        ..RuntimeConfig::default()
    });
    let mut connection = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut connection, "session-capacity");
    let first = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-one",
        "external-one",
        None,
    );
    assert!(!first.session_id.is_empty());

    let mut second = params(
        json!({"externalKey":"external-two","role":"root","engaged":false}),
        1_000,
    );
    second.workspace_id = Some(workspace.workspace_id);
    second.agent_id = Some("agent-two".to_owned());
    second.idempotency_key = Some("session-capacity-second".to_owned());
    assert_eq!(
        stable_error(&harness.call(&mut connection, RpcMethod::SessionOpen, second)),
        StableErrorCode::SubscriberBackpressure.as_str()
    );
}

#[test]
fn authenticated_host_pressure_requires_binding_and_triggers_correlated_compact() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection(ClientKind::HostAdapter);

    let mut missing_credential = params(
        json!({"adapterId":"host-pressure","capabilities":["pressure","compact"]}),
        1_000,
    );
    missing_credential.idempotency_key = Some("host-missing-credential".to_owned());
    assert_eq!(
        stable_error(&harness.call(&mut host, RpcMethod::HostRegister, missing_credential,)),
        StableErrorCode::EndpointAuthFailed.as_str()
    );

    let mut register = params(
        json!({"adapterId":"host-pressure","capabilities":["pressure","compact"]}),
        1_000,
    );
    register.capability_token = Some(harness.host_credential());
    register.idempotency_key = Some("host-pressure-register".to_owned());
    let registered = result(&harness.call(&mut host, RpcMethod::HostRegister, register));
    assert_eq!(registered["adapterId"], "host-pressure");

    let mut hook = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut hook, "host-pressure");
    let session = open_root_session(
        &harness,
        &mut hook,
        &workspace,
        "pressure-agent",
        "pressure-external",
        Some("WORK"),
    );

    let mut wrong_connection = session_params(
        &workspace,
        &session,
        "pressure-agent",
        json!({
            "adapterId":"host-pressure",
            "contextGeneration":0,
            "sampleSeq":1,
            "usedTokens":10,
            "contextWindowTokens":100,
            "observedAtUnixMs":1_000
        }),
        1_000,
    );
    wrong_connection.idempotency_key = Some("pressure-wrong-connection".to_owned());
    assert_eq!(
        stable_error(&harness.call(&mut hook, RpcMethod::HostPressureReport, wrong_connection,)),
        StableErrorCode::RoleOperationForbidden.as_str()
    );

    for (sequence, expected) in [
        (1_u64, "HighSample { consecutive: 1 }"),
        (2, "TriggerCompact"),
    ] {
        let mut pressure = session_params(
            &workspace,
            &session,
            "pressure-agent",
            json!({
                "adapterId":"host-pressure",
                "contextGeneration":0,
                "sampleSeq":sequence,
                "usedTokens":90,
                "contextWindowTokens":100,
                "observedAtUnixMs":1_000 + sequence
            }),
            1_000,
        );
        pressure.idempotency_key = Some(format!("pressure-{sequence}"));
        let response = result(&harness.call(&mut host, RpcMethod::HostPressureReport, pressure));
        assert_eq!(response["decision"], expected);
        if sequence == 2 {
            assert_eq!(response["compact"]["status"], "compact-requested");
        } else {
            assert!(response["compact"].is_null());
        }
    }
}

#[test]
fn scheduler_projects_every_terminal_outcome_and_adapter_error() {
    let harness = ScriptHarness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let workspace = harness.register_workspace(&mut connection, "job-outcomes");

    let cases = [
        (
            "pass",
            json!({"businessResult":{"outcome":"PASS","detail":"ok"}}),
            RuntimeJobStatus::Pass,
            None,
        ),
        (
            "fail",
            json!({"businessResult":{"outcome":"FAIL","findings":["blocked"]}}),
            RuntimeJobStatus::Fail,
            None,
        ),
        (
            "stale",
            json!({"businessResult":{"outcome":"STALE","changed":["policy"]}}),
            RuntimeJobStatus::Stale,
            None,
        ),
        (
            "timeout-result",
            json!({"businessResult":{"outcome":"TIMEOUT","errorCode":"SCRIPT_TIMEOUT"}}),
            RuntimeJobStatus::Timeout,
            Some("SCRIPT_TIMEOUT"),
        ),
        (
            "cancelled-result",
            json!({"businessResult":{"outcome":"CANCELLED","errorCode":"SCRIPT_CANCELLED"}}),
            RuntimeJobStatus::Cancelled,
            Some("SCRIPT_CANCELLED"),
        ),
        (
            "unknown-result",
            json!({"businessResult":{"outcome":"UNKNOWN"}}),
            RuntimeJobStatus::Error,
            Some(StableErrorCode::OperationSchemaInvalid.as_str()),
        ),
        (
            "adapter-error",
            json!({"adapterError":true}),
            RuntimeJobStatus::Error,
            Some(StableErrorCode::OperationNotRegistered.as_str()),
        ),
    ];

    for (entrypoint, arguments, expected_status, expected_error) in cases {
        let submitted: RuntimeJobRecord = serde_json::from_value(result(&harness.submit_job(
            &mut connection,
            &workspace,
            JobSubmission {
                session: None,
                entrypoint,
                arguments,
                key: &format!("job-{entrypoint}"),
                work_item_id: None,
                expected_revision: None,
                deadline_unix_ms: 10_000,
            },
        )))
        .expect("submitted job decodes");
        assert_eq!(submitted.status, RuntimeJobStatus::Queued);
        assert!(
            harness
                .runtime
                .run_one_pending_job()
                .expect("one scripted job runs")
        );
        let terminal = harness
            .persistence
            .load_job(&submitted.job_id)
            .expect("terminal job reads")
            .expect("terminal job exists");
        assert_eq!(terminal.status, expected_status, "{entrypoint}");
        assert_eq!(
            terminal.error_code.as_deref(),
            expected_error,
            "{entrypoint}"
        );
    }

    let queued_timeout: RuntimeJobRecord = serde_json::from_value(result(&harness.submit_job(
        &mut connection,
        &workspace,
        JobSubmission {
            session: None,
            entrypoint: "deadline-timeout",
            arguments: json!({"businessResult":{"outcome":"PASS"}}),
            key: "job-deadline-timeout",
            work_item_id: None,
            expected_revision: None,
            deadline_unix_ms: 1_001,
        },
    )))
    .expect("deadline job decodes");
    harness.clock.set(1_001);
    assert!(
        harness
            .runtime
            .run_one_pending_job()
            .expect("expired queued job runs to timeout")
    );
    let timed_out = harness
        .persistence
        .load_job(&queued_timeout.job_id)
        .expect("timed-out job reads")
        .expect("timed-out job exists");
    assert_eq!(timed_out.status, RuntimeJobStatus::Timeout);
    assert_eq!(
        timed_out.error_code.as_deref(),
        Some(StableErrorCode::GateTimeout.as_str())
    );
    assert!(
        !harness
            .runtime
            .run_one_pending_job()
            .expect("empty queue is a successful no-op")
    );
}

#[test]
fn toolset_jobs_bind_root_identity_and_validate_project_receipts() {
    let harness = ScriptHarness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let workspace = harness.register_workspace(&mut connection, "toolset-jobs");
    let session = harness.open_root_session(&mut connection, &workspace, "WORK");
    let fingerprint = "a".repeat(64);

    let missing_revision = harness.submit_job(
        &mut connection,
        &workspace,
        JobSubmission {
            session: Some(&session),
            entrypoint: "toolset.required",
            arguments: json!({"inputFingerprint":fingerprint}),
            key: "toolset-missing-revision",
            work_item_id: Some("WORK"),
            expected_revision: None,
            deadline_unix_ms: 10_000,
        },
    );
    assert_eq!(
        stable_error(&missing_revision),
        StableErrorCode::OperationSchemaInvalid.as_str()
    );

    let mismatched_revision = harness.submit_job(
        &mut connection,
        &workspace,
        JobSubmission {
            session: Some(&session),
            entrypoint: "toolset.receipt.record",
            arguments: json!({
                "sourceRevision":6,
                "plan":{"inputFingerprint":"a".repeat(64)},
            }),
            key: "toolset-mismatched-revision",
            work_item_id: Some("WORK"),
            expected_revision: Some(7),
            deadline_unix_ms: 10_000,
        },
    );
    assert_eq!(
        stable_error(&mismatched_revision),
        StableErrorCode::OperationSchemaInvalid.as_str()
    );

    let invalid_fingerprint = harness.submit_job(
        &mut connection,
        &workspace,
        JobSubmission {
            session: Some(&session),
            entrypoint: "toolset.required",
            arguments: json!({"inputFingerprint":"NOT-HEX"}),
            key: "toolset-invalid-fingerprint",
            work_item_id: Some("WORK"),
            expected_revision: Some(7),
            deadline_unix_ms: 10_000,
        },
    );
    assert_eq!(
        stable_error(&invalid_fingerprint),
        StableErrorCode::OperationSchemaInvalid.as_str()
    );

    let invalid_receipts = [
        json!({
            "outcome":"PASS",
            "receiptLocator":"receipts/one.json",
            "projectReceiptDigest":"b".repeat(64),
            "revisionAfter":8,
        }),
        json!({
            "outcome":"PASS",
            "mutationId":"mutation-two",
            "receiptLocator":"x".repeat(1_025),
            "projectReceiptDigest":"b".repeat(64),
            "revisionAfter":8,
        }),
        json!({
            "outcome":"PASS",
            "mutationId":"mutation-three",
            "receiptLocator":"receipts/three.json",
            "projectReceiptDigest":"NOT-HEX",
            "revisionAfter":8,
        }),
        json!({
            "outcome":"PASS",
            "mutationId":"mutation-four",
            "receiptLocator":"receipts/four.json",
            "projectReceiptDigest":"b".repeat(64),
        }),
    ];
    for (index, business_result) in invalid_receipts.into_iter().enumerate() {
        let submitted: RuntimeJobRecord = serde_json::from_value(result(&harness.submit_job(
            &mut connection,
            &workspace,
            JobSubmission {
                session: Some(&session),
                entrypoint: "toolset.receipt.record",
                arguments: json!({
                    "sourceRevision":7,
                    "plan":{"inputFingerprint":"a".repeat(64)},
                    "businessResult":business_result,
                }),
                key: &format!("toolset-invalid-receipt-{index}"),
                work_item_id: Some("WORK"),
                expected_revision: Some(7),
                deadline_unix_ms: 10_000,
            },
        )))
        .expect("invalid receipt job still queues");
        assert!(
            harness
                .runtime
                .run_one_pending_job()
                .expect("invalid receipt job reaches terminal state")
        );
        let terminal = harness
            .persistence
            .load_job(&submitted.job_id)
            .expect("invalid receipt job reads")
            .expect("invalid receipt job exists");
        assert_eq!(terminal.status, RuntimeJobStatus::Error);
        assert_eq!(
            terminal.error_code.as_deref(),
            Some(StableErrorCode::OperationSchemaInvalid.as_str())
        );
        assert!(terminal.project_receipt_digest.is_none());
    }

    let valid: RuntimeJobRecord = serde_json::from_value(result(&harness.submit_job(
        &mut connection,
        &workspace,
        JobSubmission {
            session: Some(&session),
            entrypoint: "toolset.receipt.record",
            arguments: json!({
                "sourceRevision":7,
                "plan":{"inputFingerprint":"a".repeat(64)},
                "businessResult":{
                    "outcome":"PASS",
                    "mutationId":"mutation-valid",
                    "receiptLocator":"receipts/valid.json",
                    "projectReceiptDigest":"c".repeat(64),
                    "revisionAfter":8,
                },
            }),
            key: "toolset-valid-receipt",
            work_item_id: Some("WORK"),
            expected_revision: Some(7),
            deadline_unix_ms: 10_000,
        },
    )))
    .expect("valid toolset job queues");
    assert!(
        harness
            .runtime
            .run_one_pending_job()
            .expect("valid toolset job runs")
    );
    let terminal = harness
        .persistence
        .load_job(&valid.job_id)
        .expect("valid toolset job reads")
        .expect("valid toolset job exists");
    assert_eq!(terminal.status, RuntimeJobStatus::Pass);
    assert_eq!(terminal.source_revision, Some(8));
    assert_eq!(terminal.mutation_id.as_deref(), Some("mutation-valid"));
    assert_eq!(
        terminal.receipt_locator.as_deref(),
        Some("receipts/valid.json")
    );
    assert_eq!(
        terminal.project_receipt_digest.as_deref(),
        Some(&*"c".repeat(64))
    );

    let identities = harness
        .business
        .identities
        .lock()
        .expect("trusted identities read");
    let root_identity = identities
        .iter()
        .flatten()
        .last()
        .expect("strict toolset job carries a captured identity");
    assert_eq!(root_identity.session_id, session.session_id);
    assert_eq!(root_identity.root_session_id, session.session_id);
    assert!(root_identity.delegation_id.is_none());
    assert_eq!(root_identity.context_generation, 0);
}

#[test]
fn job_replay_access_cancel_and_capacity_fail_closed() {
    let harness = ScriptHarness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let workspace = harness.register_workspace(&mut connection, "job-access");
    let other_workspace = harness.register_workspace(&mut connection, "job-access-other");
    let arguments = json!({"businessResult":{"outcome":"PASS"}});
    let first = harness.submit_job(
        &mut connection,
        &workspace,
        JobSubmission {
            session: None,
            entrypoint: "cancel-matrix",
            arguments: arguments.clone(),
            key: "cancel-matrix-key",
            work_item_id: None,
            expected_revision: None,
            deadline_unix_ms: 10_000,
        },
    );
    let job: RuntimeJobRecord =
        serde_json::from_value(result(&first)).expect("cancellable job decodes");

    let replay = harness.submit_job(
        &mut connection,
        &workspace,
        JobSubmission {
            session: None,
            entrypoint: "cancel-matrix",
            arguments,
            key: "cancel-matrix-key",
            work_item_id: None,
            expected_revision: None,
            deadline_unix_ms: 10_000,
        },
    );
    assert_eq!(replay, first);

    let conflict = harness.submit_job(
        &mut connection,
        &workspace,
        JobSubmission {
            session: None,
            entrypoint: "cancel-matrix",
            arguments: json!({"businessResult":{"outcome":"FAIL"}}),
            key: "cancel-matrix-key",
            work_item_id: None,
            expected_revision: None,
            deadline_unix_ms: 10_000,
        },
    );
    assert_eq!(
        stable_error(&conflict),
        StableErrorCode::IdempotencyKeyReused.as_str()
    );

    let mut wrong_workspace = params(json!({"jobId":job.job_id}), 1_000);
    wrong_workspace.workspace_id = Some(other_workspace.workspace_id);
    assert_eq!(
        stable_error(&harness.call(&mut connection, RpcMethod::JobStatus, wrong_workspace,)),
        StableErrorCode::ProjectMismatch.as_str()
    );

    let mut cancel = params(json!({"jobId":job.job_id}), 1_000);
    cancel.workspace_id = Some(workspace.workspace_id.clone());
    cancel.idempotency_key = Some("cancel-first".to_owned());
    let cancelled: RuntimeJobRecord = serde_json::from_value(result(&harness.call(
        &mut connection,
        RpcMethod::JobCancel,
        cancel,
    )))
    .expect("cancelled job decodes");
    assert_eq!(cancelled.status, RuntimeJobStatus::Cancelled);

    let mut replay_cancel = params(json!({"jobId":job.job_id}), 1_000);
    replay_cancel.workspace_id = Some(workspace.workspace_id.clone());
    replay_cancel.idempotency_key = Some("cancel-first".to_owned());
    let replayed = result(&harness.call(&mut connection, RpcMethod::JobCancel, replay_cancel));
    assert_eq!(
        replayed,
        serde_json::to_value(&cancelled).expect("job serializes")
    );

    let mut late_cancel = params(json!({"jobId":job.job_id}), 1_000);
    late_cancel.workspace_id = Some(workspace.workspace_id);
    late_cancel.idempotency_key = Some("cancel-late".to_owned());
    assert_eq!(
        stable_error(&harness.call(&mut connection, RpcMethod::JobCancel, late_cancel,)),
        StableErrorCode::JobNotCancellable.as_str()
    );
    assert!(
        harness
            .runtime
            .run_one_pending_job()
            .expect("cancelled queue entry is skipped")
    );
    assert!(
        !harness
            .runtime
            .run_one_pending_job()
            .expect("queue is empty after cancelled entry")
    );

    let mut unknown_status = params(json!({"jobId":"missing-job"}), 1_000);
    unknown_status.workspace_id = Some("missing-workspace".to_owned());
    assert_eq!(
        stable_error(&harness.call(&mut connection, RpcMethod::JobStatus, unknown_status,)),
        StableErrorCode::OperationSchemaInvalid.as_str()
    );

    let capacity = ScriptHarness::new(RuntimeConfig {
        max_jobs: 1,
        ..RuntimeConfig::default()
    });
    let mut capacity_connection = capacity.connection(ClientKind::Cli);
    let capacity_workspace = capacity.register_workspace(&mut capacity_connection, "job-capacity");
    let queued = capacity.submit_job(
        &mut capacity_connection,
        &capacity_workspace,
        JobSubmission {
            session: None,
            entrypoint: "capacity-one",
            arguments: json!({"businessResult":{"outcome":"PASS"}}),
            key: "capacity-one",
            work_item_id: None,
            expected_revision: None,
            deadline_unix_ms: 10_000,
        },
    );
    assert!(queued.get("result").is_some(), "{queued}");
    let overflow = capacity.submit_job(
        &mut capacity_connection,
        &capacity_workspace,
        JobSubmission {
            session: None,
            entrypoint: "capacity-two",
            arguments: json!({"businessResult":{"outcome":"PASS"}}),
            key: "capacity-two",
            work_item_id: None,
            expected_revision: None,
            deadline_unix_ms: 10_000,
        },
    );
    assert_eq!(
        stable_error(&overflow),
        StableErrorCode::SubscriberBackpressure.as_str()
    );
}

#[test]
fn delegation_receipts_are_bound_before_collect_and_replay_idempotently() {
    let fixture = accepted_delegation("delegation-receipts");
    let supervisor = fixture.harness.runtime.delegation_supervisor();

    let outsider = supervisor
        .status("outside-session", &fixture.delegation_id)
        .expect_err("unrelated sessions cannot inspect a delegation");
    assert_eq!(outsider.code(), StableErrorCode::RoleOperationForbidden);

    let early_collect = supervisor
        .collect(&fixture.root.session_id, &fixture.delegation_id)
        .expect_err("unstaged child output cannot be collected");
    assert_eq!(early_collect.code(), StableErrorCode::ChildResultInvalid);
    let missing_material = supervisor
        .completion_material(&fixture.delegation_id)
        .expect_err("an accepted child has no completion material yet");
    assert_eq!(missing_material.code(), StableErrorCode::ChildResultInvalid);

    let wrong_child = supervisor
        .report(
            "outside-child",
            child_report(
                &fixture,
                "wrong child",
                json!({"memorySnapshotDigest":"a".repeat(64)}),
            ),
        )
        .expect_err("only the attested child may report");
    assert_eq!(wrong_child.code(), StableErrorCode::RoleOperationForbidden);

    let mut stale = child_report(
        &fixture,
        "stale input",
        json!({"memorySnapshotDigest":"a".repeat(64)}),
    );
    stale.input_revision = 2;
    let stale = supervisor
        .report(&fixture.child_session_id, stale)
        .expect_err("stale child input is rejected");
    assert_eq!(stale.code(), StableErrorCode::ChildResultInvalid);

    for summary in [String::new(), "x".repeat(8_193)] {
        let error = supervisor
            .report(
                &fixture.child_session_id,
                child_report(
                    &fixture,
                    summary,
                    json!({"memorySnapshotDigest":"a".repeat(64)}),
                ),
            )
            .expect_err("child summaries are bounded");
        assert_eq!(error.code(), StableErrorCode::ChildResultTooLarge);
    }

    for forbidden_result in [
        json!({
            "nested":{"transcript":"unbounded"},
            "memorySnapshotDigest":"a".repeat(64),
        }),
        json!({
            "nested":[{"fullStdout":"unbounded"}],
            "memorySnapshotDigest":"a".repeat(64),
        }),
    ] {
        let error = supervisor
            .report(
                &fixture.child_session_id,
                child_report(&fixture, "forbidden body", forbidden_result),
            )
            .expect_err("transcript-shaped child output is rejected recursively");
        assert_eq!(error.code(), StableErrorCode::ChildResultInvalid);
    }

    let oversized = supervisor
        .report(
            &fixture.child_session_id,
            child_report(
                &fixture,
                "oversized body",
                json!({
                    "boundedField":"x".repeat(65_536),
                    "memorySnapshotDigest":"a".repeat(64),
                }),
            ),
        )
        .expect_err("canonical child output is bounded");
    assert_eq!(oversized.code(), StableErrorCode::ChildResultTooLarge);

    for result_body in [
        json!({"outcome":"succeeded"}),
        json!({"outcome":"succeeded","memorySnapshotDigest":"UPPERCASE"}),
    ] {
        let error = supervisor
            .report(
                &fixture.child_session_id,
                child_report(&fixture, "invalid memory snapshot", result_body),
            )
            .expect_err("memory snapshot identity must be canonical");
        assert_eq!(error.code(), StableErrorCode::ChildResultInvalid);
    }

    let report = child_report(
        &fixture,
        "bounded child result",
        json!({
            "outcome":"succeeded",
            "findings":[],
            "deliverables":[],
            "requestedAction":null,
            "memorySnapshotDigest":"a".repeat(64),
        }),
    );
    let staged = supervisor
        .report(&fixture.child_session_id, report.clone())
        .expect("valid child result stages");
    assert_eq!(staged.status, "result-staged");
    let replayed_stage = supervisor
        .report(&fixture.child_session_id, report)
        .expect("identical staged result replays");
    assert_eq!(replayed_stage, staged);

    let changed_stage = supervisor
        .report(
            &fixture.child_session_id,
            child_report(
                &fixture,
                "different result",
                json!({"memorySnapshotDigest":"b".repeat(64)}),
            ),
        )
        .expect_err("a different result cannot replace staged output");
    assert_eq!(changed_stage.code(), StableErrorCode::ChildResultInvalid);

    let invalid_artifact = supervisor
        .artifacts_validated(
            &fixture.delegation_id,
            json!({
                "schemaVersion":"delegation-artifact-validation/v1",
                "delegationId":fixture.delegation_id,
                "resultDigest":"b".repeat(64),
                "artifacts":[],
            }),
        )
        .expect_err("artifact receipt must bind the staged digest");
    assert_eq!(invalid_artifact.code(), StableErrorCode::ChildResultInvalid);

    let artifact_receipt = json!({
        "schemaVersion":"delegation-artifact-validation/v1",
        "delegationId":fixture.delegation_id,
        "resultDigest":staged.result_digest,
        "artifacts":[],
    });
    let validated = supervisor
        .artifacts_validated(&fixture.delegation_id, artifact_receipt.clone())
        .expect("matching artifact receipt advances the lifecycle");
    assert_eq!(validated.status, "artifacts-validated");
    assert_eq!(
        supervisor
            .artifacts_validated(&fixture.delegation_id, artifact_receipt)
            .expect("artifact receipt replay")
            .status,
        "artifacts-validated"
    );

    let invalid_cleanup = supervisor
        .memory_cleaned(
            &fixture.delegation_id,
            json!({
                "schemaVersion":"delegation-memory-cleanup/v1",
                "delegationId":fixture.delegation_id,
                "memorySnapshotDigest":"a".repeat(64),
                "cleanupDigest":"not-a-digest",
                "cleanedAtUnixMs":1_000,
            }),
        )
        .expect_err("cleanup receipt requires a canonical digest");
    assert_eq!(invalid_cleanup.code(), StableErrorCode::ChildResultInvalid);

    let cleanup_receipt = json!({
        "schemaVersion":"delegation-memory-cleanup/v1",
        "delegationId":fixture.delegation_id,
        "memorySnapshotDigest":"a".repeat(64),
        "cleanupDigest":"c".repeat(64),
        "cleanedAtUnixMs":1_000,
    });
    let cleaned = supervisor
        .memory_cleaned(&fixture.delegation_id, cleanup_receipt.clone())
        .expect("matching cleanup receipt advances the lifecycle");
    assert_eq!(cleaned.status, "memory-cleaned");
    assert_eq!(
        supervisor
            .memory_cleaned(&fixture.delegation_id, cleanup_receipt)
            .expect("cleanup receipt replay")
            .status,
        "memory-cleaned"
    );

    let wrong_parent = supervisor
        .collect("outside-parent", &fixture.delegation_id)
        .expect_err("only the parent may collect");
    assert_eq!(wrong_parent.code(), StableErrorCode::RoleOperationForbidden);
    let collected = supervisor
        .collect(&fixture.root.session_id, &fixture.delegation_id)
        .expect("validated and cleaned output collects");
    assert_eq!(collected["status"], "completed");
    assert_eq!(collected["outcome"], "succeeded");
    assert_eq!(
        supervisor
            .collect(&fixture.root.session_id, &fixture.delegation_id)
            .expect("completed collect replays"),
        collected
    );

    let completed_cancel = supervisor
        .cancel(&fixture.root.session_id, &fixture.delegation_id)
        .expect_err("completed delegation cannot be cancelled");
    assert_eq!(completed_cancel.code(), StableErrorCode::JobNotCancellable);
}

#[test]
fn delegation_cancel_and_spawn_depth_fail_closed() {
    let mut fixture = accepted_delegation("delegation-cancel");
    let supervisor = fixture.harness.runtime.delegation_supervisor();

    let wrong_parent = supervisor
        .cancel("outside-parent", &fixture.delegation_id)
        .expect_err("only the parent may cancel");
    assert_eq!(wrong_parent.code(), StableErrorCode::RoleOperationForbidden);

    let mut invalid_reason = session_params(
        &fixture.workspace,
        &fixture.root,
        "delegation-root",
        json!({"delegationId":fixture.delegation_id,"reason":"bad\nreason"}),
        1_000,
    );
    invalid_reason.work_item_id = Some("WORK".to_owned());
    invalid_reason.idempotency_key = Some("cancel-invalid-reason".to_owned());
    assert_eq!(
        stable_error(&fixture.harness.call(
            &mut fixture.connection,
            RpcMethod::DelegationCancel,
            invalid_reason,
        )),
        StableErrorCode::OperationSchemaInvalid.as_str()
    );

    let mut cancel = session_params(
        &fixture.workspace,
        &fixture.root,
        "delegation-root",
        json!({"delegationId":fixture.delegation_id}),
        1_000,
    );
    cancel.work_item_id = Some("WORK".to_owned());
    cancel.idempotency_key = Some("cancel-default-reason".to_owned());
    let cancelled = result(&fixture.harness.call(
        &mut fixture.connection,
        RpcMethod::DelegationCancel,
        cancel,
    ));
    assert_eq!(cancelled["status"], "cancelled");
    assert_eq!(cancelled["cancellationReason"], "user-abort");

    let report_after_cancel = supervisor
        .report(
            &fixture.child_session_id,
            child_report(
                &fixture,
                "late report",
                json!({"memorySnapshotDigest":"a".repeat(64)}),
            ),
        )
        .expect_err("cancelled delegations reject late output");
    assert_eq!(
        report_after_cancel.code(),
        StableErrorCode::ChildResultInvalid
    );

    let mut invalid_depth = session_params(
        &fixture.workspace,
        &fixture.root,
        "delegation-root",
        json!({
            "childRole":"task",
            "parentDelegationId":null,
            "inputRevision":1,
            "inputFingerprint":"a".repeat(64),
            "deadlineUnixMs":2_000,
            "adapterId":"host-delegation",
            "grant":{"operations":[],"capabilities":[],"paths":[]}
        }),
        1_000,
    );
    invalid_depth.work_item_id = Some("WORK".to_owned());
    invalid_depth.idempotency_key = Some("invalid-spawn-depth".to_owned());
    assert_eq!(
        stable_error(&fixture.harness.call(
            &mut fixture.connection,
            RpcMethod::DelegationCreate,
            invalid_depth,
        )),
        StableErrorCode::RunDepthExceeded.as_str()
    );

    let missing = supervisor
        .status(&fixture.root.session_id, "missing-delegation")
        .expect_err("unknown delegation identity is rejected");
    assert_eq!(missing.code(), StableErrorCode::ChildResultInvalid);
}

#[test]
fn handshake_publishes_execution_supervisor_capability_without_direct_rpc_methods() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = ConnectionState::default();
    let envelope = JsonRpcRequest::new(
        "runtime.handshake-execution-capability",
        RpcMethod::RuntimeHandshake,
        serde_json::to_value(HandshakeRequest {
            protocol_range: PROTOCOL_RANGE_V1.to_owned(),
            client_build: "test/execution-capability".to_owned(),
            client_kind: ClientKind::Cli,
            endpoint_token: SecretString::new("endpoint-test-token"),
            expected_boot_id: harness.runtime.boot_id().to_string(),
            expected_policy_digest: harness.runtime.policy_digest().to_owned(),
            adapter_id: None,
        })
        .expect("handshake serializes"),
    );
    let bytes = serde_json::to_vec(&envelope).expect("handshake request serializes");
    let response: Value =
        serde_json::from_slice(&harness.runtime.handle_payload(&mut connection, &bytes))
            .expect("handshake response is JSON");

    let handshake = result(&response);
    let capabilities = handshake["capabilities"]
        .as_array()
        .expect("capabilities is an array");
    assert!(
        capabilities
            .iter()
            .any(|capability| capability == "execution-supervisor-v1"),
        "handshake must publish execution-supervisor-v1: {capabilities:?}"
    );
    assert_eq!(
        handshake["operationSchemaDigest"].as_str(),
        Some(ae_sdd_operations::operation_schema_digest().as_str()),
        "handshake publishes the live operation registry digest"
    );

    for method in [
        "execution.resume",
        "execution.slice.start",
        "execution.slice.record",
    ] {
        assert!(
            RpcMethod::from_str(method).is_err(),
            "{method} must reuse operation.execute, not a direct RPC method"
        );
    }
}
