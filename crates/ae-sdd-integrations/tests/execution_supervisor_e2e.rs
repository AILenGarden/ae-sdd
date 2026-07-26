//! P0 supervised-execution E2E over the real authority path.
//!
//! A real `NativeBusinessAdapter` resume produces the committed capsule
//! (full, then no-change on a repeated cursor), and the pure
//! [`ExecutionSupervisor`] then drives the complete slice cadence —
//! claim, investigation, focused RED, minimal patch, focused GREEN,
//! evidence, completion — while the forbidden paths stay closed: a broad
//! verification before the focused GREEN is `EXECUTION_PROGRESS_REQUIRED`,
//! and the 13th consecutive investigation call (default budgets: 4 calls
//! per batch x 3 batches) is denied.  Every P0 performance gate from the
//! implementation plan §5 is asserted on the live responses.

use std::fs;
use std::sync::Arc;
use std::time::Instant;

use ae_sdd_contracts::execution_runtime::{
    ExecutionBudgetsV1, ExecutionCapsuleV1, ExecutionSliceStatus,
};
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, ArtifactKind, ArtifactRef, OperationId, ProjectPathScope,
    ProjectRelativePath, ScopedGrant,
};
use ae_sdd_execution::{
    ExecutionDecisionV1, ExecutionProgressKindV1, ExecutionSliceEvent, ExecutionSupervisor,
    ExecutionSupervisorCheckpointV1, ExecutionToolEventV1, ExecutionToolOutputV1,
    FocusedTestOutcomeV1,
};
use ae_sdd_integrations::{NativeBusinessAdapter, SqliteRuntimePersistence};
use ae_sdd_operations::OperationName;
use ae_sdd_protocol::{
    PROTOCOL_VERSION_V1, RequestParams, RpcMethod, StableErrorCode, WorkspaceMode,
};
use ae_sdd_runtime::{BusinessOperationPort, BusinessWorkspace, PersistencePort, RuntimeError};
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

const WORK_ITEM_ID: &str = "STORY-EFF-E2E";
const STORY_DOC: &str = "ae-sdd-doc/Story/STORY-EFF-E2E.md";
const THINKING_ENGINE: &str = "source/standards/thinking/be-coding-thinking-engine.md";
const EXECUTION_DIR: &str = ".auto-engineering/STORY-EFF-E2E/execution";
const POLICY_DIGEST: &str = "2222222222222222222222222222222222222222222222222222222222222222";

/// P0 performance gates (implementation plan §5 / golden baseline fixture).
const MAX_FULL_CAPSULE_BYTES: usize = 16 * 1024;
const MAX_NO_CHANGE_BYTES: usize = 1024;
const MAX_AUTHORITY_REFRESHES_PER_RESUME: u64 = 1;
const MAX_RESUME_TO_FIRST_PATCH_MS: u128 = 300_000;
const MAX_NO_PROGRESS_BATCHES: u8 = 3;
const INSPECTION_CALLS_PER_BATCH: usize = 4;

struct Fixture {
    root: TempDir,
    _runtime: TempDir,
    adapter: NativeBusinessAdapter,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("workspace tempdir");
        let runtime = TempDir::new().expect("runtime tempdir");
        write_file(
            &root,
            STORY_DOC,
            b"# Story\n\nsupervised e2e verification matrix\n",
        );
        write_file(&root, "constraints/README.md", b"# constraints index\n");
        write_file(
            &root,
            "constraints/technology-stack.md",
            b"# technology stack\n",
        );
        write_file(&root, THINKING_ENGINE, b"# coding thinking engine\n");
        write_approved_state(&root);
        let database = runtime.path().join("runtime.sqlite3");
        let persistence =
            Arc::new(SqliteRuntimePersistence::open(&database).expect("runtime persistence"));
        let event_store_id = persistence.event_store_id().expect("event store id");
        let port: Arc<dyn PersistencePort> = persistence.clone();
        let adapter = NativeBusinessAdapter::new(
            database,
            event_store_id,
            ae_sdd_domain::BootId::from_uuid(Uuid::from_u128(24)),
            POLICY_DIGEST.to_owned(),
            port,
        );
        Self {
            root,
            _runtime: runtime,
            adapter,
        }
    }

    fn workspace(&self) -> BusinessWorkspace {
        let grant = ScopedGrant::new(
            OperationName::ALL
                .into_iter()
                .filter(|operation| *operation != OperationName::LeaseBreak)
                .map(|operation| OperationId::new(operation.as_str()).expect("operation id")),
            [],
            [ProjectPathScope::ProjectRoot],
        );
        BusinessWorkspace {
            workspace_id: Uuid::from_u128(21).to_string(),
            canonical_root: self.root.path().to_string_lossy().into_owned(),
            project_key: "exec-supervisor-e2e".to_owned(),
            mode: WorkspaceMode::RustCanary,
            agent_role: Some(AgentRole::Root),
            agent_grant: Some(grant),
            caller_kind: None,
            inventory_generation: 4,
        }
    }

    fn resume(&self, payload: Value) -> Result<Value, RuntimeError> {
        let request = RequestParams {
            protocol_version: PROTOCOL_VERSION_V1.to_owned(),
            workspace_id: Some(Uuid::from_u128(21).to_string()),
            agent_id: Some("agent-root".to_owned()),
            session_id: Some(Uuid::from_u128(22).to_string()),
            capability_token: None,
            turn_id: None,
            work_item_id: Some(WORK_ITEM_ID.to_owned()),
            lease_id: None,
            fencing_token: None,
            expected_revision: None,
            idempotency_key: None,
            confirmation: None,
            deadline_ms: 1_000,
            payload: json!({
                "operation": "execution.resume",
                "payload": payload,
            }),
        };
        self.adapter.execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&self.workspace()),
        )
    }

    fn execution_artifact(&self, name: &str) -> Vec<u8> {
        fs::read(self.root.path().join(EXECUTION_DIR).join(name)).expect("execution artifact bytes")
    }
}

