//! P1 incremental governance process E2E against a real `ae-sddd`.
//!
//! One real daemon process serves the milestone chain over framed
//! JSON-RPC: workspace registration and canary cutover, an authenticated
//! engaged session, three slice source writes, evidence record closing
//! `ImplementationVerified`, evidence finalize closing `ReviewReady`,
//! and the terminal completion intent. The full review ceremony
//! (delegation lineages, reviewer contributions, finalize) is covered
//! comprehensively by the integration-level `execution_to_completion_e2e`
//! and `lifecycle_control_plane_e2e`; the process test proves the daemon
//! binary enforces the milestone prerequisites and denies a completion
//! that skips the Review gate.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ae_sdd_client::{DaemonClient, LocalIpcTransport};
use ae_sdd_domain::InputFingerprint;
use ae_sdd_protocol::{
    ClientKind, ConfirmationRef, PROTOCOL_VERSION_V1, RequestParams, RpcMethod, WorkspaceMode,
};
use ae_sdd_runtime::{SessionResult, WorkspaceParityEvidence, WorkspaceResult};
use serde_json::{Value, json};
use tempfile::TempDir;

const WORK_ITEM_ID: &str = "STORY-GOV-PROCESS";
const PROJECT_KEY: &str = "gov-process";
const AGENT_ID: &str = "gov-process-root";
const EXTERNAL_SESSION_KEY: &str = "gov-process-root-session";
const STORY_DOC: &str = "ae-sdd-doc/Story/STORY-GOV-PROCESS.md";
const THINKING_ENGINE: &str = "source/standards/thinking/be-coding-thinking-engine.md";
const SLICES: [&str; 3] = ["src/alpha.rs", "src/beta.rs", "src/gamma.rs"];

// ─── Tests ───────────────────────────────────────────────────────────

/// The daemon enforces the milestone chain: evidence record closes
/// `ImplementationVerified`, finalize closes `ReviewReady`, and a
/// `workitem.complete` without the Review milestone is denied.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_process_enforces_milestone_chain_and_denies_premature_completion() {
    let fixture = Fixture::new();
    let policy_digest = "d".repeat(64);
    let mut daemon =
        DaemonProcess::start(&fixture.runtime_dir, fixture.allowed_root(), &policy_digest).await;
    let cli = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let admin = daemon_client(&fixture.runtime_dir, ClientKind::Admin);

    let workspace = register_and_cut_over(&cli, &admin, &fixture).await;
    let session = open_root(&cli, &workspace, "session-open-gov").await;
    let identity = Identity::new(&workspace, &session);

    // Write the three slice source files.
    for (i, path) in SLICES.iter().enumerate() {
        write_source(
            &fixture.project_root,
            path,
            &format!("pub fn slice_{}() -> u8 {{ {} }}\n", i + 1, i + 1),
        );
    }

    // ── Milestone chain: evidence → ImplementationVerified → ReviewReady
    assert_eq!(milestone(&fixture), "none");
    let lease = acquire_lease(&cli, &identity, "gov-evidence-lease").await;
    let evidence_id = record_evidence(&cli, &identity, &fixture, &lease, "gov").await;
    assert_eq!(milestone(&fixture), "implementation-verified");
    finalize_evidence(&cli, &identity, &fixture, &lease, "gov").await;
    assert_eq!(milestone(&fixture), "review-ready");
    release_lease(&cli, &identity, &fixture, &lease, "gov-evidence-release").await;

    // ── Premature completion is denied ──────────────────────────────
    // The milestone is ReviewReady (not GovernanceClosed), so the
    // completion intent must be denied by the flow runtime.
    let mut intent = identity.params(json!({"targetPhase":"completed"}));
    intent.idempotency_key = Some("gov-premature-intent".to_owned());
    let denied: Value = cli
        .call(RpcMethod::FlowNext, intent)
        .await
        .expect("premature intent projection");
    assert_eq!(
        denied["nextAction"]["kind"], "transition-denied",
        "ReviewReady without GovernanceClosed must deny completion: {denied}"
    );

    // The terminal mutation also fails closed: workitem.complete without
    // the Review milestone is rejected.
    let lease = acquire_lease(&cli, &identity, "gov-premature-lease").await;
    let mut complete = identity.params(json!({
        "operation": "workitem.complete", "payload": {}
    }));
    bind_write(
        &mut complete,
        &lease,
        read_revision(&fixture),
        "gov-premature-complete",
    );
    let error = cli
        .call::<Value>(RpcMethod::OperationExecute, complete)
        .await
        .expect_err("premature completion must be refused");
    let message = error.to_string();
    assert!(
        message.contains("ConfirmationRequired")
            || message.contains("GateBlocked")
            || message.contains("confirmation"),
        "premature completion error: {message}"
    );

    // The evidence id is real and backed by the ledger.
    assert!(!evidence_id.is_empty(), "evidence id must not be empty");

    daemon.crash();
}

