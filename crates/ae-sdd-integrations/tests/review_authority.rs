use std::{fs, str::FromStr};

use ae_sdd_domain::{
    AgentRole, FencingToken, InputFingerprint, LeaseId, PolicyDigest, ProjectKey, SessionId,
    StateRevision, WorkItemId, WorkspaceId,
};
use ae_sdd_operations::{OperationName, OperationRequest, ValidatedOperationRequest};
use ae_sdd_protocol::{ClientKind, StableErrorCode, WorkspaceMode};
use ae_sdd_review::ReviewSupervisor;
use ae_sdd_runtime::{BusinessWorkspace, RuntimeError};
use ae_sdd_store::UtcTimestamp;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use uuid::Uuid;

#[allow(dead_code)]
#[path = "../src/gate_source/mod.rs"]
mod gate_source;
#[allow(dead_code)]
#[path = "../src/persistence.rs"]
mod persistence;
#[allow(dead_code)]
#[path = "../src/review_authority.rs"]
mod review_authority;

use persistence::SqliteRuntimePersistence;

use review_authority::{
    AuthenticatedCaller, authoritative_review_workspace_input_fingerprint, prepare_review_record,
    review_projection_write_from_state, select_or_start_session, validate_clean_contribution_depth,
};

const WORK_ITEM: &str = "STORY-REVIEW-AUTHORITY-001";
const WORKSPACE_ID: &str = "00000000-0000-0000-0000-000000000001";
const POLICY: &str = "3333333333333333333333333333333333333333333333333333333333333333";

fn session(seed: u128) -> SessionId {
    SessionId::from_uuid(Uuid::from_u128(seed))
}

fn operation_request(session_id: SessionId, revision: u64, payload: Value) -> OperationRequest {
    OperationRequest {
        operation: OperationName::ReviewRecord,
        workspace_id: Some(WorkspaceId::from_uuid(Uuid::from_u128(1))),
        project_key: Some(ProjectKey::new("review-authority").expect("project key")),
        work_item_id: Some(WorkItemId::new(WORK_ITEM).expect("work item")),
        session_id: Some(session_id),
        lease_id: Some(LeaseId::from_uuid(Uuid::from_u128(2))),
        fencing_token: Some(FencingToken::new(1)),
        expected_revision: Some(StateRevision::new(revision)),
        idempotency_key: Some("review-authority-attempt-1".into()),
        confirmation: None,
        dry_run: false,
        payload,
    }
}

fn request(session_id: SessionId, revision: u64, payload: Value) -> ValidatedOperationRequest {
    ValidatedOperationRequest::validate(operation_request(session_id, revision, payload))
        .expect("valid review request")
}

fn state() -> Value {
    json!({
        "stateMachineName":"PRD-REVIEW-AUTHORITY-001",
        "activeStory":WORK_ITEM,
        "revision":7,
        "lastFencingToken":1,
        "scale":"small",
        "phase":"test-running",
        "currentPhase":"test-running",
        "executionPlan":{"changedPaths":["src/lib.rs"]},
        "storyStates":{
            WORK_ITEM:{"phase":"test-running","currentPhase":"test-running"}
        }
    })
}

struct Fixture {
    _root: TempDir,
    _database_root: TempDir,
    workspace: BusinessWorkspace,
    persistence: SqliteRuntimePersistence,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("workspace tempdir");
        let canonical_root = fs::canonicalize(root.path())
            .expect("workspace canonicalizes")
            .to_string_lossy()
            .into_owned();
        let database_root = TempDir::new().expect("database tempdir");
        let persistence = SqliteRuntimePersistence::open(database_root.path().join("runtime.db"))
            .expect("runtime persistence opens");
        Self {
            _root: root,
            _database_root: database_root,
            workspace: BusinessWorkspace {
                workspace_id: WORKSPACE_ID.to_owned(),
                canonical_root,
                project_key: "review-authority".to_owned(),
                mode: WorkspaceMode::RustCanary,
                agent_role: Some(AgentRole::Reviewer),
                agent_grant: None,
                caller_kind: Some(ClientKind::Cli),
                inventory_generation: 3,
            },
            persistence,
        }
    }

    fn prepare(
        &self,
        state: &Value,
        request: &ValidatedOperationRequest,
        caller: &AuthenticatedCaller,
    ) -> Result<review_authority::PreparedReviewRecord, RuntimeError> {
        prepare_review_record(
            &self.workspace,
            state,
            WORK_ITEM,
            request,
            caller,
            &self.persistence,
            "00000000-0000-0000-0000-000000000099",
            POLICY,
            self.workspace.inventory_generation,
            &UtcTimestamp::now(),
        )
    }

    fn write_source(&self, relative: &str, content: &str) {
        let path = self._root.path().join(relative);
        fs::create_dir_all(path.parent().expect("source parent")).expect("source directory");
        fs::write(path, content).expect("source file");
    }

    fn write_finalized_manifest(&self, input: &str, evidence_id: &str) {
        let mut manifest = json!({
            "schemaVersion":1,
            "storyId":WORK_ITEM,
            "entries":[{
                "evidenceId":evidence_id,
                "kind":"focused-test",
                "inputFingerprint":input,
                "status":"active",
                "exitCode":0,
                "reusable":true,
                "artifacts":[]
            }]
        });
        let content_hash = manifest_content_hash(&manifest);
        manifest["contentHash"] = json!(content_hash);
        let path = self._root.path().join(format!(
            ".auto-engineering/{WORK_ITEM}/evidence/manifest.json"
        ));
        fs::create_dir_all(path.parent().expect("manifest parent")).expect("manifest directory");
        fs::write(path, serde_json::to_vec(&manifest).expect("manifest JSON"))
            .expect("manifest file");
    }
}

