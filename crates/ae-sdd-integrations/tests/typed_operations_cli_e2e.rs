#[path = "typed_operations_cli_e2e/governance.rs"]
mod governance;
#[path = "typed_operations_cli_e2e/memory_jobs.rs"]
mod memory_jobs;
#[path = "typed_operations_cli_e2e/support.rs"]
mod support;

use std::collections::BTreeSet;
use std::fs;

use ae_sdd_protocol::ClientKind;
use ae_sdd_runtime::PersistencePort;
use serde_json::{Value, json};

use support::*;

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
        assert_success(&invoke(
            &harness,
            &mut cli,
            &first_identity,
            command,
            arguments,
        ));
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
        &first_identity,
        "doc save",
        write_args(
            &lease_id,
            fencing,
            1,
            "doc-save-e2e",
            &[
                "--intent",
                "STORY",
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
        &first_identity,
        "doc save",
        write_args(
            &lease_id,
            fencing,
            1,
            "doc-save-e2e",
            &["--intent", "STORY", "--content-file", "draft/story.md"],
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
        &first_identity,
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
        &first_identity,
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
        &first_identity,
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
        &first_identity,
        "evidence record",
        evidence_arguments,
    );
    assert_eq!(success(&replayed_evidence)["changed"], false);
    assert_eq!(success(&replayed_evidence)["data"], evidence_data);
    succeeded.insert("evidence record");
    let finalized = invoke(
        &harness,
        &mut cli,
        &first_identity,
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
    let planned = invoke(
        &harness,
        &mut cli,
        &first_identity,
        "verify plan",
        write_args(
            &lease_id,
            fencing,
            4,
            "verification-plan-e2e",
            &["--changed-paths", "[\"src/lib.rs\"]"],
        ),
    );
    assert_eq!(success(&planned)["revisionAfter"], 5);
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
        &first_identity,
        "lease renew",
        lease_args(&lease_id, fencing, "lease-renew-e2e", true),
    );
    assert_success(&renewed);
    succeeded.insert("lease renew");
    let active = invoke(
        &harness,
        &mut cli,
        &first_identity,
        "lease status",
        Vec::new(),
    );
    assert_eq!(success(&active)["data"]["active"], true);
    let release_arguments = lease_args(&lease_id, fencing, "lease-release-e2e", false);
    let released = invoke(
        &harness,
        &mut cli,
        &first_identity,
        "lease release",
        release_arguments.clone(),
    );
    assert_eq!(success(&released)["data"]["status"], "released");
    let release_replay = invoke(
        &harness,
        &mut cli,
        &first_identity,
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
