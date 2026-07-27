// `support` seals evidence with the daemon's own authoritative Review input
// fingerprint, which lives in `review_authority` and needs its two siblings to
// resolve `crate::` paths.
#[allow(dead_code)]
#[path = "../src/gate_source/mod.rs"]
mod gate_source;
#[allow(dead_code)]
#[path = "../src/persistence.rs"]
mod persistence;
#[allow(dead_code)]
#[path = "../src/review_authority.rs"]
mod review_authority;

#[path = "typed_operations_cli_e2e/support.rs"]
mod support;

use std::collections::BTreeSet;
use std::fs;

use ae_sdd_contracts::{BoundedText, ExecutionId, ExecutionStepId, SchemaVersion, WorkerId};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, EvidenceDigest, InputFingerprint,
    ProjectRelativePath, WorkItemId,
};
use ae_sdd_execution::{ExecutionStep, VerificationExecutionPlan, VerificationReceipt};
use ae_sdd_protocol::{ClientKind, JobStatus, RpcMethod};
use ae_sdd_runtime::PersistencePort;
use serde_json::{Value, json};

use support::*;

const WORK_ITEM_ID: &str = "STORY-TYPED-E2E";

#[test]
fn native_trusted_job_commits_pass_receipt_manifest_and_state_atomically() {
    let mut fixture = ToolsetFixture::new();
    let plan = verification_plan();
    let receipt = plan
        .receipt(
            WorkerId::new("worker-c1").expect("worker"),
            JobStatus::Pass,
            Some(0),
            EvidenceDigest::digest(b"stdout"),
            EvidenceDigest::digest(b"stderr"),
            10,
            20,
            false,
            false,
        )
        .expect("PASS receipt");
    let plan_for_operation = serde_json::to_value(&plan).expect("plan value");

    let completed = fixture.submit(plan, receipt, "toolset-pass-job");
    assert_eq!(completed["status"], "pass", "{completed}");
    assert_eq!(completed["result"]["revisionBefore"], 1);
    assert_eq!(completed["result"]["revisionAfter"], 2);

    let job_id = completed["jobId"].as_str().expect("job id");
    let persisted = fixture
        .harness
        .persistence
        .load_job(job_id)
        .expect("job lookup")
        .expect("typed job");
    assert_eq!(persisted.source_revision, Some(2));
    assert_eq!(
        persisted.mutation_id.as_deref(),
        completed["result"]["mutationId"].as_str()
    );
    assert_eq!(
        persisted.receipt_locator.as_deref(),
        completed["result"]["receiptLocator"].as_str()
    );
    assert_eq!(
        persisted.project_receipt_digest.as_deref(),
        completed["result"]["projectReceiptDigest"].as_str()
    );

    let state: Value =
        serde_json::from_slice(&fs::read(&fixture.harness.state_path).expect("committed state"))
            .expect("state JSON");
    assert_eq!(state["revision"], 2);
    assert_eq!(state["toolsetReceiptRef"]["toolsetJobId"], job_id);
    assert_eq!(state["toolsetReceiptRef"]["sourceRevision"], 2);

    let artifact_ref = state["toolsetReceiptRef"]["artifactRef"]
        .as_str()
        .expect("artifact ref");
    let artifact_bytes = fs::read(fixture.harness.workspace_root.path().join(artifact_ref))
        .expect("immutable project receipt");
    assert_eq!(
        ArtifactDigest::digest(&artifact_bytes).to_string(),
        state["toolsetReceiptRef"]["projectReceiptDigest"]
    );
    let snapshot: Value = serde_json::from_slice(&artifact_bytes).expect("snapshot JSON");
    assert_eq!(snapshot["toolsetJobId"], job_id);
    assert_eq!(snapshot["sourceRevision"], 2);
    assert_eq!(
        snapshot["identityDigest"],
        persisted
            .identity_digest
            .as_deref()
            .expect("persisted identity digest")
    );

    let manifest_ref = state["toolsetReceiptRef"]["manifestRef"]
        .as_str()
        .expect("manifest ref");
    let manifest_bytes = fs::read(fixture.harness.workspace_root.path().join(manifest_ref))
        .expect("active manifest");
    assert_eq!(
        ArtifactDigest::digest(&manifest_bytes).to_string(),
        state["toolsetReceiptRef"]["manifestDigest"]
    );
    let manifest: Value = serde_json::from_slice(&manifest_bytes).expect("manifest JSON");
    assert_eq!(manifest["entries"][0]["status"], "active");
    assert_eq!(manifest["entries"][0]["toolsetJobId"], job_id);

    let journal = journal_snapshot(&fixture.harness)
        .into_values()
        .map(|bytes| serde_json::from_slice::<Value>(&bytes).expect("journal JSON"))
        .find(|journal| journal["operation"] == "toolset.receipt.record")
        .expect("toolset journal");
    assert_eq!(journal["status"], "COMMITTED");
    assert_eq!(journal["revisionBefore"], 1);
    assert_eq!(journal["revisionAfter"], 2);
    let target_paths = journal["targetFiles"]
        .as_array()
        .expect("target files")
        .iter()
        .map(|target| target["path"].as_str().expect("target path"))
        .collect::<BTreeSet<_>>();
    assert_eq!(target_paths.len(), 3);
    assert!(target_paths.contains(artifact_ref));
    assert!(target_paths.contains(manifest_ref));
    assert!(target_paths.contains(".auto-engineering/typed-e2e/state.json"));

    let planned = fixture.plan_verification(plan_for_operation, &completed);
    assert_eq!(planned["revisionBefore"], 2, "{planned}");
    assert_eq!(planned["revisionAfter"], 3, "{planned}");
    assert_eq!(
        planned["data"]["toolsetJobId"], completed["jobId"],
        "{planned}"
    );
    assert_eq!(
        planned["data"]["inputFingerprint"],
        completed["result"]["inputFingerprint"]
    );
}