fn manifest_content_hash(manifest: &Value) -> String {
    let mut payload = manifest.clone();
    payload
        .as_object_mut()
        .expect("manifest object")
        .retain(|key, _| key != "contentHash" && !key.starts_with('_'));
    format!(
        "sha256:{}",
        hex::encode(Sha256::digest(
            serde_json::to_vec(&payload).expect("manifest canonical bytes"),
        ))
    )
}

fn terminal_review_state() -> Value {
    let session: ae_sdd_contracts::review::ReviewSessionV2 = serde_json::from_value(json!({
        "schemaVersion":"v2",
        "reviewId":"review-replay",
        "parentReviewId":null,
        "tier":"tier1",
        "requiredSpecialties":["general"],
        "authorSessionId":"00000000-0000-0000-0000-000000000010",
        "rootSessionId":"00000000-0000-0000-0000-000000000001",
        "inputFingerprint":"1111111111111111111111111111111111111111111111111111111111111111",
        "rulesetFingerprint":"2222222222222222222222222222222222222222222222222222222222222222",
        "policyDigest":POLICY,
        "sourceRevision":7,
        "inventoryGeneration":3,
        "repairClass":"none",
        "cleanPolicy":{"cleanTarget":1,"finalProofRequirement":"none"},
        "budget":{"maxAttempts":3,"maxValidBatches":2,"maxRemediations":2,"maxWallClockMinutes":30},
        "counters":{"attempts":0,"validBatches":0,"cleanStreak":0,"remediations":0,"infraFailures":0,"protocolFailures":0},
        "status":"running",
        "startedAt":"2026-07-26T00:00:00Z",
        "deadlineAt":"2026-07-26T01:00:00Z",
        "terminalAt":null
    }))
    .expect("replay session");
    let attempt: ae_sdd_contracts::review::ReviewAttemptV2 = serde_json::from_value(json!({
        "schemaVersion":"v2",
        "reviewId":"review-replay",
        "batchId":"batch-replay",
        "attemptId":"attempt-replay",
        "attemptOrdinal":1,
        "idempotencyKey":"attempt-replay-key",
        "inputFingerprint":"1111111111111111111111111111111111111111111111111111111111111111",
        "rulesetFingerprint":"2222222222222222222222222222222222222222222222222222222222222222",
        "contributions":[{
            "sourceAttemptId":"attempt-replay",
            "reviewer":{
                "agentRole":"reviewer","specialty":"general","grantedSpecialties":["general"],
                "physicalSessionId":"00000000-0000-0000-0000-000000000020",
                "rootSessionId":"00000000-0000-0000-0000-000000000001",
                "delegationId":"10000000-0000-0000-0000-000000000020","lineageDepth":2,
                "attestationRef":"delegation:reviewer","attestationDigest":"3333333333333333333333333333333333333333333333333333333333333333",
                "specialtyGrantDigest":"3333333333333333333333333333333333333333333333333333333333333333"
            },
            "outcome":"clean",
            "findings":[],
            "reportDigest":"3333333333333333333333333333333333333333333333333333333333333333",
            "contributionDigest":"4444444444444444444444444444444444444444444444444444444444444444",
            "inputFingerprint":"1111111111111111111111111111111111111111111111111111111111111111",
            "rulesetFingerprint":"2222222222222222222222222222222222222222222222222222222222222222"
        }],
        "observedAt":"2026-07-26T00:01:00Z",
        "finalProof":{"kind":"none","digest":null,"sourceRevision":null,"inputFingerprint":null,"rulesetFingerprint":null,"observedAt":null},
        "projectAuthority":{"projectReceiptRef":"state:review","activeManifestDigest":"3333333333333333333333333333333333333333333333333333333333333333","stateReceiptRefDigest":"3333333333333333333333333333333333333333333333333333333333333333","journalMutationId":"mutation-replay"},
        "remediation":null
    }))
    .expect("replay attempt");
    let attempt_value = serde_json::to_value(&attempt).expect("attempt projection");
    let evaluated = ReviewSupervisor::evaluate(&session, None, attempt).expect("clean evaluate");
    json!({
        "reviewSession": evaluated.next_session(),
        "review": {
            "status":"passed",
            "findings":[],
            "batch":evaluated.next_batch(),
            "attempt":attempt_value,
            "receipt":evaluated.exit_receipt()
        }
    })
}

