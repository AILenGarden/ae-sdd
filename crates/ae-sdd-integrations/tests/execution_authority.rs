#[allow(dead_code)]
#[path = "../src/execution_authority.rs"]
mod execution_authority;

use std::fs;

use ae_sdd_contracts::{BoundedText, ExecutionId, ExecutionStepId, SchemaVersion, WorkerId};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, EvidenceDigest, InputFingerprint,
    ProjectRelativePath, StateRevision, WorkItemId,
};
use ae_sdd_execution::{ExecutionStep, VerificationExecutionPlan};
use ae_sdd_protocol::{JobStatus, StableErrorCode, WorkspaceMode};
use ae_sdd_runtime::{RuntimeJobRecord, RuntimeJobStatus, WireAgentRole};
use execution_authority::{
    canonical_execution_plan_digest, prepare_execution_plan_from_authority,
    validate_verification_input_binding,
};
use serde_json::{Value, json};
use tempfile::TempDir;

const WORKSPACE_ID: &str = "00000000-0000-0000-0000-000000000701";
const SESSION_ID: &str = "00000000-0000-0000-0000-000000000702";
const JOB_ID: &str = "00000000-0000-0000-0000-000000000703";
const WORK_ITEM_ID: &str = "STORY-C1-001";
const POLICY_DIGEST: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const METHODOLOGY_DIGEST: &str = "2222222222222222222222222222222222222222222222222222222222222222";

#[test]
fn shell_program_is_rejected_before_project_authority_is_consulted() {
    let fixture = AuthorityFixture::new("cmd.exe");
    let before = fixture.state.clone();

    let error = fixture
        .prepare()
        .expect_err("shell dispatchers must be rejected");

    assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
    assert_eq!(fixture.state, before);
}

#[test]
fn committed_job_and_active_project_receipt_authorize_the_exact_plan() {
    let fixture = AuthorityFixture::new("tools/cargo.exe");

    let prepared = fixture.prepare().expect("durable authority is accepted");

    assert_eq!(prepared.plan(), &fixture.plan);
    assert_eq!(prepared.toolset_job_id(), JOB_ID);
    assert_eq!(prepared.source_revision(), 7);
    assert_eq!(prepared.plan_digest(), fixture.payload["planDigest"]);
}

#[test]
fn state_only_toolset_receipt_injection_never_authorizes_verification() {
    let mut fixture = AuthorityFixture::new("tools/cargo.exe");
    fixture.state["toolsetReceipt"] = json!({
        "validated":true,
        "status":"pass",
        "sourceRevision":7,
        "plan":fixture.plan,
    });
    fixture
        .state
        .as_object_mut()
        .expect("state object")
        .remove("toolsetReceiptRef");

    let error = fixture
        .prepare()
        .expect_err("state-only receipt is not durable authority");

    assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
}

#[test]
fn non_pass_or_uncommitted_job_cannot_authorize_project_state() {
    let mut non_pass = AuthorityFixture::new("tools/cargo.exe");
    non_pass.job.status = RuntimeJobStatus::Fail;
    let error = non_pass
        .prepare()
        .expect_err("non-PASS job must be rejected");
    assert_eq!(error.code(), StableErrorCode::GateBlocked);

    let mut uncommitted = AuthorityFixture::new("tools/cargo.exe");
    uncommitted.job.mutation_id = None;
    uncommitted.job.receipt_locator = None;
    uncommitted.job.project_receipt_digest = None;
    let error = uncommitted
        .prepare()
        .expect_err("PASS without project proof must be rejected");
    assert_eq!(error.code(), StableErrorCode::GateBlocked);
}

#[test]
fn wrapper_job_project_and_current_bindings_must_all_match() {
    let mut stale = AuthorityFixture::new("tools/cargo.exe");
    stale.payload["sourceRevision"] = json!(6);
    let error = stale
        .prepare()
        .expect_err("stale wrapper revision is rejected");
    assert_eq!(error.code(), StableErrorCode::StaleGateResult);

    let mut tampered = AuthorityFixture::new("tools/cargo.exe");
    tampered.payload["plan"]["steps"][0]["args"][0] = json!("check");
    let error = tampered
        .prepare()
        .expect_err("receipt cannot authorize modified steps");
    assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);

    let inactive = AuthorityFixture::new("tools/cargo.exe");
    let manifest_path = inactive.root.path().join(
        inactive.state["toolsetReceiptRef"]["manifestRef"]
            .as_str()
            .expect("manifest ref"),
    );
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest bytes"))
            .expect("manifest json");
    manifest["entries"][0]["status"] = json!("superseded");
    fs::write(&manifest_path, pretty_json(&manifest)).expect("tampered manifest");
    let error = inactive
        .prepare()
        .expect_err("inactive manifest entry is rejected");
    assert_eq!(error.code(), StableErrorCode::ExternalStateConflict);
}