#[test]
fn non_pass_receipt_is_bounded_audit_only_and_does_not_mutate_project() {
    let mut fixture = ToolsetFixture::new();
    let before_state = fs::read(&fixture.harness.state_path).expect("state before FAIL");
    let before_journals = journal_snapshot(&fixture.harness);
    let plan = verification_plan();
    let receipt = plan
        .receipt(
            WorkerId::new("worker-c1").expect("worker"),
            JobStatus::Fail,
            Some(1),
            EvidenceDigest::digest(b"stdout"),
            EvidenceDigest::digest(b"stderr"),
            10,
            20,
            false,
            false,
        )
        .expect("FAIL receipt");

    let completed = fixture.submit(plan, receipt, "toolset-fail-job");
    assert_eq!(completed["status"], "fail", "{completed}");
    assert_eq!(
        fs::read(&fixture.harness.state_path).expect("state after FAIL"),
        before_state
    );
    assert_eq!(journal_snapshot(&fixture.harness), before_journals);
    assert!(
        !fixture
            .harness
            .workspace_root
            .path()
            .join(format!(".auto-engineering/{WORK_ITEM_ID}/evidence"))
            .exists()
    );
}

struct ToolsetFixture {
    harness: Harness,
    connection: ae_sdd_runtime::ConnectionState,
    identity: CliIdentity,
    lease_id: String,
    fencing_token: u64,
    inventory_generation: u64,
}

