//! P0 process-level execution-efficiency E2E against a real `ae-sddd`.
//!
//! One real daemon process serves the whole supervised resume loop over
//! framed JSON-RPC: workspace registration and canary cutover, an
//! authenticated engaged session, `execution.resume` full then no-change,
//! and the supervised tool cadence driven through the Hook fast path —
//! focused RED, minimal patch, focused GREEN, evidence — until the slice's
//! supervised loop is complete.  The forbidden paths stay closed on the
//! real wire: a broad verification before the focused GREEN is denied with
//! `EXECUTION_PROGRESS_REQUIRED`, and the 13th consecutive investigation
//! call (default budgets: 4 calls per batch x 3 batches) is denied while
//! patch stays admissible.  The P0 performance gates from the
//! implementation plan §5 are asserted on the live responses: full capsule
//! <= 16 KiB, no-change <= 1 KiB, exactly one authority refresh per resume
//! and zero broad verifications before the focused GREEN.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ae_sdd_client::{DaemonClient, LocalIpcTransport};
use ae_sdd_domain::{ArtifactDigest, InputFingerprint};
use ae_sdd_protocol::{
    ClientKind, ConfirmationRef, PROTOCOL_VERSION_V1, RequestParams, RpcMethod, WorkspaceMode,
};
use ae_sdd_runtime::{SessionResult, WorkspaceParityEvidence, WorkspaceResult};
use serde_json::{Value, json};
use tempfile::TempDir;

const WORK_ITEM_ID: &str = "STORY-EFF-PROCESS";
const PROJECT_KEY: &str = "exec-efficiency-process";
const AGENT_ID: &str = "efficiency-process-root";
const EXTERNAL_SESSION_KEY: &str = "efficiency-process-root-session";
const STORY_DOC: &str = "ae-sdd-doc/Story/STORY-EFF-PROCESS.md";
const THINKING_ENGINE: &str = "source/standards/thinking/be-coding-thinking-engine.md";
const EXECUTION_DIR: &str = ".auto-engineering/STORY-EFF-PROCESS/execution";
const HOOK_DEADLINE_MS: u64 = 200;

