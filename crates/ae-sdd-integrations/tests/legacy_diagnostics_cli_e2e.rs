use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;

use ae_sdd_domain::{BootId, EventStoreId};
use ae_sdd_integrations::{FileWorkspaceResolver, NativeBusinessAdapter};
use ae_sdd_protocol::{
    ClientKind, HandshakeRequest, JsonRpcRequest, PROTOCOL_RANGE_V1, PROTOCOL_VERSION_V1,
    RequestParams, RpcMethod, SecretString, StableErrorCode, WorkspaceMode,
};
use ae_sdd_runtime::{
    BusinessOperationPort, ClockPort, ConnectionState, MemoryPersistence, PersistencePort,
    RuntimeConfig, RuntimeService, SessionResult, WorkspaceResult,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

#[allow(dead_code, unused_imports)]
#[path = "../../../bins/ae-sdd-cli/src/legacy/mod.rs"]
mod legacy;

const NOW_MS: u64 = 1_000;
const ENDPOINT_TOKEN: &str = "diagnostic-e2e-token";

struct FixedClock;

impl ClockPort for FixedClock {
    fn now_unix_ms(&self) -> u64 {
        NOW_MS
    }
}

struct Harness {
    runtime: Arc<RuntimeService>,
    connection: ConnectionState,
    workspace: WorkspaceResult,
    agent_id: String,
    sessions: BTreeMap<String, SessionResult>,
}

impl Harness {
    fn new(root: &TempDir) -> Self {
        prepare_workspace(root);
        let event_store_id = EventStoreId::from_uuid(Uuid::from_u128(701));
        let persistence = Arc::new(MemoryPersistence::new(event_store_id));
        let persistence_port: Arc<dyn PersistencePort> = persistence;
        let business: Arc<dyn BusinessOperationPort> = Arc::new(NativeBusinessAdapter::new(
            root.path().join("runtime.sqlite3"),
            event_store_id,
            BootId::from_uuid(Uuid::from_u128(702)),
            ae_sdd_policy::policy_digest().to_hex(),
            Arc::clone(&persistence_port),
        ));
        let resolver = Arc::new(
            FileWorkspaceResolver::new([root.path().to_path_buf()]).expect("workspace resolver"),
        );
        let runtime = Arc::new(RuntimeService::new(
            RuntimeConfig::default(),
            BootId::from_uuid(Uuid::from_u128(703)),
            ENDPOINT_TOKEN,
            persistence_port,
            Arc::new(FixedClock),
            resolver,
            business,
        ));
        let mut connection = ConnectionState::default();
        let handshake = HandshakeRequest {
            protocol_range: PROTOCOL_RANGE_V1.to_owned(),
            client_build: "diagnostic-e2e".to_owned(),
            client_kind: ClientKind::Cli,
            endpoint_token: SecretString::new(ENDPOINT_TOKEN.to_owned()),
            expected_boot_id: runtime.boot_id().to_string(),
            expected_policy_digest: runtime.policy_digest().to_owned(),
            adapter_id: None,
        };
        result(call(
            &runtime,
            &mut connection,
            RpcMethod::RuntimeHandshake,
            serde_json::to_value(handshake).expect("handshake JSON"),
        ));
        let mut register = params(
            json!({
                "projectRoot":root.path(),
                "projectKey":"diagnostic-e2e",
                "mode":WorkspaceMode::Shadow,
            }),
            1_000,
        );
        register.idempotency_key = Some("register-diagnostic-e2e".to_owned());
        let workspace = serde_json::from_value(result(call(
            &runtime,
            &mut connection,
            RpcMethod::WorkspaceRegister,
            serde_json::to_value(register).expect("register JSON"),
        )))
        .expect("workspace result");
        Self {
            runtime,
            connection,
            workspace,
            agent_id: "agent-diagnostic".to_owned(),
            sessions: BTreeMap::new(),
        }
    }

    fn open_session(&mut self, work_item: &str, external_key: &str) -> SessionResult {
        let mut request = params(
            json!({
                "externalKey":external_key,
                "role":"root",
                "engaged":false,
            }),
            1_000,
        );
        request.workspace_id = Some(self.workspace.workspace_id.clone());
        request.work_item_id = Some(work_item.to_owned());
        request.agent_id = Some(self.agent_id.clone());
        request.idempotency_key = Some(format!("open-{external_key}"));
        serde_json::from_value(result(call(
            &self.runtime,
            &mut self.connection,
            RpcMethod::SessionOpen,
            serde_json::to_value(request).expect("session JSON"),
        )))
        .expect("session result")
    }

    fn identity_for(&mut self, work_item: &str) -> SessionResult {
        if let Some(session) = self.sessions.get(work_item) {
            return session.clone();
        }
        let session = self.open_session(work_item, &format!("diagnostic-{work_item}"));
        self.sessions.insert(work_item.to_owned(), session.clone());
        session
    }

    fn submit(&mut self, id: &str, business: &[&str], work_item: &str, key: &str) -> Value {
        let session = self.identity_for(work_item);
        let route = legacy::resolve_command_id(id).expect("diagnostic route");
        let entrypoint = match &route.target {
            legacy::LegacyTarget::Rpc {
                method: RpcMethod::JobSubmit,
                adapter: legacy::LegacyRpcAdapter::JobSubmission { entrypoint, .. },
            } => entrypoint.clone(),
            target => panic!("not a job route: {target:?}"),
        };
        let mut argv = business
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        argv.extend([
            "--workspace-id".to_owned(),
            self.workspace.workspace_id.clone(),
            "--work-item-id".to_owned(),
            work_item.to_owned(),
            "--agent-id".to_owned(),
            self.agent_id.clone(),
            "--session-id".to_owned(),
            session.session_id.clone(),
            "--capability-token".to_owned(),
            session.capability_token.clone(),
            "--idempotency-key".to_owned(),
            key.to_owned(),
        ]);
        let invocation =
            legacy::parse_rpc_invocation(&route, RpcMethod::JobSubmit, &argv, |_| None)
                .expect("diagnostic argv");
        let mut request = match invocation.request {
            legacy::LegacyRequestSource::Synthesized(request) => *request,
            legacy::LegacyRequestSource::ExplicitJson(_) => panic!("synthesized request expected"),
        };
        legacy::adapt_job_submission(&route, &entrypoint, &mut request, NOW_MS)
            .expect("job adapter");
        let submitted = result(call(
            &self.runtime,
            &mut self.connection,
            RpcMethod::JobSubmit,
            serde_json::to_value(request).expect("job JSON"),
        ));
        assert_eq!(submitted["workItemId"], work_item);
        assert_eq!(submitted["sessionId"], session.session_id);
        submitted
    }

    fn run_and_status(&mut self, submitted: &Value, work_item: Option<&str>) -> Value {
        assert!(self.runtime.run_one_pending_job().expect("run job"));
        self.status(submitted, work_item)
    }

    fn status(&mut self, submitted: &Value, work_item: Option<&str>) -> Value {
        let session_id = submitted["sessionId"]
            .as_str()
            .expect("strict diagnostic session binding");
        let session = self
            .sessions
            .values()
            .find(|candidate| candidate.session_id == session_id)
            .cloned()
            .expect("known diagnostic session");
        self.status_as(submitted, work_item, &session)
    }

    fn status_as(
        &mut self,
        submitted: &Value,
        work_item: Option<&str>,
        session: &SessionResult,
    ) -> Value {
        let mut request = params(json!({"jobId":submitted["jobId"]}), 1_000);
        request.workspace_id = Some(self.workspace.workspace_id.clone());
        request.work_item_id = work_item.map(str::to_owned);
        request.agent_id = Some(self.agent_id.clone());
        request.session_id = Some(session.session_id.clone());
        request.capability_token = Some(session.capability_token.clone());
        call(
            &self.runtime,
            &mut self.connection,
            RpcMethod::JobStatus,
            serde_json::to_value(request).expect("status JSON"),
        )
    }
}

#[test]
fn three_diagnostics_reach_native_scheduler_and_return_terminal_passes() {
    let root = TempDir::new().expect("tempdir");
    let mut harness = Harness::new(&root);
    let cases: [(&str, &[&str]); 3] = [
        (
            "gate doc-storage",
            &["--path", "design/story.md", "--intent", "STORY"],
        ),
        ("iteration-check", &[]),
        ("update-check", &["--only", "UC-01"]),
    ];
    for (index, (id, argv)) in cases.into_iter().enumerate() {
        let submitted = harness.submit(id, argv, "STORY-DIAGNOSTIC-001", &format!("job-{index}"));
        let response = harness.run_and_status(&submitted, Some("STORY-DIAGNOSTIC-001"));
        let completed = result(response);
        assert_eq!(completed["status"], "pass", "{id}: {completed}");
        assert_eq!(completed["result"]["outcome"], "PASS", "{id}");
        assert!(
            legacy::validate_job_terminal_status(id, &completed).expect("terminal status"),
            "{id}"
        );
    }
}

#[test]
fn trusted_work_item_binding_is_not_replaceable_by_payload_or_status_callers() {
    let root = TempDir::new().expect("tempdir");
    let mut harness = Harness::new(&root);
    let first = harness.submit("iteration-check", &[], "STORY-A", "same-key");
    let missing = harness.status(&first, None);
    assert_eq!(code(&missing), StableErrorCode::ProjectMismatch);
    let wrong = harness.status(&first, Some("STORY-B"));
    assert_eq!(code(&wrong), StableErrorCode::ProjectMismatch);

    let other_session = harness.open_session("STORY-A", "diagnostic-other-session");
    let wrong_session = harness.status_as(&first, Some("STORY-A"), &other_session);
    assert_eq!(code(&wrong_session), StableErrorCode::TurnIdentityMismatch);

    let second = harness.submit("iteration-check", &[], "STORY-B", "same-key");
    assert_ne!(first["jobId"], second["jobId"]);
    assert_eq!(second["workItemId"], "STORY-B");
}

#[test]
fn diagnostic_failures_are_terminal_non_success_not_successful_rpc_calls() {
    let root = TempDir::new().expect("tempdir");
    let mut harness = Harness::new(&root);
    let submitted = harness.submit(
        "gate doc-storage",
        &["--path", "../tmp/report.md"],
        "STORY-FAIL",
        "gate-fail",
    );
    let completed = result(harness.run_and_status(&submitted, Some("STORY-FAIL")));
    assert_eq!(completed["status"], "fail");
    assert_eq!(completed["result"]["outcome"], "FAIL");
    assert!(legacy::validate_job_terminal_status("gate doc-storage", &completed).is_err());
}

#[test]
fn payload_cannot_self_report_a_replacement_work_item() {
    let root = TempDir::new().expect("tempdir");
    let mut harness = Harness::new(&root);
    let mut request = params(
        json!({
            "entrypoint":"iteration-check",
            "arguments":{"workItemId":"FORGED"},
            "deadlineUnixMs":NOW_MS + 10_000,
        }),
        10_000,
    );
    request.workspace_id = Some(harness.workspace.workspace_id.clone());
    request.work_item_id = Some("STORY-TRUSTED".to_owned());
    let session = harness.identity_for("STORY-TRUSTED");
    request.agent_id = Some(harness.agent_id.clone());
    request.session_id = Some(session.session_id);
    request.capability_token = Some(session.capability_token);
    request.idempotency_key = Some("forged-payload".to_owned());
    let submitted = result(call(
        &harness.runtime,
        &mut harness.connection,
        RpcMethod::JobSubmit,
        serde_json::to_value(request).expect("submit JSON"),
    ));
    assert_eq!(submitted["workItemId"], "STORY-TRUSTED");
    let completed = result(harness.run_and_status(&submitted, Some("STORY-TRUSTED")));
    assert_eq!(completed["status"], "error");
    assert_eq!(
        completed["errorCode"],
        serde_json::to_value(StableErrorCode::OperationSchemaInvalid).expect("stable code JSON")
    );
}

#[test]
fn diagnostic_admission_requires_a_bound_session_but_generic_jobs_remain_workspace_scoped() {
    let root = TempDir::new().expect("tempdir");
    let mut harness = Harness::new(&root);
    let mut strict = params(
        json!({
            "entrypoint":"iteration-check",
            "arguments":{},
            "deadlineUnixMs":NOW_MS + 10_000,
        }),
        10_000,
    );
    strict.workspace_id = Some(harness.workspace.workspace_id.clone());
    strict.work_item_id = Some("STORY-DIAGNOSTIC-001".to_owned());
    strict.idempotency_key = Some("strict-without-session".to_owned());
    let rejected = call(
        &harness.runtime,
        &mut harness.connection,
        RpcMethod::JobSubmit,
        serde_json::to_value(strict).expect("strict job JSON"),
    );
    assert_eq!(code(&rejected), StableErrorCode::OperationSchemaInvalid);

    let mut generic = params(
        json!({
            "entrypoint":"assets.read",
            "arguments":{},
            "deadlineUnixMs":NOW_MS + 10_000,
        }),
        10_000,
    );
    generic.workspace_id = Some(harness.workspace.workspace_id.clone());
    generic.idempotency_key = Some("generic-workspace-only".to_owned());
    let submitted = result(call(
        &harness.runtime,
        &mut harness.connection,
        RpcMethod::JobSubmit,
        serde_json::to_value(generic).expect("generic job JSON"),
    ));
    assert_eq!(submitted["status"], "queued");
    assert!(submitted["sessionId"].is_null());
    assert!(submitted["agentGrant"].is_null());
}

#[test]
fn update_check_does_not_false_green_an_unported_semantic_checker() {
    let root = TempDir::new().expect("tempdir");
    let mut harness = Harness::new(&root);
    let submitted = harness.submit(
        "update-check",
        &["--only", "UC-02"],
        "STORY-UPDATE",
        "update-unported",
    );
    let completed = result(harness.run_and_status(&submitted, Some("STORY-UPDATE")));
    assert_eq!(completed["status"], "fail");
    assert_eq!(completed["result"]["all_pass"], false);
    assert!(legacy::validate_job_terminal_status("update-check", &completed).is_err());
}

fn prepare_workspace(root: &TempDir) {
    for directory in [
        "source/standards",
        "tools/lib",
        "design",
        "crates/ae-sdd-gates/src",
        "crates/ae-sdd-runtime/src",
        "tests/fixtures/compatibility",
        ".auto-engineering/diagnostic-e2e",
    ] {
        fs::create_dir_all(root.path().join(directory)).expect("fixture directory");
    }
    fs::write(
        root.path().join("source/SKILL.md"),
        "---\nversion: 3.14.0\n---\n",
    )
    .expect("skill");
    fs::write(
        root.path().join("tools/lib/paths.py"),
        "MASTER_VERSION = \"3.14.0\"\n",
    )
    .expect("paths");
    fs::write(root.path().join("README.md"), "# ae-sdd\nversion v3.14.0\n").expect("readme");
    fs::write(
        root.path().join("source/standards/update-graph.json"),
        serde_json::to_vec(&json!({
            "$schema":"ae-sdd-update-graph/v1",
            "description":"fixture",
            "version":"1.0.0",
            "rules":[{
                "id":"UG-01","name":"version","trigger":["source/SKILL.md"],
                "trigger_condition":"version changed",
                "affected":[{"path":"README.md","action":"align","auto_checkable":true}],
                "checks":["UC-01","UC-02"]
            }]
        }))
        .expect("graph JSON"),
    )
    .expect("graph");
    let gates = (0..36)
        .map(|index| format!("Gate {{ id: \"G-{index:02}\" }}\n"))
        .collect::<String>();
    fs::write(
        root.path().join("crates/ae-sdd-gates/src/registry.rs"),
        gates,
    )
    .expect("gates");
    fs::write(
        root.path().join("crates/ae-sdd-runtime/src/service.rs"),
        "RpcMethod::HookUserPrompt RpcMethod::HookPreTool RpcMethod::HookPostTool RpcMethod::HookStop",
    )
    .expect("hook dispatch");
    fs::write(
        root.path()
            .join("crates/ae-sdd-runtime/src/service_hook_context.rs"),
        "pub(super) fn hook() { commit_receipt_event(); }",
    )
    .expect("hook implementation");
    let commands = (0..113)
        .map(|index| json!({"id":format!("command-{index}")}))
        .collect::<Vec<_>>();
    fs::write(
        root.path()
            .join("tests/fixtures/compatibility/cli-routing.v1.json"),
        serde_json::to_vec(&json!({"commands":commands})).expect("routes JSON"),
    )
    .expect("routes");
    fs::write(
        root.path()
            .join(".auto-engineering/diagnostic-e2e/state.json"),
        serde_json::to_vec_pretty(&json!({
            "stateMachineName":"PRD-DIAGNOSTIC-E2E",
            "activeStory":"STORY-DIAGNOSTIC-001",
            "revision":1,
            "lastFencingToken":0,
            "scale":"small",
            "selectedDesign":"Story",
            "phase":"coding",
            "currentPhase":"coding",
            "storyStates":{
                "STORY-DIAGNOSTIC-001":{"phase":"coding","currentPhase":"coding"},
                "STORY-A":{"phase":"coding","currentPhase":"coding"},
                "STORY-B":{"phase":"coding","currentPhase":"coding"},
                "STORY-FAIL":{"phase":"coding","currentPhase":"coding"},
                "STORY-TRUSTED":{"phase":"coding","currentPhase":"coding"},
                "STORY-UPDATE":{"phase":"coding","currentPhase":"coding"}
            },
            "documentPaths":{"STORY":"design/story.md"}
        }))
        .expect("state JSON"),
    )
    .expect("state");
}

fn params(payload: Value, deadline_ms: u64) -> RequestParams<Value> {
    RequestParams {
        protocol_version: PROTOCOL_VERSION_V1.to_owned(),
        workspace_id: None,
        agent_id: None,
        session_id: None,
        capability_token: None,
        turn_id: None,
        work_item_id: None,
        lease_id: None,
        fencing_token: None,
        expected_revision: None,
        idempotency_key: None,
        confirmation: None,
        deadline_ms,
        payload,
    }
}

fn call(
    runtime: &RuntimeService,
    connection: &mut ConnectionState,
    method: RpcMethod,
    params: Value,
) -> Value {
    let request = JsonRpcRequest::new(format!("{}-e2e", method.as_str()), method, params);
    serde_json::from_slice(&runtime.handle_payload(
        connection,
        &serde_json::to_vec(&request).expect("request JSON"),
    ))
    .expect("response JSON")
}

fn result(response: Value) -> Value {
    response
        .get("result")
        .cloned()
        .unwrap_or_else(|| panic!("RPC failed: {response}"))
}

fn code(response: &Value) -> StableErrorCode {
    serde_json::from_value(response["error"]["data"]["stableCode"].clone())
        .unwrap_or_else(|_| panic!("missing stable code: {response}"))
}