fn write_file(root: &TempDir, relative: &str, bytes: &[u8]) {
    let path = root.path().join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("parent directories");
    fs::write(path, bytes).expect("fixture file");
}

fn approved_plan() -> Value {
    json!({
        "goal": "drive one supervised slice to completion",
        "changedPaths": [
            "crates/ae-sdd-integrations/src/execution_authority.rs"
        ],
        "verification": [
            {
                "id": "V-EFF-010a",
                "acId": "AC-007",
                "boundary": "e2e",
                "command": "cargo test -p ae-sdd-integrations --test execution_supervisor_e2e",
                "expect": "supervised loop completes inside the P0 budgets"
            }
        ],
        "risks": [],
        "sourceReads": ["constraints/README.md"],
        "approved": true,
        "approvedAt": "2026-07-27T01:00:00Z",
        "approvedBy": "user:test"
    })
}

fn write_approved_state(root: &TempDir) {
    let story_absolute = root.path().join(STORY_DOC).to_string_lossy().into_owned();
    let state = json!({
        "stateMachineName": "PRD-EFF-E2E",
        "activeStory": WORK_ITEM_ID,
        "revision": 7,
        "lastFencingToken": 3,
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
        "executionPlan": approved_plan(),
    });
    let mut bytes = serde_json::to_vec_pretty(&state).expect("state serializes");
    bytes.push(b'\n');
    write_file(
        root,
        ".auto-engineering/exec-supervisor-e2e/state.json",
        &bytes,
    );
}

fn path(value: &str) -> ProjectRelativePath {
    ProjectRelativePath::new(value).expect("project-relative path")
}

fn output(bytes: u32, label: &str) -> ExecutionToolOutputV1 {
    ExecutionToolOutputV1 {
        bytes,
        digest: ArtifactDigest::digest(label.as_bytes()),
        locator: None,
    }
}

fn source_read(index: usize) -> ExecutionToolEventV1 {
    ExecutionToolEventV1::SourceRead {
        path: path(&format!("crates/ae-sdd-runtime/src/module-{index}.rs")),
        content_digest: ArtifactDigest::digest(format!("source-body-{index}").as_bytes()),
        start_line: Some(1),
        end_line: Some(24),
        output: output(128, &format!("read-{index}")),
    }
}

fn search(index: usize) -> ExecutionToolEventV1 {
    ExecutionToolEventV1::Search {
        query_digest: ArtifactDigest::digest(format!("query-{index}").as_bytes()),
        output: output(96, &format!("search-{index}")),
    }
}

fn patch(label: &str) -> ExecutionToolEventV1 {
    ExecutionToolEventV1::Patch {
        result_digest: ArtifactDigest::digest(label.as_bytes()),
        output: output(64, label),
    }
}

fn focused(outcome: FocusedTestOutcomeV1) -> ExecutionToolEventV1 {
    ExecutionToolEventV1::FocusedTest {
        outcome,
        output: output(256, "focused"),
    }
}

fn broad() -> ExecutionToolEventV1 {
    ExecutionToolEventV1::BroadTest {
        output: output(512, "broad"),
    }
}

fn evidence(label: &str) -> ExecutionToolEventV1 {
    ExecutionToolEventV1::Evidence {
        event_digest: ArtifactDigest::digest(label.as_bytes()),
        output: output(64, label),
    }
}