#[test]
fn review_projection_replay_requires_a_complete_consistent_v2_tuple() {
    let state = terminal_review_state();
    let write = review_projection_write_from_state(&state, WORKSPACE_ID, WORK_ITEM, 41)
        .expect("valid replay state")
        .expect("v2 projection write");
    assert_eq!(write.event_sequence(), 41);

    let mut drifted = state;
    drifted["review"]["receipt"]["reviewId"] = json!("review-other");
    let error = review_projection_write_from_state(&drifted, WORKSPACE_ID, WORK_ITEM, 41)
        .expect_err("cross-record drift must fail closed");
    assert_eq!(error.code(), StableErrorCode::ExternalStateConflict);
}

#[test]
fn payload_identity_fields_are_rejected_by_the_operation_schema() {
    let state = state();
    let before = state.clone();
    let reviewer_session = session(10);
    let error = ValidatedOperationRequest::validate(operation_request(
        reviewer_session,
        7,
        json!({
            "status":"passed",
            "findings":[],
            "physicalSessionId":reviewer_session.to_string(),
            "specialty":"general"
        }),
    ))
    .expect_err("caller identity fields are not part of the review payload");

    assert!(error.to_string().contains("physicalSessionId"));
    assert_eq!(state, before, "schema rejection must not mutate state");
}

#[test]
fn payload_claims_cannot_replace_missing_typed_session_authority() {
    let fixture = Fixture::new();
    let state = state();
    let before = state.clone();
    let reviewer_session = session(10);
    let request = request(
        reviewer_session,
        7,
        json!({"status":"passed","findings":[]}),
    );
    let caller = AuthenticatedCaller::new("reviewer-agent", reviewer_session, AgentRole::Reviewer);

    let error = fixture
        .prepare(&state, &request, &caller)
        .expect_err("payload claims cannot manufacture typed session authority");

    assert_eq!(error.code(), StableErrorCode::TurnIdentityMismatch);
    assert_eq!(state, before, "failed preparation must not mutate state");
}

#[test]
fn authenticated_session_mismatch_fails_before_persistence() {
    let fixture = Fixture::new();
    let state = state();
    let request = request(session(10), 7, json!({"status":"passed","findings":[]}));
    let caller = AuthenticatedCaller::new("reviewer-agent", session(11), AgentRole::Reviewer);

    let error = fixture
        .prepare(&state, &request, &caller)
        .expect_err("request and physical caller must be the same daemon session");

    assert_eq!(error.code(), StableErrorCode::TurnIdentityMismatch);
}

#[test]
fn review_projection_and_commit_metadata_do_not_drift_locked_input() {
    let fixture = Fixture::new();
    let initial = state();
    let expected = authoritative_review_workspace_input_fingerprint(&fixture.workspace, &initial)
        .expect("input fingerprints");
    let mut projected = initial;
    projected["revision"] = json!(9);
    projected["lastFencingToken"] = json!(3);
    projected["lastMutation"] = json!({
        "operation":"review.record",
        "revisionBefore":8,
        "revisionAfter":9
    });
    projected["inputFingerprint"] = json!(expected.to_string());
    projected["rulesetFingerprint"] = json!(POLICY);
    projected["policyDigest"] = json!(POLICY);
    projected["inventoryGeneration"] = json!(3);
    projected["reviewSession"] = json!({"schemaVersion":"v2"});
    projected["review"] = json!({"status":"pending","batch":{"schemaVersion":"v2"}});

    assert_eq!(
        authoritative_review_workspace_input_fingerprint(&fixture.workspace, &projected)
            .expect("projected input fingerprints"),
        expected
    );
}

