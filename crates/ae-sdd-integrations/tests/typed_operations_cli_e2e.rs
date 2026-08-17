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

#[path = "typed_operations_cli_e2e/bootstrap.rs"]
mod bootstrap;
#[path = "typed_operations_cli_e2e/governance.rs"]
mod governance;
#[path = "typed_operations_cli_e2e/memory_jobs.rs"]
mod memory_jobs;
#[path = "typed_operations_cli_e2e/support.rs"]
mod support;

use std::collections::BTreeSet;
use std::fs;

use ae_sdd_contracts::{BoundedText, ExecutionId, ExecutionStepId, SchemaVersion, WorkerId};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, EvidenceDigest, InputFingerprint,
    ProjectRelativePath, WorkItemId,
};
use ae_sdd_execution::{ExecutionStep, VerificationExecutionPlan};
use ae_sdd_protocol::{ClientKind, JobStatus, RpcMethod};
use ae_sdd_runtime::{PersistencePort, RuntimeIdentityKind};
use serde_json::{Value, json};

use support::*;

#[test]
fn delegation_identity_bundle_satisfies_sqlite_action_and_attestation_joins() {
    let harness = Harness::new();
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_and_cut_over(&harness, &mut cli);
    let root = open_root(
        &harness,
        &mut cli,
        &workspace,
        "delegation-root",
        "delegation-agent",
    );
    let root_identity = identity(&workspace, &root, "delegation-agent");

    let mut host = harness.connection(ClientKind::HostAdapter);
    let mut register = plain_params(json!({
        "adapterId":"typed-delegation-host",
        "capabilities":["create","attest"]
    }));
    register.capability_token = Some(harness.host_credential());
    register.idempotency_key = Some("typed-delegation-host-register".to_owned());
    assert_success(&call(
        &harness.runtime,
        &mut host,
        RpcMethod::HostRegister,
        register,
    ));

    let flow = success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::FlowNext,
        trusted_params(&root_identity, json!({})),
    ));
    let mut create = trusted_params(
        &root_identity,
        json!({"flowDecisionDigest":flow["decisionDigest"]}),
    );
    create.idempotency_key = Some("typed-delegation-create".to_owned());
    let created = success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::DelegationCreate,
        create,
    ));
    let delegation_id = created["delegationId"]
        .as_str()
        .expect("delegation id")
        .to_owned();
    let created_snapshot = harness
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Delegation)
        .expect("typed delegation snapshots")
        .into_iter()
        .find(|snapshot| {
            snapshot
                .delegation
                .as_ref()
                .is_some_and(|delegation| delegation.delegation_id == delegation_id)
        })
        .expect("typed create snapshot");
    assert_eq!(
        created_snapshot
            .delegation
            .as_ref()
            .expect("delegation")
            .status,
        "spawning"
    );
    assert!(created_snapshot.host_action.is_some());
    assert!(created_snapshot.attestation.is_none());

    let action = success(&call(
        &harness.runtime,
        &mut host,
        RpcMethod::HostActionNext,
        plain_params(json!({"adapterId":"typed-delegation-host"})),
    ));
    let claim_id = action["claimId"]
        .as_str()
        .expect("daemon-issued Host claim")
        .to_owned();
    let child_session_id = "00000000-0000-0000-0000-000000000711";
    let ack_id = "00000000-0000-0000-0000-000000000712";
    let mut ack = plain_params(json!({
        "adapterId":"typed-delegation-host",
        "ack":{
            "ackId":ack_id,
            "actionId":action["actionId"],
            "commandSeq":action["commandSeq"],
            "outcome":"accepted",
            "hostTaskId":"typed-host-task",
            "sessionId":child_session_id
        }
    }));
    ack.idempotency_key = Some("typed-delegation-ack".to_owned());
    assert_success(&call(
        &harness.runtime,
        &mut host,
        RpcMethod::HostActionAck,
        ack,
    ));

    let mut accept = plain_params(json!({
        "delegationId":delegation_id,
        "claimId":claim_id,
        "actionId":action["actionId"],
        "childSessionId":child_session_id,
        "expiresAtUnixMs":1_900
    }));
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.work_item_id = Some("STORY-TYPED-E2E".to_owned());
    accept.idempotency_key = Some("typed-delegation-accept".to_owned());
    let accepted = call(
        &harness.runtime,
        &mut cli,
        RpcMethod::DelegationAccept,
        accept,
    );
    assert_eq!(success(&accepted)["status"], "running");

    let accepted_snapshot = harness
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Delegation)
        .expect("typed delegation snapshots")
        .into_iter()
        .find(|snapshot| {
            snapshot
                .delegation
                .as_ref()
                .is_some_and(|delegation| delegation.delegation_id == delegation_id)
        })
        .expect("typed accept snapshot");
    assert_eq!(
        accepted_snapshot
            .session
            .as_ref()
            .expect("opening child session")
            .status,
        "opening"
    );
    let attestation = accepted_snapshot
        .attestation
        .as_ref()
        .expect("physical attestation");
    assert_eq!(attestation.physical_session_id, child_session_id);
    assert_eq!(attestation.host_ack_id, ack_id);
    assert_ne!(attestation.claim_digest, claim_id);
    let receipt_json = serde_json::to_string(&accepted_snapshot).expect("snapshot serializes");
    assert!(!receipt_json.contains(&claim_id));
    assert!(!receipt_json.contains("claimId"));

    harness
        .runtime
        .recover()
        .expect("typed delegation recovers");
    let mut child_open = plain_params(json!({
        "externalKey":"typed-series-external",
        "role":"series",
        "engaged":true,
        "delegationId":delegation_id
    }));
    child_open.workspace_id = Some(workspace.workspace_id.clone());
    child_open.work_item_id = Some("STORY-TYPED-E2E".to_owned());
    child_open.agent_id = Some("typed-series-agent".to_owned());
    child_open.session_id = Some(child_session_id.to_owned());
    child_open.idempotency_key = Some("typed-series-open".to_owned());
    assert_success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::SessionOpen,
        child_open,
    ));
}