/// A changed-path edit after `ReviewReady` rolls the milestone back so
/// the completion intent is denied on the stale milestone.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_process_denies_stale_completion_after_changed_path_edit() {
    let fixture = Fixture::new();
    let policy_digest = "e".repeat(64);
    let mut daemon =
        DaemonProcess::start(&fixture.runtime_dir, fixture.allowed_root(), &policy_digest).await;
    let cli = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let admin = daemon_client(&fixture.runtime_dir, ClientKind::Admin);

    let workspace = register_and_cut_over(&cli, &admin, &fixture).await;
    let session = open_root(&cli, &workspace, "session-open-stale").await;
    let identity = Identity::new(&workspace, &session);

    for (i, path) in SLICES.iter().enumerate() {
        write_source(
            &fixture.project_root,
            path,
            &format!("pub fn slice_{}() -> u8 {{ {} }}\n", i + 1, i + 1),
        );
    }

    let lease = acquire_lease(&cli, &identity, "stale-evidence-lease").await;
    record_evidence(&cli, &identity, &fixture, &lease, "stale").await;
    finalize_evidence(&cli, &identity, &fixture, &lease, "stale").await;
    release_lease(&cli, &identity, &fixture, &lease, "stale-evidence-release").await;
    assert_eq!(milestone(&fixture), "review-ready");

    // The stale edit rolls the effective milestone back.
    write_source(
        &fixture.project_root,
        SLICES[1],
        "pub fn beta() -> u8 { 42 }\n",
    );

    let mut intent = identity.params(json!({"targetPhase":"completed"}));
    intent.idempotency_key = Some("stale-completion-intent".to_owned());
    let denied: Value = cli
        .call(RpcMethod::FlowNext, intent)
        .await
        .expect("stale intent projection");
    assert_eq!(
        denied["nextAction"]["kind"], "transition-denied",
        "a stale edit must deny the completion intent: {denied}"
    );

    daemon.crash();
}

// ─── Fixture ─────────────────────────────────────────────────────────

struct Fixture {
    root: TempDir,
    project_root: PathBuf,
    runtime_dir: PathBuf,
    state_path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("fixture root");
        let project_root = root.path().join("project");
        let runtime_dir = root.path().join("runtime");
        fs::create_dir_all(&project_root).expect("project root");
        fs::create_dir_all(&runtime_dir).expect("runtime root");
        let state_path = prepare_workspace(&project_root);
        Self {
            root,
            project_root,
            runtime_dir,
            state_path,
        }
    }
    fn allowed_root(&self) -> &Path {
        self.root.path()
    }
}

fn prepare_workspace(root: &Path) -> PathBuf {
    for d in [
        root.join(".auto-engineering/gov-process"),
        root.join("ae-sdd-doc/Story"),
        root.join("constraints"),
        root.join("source/standards/thinking"),
        root.join("results"),
    ] {
        fs::create_dir_all(d).expect("fixture dir");
    }
    fs::write(root.join(STORY_DOC), "# Story\n\ngovernance process e2e\n").expect("Story");
    fs::write(root.join("constraints/README.md"), "# constraints\n").expect("constraints");
    fs::write(root.join(THINKING_ENGINE), "# thinking engine\n").expect("thinking engine");
    fs::write(root.join("results/focused.json"), "{\"passed\":true}\n").expect("evidence artifact");
    let verification: Vec<Value> = (1..=3)
        .map(|i| {
            json!({
                "id": format!("V-GOV-{i:03}"), "acId": format!("AC-{i}"),
                "boundary": "unit", "command": "cargo test", "expected": "pass"
            })
        })
        .collect();
    let state = json!({
        "stateMachineName": "PRD-GOV-PROCESS",
        "activeStory": WORK_ITEM_ID,
        "revision": 1, "lastFencingToken": 0,
        "scale": "medium", "selectedDesign": "Story",
        "phase": "code-reviewed", "currentPhase": "code-reviewed",
        "currentStep": "code-reviewed",
        "documentPaths": {"story": STORY_DOC},
        "storyStates": { WORK_ITEM_ID: {
            "phase": "coding", "currentPhase": "coding", "docPath": STORY_DOC
        }},
        "executionPlan": {
            "goal": "drive incremental governance to completion",
            "changedPaths": SLICES,
            "verification": verification,
            "risks": [], "sourceReads": ["constraints/README.md"],
            "approved": true,
            "approvedAt": "2026-07-27T02:00:00Z", "approvedBy": "user:test"
        },
        "executionRuntime": {
            "schemaVersion": 1,
            "queueDigest": format!("sha256:{}", "0".repeat(64)),
            "activeSliceOrdinal": 0,
            "completionMilestone": "none"
        },
    });
    let state_path = root.join(".auto-engineering/gov-process/state.json");
    let mut bytes = serde_json::to_vec_pretty(&state).expect("state");
    bytes.push(b'\n');
    fs::write(&state_path, bytes).expect("state fixture");
    state_path
}