#[test]
fn non_review_state_change_invalidates_locked_input() {
    let fixture = Fixture::new();
    let initial = state();
    let expected = authoritative_review_workspace_input_fingerprint(&fixture.workspace, &initial)
        .expect("input fingerprints");
    let mut changed = initial;
    changed["executionPlan"] = json!({"goal":"different authority"});

    assert_ne!(
        authoritative_review_workspace_input_fingerprint(&fixture.workspace, &changed)
            .expect("changed input fingerprints"),
        expected
    );
}

#[test]
fn source_change_invalidates_locked_review_input() {
    let fixture = Fixture::new();
    fixture.write_source("src/lib.rs", "pub fn value() -> u8 { 1 }\n");
    let initial = state();
    let expected = authoritative_review_workspace_input_fingerprint(&fixture.workspace, &initial)
        .expect("input fingerprints");

    fixture.write_source("src/lib.rs", "pub fn value() -> u8 { 2 }\n");

    assert_ne!(
        authoritative_review_workspace_input_fingerprint(&fixture.workspace, &initial)
            .expect("changed source fingerprint"),
        expected
    );
}

#[test]
fn clean_review_requires_depth_evidence() {
    let fixture = Fixture::new();
    fixture.write_source("src/lib.rs", "pub fn value() -> u8 { 1 }\n");
    let state = state();
    let input = authoritative_review_workspace_input_fingerprint(&fixture.workspace, &state)
        .expect("input fingerprint");
    let payload = json!({"status":"passed","findings":[]});

    let error = validate_clean_contribution_depth(
        &fixture.workspace,
        &state,
        WORK_ITEM,
        payload.as_object().expect("payload object"),
        input,
    )
    .expect_err("bare clean review must be rejected");

    assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
}

#[test]
fn clean_review_accepts_only_scoped_existing_paths_and_fresh_active_evidence() {
    let fixture = Fixture::new();
    fixture.write_source("src/lib.rs", "pub fn value() -> u8 { 1 }\n");
    let state = state();
    let input = authoritative_review_workspace_input_fingerprint(&fixture.workspace, &state)
        .expect("input fingerprint");
    fixture.write_finalized_manifest(&input.to_string(), "ev-review-depth");
    let payload = json!({
        "status":"passed",
        "findings":[],
        "reviewedPaths":["src/lib.rs"],
        "evidenceIds":["ev-review-depth"]
    });

    validate_clean_contribution_depth(
        &fixture.workspace,
        &state,
        WORK_ITEM,
        payload.as_object().expect("payload object"),
        input,
    )
    .expect("fresh scoped evidence should be accepted");

    let mut out_of_scope = payload.clone();
    out_of_scope["reviewedPaths"] = json!(["src/other.rs"]);
    let error = validate_clean_contribution_depth(
        &fixture.workspace,
        &state,
        WORK_ITEM,
        out_of_scope.as_object().expect("payload object"),
        input,
    )
    .expect_err("unapproved path must be rejected");
    assert_eq!(error.code(), StableErrorCode::GateBlocked);

    fixture.write_finalized_manifest(
        &InputFingerprint::digest(b"stale-input").to_string(),
        "ev-review-depth-stale",
    );
    let mut stale = payload;
    stale["evidenceIds"] = json!(["ev-review-depth-stale"]);
    let error = validate_clean_contribution_depth(
        &fixture.workspace,
        &state,
        WORK_ITEM,
        stale.as_object().expect("payload object"),
        input,
    )
    .expect_err("stale evidence input must be rejected");
    assert_eq!(error.code(), StableErrorCode::GateBlocked);
}