#[test]
fn generated_verification_plan_must_match_execution_input_fingerprint() {
    let execution_plan = plan("tools/cargo.exe");
    let raw_fingerprint = execution_plan["inputFingerprint"]
        .as_str()
        .expect("execution fingerprint");
    let matching_raw = json!({"inputFingerprint":raw_fingerprint});
    validate_verification_input_binding(&execution_plan, &matching_raw)
        .expect("same raw verification input is bound");
    let matching_prefixed = json!({
        "inputFingerprint":format!("sha256:{raw_fingerprint}")
    });
    validate_verification_input_binding(&execution_plan, &matching_prefixed)
        .expect("sha256-prefixed verification input matches the raw contract digest");

    let different = json!({"inputFingerprint":format!("sha256:{}", "0".repeat(64))});
    let error = validate_verification_input_binding(&execution_plan, &different)
        .expect_err("a receipt cannot authorize a different verification input");
    assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
}

struct AuthorityFixture {
    root: TempDir,
    state: Value,
    payload: Value,
    job: RuntimeJobRecord,
    plan: Value,
}

impl AuthorityFixture {
    fn new(program: &str) -> Self {
        let root = TempDir::new().expect("workspace");
        let plan = plan(program);
        let typed_plan: VerificationExecutionPlan =
            serde_json::from_value(plan.clone()).expect("typed plan");
        let receipt = typed_plan
            .receipt(
                WorkerId::new("worker-c1").expect("worker"),
                JobStatus::Pass,
                Some(0),
                EvidenceDigest::digest(b"stdout"),
                EvidenceDigest::digest(b"stderr"),
                1,
                2,
                false,
                false,
            )
            .expect("receipt");
        let receipt_value = serde_json::to_value(&receipt).expect("receipt value");
        let plan_digest = canonical_execution_plan_digest(&plan).expect("plan digest");
        let receipt_digest = digest_json(&receipt_value);
        let receipt_id = format!("toolset-{}", &receipt_digest[..16]);
        let mutation_id = "00000000-0000-0000-0000-000000000704";
        let artifact_ref =
            format!(".auto-engineering/{WORK_ITEM_ID}/evidence/toolset/{receipt_id}.json");
        let manifest_ref = format!(".auto-engineering/{WORK_ITEM_ID}/evidence/manifest.json");
        let input_fingerprint = plan["inputFingerprint"]
            .as_str()
            .expect("input fingerprint")
            .to_owned();
        let snapshot = json!({
            "schemaVersion":1,
            "kind":"toolsetReceiptAuthority",
            "toolsetJobId":JOB_ID,
            "workspaceId":WORKSPACE_ID,
            "workItemId":WORK_ITEM_ID,
            "outcome":"PASS",
            "status":"pass",
            "validated":true,
            "receiptId":receipt_id,
            "receiptDigest":receipt_digest,
            "plan":plan,
            "receipt":receipt_value,
            "planDigest":plan_digest,
            "methodologyDigest":METHODOLOGY_DIGEST,
            "policyDigest":POLICY_DIGEST,
            "inputFingerprint":input_fingerprint,
            "sourceRevision":7,
            "committedRevision":7,
            "inventoryGeneration":3,
            "identityDigest":"3333333333333333333333333333333333333333333333333333333333333333",
            "mutationId":mutation_id,
            "recorder":{
                "sessionId":SESSION_ID,
                "rootSessionId":SESSION_ID,
                "delegationId":null,
                "contextGeneration":2
            }
        });
        let snapshot_bytes = pretty_json(&snapshot);
        let project_receipt_digest = ArtifactDigest::digest(&snapshot_bytes).to_string();
        write_file(&root, &artifact_ref, &snapshot_bytes);

        let manifest = json!({
            "schemaVersion":1,
            "storyId":WORK_ITEM_ID,
            "entries":[{
                "evidenceId":receipt_id,
                "kind":"toolset-receipt",
                "logicalKey":format!("toolset/{WORK_ITEM_ID}"),
                "status":"active",
                "toolsetJobId":JOB_ID,
                "workItemId":WORK_ITEM_ID,
                "inputFingerprint":input_fingerprint,
                "exitCode":0,
                "receiptDigest":receipt_digest,
                "planDigest":plan_digest,
                "policyDigest":POLICY_DIGEST,
                "methodologyDigest":METHODOLOGY_DIGEST,
                "sourceRevision":7,
                "inventoryGeneration":3,
                "recorderSessionId":SESSION_ID,
                "artifacts":[{
                    "path":artifact_ref,
                    "snapshotPath":artifact_ref,
                    "sha256":format!("sha256:{project_receipt_digest}")
                }]
            }]
        });
        let manifest_bytes = pretty_json(&manifest);
        let manifest_digest = ArtifactDigest::digest(&manifest_bytes).to_string();
        write_file(&root, &manifest_ref, &manifest_bytes);

        let state = json!({
            "revision":7,
            "toolsetReceiptRef":{
                "schemaVersion":1,
                "toolsetJobId":JOB_ID,
                "receiptId":receipt_id,
                "receiptDigest":receipt_digest,
                "artifactRef":artifact_ref,
                "projectReceiptDigest":project_receipt_digest,
                "manifestRef":manifest_ref,
                "manifestDigest":manifest_digest,
                "mutationId":mutation_id,
                "sourceRevision":7,
                "committedRevision":7
            }
        });
        let result = json!({
            "outcome":"PASS",
            "validated":true,
            "toolsetJobId":JOB_ID,
            "receiptId":receipt_id,
            "receiptDigest":receipt_digest,
            "planDigest":plan_digest,
            "methodologyDigest":METHODOLOGY_DIGEST,
            "policyDigest":POLICY_DIGEST,
            "inputFingerprint":input_fingerprint,
            "sourceRevision":7,
            "committedRevision":7,
            "revisionAfter":7,
            "inventoryGeneration":3,
            "workItemId":WORK_ITEM_ID,
            "identityDigest":"3333333333333333333333333333333333333333333333333333333333333333",
            "mutationId":mutation_id,
            "receiptLocator":artifact_ref,
            "projectReceiptDigest":project_receipt_digest
        });
        let payload = json!({
            "toolsetJobId":JOB_ID,
            "plan":plan,
            "receiptId":receipt_id,
            "receiptDigest":receipt_digest,
            "sourceRevision":7,
            "planDigest":plan_digest,
            "methodologyDigest":METHODOLOGY_DIGEST,
            "policyDigest":POLICY_DIGEST,
            "inputFingerprint":input_fingerprint,
            "changedPaths":["crates/core/src.rs"],
            "sinceFingerprint":null,
            "persist":true
        });
        let job = RuntimeJobRecord {
            job_id: JOB_ID.to_owned(),
            workspace_id: WORKSPACE_ID.to_owned(),
            work_item_id: Some(WORK_ITEM_ID.to_owned()),
            session_id: Some(SESSION_ID.to_owned()),
            root_session_id: Some(SESSION_ID.to_owned()),
            delegation_id: None,
            agent_role: Some(WireAgentRole::Root),
            context_generation: Some(2),
            submission_boot_id: Some("boot-c1".to_owned()),
            attestation_ref: None,
            attestation_digest: None,
            grant: None,
            identity_digest: Some(
                "3333333333333333333333333333333333333333333333333333333333333333".to_owned(),
            ),
            workspace_mode: WorkspaceMode::RustCanary,
            inventory_generation: 3,
            entrypoint: "toolset.receipt.record".to_owned(),
            arguments: json!({}),
            submission_scope_digest: "4".repeat(64),
            submission_idempotency_key: "toolset-job-c1".to_owned(),
            submission_idempotency_key_digest: "5".repeat(64),
            request_digest: "6".repeat(64),
            source_revision: Some(7),
            input_fingerprint: Some(input_fingerprint),
            deadline_unix_ms: 10_000,
            status: RuntimeJobStatus::Pass,
            row_version: 2,
            result: Some(result),
            error_code: None,
            mutation_id: Some(mutation_id.to_owned()),
            receipt_locator: Some(artifact_ref),
            project_receipt_digest: Some(project_receipt_digest),
            submitted_event_seq: 1,
            last_event_seq: 3,
            created_at_unix_ms: 1,
            started_at_unix_ms: Some(2),
            finished_at_unix_ms: Some(3),
            updated_at_unix_ms: 3,
        };
        Self {
            root,
            state,
            payload,
            job,
            plan,
        }
    }

