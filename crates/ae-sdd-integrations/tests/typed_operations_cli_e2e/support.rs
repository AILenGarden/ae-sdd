#![allow(dead_code)]

#[allow(dead_code, unused_imports)]
#[path = "../../../../bins/ae-sdd-cli/src/legacy/mod.rs"]
mod legacy;

// The authoritative Review input fingerprint spans locked state plus the
// workspace source inventory, and the daemon implementation is reused so the
// evidence manifest this fixture seals cannot drift from production hashing.
// The `review_authority` module and its two siblings resolve `crate::` paths,
// so every test crate that includes this file declares them at its own root.
use crate::review_authority::authoritative_review_workspace_input_fingerprint;

use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ae_sdd_contracts::review::{ReviewAttemptV2, ReviewSessionV2};
use ae_sdd_domain::{BootId, InputFingerprint};
use ae_sdd_integrations::{NativeBusinessAdapter, SqliteRuntimePersistence, SystemClock};
use ae_sdd_protocol::{
    ClientKind, ConfirmationRef, HandshakeRequest, JsonRpcRequest, PROTOCOL_RANGE_V1,
    PROTOCOL_VERSION_V1, RequestParams, RpcMethod, SecretString, WorkspaceMode,
};
use ae_sdd_review::ReviewSupervisor;
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
    now_unix_ms: u64,
    pub(super) state_path: std::path::PathBuf,
    pub(super) document_path: std::path::PathBuf,
}

impl Harness {
    pub(super) fn new() -> Self {
        Self::with_clock(Arc::new(FixedClock), 1_000)
    }

    pub(super) fn new_realtime() -> Self {
        let clock = SystemClock;
        let now_unix_ms = clock.now_unix_ms();
        Self::with_clock(Arc::new(clock), now_unix_ms)
    }

    fn with_clock(clock: Arc<dyn ClockPort>, now_unix_ms: u64) -> Self {
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
            clock,
            Arc::new(TestResolver),
            business,
        ));
        runtime.recover().expect("runtime recovers");
        let state_path = workspace_root
            .path()
            .join(".auto-engineering/typed-e2e/state.json");
        let document_path = workspace_root.path().join("ae-sdd-doc/Story/story.md");
        Self {
            runtime,
            persistence,
            endpoint_token,
            workspace_root,
            _runtime_root: runtime_root,
            database,
            now_unix_ms,
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
                adapter_id: None,
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

    pub(super) fn host_credential(&self) -> String {
        self.endpoint_token.clone()
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
    fs::create_dir_all(root.path().join("ae-sdd-doc/Story")).expect("story doc dir");
    fs::create_dir_all(root.path().join("draft")).expect("draft dir");
    fs::create_dir_all(root.path().join("evidence")).expect("evidence dir");
    fs::create_dir_all(root.path().join("src")).expect("source dir");
    fs::write(
        root.path().join("ae-sdd-doc/Story/story.md"),
        "# original story\n",
    )
    .expect("document");
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
                "STORY-TYPED-E2E":{
                    "phase":"coding",
                    "currentPhase":"coding",
                    "currentStep":"coding",
                    "completedSteps":[],
                    "pendingOutputs":[],
                    "codingRound":1
                }
            },
            "documentPaths":{"STORY":"ae-sdd-doc/Story/story.md"}
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
        observed_at_unix_ms: harness.now_unix_ms,
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
    open_root_for_work_item(
        harness,
        cli,
        workspace,
        "STORY-TYPED-E2E",
        external_key,
        agent_id,
    )
}

pub(super) fn open_root_for_work_item(
    harness: &Harness,
    cli: &mut ConnectionState,
    workspace: &WorkspaceResult,
    work_item_id: &str,
    external_key: &str,
    agent_id: &str,
) -> SessionResult {
    let mut open = params(json!({
        "externalKey":external_key,
        "role":"root",
        "engaged":true,
    }));
    open.workspace_id = Some(workspace.workspace_id.clone());
    open.work_item_id = Some(work_item_id.to_owned());
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
    identity_for_work_item(workspace, session, "STORY-TYPED-E2E", agent_id)
}

