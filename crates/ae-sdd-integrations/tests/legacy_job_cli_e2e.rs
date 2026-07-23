use std::sync::Arc;

use ae_sdd_domain::{BootId, EventStoreId};
use ae_sdd_integrations::{FileWorkspaceResolver, NativeBusinessAdapter};
use ae_sdd_protocol::{
    ClientKind, HandshakeRequest, JsonRpcRequest, PROTOCOL_RANGE_V1, PROTOCOL_VERSION_V1,
    RequestParams, RpcMethod, SecretString, StableErrorCode, WorkspaceMode,
};
use ae_sdd_runtime::{
    BusinessOperationPort, ClockPort, ConnectionState, MemoryPersistence, PersistencePort,
    RuntimeConfig, RuntimeService, WorkspaceResult,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

#[allow(dead_code, unused_imports)]
#[path = "../../../bins/ae-sdd-cli/src/legacy/mod.rs"]
mod legacy;
#[path = "support/legacy_job_fixture.rs"]
mod legacy_job_fixture;

use legacy_job_fixture::prepare_workspace;

const NOW_MS: u64 = 1_000;
const ENDPOINT_TOKEN: &str = "legacy-job-e2e-token";

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
}

impl Harness {
    fn new(root: &TempDir) -> Self {
        prepare_workspace(root);
        let event_store_id = EventStoreId::from_uuid(Uuid::from_u128(501));
        let persistence = Arc::new(MemoryPersistence::new(event_store_id));
        let persistence_port: Arc<dyn PersistencePort> = persistence.clone();
        let business: Arc<dyn BusinessOperationPort> = Arc::new(NativeBusinessAdapter::new(
            root.path().join("runtime.sqlite3"),
            event_store_id,
            BootId::from_uuid(Uuid::from_u128(502)),
            ae_sdd_policy::policy_digest().to_hex(),
            Arc::clone(&persistence_port),
        ));
        let resolver = Arc::new(
            FileWorkspaceResolver::new([root.path().to_path_buf()]).expect("workspace resolver"),
        );
        let runtime = Arc::new(RuntimeService::new(
            RuntimeConfig::default(),
            BootId::from_uuid(Uuid::from_u128(503)),
            ENDPOINT_TOKEN,
            persistence_port,
            Arc::new(FixedClock),
            resolver,
            business,
        ));
        let mut connection = ConnectionState::default();
        let handshake = HandshakeRequest {
            protocol_range: PROTOCOL_RANGE_V1.to_owned(),
            client_build: "legacy-job-e2e".to_owned(),
            client_kind: ClientKind::Cli,
            endpoint_token: SecretString::new(ENDPOINT_TOKEN.to_owned()),
            expected_boot_id: runtime.boot_id().to_string(),
            expected_policy_digest: runtime.policy_digest().to_owned(),
        };
        assert_result(raw_call(
            &runtime,
            &mut connection,
            RpcMethod::RuntimeHandshake,
            serde_json::to_value(handshake).expect("handshake JSON"),
        ));
        let mut register = params(
            json!({
                "projectRoot":root.path(),
                "projectKey":"legacy-e2e",
                "mode":WorkspaceMode::Shadow,
            }),
            1_000,
        );
        register.idempotency_key = Some("workspace-register-legacy-e2e".to_owned());
        let workspace = serde_json::from_value(assert_result(raw_call(
            &runtime,
            &mut connection,
            RpcMethod::WorkspaceRegister,
            serde_json::to_value(register).expect("register params JSON"),
        )))
        .expect("workspace result");
        Self {
            runtime,
            connection,
            workspace,
        }
    }