/// P0 performance gates (implementation plan §5 / golden baseline fixture).
const MAX_FULL_CAPSULE_BYTES: usize = 16 * 1024;
const MAX_NO_CHANGE_BYTES: usize = 1024;
const MAX_AUTHORITY_REFRESHES_PER_RESUME: u64 = 1;
const MAX_RESUME_TO_FIRST_PATCH_MS: u128 = 300_000;
const INSPECTION_CALLS_PER_BATCH: u32 = 4;
const MAX_NO_PROGRESS_BATCHES: u32 = 3;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_process_resumes_and_supervises_one_slice_within_p0_budgets() {
    let fixture = Fixture::new();
    let policy_digest = "c".repeat(64);
    let mut daemon =
        DaemonProcess::start(&fixture.runtime_dir, fixture.allowed_root(), &policy_digest).await;
    let cli = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let admin = daemon_client(&fixture.runtime_dir, ClientKind::Admin);
    let hook = daemon_client(&fixture.runtime_dir, ClientKind::Hook);

    let workspace = register_and_cut_over(&cli, &admin, &fixture).await;
    let session = open_root(&cli, &workspace, "session-open-efficiency").await;
    let identity = Identity::new(&workspace, &session);
    let started = Instant::now();

    // Resume full: one authority refresh, a bounded capsule and the
    // FlowRuntime-owned next action.
    let first = cli
        .call::<Value>(
            RpcMethod::OperationExecute,
            operation_request(&identity, "execution.resume", json!({}), "resume-full"),
        )
        .await
        .expect("first resume succeeds through the process");
    let data = &first["data"];
    assert_eq!(data["projectionKind"], "full", "{first}");
    assert_eq!(
        data["authorityRefreshCount"].as_u64(),
        Some(MAX_AUTHORITY_REFRESHES_PER_RESUME),
        "{first}"
    );
    assert_eq!(data["nextAction"]["kind"], "execute-approved-slice");
    let capsule_wire = serde_json::to_vec(&data["capsule"]).expect("capsule serializes");
    assert!(
        capsule_wire.len() <= MAX_FULL_CAPSULE_BYTES,
        "full capsule wire projection exceeds the 16 KiB gate: {} bytes",
        capsule_wire.len()
    );
    let capsule_artifact = fixture.execution_artifact("capsule.json");
    assert!(
        capsule_artifact.len() <= MAX_FULL_CAPSULE_BYTES,
        "committed capsule artifact exceeds the 16 KiB gate: {} bytes",
        capsule_artifact.len()
    );
    let capsule_digest = data["capsuleDigest"].as_str().expect("capsule digest");
    let context_revision = data["contextRevision"].as_u64().expect("revision");

    // Resume no-change: the same cursor stays within 1 KiB and refreshes
    // the authority exactly once.
    let repeated = cli
        .call::<Value>(
            RpcMethod::OperationExecute,
            operation_request(
                &identity,
                "execution.resume",
                json!({
                    "knownCapsuleDigest": capsule_digest,
                    "knownContextRevision": context_revision,
                }),
                "resume-no-change",
            ),
        )
        .await
        .expect("repeated resume succeeds through the process");
    let no_change = &repeated["data"];
    assert_eq!(no_change["projectionKind"], "no-change", "{repeated}");
    assert_eq!(no_change["capsule"], Value::Null);
    assert_eq!(
        no_change["authorityRefreshCount"].as_u64(),
        Some(MAX_AUTHORITY_REFRESHES_PER_RESUME),
        "{repeated}"
    );
    let no_change_bytes = serde_json::to_vec(no_change).expect("no-change serializes");
    assert!(
        no_change_bytes.len() <= MAX_NO_CHANGE_BYTES,
        "no-change response exceeds the 1 KiB gate: {} bytes",
        no_change_bytes.len()
    );
    let no_change_result_bytes = serde_json::to_vec(&repeated).expect("result serializes");
    assert!(
        no_change_result_bytes.len() <= MAX_NO_CHANGE_BYTES,
        "whole no-change result exceeds the 1 KiB gate: {} bytes",
        no_change_result_bytes.len()
    );

    // A real host opens the turn with `hook.user_prompt`, which loads the
    // authoritative context projection (including the fresh passing guard)
    // before any tool event is adjudicated.
    inject_hook_guard(
        &fixture.project_root,
        &policy_digest,
        workspace.inventory_generation,
    );
    let prompt = await_guarded_prompt(&hook, &identity).await;
    assert_eq!(prompt["decision"], "context", "{prompt}");

    let mut script = HookScript::new(&hook, &identity);

    // Forbidden path: a broad verification before the focused GREEN.
    let broad = script.pre_tool("broad-early", broad_event()).await;
    assert_eq!(broad["decision"], "deny", "{broad}");
    assert_eq!(broad["executionDirective"]["decision"], "require-progress");
    assert_eq!(
        broad["executionDirective"]["reasonCode"],
        "EXECUTION_PROGRESS_REQUIRED"
    );
    let broad_before_green = 0_u32;

    // Focused RED: the first focused run is machine progress.
    let pre_red = script
        .pre_tool("focused-red-pre", focused_event(None))
        .await;
    assert_eq!(pre_red["decision"], "allow", "{pre_red}");
    let post_red = script
        .post_tool("focused-red-post", focused_event(Some("fail")))
        .await;
    assert_eq!(post_red["decision"], "allow", "{post_red}");

    // Minimal patch.
    let pre_patch = script.pre_tool("patch-pre", patch_event("patch/v1")).await;
    assert_eq!(pre_patch["decision"], "allow", "{pre_patch}");
    let post_patch = script
        .post_tool("patch-post", patch_event("patch/v1"))
        .await;
    assert_eq!(post_patch["decision"], "allow", "{post_patch}");
    let resume_to_first_patch_ms = started.elapsed().as_millis();

    // Focused GREEN.
    let pre_green = script
        .pre_tool("focused-green-pre", focused_event(None))
        .await;
    assert_eq!(pre_green["decision"], "allow", "{pre_green}");
    let post_green = script
        .post_tool("focused-green-post", focused_event(Some("pass")))
        .await;
    assert_eq!(post_green["decision"], "allow", "{post_green}");

    // The broad gate opens only after the focused GREEN.
    let broad_late = script.pre_tool("broad-late", broad_event()).await;
    assert_eq!(broad_late["decision"], "allow", "{broad_late}");
    assert_eq!(broad_late["executionDirective"]["decision"], "allow");
    assert_eq!(
        broad_late["executionDirective"]["outputBudgetBytes"],
        65_536
    );

    // Evidence: the supervised cadence for this slice is complete.
    let pre_evidence = script
        .pre_tool("evidence-pre", evidence_event("ledger/v1"))
        .await;
    assert_eq!(pre_evidence["decision"], "allow", "{pre_evidence}");
    let post_evidence = script
        .post_tool("evidence-post", evidence_event("ledger/v1"))
        .await;
    assert_eq!(post_evidence["decision"], "allow", "{post_evidence}");

    // Default budgets: 4 investigation calls per batch, 3 consecutive
    // no-progress batches; the 13th consecutive call is denied.
    let total_admissible = INSPECTION_CALLS_PER_BATCH * MAX_NO_PROGRESS_BATCHES;
    for index in 0..total_admissible {
        let pre = script
            .pre_tool_indexed("read-pre", index, read_event(index))
            .await;
        assert_eq!(pre["decision"], "allow", "read {index} pre: {pre}");
        let post = script
            .post_tool_indexed("read-post", index, read_event(index))
            .await;
        assert_eq!(post["decision"], "allow", "read {index} post: {post}");
    }
    let thirteenth = script
        .pre_tool("read-thirteenth", read_event(total_admissible))
        .await;
    assert_eq!(thirteenth["decision"], "deny", "{thirteenth}");
    assert_eq!(
        thirteenth["executionDirective"]["decision"],
        "require-progress"
    );
    assert_eq!(
        thirteenth["executionDirective"]["reasonCode"],
        "EXECUTION_PROGRESS_REQUIRED"
    );

    // Only progress-producing events stay admissible after exhaustion.
    let patch_after = script
        .pre_tool("patch-after-exhaustion", patch_event("patch/v2"))
        .await;
    assert_eq!(patch_after["decision"], "allow", "{patch_after}");

    // A later resume with the same cursor is still one bounded no-change.
    let final_resume = cli
        .call::<Value>(
            RpcMethod::OperationExecute,
            operation_request(
                &identity,
                "execution.resume",
                json!({
                    "knownCapsuleDigest": capsule_digest,
                    "knownContextRevision": context_revision,
                }),
                "resume-final",
            ),
        )
        .await
        .expect("final resume succeeds through the process");
    assert_eq!(final_resume["data"]["projectionKind"], "no-change");
    assert_eq!(
        final_resume["data"]["authorityRefreshCount"].as_u64(),
        Some(MAX_AUTHORITY_REFRESHES_PER_RESUME)
    );

    // The bounded durable events replay the exact supervised cadence —
    // classification and decisions only, never a tool output body.
    let status = cli
        .call::<Value>(RpcMethod::RuntimeStatus, params(json!({})))
        .await
        .expect("runtime status");
    let events = events(
        &cli,
        &identity,
        status["eventStoreId"].as_str().expect("store id"),
    )
    .await;
    let cadence: Vec<(&str, &str)> = events
        .iter()
        .filter(|event| event["kind"].as_str() == Some("execution.tool"))
        .map(|event| {
            (
                event["payload"]["class"].as_str().expect("class"),
                event["payload"]["decision"].as_str().expect("decision"),
            )
        })
        .collect();
    let mut expected: Vec<(&str, &str)> = vec![
        ("broad-test", "require-progress"),
        ("focused-test", "allow"),
        ("focused-test", "allow"),
        ("patch", "allow"),
        ("patch", "allow"),
        ("focused-test", "allow"),
        ("focused-test", "allow"),
        ("broad-test", "allow"),
        ("evidence", "allow"),
        ("evidence", "allow"),
    ];
    for _ in 0..total_admissible {
        expected.push(("source-read", "allow"));
        expected.push(("source-read", "allow"));
    }
    expected.push(("source-read", "deny"));
    expected.push(("patch", "allow"));
    assert_eq!(cadence, expected, "bounded execution cadence");

    assert_eq!(
        broad_before_green, 0,
        "no broad verification may execute before the focused GREEN"
    );
    assert!(
        resume_to_first_patch_ms <= MAX_RESUME_TO_FIRST_PATCH_MS,
        "resume-to-first-patch exceeds the 5 minute gate: {resume_to_first_patch_ms} ms"
    );
    daemon.crash();
}