#[test]
fn ops_execute_request_file_is_bound_to_registered_workspace_story_and_session() {
    let harness = Harness::new();
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_and_cut_over(&harness, &mut cli);
    let root = open_root(
        &harness,
        &mut cli,
        &workspace,
        "ops-file-root",
        "ops-file-agent",
    );
    let identity = identity(&workspace, &root, "ops-file-agent");
    let request_path = harness
        .workspace_root
        .path()
        .join("ops-execute-request.json");

    let request = |project: &str, project_key: &str, story: &str| {
        json!({
            "schemaVersion":"1",
            "operation":"state.next_actions",
            "project":project,
            "projectKey":project_key,
            "workItem":"STORY-TYPED-E2E",
            "story":story,
            "dryRun":false,
            "parameters":{}
        })
    };
    fs::write(
        &request_path,
        serde_json::to_vec_pretty(&request(
            &harness.workspace_root.path().to_string_lossy(),
            "typed-e2e",
            "STORY-TYPED-E2E",
        ))
        .expect("request serializes"),
    )
    .expect("request file");
    let accepted = call(
        &harness.runtime,
        &mut cli,
        ae_sdd_protocol::RpcMethod::OperationExecute,
        ops_execute_params(&identity, &request_path).expect("request adapts"),
    );
    assert_success(&accepted);

    let other_root = tempfile::TempDir::new().expect("other root");
    fs::write(
        &request_path,
        serde_json::to_vec_pretty(&request(
            &other_root.path().to_string_lossy(),
            "typed-e2e",
            "STORY-TYPED-E2E",
        ))
        .expect("request serializes"),
    )
    .expect("request file");
    let wrong_root = call(
        &harness.runtime,
        &mut cli,
        ae_sdd_protocol::RpcMethod::OperationExecute,
        ops_execute_params(&identity, &request_path).expect("request adapts"),
    );
    assert_eq!(stable_error(&wrong_root), "PROJECT_MISMATCH");

    fs::write(
        &request_path,
        serde_json::to_vec_pretty(&request(
            &harness.workspace_root.path().to_string_lossy(),
            "another-project",
            "STORY-TYPED-E2E",
        ))
        .expect("request serializes"),
    )
    .expect("request file");
    let wrong_key = call(
        &harness.runtime,
        &mut cli,
        ae_sdd_protocol::RpcMethod::OperationExecute,
        ops_execute_params(&identity, &request_path).expect("request adapts"),
    );
    assert_eq!(stable_error(&wrong_key), "PROJECT_MISMATCH");

    fs::write(
        &request_path,
        serde_json::to_vec_pretty(&request(
            &harness.workspace_root.path().to_string_lossy(),
            "typed-e2e",
            "STORY-FORGED",
        ))
        .expect("request serializes"),
    )
    .expect("request file");
    let wrong_story = call(
        &harness.runtime,
        &mut cli,
        ae_sdd_protocol::RpcMethod::OperationExecute,
        ops_execute_params(&identity, &request_path).expect("request adapts"),
    );
    assert_eq!(stable_error(&wrong_story), "PROJECT_MISMATCH");
}

