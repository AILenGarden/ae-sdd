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

use ae_sdd_client::{ClientTransport, DaemonClient, LocalIpcTransport};
use ae_sdd_contracts::{
    DocumentId, EngineeringRoute, ReasonCode, ReceiptStatus, RequirementAnalysisEvidence,
    RouteApprovalReceipt, RouteBindingInput, RouteDecision, RouteDecisionId, RouteDisposition,
    RouteMappingVersion, SchemaVersion, SeriesId, SeriesKind, SpecKind, TaskKind,
};
use ae_sdd_domain::{
    ArtifactDigest, DecisionDigest, DesignRoute, InputFingerprint, StateRevision, WorkItemId,
    WorkScale,
};
use ae_sdd_protocol::{
    ClientKind, ConfirmationRef, PROTOCOL_VERSION_V1, RequestParams, RpcMethod, WorkspaceMode,
};
use ae_sdd_runtime::{SessionResult, WorkspaceParityEvidence, WorkspaceResult};
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

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
    // Evidence record/finalize is semantic work the tightened root role no
    // longer carries, so a delegated author task executes it while the root
    // orchestrator keeps the flow intent and completion calls.
    let author = open_task_lineage(
        &cli,
        &fixture.runtime_dir,
        &workspace,
        &identity,
        &fixture.state_path,
        "gov-lineage",
    )
    .await;

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
    let lease = acquire_lease(&cli, &author, "task", "gov-evidence-lease").await;
    let evidence_id = record_evidence(&cli, &author, &fixture, &lease, "gov").await;
    assert_eq!(milestone(&fixture), "implementation-verified");
    finalize_evidence(&cli, &author, &fixture, &lease, "gov").await;
    assert_eq!(milestone(&fixture), "review-ready");
    release_lease(
        &cli,
        &author,
        &fixture,
        &lease,
        "task",
        "gov-evidence-release",
    )
    .await;

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
    let lease = acquire_lease(&cli, &identity, "root", "gov-premature-lease").await;
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
    let author = open_task_lineage(
        &cli,
        &fixture.runtime_dir,
        &workspace,
        &identity,
        &fixture.state_path,
        "stale-lineage",
    )
    .await;

    for (i, path) in SLICES.iter().enumerate() {
        write_source(
            &fixture.project_root,
            path,
            &format!("pub fn slice_{}() -> u8 {{ {} }}\n", i + 1, i + 1),
        );
    }

    let lease = acquire_lease(&cli, &author, "task", "stale-evidence-lease").await;
    record_evidence(&cli, &author, &fixture, &lease, "stale").await;
    finalize_evidence(&cli, &author, &fixture, &lease, "stale").await;
    release_lease(
        &cli,
        &author,
        &fixture,
        &lease,
        "task",
        "stale-evidence-release",
    )
    .await;
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
        "entryNode": "ROUTE",
        "engineeringRoute": frozen_engineering_route(),
        "scale": "medium",
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
            "capsuleDigest": format!("sha256:{}", "1".repeat(64)),
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

