#[allow(dead_code, unused_imports)]
#[path = "../../../../bins/ae-sdd-cli/src/legacy/mod.rs"]
mod legacy;

use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ae_sdd_domain::BootId;
use ae_sdd_integrations::{NativeBusinessAdapter, SqliteRuntimePersistence};
use ae_sdd_protocol::{
    ClientKind, ConfirmationRef, HandshakeRequest, JsonRpcRequest, PROTOCOL_RANGE_V1,
    PROTOCOL_VERSION_V1, RequestParams, RpcMethod, SecretString, WorkspaceMode,
};
use ae_sdd_runtime::{
    BusinessOperationPort, ClockPort, ConnectionState, PersistencePort, ResolvedWorkspace,
    RuntimeConfig, RuntimeResult, RuntimeService, SessionResult, WorkspaceParityEvidence,
    WorkspaceResolverPort, WorkspaceResult,
};
use legacy::{
    LegacyRequestSource, LegacyRpcAdapter, LegacyTarget, adapt_passthrough_request,
    adapt_typed_operation_request, parse_rpc_invocation, resolve_command_id,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use uuid::Uuid;

static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct FixedClock;

impl ClockPort for FixedClock {
    fn now_unix_ms(&self) -> u64 {
        1_000
    }
}

struct TestResolver;

impl WorkspaceResolverPort for TestResolver {
    fn resolve(&self, requested_root: &str) -> RuntimeResult<ResolvedWorkspace> {
        Ok(ResolvedWorkspace {
            canonical_root: fs::canonicalize(requested_root)
                .expect("workspace canonicalizes")
                .to_string_lossy()
                .into_owned(),
            inside_allowed_root: true,
        })
    }
}

pub(super) struct Harness {
    pub(super) runtime: Arc<RuntimeService>,
    pub(super) persistence: Arc<SqliteRuntimePersistence>,
    endpoint_token: String,
    pub(super) workspace_root: TempDir,
    _runtime_root: TempDir,
    database: std::path::PathBuf,
    pub(super) state_path: std::path::PathBuf,
    pub(super) document_path: std::path::PathBuf,
}

impl Harness {
    pub(super) fn new() -> Self {
        let workspace_root = TempDir::new().expect("workspace");
        prepare_workspace(&workspace_root);
        let runtime_root = TempDir::new().expect("runtime");
        let database = runtime_root.path().join("runtime.sqlite3");
        let persistence =
            Arc::new(SqliteRuntimePersistence::open(&database).expect("runtime persistence opens"));
        let event_store_id = persistence.event_store_id().expect("event store identity");
        let boot_id = BootId::from_uuid(Uuid::from_u128(600));
        let config = RuntimeConfig::default();
        let persistence_port: Arc<dyn PersistencePort> = persistence.clone();
        let business: Arc<dyn BusinessOperationPort> = Arc::new(NativeBusinessAdapter::new(
            database.clone(),
            event_store_id,
            boot_id,
            config.policy_digest.clone(),
            Arc::clone(&persistence_port),
        ));
        let endpoint_token = "typed-operation-e2e-token".to_owned();
        let runtime = Arc::new(RuntimeService::new(
            config,
            boot_id,
            endpoint_token.clone(),
            persistence_port,
            Arc::new(FixedClock),
            Arc::new(TestResolver),
            business,
        ));
        runtime.recover().expect("runtime recovers");
        let state_path = workspace_root
            .path()
            .join(".auto-engineering/typed-e2e/state.json");
        let document_path = workspace_root.path().join("docs/story.md");
        Self {
            runtime,
            persistence,
            endpoint_token,
            workspace_root,
            _runtime_root: runtime_root,
            database,
            state_path,
            document_path,
        }
    }

    pub(super) fn connection(&self, kind: ClientKind) -> ConnectionState {
        let mut connection = ConnectionState::default();
        let response = raw_call(
            &self.runtime,
            &mut connection,
            RpcMethod::RuntimeHandshake,
            serde_json::to_value(HandshakeRequest {
                protocol_range: PROTOCOL_RANGE_V1.to_owned(),
                client_build: "typed-operation-e2e".to_owned(),
                client_kind: kind,
                endpoint_token: SecretString::new(self.endpoint_token.clone()),
                expected_boot_id: self.runtime.boot_id().to_string(),
                expected_policy_digest: self.runtime.policy_digest().to_owned(),
            })
            .expect("handshake serializes"),
        );
        assert!(response.get("result").is_some(), "{response}");
        connection
    }

    pub(super) fn business_adapter(&self) -> NativeBusinessAdapter {
        let persistence: Arc<dyn PersistencePort> = self.persistence.clone();
        NativeBusinessAdapter::new(
            self.database.clone(),
            self.persistence
                .event_store_id()
                .expect("event store identity"),
            self.runtime.boot_id(),
            self.runtime.policy_digest().to_owned(),
            persistence,
        )
    }
}

#[derive(Clone)]
pub(super) struct CliIdentity {
    workspace_id: String,
    work_item_id: String,
    session_id: String,
    agent_id: String,
    capability_token: String,
}

impl CliIdentity {
    fn environment(&self) -> impl Fn(&str) -> Option<String> {
        let values = BTreeMap::from([
            ("AE_SDD_WORKSPACE_ID".to_owned(), self.workspace_id.clone()),
            ("AE_SDD_WORK_ITEM_ID".to_owned(), self.work_item_id.clone()),
            ("AE_SDD_SESSION_ID".to_owned(), self.session_id.clone()),
            ("AE_SDD_AGENT_ID".to_owned(), self.agent_id.clone()),
            (
                "AE_SDD_CAPABILITY_TOKEN".to_owned(),
                self.capability_token.clone(),
            ),
        ]);
        move |name| values.get(name).cloned()
    }
}

fn prepare_workspace(root: &TempDir) {
    let state_dir = root.path().join(".auto-engineering/typed-e2e");
    fs::create_dir_all(&state_dir).expect("state dir");
    fs::create_dir_all(root.path().join("docs")).expect("docs dir");
    fs::create_dir_all(root.path().join("draft")).expect("draft dir");
    fs::create_dir_all(root.path().join("evidence")).expect("evidence dir");
    fs::create_dir_all(root.path().join("src")).expect("source dir");
    fs::write(root.path().join("docs/story.md"), "# original story\n").expect("document");
    fs::write(root.path().join("draft/story.md"), "# updated story\n").expect("draft");
    fs::write(root.path().join("evidence/result.json"), "{\"ok\":true}\n").expect("evidence");
    fs::write(
        root.path().join("src/lib.rs"),
        "pub fn ready() -> bool { true }\n",
    )
    .expect("Rust source");
    fs::write(root.path().join("Cargo.toml"), "[workspace]\n").expect("Cargo.toml");
    let constraints = root.path().join("constraints");
    fs::create_dir_all(&constraints).expect("constraints");
    for index in 0..5 {
        fs::write(
            constraints.join(format!("constraint-{index}.md")),
            "# constraint\n",
        )
        .expect("constraint");
    }
    fs::write(
        state_dir.join("state.json"),
        serde_json::to_vec_pretty(&json!({
            "stateMachineName":"PRD-TYPED-E2E",
            "activeStory":"STORY-TYPED-E2E",
            "revision":1,
            "lastFencingToken":0,
            "scale":"small",
            "selectedDesign":"Story",
            "phase":"coding",
            "currentPhase":"coding",
            "storyStates":{
                "STORY-TYPED-E2E":{"phase":"coding","currentPhase":"coding"}
            },
            "documentPaths":{"STORY":"docs/story.md"}
        }))
        .expect("state serializes"),
    )
    .expect("state");
}

pub(super) fn register_and_cut_over(
    harness: &Harness,
    cli: &mut ConnectionState,
) -> WorkspaceResult {
    let mut register = params(json!({
        "projectRoot":harness.workspace_root.path().to_string_lossy(),
        "projectKey":"typed-e2e",
    }));
    register.idempotency_key = Some("workspace-register-e2e".to_owned());
    let workspace: WorkspaceResult = serde_json::from_value(success(&call(
        &harness.runtime,
        cli,
        RpcMethod::WorkspaceRegister,
        register,
    )))
    .expect("workspace result");
    let mut admin = harness.connection(ClientKind::Admin);
    let mut drain = params(json!({"stop":false}));
    drain.idempotency_key = Some("runtime-drain-e2e".to_owned());
    drain.confirmation = Some(confirmation());
    assert_success(&call(
        &harness.runtime,
        &mut admin,
        RpcMethod::RuntimeDrain,
        drain,
    ));
    let parity = WorkspaceParityEvidence {
        comparison_count: 12,
        mismatch_count: 0,
        source_revision: 1,
        legacy_digest: "a".repeat(64),
        rust_digest: "a".repeat(64),
        observed_at_unix_ms: 1_000,
    };
    let parity_digest = hex::encode(Sha256::digest(
        serde_json::to_vec(&parity).expect("parity serializes"),
    ));
    let mut transition = params(json!({
        "targetMode":WorkspaceMode::RustCanary,
        "reason":"typed operation E2E parity fixture",
        "parityDigest":parity_digest,
        "parity":parity,
    }));
    transition.workspace_id = Some(workspace.workspace_id.clone());
    transition.idempotency_key = Some("workspace-canary-e2e".to_owned());
    transition.confirmation = Some(confirmation());
    serde_json::from_value(success(&call(
        &harness.runtime,
        &mut admin,
        RpcMethod::WorkspaceModeTransition,
        transition,
    )))
    .expect("canary workspace")
}

pub(super) fn open_root(
    harness: &Harness,
    cli: &mut ConnectionState,
    workspace: &WorkspaceResult,
    external_key: &str,
    agent_id: &str,
) -> SessionResult {
    let mut open = params(json!({
        "externalKey":external_key,
        "role":"root",
        "engaged":true,
    }));
    open.workspace_id = Some(workspace.workspace_id.clone());
    open.work_item_id = Some("STORY-TYPED-E2E".to_owned());
    open.agent_id = Some(agent_id.to_owned());
    open.idempotency_key = Some(format!("session-open-{external_key}"));
    serde_json::from_value(success(&call(
        &harness.runtime,
        cli,
        RpcMethod::SessionOpen,
        open,
    )))
    .expect("root session")
}

pub(super) fn identity(
    workspace: &WorkspaceResult,
    session: &SessionResult,
    agent_id: &str,
) -> CliIdentity {
    CliIdentity {
        workspace_id: workspace.workspace_id.clone(),
        work_item_id: "STORY-TYPED-E2E".to_owned(),
        session_id: session.session_id.clone(),
        agent_id: agent_id.to_owned(),
        capability_token: session.capability_token.clone(),
    }
}

pub(super) fn invoke(
    harness: &Harness,
    connection: &mut ConnectionState,
    identity: &CliIdentity,
    command: &str,
    arguments: Vec<String>,
) -> Value {
    let params = route_params(identity, command, arguments)
        .unwrap_or_else(|error| panic!("{command}: {error}"));
    call(
        &harness.runtime,
        connection,
        RpcMethod::OperationExecute,
        params,
    )
}

pub(super) fn route_params(
    identity: &CliIdentity,
    command: &str,
    arguments: Vec<String>,
) -> Result<RequestParams<Value>, String> {
    let route = resolve_command_id(command).expect("known route");
    let invocation = parse_rpc_invocation(
        &route,
        RpcMethod::OperationExecute,
        &arguments,
        identity.environment(),
    )
    .map_err(|error| error.to_string())?;
    let LegacyRequestSource::Synthesized(mut params) = invocation.request else {
        panic!("E2E uses synthesized argv")
    };
    let LegacyTarget::Rpc {
        adapter: LegacyRpcAdapter::TypedOperation { operation },
        ..
    } = &route.target
    else {
        panic!("typed route")
    };
    adapt_typed_operation_request(operation, &mut params).map_err(|error| error.to_string())?;
    Ok(*params)
}

pub(super) fn ops_execute_params(
    identity: &CliIdentity,
    request_file: &std::path::Path,
) -> Result<RequestParams<Value>, String> {
    let route = resolve_command_id("ops execute").expect("known route");
    let invocation = parse_rpc_invocation(
        &route,
        RpcMethod::OperationExecute,
        &[
            "--request-file".to_owned(),
            request_file.to_string_lossy().into_owned(),
        ],
        identity.environment(),
    )
    .map_err(|error| error.to_string())?;
    let LegacyRequestSource::Synthesized(mut params) = invocation.request else {
        panic!("E2E uses synthesized argv")
    };
    let LegacyTarget::Rpc {
        adapter: LegacyRpcAdapter::Passthrough,
        ..
    } = &route.target
    else {
        panic!("ops execute passthrough route")
    };
    adapt_passthrough_request("ops execute", RpcMethod::OperationExecute, &mut params)
        .map_err(|error| error.to_string())?;
    Ok(*params)
}

pub(super) fn trusted_params(identity: &CliIdentity, payload: Value) -> RequestParams<Value> {
    let mut request = params(payload);
    request.workspace_id = Some(identity.workspace_id.clone());
    request.work_item_id = Some(identity.work_item_id.clone());
    request.session_id = Some(identity.session_id.clone());
    request.agent_id = Some(identity.agent_id.clone());
    request.capability_token = Some(identity.capability_token.clone());
    request
}

pub(super) fn operation_params(
    identity: &CliIdentity,
    operation: &str,
    payload: Value,
) -> RequestParams<Value> {
    trusted_params(
        identity,
        json!({
            "operation":operation,
            "payload":payload,
        }),
    )
}

pub(super) fn confirmation_ref(
    confirmation_id: &str,
    approved_by: &str,
    approved_at: &str,
) -> ConfirmationRef {
    ConfirmationRef {
        confirmation_id: confirmation_id.to_owned(),
        approved_by: approved_by.to_owned(),
        approved_at: approved_at.to_owned(),
    }
}

pub(super) fn write_args(
    lease_id: &str,
    fencing: u64,
    revision: u64,
    key: &str,
    business: &[&str],
) -> Vec<String> {
    let mut values = args(&[
        "--lease-id",
        lease_id,
        "--fencing-token",
        &fencing.to_string(),
        "--expected-revision",
        &revision.to_string(),
        "--idempotency-key",
        key,
    ]);
    values.extend(business.iter().map(|value| (*value).to_owned()));
    values
}

pub(super) fn lease_args(lease_id: &str, fencing: u64, key: &str, renew: bool) -> Vec<String> {
    let mut values = args(&[
        "--owner",
        "{\"role\":\"root\"}",
        "--lease-id",
        lease_id,
        "--fencing-token",
        &fencing.to_string(),
        "--idempotency-key",
        key,
    ]);
    if renew {
        values.extend(args(&["--ttl-seconds", "600"]));
    }
    values
}

pub(super) fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn params(payload: Value) -> RequestParams<Value> {
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
        deadline_ms: 10_000,
        payload,
    }
}

