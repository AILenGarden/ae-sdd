//! Authoritative `execution.resume` contract tests.
//!
//! One resume call performs a single project-authority snapshot and a single
//! required-context bundle load, verifies the approved plan and the four
//! required contexts (story, constraints, thinking engine, verification),
//! seeds queue/ledger/capsule/state locator atomically on first generation,
//! and fails closed with `EXECUTION_CAPSULE_STALE` on an unapproved plan or
//! any digest drift without writing an artifact.

use std::fs;
use std::sync::Arc;

use ae_sdd_domain::{AgentRole, ArtifactDigest, OperationId, ProjectPathScope, ScopedGrant};
use ae_sdd_integrations::{NativeBusinessAdapter, SqliteRuntimePersistence};
use ae_sdd_operations::OperationName;
use ae_sdd_protocol::{
    PROTOCOL_VERSION_V1, RequestParams, RpcMethod, StableErrorCode, WorkspaceMode,
};
use ae_sdd_runtime::{BusinessOperationPort, BusinessWorkspace, PersistencePort, RuntimeError};
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

const WORK_ITEM_ID: &str = "STORY-EFF-001";
const STORY_DOC: &str = "ae-sdd-doc/Story/STORY-EFF-001.md";
const THINKING_ENGINE: &str = "source/standards/thinking/be-coding-thinking-engine.md";
const EXECUTION_DIR: &str = ".auto-engineering/STORY-EFF-001/execution";
const POLICY_DIGEST: &str = "1111111111111111111111111111111111111111111111111111111111111111";

struct Fixture {
    root: TempDir,
    _runtime: TempDir,
    adapter: NativeBusinessAdapter,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("workspace tempdir");
        let runtime = TempDir::new().expect("runtime tempdir");
        write_file(&root, STORY_DOC, b"# Story\n\nverification matrix\n");
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
            ae_sdd_domain::BootId::from_uuid(Uuid::from_u128(14)),
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
            workspace_id: Uuid::from_u128(11).to_string(),
            canonical_root: self.root.path().to_string_lossy().into_owned(),
            project_key: "exec-capsule".to_owned(),
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
            workspace_id: Some(Uuid::from_u128(11).to_string()),
            agent_id: Some("agent-root".to_owned()),
            session_id: Some(Uuid::from_u128(12).to_string()),
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

    fn state(&self) -> Value {
        let bytes = fs::read(
            self.root
                .path()
                .join(".auto-engineering/exec-capsule/state.json"),
        )
        .expect("state bytes");
        serde_json::from_slice(&bytes).expect("state JSON")
    }

    fn state_bytes(&self) -> Vec<u8> {
        fs::read(
            self.root
                .path()
                .join(".auto-engineering/exec-capsule/state.json"),
        )
        .expect("state bytes")
    }

    fn execution_artifact(&self, name: &str) -> Vec<u8> {
        fs::read(self.root.path().join(EXECUTION_DIR).join(name)).expect("execution artifact bytes")
    }

    fn rewrite_state(&self, mutate: impl FnOnce(&mut Value)) {
        let mut state = self.state();
        mutate(&mut state);
        let revision = state["revision"].as_u64().expect("revision") + 1;
        state["revision"] = json!(revision);
        let mut bytes = serde_json::to_vec_pretty(&state).expect("state serializes");
        bytes.push(b'\n');
        fs::write(
            self.root
                .path()
                .join(".auto-engineering/exec-capsule/state.json"),
            bytes,
        )
        .expect("rewrite state");
    }
}

fn write_file(root: &TempDir, relative: &str, bytes: &[u8]) {
    let path = root.path().join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("parent directories");
    fs::write(path, bytes).expect("fixture file");
}

fn approved_plan() -> Value {
    json!({
        "goal": "deliver a bounded deterministic resume",
        "changedPaths": [
            "crates/ae-sdd-integrations/src/business.rs",
            "crates/ae-sdd-integrations/src/execution_authority.rs"
        ],
        "verification": [
            {
                "id": "V-EFF-003",
                "acId": "AC-003",
                "boundary": "authority",
                "command": "cargo test -p ae-sdd-integrations --test execution_capsule_authority",
                "expect": "unapproved plan and drift fail closed; repeated resume is no-change"
            },
            {
                "id": "V-EFF-001b",
                "acId": "AC-001",
                "boundary": "contract",
                "command": "cargo test -p ae-sdd-contracts --test execution_capsule_contract",
                "expect": "contract round trip passes"
            }
        ],
        "risks": [],
        "sourceReads": [
            "constraints/README.md",
            "crates/ae-sdd-integrations/src/business.rs"
        ],
        "approved": true,
        "approvedAt": "2026-07-27T01:00:00Z",
        "approvedBy": "user:test"
    })
}

