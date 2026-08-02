use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ae_sdd_client::{ClientTransport, DaemonClient, LocalIpcTransport};
use ae_sdd_contracts::{BoundedText, ExecutionId, ExecutionStepId, SchemaVersion, WorkerId};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, EvidenceDigest, InputFingerprint,
    ProjectRelativePath, WorkItemId,
};
use ae_sdd_execution::{ExecutionStep, VerificationExecutionPlan};
use ae_sdd_integrations::{RuntimePaths, SqliteRuntimePersistence};
use ae_sdd_protocol::{
    ClientKind, ConfirmationRef, JobStatus, PROTOCOL_VERSION_V1, RequestParams, RpcMethod,
    StableErrorCode, WorkspaceMode,
};
use ae_sdd_runtime::{
    PersistencePort, RuntimeJobRecord, RuntimeJobStatus, SessionResult, WorkspaceParityEvidence,
    WorkspaceResult,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

const WORK_ITEM_ID: &str = "STORY-C1-PROCESS-E2E";
const PROJECT_KEY: &str = "c1-process-e2e";
const AGENT_ID: &str = "c1-process-root";
const EXTERNAL_SESSION_KEY: &str = "c1-process-root-session";
const COMMIT_ABORT_ENV: &str = "AE_SDD_TEST_COMMIT_ABORT_AT";
const ABORT_AFTER_PREPARED: &str = "after_prepared";
const ABORT_AFTER_REPLACE_0: &str = "after_replace_0";
const REVIEW_WORK_ITEM_ID: &str = "STORY-C1-REVIEW-PROCESS";
const REVIEW_PROJECT_KEY: &str = "c1-review-process";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_process_commits_and_replays_toolset_verification_exactly_once() {
    let fixture = Fixture::new();
    let policy_digest = "b".repeat(64);
    let mut daemon =
        DaemonProcess::start(&fixture.runtime_dir, fixture.allowed_root(), &policy_digest).await;
    let cli = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let admin = daemon_client(&fixture.runtime_dir, ClientKind::Admin);

    let workspace = register_and_cut_over(&cli, &admin, &fixture).await;
    let first_session = open_root(&cli, &workspace, "session-open-first").await;
    let first_identity = Identity::for_work_item(
        &workspace,
        &first_session,
        WORK_ITEM_ID,
        AGENT_ID,
        fixture.state_path.clone(),
    );
    // `verification.plan` and the receipt job are semantic work, so a
    // delegated author task drives them and owns the project lease; the
    // tightened root role only opens the delegation lineage.
    let first_lineage = open_review_lineage(
        &cli,
        &fixture.runtime_dir,
        &workspace,
        &first_identity,
        "verification-lineage-first",
    )
    .await;
    let (lease_id, fencing_token) = acquire_lease(
        &cli,
        &first_lineage.author,
        "task",
        "verification-lease-first",
    )
    .await;

    let plan = verification_plan();
    let plan_value = serde_json::to_value(&plan).expect("verification plan serializes");
    let before_forgery = fs::read(&fixture.state_path).expect("state before forged authority");
    let forged = verification_request(
        &first_lineage.author,
        &lease_id,
        fencing_token,
        1,
        "forged-verification-plan",
        json!({
            "toolsetJobId":"forged-job",
            "plan":plan_value.clone(),
            "receiptId":"forged-receipt",
            "receiptDigest":"0".repeat(64),
            "sourceRevision":1,
            "planDigest":"1".repeat(64),
            "methodologyDigest":"2".repeat(64),
            "policyDigest":policy_digest.clone(),
            "inputFingerprint":plan_value["inputFingerprint"],
            "changedPaths":["src/lib.rs"],
            "persist":true,
        }),
    );
    let error = cli
        .call::<Value>(RpcMethod::OperationExecute, forged)
        .await
        .expect_err("state-injected PASS must not manufacture job authority");
    assert_eq!(error.stable_code(), StableErrorCode::GateBlocked);
    assert_eq!(
        fs::read(&fixture.state_path).expect("state after forged authority"),
        before_forgery,
        "rejected state injection must not mutate project authority"
    );

    let receipt = plan
        .receipt(
            WorkerId::new("c1-process-worker").expect("worker id"),
            JobStatus::Pass,
            Some(0),
            EvidenceDigest::digest(b"process stdout"),
            EvidenceDigest::digest(b"process stderr"),
            10,
            20,
            false,
            false,
        )
        .expect("PASS receipt matches the plan");
    let job_payload = json!({
        "entrypoint":"toolset.receipt.record",
        "arguments":{
            "plan":plan,
            "receipt":receipt,
            "sourceRevision":1,
            "policyDigest":policy_digest.clone(),
            "methodologyDigest":"2".repeat(64),
            "inventoryGeneration":workspace.inventory_generation,
            "leaseId":lease_id.clone(),
            "fencingToken":fencing_token,
        },
        "deadlineUnixMs":now_unix_ms().saturating_add(60_000),
    });
    let submitted = cli
        .call::<Value>(
            RpcMethod::JobSubmit,
            job_submit_request(
                &first_lineage.author,
                job_payload.clone(),
                1,
                "toolset-receipt-job-process-e2e",
            ),
        )
        .await
        .expect("toolset receipt job is accepted over framed RPC");
    let job_id = submitted["jobId"]
        .as_str()
        .expect("submitted job id")
        .to_owned();
    let completed = wait_for_job(&cli, &first_lineage.author, &job_id).await;
    assert_eq!(completed["status"], "pass", "{completed}");
    assert_eq!(completed["result"]["revisionBefore"], 1);
    assert_eq!(completed["result"]["revisionAfter"], 2);

    let same_boot_replay = cli
        .call::<Value>(
            RpcMethod::JobSubmit,
            job_submit_request(
                &first_lineage.author,
                job_payload,
                1,
                "toolset-receipt-job-process-e2e",
            ),
        )
        .await
        .expect("same trusted job submission replays");
    assert_eq!(same_boot_replay, completed);

    let verification_payload = json!({
        "toolsetJobId":job_id.clone(),
        "plan":plan_value,
        "receiptId":completed["result"]["receiptId"],
        "receiptDigest":completed["result"]["receiptDigest"],
        "sourceRevision":completed["result"]["sourceRevision"],
        "planDigest":completed["result"]["planDigest"],
        "methodologyDigest":completed["result"]["methodologyDigest"],
        "policyDigest":completed["result"]["policyDigest"],
        "inputFingerprint":completed["result"]["inputFingerprint"],
        "changedPaths":["src/lib.rs"],
        "persist":true,
    });
    let committed = cli
        .call::<Value>(
            RpcMethod::OperationExecute,
            verification_request(
                &first_lineage.author,
                &lease_id,
                fencing_token,
                2,
                "verification-plan-process-e2e",
                verification_payload.clone(),
            ),
        )
        .await
        .expect("verification.plan commits from durable job authority");
    assert_eq!(committed["changed"], true, "{committed}");
    assert_eq!(committed["revisionBefore"], 2);
    assert_eq!(committed["revisionAfter"], 3);

    let state_after_commit = fs::read(&fixture.state_path).expect("committed state");
    let state: Value = serde_json::from_slice(&state_after_commit).expect("state JSON");
    assert_eq!(state["revision"], 3);
    assert_eq!(state["toolsetReceiptRef"]["toolsetJobId"], job_id);
    assert_eq!(state["verificationPlan"]["toolsetJobId"], job_id);
    let journals_after_commit = journal_snapshot(&fixture.project_root);
    assert_committed_once(&journals_after_commit, "toolset.receipt.record");
    assert_committed_once(&journals_after_commit, "verification.plan");

    let before_restart_status = runtime_status(&cli).await;
    let event_store_id = before_restart_status["eventStoreId"]
        .as_str()
        .expect("event store id")
        .to_owned();
    let events_before_restart = events(&cli, &first_identity, &event_store_id).await;
    assert_event_once(&events_before_restart, "toolset.receipt.record");
    assert_event_once(&events_before_restart, "verification.plan");

    daemon.crash();
    let mut restarted =
        DaemonProcess::start(&fixture.runtime_dir, fixture.allowed_root(), &policy_digest).await;
    let restarted_cli = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let after_restart_status = runtime_status(&restarted_cli).await;
    assert_ne!(
        before_restart_status["bootId"], after_restart_status["bootId"],
        "a real process restart must rotate boot identity"
    );
    assert_eq!(after_restart_status["eventStoreId"], event_store_id);

    let old_capability = restarted_cli
        .call::<Value>(
            RpcMethod::JobStatus,
            job_status_request(&first_lineage.author, &job_id),
        )
        .await
        .expect_err("the old boot capability must fail closed");
    assert_eq!(
        old_capability.stable_code(),
        StableErrorCode::SessionExpired
    );

    let second_session = open_root(&restarted_cli, &workspace, "session-open-restart").await;
    assert_eq!(second_session.session_id, first_session.session_id);
    assert_ne!(
        second_session.capability_token, first_session.capability_token,
        "restart must re-sign the stable session identity"
    );
    let second_identity = Identity::for_work_item(
        &workspace,
        &second_session,
        WORK_ITEM_ID,
        AGENT_ID,
        fixture.state_path.clone(),
    );
    // A delegated child session is bound to the accepting daemon boot, so the
    // author lineage is re-established under the new boot identity before the
    // same-key replay. The committed job stays bound to the first author's
    // physical session, which no valid session can re-assume after the
    // restart, so job survival is verified from the durable runtime store
    // once the daemon is down instead of the session-bound job.status RPC.
    let second_lineage = open_review_lineage(
        &restarted_cli,
        &fixture.runtime_dir,
        &workspace,
        &second_identity,
        "verification-lineage-restart",
    )
    .await;
    assert_ne!(
        second_lineage.author_session_id, first_lineage.author_session_id,
        "a rebuilt lineage must be a new physical author session"
    );

    let replayed = restarted_cli
        .call::<Value>(
            RpcMethod::OperationExecute,
            verification_request(
                &second_lineage.author,
                &lease_id,
                fencing_token,
                2,
                "verification-plan-process-e2e",
                verification_payload,
            ),
        )
        .await
        .expect("committed verification operation replays after restart");
    assert_eq!(replayed["changed"], false, "{replayed}");
    assert_eq!(replayed["revisionAfter"], 3);
    assert_eq!(
        fs::read(&fixture.state_path).expect("state after replay"),
        state_after_commit
    );
    assert_eq!(
        journal_snapshot(&fixture.project_root),
        journals_after_commit
    );

    let events_after_replay = events(&restarted_cli, &second_identity, &event_store_id).await;
    assert_event_once(&events_after_replay, "toolset.receipt.record");
    assert_event_once(&events_after_replay, "verification.plan");
    restarted.crash();

    let recovered_job =
        persisted_job_by_submission_key(&fixture.runtime_dir, "toolset-receipt-job-process-e2e");
    assert_eq!(
        serde_json::to_value(&recovered_job).expect("durable job serializes"),
        completed,
        "the committed PASS job survives the restart unchanged"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_process_aborts_unapplied_prepared_receipt_without_fake_pass() {
    let fixture = Fixture::new();
    let policy_digest = "b".repeat(64);
    let crashed = crash_during_toolset_receipt(
        &fixture,
        &policy_digest,
        ABORT_AFTER_PREPARED,
        "prepared-abort-job",
    )
    .await;

    assert_eq!(
        fs::read(&fixture.state_path).expect("state after PREPARED crash"),
        crashed.state_before,
        "PREPARED without a target replace must not activate project authority"
    );
    let prepared_journals = journal_snapshot(&fixture.project_root);
    let prepared =
        single_journal_with_status(&prepared_journals, "toolset.receipt.record", "PREPARED");
    assert_target_progress(&fixture.project_root, &prepared, 0, false);

    let mut restarted =
        DaemonProcess::start(&fixture.runtime_dir, fixture.allowed_root(), &policy_digest).await;
    let client = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    // The crashed job stays bound to the author session that died with the
    // commit-abort boot, so the stale diagnosis reads the durable runtime
    // store: job.status would reject every session the new boot can open.
    assert_session_expired_job(&live_job_wire(&fixture, &crashed.job_id));

    let session = open_root(&client, &crashed.workspace, "prepared-restart-session").await;
    let identity = Identity::for_work_item(
        &crashed.workspace,
        &session,
        WORK_ITEM_ID,
        AGENT_ID,
        fixture.state_path.clone(),
    );
    // The crashed author's lease is orphaned with its boot, so it is broken
    // through the confirmed Admin path before the rebuilt author acquires
    // the lease its job lease proof binds.
    break_orphaned_lease(
        &daemon_client(&fixture.runtime_dir, ClientKind::Admin),
        &crashed.workspace,
        WORK_ITEM_ID,
        "prepared-recovery-break",
    )
    .await;
    let lineage = open_review_lineage(
        &client,
        &fixture.runtime_dir,
        &crashed.workspace,
        &identity,
        "prepared-recovery-lineage",
    )
    .await;
    let (lease_id, fencing_token) =
        acquire_lease(&client, &lineage.author, "task", "prepared-recovery-lease").await;

    let recovery_payload = toolset_receipt_job_payload(
        crashed.plan.clone(),
        crashed.receipt.clone(),
        1,
        &policy_digest,
        &crashed.workspace,
        &lease_id,
        fencing_token,
    );
    let submitted = client
        .call::<Value>(
            RpcMethod::JobSubmit,
            job_submit_request(
                &lineage.author,
                recovery_payload,
                1,
                "prepared-recovery-job",
            ),
        )
        .await
        .expect("a fresh job triggers project recovery and is accepted");
    let recovered_job_id = submitted["jobId"]
        .as_str()
        .expect("recovery job id")
        .to_owned();
    let completed = wait_for_job(&client, &lineage.author, &recovered_job_id).await;
    assert_eq!(completed["status"], "pass", "{completed}");
    assert_eq!(completed["result"]["revisionBefore"], 1);
    assert_eq!(completed["result"]["revisionAfter"], 2);

    let recovered_journals = journal_snapshot(&fixture.project_root);
    assert_journal_status_count(&recovered_journals, "toolset.receipt.record", "ABORTED", 1);
    assert_journal_status_count(
        &recovered_journals,
        "toolset.receipt.record",
        "COMMITTED",
        1,
    );
    let aborted =
        single_journal_with_status(&recovered_journals, "toolset.receipt.record", "ABORTED");
    assert_eq!(
        aborted["abortReason"],
        "ABORTED_RESTART: no target was applied"
    );

    let state_after_receipt: Value = serde_json::from_slice(
        &fs::read(&fixture.state_path).expect("state after recovered receipt"),
    )
    .expect("recovered state JSON");
    assert_eq!(state_after_receipt["revision"], 2);
    assert_eq!(
        state_after_receipt["toolsetReceiptRef"]["toolsetJobId"],
        recovered_job_id
    );

    let verification_payload = verification_payload_from_job(&completed, crashed.plan);
    let committed = client
        .call::<Value>(
            RpcMethod::OperationExecute,
            verification_request(
                &lineage.author,
                &lease_id,
                fencing_token,
                2,
                "prepared-recovery-verification",
                verification_payload,
            ),
        )
        .await
        .expect("fresh recovered authority can commit verification.plan");
    assert_eq!(committed["changed"], true, "{committed}");
    assert_eq!(committed["revisionAfter"], 3);

    let events = events(&client, &identity, &crashed.event_store_id).await;
    assert_event_once(&events, "job.stale");
    assert_event_once(&events, "toolset.receipt.record");
    assert_event_once(&events, "verification.plan");
    restarted.crash();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_process_completes_partial_replace_but_keeps_runtime_job_non_pass() {
    let fixture = Fixture::new();
    let policy_digest = "b".repeat(64);
    let crashed = crash_during_toolset_receipt(
        &fixture,
        &policy_digest,
        ABORT_AFTER_REPLACE_0,
        "replace-abort-job",
    )
    .await;

    assert_eq!(
        fs::read(&fixture.state_path).expect("state after first target crash"),
        crashed.state_before,
        "state is ordered last and must remain at the old revision"
    );
    let prepared_journals = journal_snapshot(&fixture.project_root);
    let prepared =
        single_journal_with_status(&prepared_journals, "toolset.receipt.record", "PREPARED");
    assert_target_progress(&fixture.project_root, &prepared, 1, true);

    let mut restarted =
        DaemonProcess::start(&fixture.runtime_dir, fixture.allowed_root(), &policy_digest).await;
    let client = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    // The crashed job stays bound to the author session that died with the
    // commit-abort boot, so the stale diagnosis reads the durable runtime
    // store: job.status would reject every session the new boot can open.
    assert_session_expired_job(&live_job_wire(&fixture, &crashed.job_id));

    let session = open_root(&client, &crashed.workspace, "replace-restart-session").await;
    let identity = Identity::for_work_item(
        &crashed.workspace,
        &session,
        WORK_ITEM_ID,
        AGENT_ID,
        fixture.state_path.clone(),
    );
    break_orphaned_lease(
        &daemon_client(&fixture.runtime_dir, ClientKind::Admin),
        &crashed.workspace,
        WORK_ITEM_ID,
        "replace-recovery-break",
    )
    .await;
    let lineage = open_review_lineage(
        &client,
        &fixture.runtime_dir,
        &crashed.workspace,
        &identity,
        "replace-recovery-lineage",
    )
    .await;
    let (lease_id, fencing_token) =
        acquire_lease(&client, &lineage.author, "task", "replace-recovery-lease").await;

    let recovery_payload = toolset_receipt_job_payload(
        crashed.plan.clone(),
        crashed.receipt,
        1,
        &policy_digest,
        &crashed.workspace,
        &lease_id,
        fencing_token,
    );
    let submitted = client
        .call::<Value>(
            RpcMethod::JobSubmit,
            job_submit_request(
                &lineage.author,
                recovery_payload,
                1,
                "replace-recovery-trigger",
            ),
        )
        .await
        .expect("a fresh job triggers project recovery");
    let recovery_job_id = submitted["jobId"]
        .as_str()
        .expect("recovery trigger job id");
    let recovery_job = wait_for_job(&client, &lineage.author, recovery_job_id).await;
    assert_eq!(recovery_job["status"], "error", "{recovery_job}");
    assert!(
        matches!(
            recovery_job["errorCode"].as_str(),
            Some("STALE_GATE_RESULT" | "EXTERNAL_STATE_CONFLICT")
        ),
        "recovered old-source job must fail closed: {recovery_job}"
    );

    let recovered_journals = journal_snapshot(&fixture.project_root);
    assert_journal_status_count(
        &recovered_journals,
        "toolset.receipt.record",
        "COMMITTED",
        1,
    );
    assert_journal_status_count(&recovered_journals, "toolset.receipt.record", "PREPARED", 0);
    let committed =
        single_journal_with_status(&recovered_journals, "toolset.receipt.record", "COMMITTED");
    assert_target_progress(&fixture.project_root, &committed, 3, true);
    assert_eq!(committed["event"]["eventType"], "toolset.receipt.record");

    let state_after_recovery_bytes =
        fs::read(&fixture.state_path).expect("state after partial replacement recovery");
    let state_after_recovery: Value =
        serde_json::from_slice(&state_after_recovery_bytes).expect("recovered state JSON");
    assert_eq!(state_after_recovery["revision"], 2);
    assert_eq!(
        state_after_recovery["toolsetReceiptRef"]["toolsetJobId"],
        crashed.job_id
    );
    let authority = read_project_receipt(&fixture.project_root, &state_after_recovery);
    let before_rejected_verification = journal_snapshot(&fixture.project_root);
    let rejected = client
        .call::<Value>(
            RpcMethod::OperationExecute,
            verification_request(
                &lineage.author,
                &lease_id,
                fencing_token,
                2,
                "replace-recovery-verification-denied",
                json!({
                    "toolsetJobId":crashed.job_id,
                    "plan":crashed.plan,
                    "receiptId":authority["receiptId"],
                    "receiptDigest":authority["receiptDigest"],
                    "sourceRevision":2,
                    "planDigest":authority["planDigest"],
                    "methodologyDigest":authority["methodologyDigest"],
                    "policyDigest":authority["policyDigest"],
                    "inputFingerprint":authority["inputFingerprint"],
                    "changedPaths":["src/lib.rs"],
                    "persist":true,
                }),
            ),
        )
        .await
        .expect_err("stale runtime job must not become PASS from recovered project files");
    assert_eq!(rejected.stable_code(), StableErrorCode::StaleGateResult);
    assert_eq!(
        fs::read(&fixture.state_path).expect("state after rejected verification"),
        state_after_recovery_bytes
    );
    assert_eq!(
        journal_snapshot(&fixture.project_root),
        before_rejected_verification,
        "rejected verification must not create another project mutation"
    );

    let events = events(&client, &identity, &crashed.event_store_id).await;
    assert_event_once(&events, "job.stale");
    assert_eq!(
        events
            .iter()
            .filter(|event| event["kind"].as_str() == Some("verification.plan"))
            .count(),
        0,
        "a stale recovered job must never emit verification.plan"
    );
    restarted.crash();
}

/// Scenario 1: the daemon dies right after the PREPARED journal, before any
/// target was replaced. Restart must abort the unapplied mutation, and the retry
/// must produce exactly one committed mutation, one event, and one projection.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_process_aborts_unapplied_review_record_then_commits_once() {
    let policy_digest = "b".repeat(64);
    let fixture = Fixture::review(&policy_digest);
    let database = fixture.database();
    let crashed = crash_during_review_record(
        &fixture,
        &policy_digest,
        ABORT_AFTER_PREPARED,
        "prepared-review",
    )
    .await;

    assert_eq!(
        fs::read(&fixture.state_path).expect("state after the PREPARED review crash"),
        crashed.state_before,
        "a PREPARED review with no replaced target must not advance project authority"
    );
    let prepared = single_journal_with_status(
        &journal_snapshot(&fixture.project_root),
        "review.record",
        "PREPARED",
    );
    assert_target_progress(&fixture.project_root, &prepared, 0, false);
    assert_eq!(
        review_projection_counts(&database),
        ReviewProjectionCounts {
            sessions: 0,
            batches: 0,
            attempts: 0,
            contributions: 0,
            findings: 0,
            remediations: 0,
            exit_receipts: 0,
        },
        "an unapplied review must not project any durable row"
    );

    let mut restarted =
        DaemonProcess::start(&fixture.runtime_dir, fixture.allowed_root(), &policy_digest).await;
    let cli = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let (root, committed) = retry_review_record(
        &cli,
        &daemon_client(&fixture.runtime_dir, ClientKind::Admin),
        &fixture,
        &crashed.workspace,
        &crashed.review_key,
        "prepared-retry",
    )
    .await;
    assert_eq!(
        committed["changed"], true,
        "the aborted attempt left no receipt, so the retry must commit: {committed}"
    );

    let journals = journal_snapshot(&fixture.project_root);
    assert_journal_status_count(&journals, "review.record", "ABORTED", 1);
    assert_journal_status_count(&journals, "review.record", "COMMITTED", 1);
    assert_journal_status_count(&journals, "review.record", "PREPARED", 0);
    assert_eq!(
        single_journal_with_status(&journals, "review.record", "ABORTED")["abortReason"],
        "ABORTED_RESTART: no target was applied"
    );
    assert_eq!(
        review_projection_counts(&database),
        one_row_per_review_projection(),
        "the retry must project exactly one row per Review table"
    );
    let events = events(
        &cli,
        &root,
        runtime_status(&cli).await["eventStoreId"]
            .as_str()
            .expect("event store id"),
    )
    .await;
    assert_event_once(&events, "review.record");
    restarted.crash();
}

/// Scenario 2: the daemon dies after `state.json` was replaced but before the
/// journal was committed and before the SQLite projection was written. Restart
/// must recover project state and the journal, and the same-key replay must
/// restore the projection before reporting success.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_process_recovers_replaced_review_record_and_restores_projection() {
    let policy_digest = "b".repeat(64);
    let fixture = Fixture::review(&policy_digest);
    let database = fixture.database();
    let crashed = crash_during_review_record(
        &fixture,
        &policy_digest,
        ABORT_AFTER_REPLACE_0,
        "replace-review",
    )
    .await;

    // `review.record` commits exactly one target, so aborting after replace 0
    // leaves the new state on disk with the journal still PREPARED.
    let crashed_state = fs::read(&fixture.state_path).expect("state after the replaced crash");
    assert_ne!(
        crashed_state, crashed.state_before,
        "the replaced target must survive the crash"
    );
    assert_eq!(
        state_revision(&fixture.state_path),
        crashed.source_revision + 1,
        "the replaced state carries the advanced revision"
    );
    let prepared = single_journal_with_status(
        &journal_snapshot(&fixture.project_root),
        "review.record",
        "PREPARED",
    );
    assert_target_progress(&fixture.project_root, &prepared, 1, true);
    assert_eq!(
        review_projection_counts(&database),
        ReviewProjectionCounts {
            sessions: 0,
            batches: 0,
            attempts: 0,
            contributions: 0,
            findings: 0,
            remediations: 0,
            exit_receipts: 0,
        },
        "the projection is written after commit, so it cannot exist yet"
    );

    let mut restarted =
        DaemonProcess::start(&fixture.runtime_dir, fixture.allowed_root(), &policy_digest).await;
    let cli = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let (root, replayed) = retry_review_record(
        &cli,
        &daemon_client(&fixture.runtime_dir, ClientKind::Admin),
        &fixture,
        &crashed.workspace,
        &crashed.review_key,
        "replace-retry",
    )
    .await;
    assert_eq!(
        replayed["changed"], false,
        "recovery completed the mutation, so the same key must replay: {replayed}"
    );
    assert_eq!(replayed["revisionAfter"], crashed.source_revision + 1);
    assert_eq!(
        review_projection_counts(&database),
        one_row_per_review_projection(),
        "the replay must restore the projection before returning success"
    );

    let journals = journal_snapshot(&fixture.project_root);
    assert_journal_status_count(&journals, "review.record", "COMMITTED", 1);
    assert_journal_status_count(&journals, "review.record", "PREPARED", 0);
    assert_journal_status_count(&journals, "review.record", "ABORTED", 0);
    let events = events(
        &cli,
        &root,
        runtime_status(&cli).await["eventStoreId"]
            .as_str()
            .expect("event store id"),
    )
    .await;
    assert_event_once(&events, "review.record");
    restarted.crash();
}

/// Scenario 3: the project commit survives, the Review projection rows are
/// destroyed, and a same-key replay after a real restart must repair the
/// projection before reporting `changed=false`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_process_repairs_deleted_review_projection_on_same_key_replay() {
    let policy_digest = "b".repeat(64);
    let fixture = Fixture::review(&policy_digest);
    let database = fixture.database();
    let mut daemon =
        DaemonProcess::start(&fixture.runtime_dir, fixture.allowed_root(), &policy_digest).await;
    let cli = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let admin = daemon_client(&fixture.runtime_dir, ClientKind::Admin);
    let workspace = register_and_cut_over_as(&cli, &admin, &fixture, REVIEW_PROJECT_KEY).await;
    let root_session =
        open_root_for_work_item(&cli, &workspace, REVIEW_WORK_ITEM_ID, "review-root-session").await;
    let root = Identity::for_work_item(
        &workspace,
        &root_session,
        REVIEW_WORK_ITEM_ID,
        AGENT_ID,
        fixture.state_path.clone(),
    );
    let lineage = open_review_lineage(
        &cli,
        &fixture.runtime_dir,
        &workspace,
        &root,
        "review-first",
    )
    .await;
    let (lease_id, fencing_token) =
        acquire_review_lease(&cli, &lineage.reviewer, "review-lease-first").await;
    let review_revision = state_revision(&fixture.state_path);

    let committed = cli
        .call::<Value>(
            RpcMethod::OperationExecute,
            review_record_request(
                &lineage.reviewer,
                &lease_id,
                fencing_token,
                review_revision,
                "review-findings-once",
            ),
        )
        .await
        .expect("review.record commits through real daemon authority");
    assert_eq!(committed["changed"], true, "{committed}");
    assert_eq!(committed["revisionAfter"], review_revision + 1);
    let committed_counts = review_projection_counts(&database);
    assert_eq!(
        committed_counts,
        ReviewProjectionCounts {
            sessions: 1,
            batches: 1,
            attempts: 1,
            contributions: 1,
            findings: 1,
            remediations: 0,
            exit_receipts: 0,
        },
        "one committed review.record projects exactly one row per table"
    );
    let state_after_commit = fs::read(&fixture.state_path).expect("review state after commit");
    let journals_after_commit = journal_snapshot(&fixture.project_root);
    assert_committed_once(&journals_after_commit, "review.record");

    delete_review_projections(&database);
    assert_eq!(
        review_projection_counts(&database),
        ReviewProjectionCounts {
            sessions: 0,
            batches: 0,
            attempts: 0,
            contributions: 0,
            findings: 0,
            remediations: 0,
            exit_receipts: 0,
        },
        "the projection loss must be observable before the restart"
    );

    daemon.crash();
    let mut restarted =
        DaemonProcess::start(&fixture.runtime_dir, fixture.allowed_root(), &policy_digest).await;
    let restarted_cli = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let reopened_root = open_root_for_work_item(
        &restarted_cli,
        &workspace,
        REVIEW_WORK_ITEM_ID,
        "review-root-restart",
    )
    .await;
    assert_eq!(reopened_root.session_id, root_session.session_id);
    let restarted_root = Identity::for_work_item(
        &workspace,
        &reopened_root,
        REVIEW_WORK_ITEM_ID,
        AGENT_ID,
        fixture.state_path.clone(),
    );
    // A delegated child session is bound to the accepting daemon boot, so the
    // reviewer lineage is re-established under the new boot identity before the
    // same-key replay.
    let replay_lineage = open_review_lineage(
        &restarted_cli,
        &fixture.runtime_dir,
        &workspace,
        &restarted_root,
        "review-second",
    )
    .await;
    assert_ne!(
        replay_lineage.reviewer_session_id, lineage.reviewer_session_id,
        "a rebuilt lineage must be a new physical reviewer session"
    );
    let replayed = restarted_cli
        .call::<Value>(
            RpcMethod::OperationExecute,
            review_record_request(
                &replay_lineage.reviewer,
                &lease_id,
                fencing_token,
                review_revision,
                "review-findings-once",
            ),
        )
        .await
        .expect("same review idempotency key replays after restart");
    assert_eq!(replayed["changed"], false, "{replayed}");
    assert_eq!(replayed["revisionAfter"], review_revision + 1);
    assert_eq!(
        review_projection_counts(&database),
        committed_counts,
        "same-key replay must repair the projection before returning changed=false"
    );
    assert_eq!(
        fs::read(&fixture.state_path).expect("review state after replay"),
        state_after_commit,
        "a replay must not mutate authoritative project state"
    );
    assert_eq!(
        journal_snapshot(&fixture.project_root),
        journals_after_commit,
        "a replay must not create another project mutation"
    );
    let events = events(
        &restarted_cli,
        &restarted_root,
        runtime_status(&restarted_cli).await["eventStoreId"]
            .as_str()
            .expect("event store id"),
    )
    .await;
    assert_event_once(&events, "review.record");
    restarted.crash();
}

/// State captured after a real daemon died inside a `review.record` commit.
struct CrashedReviewRecord {
    workspace: WorkspaceResult,
    review_key: String,
    state_before: Vec<u8>,
    source_revision: u64,
}

/// Bootstraps a review-ready workspace, establishes a real reviewer lineage,
/// acquires the reviewer lease, and then aborts the daemon inside the
/// `review.record` commit at `abort_at`.
///
/// The failpoint is scoped to `review.record` so the reviewer's own
/// `lease.acquire` still commits: a delegated reviewer session is bound to the
/// accepting daemon boot, so a reviewer-owned lease cannot be carried across a
/// restart and must be acquired on the same boot that records the review.
async fn crash_during_review_record(
    fixture: &Fixture,
    policy_digest: &str,
    abort_at: &str,
    key: &str,
) -> CrashedReviewRecord {
    let mut bootstrap =
        DaemonProcess::start(&fixture.runtime_dir, fixture.allowed_root(), policy_digest).await;
    let cli = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let admin = daemon_client(&fixture.runtime_dir, ClientKind::Admin);
    let workspace = register_and_cut_over_as(&cli, &admin, fixture, REVIEW_PROJECT_KEY).await;
    bootstrap.crash();

    let mut daemon = DaemonProcess::start_with_commit_abort(
        &fixture.runtime_dir,
        fixture.allowed_root(),
        policy_digest,
        &format!("{abort_at}@review.record"),
    )
    .await;
    let cli = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let root_session = open_root_for_work_item(
        &cli,
        &workspace,
        REVIEW_WORK_ITEM_ID,
        &format!("{key}-root"),
    )
    .await;
    let root = Identity::for_work_item(
        &workspace,
        &root_session,
        REVIEW_WORK_ITEM_ID,
        AGENT_ID,
        fixture.state_path.clone(),
    );
    let lineage = open_review_lineage(
        &cli,
        &fixture.runtime_dir,
        &workspace,
        &root,
        &format!("{key}-lineage"),
    )
    .await;
    let (lease_id, fencing_token) =
        acquire_review_lease(&cli, &lineage.reviewer, &format!("{key}-lease")).await;
    let source_revision = state_revision(&fixture.state_path);
    let state_before = fs::read(&fixture.state_path).expect("state before the review crash");

    let review_key = format!("{key}-review");
    let _ = cli
        .call::<Value>(
            RpcMethod::OperationExecute,
            review_record_request(
                &lineage.reviewer,
                &lease_id,
                fencing_token,
                source_revision,
                &review_key,
            ),
        )
        .await;
    daemon.wait_for_crash(&fixture.runtime_dir).await;

    CrashedReviewRecord {
        workspace,
        review_key,
        state_before,
        source_revision,
    }
}

/// Re-establishes a reviewer lineage plus lease on the current daemon boot and
/// retries `review.record` under `review_key`.
async fn retry_review_record(
    cli: &DaemonClient,
    admin: &DaemonClient,
    fixture: &Fixture,
    workspace: &WorkspaceResult,
    review_key: &str,
    key: &str,
) -> (Identity, Value) {
    break_orphaned_lease(
        admin,
        workspace,
        REVIEW_WORK_ITEM_ID,
        &format!("{key}-break"),
    )
    .await;
    let root_session =
        open_root_for_work_item(cli, workspace, REVIEW_WORK_ITEM_ID, &format!("{key}-root")).await;
    let root = Identity::for_work_item(
        workspace,
        &root_session,
        REVIEW_WORK_ITEM_ID,
        AGENT_ID,
        fixture.state_path.clone(),
    );
    let lineage = open_review_lineage(
        cli,
        &fixture.runtime_dir,
        workspace,
        &root,
        &format!("{key}-lineage"),
    )
    .await;
    let (lease_id, fencing_token) =
        acquire_review_lease(cli, &lineage.reviewer, &format!("{key}-lease")).await;
    let response = cli
        .call::<Value>(
            RpcMethod::OperationExecute,
            review_record_request(
                &lineage.reviewer,
                &lease_id,
                fencing_token,
                state_revision(&fixture.state_path),
                review_key,
            ),
        )
        .await
        .unwrap_or_else(|error| panic!("{key} review retry: {error:?}"));
    (root, response)
}

/// Breaks the lease left behind by a delegated child session that died with
/// its boot.
///
/// A delegated reviewer session cannot be revived on a later boot, so its lease
/// can only be reclaimed through the confirmed Admin `lease.break` path.
async fn break_orphaned_lease(
    admin: &DaemonClient,
    workspace: &WorkspaceResult,
    work_item_id: &str,
    key: &str,
) {
    let mut request = params(json!({
        "operation":"lease.break",
        "payload":{
            "actor":{"role":"admin"},
            "reason":"delegated child session died with its daemon boot",
        },
    }));
    request.workspace_id = Some(workspace.workspace_id.clone());
    request.work_item_id = Some(work_item_id.to_owned());
    request.idempotency_key = Some(key.to_owned());
    request.confirmation = Some(confirmation(key));
    admin
        .call::<Value>(RpcMethod::OperationExecute, request)
        .await
        .unwrap_or_else(|error| panic!("{key} admin lease.break: {error:?}"));
}

/// Exactly one projection row per Review Batch v2 table.
fn one_row_per_review_projection() -> ReviewProjectionCounts {
    ReviewProjectionCounts {
        sessions: 1,
        batches: 1,
        attempts: 1,
        contributions: 1,
        findings: 1,
        remediations: 0,
        exit_receipts: 0,
    }
}

struct CrashedToolsetReceipt {
    workspace: WorkspaceResult,
    plan: Value,
    receipt: Value,
    job_id: String,
    state_before: Vec<u8>,
    event_store_id: String,
}

async fn crash_during_toolset_receipt(
    fixture: &Fixture,
    policy_digest: &str,
    abort_at: &str,
    submission_key: &str,
) -> CrashedToolsetReceipt {
    let mut bootstrap =
        DaemonProcess::start(&fixture.runtime_dir, fixture.allowed_root(), policy_digest).await;
    let bootstrap_client = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let admin = daemon_client(&fixture.runtime_dir, ClientKind::Admin);
    let workspace = register_and_cut_over(&bootstrap_client, &admin, fixture).await;
    let event_store_id = runtime_status(&bootstrap_client).await["eventStoreId"]
        .as_str()
        .expect("event store id")
        .to_owned();
    bootstrap.crash();

    let mut daemon = DaemonProcess::start_with_commit_abort(
        &fixture.runtime_dir,
        fixture.allowed_root(),
        policy_digest,
        abort_at,
    )
    .await;
    let client = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let root_session = open_root(
        &client,
        &workspace,
        &format!("{submission_key}-failpoint-session"),
    )
    .await;
    let root = Identity::for_work_item(
        &workspace,
        &root_session,
        WORK_ITEM_ID,
        AGENT_ID,
        fixture.state_path.clone(),
    );
    // The receipt job is semantic work, so a delegated author task submits
    // it and owns the project lease its job lease proof binds. Both must
    // live on the same boot: the lease owner is the physical session id.
    let lineage = open_review_lineage(
        &client,
        &fixture.runtime_dir,
        &workspace,
        &root,
        &format!("{submission_key}-lineage"),
    )
    .await;
    let (lease_id, fencing_token) = acquire_lease(
        &client,
        &lineage.author,
        "task",
        &format!("{submission_key}-lease"),
    )
    .await;
    let plan = verification_plan();
    let receipt = plan
        .receipt(
            WorkerId::new(format!("{submission_key}-worker")).expect("worker id"),
            JobStatus::Pass,
            Some(0),
            EvidenceDigest::digest(b"process stdout"),
            EvidenceDigest::digest(b"process stderr"),
            10,
            20,
            false,
            false,
        )
        .expect("PASS receipt matches the plan");
    let plan = serde_json::to_value(plan).expect("verification plan serializes");
    let receipt = serde_json::to_value(receipt).expect("verification receipt serializes");
    let payload = toolset_receipt_job_payload(
        plan.clone(),
        receipt.clone(),
        1,
        policy_digest,
        &workspace,
        &lease_id,
        fencing_token,
    );
    let state_before = fs::read(&fixture.state_path).expect("state before commit abort");
    let submission = client
        .call::<Value>(
            RpcMethod::JobSubmit,
            job_submit_request(&lineage.author, payload, 1, submission_key),
        )
        .await;
    daemon.wait_for_crash(&fixture.runtime_dir).await;

    let persisted = persisted_job_by_submission_key(&fixture.runtime_dir, submission_key);
    assert_eq!(
        persisted.status,
        RuntimeJobStatus::Running,
        "the real worker must durably enter running before the project commit abort"
    );
    assert!(persisted.result.is_none());
    assert!(persisted.error_code.is_none());
    if let Ok(response) = submission {
        assert_eq!(response["jobId"], persisted.job_id);
    }

    CrashedToolsetReceipt {
        workspace,
        plan,
        receipt,
        job_id: persisted.job_id,
        state_before,
        event_store_id,
    }
}

fn toolset_receipt_job_payload(
    plan: Value,
    receipt: Value,
    source_revision: u64,
    policy_digest: &str,
    workspace: &WorkspaceResult,
    lease_id: &str,
    fencing_token: u64,
) -> Value {
    json!({
        "entrypoint":"toolset.receipt.record",
        "arguments":{
            "plan":plan,
            "receipt":receipt,
            "sourceRevision":source_revision,
            "policyDigest":policy_digest,
            "methodologyDigest":"2".repeat(64),
            "inventoryGeneration":workspace.inventory_generation,
            "leaseId":lease_id,
            "fencingToken":fencing_token,
        },
        "deadlineUnixMs":now_unix_ms().saturating_add(60_000),
    })
}

fn verification_payload_from_job(job: &Value, plan: Value) -> Value {
    json!({
        "toolsetJobId":job["jobId"],
        "plan":plan,
        "receiptId":job["result"]["receiptId"],
        "receiptDigest":job["result"]["receiptDigest"],
        "sourceRevision":job["result"]["sourceRevision"],
        "planDigest":job["result"]["planDigest"],
        "methodologyDigest":job["result"]["methodologyDigest"],
        "policyDigest":job["result"]["policyDigest"],
        "inputFingerprint":job["result"]["inputFingerprint"],
        "changedPaths":["src/lib.rs"],
        "persist":true,
    })
}

fn persisted_job_by_submission_key(state_dir: &Path, submission_key: &str) -> RuntimeJobRecord {
    let paths = RuntimePaths::from_state_dir(state_dir.to_path_buf());
    let persistence = SqliteRuntimePersistence::open(&paths.database)
        .expect("runtime SQLite reopens after crash");
    persistence
        .list_jobs()
        .expect("durable jobs are readable")
        .into_iter()
        .find(|job| job.submission_idempotency_key == submission_key)
        .expect("submitted job is durable before process abort")
}

/// Reads one durable runtime job row while the daemon is still running.
///
/// A job submitted by a delegated child session stays bound to that physical
/// session, which dies with its daemon boot, so no session the new boot can
/// open passes the `job.status` session check. The stale diagnosis therefore
/// reads the row through a read-only SQLite connection that never blocks the
/// live writer, and shapes it like the wire job for the shared assertions.
fn live_job_wire(fixture: &Fixture, job_id: &str) -> Value {
    let connection = rusqlite::Connection::open_with_flags(
        fixture.database(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("runtime SQLite opens read-only");
    let (status, result_json, error_code, started_at, finished_at): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = connection
        .query_row(
            "SELECT status,result_json,error_code,started_at,finished_at FROM runtime_job_v1 WHERE job_id=?1",
            [job_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("durable job row is readable");
    json!({
        "status": status,
        "result": result_json
            .map(|json| serde_json::from_str::<Value>(&json).expect("job result JSON")),
        "errorCode": error_code,
        "startedAtUnixMs": started_at.map(|_| 1),
        "finishedAtUnixMs": finished_at.map(|_| 1),
    })
}

fn assert_session_expired_job(job: &Value) {
    assert_eq!(job["status"], "stale", "{job}");
    assert_eq!(
        job["result"]["errorCode"],
        StableErrorCode::SessionExpired.as_str()
    );
    assert!(job["errorCode"].is_null(), "{job}");
    assert!(job["startedAtUnixMs"].as_u64().is_some(), "{job}");
    assert!(job["finishedAtUnixMs"].as_u64().is_some(), "{job}");
}

fn read_project_receipt(root: &Path, state: &Value) -> Value {
    let relative = state["toolsetReceiptRef"]["artifactRef"]
        .as_str()
        .expect("toolset receipt artifact ref");
    serde_json::from_slice(
        &fs::read(root.join(relative)).expect("recovered project receipt artifact"),
    )
    .expect("project receipt JSON")
}

fn single_journal_with_status(
    journals: &BTreeMap<String, Vec<u8>>,
    operation: &str,
    status: &str,
) -> Value {
    let mut matching = journals
        .values()
        .map(|bytes| serde_json::from_slice::<Value>(bytes).expect("journal JSON"))
        .filter(|journal| {
            journal["operation"].as_str() == Some(operation)
                && journal["status"].as_str() == Some(status)
        });
    let journal = matching
        .next()
        .unwrap_or_else(|| panic!("missing {status} journal for {operation}: {journals:?}"));
    assert!(
        matching.next().is_none(),
        "multiple {status} journals for {operation}: {journals:?}"
    );
    journal
}

fn assert_journal_status_count(
    journals: &BTreeMap<String, Vec<u8>>,
    operation: &str,
    status: &str,
    expected: usize,
) {
    let actual = journals
        .values()
        .map(|bytes| serde_json::from_slice::<Value>(bytes).expect("journal JSON"))
        .filter(|journal| {
            journal["operation"].as_str() == Some(operation)
                && journal["status"].as_str() == Some(status)
        })
        .count();
    assert_eq!(
        actual, expected,
        "unexpected {status} journal count for {operation}: {journals:?}"
    );
}

fn assert_target_progress(
    root: &Path,
    journal: &Value,
    expected_after_count: usize,
    staged_must_exist: bool,
) {
    let targets = journal["targetFiles"].as_array().expect("journal targets");
    let mut after_count = 0;
    for target in targets {
        let relative = target["path"].as_str().expect("target path");
        let current = match fs::read(root.join(relative)) {
            Ok(bytes) => Some(ArtifactDigest::digest(bytes).to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => panic!("target {relative} is unreadable: {error}"),
        };
        let before = target["beforeDigest"].as_str();
        let after = target["afterDigest"].as_str().expect("after digest");
        if current.as_deref() == Some(after) {
            after_count += 1;
        } else {
            assert_eq!(
                current.as_deref(),
                before,
                "target {relative} is neither before nor after: {target}"
            );
        }

        let staged = root.join(target["stagedRef"].as_str().expect("staged ref"));
        if staged_must_exist {
            let staged_bytes = fs::read(&staged).expect("staged target must survive crash");
            assert_eq!(ArtifactDigest::digest(staged_bytes).to_string(), after);
        } else {
            assert!(
                !staged.exists(),
                "staged target exists before staging: {staged:?}"
            );
        }
    }
    assert_eq!(
        after_count, expected_after_count,
        "unexpected applied target count: {journal}"
    );
}

/// Workspace fixture whose authoritative state is already positioned for a
/// Review Batch v2 attempt: `test-running`, tier1 scale, and an approved
/// `executionPlan.changedPaths` scope covering the reviewed source.
fn prepare_review_workspace(root: &Path, policy_digest: &str) -> PathBuf {
    let state_dir = root.join(".auto-engineering/typed-e2e");
    for directory in [state_dir.clone(), root.join("docs"), root.join("src")] {
        fs::create_dir_all(directory).expect("review fixture directory");
    }
    fs::write(root.join("docs/story.md"), "# C1 review process Story\n").expect("Story fixture");
    fs::write(root.join("src/lib.rs"), "pub fn ready() -> bool { true }\n")
        .expect("review source fixture");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("Cargo fixture");
    let state_path = state_dir.join("state.json");
    fs::write(
        &state_path,
        serde_json::to_vec_pretty(&json!({
            "stateMachineName":"PRD-C1-REVIEW-PROCESS",
            "activeStory":REVIEW_WORK_ITEM_ID,
            "revision":1,
            "lastFencingToken":0,
            "scale":"small",
            "selectedDesign":"Story",
            "phase":"test-running",
            "currentPhase":"test-running",
            "policyDigest":policy_digest,
            "executionPlan":{"changedPaths":["src/lib.rs"]},
            "storyStates":{
                REVIEW_WORK_ITEM_ID:{"phase":"test-running","currentPhase":"test-running"}
            },
            "documentPaths":{"STORY":"docs/story.md"},
        }))
        .expect("review state serializes"),
    )
    .expect("review state fixture");
    state_path
}

struct Fixture {
    root: TempDir,
    project_root: PathBuf,
    runtime_dir: PathBuf,
    state_path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        Self::build(prepare_workspace)
    }

    /// Fixture whose state is positioned for a real Review Batch v2 attempt.
    fn review(policy_digest: &str) -> Self {
        Self::build(|project_root| prepare_review_workspace(project_root, policy_digest))
    }

    fn build(prepare: impl FnOnce(&Path) -> PathBuf) -> Self {
        let root = tempfile::tempdir().expect("process fixture root");
        let project_root = root.path().join("project");
        let runtime_dir = root.path().join("runtime");
        fs::create_dir_all(&project_root).expect("project root");
        fs::create_dir_all(&runtime_dir).expect("runtime root");
        let state_path = prepare(&project_root);
        Self {
            root,
            project_root,
            runtime_dir,
            state_path,
        }
    }

    fn database(&self) -> PathBuf {
        RuntimePaths::from_state_dir(self.runtime_dir.clone()).database
    }

    fn allowed_root(&self) -> &Path {
        self.root.path()
    }
}

struct DaemonProcess {
    child: Child,
}

impl DaemonProcess {
    async fn start(state_dir: &Path, allowed_root: &Path, policy_digest: &str) -> Self {
        Self::start_inner(state_dir, allowed_root, policy_digest, None).await
    }

    async fn start_with_commit_abort(
        state_dir: &Path,
        allowed_root: &Path,
        policy_digest: &str,
        abort_at: &str,
    ) -> Self {
        Self::start_inner(state_dir, allowed_root, policy_digest, Some(abort_at)).await
    }

    async fn start_inner(
        state_dir: &Path,
        allowed_root: &Path,
        policy_digest: &str,
        abort_at: Option<&str>,
    ) -> Self {
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
        if let Some(point) = abort_at {
            command.env(COMMIT_ABORT_ENV, point);
        }
        let mut child = command.spawn().expect("real ae-sddd process starts");
        let manifest = state_dir.join("endpoint.v1.json");
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(status) = child.try_wait().expect("daemon status is readable") {
                panic!(
                    "ae-sddd exited before ready ({status}); log: {}",
                    daemon_log(state_dir)
                );
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
                let log = daemon_log(state_dir);
                let _ = child.kill();
                let _ = child.wait();
                panic!("ae-sddd did not become ready; log: {log}");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    /// Waits for the commit-abort failpoint to kill the daemon.
    ///
    /// The timeout has been observed to expire under full-suite parallel load
    /// while passing in isolation, and the bare deadline message could not
    /// distinguish "the request never reached the commit point" from "the abort
    /// was merely slow". The failure text therefore carries how long the daemon
    /// stayed alive, how many polls elapsed, and the daemon log, so the next
    /// occurrence is diagnosable from the test output alone.
    async fn wait_for_crash(&mut self, state_dir: &Path) {
        const BUDGET: Duration = Duration::from_secs(15);
        let started = Instant::now();
        let deadline = started + BUDGET;
        let mut polls = 0_u32;
        loop {
            if let Some(status) = self.child.try_wait().expect("daemon status is readable") {
                assert!(
                    !status.success(),
                    "commit abort unexpectedly exited successfully after {:?} and {polls} polls",
                    started.elapsed()
                );
                return;
            }
            polls += 1;
            assert!(
                Instant::now() < deadline,
                "daemon did not abort at the configured commit point: still alive after {:?} \
                 and {polls} polls (budget {BUDGET:?}). A live daemon here means the operation \
                 never reached the armed commit point, so check that the request was admitted \
                 at all; log: {}",
                started.elapsed(),
                daemon_log(state_dir)
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
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
    work_item_id: String,
    agent_id: String,
    session_id: String,
    capability_token: String,
    state_path: PathBuf,
}

impl Identity {
    fn for_work_item(
        workspace: &WorkspaceResult,
        session: &SessionResult,
        work_item_id: &str,
        agent_id: &str,
        state_path: PathBuf,
    ) -> Self {
        Self {
            workspace_id: workspace.workspace_id.clone(),
            work_item_id: work_item_id.to_owned(),
            agent_id: agent_id.to_owned(),
            session_id: session.session_id.clone(),
            capability_token: session.capability_token.clone(),
            state_path,
        }
    }

    fn params(&self, payload: Value) -> RequestParams<Value> {
        let mut request = params(payload);
        request.workspace_id = Some(self.workspace_id.clone());
        request.work_item_id = Some(self.work_item_id.clone());
        request.session_id = Some(self.session_id.clone());
        request.agent_id = Some(self.agent_id.clone());
        request.capability_token = Some(self.capability_token.clone());
        request
    }
}

async fn register_and_cut_over(
    cli: &DaemonClient,
    admin: &DaemonClient,
    fixture: &Fixture,
) -> WorkspaceResult {
    register_and_cut_over_as(cli, admin, fixture, PROJECT_KEY).await
}

async fn register_and_cut_over_as(
    cli: &DaemonClient,
    admin: &DaemonClient,
    fixture: &Fixture,
    project_key: &str,
) -> WorkspaceResult {
    let mut register = params(json!({
        "projectRoot":fixture.project_root.to_string_lossy(),
        "projectKey":project_key,
    }));
    register.idempotency_key = Some("workspace-register-process-e2e".to_owned());
    let registered = cli
        .call::<WorkspaceResult>(RpcMethod::WorkspaceRegister, register)
        .await
        .expect("workspace registers through the process");
    assert_eq!(registered.mode, WorkspaceMode::Shadow);

    let mut drain = params(json!({"stop":false}));
    drain.idempotency_key = Some("runtime-drain-process-e2e".to_owned());
    drain.confirmation = Some(confirmation("runtime-drain-process-e2e"));
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
        "targetMode":WorkspaceMode::RustCanary,
        "reason":"C1 process E2E parity fixture",
        "parityDigest":parity_digest,
        "parity":parity,
    }));
    transition.workspace_id = Some(registered.workspace_id);
    transition.idempotency_key = Some("workspace-cutover-process-e2e".to_owned());
    transition.confirmation = Some(confirmation("workspace-cutover-process-e2e"));
    let cut_over = admin
        .call::<WorkspaceResult>(RpcMethod::WorkspaceModeTransition, transition)
        .await
        .expect("workspace enters Rust canary mode");
    assert_eq!(cut_over.mode, WorkspaceMode::RustCanary);
    assert_eq!(cut_over.project_key, project_key);
    cut_over
}

async fn open_root(
    client: &DaemonClient,
    workspace: &WorkspaceResult,
    idempotency_key: &str,
) -> SessionResult {
    open_root_for_work_item(client, workspace, WORK_ITEM_ID, idempotency_key).await
}

async fn open_root_for_work_item(
    client: &DaemonClient,
    workspace: &WorkspaceResult,
    work_item_id: &str,
    idempotency_key: &str,
) -> SessionResult {
    let mut request = params(json!({
        "externalKey":EXTERNAL_SESSION_KEY,
        "role":"root",
        "engaged":true,
    }));
    request.workspace_id = Some(workspace.workspace_id.clone());
    request.work_item_id = Some(work_item_id.to_owned());
    request.agent_id = Some(AGENT_ID.to_owned());
    request.idempotency_key = Some(idempotency_key.to_owned());
    client
        .call(RpcMethod::SessionOpen, request)
        .await
        .expect("root session opens through the process")
}

async fn acquire_lease(
    client: &DaemonClient,
    identity: &Identity,
    owner_role: &str,
    idempotency_key: &str,
) -> (String, u64) {
    let mut request = operation_request(
        identity,
        "lease.acquire",
        json!({"owner":{"role":owner_role},"ttlSeconds":300}),
    );
    request.idempotency_key = Some(idempotency_key.to_owned());
    let response = client
        .call::<Value>(RpcMethod::OperationExecute, request)
        .await
        .expect("project lease is acquired");
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

fn job_submit_request(
    identity: &Identity,
    payload: Value,
    expected_revision: u64,
    idempotency_key: &str,
) -> RequestParams<Value> {
    let mut request = identity.params(payload);
    request.expected_revision = Some(expected_revision);
    request.idempotency_key = Some(idempotency_key.to_owned());
    request
}

fn job_status_request(identity: &Identity, job_id: &str) -> RequestParams<Value> {
    identity.params(json!({"jobId":job_id}))
}

async fn wait_for_job(client: &DaemonClient, identity: &Identity, job_id: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let status = client
            .call::<Value>(RpcMethod::JobStatus, job_status_request(identity, job_id))
            .await
            .expect("job status is readable");
        match status["status"].as_str() {
            Some("queued" | "running") => {}
            Some(_) => return status,
            None => panic!("job status is malformed: {status}"),
        }
        assert!(Instant::now() < deadline, "job did not finish: {status}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn operation_request(identity: &Identity, operation: &str, payload: Value) -> RequestParams<Value> {
    identity.params(json!({"operation":operation,"payload":payload}))
}

fn verification_request(
    identity: &Identity,
    lease_id: &str,
    fencing_token: u64,
    expected_revision: u64,
    idempotency_key: &str,
    payload: Value,
) -> RequestParams<Value> {
    let mut request = operation_request(identity, "verification.plan", payload);
    request.lease_id = Some(lease_id.to_owned());
    request.fencing_token = Some(fencing_token);
    request.expected_revision = Some(expected_revision);
    request.idempotency_key = Some(idempotency_key.to_owned());
    request
}

async fn runtime_status(client: &DaemonClient) -> Value {
    client
        .call(RpcMethod::RuntimeStatus, params(json!({})))
        .await
        .expect("runtime status")
}

async fn events(client: &DaemonClient, identity: &Identity, event_store_id: &str) -> Vec<Value> {
    let response: Value = client
        .call(
            RpcMethod::EventsSubscribe,
            identity.params(json!({
                "eventStoreId":event_store_id,
                "afterEventSeq":0,
                "limit":128,
            })),
        )
        .await
        .expect("events are readable");
    response["events"].as_array().expect("event batch").clone()
}

fn assert_event_once(events: &[Value], kind: &str) {
    let matches = events
        .iter()
        .filter(|event| event["kind"].as_str() == Some(kind))
        .count();
    assert_eq!(matches, 1, "expected one {kind} event: {events:?}");
}

fn verification_plan() -> VerificationExecutionPlan {
    let binding = json!({
        "storyId":WORK_ITEM_ID,
        "workItem":WORK_ITEM_ID,
        "changedPaths":["src/lib.rs"],
        "sinceFingerprint":"",
    });
    let input_fingerprint = InputFingerprint::digest(
        serde_json::to_vec(&binding).expect("verification binding serializes"),
    );
    let program = ArtifactRef::new(
        ArtifactKind::new("verification-program").expect("artifact kind"),
        ProjectRelativePath::new("tools/cargo.exe").expect("program path"),
        ArtifactDigest::digest(b"tools/cargo.exe"),
        1,
    );
    let step = ExecutionStep::new(
        SchemaVersion::V1,
        ExecutionStepId::new("c1-process-focused-tests").expect("step id"),
        program,
        vec![BoundedText::new("test").expect("argument")],
        None,
        Vec::new(),
    )
    .expect("execution step");
    VerificationExecutionPlan::new(
        SchemaVersion::V1,
        ExecutionId::new("c1-process-execution").expect("execution id"),
        WorkItemId::new(WORK_ITEM_ID).expect("Work Item"),
        input_fingerprint,
        vec![step],
    )
    .expect("verification plan")
}

fn prepare_workspace(root: &Path) -> PathBuf {
    let state_dir = root.join(".auto-engineering/typed-e2e");
    for directory in [
        state_dir.clone(),
        root.join("docs"),
        root.join("src"),
        root.join("tools"),
        root.join("constraints"),
    ] {
        fs::create_dir_all(directory).expect("fixture directory");
    }
    fs::write(root.join("docs/story.md"), "# C1 process Story\n").expect("Story fixture");
    fs::write(root.join("src/lib.rs"), "pub fn ready() -> bool { true }\n")
        .expect("source fixture");
    fs::write(root.join("tools/cargo.exe"), b"fixture program").expect("program fixture");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("Cargo fixture");
    for index in 0..5 {
        fs::write(
            root.join(format!("constraints/constraint-{index}.md")),
            "# constraint\n",
        )
        .expect("constraint fixture");
    }
    let state_path = state_dir.join("state.json");
    fs::write(
        &state_path,
        serde_json::to_vec_pretty(&json!({
            "stateMachineName":"PRD-C1-PROCESS-E2E",
            "activeStory":WORK_ITEM_ID,
            "revision":1,
            "lastFencingToken":0,
            "scale":"small",
            "selectedDesign":"Story",
            "phase":"coding",
            "currentPhase":"coding",
            "storyStates":{
                WORK_ITEM_ID:{"phase":"coding","currentPhase":"coding"}
            },
            "documentPaths":{"STORY":"docs/story.md"},
            "toolsetReceiptRef":{
                "schemaVersion":1,
                "toolsetJobId":"forged-job",
                "receiptId":"forged-receipt",
                "receiptDigest":"0".repeat(64),
                "artifactRef":"forged/receipt.json",
                "projectReceiptDigest":"0".repeat(64),
                "manifestRef":"forged/manifest.json",
                "manifestDigest":"0".repeat(64),
                "mutationId":"00000000-0000-0000-0000-000000000000",
                "sourceRevision":1,
            },
            "verificationPlan":{"status":"PASS","source":"state-injection"},
        }))
        .expect("state serializes"),
    )
    .expect("state fixture");
    state_path
}

fn journal_snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let directory = root.join(".auto-engineering/typed-e2e/mutation-journal/v1");
    if !directory.is_dir() {
        return BTreeMap::new();
    }
    fs::read_dir(directory)
        .expect("journal directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("json"))
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path()).expect("journal bytes"),
            )
        })
        .collect()
}

fn assert_committed_once(journals: &BTreeMap<String, Vec<u8>>, operation: &str) {
    let matching = journals
        .values()
        .map(|bytes| serde_json::from_slice::<Value>(bytes).expect("journal JSON"))
        .filter(|journal| journal["operation"].as_str() == Some(operation))
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), 1, "journals for {operation}: {matching:?}");
    assert_eq!(matching[0]["status"], "COMMITTED");
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

fn daemon_log(state_dir: &Path) -> String {
    fs::read_to_string(state_dir.join("daemon.log")).unwrap_or_else(|_| "<unavailable>".to_owned())
}

/// Root -> Series -> {Task author, Reviewer} lineage established through real
/// daemon RPC: `host.register`, `delegation.create`, `host.action_next`,
/// `host.action_ack`, `delegation.accept`, then `session.open`.
///
/// A delegated child session is bound to the accepting daemon boot, so this
/// lineage must be rebuilt after every real process restart. `session_seed`
/// keeps each rebuild's identities distinct.
struct ReviewLineage {
    author: Identity,
    author_session_id: String,
    reviewer: Identity,
    reviewer_session_id: String,
}

async fn open_review_lineage(
    cli: &DaemonClient,
    state_dir: &Path,
    workspace: &WorkspaceResult,
    root: &Identity,
    key: &str,
) -> ReviewLineage {
    let adapter_id = "codex".to_owned();
    let host = HostAdapter::register(state_dir, &adapter_id, &format!("{key}-host-register")).await;

    let (series, series_delegation) = open_delegated_child(
        cli,
        &host,
        workspace,
        root,
        &adapter_id,
        "series",
        None,
        json!({
            "operations":["document.save","evidence.finalize","evidence.record","lease.acquire","lease.release","review.record","verification.plan"],
            "capabilities":["review.specialty.general"],
            "paths":[{"kind":"project_root"}],
        }),
        &format!("{key}-series"),
    )
    .await;
    let (author, _) = open_delegated_child(
        cli,
        &host,
        workspace,
        &series,
        &adapter_id,
        "task",
        Some(&series_delegation),
        json!({
            "operations":["document.save","evidence.finalize","evidence.record","lease.acquire","lease.release","verification.plan"],
            "capabilities":[],
            "paths":[{"kind":"project_root"}],
        }),
        &format!("{key}-author"),
    )
    .await;
    let (reviewer, _) = open_delegated_child(
        cli,
        &host,
        workspace,
        &series,
        &adapter_id,
        "reviewer",
        Some(&series_delegation),
        json!({
            "operations":["lease.acquire","review.record"],
            "capabilities":["review.specialty.general"],
            "paths":[{"kind":"project_root"}],
        }),
        &format!("{key}-reviewer"),
    )
    .await;
    ReviewLineage {
        author_session_id: author.session_id.clone(),
        author,
        reviewer_session_id: reviewer.session_id.clone(),
        reviewer,
    }
}

#[allow(clippy::too_many_arguments)]
async fn open_delegated_child(
    cli: &DaemonClient,
    host: &HostAdapter,
    workspace: &WorkspaceResult,
    parent: &Identity,
    adapter_id: &str,
    child_role: &str,
    parent_delegation_id: Option<&str>,
    grant: Value,
    key: &str,
) -> (Identity, String) {
    let child_session_id = Uuid::new_v4().to_string();
    let state_bytes = fs::read(parent.state_path.as_path()).expect("delegation input state");
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
            "adapterId":adapter_id,
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
        state_path: parent.state_path.clone(),
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
                "clientBuild":"c1-process-host-adapter",
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

fn state_revision(state_path: &Path) -> u64 {
    serde_json::from_slice::<Value>(&fs::read(state_path).expect("state bytes"))
        .expect("state JSON")["revision"]
        .as_u64()
        .expect("authoritative state revision")
}

/// Bounded row counts for every Review Batch v2 projection table.
#[derive(Debug, Eq, PartialEq)]
struct ReviewProjectionCounts {
    sessions: i64,
    batches: i64,
    attempts: i64,
    contributions: i64,
    findings: i64,
    remediations: i64,
    exit_receipts: i64,
}

fn review_projection_counts(database: &Path) -> ReviewProjectionCounts {
    let connection = rusqlite::Connection::open_with_flags(
        database,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("runtime SQLite opens read-only");
    let count = |table: &str| -> i64 {
        connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap_or_else(|error| panic!("{table} is countable: {error}"))
    };
    ReviewProjectionCounts {
        sessions: count("review_session_v2_projection"),
        batches: count("review_batch_v2_projection"),
        attempts: count("review_attempt_v2_projection"),
        contributions: count("review_effective_contribution_v2_projection"),
        findings: count("review_finding_v2_projection"),
        remediations: count("review_remediation_v2_projection"),
        exit_receipts: count("review_exit_receipt_v2_projection"),
    }
}

/// Deletes every Review projection row and its projection receipt, simulating a
/// durable projection loss while the authoritative project commit, mutation
/// journal, and committed runtime event all survive.
fn delete_review_projections(database: &Path) {
    let mut connection =
        rusqlite::Connection::open(database).expect("runtime SQLite opens writable");
    let transaction = connection
        .transaction()
        .expect("projection deletion transaction begins");
    transaction
        .execute_batch("PRAGMA defer_foreign_keys=ON;")
        .expect("projection foreign keys defer inside the transaction");
    for table in [
        "review_exit_receipt_v2_projection",
        "review_remediation_v2_projection",
        "review_finding_v2_projection",
        "review_effective_contribution_v2_projection",
        "review_attempt_v2_projection",
        "review_batch_v2_projection",
        "review_session_v2_projection",
    ] {
        transaction
            .execute(&format!("DELETE FROM {table}"), [])
            .unwrap_or_else(|error| panic!("{table} rows are removable: {error}"));
    }
    // The idempotent projection receipt must go too; otherwise a repair would
    // short-circuit on an exact receipt and leave the rows missing.
    transaction
        .execute(
            "DELETE FROM runtime_record_v1 WHERE namespace='review-projection-event/v2'",
            [],
        )
        .expect("review projection receipts are removable");
    transaction
        .commit()
        .expect("projection deletion transaction commits");
}

fn review_record_request(
    reviewer: &Identity,
    lease_id: &str,
    fencing_token: u64,
    expected_revision: u64,
    idempotency_key: &str,
) -> RequestParams<Value> {
    let mut request = reviewer.params(json!({
        "operation":"review.record",
        "payload":{
            "status":"changes_required",
            "findings":[{
                "severity":"major",
                "code":"review.process.restart",
                "summary":"crash/restart projection coverage",
            }],
        },
    }));
    request.lease_id = Some(lease_id.to_owned());
    request.fencing_token = Some(fencing_token);
    request.expected_revision = Some(expected_revision);
    request.idempotency_key = Some(idempotency_key.to_owned());
    request
}

async fn acquire_review_lease(cli: &DaemonClient, reviewer: &Identity, key: &str) -> (String, u64) {
    let mut request = reviewer.params(json!({
        "operation":"lease.acquire",
        "payload":{"owner":{"role":"reviewer"},"ttlSeconds":900},
    }));
    request.idempotency_key = Some(key.to_owned());
    let response = cli
        .call::<Value>(RpcMethod::OperationExecute, request)
        .await
        .unwrap_or_else(|error| panic!("reviewer lease acquires: {error:?}"));
    (
        response["data"]["leaseId"]
            .as_str()
            .expect("reviewer lease id")
            .to_owned(),
        response["data"]["fencingToken"]
            .as_u64()
            .expect("reviewer fencing token"),
    )
}
