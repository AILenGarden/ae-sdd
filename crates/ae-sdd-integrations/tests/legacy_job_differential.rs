use std::sync::Arc;

use ae_sdd_domain::{BootId, EventStoreId};
use ae_sdd_integrations::{FileWorkspaceResolver, NativeBusinessAdapter};
use ae_sdd_protocol::{
    ClientKind, HandshakeRequest, JsonRpcRequest, PROTOCOL_RANGE_V1, PROTOCOL_VERSION_V1,
    RequestParams, RpcMethod, SecretString, WorkspaceMode,
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
const ENDPOINT_TOKEN: &str = "legacy-job-differential-token";

struct FixedClock;

impl ClockPort for FixedClock {
    fn now_unix_ms(&self) -> u64 {
        NOW_MS
    }
}

struct RustHarness {
    runtime: Arc<RuntimeService>,
    connection: ConnectionState,
    workspace: WorkspaceResult,
}

impl RustHarness {
    fn new(root: &TempDir) -> Self {
        prepare_workspace(root);
        let event_store_id = EventStoreId::from_uuid(Uuid::from_u128(601));
        let persistence = Arc::new(MemoryPersistence::new(event_store_id));
        let persistence_port: Arc<dyn PersistencePort> = persistence.clone();
        let business: Arc<dyn BusinessOperationPort> = Arc::new(NativeBusinessAdapter::new(
            root.path().join("runtime.sqlite3"),
            event_store_id,
            BootId::from_uuid(Uuid::from_u128(602)),
            ae_sdd_policy::policy_digest().to_hex(),
            Arc::clone(&persistence_port),
        ));
        let resolver = Arc::new(
            FileWorkspaceResolver::new([root.path().to_path_buf()]).expect("workspace resolver"),
        );
        let runtime = Arc::new(RuntimeService::new(
            RuntimeConfig::default(),
            BootId::from_uuid(Uuid::from_u128(603)),
            ENDPOINT_TOKEN,
            persistence_port,
            Arc::new(FixedClock),
            resolver,
            business,
        ));
        let mut connection = ConnectionState::default();
        let handshake = HandshakeRequest {
            protocol_range: PROTOCOL_RANGE_V1.to_owned(),
            client_build: "legacy-job-differential".to_owned(),
            client_kind: ClientKind::Cli,
            endpoint_token: SecretString::new(ENDPOINT_TOKEN.to_owned()),
            expected_boot_id: runtime.boot_id().to_string(),
            expected_policy_digest: runtime.policy_digest().to_owned(),
        };
        result(raw_call(
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
        register.idempotency_key = Some("workspace-register-differential".to_owned());
        let workspace = serde_json::from_value(result(raw_call(
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

    fn run(&mut self, id: &str, business_argv: &[&str], sequence: usize) -> Value {
        let mut argv = id.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        argv.extend(business_argv.iter().map(|value| (*value).to_owned()));
        argv.extend([
            "--workspace-id".to_owned(),
            self.workspace.workspace_id.clone(),
            "--idempotency-key".to_owned(),
            format!("legacy-job-differential-{sequence}"),
        ]);
        let resolved = legacy::resolve_legacy_argv(&argv).expect("legacy route");
        let entrypoint = match &resolved.route.target {
            legacy::LegacyTarget::Rpc {
                method: RpcMethod::JobSubmit,
                adapter: legacy::LegacyRpcAdapter::JobSubmission { entrypoint, .. },
            } => entrypoint.clone(),
            target => panic!("not a job route: {target:?}"),
        };
        let invocation = legacy::parse_rpc_invocation(
            &resolved.route,
            RpcMethod::JobSubmit,
            &resolved.trailing_arguments,
            |_| None,
        )
        .expect("legacy argv adapter");
        let mut request = match invocation.request {
            legacy::LegacyRequestSource::Synthesized(request) => *request,
            legacy::LegacyRequestSource::ExplicitJson(_) => panic!("synthesized request expected"),
        };
        legacy::adapt_job_submission(&resolved.route, &entrypoint, &mut request, NOW_MS)
            .expect("job adapter");
        let submitted = result(raw_call(
            &self.runtime,
            &mut self.connection,
            RpcMethod::JobSubmit,
            serde_json::to_value(request).expect("job params JSON"),
        ));
        assert!(self.runtime.run_one_pending_job().expect("run queued job"));
        let mut status = params(json!({"jobId":submitted["jobId"]}), 1_000);
        status.workspace_id = Some(self.workspace.workspace_id.clone());
        let completed = result(raw_call(
            &self.runtime,
            &mut self.connection,
            RpcMethod::JobStatus,
            serde_json::to_value(status).expect("status params JSON"),
        ));
        assert_eq!(completed["status"], "pass", "{id}: {completed}");
        completed["result"].clone()
    }
}

// The differential oracle that ran each of the 25 read-only jobs through both
// `python tools/bin/ae-sdd` and the native adapter was removed with the Python
// tree it depended on. `legacy_job_cli_e2e.rs` still submits all 25 natively,
// but only asserts that each reaches a passing outcome; the field-level payload
// assertions the oracle carried now exist for 8 of them and are absent for the
// other 17.

#[test]
fn verified_breaking_fixes_reject_ambient_database_and_plugin_authority() {
    for (id, args) in [
        ("db audit", &["--project", "../outside"][..]),
        (
            "db explain",
            &[
                "--project",
                "../outside",
                "--profile",
                "local",
                "--sql",
                "SELECT 1",
            ][..],
        ),
        ("db profiles", &["--project", "../outside"][..]),
        (
            "db query",
            &[
                "--project",
                "../outside",
                "--profile",
                "local",
                "--sql",
                "SELECT 1",
            ][..],
        ),
        ("db profiles", &["--init"][..]),
        (
            "db query",
            &[
                "--profile",
                "local",
                "--sql",
                "CREATE TABLE escaped(id INTEGER)",
                "--write",
            ][..],
        ),
    ] {
        let error = rust_adapter_error(id, args);
        assert!(
            error.contains("unknown legacy job field") || error.contains("mutating"),
            "{id} did not fail closed: {error}"
        );
    }

    // Dropping ambient global plugin authority was a deliberate breaking fix.
    // The fixture registers a project plugin and a global one; only the project
    // plugin may be seen, and the global layer must be reported as absent rather
    // than silently consulted. This used to be stated as a Python-versus-Rust
    // difference, but the native side is the whole contract now.
    let rust_root = TempDir::new().expect("Rust plugin fixture root");
    let mut rust = RustHarness::new(&rust_root);

    let rust_list = rust.run("plugin list", &[], 101);
    assert_eq!(rust_list["totalPlugins"], 1);
    assert_eq!(rust_list.pointer("/layers/1/exists"), Some(&json!(false)));

    let rust_trace = rust.run("plugin trace", &["global-skill"], 102);
    assert_eq!(rust_trace["hit"], false);
    assert!(rust_trace["plugin"].is_null());

    let rust_validate = rust.run("plugin validate", &[], 103);
    assert_eq!(rust_validate["valid"], true);
    assert_eq!(rust_validate["totalPlugins"], 1);
}

fn rust_adapter_error(id: &str, business_args: &[&str]) -> String {
    let mut argv = id.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
    argv.extend(business_args.iter().map(|value| (*value).to_owned()));
    argv.extend([
        "--workspace-id".to_owned(),
        "workspace-fixture".to_owned(),
        "--idempotency-key".to_owned(),
        format!("negative-{}", id.replace(' ', "-")),
    ]);
    let resolved = legacy::resolve_legacy_argv(&argv).expect("negative legacy route");
    let entrypoint = match &resolved.route.target {
        legacy::LegacyTarget::Rpc {
            method: RpcMethod::JobSubmit,
            adapter: legacy::LegacyRpcAdapter::JobSubmission { entrypoint, .. },
        } => entrypoint.clone(),
        target => panic!("not a job route: {target:?}"),
    };
    let invocation = match legacy::parse_rpc_invocation(
        &resolved.route,
        RpcMethod::JobSubmit,
        &resolved.trailing_arguments,
        |_| None,
    ) {
        Ok(invocation) => invocation,
        Err(error) => return error.to_string(),
    };
    let mut request = match invocation.request {
        legacy::LegacyRequestSource::Synthesized(request) => *request,
        legacy::LegacyRequestSource::ExplicitJson(_) => panic!("synthesized request expected"),
    };
    legacy::adapt_job_submission(&resolved.route, &entrypoint, &mut request, NOW_MS)
        .expect_err("breaking-fix arguments must be rejected")
        .to_string()
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
    let request = JsonRpcRequest::new(format!("{}-differential", method.as_str()), method, params);
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