struct Fixture {
    root: TempDir,
    project_root: PathBuf,
    runtime_dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("process fixture root");
        let project_root = root.path().join("project");
        let runtime_dir = root.path().join("runtime");
        fs::create_dir_all(&project_root).expect("project root");
        fs::create_dir_all(&runtime_dir).expect("runtime root");
        prepare_workspace(&project_root);
        Self {
            root,
            project_root,
            runtime_dir,
        }
    }

    fn allowed_root(&self) -> &Path {
        self.root.path()
    }

    fn execution_artifact(&self, name: &str) -> Vec<u8> {
        fs::read(self.project_root.join(EXECUTION_DIR).join(name))
            .expect("execution artifact bytes")
    }
}

fn prepare_workspace(root: &Path) {
    for directory in [
        root.join(".auto-engineering/exec-efficiency-process"),
        root.join("ae-sdd-doc/Story"),
        root.join("constraints"),
        root.join("source/standards/thinking"),
    ] {
        fs::create_dir_all(directory).expect("fixture directory");
    }
    fs::write(
        root.join(STORY_DOC),
        "# Story\n\nprocess e2e verification matrix\n",
    )
    .expect("Story fixture");
    fs::write(root.join("constraints/README.md"), "# constraints index\n")
        .expect("constraints fixture");
    fs::write(root.join(THINKING_ENGINE), "# coding thinking engine\n")
        .expect("thinking engine fixture");
    let story_absolute = root.join(STORY_DOC).to_string_lossy().into_owned();
    let state = json!({
        "stateMachineName": "PRD-EFF-PROCESS",
        "activeStory": WORK_ITEM_ID,
        "revision": 1,
        "lastFencingToken": 0,
        "scale": "large",
        "selectedDesign": "DR",
        "phase": "completed",
        "currentPhase": "completed",
        "documentPaths": {"STORY": STORY_DOC},
        "storyStates": {
            WORK_ITEM_ID: {
                "phase": "coding",
                "currentPhase": "coding",
                "docPath": story_absolute,
            }
        },
        "executionPlan": {
            "goal": "drive one supervised slice to completion in a real process",
            "changedPaths": ["bins/ae-sdd-daemon/tests/execution_efficiency_process.rs"],
            "verification": [
                {
                    "id": "V-EFF-010a",
                    "acId": "AC-007",
                    "boundary": "e2e",
                    "command": "cargo test -p ae-sdd-daemon --test execution_efficiency_process",
                    "expect": "process e2e completes inside the P0 budgets"
                }
            ],
            "risks": [],
            "sourceReads": ["constraints/README.md"],
            "approved": true,
            "approvedAt": "2026-07-27T01:00:00Z",
            "approvedBy": "user:test"
        },
    });
    let mut bytes = serde_json::to_vec_pretty(&state).expect("state serializes");
    bytes.push(b'\n');
    fs::write(
        root.join(".auto-engineering/exec-efficiency-process/state.json"),
        bytes,
    )
    .expect("state fixture");
}