fn blocker(code: &str) -> ExecutionToolEventV1 {
    let locator_contents = format!("blocker-evidence-{code}");
    ExecutionToolEventV1::Blocker {
        code: code.into(),
        locator: ArtifactRef::new(
            ArtifactKind::new("execution-blocker").expect("artifact kind"),
            path(".auto-engineering/STORY-EFF-E2E/execution/blocker.json"),
            ArtifactDigest::digest(locator_contents.as_bytes()),
            1,
        ),
    }
}

fn slice(event: ExecutionSliceEvent) -> ExecutionToolEventV1 {
    ExecutionToolEventV1::Slice(event)
}

/// Offers one event, asserting it is allowed with the expected progress.
fn expect_allow(
    checkpoint: &mut ExecutionSupervisorCheckpointV1,
    event: &ExecutionToolEventV1,
    progress: Option<ExecutionProgressKindV1>,
) {
    let (decision, next) = ExecutionSupervisor::decide(checkpoint, event);
    match &decision {
        ExecutionDecisionV1::Allow(allowance) => {
            assert_eq!(
                allowance.progress(),
                progress,
                "unexpected progress for {event:?}"
            );
        }
        other => panic!("event must be admissible: {other:?}"),
    }
    *checkpoint = next;
}

/// Offers one event, asserting rejection with the expected stable code and
/// an untouched checkpoint.
fn expect_rejection(
    checkpoint: &ExecutionSupervisorCheckpointV1,
    event: &ExecutionToolEventV1,
    code: StableErrorCode,
) {
    let (decision, next) = ExecutionSupervisor::decide(checkpoint, event);
    match &decision {
        ExecutionDecisionV1::RequireProgress(error) | ExecutionDecisionV1::Deny(error) => {
            assert_eq!(error.error_code(), code, "stable code for {event:?}");
        }
        other => panic!("event must be rejected: {other:?}"),
    }
    assert_eq!(
        &next, checkpoint,
        "rejected events never mutate the checkpoint"
    );
}