pub(super) fn identity_for_work_item(
    workspace: &WorkspaceResult,
    session: &SessionResult,
    work_item_id: &str,
    agent_id: &str,
) -> CliIdentity {
    CliIdentity {
        workspace_id: workspace.workspace_id.clone(),
        work_item_id: work_item_id.to_owned(),
        session_id: session.session_id.clone(),
        agent_id: agent_id.to_owned(),
        capability_token: session.capability_token.clone(),
    }
}

#[allow(dead_code)]
pub(super) fn open_review_lineage(
    harness: &Harness,
    cli: &mut ConnectionState,
    workspace: &WorkspaceResult,
    root_identity: &CliIdentity,
    specialty: &str,
    key: &str,
) -> (SessionResult, SessionResult) {
    let (author, mut reviewers) = open_review_lineage_for_specialties(
        harness,
        cli,
        workspace,
        root_identity,
        &[specialty],
        key,
    );
    (author, reviewers.remove(0))
}

/// Opens one Root -> Series -> {author, reviewer per specialty} lineage.
///
/// A review session binds the author and Series of its first contribution, so
/// every specialty of one review must hang off the same Series; a second
/// lineage is rejected as a lineage mismatch. The Series therefore carries the
/// union of the specialty capabilities, because a child grant may never widen
/// its parent.
#[allow(dead_code)]
pub(super) fn open_review_lineage_for_specialties(
    harness: &Harness,
    cli: &mut ConnectionState,
    workspace: &WorkspaceResult,
    root_identity: &CliIdentity,
    specialties: &[&str],
    key: &str,
) -> (SessionResult, Vec<SessionResult>) {
    let adapter_id = format!("{key}-host");
    let mut host = harness.connection(ClientKind::HostAdapter);
    let mut register = plain_params(json!({
        "adapterId":adapter_id,
        "capabilities":["create","attest"]
    }));
    register.capability_token = Some(harness.host_credential());
    register.idempotency_key = Some(format!("{key}-host-register"));
    assert_success(&call(
        &harness.runtime,
        &mut host,
        RpcMethod::HostRegister,
        register,
    ));
    let capabilities = specialties
        .iter()
        .map(|specialty| Value::String(format!("review.specialty.{specialty}")))
        .collect::<Vec<_>>();
    let series_key = format!("{key}-series");
    let (series, series_delegation_id) = open_delegated_child(
        harness,
        cli,
        &mut host,
        workspace,
        root_identity,
        &adapter_id,
        "series",
        None,
        json!({
            // A child grant may never widen its parent, so the Series carries
            // every operation its author and Reviewer children execute.
            "operations":["document.save","evidence.finalize","evidence.record","lease.acquire","lease.release","lease.renew","lease.status","review.record","verification.plan"],
            "capabilities":capabilities,
            "paths":[{"kind":"project_root"}]
        }),
        &series_key,
    );
    let series_identity = identity_for_work_item(
        workspace,
        &series,
        &root_identity.work_item_id,
        &format!("{series_key}-agent"),
    );
    let author = open_delegated_child(
        harness,
        cli,
        &mut host,
        workspace,
        &series_identity,
        &adapter_id,
        "task",
        Some(&series_delegation_id),
        json!({
            "operations":["document.save","evidence.finalize","evidence.record","lease.acquire","lease.release","lease.renew","lease.status","verification.plan"],
            "capabilities":[],
            "paths":[{"kind":"project_root"}]
        }),
        &format!("{key}-author"),
    )
    .0;
    let reviewers = specialties
        .iter()
        .map(|specialty| {
            open_delegated_child(
                harness,
                cli,
                &mut host,
                workspace,
                &series_identity,
                &adapter_id,
                "reviewer",
                Some(&series_delegation_id),
                json!({
                    // Tier 2+ hands the exclusive Work Item lease from one
                    // specialty to the next, so a reviewer must be able to
                    // release what it acquired.
                    "operations":["lease.acquire","lease.release","review.record"],
                    "capabilities":[format!("review.specialty.{specialty}")],
                    "paths":[{"kind":"project_root"}]
                }),
                &reviewer_child_key(key, specialties, specialty),
            )
            .0
        })
        .collect();
    (author, reviewers)
}

/// Delegation keys must be unique per child, so a multi-specialty lineage
/// qualifies each reviewer while a single-specialty lineage keeps the original
/// `{key}-reviewer` key its callers already depend on.
pub(super) fn reviewer_child_key(key: &str, specialties: &[&str], specialty: &str) -> String {
    if specialties.len() == 1 {
        format!("{key}-reviewer")
    } else {
        format!("{key}-{specialty}-reviewer")
    }
}