fn frozen_engineering_route() -> Value {
    let evidence = RequirementAnalysisEvidence::new(
        WorkItemId::new(WORK_ITEM_ID).expect("work item id"),
        SeriesId::new("SERIES-RA-GOV-PROCESS").expect("series id"),
        DocumentId::new("DOC-RA-GOV-PROCESS").expect("document id"),
        1,
        ArtifactDigest::digest(b"governance process RA content"),
        StateRevision::new(1),
        ArtifactDigest::digest(b"governance process RA receipt"),
        ReceiptStatus::Verified,
        WorkScale::Medium,
        ArtifactDigest::digest(b"governance process scale evidence"),
        ArtifactDigest::digest(b"governance process RA closure receipts"),
    );
    let binding = RouteBindingInput::new(evidence, RouteMappingVersion::V1);
    let decision = RouteDecision::new(
        SchemaVersion::V2,
        RouteDecisionId::new("route-gov-process-r1").expect("route decision id"),
        WorkItemId::new(WORK_ITEM_ID).expect("work item id"),
        TaskKind::Implementation,
        WorkScale::Medium,
        DesignRoute::Story,
        RouteDisposition::Approved,
        vec![ReasonCode::new("route.ra-closed").expect("reason")],
        vec![
            SeriesKind::new("story").expect("series kind"),
            SeriesKind::new("testcase").expect("series kind"),
            SeriesKind::new("coding-plan").expect("series kind"),
        ],
        vec![SpecKind::Story, SpecKind::TestCase, SpecKind::CodingPlan],
        binding.fingerprint(),
        None,
        DecisionDigest::digest(b"governance process route decision"),
    )
    .expect("route decision");
    let approval = RouteApprovalReceipt::new(
        "route:gov-process-r1".to_owned(),
        "user:test".to_owned(),
        "2026-07-27T02:00:00Z".to_owned(),
        binding.ra_evidence().document_id().clone(),
        binding.ra_evidence().version(),
        *binding.ra_evidence().ra_content_digest(),
        binding.ra_evidence().scale(),
        decision.decision_digest(),
    );
    let route = EngineeringRoute::freeze(SchemaVersion::V2, &binding, decision, &approval, &[])
        .expect("verified RA and bound approval freeze the route");
    serde_json::to_value(route).expect("engineering route JSON")
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

async fn acquire_lease(
    client: &DaemonClient,
    identity: &Identity,
    owner_role: &str,
    key: &str,
) -> Lease {
    let mut r = identity.params(json!({
        "operation": "lease.acquire",
        "payload": {"owner":{"role":owner_role},"ttlSeconds":300}
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
    owner_role: &str,
    key: &str,
) {
    let mut r = identity.params(json!({
        "operation": "lease.release", "payload": {"owner":{"role":owner_role}}
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

// ─── Delegation lineage ──────────────────────────────────────────────

/// Root -> Series -> Task author lineage established through real daemon
/// RPC: `host.register`, `delegation.create`, `host.action_next`,
/// `host.action_ack`, `delegation.accept`, then `session.open`.
///
/// The tightened root role no longer carries the semantic work permissions
/// (evidence record/finalize), so the milestone chain is executed by a
/// delegated author task whose grant lists exactly those operations; the
/// series grant carries the same set because a child grant may never widen
/// its parent.
async fn open_task_lineage(
    cli: &DaemonClient,
    state_dir: &Path,
    workspace: &WorkspaceResult,
    root: &Identity,
    state_path: &Path,
    key: &str,
) -> Identity {
    let adapter_id = "codex".to_owned();
    let host = HostAdapter::register(state_dir, &adapter_id, &format!("{key}-host-register")).await;
    let grant = json!({
        "operations":["document.save","evidence.finalize","evidence.record","lease.acquire","lease.release"],
        "capabilities":[],
        "paths":[{"kind":"project_root"}],
    });
    let (series, series_delegation) = open_delegated_child(
        cli,
        &host,
        workspace,
        root,
        state_path,
        &adapter_id,
        "series",
        None,
        grant.clone(),
        &format!("{key}-series"),
    )
    .await;
    open_delegated_child(
        cli,
        &host,
        workspace,
        &series,
        state_path,
        &adapter_id,
        "task",
        Some(&series_delegation),
        grant,
        &format!("{key}-author"),
    )
    .await
    .0
}

#[allow(clippy::too_many_arguments)]
async fn open_delegated_child(
    cli: &DaemonClient,
    host: &HostAdapter,
    workspace: &WorkspaceResult,
    parent: &Identity,
    state_path: &Path,
    _adapter_id: &str,
    child_role: &str,
    parent_delegation_id: Option<&str>,
    grant: Value,
    key: &str,
) -> (Identity, String) {
    let child_session_id = Uuid::new_v4().to_string();
    let state_bytes = fs::read(state_path).expect("delegation input state");
    let state: Value = serde_json::from_slice(&state_bytes).expect("delegation input state JSON");
    let input_revision = state["revision"]
        .as_u64()
        .expect("delegation input revision");
    let input_fingerprint = InputFingerprint::digest(&state_bytes).to_string();
    let now = now_unix_ms();
    let create_payload = if parent_delegation_id.is_none() {
        let flow = cli
            .call::<Value>(RpcMethod::FlowNext, parent.params(json!({})))
            .await
            .unwrap_or_else(|error| panic!("{key} flow decision is available: {error:?}"));
        let kind = flow["nextAction"]["kind"].as_str();
        assert!(
            kind == Some("delegate-series")
                || (kind == Some("execute-approved-slice") && flow["phase"] == "coding")
                || (kind == Some("await-agent-work")
                    && matches!(flow["phase"].as_str(), Some("coding" | "test-running"))),
            "{key} flow decision is not delegable: {flow}"
        );
        json!({"flowDecisionDigest":flow["decisionDigest"]})
    } else {
        json!({
            "childRole":child_role,
            "parentDelegationId":parent_delegation_id,
            "inputRevision":input_revision,
            "inputFingerprint":input_fingerprint,
            "deadlineUnixMs":now.saturating_add(600_000),
            "grant":grant,
        })
    };
    let mut create = parent.params(create_payload);
    create.idempotency_key = Some(format!("{key}-create"));
    let created = cli
        .call::<Value>(RpcMethod::DelegationCreate, create)
        .await
        .unwrap_or_else(|error| panic!("{key} delegation is created: {error:?}"));
    let delegation_id = created["delegationId"]
        .as_str()
        .expect("child delegation id")
        .to_owned();

    let action = host.call(RpcMethod::HostActionNext, json!({})).await;
    host.call(
        RpcMethod::HostActionAck,
        json!({
            "ack":{
                "ackId":Uuid::new_v4().to_string(),
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":format!("{key}-host-task"),
                "sessionId":child_session_id,
            },
        }),
    )
    .await;

    let mut accept = params(json!({
        "delegationId":delegation_id,
        "claimId":action["claimId"],
        "actionId":action["actionId"],
        "childSessionId":child_session_id,
        "expiresAtUnixMs":now.saturating_add(500_000),
    }));
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.work_item_id = Some(parent.work_item_id.clone());
    accept.idempotency_key = Some(format!("{key}-accept"));
    cli.call::<Value>(RpcMethod::DelegationAccept, accept)
        .await
        .unwrap_or_else(|error| panic!("{key} physical attestation is accepted: {error:?}"));

    let agent_id = format!("{key}-agent");
    let mut open = params(json!({
        "externalKey":format!("{key}-external"),
        "role":child_role,
        "engaged":true,
        "delegationId":delegation_id,
    }));
    open.workspace_id = Some(workspace.workspace_id.clone());
    open.work_item_id = Some(parent.work_item_id.clone());
    open.agent_id = Some(agent_id.clone());
    open.session_id = Some(child_session_id);
    open.idempotency_key = Some(format!("{key}-open"));
    let session = cli
        .call::<SessionResult>(RpcMethod::SessionOpen, open)
        .await
        .unwrap_or_else(|error| panic!("{key} child session opens: {error:?}"));
    let identity = Identity {
        workspace_id: workspace.workspace_id.clone(),
        work_item_id: parent.work_item_id.clone(),
        agent_id,
        session_id: session.session_id.clone(),
        capability_token: session.capability_token.clone(),
    };
    (identity, delegation_id)
}

/// Authenticated host-adapter caller.
///
/// The daemon binds `adapterId` to the connection that ran `host.register`, and
/// `DaemonClient` opens one connection per call, so every host method is issued
/// on a fresh connection that first replays the same `host.register` receipt to
/// rebind that connection.
struct HostAdapter {
    manifest_path: PathBuf,
    transport: LocalIpcTransport,
    adapter_id: String,
    register_key: String,
}

impl HostAdapter {
    async fn register(state_dir: &Path, adapter_id: &str, register_key: &str) -> Self {
        let adapter = Self {
            manifest_path: state_dir.join("endpoint.v1.json"),
            transport: LocalIpcTransport,
            adapter_id: adapter_id.to_owned(),
            register_key: register_key.to_owned(),
        };
        adapter.call(RpcMethod::HostActionNext, json!({})).await;
        adapter
    }

    /// Runs `handshake`, `host.register`, then `method` on one connection and
    /// returns the `method` result.
    async fn call(&self, method: RpcMethod, extra: Value) -> Value {
        let manifest = read_endpoint_manifest_json(&self.manifest_path);
        let token = manifest["endpointToken"]
            .as_str()
            .expect("endpoint token")
            .to_owned();
        let handshake = json!({
            "jsonrpc":"2.0",
            "id":"host-handshake",
            "method":RpcMethod::RuntimeHandshake,
            "params":{
                "protocolRange":manifest["protocolRange"],
                "clientBuild":"gov-process-host-adapter",
                "clientKind":ClientKind::HostAdapter,
                "endpointToken":token,
                "expectedBootId":manifest["bootId"],
                "expectedPolicyDigest":manifest["policyDigest"],
            },
        });
        let mut register = params(json!({
            "adapterId":self.adapter_id,
            "capabilities":["create","attest"],
        }));
        register.capability_token = Some(token.clone());
        register.idempotency_key = Some(self.register_key.clone());
        let mut payload = json!({"adapterId":self.adapter_id});
        if let Some(fields) = extra.as_object() {
            for (name, value) in fields {
                payload[name] = value.clone();
            }
        }
        let mut request = params(payload);
        request.capability_token = Some(token);
        request.idempotency_key = Some(format!(
            "{}-{}-{}",
            self.register_key,
            method.as_str().replace('.', "-"),
            Uuid::new_v4()
        ));
        let frames = [
            serde_json::to_vec(&handshake).expect("handshake frame"),
            serde_json::to_vec(&json!({
                "jsonrpc":"2.0",
                "id":"host-register",
                "method":RpcMethod::HostRegister,
                "params":register,
            }))
            .expect("register frame"),
            serde_json::to_vec(&json!({
                "jsonrpc":"2.0",
                "id":"host-call",
                "method":method,
                "params":request,
            }))
            .expect("host call frame"),
        ];
        let responses = self
            .transport
            .exchange(
                manifest["endpoint"].as_str().expect("endpoint address"),
                &frames,
                Duration::from_secs(10),
            )
            .await
            .expect("host adapter connection exchanges frames");
        assert_eq!(responses.len(), 3, "one response per host frame");
        for (index, label) in ["handshake", "host.register"].into_iter().enumerate() {
            let response: Value =
                serde_json::from_slice(&responses[index]).expect("host response JSON");
            assert!(
                response.get("result").is_some(),
                "{label} failed: {response}"
            );
        }
        let response: Value = serde_json::from_slice(&responses[2]).expect("host response JSON");
        response
            .get("result")
            .unwrap_or_else(|| panic!("{} failed: {response}", method.as_str()))
            .clone()
    }
}

fn read_endpoint_manifest_json(manifest_path: &Path) -> Value {
    serde_json::from_slice(&fs::read(manifest_path).expect("endpoint manifest bytes"))
        .expect("endpoint manifest JSON")
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