#[test]
fn twelve_typed_cli_routes_execute_through_the_authoritative_runtime() {
    let harness = Harness::new();
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_and_cut_over(&harness, &mut cli);
    let first = open_root(&harness, &mut cli, &workspace, "root-one", "agent-one");
    let second = open_root(&harness, &mut cli, &workspace, "root-two", "agent-two");
    let first_identity = identity(&workspace, &first, "agent-one");
    let second_identity = identity(&workspace, &second, "agent-two");
    let mut succeeded = BTreeSet::new();

    for (command, arguments) in [
        ("state read", Vec::<String>::new()),
        ("state next-step", Vec::new()),
        ("doc resolve", args(&["--intent", "STORY"])),
        ("gates check", args(&["--gate-ids", "[\"G-00\"]"])),
        ("lease status", Vec::new()),
    ] {
        let response = invoke(&harness, &mut cli, &first_identity, command, arguments);
        assert!(response.get("result").is_some(), "{command}: {response}");
        succeeded.insert(command);
    }

    let acquire_arguments = args(&[
        "--owner",
        "{\"role\":\"root\"}",
        "--ttl-seconds",
        "300",
        "--idempotency-key",
        "lease-acquire-e2e",
    ]);
    let acquired = invoke(
        &harness,
        &mut cli,
        &first_identity,
        "lease acquire",
        acquire_arguments.clone(),
    );
    let acquire_result = success(&acquired);
    let lease_id = acquire_result["data"]["leaseId"]
        .as_str()
        .expect("lease id")
        .to_owned();
    let fencing = acquire_result["data"]["fencingToken"]
        .as_u64()
        .expect("fencing token");
    succeeded.insert("lease acquire");
    let replay = invoke(
        &harness,
        &mut cli,
        &first_identity,
        "lease acquire",
        acquire_arguments.clone(),
    );
    assert_eq!(success(&replay)["changed"], false);
    let cross_session = invoke(
        &harness,
        &mut cli,
        &second_identity,
        "lease acquire",
        acquire_arguments,
    );
    assert_eq!(stable_error(&cross_session), "IDEMPOTENCY_KEY_REUSED");

    // The root orchestrator holds no semantic-work permissions, so it releases
    // its lease and every project mutation below runs through the delegated
    // author task of a Root -> Series -> Task lineage.
    assert_success(&invoke(
        &harness,
        &mut cli,
        &first_identity,
        "lease release",
        lease_args(&lease_id, fencing, "lease-release-root-e2e", false, "root"),
    ));
    let (author, _reviewer) = open_review_lineage(
        &harness,
        &mut cli,
        &workspace,
        &first_identity,
        "general",
        "typed-lineage",
    );
    let author_identity = identity(&workspace, &author, "typed-lineage-author-agent");
    let author_acquired = success(&invoke(
        &harness,
        &mut cli,
        &author_identity,
        "lease acquire",
        args(&[
            "--owner",
            "{\"role\":\"task\"}",
            "--ttl-seconds",
            "300",
            "--idempotency-key",
            "lease-acquire-author-e2e",
        ]),
    ));
    let lease_id = author_acquired["data"]["leaseId"]
        .as_str()
        .expect("author lease id")
        .to_owned();
    let fencing = author_acquired["data"]["fencingToken"]
        .as_u64()
        .expect("author fencing token");

    let before_state = fs::read(&harness.state_path).expect("state before dry-run");
    let before_document = fs::read(&harness.document_path).expect("document before dry-run");
    let lease_path = harness
        .workspace_root
        .path()
        .join(".auto-engineering/typed-e2e/state.lease.json");
    let before_lease = fs::read(&lease_path).expect("lease before dry-run");
    let before_events = harness
        .persistence
        .latest_event_sequence()
        .expect("event cursor");
    let before_journals = journal_snapshot(&harness);
    let dry_run = invoke(
        &harness,
        &mut cli,
        &author_identity,
        "doc save",
        write_args(
            &lease_id,
            fencing,
            1,
            "doc-save-e2e",
            &[
                "--intent",
                "STORY",
                "--doc-id",
                "STORY-TYPED-E2E",
                "--content-file",
                "draft/story.md",
                "--dry-run",
            ],
        ),
    );
    let dry_result = success(&dry_run);
    assert_eq!(dry_result["changed"], false);
    assert_eq!(dry_result["revisionBefore"], 1);
    assert_eq!(dry_result["revisionAfter"], 1);
    assert!(dry_result["receiptDigest"].is_null());
    assert_eq!(dry_result["data"]["dryRun"], true);
    assert_eq!(
        fs::read(&harness.state_path).expect("state after dry-run"),
        before_state
    );
    assert_eq!(
        fs::read(&harness.document_path).expect("document after dry-run"),
        before_document
    );
    assert_eq!(
        fs::read(&lease_path).expect("lease after dry-run"),
        before_lease
    );
    assert_eq!(journal_snapshot(&harness), before_journals);
    assert_eq!(
        harness
            .persistence
            .latest_event_sequence()
            .expect("event cursor after dry-run"),
        before_events
    );

    let saved = invoke(
        &harness,
        &mut cli,
        &author_identity,
        "doc save",
        write_args(
            &lease_id,
            fencing,
            1,
            "doc-save-e2e",
            &[
                "--intent",
                "STORY",
                "--doc-id",
                "STORY-TYPED-E2E",
                "--content-file",
                "draft/story.md",
            ],
        ),
    );
    assert_eq!(success(&saved)["revisionAfter"], 2);
    assert_eq!(
        fs::read_to_string(&harness.document_path).expect("saved document"),
        "# updated story\n"
    );
    succeeded.insert("doc save");

    let stale = invoke(
        &harness,
        &mut cli,
        &author_identity,
        "evidence record",
        write_args(
            &lease_id,
            0,
            999,
            "evidence-stale-e2e",
            &[
                "--artifact-path",
                "evidence/result.json",
                "--input-fingerprint",
                "input-e2e",
            ],
        ),
    );
    assert_eq!(stable_error(&stale), "STALE_FENCING_TOKEN");
    let revision_conflict = invoke(
        &harness,
        &mut cli,
        &author_identity,
        "evidence record",
        write_args(
            &lease_id,
            fencing,
            999,
            "evidence-revision-e2e",
            &[
                "--artifact-path",
                "evidence/result.json",
                "--input-fingerprint",
                "input-e2e",
            ],
        ),
    );
    assert_eq!(stable_error(&revision_conflict), "REVISION_CONFLICT");

    let evidence_arguments = write_args(
        &lease_id,
        fencing,
        2,
        "evidence-record-e2e",
        &[
            "--artifact-path",
            "evidence/result.json",
            "--input-fingerprint",
            "input-e2e",
        ],
    );
    let evidence = invoke(
        &harness,
        &mut cli,
        &author_identity,
        "evidence record",
        evidence_arguments.clone(),
    );
    assert_eq!(success(&evidence)["revisionAfter"], 3);
    let evidence_data = success(&evidence)["data"].clone();
    let snapshot = evidence_data["artifacts"][0]["snapshotPath"]
        .as_str()
        .expect("snapshot path");
    assert!(harness.workspace_root.path().join(snapshot).is_file());
    let replayed_evidence = invoke(
        &harness,
        &mut cli,
        &author_identity,
        "evidence record",
        evidence_arguments,
    );
    assert_eq!(success(&replayed_evidence)["changed"], false);
    assert_eq!(success(&replayed_evidence)["data"], evidence_data);
    succeeded.insert("evidence record");
    let finalized = invoke(
        &harness,
        &mut cli,
        &author_identity,
        "evidence finalize",
        write_args(&lease_id, fencing, 3, "evidence-finalize-e2e", &[]),
    );
    assert_eq!(success(&finalized)["revisionAfter"], 4);
    assert_eq!(success(&finalized)["data"]["entryCount"], 1);
    let manifest_path = harness
        .workspace_root
        .path()
        .join(".auto-engineering/STORY-TYPED-E2E/evidence/manifest.json");
    let manifest: Value =
        serde_json::from_slice(&fs::read(manifest_path).expect("manifest")).expect("manifest JSON");
    assert!(
        manifest["contentHash"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(manifest["entries"][0]["status"], "active");
    succeeded.insert("evidence finalize");

    let verification_plan = typed_cli_verification_plan();
    let verification_plan_value =
        serde_json::to_value(&verification_plan).expect("verification plan serializes");
    let verification_receipt = verification_plan
        .receipt(
            WorkerId::new("typed-e2e-worker").expect("worker id"),
            JobStatus::Pass,
            Some(0),
            EvidenceDigest::digest(b"typed e2e stdout"),
            EvidenceDigest::digest(b"typed e2e stderr"),
            10,
            20,
            false,
            false,
        )
        .expect("PASS verification receipt");
    let mut receipt_job = trusted_params(
        &author_identity,
        json!({
            "entrypoint": "toolset.receipt.record",
            "arguments": {
                "plan": verification_plan,
                "receipt": verification_receipt,
                "sourceRevision": 4,
                "policyDigest": harness.runtime.policy_digest(),
                "methodologyDigest": "2".repeat(64),
                "inventoryGeneration": workspace.inventory_generation,
                "leaseId": lease_id,
                "fencingToken": fencing,
            },
            "deadlineUnixMs": 300_000,
        }),
    );
    receipt_job.expected_revision = Some(4);
    receipt_job.idempotency_key = Some("toolset-receipt-e2e".to_owned());
    let submitted = success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::JobSubmit,
        receipt_job,
    ));
    assert_eq!(submitted["status"], "queued", "{submitted}");
    assert!(
        harness
            .runtime
            .run_one_pending_job()
            .expect("toolset receipt job executes")
    );
    let completed = success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::JobStatus,
        trusted_params(&author_identity, json!({"jobId": submitted["jobId"]})),
    ));
    assert_eq!(completed["status"], "pass", "{completed}");
    assert_eq!(completed["result"]["revisionBefore"], 4);
    assert_eq!(completed["result"]["revisionAfter"], 5);

    let verification_payload = serde_json::to_string(&json!({
        "toolsetJobId": completed["jobId"],
        "plan": verification_plan_value,
        "receiptId": completed["result"]["receiptId"],
        "receiptDigest": completed["result"]["receiptDigest"],
        "sourceRevision": 5,
        "planDigest": completed["result"]["planDigest"],
        "methodologyDigest": completed["result"]["methodologyDigest"],
        "policyDigest": completed["result"]["policyDigest"],
        "inputFingerprint": completed["result"]["inputFingerprint"],
        "changedPaths": ["src/lib.rs"],
        "persist": true,
    }))
    .expect("verification payload serializes");
    let planned = invoke(
        &harness,
        &mut cli,
        &author_identity,
        "verify plan",
        write_args(
            &lease_id,
            fencing,
            5,
            "verification-plan-e2e",
            &["--payload-json", &verification_payload],
        ),
    );
    assert_eq!(success(&planned)["revisionAfter"], 6);
    assert_eq!(
        success(&planned)["data"]["changeClass"],
        json!(["production-code"])
    );
    assert_eq!(
        success(&planned)["data"]["inputFingerprint"],
        success(&planned)["data"]["evidenceInputFingerprint"]
    );
    succeeded.insert("verify plan");

    let renewed = invoke(
        &harness,
        &mut cli,
        &author_identity,
        "lease renew",
        lease_args(&lease_id, fencing, "lease-renew-e2e", true, "task"),
    );
    assert_success(&renewed);
    succeeded.insert("lease renew");
    let active = invoke(
        &harness,
        &mut cli,
        &author_identity,
        "lease status",
        Vec::new(),
    );
    assert_eq!(success(&active)["data"]["active"], true);
    let release_arguments = lease_args(&lease_id, fencing, "lease-release-e2e", false, "task");
    let released = invoke(
        &harness,
        &mut cli,
        &author_identity,
        "lease release",
        release_arguments.clone(),
    );
    assert_eq!(success(&released)["data"]["status"], "released");
    let release_replay = invoke(
        &harness,
        &mut cli,
        &author_identity,
        "lease release",
        release_arguments,
    );
    assert_eq!(success(&release_replay)["changed"], false);
    succeeded.insert("lease release");

    assert_eq!(
        succeeded,
        BTreeSet::from([
            "doc resolve",
            "doc save",
            "evidence finalize",
            "evidence record",
            "gates check",
            "lease acquire",
            "lease release",
            "lease renew",
            "lease status",
            "state next-step",
            "state read",
            "verify plan",
        ])
    );

    let break_without_confirmation = route_params(
        &first_identity,
        "lease break",
        args(&[
            "--actor",
            "{\"role\":\"admin\"}",
            "--reason",
            "recovery",
            "--idempotency-key",
            "lease-break-e2e",
        ]),
    )
    .expect_err("lease break requires confirmation before IPC");
    assert!(break_without_confirmation.contains("confirmation"));
    let denied_break = invoke(
        &harness,
        &mut cli,
        &first_identity,
        "lease break",
        args(&[
            "--actor",
            "{\"role\":\"admin\"}",
            "--reason",
            "recovery",
            "--idempotency-key",
            "lease-break-e2e-confirmed",
            "--confirmation-id",
            "confirmation-e2e",
            "--approved-by",
            "user:test",
            "--approved-at",
            "2026-07-23T00:00:00Z",
        ]),
    );
    assert_eq!(stable_error(&denied_break), "ROLE_OPERATION_FORBIDDEN");
}