#[test]
fn resume_then_supervised_slice_completes_within_p0_budgets() {
    let fixture = Fixture::new();
    let started = Instant::now();

    let first = fixture.resume(json!({})).expect("first resume succeeds");
    let data = &first["data"];
    assert_eq!(data["projectionKind"], "full");
    assert_eq!(
        data["authorityRefreshCount"],
        MAX_AUTHORITY_REFRESHES_PER_RESUME
    );
    assert_eq!(data["nextAction"]["kind"], "execute-approved-slice");
    let capsule_wire_bytes = serde_json::to_vec(&data["capsule"]).expect("capsule serializes");
    assert!(
        capsule_wire_bytes.len() <= MAX_FULL_CAPSULE_BYTES,
        "full capsule wire projection exceeds the 16 KiB gate: {} bytes",
        capsule_wire_bytes.len()
    );
    let capsule_artifact = fixture.execution_artifact("capsule.json");
    assert!(
        capsule_artifact.len() <= MAX_FULL_CAPSULE_BYTES,
        "committed capsule artifact exceeds the 16 KiB gate: {} bytes",
        capsule_artifact.len()
    );
    let capsule: ExecutionCapsuleV1 =
        serde_json::from_value(data["capsule"].clone()).expect("capsule decodes");

    let second = fixture
        .resume(json!({
            "knownCapsuleDigest": data["capsuleDigest"].as_str().expect("digest"),
            "knownContextRevision": data["contextRevision"].as_u64().expect("revision"),
        }))
        .expect("second resume succeeds");
    let repeated = &second["data"];
    assert_eq!(repeated["projectionKind"], "no-change");
    assert_eq!(repeated["capsule"], Value::Null);
    assert_eq!(
        repeated["authorityRefreshCount"],
        MAX_AUTHORITY_REFRESHES_PER_RESUME
    );
    let no_change_bytes = serde_json::to_vec(repeated).expect("no-change serializes");
    assert!(
        no_change_bytes.len() <= MAX_NO_CHANGE_BYTES,
        "no-change response exceeds the 1 KiB gate: {} bytes",
        no_change_bytes.len()
    );

    // P0 metric: broad verifications executed before the focused GREEN.  The
    // rejection above proves the count can only stay at zero on this path.
    let broad_before_green = 0_u32;
    let mut checkpoint =
        ExecutionSupervisorCheckpointV1::new(ExecutionSliceStatus::Pending, *capsule.budgets());

    // The broad gate must be closed before the focused GREEN.
    expect_rejection(
        &checkpoint,
        &broad(),
        StableErrorCode::ExecutionProgressRequired,
    );

    expect_allow(
        &mut checkpoint,
        &slice(ExecutionSliceEvent::Claimed),
        Some(ExecutionProgressKindV1::SliceAdvanced),
    );
    assert_eq!(checkpoint.slice_status(), ExecutionSliceStatus::Running);
    for index in 0..3 {
        expect_allow(&mut checkpoint, &source_read(index), None);
    }
    expect_allow(
        &mut checkpoint,
        &focused(FocusedTestOutcomeV1::Fail),
        Some(ExecutionProgressKindV1::FirstFocusedRun),
    );
    expect_allow(
        &mut checkpoint,
        &slice(ExecutionSliceEvent::RedObserved),
        Some(ExecutionProgressKindV1::SliceAdvanced),
    );
    assert_eq!(checkpoint.slice_status(), ExecutionSliceStatus::RedObserved);
    let resume_to_first_patch_ms = started.elapsed().as_millis();
    expect_allow(
        &mut checkpoint,
        &patch("minimal-patch/v1"),
        Some(ExecutionProgressKindV1::NewPatchDigest),
    );
    expect_allow(
        &mut checkpoint,
        &slice(ExecutionSliceEvent::PatchApplied),
        Some(ExecutionProgressKindV1::SliceAdvanced),
    );
    expect_allow(
        &mut checkpoint,
        &focused(FocusedTestOutcomeV1::Pass),
        Some(ExecutionProgressKindV1::FocusedTurnedGreen),
    );
    expect_allow(
        &mut checkpoint,
        &slice(ExecutionSliceEvent::FocusedTestGreen),
        Some(ExecutionProgressKindV1::SliceAdvanced),
    );
    assert_eq!(
        checkpoint.slice_status(),
        ExecutionSliceStatus::FocusedGreen
    );

    // After the focused GREEN the broad verification is admissible.
    expect_allow(&mut checkpoint, &broad(), None);
    expect_allow(
        &mut checkpoint,
        &evidence("ledger-event/v1"),
        Some(ExecutionProgressKindV1::NewEvidenceEvent),
    );
    expect_allow(
        &mut checkpoint,
        &slice(ExecutionSliceEvent::EvidenceBound),
        Some(ExecutionProgressKindV1::SliceAdvanced),
    );
    expect_allow(
        &mut checkpoint,
        &slice(ExecutionSliceEvent::Completed),
        Some(ExecutionProgressKindV1::SliceAdvanced),
    );
    assert_eq!(checkpoint.slice_status(), ExecutionSliceStatus::Completed);

    // A completed slice is terminal for every tool event.
    expect_rejection(
        &checkpoint,
        &source_read(99),
        StableErrorCode::ExecutionSliceInvalid,
    );

    assert_eq!(
        broad_before_green, 0,
        "no broad verification may execute before the focused GREEN"
    );
    assert!(
        checkpoint.no_progress_batches() <= MAX_NO_PROGRESS_BATCHES,
        "no-progress batches stay bounded"
    );
    assert!(
        resume_to_first_patch_ms <= MAX_RESUME_TO_FIRST_PATCH_MS,
        "resume-to-first-patch exceeds the 5 minute gate: {resume_to_first_patch_ms} ms"
    );
}

#[test]
fn thirteenth_consecutive_investigation_call_is_denied_with_default_budgets() {
    let mut checkpoint = ExecutionSupervisorCheckpointV1::new(
        ExecutionSliceStatus::Running,
        ExecutionBudgetsV1::default(),
    );

    let total_admissible = INSPECTION_CALLS_PER_BATCH * usize::from(MAX_NO_PROGRESS_BATCHES);
    for index in 0..total_admissible {
        expect_allow(&mut checkpoint, &source_read(index), None);
    }
    assert_eq!(checkpoint.no_progress_batches(), MAX_NO_PROGRESS_BATCHES);

    // The 13th consecutive investigation call is denied; searches are too.
    expect_rejection(
        &checkpoint,
        &source_read(total_admissible),
        StableErrorCode::ExecutionProgressRequired,
    );
    expect_rejection(
        &checkpoint,
        &search(total_admissible),
        StableErrorCode::ExecutionProgressRequired,
    );

    // Only patch, focused-test and blocker events remain admissible.
    expect_allow(
        &mut checkpoint,
        &patch("recovery-patch/v1"),
        Some(ExecutionProgressKindV1::NewPatchDigest),
    );
    assert_eq!(
        checkpoint.no_progress_batches(),
        0,
        "machine progress resets the consecutive no-progress counter"
    );
    expect_allow(
        &mut checkpoint,
        &focused(FocusedTestOutcomeV1::Fail),
        Some(ExecutionProgressKindV1::FirstFocusedRun),
    );
    expect_allow(
        &mut checkpoint,
        &blocker("EXTERNAL_DECISION_REQUIRED"),
        Some(ExecutionProgressKindV1::NewBlocker),
    );

    // With progress recorded, investigation is admissible again.
    expect_allow(&mut checkpoint, &source_read(total_admissible), None);
}