    fn submit_legacy(&mut self, id: &str, business_argv: &[&str], sequence: usize) -> Value {
        let mut argv = id.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        argv.extend(business_argv.iter().map(|value| (*value).to_owned()));
        argv.extend([
            "--workspace-id".to_owned(),
            self.workspace.workspace_id.clone(),
            "--idempotency-key".to_owned(),
            format!("legacy-job-{sequence}"),
        ]);
        let resolved = legacy::resolve_legacy_argv(&argv).expect("legacy route");
        let (method, entrypoint) = job_target(&resolved.route);
        let invocation = legacy::parse_rpc_invocation(
            &resolved.route,
            method,
            &resolved.trailing_arguments,
            |_| None,
        )
        .expect("legacy argv adapter");
        let mut request = match invocation.request {
            legacy::LegacyRequestSource::Synthesized(request) => *request,
            legacy::LegacyRequestSource::ExplicitJson(_) => panic!("test uses synthesized argv"),
        };
        legacy::adapt_job_submission(&resolved.route, &entrypoint, &mut request, NOW_MS)
            .expect("job adapter");
        assert_eq!(request.deadline_ms, 30_000);
        assert_eq!(request.payload["entrypoint"], entrypoint);
        assert!(request.payload["arguments"].is_object());
        assert_eq!(request.payload["deadlineUnixMs"], NOW_MS + 300_000);
        assert_eq!(request.payload.as_object().expect("job payload").len(), 3);

        let submitted = assert_result(raw_call(
            &self.runtime,
            &mut self.connection,
            RpcMethod::JobSubmit,
            serde_json::to_value(request).expect("job params JSON"),
        ));
        assert_eq!(submitted["status"], "queued");
        assert!(self.runtime.run_one_pending_job().expect("run queued job"));
        let mut status = params(json!({"jobId":submitted["jobId"]}), 1_000);
        status.workspace_id = Some(self.workspace.workspace_id.clone());
        let completed = assert_result(raw_call(
            &self.runtime,
            &mut self.connection,
            RpcMethod::JobStatus,
            serde_json::to_value(status).expect("status params JSON"),
        ));
        assert_eq!(completed["status"], "pass", "{id}: {completed}");
        assert_eq!(completed["result"]["outcome"], "PASS", "{id}");
        completed
    }
}

#[test]
fn all_twenty_five_read_only_legacy_jobs_reach_native_daemon_execution() {
    let root = TempDir::new().expect("tempdir");
    let mut harness = Harness::new(&root);
    let cases: Vec<(&str, Vec<&str>)> = vec![
        ("assets check", vec![]),
        ("assets outline", vec![]),
        ("assets query", vec!["service", "--top", "5"]),
        ("assets read", vec!["coding", "--keys", "service"]),
        ("assets section", vec!["A"]),
        ("assets stats", vec![]),
        ("automation status", vec![]),
        (
            "baseline diff",
            vec![
                "--report",
                r#"{"findings":[{"findingKey":"finding-1","ruleId":"R1","path":"tracked.txt","severity":"WARNING"}]}"#,
            ],
        ),
        ("baseline inspect", vec![]),
        ("classify", vec!["--text", "large architecture migration"]),
        ("db audit", vec![]),
        (
            "db explain",
            vec!["--profile", "local", "--sql", "SELECT id,name FROM item"],
        ),
        ("db profiles", vec![]),
        (
            "db query",
            vec!["--profile", "local", "--sql", "SELECT id,name FROM item"],
        ),
        (
            "evidence lookup",
            vec![
                "--story",
                "STORY-EVIDENCE-001",
                "--command",
                "cargo test",
                "--input-fingerprint",
                "input-1",
                "--toolchain-fingerprint",
                "toolchain-1",
            ],
        ),
        ("git blame", vec!["--file", "tracked.txt"]),
        ("git diff", vec!["--stat"]),
        (
            "git impact",
            vec!["--file", "tracked.txt", "--file", "src/lib.rs"],
        ),
        ("git log", vec!["--limit", "5"]),
        ("git status", vec![]),
        ("perf doctor", vec!["--last", "10", "--limit", "5"]),
        ("perf report", vec!["--last", "10", "--limit", "5"]),
        ("plugin list", vec![]),
        ("plugin trace", vec!["fixture-skill"]),
        ("plugin validate", vec![]),
    ];
    assert_eq!(cases.len(), 25);
    let unique = cases
        .iter()
        .map(|(id, _)| *id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), 25);
    for (sequence, (id, argv)) in cases.iter().enumerate() {
        let completed = harness.submit_legacy(id, argv, sequence + 1);
        if *id == "git impact" {
            assert_eq!(completed["result"]["fileCount"], 2);
        }
    }
}