    fn prepare(
        &self,
    ) -> Result<execution_authority::VerifiedExecutionAuthority, ae_sdd_runtime::RuntimeError> {
        prepare_execution_plan_from_authority(
            self.root.path(),
            &self.state,
            &self.payload,
            &self.job,
            WORKSPACE_ID,
            WORK_ITEM_ID,
            StateRevision::new(7),
            POLICY_DIGEST,
            3,
        )
    }
}

fn plan(program: &str) -> Value {
    let executable = ArtifactRef::new(
        ArtifactKind::new("verification-program").expect("kind"),
        ProjectRelativePath::new(program).expect("program path"),
        ArtifactDigest::digest(program.as_bytes()),
        1,
    );
    let step = ExecutionStep::new(
        SchemaVersion::V1,
        ExecutionStepId::new("focused-tests").expect("step id"),
        executable,
        vec![BoundedText::new("test").expect("argument")],
        None,
        Vec::new(),
    )
    .expect("step");
    let plan = VerificationExecutionPlan::new(
        SchemaVersion::V1,
        ExecutionId::new("execution-c1").expect("execution id"),
        WorkItemId::new(WORK_ITEM_ID).expect("work item"),
        InputFingerprint::digest(b"c1 verification input"),
        vec![step],
    )
    .expect("plan");
    serde_json::to_value(plan).expect("plan serializes")
}

fn digest_json(value: &Value) -> String {
    ArtifactDigest::digest(serde_json::to_vec(value).expect("canonical json")).to_string()
}

fn pretty_json(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("pretty json");
    bytes.push(b'\n');
    bytes
}

fn write_file(root: &TempDir, relative: &str, bytes: &[u8]) {
    let path = root.path().join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("parent directories");
    fs::write(path, bytes).expect("fixture file");
}