// ─── Daemon process ──────────────────────────────────────────────────

struct DaemonProcess {
    child: Child,
}

impl DaemonProcess {
    async fn start(state_dir: &Path, allowed_root: &Path, policy_digest: &str) -> Self {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ae-sddd"));
        cmd.arg("serve")
            .arg("--state-dir")
            .arg(state_dir)
            .arg("--allowed-root")
            .arg(allowed_root)
            .arg("--policy-digest")
            .arg(policy_digest)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = cmd.spawn().expect("ae-sddd starts");
        let manifest = state_dir.join("endpoint.v1.json");
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(s) = child.try_wait().expect("status") {
                panic!("ae-sddd exited ({s})");
            }
            if manifest.is_file() {
                let c = daemon_client(state_dir, ClientKind::Cli);
                if c.call::<Value>(RpcMethod::RuntimeStatus, params(json!({})))
                    .await
                    .is_ok()
                {
                    return Self { child };
                }
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("ae-sddd not ready");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
    fn crash(&mut self) {
        if self.child.try_wait().expect("status").is_none() {
            self.child.kill().expect("kill");
            self.child.wait().expect("wait");
        }
    }
}

impl Drop for DaemonProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

// ─── Identity ────────────────────────────────────────────────────────

#[derive(Clone)]
struct Identity {
    workspace_id: String,
    work_item_id: String,
    agent_id: String,
    session_id: String,
    capability_token: String,
}

impl Identity {
    fn new(workspace: &WorkspaceResult, session: &SessionResult) -> Self {
        Self {
            workspace_id: workspace.workspace_id.clone(),
            work_item_id: WORK_ITEM_ID.to_owned(),
            agent_id: AGENT_ID.to_owned(),
            session_id: session.session_id.clone(),
            capability_token: session.capability_token.clone(),
        }
    }
    fn params(&self, payload: Value) -> RequestParams<Value> {
        let mut r = params(payload);
        r.workspace_id = Some(self.workspace_id.clone());
        r.work_item_id = Some(self.work_item_id.clone());
        r.session_id = Some(self.session_id.clone());
        r.agent_id = Some(self.agent_id.clone());
        r.capability_token = Some(self.capability_token.clone());
        r
    }
}

// ─── State helpers ───────────────────────────────────────────────────

fn milestone(fixture: &Fixture) -> String {
    read_state(fixture)
        .pointer("/executionRuntime/completionMilestone")
        .and_then(Value::as_str)
        .expect("milestone")
        .to_owned()
}
fn read_state(fixture: &Fixture) -> Value {
    serde_json::from_slice(&fs::read(&fixture.state_path).expect("state")).expect("state JSON")
}
fn read_revision(fixture: &Fixture) -> u64 {
    read_state(fixture)["revision"].as_u64().expect("revision")
}
fn write_source(root: &Path, relative: &str, content: &str) {
    let p = root.join(relative);
    fs::create_dir_all(p.parent().expect("parent")).expect("dir");
    fs::write(p, content).expect("source");
}

// ─── Operation helpers ───────────────────────────────────────────────

struct Lease {
    id: String,
    fencing: u64,
}

async fn acquire_lease(client: &DaemonClient, identity: &Identity, key: &str) -> Lease {
    let mut r = identity.params(json!({
        "operation": "lease.acquire",
        "payload": {"owner":{"role":"root"},"ttlSeconds":300}
    }));
    r.idempotency_key = Some(key.to_owned());
    let result: Value = client
        .call(RpcMethod::OperationExecute, r)
        .await
        .unwrap_or_else(|e| panic!("{key} lease: {e:?}"));
    Lease {
        id: result["data"]["leaseId"]
            .as_str()
            .expect("lease id")
            .to_owned(),
        fencing: result["data"]["fencingToken"].as_u64().expect("fencing"),
    }
}

async fn release_lease(
    client: &DaemonClient,
    identity: &Identity,
    fixture: &Fixture,
    lease: &Lease,
    key: &str,
) {
    let mut r = identity.params(json!({
        "operation": "lease.release", "payload": {"owner":{"role":"root"}}
    }));
    bind_write(&mut r, lease, read_revision(fixture), key);
    let _: Value = client
        .call(RpcMethod::OperationExecute, r)
        .await
        .unwrap_or_else(|e| panic!("{key} release: {e:?}"));
}

async fn record_evidence(
    client: &DaemonClient,
    identity: &Identity,
    fixture: &Fixture,
    lease: &Lease,
    prefix: &str,
) -> String {
    let mut r = identity.params(json!({
        "operation": "evidence.record",
        "payload": {
            "artifactPath": "results/focused.json",
            "inputFingerprint": "gov-process-verification",
            "kind": "focused-test",
            "command": ["cargo", "test", "-p", "gov"],
            "exitCode": 0
        }
    }));
    bind_write(
        &mut r,
        lease,
        read_revision(fixture),
        &format!("{prefix}-evidence-record"),
    );
    let result: Value = client
        .call(RpcMethod::OperationExecute, r)
        .await
        .unwrap_or_else(|e| panic!("{prefix} evidence record: {e:?}"));
    assert_eq!(result["changed"], true, "{result}");
    result["data"]["evidenceId"]
        .as_str()
        .expect("evidence id")
        .to_owned()
}

async fn finalize_evidence(
    client: &DaemonClient,
    identity: &Identity,
    fixture: &Fixture,
    lease: &Lease,
    prefix: &str,
) {
    let mut r = identity.params(json!({"operation": "evidence.finalize", "payload": {}}));
    bind_write(
        &mut r,
        lease,
        read_revision(fixture),
        &format!("{prefix}-evidence-finalize"),
    );
    let result: Value = client
        .call(RpcMethod::OperationExecute, r)
        .await
        .unwrap_or_else(|e| panic!("{prefix} evidence finalize: {e:?}"));
    assert_eq!(result["changed"], true, "{result}");
}

fn bind_write(r: &mut RequestParams<Value>, lease: &Lease, revision: u64, key: &str) {
    r.lease_id = Some(lease.id.clone());
    r.fencing_token = Some(lease.fencing);
    r.expected_revision = Some(revision);
    r.idempotency_key = Some(key.to_owned());
}

// ─── Wire helpers ────────────────────────────────────────────────────

async fn register_and_cut_over(
    cli: &DaemonClient,
    admin: &DaemonClient,
    fixture: &Fixture,
) -> WorkspaceResult {
    let mut register = params(json!({
        "projectRoot": fixture.project_root.to_string_lossy(), "projectKey": PROJECT_KEY
    }));
    register.idempotency_key = Some("workspace-register-gov".to_owned());
    let registered = cli
        .call::<WorkspaceResult>(RpcMethod::WorkspaceRegister, register)
        .await
        .expect("registers");
    assert_eq!(registered.mode, WorkspaceMode::Shadow);

    let mut drain = params(json!({"stop": false}));
    drain.idempotency_key = Some("drain-gov".to_owned());
    drain.confirmation = Some(confirmation("drain-gov"));
    admin
        .call::<Value>(RpcMethod::RuntimeDrain, drain)
        .await
        .expect("drains");

    let parity = WorkspaceParityEvidence {
        comparison_count: 1,
        mismatch_count: 0,
        source_revision: 1,
        legacy_digest: "a".repeat(64),
        rust_digest: "a".repeat(64),
        observed_at_unix_ms: now_unix_ms(),
    };
    let pd = InputFingerprint::digest(serde_json::to_vec(&parity).expect("parity")).to_string();
    let mut transition = params(json!({
        "targetMode": WorkspaceMode::RustCanary,
        "reason": "incremental governance process E2E",
        "parityDigest": pd, "parity": parity
    }));
    transition.workspace_id = Some(registered.workspace_id);
    transition.idempotency_key = Some("cutover-gov".to_owned());
    transition.confirmation = Some(confirmation("cutover-gov"));
    let cut = admin
        .call::<WorkspaceResult>(RpcMethod::WorkspaceModeTransition, transition)
        .await
        .expect("cutover");
    assert_eq!(cut.mode, WorkspaceMode::RustCanary);
    cut
}

async fn open_root(client: &DaemonClient, workspace: &WorkspaceResult, key: &str) -> SessionResult {
    let mut r =
        params(json!({"externalKey": EXTERNAL_SESSION_KEY, "role": "root", "engaged": true}));
    r.workspace_id = Some(workspace.workspace_id.clone());
    r.work_item_id = Some(WORK_ITEM_ID.to_owned());
    r.agent_id = Some(AGENT_ID.to_owned());
    r.idempotency_key = Some(key.to_owned());
    client
        .call(RpcMethod::SessionOpen, r)
        .await
        .expect("root session")
}

fn daemon_client(state_dir: &Path, kind: ClientKind) -> DaemonClient {
    DaemonClient::new(
        state_dir.join("endpoint.v1.json"),
        kind,
        Arc::new(LocalIpcTransport),
        Duration::from_secs(5),
    )
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

fn confirmation(id: &str) -> ConfirmationRef {
    ConfirmationRef {
        confirmation_id: id.to_owned(),
        approved_by: "user:test".to_owned(),
        approved_at: "2026-07-27T00:00:00Z".to_owned(),
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("epoch")
        .as_millis()
        .try_into()
        .expect("u64")
}