#[test]
fn job_adapter_and_daemon_fail_closed_on_schema_bounds_and_identity() {
    let root = TempDir::new().expect("tempdir");
    let mut harness = Harness::new(&root);
    let route = legacy::resolve_command_id("assets query").expect("assets query route");

    let missing_workspace = parse_route(&route, &["service", "--idempotency-key", "missing-ws"])
        .expect_err("workspace identity is required");
    assert!(missing_workspace.to_string().contains("workspace identity"));
    let missing_idempotency = parse_route(
        &route,
        &["service", "--workspace-id", &harness.workspace.workspace_id],
    )
    .expect_err("idempotency identity is required");
    assert!(missing_idempotency.to_string().contains("idempotency key"));

    for invalid in [
        vec!["service", "--mystery", "value"],
        vec!["service", "--top", "101"],
        vec!["service", "--query", "duplicate"],
    ] {
        let mut request = parsed_job_request(&route, &harness.workspace.workspace_id, &invalid);
        legacy::adapt_job_submission(&route, "assets.query", &mut request, NOW_MS)
            .expect_err("invalid command schema must fail before IPC");
    }

    for (command_id, business) in [
        ("db profiles", vec!["--init"]),
        (
            "db query",
            vec!["--profile", "local", "--sql", "SELECT 1", "--write"],
        ),
    ] {
        let mutation_route = legacy::resolve_command_id(command_id).expect("mutation route");
        let (_, entrypoint) = job_target(&mutation_route);
        let mut request =
            parsed_job_request(&mutation_route, &harness.workspace.workspace_id, &business);
        let error =
            legacy::adapt_job_submission(&mutation_route, &entrypoint, &mut request, NOW_MS)
                .expect_err("mutating legacy option must fail before IPC");
        assert!(error.to_string().contains("mutating"));
    }

    let oversized = "x".repeat(65_537);
    let error = parse_route(
        &route,
        &[
            oversized.as_str(),
            "--workspace-id",
            &harness.workspace.workspace_id,
            "--idempotency-key",
            "oversized",
        ],
    )
    .expect_err("oversized argv value must fail");
    assert!(error.to_string().contains("value budget"));

    let mut unknown_wrapper = params(
        json!({
            "entrypoint":"assets.query",
            "arguments":{"query":"service"},
            "deadlineUnixMs":NOW_MS + 1_000,
            "unknown":true,
        }),
        1_000,
    );
    unknown_wrapper.workspace_id = Some(harness.workspace.workspace_id.clone());
    unknown_wrapper.idempotency_key = Some("unknown-wrapper".to_owned());
    let response = raw_call(
        &harness.runtime,
        &mut harness.connection,
        RpcMethod::JobSubmit,
        serde_json::to_value(unknown_wrapper).expect("unknown wrapper JSON"),
    );
    assert_eq!(
        stable_code(&response),
        StableErrorCode::OperationSchemaInvalid
    );

    let mut forged = parsed_job_request(&route, "forged-workspace", &["service"]);
    legacy::adapt_job_submission(&route, "assets.query", &mut forged, NOW_MS)
        .expect("valid job adapter");
    let response = raw_call(
        &harness.runtime,
        &mut harness.connection,
        RpcMethod::JobSubmit,
        serde_json::to_value(forged).expect("forged params JSON"),
    );
    assert_eq!(stable_code(&response), StableErrorCode::ProjectMismatch);
}

fn parse_route(
    route: &legacy::LegacyCommandRoute,
    trailing: &[&str],
) -> Result<legacy::LegacyRpcInvocation, legacy::LegacyArgumentError> {
    legacy::parse_rpc_invocation(
        route,
        RpcMethod::JobSubmit,
        &trailing
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>(),
        |_| None,
    )
}

fn parsed_job_request(
    route: &legacy::LegacyCommandRoute,
    workspace_id: &str,
    business: &[&str],
) -> RequestParams<Value> {
    let mut trailing = business
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    trailing.extend([
        "--workspace-id".to_owned(),
        workspace_id.to_owned(),
        "--idempotency-key".to_owned(),
        format!("negative-{}", trailing.len()),
    ]);
    match legacy::parse_rpc_invocation(route, RpcMethod::JobSubmit, &trailing, |_| None)
        .expect("generic legacy job params")
        .request
    {
        legacy::LegacyRequestSource::Synthesized(request) => *request,
        legacy::LegacyRequestSource::ExplicitJson(_) => panic!("synthesized request expected"),
    }
}

fn job_target(route: &legacy::LegacyCommandRoute) -> (RpcMethod, String) {
    match &route.target {
        legacy::LegacyTarget::Rpc {
            method,
            adapter: legacy::LegacyRpcAdapter::JobSubmission { entrypoint, .. },
        } => (*method, entrypoint.clone()),
        target => panic!("not a job route: {target:?}"),
    }
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

fn raw_call(
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

fn assert_result(response: Value) -> Value {
    response
        .get("result")
        .cloned()
        .unwrap_or_else(|| panic!("RPC failed: {response}"))
}

fn stable_code(response: &Value) -> StableErrorCode {
    serde_json::from_value(response["error"]["data"]["stableCode"].clone())
        .unwrap_or_else(|_| panic!("missing stable code: {response}"))
}