fn typed_cli_verification_plan() -> VerificationExecutionPlan {
    let program = "tools/cargo.exe";
    let program_ref = ArtifactRef::new(
        ArtifactKind::new("verification-program").expect("program kind"),
        ProjectRelativePath::new(program).expect("program path"),
        ArtifactDigest::digest(program.as_bytes()),
        1,
    );
    let step = ExecutionStep::new(
        SchemaVersion::V1,
        ExecutionStepId::new("typed-e2e-tests").expect("step id"),
        program_ref,
        vec![BoundedText::new("test").expect("argument")],
        None,
        Vec::new(),
    )
    .expect("execution step");
    let binding = json!({
        "storyId": "STORY-TYPED-E2E",
        "workItem": "STORY-TYPED-E2E",
        "changedPaths": ["src/lib.rs"],
        "sinceFingerprint": "",
    });
    VerificationExecutionPlan::new(
        SchemaVersion::V1,
        ExecutionId::new("execution-typed-e2e").expect("execution id"),
        WorkItemId::new("STORY-TYPED-E2E").expect("work item"),
        InputFingerprint::digest(serde_json::to_vec(&binding).expect("binding serializes")),
        vec![step],
    )
    .expect("verification execution plan")
}

#[test]
fn lease_acquire_dry_run_writes_no_ledger_journal_event_or_receipt() {
    let harness = Harness::new();
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_and_cut_over(&harness, &mut cli);
    let root = open_root(&harness, &mut cli, &workspace, "root-dry", "agent-dry");
    let identity = identity(&workspace, &root, "agent-dry");
    let lease_path = harness
        .workspace_root
        .path()
        .join(".auto-engineering/typed-e2e/state.lease.json");
    let before_events = harness
        .persistence
        .latest_event_sequence()
        .expect("event cursor");
    let before_journals = journal_snapshot(&harness);
    let mut acquire = args(&[
        "--owner",
        "{\"role\":\"root\"}",
        "--ttl-seconds",
        "300",
        "--idempotency-key",
        "lease-acquire-dry-e2e",
    ]);
    acquire.push("--dry-run".to_owned());

    let preview = success(&invoke(
        &harness,
        &mut cli,
        &identity,
        "lease acquire",
        acquire,
    ));
    assert_eq!(preview["changed"], false);
    assert_eq!(preview["revisionBefore"], 1);
    assert_eq!(preview["revisionAfter"], 1);
    assert!(preview["receiptDigest"].is_null());
    assert_eq!(preview["data"]["dryRun"], true);
    assert!(!lease_path.exists());
    assert_eq!(journal_snapshot(&harness), before_journals);
    assert_eq!(
        harness
            .persistence
            .latest_event_sequence()
            .expect("event cursor after preview"),
        before_events
    );

    let committed = success(&invoke(
        &harness,
        &mut cli,
        &identity,
        "lease acquire",
        args(&[
            "--owner",
            "{\"role\":\"root\"}",
            "--ttl-seconds",
            "300",
            "--idempotency-key",
            "lease-acquire-dry-e2e",
        ]),
    ));
    assert_eq!(committed["changed"], true);
    assert!(committed["receiptDigest"].is_string());
    assert!(lease_path.is_file());
}