fn confirmation() -> ConfirmationRef {
    confirmation_ref("confirmation-e2e", "user:test", "2026-07-23T00:00:00Z")
}

pub(super) fn call(
    runtime: &RuntimeService,
    connection: &mut ConnectionState,
    method: RpcMethod,
    params: RequestParams<Value>,
) -> Value {
    raw_call(
        runtime,
        connection,
        method,
        serde_json::to_value(params).expect("params serialize"),
    )
}

pub(super) fn raw_call(
    runtime: &RuntimeService,
    connection: &mut ConnectionState,
    method: RpcMethod,
    params: Value,
) -> Value {
    let id = REQUEST_SEQUENCE.fetch_add(1, Ordering::AcqRel).to_string();
    let request = JsonRpcRequest::new(id, method, params);
    serde_json::from_slice(
        &runtime.handle_payload(connection, &serde_json::to_vec(&request).expect("request")),
    )
    .expect("response")
}

pub(super) fn success(response: &Value) -> Value {
    response
        .get("result")
        .cloned()
        .unwrap_or_else(|| panic!("{response}"))
}

pub(super) fn assert_success(response: &Value) {
    assert!(response.get("result").is_some(), "{response}");
}

pub(super) fn stable_error(response: &Value) -> &str {
    response["error"]["data"]["stableCode"]
        .as_str()
        .unwrap_or_else(|| panic!("{response}"))
}

pub(super) fn journal_snapshot(harness: &Harness) -> BTreeMap<String, Vec<u8>> {
    let directory = harness
        .workspace_root
        .path()
        .join(".auto-engineering/typed-e2e/mutation-journal/v1");
    if !directory.is_dir() {
        return BTreeMap::new();
    }
    fs::read_dir(directory)
        .expect("journal directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("json"))
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path()).expect("journal"),
            )
        })
        .collect()
}