#[test]
fn remediation_drift_starts_child_session_with_parent_and_incremented_counter() {
    let session: ae_sdd_contracts::review::ReviewSessionV2 = serde_json::from_value(json!({
        "schemaVersion":"v2",
        "reviewId":"review-parent",
        "parentReviewId":null,
        "tier":"tier1",
        "requiredSpecialties":["general"],
        "authorSessionId":"00000000-0000-0000-0000-000000000010",
        "rootSessionId":"00000000-0000-0000-0000-000000000001",
        "inputFingerprint":"1111111111111111111111111111111111111111111111111111111111111111",
        "rulesetFingerprint":"2222222222222222222222222222222222222222222222222222222222222222",
        "policyDigest":"3333333333333333333333333333333333333333333333333333333333333333",
        "sourceRevision":7,
        "inventoryGeneration":3,
        "repairClass":"none",
        "cleanPolicy":{"cleanTarget":1,"finalProofRequirement":"none"},
        "budget":{"maxAttempts":3,"maxValidBatches":2,"maxRemediations":2,"maxWallClockMinutes":30},
        "counters":{"attempts":0,"validBatches":0,"cleanStreak":0,"remediations":0,"infraFailures":0,"protocolFailures":0},
        "status":"running",
        "startedAt":"2026-07-26T00:00:00Z",
        "deadlineAt":"2026-07-26T01:00:00Z",
        "terminalAt":null
    }))
    .expect("parent session");
    let attempt: ae_sdd_contracts::review::ReviewAttemptV2 = serde_json::from_value(json!({
        "schemaVersion":"v2",
        "reviewId":"review-parent",
        "batchId":"batch-findings",
        "attemptId":"attempt-findings",
        "attemptOrdinal":1,
        "idempotencyKey":"attempt-findings-key",
        "inputFingerprint":"1111111111111111111111111111111111111111111111111111111111111111",
        "rulesetFingerprint":"2222222222222222222222222222222222222222222222222222222222222222",
        "contributions":[{
            "sourceAttemptId":"attempt-findings",
            "reviewer":{
                "agentRole":"reviewer","specialty":"general","grantedSpecialties":["general"],
                "physicalSessionId":"00000000-0000-0000-0000-000000000020",
                "rootSessionId":"00000000-0000-0000-0000-000000000001",
                "delegationId":"10000000-0000-0000-0000-000000000020","lineageDepth":2,
                "attestationRef":"delegation:reviewer","attestationDigest":"3333333333333333333333333333333333333333333333333333333333333333",
                "specialtyGrantDigest":"3333333333333333333333333333333333333333333333333333333333333333"
            },
            "outcome":"findings",
            "findings":[{"code":"review.defect","severity":"major","summary":"defect"}],
            "reportDigest":"3333333333333333333333333333333333333333333333333333333333333333",
            "contributionDigest":"4444444444444444444444444444444444444444444444444444444444444444",
            "inputFingerprint":"1111111111111111111111111111111111111111111111111111111111111111",
            "rulesetFingerprint":"2222222222222222222222222222222222222222222222222222222222222222"
        }],
        "observedAt":"2026-07-26T00:01:00Z",
        "finalProof":{"kind":"none","digest":null,"sourceRevision":null,"inputFingerprint":null,"rulesetFingerprint":null,"observedAt":null},
        "projectAuthority":{"projectReceiptRef":"state:review","activeManifestDigest":"3333333333333333333333333333333333333333333333333333333333333333","stateReceiptRefDigest":"3333333333333333333333333333333333333333333333333333333333333333","journalMutationId":"mutation-parent"},
        "remediation":null
    }))
    .expect("findings attempt");
    let evaluated = ReviewSupervisor::evaluate(&session, None, attempt).expect("findings evaluate");
    assert_eq!(
        evaluated.next_session().status(),
        ae_sdd_contracts::review::ReviewSessionStatusV2::RemediationRequired
    );

    let drifted_input = InputFingerprint::digest(b"changed-input");
    let plan = InputFingerprint::digest(b"committed-plan");
    let (child, remediation) = select_or_start_session(
        Some(evaluated.next_session().clone()),
        Some(evaluated.next_batch()),
        ae_sdd_contracts::review::ReviewTier::Tier1,
        ae_sdd_contracts::review::ReviewRepairClass::None,
        WORK_ITEM,
        SessionId::from_uuid(Uuid::from_u128(0x10)),
        SessionId::from_uuid(Uuid::from_u128(1)),
        drifted_input,
        InputFingerprint::from_str(
            "2222222222222222222222222222222222222222222222222222222222222222",
        )
        .expect("ruleset"),
        PolicyDigest::from_str(POLICY).expect("policy"),
        8,
        3,
        Some(plan),
        &ae_sdd_contracts::review::ReviewTimestamp::new("2026-07-26T00:02:00Z".to_owned())
            .expect("timestamp"),
    )
    .expect("child session");

    assert_eq!(child.parent_review_id(), Some(session.review_id()));
    assert_eq!(child.counters().remediations(), 1);
    let remediation = remediation.expect("committed remediation");
    assert_eq!(
        remediation.finding_batch_id(),
        evaluated.next_batch().batch_id()
    );
    assert_eq!(remediation.plan_fingerprint(), plan);
    assert_eq!(remediation.new_input_fingerprint(), drifted_input);
    assert_eq!(remediation.next_review_id(), child.review_id());
}