/// Injects the fresh passing `hookGuard` the authoritative context loader
/// expects to find in project state, computed exactly the way the business
/// adapter recomputes it: same revision, daemon policy digest, current
/// inventory generation and the input fingerprint of the guard-free state.
fn inject_hook_guard(project_root: &Path, policy_digest: &str, inventory_generation: u64) {
    let state_path = project_root.join(".auto-engineering/exec-efficiency-process/state.json");
    let state: Value =
        serde_json::from_slice(&fs::read(&state_path).expect("state bytes")).expect("state JSON");
    let fingerprint =
        InputFingerprint::digest(serde_json::to_vec(&state).expect("state serializes")).to_string();
    let revision = state["revision"].as_u64().expect("state revision");
    let mut guarded = state;
    guarded["hookGuard"] = json!({
        "outcome": "PASS",
        "stateRevision": revision,
        "policyDigest": policy_digest,
        "inventoryGeneration": inventory_generation,
        "inputFingerprint": fingerprint,
    });
    let mut bytes = serde_json::to_vec_pretty(&guarded).expect("state serializes");
    bytes.push(b'\n');
    fs::write(&state_path, bytes).expect("guarded state fixture");
}

struct DaemonProcess {
    child: Child,
}

impl DaemonProcess {
    async fn start(state_dir: &Path, allowed_root: &Path, policy_digest: &str) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ae-sddd"));
        command
            .arg("serve")
            .arg("--state-dir")
            .arg(state_dir)
            .arg("--allowed-root")
            .arg(allowed_root)
            .arg("--policy-digest")
            .arg(policy_digest)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = command.spawn().expect("real ae-sddd process starts");
        let manifest = state_dir.join("endpoint.v1.json");
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(status) = child.try_wait().expect("daemon status is readable") {
                panic!("ae-sddd exited before ready ({status})");
            }
            if manifest.is_file() {
                let client = daemon_client(state_dir, ClientKind::Cli);
                if client
                    .call::<Value>(RpcMethod::RuntimeStatus, params(json!({})))
                    .await
                    .is_ok()
                {
                    return Self { child };
                }
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("ae-sddd did not become ready");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    fn crash(&mut self) {
        if self
            .child
            .try_wait()
            .expect("daemon status is readable")
            .is_none()
        {
            self.child
                .kill()
                .expect("daemon process is force-terminated");
            self.child.wait().expect("daemon process exits after kill");
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

#[derive(Clone)]
struct Identity {
    workspace_id: String,
    session_id: String,
    capability_token: String,
}

impl Identity {
    fn new(workspace: &WorkspaceResult, session: &SessionResult) -> Self {
        Self {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session.session_id.clone(),
            capability_token: session.capability_token.clone(),
        }
    }

    fn params(&self, payload: Value) -> RequestParams<Value> {
        let mut request = params(payload);
        request.workspace_id = Some(self.workspace_id.clone());
        request.work_item_id = Some(WORK_ITEM_ID.to_owned());
        request.session_id = Some(self.session_id.clone());
        request.agent_id = Some(AGENT_ID.to_owned());
        request.capability_token = Some(self.capability_token.clone());
        request
    }
}

/// Drives the Hook fast path with unique event identities for one session.
struct HookScript<'a> {
    client: &'a DaemonClient,
    identity: &'a Identity,
    sequence: u32,
}

impl<'a> HookScript<'a> {
    fn new(client: &'a DaemonClient, identity: &'a Identity) -> Self {
        Self {
            client,
            identity,
            sequence: 0,
        }
    }

    async fn pre_tool(&mut self, label: &str, execution_event: Value) -> Value {
        self.call(RpcMethod::HookPreTool, label, 0, execution_event)
            .await
    }

    async fn post_tool(&mut self, label: &str, execution_event: Value) -> Value {
        self.call(RpcMethod::HookPostTool, label, 0, execution_event)
            .await
    }

    async fn pre_tool_indexed(&mut self, label: &str, index: u32, event: Value) -> Value {
        self.call(RpcMethod::HookPreTool, label, index + 1, event)
            .await
    }

    async fn post_tool_indexed(&mut self, label: &str, index: u32, event: Value) -> Value {
        self.call(RpcMethod::HookPostTool, label, index + 1, event)
            .await
    }

    async fn call(
        &mut self,
        method: RpcMethod,
        label: &str,
        index: u32,
        execution_event: Value,
    ) -> Value {
        self.sequence += 1;
        let event_id = format!("hook-{:03}-{}", self.sequence, label.replace('_', "-"));
        let event_id = if index == 0 {
            event_id
        } else {
            format!("{event_id}-{index}")
        };
        hook_call(
            self.client,
            self.identity,
            &event_id,
            method,
            json!({"executionEvent": execution_event}),
        )
        .await
    }
}

/// Opens the turn with `hook.user_prompt` and waits until the cached context
/// projection actually carries the freshly injected passing `hookGuard`.
///
/// `hook_projection` serves the cache without recomputing, and the daemon's
/// context worker only rebuilds it on its own interval. Adjudicating a PreTool
/// event before that rebuild lands reads a guard-free projection, which fails
/// closed — so the assertion under test would depend on the refresh window
/// rather than on the supervisor rule it means to prove. Each attempt needs a
/// distinct `hookEventId`; a repeated identity would replay the first receipt.
async fn await_guarded_prompt(client: &DaemonClient, identity: &Identity) -> Value {
    const MAX_ATTEMPTS: u32 = 200;
    let mut last = Value::Null;
    for attempt in 0..MAX_ATTEMPTS {
        last = hook_call(
            client,
            identity,
            &format!("hook-000-user-prompt-{attempt}"),
            RpcMethod::HookUserPrompt,
            json!({}),
        )
        .await;
        if last["context"]["hookGuard"]["outcome"] == "PASS" {
            return last;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("context projection never published the injected passing guard: {last}");
}

async fn hook_call(
    client: &DaemonClient,
    identity: &Identity,
    event_id: &str,
    method: RpcMethod,
    host_payload: Value,
) -> Value {
    let mut request = identity.params(json!({
        "hookEventId": event_id,
        "turnSeq": 1,
        "hostPayload": host_payload,
    }));
    request.turn_id = Some("turn".to_owned());
    request.idempotency_key = Some(format!("request-{event_id}"));
    request.deadline_ms = HOOK_DEADLINE_MS;
    client
        .call::<Value>(method, request)
        .await
        .unwrap_or_else(|error| panic!("{event_id} reaches the daemon: {error:?}"))
}

fn digest(label: &str) -> String {
    ArtifactDigest::digest(label.as_bytes()).to_string()
}

fn focused_event(outcome: Option<&str>) -> Value {
    let mut event = json!({"class": "focused-test", "outputBytes": 256});
    if let Some(outcome) = outcome {
        event["outcome"] = Value::String(outcome.to_owned());
        event["outputDigest"] = Value::String(digest("focused-output"));
    }
    event
}

fn patch_event(label: &str) -> Value {
    json!({
        "class": "patch",
        "resultDigest": digest(label),
        "outputBytes": 64,
        "outputDigest": digest(&format!("{label}-output")),
    })
}

fn broad_event() -> Value {
    json!({"class": "broad-test", "outputBytes": 512})
}

fn evidence_event(label: &str) -> Value {
    json!({
        "class": "evidence",
        "eventDigest": digest(label),
        "outputBytes": 64,
    })
}

fn read_event(index: u32) -> Value {
    json!({
        "class": "source-read",
        "path": format!("src/module-{index}.rs"),
        "contentDigest": digest(&format!("source-body-{index}")),
        "startLine": 1,
        "endLine": 24,
        "outputBytes": 128,
        "outputDigest": digest(&format!("read-output-{index}")),
    })
}

async fn register_and_cut_over(
    cli: &DaemonClient,
    admin: &DaemonClient,
    fixture: &Fixture,
) -> WorkspaceResult {
    let mut register = params(json!({
        "projectRoot": fixture.project_root.to_string_lossy(),
        "projectKey": PROJECT_KEY,
    }));
    register.idempotency_key = Some("workspace-register-efficiency".to_owned());
    let registered = cli
        .call::<WorkspaceResult>(RpcMethod::WorkspaceRegister, register)
        .await
        .expect("workspace registers through the process");
    assert_eq!(registered.mode, WorkspaceMode::Shadow);

    let mut drain = params(json!({"stop": false}));
    drain.idempotency_key = Some("runtime-drain-efficiency".to_owned());
    drain.confirmation = Some(confirmation("runtime-drain-efficiency"));
    admin
        .call::<Value>(RpcMethod::RuntimeDrain, drain)
        .await
        .expect("admin drains the runtime for cutover");

    let parity = WorkspaceParityEvidence {
        comparison_count: 1,
        mismatch_count: 0,
        source_revision: 1,
        legacy_digest: "a".repeat(64),
        rust_digest: "a".repeat(64),
        observed_at_unix_ms: now_unix_ms(),
    };
    let parity_digest =
        InputFingerprint::digest(serde_json::to_vec(&parity).expect("parity evidence serializes"))
            .to_string();
    let mut transition = params(json!({
        "targetMode": WorkspaceMode::RustCanary,
        "reason": "execution efficiency process E2E parity fixture",
        "parityDigest": parity_digest,
        "parity": parity,
    }));
    transition.workspace_id = Some(registered.workspace_id);
    transition.idempotency_key = Some("workspace-cutover-efficiency".to_owned());
    transition.confirmation = Some(confirmation("workspace-cutover-efficiency"));
    let cut_over = admin
        .call::<WorkspaceResult>(RpcMethod::WorkspaceModeTransition, transition)
        .await
        .expect("workspace enters Rust canary mode");
    assert_eq!(cut_over.mode, WorkspaceMode::RustCanary);
    cut_over
}

async fn open_root(
    client: &DaemonClient,
    workspace: &WorkspaceResult,
    idempotency_key: &str,
) -> SessionResult {
    let mut request = params(json!({
        "externalKey": EXTERNAL_SESSION_KEY,
        "role": "root",
        "engaged": true,
    }));
    request.workspace_id = Some(workspace.workspace_id.clone());
    request.work_item_id = Some(WORK_ITEM_ID.to_owned());
    request.agent_id = Some(AGENT_ID.to_owned());
    request.idempotency_key = Some(idempotency_key.to_owned());
    client
        .call(RpcMethod::SessionOpen, request)
        .await
        .expect("root session opens through the process")
}

fn operation_request(
    identity: &Identity,
    operation: &str,
    payload: Value,
    idempotency_key: &str,
) -> RequestParams<Value> {
    let mut request = identity.params(json!({"operation": operation, "payload": payload}));
    request.idempotency_key = Some(idempotency_key.to_owned());
    request
}

async fn events(client: &DaemonClient, identity: &Identity, event_store_id: &str) -> Vec<Value> {
    let response: Value = client
        .call(
            RpcMethod::EventsSubscribe,
            identity.params(json!({
                "eventStoreId": event_store_id,
                "afterEventSeq": 0,
                "limit": 256,
            })),
        )
        .await
        .expect("events are readable");
    response["events"].as_array().expect("event batch").clone()
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
        approved_at: "2026-07-26T00:00:00Z".to_owned(),
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time follows Unix epoch")
        .as_millis()
        .try_into()
        .expect("current timestamp fits u64")
}