fn write_approved_state(root: &TempDir) {
    let story_absolute = root.path().join(STORY_DOC).to_string_lossy().into_owned();
    let state = json!({
        "stateMachineName": "PRD-EFF-001",
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
    write_file(root, ".auto-engineering/exec-capsule/state.json", &bytes);
}

/// Plain 64-hex digest, the form typed contracts serialize.
fn plain_digest(bytes: &[u8]) -> String {
    ArtifactDigest::digest(bytes).to_string()
}

/// `sha256:`-prefixed digest, the form project state locators and the resume
/// response use.
fn prefixed_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", plain_digest(bytes))
}

#[test]
fn first_resume_seeds_project_authority_and_returns_the_full_capsule() {
    let fixture = Fixture::new();

    let response = fixture.resume(json!({})).expect("first resume succeeds");

    assert_eq!(response["changed"], false);
    let data = &response["data"];
    assert_eq!(data["projectionKind"], "full");
    assert_eq!(data["authorityRefreshCount"], 1);
    assert_eq!(data["capsule"]["schemaVersion"], "v1");
    assert_eq!(data["capsule"]["workItemId"], WORK_ITEM_ID);
    assert_eq!(data["capsule"]["storyId"], WORK_ITEM_ID);
    assert_eq!(data["capsule"]["queue"]["totalSlices"], 2);
    assert_eq!(data["capsule"]["queue"]["activeOrdinal"], 1);
    assert_eq!(
        data["capsule"]["activeSlice"]["focusedVerificationId"],
        "V-EFF-003"
    );
    assert_eq!(
        data["nextAction"],
        json!({
            "kind": "execute-approved-slice",
            "activeOrdinal": 1,
            "queueDigest": data["capsule"]["queue"]["queueDigest"],
        })
    );
    let capsule_bytes = fixture.execution_artifact("capsule.json");
    assert_eq!(data["capsuleDigest"], prefixed_digest(&capsule_bytes));
    let queue_bytes = fixture.execution_artifact("queue.json");
    let ledger_bytes = fixture.execution_artifact("ledger.jsonl");
    assert_eq!(
        data["capsule"]["queue"]["queueDigest"],
        plain_digest(&queue_bytes)
    );

    let state = fixture.state();
    assert_eq!(state["revision"], 8);
    let runtime = &state["executionRuntime"];
    assert_eq!(runtime["schemaVersion"], 1);
    assert_eq!(
        runtime["capsuleRef"],
        format!("{EXECUTION_DIR}/capsule.json")
    );
    assert_eq!(runtime["capsuleDigest"], prefixed_digest(&capsule_bytes));
    assert_eq!(runtime["queueRef"], format!("{EXECUTION_DIR}/queue.json"));
    assert_eq!(runtime["queueDigest"], prefixed_digest(&queue_bytes));
    assert_eq!(
        runtime["ledgerRef"],
        format!("{EXECUTION_DIR}/ledger.jsonl")
    );
    assert_eq!(runtime["ledgerDigest"], prefixed_digest(&ledger_bytes));
    assert_eq!(runtime["activeSliceOrdinal"], 1);
    assert_eq!(runtime["completionMilestone"], "none");

    let plan_bytes = serde_json::to_vec(&approved_plan()).expect("canonical plan");
    assert_eq!(
        data["capsule"]["approvedPlanDigest"],
        plain_digest(&plan_bytes)
    );
}

#[test]
fn repeated_resume_with_the_known_digest_is_no_change_and_writes_nothing() {
    let fixture = Fixture::new();
    let first = fixture.resume(json!({})).expect("first resume succeeds");
    let state_after_first = fixture.state_bytes();
    let capsule_after_first = fixture.execution_artifact("capsule.json");
    let queue_after_first = fixture.execution_artifact("queue.json");
    let ledger_after_first = fixture.execution_artifact("ledger.jsonl");

    let known_digest = first["data"]["capsuleDigest"].as_str().expect("digest");
    let known_revision = first["data"]["contextRevision"]
        .as_u64()
        .expect("context revision");
    let second = fixture
        .resume(json!({
            "knownCapsuleDigest": known_digest,
            "knownContextRevision": known_revision,
        }))
        .expect("second resume succeeds");

    let data = &second["data"];
    assert_eq!(data["projectionKind"], "no-change");
    assert_eq!(data["capsule"], Value::Null);
    assert_eq!(data["capsuleDigest"], known_digest);
    assert_eq!(data["authorityRefreshCount"], 1);
    assert!(
        serde_json::to_vec(data).expect("data serializes").len() <= 1024,
        "a no-change response must stay within the 1 KiB budget",
    );
    assert_eq!(fixture.state_bytes(), state_after_first);
    assert_eq!(
        fixture.execution_artifact("capsule.json"),
        capsule_after_first
    );
    assert_eq!(fixture.execution_artifact("queue.json"), queue_after_first);
    assert_eq!(
        fixture.execution_artifact("ledger.jsonl"),
        ledger_after_first
    );
}

#[test]
fn resume_without_a_known_digest_reuses_the_committed_capsule() {
    let fixture = Fixture::new();
    let first = fixture.resume(json!({})).expect("first resume succeeds");
    let state_after_first = fixture.state_bytes();

    let second = fixture.resume(json!({})).expect("second resume succeeds");

    assert_eq!(second["data"]["projectionKind"], "full");
    assert_eq!(
        second["data"]["capsuleDigest"],
        first["data"]["capsuleDigest"]
    );
    assert_eq!(second["data"]["capsule"], first["data"]["capsule"]);
    assert_eq!(fixture.state_bytes(), state_after_first);
}

#[test]
fn an_unapproved_plan_fails_closed_without_writing_artifacts() {
    let fixture = Fixture::new();
    fixture.rewrite_state(|state| {
        state["executionPlan"]["approved"] = json!(false);
        state["executionPlan"]["approvedAt"] = Value::Null;
        state["executionPlan"]["approvedBy"] = Value::Null;
    });
    let state_before = fixture.state_bytes();

    let error = fixture
        .resume(json!({}))
        .expect_err("an unapproved plan must fail closed");

    assert_eq!(error.code(), StableErrorCode::ExecutionCapsuleStale);
    assert_eq!(fixture.state_bytes(), state_before);
    assert!(
        !fixture.root.path().join(EXECUTION_DIR).exists(),
        "no execution artifact may be written for an unapproved plan",
    );
}

#[test]
fn plan_digest_drift_fails_closed_without_rewriting_artifacts() {
    let fixture = Fixture::new();
    fixture.resume(json!({})).expect("first resume succeeds");
    let capsule_after_first = fixture.execution_artifact("capsule.json");
    fixture.rewrite_state(|state| {
        state["executionPlan"]["goal"] = json!("a re-scoped goal");
    });
    let state_before = fixture.state_bytes();

    let error = fixture
        .resume(json!({}))
        .expect_err("plan digest drift must fail closed");

    assert_eq!(error.code(), StableErrorCode::ExecutionCapsuleStale);
    assert_eq!(fixture.state_bytes(), state_before);
    assert_eq!(
        fixture.execution_artifact("capsule.json"),
        capsule_after_first
    );
}

#[test]
fn story_drift_fails_closed_without_rewriting_artifacts() {
    let fixture = Fixture::new();
    fixture.resume(json!({})).expect("first resume succeeds");
    let capsule_after_first = fixture.execution_artifact("capsule.json");
    write_file(
        &fixture.root,
        STORY_DOC,
        b"# Story\n\nrewritten verification matrix\n",
    );
    let state_before = fixture.state_bytes();

    let error = fixture
        .resume(json!({}))
        .expect_err("story drift must fail closed");

    assert_eq!(error.code(), StableErrorCode::ExecutionCapsuleStale);
    assert_eq!(fixture.state_bytes(), state_before);
    assert_eq!(
        fixture.execution_artifact("capsule.json"),
        capsule_after_first
    );
}

#[test]
fn constraints_drift_fails_closed_without_rewriting_artifacts() {
    let fixture = Fixture::new();
    fixture.resume(json!({})).expect("first resume succeeds");
    let capsule_after_first = fixture.execution_artifact("capsule.json");
    write_file(
        &fixture.root,
        "constraints/technology-stack.md",
        b"# technology stack\n\nchanged rule\n",
    );
    let state_before = fixture.state_bytes();

    let error = fixture
        .resume(json!({}))
        .expect_err("constraints drift must fail closed");

    assert_eq!(error.code(), StableErrorCode::ExecutionCapsuleStale);
    assert_eq!(fixture.state_bytes(), state_before);
    assert_eq!(
        fixture.execution_artifact("capsule.json"),
        capsule_after_first
    );
}

#[test]
fn verification_drift_fails_closed_without_rewriting_artifacts() {
    let fixture = Fixture::new();
    fixture.resume(json!({})).expect("first resume succeeds");
    let capsule_after_first = fixture.execution_artifact("capsule.json");
    fixture.rewrite_state(|state| {
        state["executionPlan"]["verification"][0]["expect"] =
            json!("a different verification expectation");
    });
    let state_before = fixture.state_bytes();

    let error = fixture
        .resume(json!({}))
        .expect_err("verification drift must fail closed");

    assert_eq!(error.code(), StableErrorCode::ExecutionCapsuleStale);
    assert_eq!(fixture.state_bytes(), state_before);
    assert_eq!(
        fixture.execution_artifact("capsule.json"),
        capsule_after_first
    );
}
