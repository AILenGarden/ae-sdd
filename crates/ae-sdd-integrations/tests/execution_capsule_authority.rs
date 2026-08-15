//! Authoritative `execution.resume` contract tests.
//!
//! One resume call performs a single project-authority snapshot and a single
//! required-context bundle load, verifies the approved plan and the four
//! required contexts (story, constraints, thinking engine, verification),
//! seeds queue/ledger/capsule/state locator atomically on first generation,
//! and fails closed with `EXECUTION_CAPSULE_STALE` on an unapproved plan or
//! any digest drift without writing an artifact.

use std::collections::BTreeSet;
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

    fn acquire_lease(&self, key: &str) -> (String, u64) {
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
            idempotency_key: Some(key.to_owned()),
            confirmation: None,
            deadline_ms: 1_000,
            payload: json!({
                "operation": "lease.acquire",
                "payload": {"owner":{"role":"root"},"ttlSeconds":300},
            }),
        };
        let response = self
            .adapter
            .execute(
                RpcMethod::OperationExecute,
                &request,
                Some(&self.workspace()),
            )
            .expect("lease acquisition succeeds");
        (
            response["data"]["leaseId"]
                .as_str()
                .expect("lease id")
                .to_owned(),
            response["data"]["fencingToken"]
                .as_u64()
                .expect("fencing token"),
        )
    }

    fn slice_operation(
        &self,
        operation: &str,
        payload: Value,
        lease: &(String, u64),
        key: &str,
    ) -> Result<Value, RuntimeError> {
        let request = RequestParams {
            protocol_version: PROTOCOL_VERSION_V1.to_owned(),
            workspace_id: Some(Uuid::from_u128(11).to_string()),
            agent_id: Some("agent-root".to_owned()),
            session_id: Some(Uuid::from_u128(12).to_string()),
            capability_token: None,
            turn_id: None,
            work_item_id: Some(WORK_ITEM_ID.to_owned()),
            lease_id: Some(lease.0.clone()),
            fencing_token: Some(lease.1),
            expected_revision: self.state()["revision"].as_u64(),
            idempotency_key: Some(key.to_owned()),
            confirmation: None,
            deadline_ms: 1_000,
            payload: json!({"operation":operation,"payload":payload}),
        };
        self.adapter.execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&self.workspace()),
        )
    }

    fn slice_operation_jit(
        &self,
        operation: &str,
        payload: Value,
        key: &str,
    ) -> Result<Value, RuntimeError> {
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
            expected_revision: self.state()["revision"].as_u64(),
            idempotency_key: Some(key.to_owned()),
            confirmation: None,
            deadline_ms: 1_000,
            payload: json!({"operation":operation,"payload":payload}),
        };
        self.adapter.execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&self.workspace()),
        )
    }

    fn slice_operation_jit_dry_run(
        &self,
        operation: &str,
        payload: Value,
        key: &str,
    ) -> Result<Value, RuntimeError> {
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
            expected_revision: self.state()["revision"].as_u64(),
            idempotency_key: Some(key.to_owned()),
            confirmation: None,
            deadline_ms: 1_000,
            payload: json!({"operation":operation,"payload":payload,"dryRun":true}),
        };
        self.adapter.execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&self.workspace()),
        )
    }

    fn lease_status(&self) -> Value {
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
            payload: json!({"operation":"lease.status","payload":{}}),
        };
        self.adapter
            .execute(
                RpcMethod::OperationExecute,
                &request,
                Some(&self.workspace()),
            )
            .expect("lease status succeeds")
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
                "expected": "unapproved plan and drift fail closed; repeated resume is no-change"
            },
            {
                "id": "V-EFF-001b",
                "acId": "AC-001",
                "boundary": "contract",
                "command": [
                    "cargo test -p ae-sdd-contracts --test execution_capsule_contract",
                    "cargo test -p ae-sdd-contracts --test resource_assurance_contract"
                ],
                "expected": "contract round trip passes"
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
        data["capsule"]["activeSlice"]["objective"],
        "unapproved plan and drift fail closed; repeated resume is no-change"
    );
    assert_eq!(
        data["nextAction"],
        json!({
            "kind": "execute-approved-slice",
            "activeOrdinal": 1,
            "queueDigest": data["capsule"]["queue"]["queueDigest"],
            "capsuleDigest": data["capsuleDigest"]
                .as_str()
                .expect("capsule digest")
                .strip_prefix("sha256:")
                .expect("prefixed capsule digest"),
            "activeSliceStatus": "pending",
            "nextSliceTransition": "running",
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
fn resume_distributes_large_approved_path_sets_without_widening_authority() {
    let fixture = Fixture::new();
    let changed_paths = (0..40)
        .map(|index| format!("crates/package-{}/src/file-{index}.rs", index / 10))
        .collect::<Vec<_>>();
    fixture.rewrite_state(|state| {
        state["executionPlan"]["changedPaths"] = json!(changed_paths);
    });

    let response = fixture
        .resume(json!({}))
        .expect("a schema-valid large plan produces a bounded capsule");

    let queue: Value = serde_json::from_slice(&fixture.execution_artifact("queue.json"))
        .expect("queue artifact is valid JSON");
    let slices = queue["slices"].as_array().expect("queue slices");
    let approved = changed_paths
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut assigned = BTreeSet::new();
    for slice in slices {
        let scopes = slice["pathScope"].as_array().expect("slice path scope");
        assert!(!scopes.is_empty(), "every slice needs writable authority");
        assert!(scopes.len() <= 32, "v1 path scope limit must be preserved");
        for scope in scopes.iter().filter_map(Value::as_str) {
            assert!(
                approved.contains(scope),
                "slice scope {scope} must be an exact approved changed path"
            );
            assigned.insert(scope);
        }
    }
    assert_eq!(assigned, approved, "all approved paths must be assigned");
    assert_eq!(response["data"]["capsule"]["queue"]["totalSlices"], 2);
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
        state["executionPlan"]["verification"][0]["expected"] =
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

#[test]
fn slice_start_validates_the_authoritative_cursor_without_writing_on_mismatch() {
    let fixture = Fixture::new();
    let resumed = fixture.resume(json!({})).expect("resume succeeds");
    let queue_digest = resumed["data"]["capsule"]["queue"]["queueDigest"]
        .as_str()
        .expect("queue digest");
    let lease = fixture.acquire_lease("slice-start-lease");
    let before = fixture.state_bytes();

    let wrong_ordinal = fixture
        .slice_operation(
            "execution.slice.start",
            json!({"activeOrdinal":2,"queueDigest":queue_digest}),
            &lease,
            "slice-start-wrong-ordinal",
        )
        .expect_err("a stale ordinal must fail closed");
    assert_eq!(wrong_ordinal.code(), StableErrorCode::ExecutionSliceInvalid);
    assert_eq!(fixture.state_bytes(), before);

    let wrong_digest = fixture
        .slice_operation(
            "execution.slice.start",
            json!({"activeOrdinal":1,"queueDigest":"0".repeat(64)}),
            &lease,
            "slice-start-wrong-digest",
        )
        .expect_err("a stale queue digest must fail closed");
    assert_eq!(wrong_digest.code(), StableErrorCode::ExecutionCapsuleStale);
    assert_eq!(fixture.state_bytes(), before);
}

#[test]
fn slice_transition_uses_and_releases_a_just_in_time_writer_lease() {
    let fixture = Fixture::new();
    let resumed = fixture.resume(json!({})).expect("resume succeeds");
    let queue_digest = resumed["data"]["capsule"]["queue"]["queueDigest"]
        .as_str()
        .expect("queue digest");

    let started = fixture
        .slice_operation_jit(
            "execution.slice.start",
            json!({"activeOrdinal":1,"queueDigest":queue_digest}),
            "slice-jit-start",
        )
        .expect("JIT lease starts the projected slice");

    assert_eq!(started["data"]["status"], "running");
    assert_eq!(
        fixture.state()["executionRuntime"]["activeSliceStatus"],
        "running"
    );
    assert_eq!(fixture.lease_status()["data"]["active"], false);
}

#[test]
fn committed_slice_replays_before_a_new_jit_lease_is_acquired() {
    let fixture = Fixture::new();
    let resumed = fixture.resume(json!({})).expect("resume succeeds");
    let queue_digest = resumed["data"]["capsule"]["queue"]["queueDigest"]
        .as_str()
        .expect("queue digest")
        .to_owned();
    let payload = json!({"activeOrdinal":1,"queueDigest":queue_digest});
    let committed = fixture
        .slice_operation_jit("execution.slice.start", payload.clone(), "slice-jit-replay")
        .expect("JIT slice commits");
    let _competing_lease = fixture.acquire_lease("slice-replay-competing-lease");

    let replayed = fixture
        .slice_operation_jit("execution.slice.start", payload, "slice-jit-replay")
        .expect("committed slice replays despite a later active writer lease");

    assert_eq!(replayed["changed"], false);
    assert_eq!(replayed["data"], committed["data"]);
    assert_eq!(replayed["receiptDigest"], committed["receiptDigest"]);
    assert_eq!(replayed["revisionBefore"], committed["revisionBefore"]);
    assert_eq!(replayed["revisionAfter"], committed["revisionAfter"]);
}

#[test]
fn slice_transition_dry_run_has_no_jit_lease_side_effects() {
    let fixture = Fixture::new();
    let resumed = fixture.resume(json!({})).expect("resume succeeds");
    let queue_digest = resumed["data"]["capsule"]["queue"]["queueDigest"]
        .as_str()
        .expect("queue digest");
    let state_before = fixture.state_bytes();
    let lease_before = fixture.lease_status();

    let rejected = fixture
        .slice_operation_jit_dry_run(
            "execution.slice.start",
            json!({"activeOrdinal":1,"queueDigest":queue_digest}),
            "slice-jit-dry-run",
        )
        .expect_err("a validation-only slice without a lease must fail closed");

    assert_eq!(rejected.code(), StableErrorCode::LeaseRequired);
    assert_eq!(fixture.state_bytes(), state_before);
    assert_eq!(fixture.lease_status(), lease_before);
}

#[test]
fn completed_slice_advances_the_authoritative_capsule_one_ordinal() {
    let fixture = Fixture::new();
    let resumed = fixture.resume(json!({})).expect("resume succeeds");
    let first_capsule_digest = resumed["data"]["capsuleDigest"]
        .as_str()
        .expect("capsule digest")
        .to_owned();
    let queue_digest = resumed["data"]["capsule"]["queue"]["queueDigest"]
        .as_str()
        .expect("queue digest")
        .to_owned();
    let lease = fixture.acquire_lease("slice-progress-lease");

    let started = fixture
        .slice_operation(
            "execution.slice.start",
            json!({"activeOrdinal":1,"queueDigest":queue_digest}),
            &lease,
            "slice-1-start",
        )
        .expect("active slice starts");
    assert_eq!(started["data"]["status"], "running");
    assert_eq!(
        fixture.state()["executionRuntime"]["activeSliceStatus"],
        "running"
    );

    let direct_completion = fixture
        .slice_operation(
            "execution.slice.record",
            json!({
                "sliceId":"slice-V-EFF-003",
                "status":"completed",
                "progressDigest":"1".repeat(64),
            }),
            &lease,
            "slice-1-false-complete",
        )
        .expect_err("running cannot jump directly to completed");
    assert_eq!(
        direct_completion.code(),
        StableErrorCode::ExecutionSliceInvalid
    );

    for (index, status) in [
        "red-observed",
        "patched",
        "focused-green",
        "evidence-bound",
        "completed",
    ]
    .into_iter()
    .enumerate()
    {
        fixture
            .slice_operation(
                "execution.slice.record",
                json!({
                    "sliceId":"slice-V-EFF-003",
                    "status":status,
                    "progressDigest":format!("{:064x}", index + 1),
                }),
                &lease,
                &format!("slice-1-{status}"),
            )
            .unwrap_or_else(|error| panic!("{status} commits: {error:?}"));
    }

    let state = fixture.state();
    assert_eq!(state["executionRuntime"]["activeSliceOrdinal"], 2);
    assert_eq!(state["executionRuntime"]["activeSliceStatus"], "pending");
    assert_ne!(
        state["executionRuntime"]["capsuleDigest"],
        first_capsule_digest
    );
    let next = fixture.resume(json!({})).expect("next capsule resumes");
    assert_eq!(next["data"]["capsule"]["queue"]["activeOrdinal"], 2);
    assert_eq!(next["data"]["nextAction"]["kind"], "execute-approved-slice");
    assert_eq!(next["data"]["nextAction"]["activeOrdinal"], 2);
}

#[test]
fn final_completed_slice_closes_the_execution_cursor() {
    let fixture = Fixture::new();
    let resumed = fixture.resume(json!({})).expect("resume succeeds");
    let queue_digest = resumed["data"]["capsule"]["queue"]["queueDigest"]
        .as_str()
        .expect("queue digest")
        .to_owned();
    let lease = fixture.acquire_lease("final-slice-lease");

    complete_slice(
        &fixture,
        &lease,
        1,
        "slice-V-EFF-003",
        &queue_digest,
        "final-first",
    );
    complete_slice(
        &fixture,
        &lease,
        2,
        "slice-V-EFF-001b",
        &queue_digest,
        "final-second",
    );

    let state = fixture.state();
    assert_eq!(state["executionRuntime"]["activeSliceOrdinal"], 2);
    assert_eq!(state["executionRuntime"]["activeSliceStatus"], "completed");
    let terminal = fixture.resume(json!({})).expect("terminal capsule resumes");
    assert_ne!(
        terminal["data"]["nextAction"]["kind"],
        "execute-approved-slice"
    );
}

fn complete_slice(
    fixture: &Fixture,
    lease: &(String, u64),
    ordinal: u32,
    slice_id: &str,
    queue_digest: &str,
    key_prefix: &str,
) {
    fixture
        .slice_operation(
            "execution.slice.start",
            json!({"activeOrdinal":ordinal,"queueDigest":queue_digest}),
            lease,
            &format!("{key_prefix}-start"),
        )
        .unwrap_or_else(|error| panic!("slice {ordinal} starts: {error:?}"));
    for (index, status) in [
        "red-observed",
        "patched",
        "focused-green",
        "evidence-bound",
        "completed",
    ]
    .into_iter()
    .enumerate()
    {
        fixture
            .slice_operation(
                "execution.slice.record",
                json!({
                    "sliceId":slice_id,
                    "status":status,
                    "progressDigest":format!("{:064x}", ordinal * 10 + index as u32),
                }),
                lease,
                &format!("{key_prefix}-{status}"),
            )
            .unwrap_or_else(|error| panic!("slice {ordinal} {status} commits: {error:?}"));
    }
}