#[allow(clippy::too_many_arguments)]
fn open_delegated_child(
    harness: &Harness,
    cli: &mut ConnectionState,
    host: &mut ConnectionState,
    workspace: &WorkspaceResult,
    parent_identity: &CliIdentity,
    adapter_id: &str,
    child_role: &str,
    parent_delegation_id: Option<&str>,
    grant: Value,
    key: &str,
) -> (SessionResult, String) {
    let child_session_id = Uuid::new_v4().to_string();
    let ack_id = Uuid::new_v4().to_string();
    let state_bytes = fs::read(&harness.state_path).expect("delegation state");
    let state: Value = serde_json::from_slice(&state_bytes).expect("delegation state JSON");
    let input_revision = state["revision"].as_u64().expect("delegation revision");
    let input_fingerprint = hex::encode(Sha256::digest(&state_bytes));
    let deadline_unix_ms = harness.now_unix_ms.saturating_add(60_000);
    let expires_at_unix_ms = harness.now_unix_ms.saturating_add(50_000);
    let create_payload = if parent_delegation_id.is_none() {
        let flow = success(&call(
            &harness.runtime,
            cli,
            RpcMethod::FlowNext,
            trusted_params(parent_identity, json!({})),
        ));
        let kind = flow["nextAction"]["kind"].as_str();
        assert!(
            kind == Some("delegate-series")
                || (kind == Some("await-agent-work")
                    && matches!(flow["phase"].as_str(), Some("coding" | "test-running"))),
            "Root flow decision is not delegable: {flow}"
        );
        let decision_digest = flow["decisionDigest"]
            .as_str()
            .unwrap_or_else(|| panic!("delegable Root decision lacks decisionDigest: {flow}"));
        let intent_key = format!(
            "{}\0{}\0{}",
            parent_identity.workspace_id, parent_identity.work_item_id, decision_digest
        );
        let intent = harness
            .persistence
            .load_record("flow-delegation-intent/v1", &intent_key)
            .expect("flow delegation intent lookup")
            .unwrap_or_else(|| panic!("flow.next did not persist its delegation intent: {flow}"));
        assert_eq!(
            intent["workspaceId"], parent_identity.workspace_id,
            "{intent}"
        );
        assert_eq!(
            intent["workItemId"], parent_identity.work_item_id,
            "{intent}"
        );
        if flow["nextAction"]["kind"] == "delegate-series" {
            assert_eq!(
                intent["seriesKind"], flow["nextAction"]["seriesKind"],
                "{intent}"
            );
        }
        assert_eq!(intent["decisionDigest"], decision_digest, "{intent}");
        assert_eq!(
            intent["stateRevision"], flow["stateRevision"],
            "flow delegation intent must bind the projected revision: {intent}"
        );
        assert!(
            intent["inputFingerprint"]
                .as_str()
                .is_some_and(|value| value.len() == 64),
            "flow delegation intent must bind a digest-shaped input: {intent}"
        );
        json!({"flowDecisionDigest":decision_digest})
    } else {
        json!({
            "childRole":child_role,
            "parentDelegationId":parent_delegation_id,
            "inputRevision":input_revision,
            "inputFingerprint":input_fingerprint,
            "deadlineUnixMs":deadline_unix_ms,
            "grant":grant
        })
    };
    let mut create = trusted_params(parent_identity, create_payload);
    create.idempotency_key = Some(format!("{key}-create"));
    let create_wire = serde_json::to_value(&create).expect("delegation.create request serializes");
    serde_json::from_value::<RequestParams<Value>>(create_wire.clone()).unwrap_or_else(|error| {
        panic!(
            "delegation.create RequestParams fail their own wire contract: {error}: {create_wire}"
        )
    });
    let created = success(&call(
        &harness.runtime,
        cli,
        RpcMethod::DelegationCreate,
        create,
    ));
    let delegation_id = created["delegationId"]
        .as_str()
        .expect("child delegation id")
        .to_owned();
    let action = success(&call(
        &harness.runtime,
        host,
        RpcMethod::HostActionNext,
        plain_params(json!({"adapterId":adapter_id})),
    ));
    let mut ack = plain_params(json!({
        "adapterId":adapter_id,
        "ack":{
            "ackId":ack_id,
            "actionId":action["actionId"],
            "commandSeq":action["commandSeq"],
            "outcome":"accepted",
            "hostTaskId":format!("{key}-host-task"),
            "sessionId":child_session_id
        }
    }));
    ack.idempotency_key = Some(format!("{key}-ack"));
    assert_success(&call(&harness.runtime, host, RpcMethod::HostActionAck, ack));
    let mut accept = plain_params(json!({
        "delegationId":delegation_id,
        "claimId":action["claimId"],
        "actionId":action["actionId"],
        "childSessionId":child_session_id,
        "expiresAtUnixMs":expires_at_unix_ms
    }));
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.work_item_id = Some(parent_identity.work_item_id.clone());
    accept.idempotency_key = Some(format!("{key}-accept"));
    assert_success(&call(
        &harness.runtime,
        cli,
        RpcMethod::DelegationAccept,
        accept,
    ));
    let agent_id = format!("{key}-agent");
    let mut child_open = plain_params(json!({
        "externalKey":format!("{key}-external"),
        "role":child_role,
        "engaged":true,
        "delegationId":delegation_id
    }));
    child_open.workspace_id = Some(workspace.workspace_id.clone());
    child_open.work_item_id = Some(parent_identity.work_item_id.clone());
    child_open.agent_id = Some(agent_id);
    child_open.session_id = Some(child_session_id);
    child_open.idempotency_key = Some(format!("{key}-open"));
    let session = serde_json::from_value(success(&call(
        &harness.runtime,
        cli,
        RpcMethod::SessionOpen,
        child_open,
    )))
    .expect("child session");
    (session, delegation_id)
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

pub(super) fn lease_args(
    lease_id: &str,
    fencing: u64,
    key: &str,
    renew: bool,
    owner_role: &str,
) -> Vec<String> {
    let mut values = args(&[
        "--owner",
        &format!("{{\"role\":\"{owner_role}\"}}"),
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

pub(super) fn plain_params(payload: Value) -> RequestParams<Value> {
    params(payload)
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

/// Drives one clean `review.record` per required specialty until the Review
/// reaches its terminal authority.
///
/// Hand-writing a completed `review` projection into project state is not a
/// shortcut for this: the Review Gates deliberately reject state that has no
/// matching SQLite projection (see
/// `review_gate_e2e::valid_state_without_the_sqlite_projection_fails_every_review_gate`),
/// because state alone is forgeable. Only the real operation writes the durable
/// event and the projection together, so the fixture has to go through it.
///
/// The specialty set follows the tier the daemon derives from `state.scale`, so
/// a caller that changes the scale automatically gets the matching reviewer
/// count.
///
/// The caller must use [`Harness::new_realtime`]: `review.record` checks
/// reviewer/Series/Root session liveness against the operation's observed
/// timestamp, which is real wall-clock time, so lineage opened under the fixed
/// test clock is already expired when the contribution is adjudicated.
pub(super) fn install_completed_review_authority(
    harness: &Harness,
    cli: &mut ConnectionState,
    workspace: &WorkspaceResult,
    root_identity: &CliIdentity,
    work_item_id: &str,
    key: &str,
) {
    let scale = read_state_value(harness)["scale"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let specialties: &[&str] = match scale.as_str() {
        "large" | "大" => &["be", "ar", "qa"],
        "medium" | "中" => &["be", "ar"],
        _ => &["general"],
    };

    // Some completion fixtures seed `code-reviewed` up front because the test
    // is about terminal governance, not the preceding transition. A physical
    // execution lineage must still be opened from an agent-work phase, so open
    // it against the immediately preceding `test-running` projection and then
    // restore the seeded after-image before evidence/review bind their inputs.
    let seeded_state = read_state_value(harness);
    let seeded_code_reviewed =
        seeded_state["storyStates"][work_item_id]["phase"].as_str() == Some("code-reviewed");
    if seeded_code_reviewed {
        let mut delegable = seeded_state.clone();
        delegable["storyStates"][work_item_id]["phase"] = json!("test-running");
        delegable["storyStates"][work_item_id]["currentPhase"] = json!("test-running");
        fs::write(
            &harness.state_path,
            serde_json::to_vec_pretty(&delegable).expect("delegable review fixture serializes"),
        )
        .expect("delegable review fixture");
    }

    // The lineage opens first: the delegated author task drives the semantic
    // evidence operations the root orchestrator may no longer execute itself.
    let (author, reviewers) = open_review_lineage_for_specialties(
        harness,
        cli,
        workspace,
        root_identity,
        specialties,
        key,
    );
    if seeded_code_reviewed {
        fs::write(
            &harness.state_path,
            serde_json::to_vec_pretty(&seeded_state).expect("seeded review state serializes"),
        )
        .expect("restore seeded review state");
    }
    let author_identity = identity_for_work_item(
        workspace,
        &author,
        work_item_id,
        &format!("{key}-author-agent"),
    );

    // The completion milestone only closes after evidence.record and
    // evidence.finalize, and the review must cite the evidence id those
    // operations put in the ledger.
    let evidence_id = close_evidence_milestones(harness, cli, workspace, &author_identity, key);

    for (index, (specialty, reviewer)) in specialties.iter().zip(reviewers.iter()).enumerate() {
        let lineage_key = format!("{key}-{specialty}");
        // `open_delegated_child` registers each session under `{child key}-agent`.
        let reviewer_identity = identity_for_work_item(
            workspace,
            reviewer,
            work_item_id,
            &format!("{}-agent", reviewer_child_key(key, specialties, specialty)),
        );

        let mut lease_request = operation_params(
            &reviewer_identity,
            "lease.acquire",
            json!({"owner":{"role":"reviewer"},"ttlSeconds":300}),
        );
        lease_request.idempotency_key = Some(format!("{lineage_key}-lease"));
        let lease = success(&call(
            &harness.runtime,
            cli,
            RpcMethod::OperationExecute,
            lease_request,
        ));
        let lease_id = lease["data"]["leaseId"]
            .as_str()
            .expect("reviewer lease id")
            .to_owned();
        let fencing = lease["data"]["fencingToken"]
            .as_u64()
            .expect("reviewer fencing token");

        let mut record = operation_params(
            &reviewer_identity,
            "review.record",
            json!({
                "status":"passed",
                "findings":[],
                "reviewedPaths":["src/lib.rs"],
                "evidenceIds":[evidence_id.clone()]
            }),
        );
        record.lease_id = Some(lease_id.clone());
        record.fencing_token = Some(fencing);
        record.expected_revision = Some(current_revision(harness));
        record.idempotency_key = Some(format!("{lineage_key}-record"));
        let recorded = success(&call(
            &harness.runtime,
            cli,
            RpcMethod::OperationExecute,
            record,
        ));
        let last = index + 1 == specialties.len();
        // A batch is only adjudicated once every required specialty has
        // contributed; before that the runtime keeps asking for the rest.
        let retained = recorded["data"]["batch"]["retainedContributions"]
            .as_array()
            .map(Vec::len);
        assert_eq!(
            retained,
            Some(index + 1),
            "{specialty} contribution must be retained: {recorded}"
        );
        if last {
            assert_eq!(
                recorded["data"]["batch"]["latestStatus"], "VALID_CLEAN",
                "the complete specialty set must produce a clean batch: {recorded}"
            );
        } else {
            assert_eq!(
                recorded["data"]["nextAction"]["kind"], "retry_missing",
                "{specialty} contribution must leave the batch pending: {recorded}"
            );
        }

        // Tier 2+ serializes specialties by handing the exclusive Work Item
        // lease from one reviewer to the next, so each contribution releases.
        let mut release = operation_params(
            &reviewer_identity,
            "lease.release",
            json!({"owner":{"role":"reviewer"}}),
        );
        release.lease_id = Some(lease_id);
        release.fencing_token = Some(fencing);
        release.idempotency_key = Some(format!("{lineage_key}-release"));
        assert_success(&call(
            &harness.runtime,
            cli,
            RpcMethod::OperationExecute,
            release,
        ));

        if last {
            let state = read_state_value(harness);
            assert_eq!(
                state["reviewSession"]["status"], "completed",
                "the final contribution must complete the review session: {state}"
            );
            assert!(
                state["review"]["attempt"].is_object(),
                "the terminal projection must carry the latest attempt: {state}"
            );
        }
    }
}

/// Drives `evidence.record` then `evidence.finalize` as the delegated author
/// task, which is what advances the completion milestone `None ->
/// ImplementationVerified -> ReviewReady`. Returns the daemon-derived evidence
/// id the review must cite.
///
/// `Completed` may only be committed from `GovernanceClosed`, and that milestone
/// is only reachable through this chain followed by `review.record`, so a
/// completion fixture has to walk all three operations.
fn close_evidence_milestones(
    harness: &Harness,
    cli: &mut ConnectionState,
    workspace: &WorkspaceResult,
    author_identity: &CliIdentity,
    key: &str,
) -> String {
    let mut lease_request = operation_params(
        author_identity,
        "lease.acquire",
        json!({"owner":{"role":"task"},"ttlSeconds":300}),
    );
    lease_request.idempotency_key = Some(format!("{key}-evidence-lease"));
    let lease = success(&call(
        &harness.runtime,
        cli,
        RpcMethod::OperationExecute,
        lease_request,
    ));
    let lease_id = lease["data"]["leaseId"]
        .as_str()
        .expect("evidence lease id")
        .to_owned();
    let fencing = lease["data"]["fencingToken"]
        .as_u64()
        .expect("evidence fencing token");

    let input = authoritative_input_fingerprint(harness, workspace);
    let mut record = operation_params(
        author_identity,
        "evidence.record",
        json!({
            "artifactPath":"evidence/result.json",
            "kind":"focused-test",
            "inputFingerprint":input,
            "exitCode":0
        }),
    );
    record.lease_id = Some(lease_id.clone());
    record.fencing_token = Some(fencing);
    record.expected_revision = Some(current_revision(harness));
    record.idempotency_key = Some(format!("{key}-evidence-record"));
    let recorded = success(&call(
        &harness.runtime,
        cli,
        RpcMethod::OperationExecute,
        record,
    ));
    let evidence_id = recorded["data"]["evidenceId"]
        .as_str()
        .unwrap_or_else(|| panic!("evidence.record returns its evidence id: {recorded}"))
        .to_owned();

    let mut finalize = operation_params(author_identity, "evidence.finalize", json!({}));
    finalize.lease_id = Some(lease_id.clone());
    finalize.fencing_token = Some(fencing);
    finalize.expected_revision = Some(current_revision(harness));
    finalize.idempotency_key = Some(format!("{key}-evidence-finalize"));
    assert_success(&call(
        &harness.runtime,
        cli,
        RpcMethod::OperationExecute,
        finalize,
    ));

    let mut release = operation_params(
        author_identity,
        "lease.release",
        json!({"owner":{"role":"task"}}),
    );
    release.lease_id = Some(lease_id);
    release.fencing_token = Some(fencing);
    release.idempotency_key = Some(format!("{key}-evidence-release"));
    assert_success(&call(
        &harness.runtime,
        cli,
        RpcMethod::OperationExecute,
        release,
    ));
    evidence_id
}

fn read_state_value(harness: &Harness) -> Value {
    serde_json::from_slice(&fs::read(&harness.state_path).expect("review state bytes"))
        .expect("review state JSON")
}

/// Story document whose AC ids cover the plan verification matrix below.
const TIER2_STORY_DOCUMENT: &str = "# Story\n\nAC-1 AC-2 AC-3 AC-4 AC-5 AC-6 AC-7\nAC-8 AC-9 AC-10 AC-11 AC-12 AC-13 AC-14\n\n## verification\n";

/// Installs the state and documents a Tier 2 review needs to close.
///
/// A Tier 2 clean batch is only sealed once the deterministic final proof
/// (`G-CODEPLAN-SRC`, `G-14`, `G-08`) evaluates PASS, which requires an approved
/// plan whose verification matrix covers every AC the Story document declares
/// (this fixture uses 14 rows for its 14 ACs; no fixed row count is required),
/// and source reads that exist on disk.
pub(super) fn install_tier2_review_prerequisites(
    harness: &Harness,
    work_item_id: &str,
    story_id: &str,
) {
    let root = harness.workspace_root.path();
    let story_path = format!("ae-sdd-doc/Story/{story_id}.md");
    for (relative, body) in [
        (story_path.as_str(), TIER2_STORY_DOCUMENT),
        ("ae-sdd-doc/RA/review.md", "# RA\n"),
        ("ae-sdd-doc/DR/review.md", "# DR\n"),
    ] {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("document parent")).expect("document directory");
        fs::write(path, body).expect("review document");
    }

    let verification = (1..=14)
        .map(|index| {
            json!({
                "id":format!("V-{index:03}"),
                "acId":format!("AC-{index}"),
                "boundary":"unit",
                "command":"cargo test",
                "expected":"pass"
            })
        })
        .collect::<Vec<_>>();
    let mut state = read_state_value(harness);
    state["executionPlan"] = json!({
        "goal":"tier 2 review prerequisite fixture",
        "changedPaths":["src/lib.rs"],
        "verification":verification,
        "risks":["fixture risk"],
        "approved":true,
        "sourceReads":[
            "src/lib.rs",
            "ae-sdd-doc/RA/review.md",
            "ae-sdd-doc/DR/review.md",
            story_path
        ]
    });
    state["documentPaths"] = json!({"story":story_path});
    // Completion is authorized from the `GovernanceClosed` milestone, and the
    // milestone only advances for a Work Item that carries an execution runtime
    // section. A code-reviewed item has executed, so the fixture seeds the
    // cursor the daemon would have committed and lets the evidence and review
    // operations earn the milestone from `none`.
    state["executionRuntime"] = json!({
        "schemaVersion":1,
        "queueDigest":format!("sha256:{}", "1".repeat(64)),
        "capsuleDigest":format!("sha256:{}", "2".repeat(64)),
        "activeSliceOrdinal":1,
        "completionMilestone":"none"
    });
    if let Some(story_state) = state
        .pointer_mut(&format!("/storyStates/{work_item_id}"))
        .and_then(Value::as_object_mut)
    {
        story_state.insert("docPath".to_owned(), json!(story_path));
    }
    fs::write(
        &harness.state_path,
        serde_json::to_vec_pretty(&state).expect("prerequisite state serializes"),
    )
    .expect("prerequisite state");
}

/// Computes the Review input fingerprint the daemon would derive from the
/// current state plus workspace inventory.
fn authoritative_input_fingerprint(harness: &Harness, workspace: &WorkspaceResult) -> String {
    let state = read_state_value(harness);
    // Only the canonical root and the locked state feed the fingerprint, so a
    // minimal workspace projection is sufficient here.
    let business = ae_sdd_runtime::BusinessWorkspace {
        workspace_id: workspace.workspace_id.clone(),
        canonical_root: fs::canonicalize(harness.workspace_root.path())
            .expect("workspace canonicalizes")
            .to_string_lossy()
            .into_owned(),
        project_key: workspace.project_key.clone(),
        mode: WorkspaceMode::RustCanary,
        agent_role: None,
        agent_grant: None,
        caller_kind: None,
        inventory_generation: workspace.inventory_generation,
    };
    authoritative_review_workspace_input_fingerprint(&business, &state)
        .expect("authoritative review input fingerprint")
        .to_string()
}

fn current_revision(harness: &Harness) -> u64 {
    read_state_value(harness)["revision"]
        .as_u64()
        .expect("state revision")
}

#[allow(dead_code)]
fn install_hand_written_review_authority(
    harness: &Harness,
    root_session_id: &str,
    inventory_generation: u64,
    review_id: &str,
) {
    const RULESET: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    let policy = RuntimeConfig::default().policy_digest;
    let mut state: Value = serde_json::from_slice(
        &fs::read(&harness.state_path).expect("review authority state bytes"),
    )
    .expect("review authority state JSON");
    let input = review_input_fingerprint(&state).to_string();
    let source_revision = state["revision"].as_u64().expect("source revision");
    let batch_id = format!("{review_id}-batch");
    let attempt_id = format!("{review_id}-attempt");
    let session: ReviewSessionV2 = serde_json::from_value(json!({
        "schemaVersion":"v2",
        "reviewId":review_id,
        "parentReviewId":null,
        "tier":"tier2",
        "requiredSpecialties":["be","ar"],
        "authorSessionId":"00000000-0000-0000-0000-000000000099",
        "rootSessionId":root_session_id,
        "inputFingerprint":input,
        "rulesetFingerprint":RULESET,
        "policyDigest":policy,
        "sourceRevision":source_revision,
        "inventoryGeneration":inventory_generation,
        "repairClass":"none",
        "cleanPolicy":{"cleanTarget":1,"finalProofRequirement":"deterministic_gates"},
        "budget":{
            "maxAttempts":5,
            "maxValidBatches":3,
            "maxRemediations":2,
            "maxWallClockMinutes":60
        },
        "counters":{
            "attempts":0,
            "validBatches":0,
            "cleanStreak":0,
            "remediations":0,
            "infraFailures":0,
            "protocolFailures":0
        },
        "status":"running",
        "startedAt":"2026-07-25T10:00:00Z",
        "deadlineAt":"2026-07-25T11:00:00Z",
        "terminalAt":null
    }))
    .expect("review session fixture");
    let attempt: ReviewAttemptV2 = serde_json::from_value(json!({
        "schemaVersion":"v2",
        "reviewId":review_id,
        "batchId":batch_id,
        "attemptId":attempt_id,
        "attemptOrdinal":1,
        "idempotencyKey":format!("{review_id}-attempt"),
        "inputFingerprint":input,
        "rulesetFingerprint":RULESET,
        "contributions":[
            clean_review_contribution(
                &attempt_id, &input, RULESET, &policy, root_session_id, "be", 10,
            ),
            clean_review_contribution(
                &attempt_id, &input, RULESET, &policy, root_session_id, "ar", 11,
            )
        ],
        "observedAt":"2026-07-25T10:01:00Z",
        "finalProof":{
            "kind":"deterministic_gates",
            "digest":policy,
            "sourceRevision":source_revision,
            "inputFingerprint":input,
            "rulesetFingerprint":RULESET,
            "observedAt":"2026-07-25T10:00:30Z"
        },
        "projectAuthority":{
            "projectReceiptRef":".ae-sdd/evidence/review.json",
            "activeManifestDigest":policy,
            "stateReceiptRefDigest":policy,
            "journalMutationId":format!("{review_id}-mutation")
        },
        "remediation":null
    }))
    .expect("review attempt fixture");
    let evaluated = ReviewSupervisor::evaluate(&session, None, attempt).expect("review evaluates");
    state["inputFingerprint"] = json!(input);
    state["rulesetFingerprint"] = json!(RULESET);
    state["policyDigest"] = json!(policy);
    state["inventoryGeneration"] = json!(inventory_generation);
    state["reviewSession"] =
        serde_json::to_value(evaluated.next_session()).expect("review session serializes");
    state["review"] = json!({
        "status":"passed",
        "findings":[],
        "batch":evaluated.next_batch(),
        "receipt":evaluated.exit_receipt().expect("review exit receipt")
    });
    fs::write(
        &harness.state_path,
        serde_json::to_vec_pretty(&state).expect("review state serializes"),
    )
    .expect("review state");
}

fn clean_review_contribution(
    source_attempt_id: &str,
    input: &str,
    ruleset: &str,
    policy: &str,
    root_session_id: &str,
    specialty: &str,
    seed: u128,
) -> Value {
    json!({
        "sourceAttemptId":source_attempt_id,
        "reviewer":{
            "agentRole":"reviewer",
            "specialty":specialty,
            "grantedSpecialties":[specialty],
            "physicalSessionId":format!("00000000-0000-0000-0000-{seed:012x}"),
            "rootSessionId":root_session_id,
            "delegationId":format!("10000000-0000-0000-0000-{seed:012x}"),
            "lineageDepth":2,
            "attestationRef":format!(".ae-sdd/evidence/attestation-{seed}.json"),
            "attestationDigest":policy,
            "specialtyGrantDigest":policy
        },
        "outcome":"clean",
        "findings":[],
        "reportDigest":policy,
        "contributionDigest":format!("{seed:064x}"),
        "inputFingerprint":input,
        "rulesetFingerprint":ruleset
    })
}

fn review_input_fingerprint(state: &Value) -> InputFingerprint {
    let mut authority = state.clone();
    strip_review_derived_fields(&mut authority);
    InputFingerprint::digest(serde_json::to_vec(&authority).expect("review input serializes"))
}

fn strip_review_derived_fields(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for field in [
                "review",
                "reviewSession",
                "reviewLoop",
                "gateResults",
                "hookGuard",
                "nextActions",
                "inputFingerprint",
                "rulesetFingerprint",
                "policyDigest",
                "inventoryGeneration",
                "revision",
                "lastFencingToken",
                "lastMutation",
            ] {
                object.remove(field);
            }
            for child in object.values_mut() {
                strip_review_derived_fields(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                strip_review_derived_fields(child);
            }
        }
        _ => {}
    }
}