#[test]
fn admin_lease_break_requires_no_agent_session_and_is_durable() {
    let harness = Harness::new();
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_and_cut_over(&harness, &mut cli);
    let root = open_root(&harness, &mut cli, &workspace, "root-break", "agent-break");
    let identity = identity(&workspace, &root, "agent-break");
    let acquired = invoke(
        &harness,
        &mut cli,
        &identity,
        "lease acquire",
        args(&[
            "--owner",
            "{\"role\":\"root\"}",
            "--ttl-seconds",
            "300",
            "--idempotency-key",
            "lease-acquire-admin-break-e2e",
        ]),
    );
    assert!(success(&acquired)["data"]["leaseId"].as_str().is_some());
    let lease_path = harness
        .workspace_root
        .path()
        .join(".auto-engineering/typed-e2e/state.lease.json");
    let before_ledger = fs::read(&lease_path).expect("active ledger");
    let before_journals = journal_snapshot(&harness);
    let before_events = harness
        .persistence
        .latest_event_sequence()
        .expect("event cursor");
    let confirmed = [
        "--actor",
        "{\"claimedBy\":\"operator\"}",
        "--reason",
        "owner process is no longer alive",
        "--idempotency-key",
        "lease-break-admin-e2e",
        "--confirmation-id",
        "confirmation-admin-break",
        "--approved-by",
        "user:test",
        "--approved-at",
        "2026-07-23T00:00:00Z",
    ];
    let mut preview_args = args(&confirmed);
    preview_args.push("--dry-run".to_owned());
    let mut preview = route_params(&identity, "lease break", preview_args).expect("preview params");
    preview.session_id = None;
    preview.capability_token = None;
    preview.agent_id = None;
    let mut admin = harness.connection(ClientKind::Admin);
    let preview = success(&raw_call(
        &harness.runtime,
        &mut admin,
        ae_sdd_protocol::RpcMethod::OperationExecute,
        serde_json::to_value(preview).expect("preview wire"),
    ));
    assert_eq!(preview["changed"], false);
    assert_eq!(preview["data"]["dryRun"], true);
    assert_eq!(
        fs::read(&lease_path).expect("ledger after preview"),
        before_ledger
    );
    assert_eq!(journal_snapshot(&harness), before_journals);
    assert_eq!(
        harness
            .persistence
            .latest_event_sequence()
            .expect("event cursor after preview"),
        before_events
    );

    let mut request =
        route_params(&identity, "lease break", args(&confirmed)).expect("break params");
    request.session_id = None;
    request.capability_token = None;
    request.agent_id = None;
    let wire = serde_json::to_value(&request).expect("break wire");
    let broken = success(&raw_call(
        &harness.runtime,
        &mut admin,
        ae_sdd_protocol::RpcMethod::OperationExecute,
        wire.clone(),
    ));
    assert_eq!(broken["data"]["broken"], true);
    assert_eq!(broken["data"]["actor"], "admin:authenticated-local-client");
    assert_eq!(broken["data"]["reason"], "owner process is no longer alive");
    assert!(broken["data"]["ledgerBeforeDigest"].as_str().is_some());
    assert!(broken["data"]["ledgerAfterDigest"].as_str().is_some());
    let replay = success(&raw_call(
        &harness.runtime,
        &mut admin,
        ae_sdd_protocol::RpcMethod::OperationExecute,
        wire,
    ));
    assert_eq!(replay["changed"], false);
    assert_eq!(replay["data"], broken["data"]);
}