impl ToolsetFixture {
    fn new() -> Self {
        let harness = Harness::new();
        let mut connection = harness.connection(ClientKind::Cli);
        let workspace = register_and_cut_over(&harness, &mut connection);
        let root = open_root(
            &harness,
            &mut connection,
            &workspace,
            "toolset-root",
            "toolset-agent",
        );
        let identity = identity(&workspace, &root, "toolset-agent");
        let acquired = success(&invoke(
            &harness,
            &mut connection,
            &identity,
            "lease acquire",
            args(&[
                "--owner",
                "{\"role\":\"root\"}",
                "--ttl-seconds",
                "300",
                "--idempotency-key",
                "toolset-project-lease",
            ]),
        ));
        Self {
            harness,
            connection,
            identity,
            lease_id: acquired["data"]["leaseId"]
                .as_str()
                .expect("lease id")
                .to_owned(),
            fencing_token: acquired["data"]["fencingToken"]
                .as_u64()
                .expect("fencing token"),
            inventory_generation: workspace.inventory_generation,
        }
    }

    fn submit(
        &mut self,
        plan: VerificationExecutionPlan,
        receipt: VerificationReceipt,
        idempotency_key: &str,
    ) -> Value {
        let mut request = trusted_params(
            &self.identity,
            json!({
                "entrypoint": "toolset.receipt.record",
                "arguments": {
                    "plan": plan,
                    "receipt": receipt,
                    "sourceRevision": 1,
                    "policyDigest": self.harness.runtime.policy_digest(),
                    "methodologyDigest": "2".repeat(64),
                    "inventoryGeneration": self.inventory_generation,
                    "leaseId": self.lease_id,
                    "fencingToken": self.fencing_token,
                },
                "deadlineUnixMs": 300_000,
            }),
        );
        request.expected_revision = Some(1);
        request.idempotency_key = Some(idempotency_key.to_owned());
        let submitted = success(&call(
            &self.harness.runtime,
            &mut self.connection,
            RpcMethod::JobSubmit,
            request,
        ));
        assert_eq!(submitted["status"], "queued", "{submitted}");
        assert!(
            self.harness
                .runtime
                .run_one_pending_job()
                .expect("trusted job executes")
        );
        let status = trusted_params(&self.identity, json!({"jobId":submitted["jobId"]}));
        success(&call(
            &self.harness.runtime,
            &mut self.connection,
            RpcMethod::JobStatus,
            status,
        ))
    }

    fn plan_verification(&mut self, plan: Value, completed: &Value) -> Value {
        let mut request = operation_params(
            &self.identity,
            "verification.plan",
            json!({
                "toolsetJobId": completed["jobId"],
                "plan": plan,
                "receiptId": completed["result"]["receiptId"],
                "receiptDigest": completed["result"]["receiptDigest"],
                "sourceRevision": 2,
                "planDigest": completed["result"]["planDigest"],
                "methodologyDigest": completed["result"]["methodologyDigest"],
                "policyDigest": completed["result"]["policyDigest"],
                "inputFingerprint": completed["result"]["inputFingerprint"],
                "changedPaths": ["src/lib.rs"],
                "persist": true,
            }),
        );
        request.lease_id = Some(self.lease_id.clone());
        request.fencing_token = Some(self.fencing_token);
        request.expected_revision = Some(2);
        request.idempotency_key = Some("verification-plan-from-toolset".to_owned());
        success(&call(
            &self.harness.runtime,
            &mut self.connection,
            RpcMethod::OperationExecute,
            request,
        ))
    }
}

fn verification_plan() -> VerificationExecutionPlan {
    let program = ArtifactRef::new(
        ArtifactKind::new("verification-program").expect("artifact kind"),
        ProjectRelativePath::new("tools/cargo.exe").expect("program path"),
        ArtifactDigest::digest(b"tools/cargo.exe"),
        1,
    );
    let step = ExecutionStep::new(
        SchemaVersion::V1,
        ExecutionStepId::new("focused-tests").expect("step id"),
        program,
        vec![BoundedText::new("test").expect("argument")],
        None,
        Vec::new(),
    )
    .expect("execution step");
    VerificationExecutionPlan::new(
        SchemaVersion::V1,
        ExecutionId::new("execution-c1").expect("execution id"),
        WorkItemId::new(WORK_ITEM_ID).expect("Work Item"),
        InputFingerprint::digest(b"c1 verification input"),
        vec![step],
    )
    .expect("verification plan")
}
