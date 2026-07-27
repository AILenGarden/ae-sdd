mod support;

use std::sync::Arc;

use ae_sdd_domain::EventStoreId;
use ae_sdd_protocol::{ClientKind, RpcMethod, WorkspaceMode};
use ae_sdd_runtime::{
    DurableEvent, MemoryPersistence, PersistencePort, RuntimeConfig, RuntimeJobRecord,
    RuntimeJobStatus, RuntimeJobTransition, ScopedGrantWire, WireAgentRole,
};
use serde_json::json;
use uuid::Uuid;

use support::{Harness, params, register_workspace, result};

#[test]
fn queued_job_recovers_runs_once_and_publishes_lifecycle_events() {
    let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
        Uuid::from_u128(101),
    )));
    let first = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        102,
        "first-token".to_owned(),
    );
    let mut first_connection = first.connection(ClientKind::Cli);
    let workspace = register_workspace(&first, &mut first_connection, "job-restart");
    let mut submit = params(
        json!({"entrypoint":"assets.read","arguments":{},"deadlineUnixMs":2_000}),
        1_000,
    );
    submit.workspace_id = Some(workspace.workspace_id.clone());
    submit.work_item_id = Some("WORK".to_owned());
    submit.idempotency_key = Some("job-submit".to_owned());
    let queued = result(&first.call(&mut first_connection, RpcMethod::JobSubmit, submit));
    assert_eq!(queued["status"], "queued");
    let job_id = queued["jobId"].as_str().expect("job id").to_owned();

    let second = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence,
        103,
        "second-token".to_owned(),
    );
    second.runtime.recover().expect("runtime recovers");
    assert!(second.runtime.run_one_pending_job().expect("job runs"));
    assert!(
        !second
            .runtime
            .run_one_pending_job()
            .expect("queue is empty")
    );

    let mut second_connection = second.connection(ClientKind::Cli);
    let mut status = params(json!({"jobId":job_id}), 1_000);
    status.workspace_id = Some(workspace.workspace_id.clone());
    status.work_item_id = Some("WORK".to_owned());
    let completed = result(&second.call(&mut second_connection, RpcMethod::JobStatus, status));
    assert_eq!(completed["status"], "pass");
    assert_eq!(completed["result"]["outcome"], "PASS");

    let mut events = params(
        json!({
            "eventStoreId":second.runtime.event_store_id().expect("store id").to_string(),
            "afterEventSeq":0,
            "limit":32
        }),
        1_000,
    );
    events.workspace_id = Some(workspace.workspace_id);
    let batch = result(&second.call(&mut second_connection, RpcMethod::EventsSubscribe, events));
    let kinds = batch["events"]
        .as_array()
        .expect("events array")
        .iter()
        .filter_map(|event| event["kind"].as_str())
        .collect::<Vec<_>>();
    assert!(kinds.contains(&"job.submitted"));
    assert!(kinds.contains(&"job.started"));
    assert!(kinds.contains(&"job.completed"));
}

#[test]
fn queued_job_can_be_cancelled_idempotently_before_execution() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut connection, "job-cancel");
    let mut submit = params(
        json!({"entrypoint":"assets.read","arguments":{},"deadlineUnixMs":2_000}),
        1_000,
    );
    submit.workspace_id = Some(workspace.workspace_id.clone());
    submit.idempotency_key = Some("submit-cancel".to_owned());
    let job = result(&harness.call(&mut connection, RpcMethod::JobSubmit, submit));
    let mut cancel = params(json!({"jobId":job["jobId"]}), 1_000);
    cancel.workspace_id = Some(workspace.workspace_id);
    cancel.idempotency_key = Some("cancel-job".to_owned());
    let first = result(&harness.call(&mut connection, RpcMethod::JobCancel, cancel));
    assert_eq!(first["status"], "cancelled");
    assert!(harness.runtime.run_one_pending_job().expect("queue drains"));
    assert!(
        !harness
            .runtime
            .run_one_pending_job()
            .expect("queue is empty")
    );
}

#[test]
fn old_boot_identity_bound_queued_and_running_jobs_become_stale() {
    let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
        Uuid::from_u128(201),
    )));
    let first = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        202,
        "first-token".to_owned(),
    );
    let mut connection = first.connection(ClientKind::Cli);
    let workspace = register_workspace(&first, &mut connection, "job-stale");
    let digest = "a".repeat(64);

    for (ordinal, initial_status) in [RuntimeJobStatus::Queued, RuntimeJobStatus::Running]
        .into_iter()
        .enumerate()
    {
        let job_id = format!("old-boot-{ordinal}");
        let mut record = RuntimeJobRecord {
            job_id: job_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            work_item_id: Some("WORK".to_owned()),
            session_id: Some(format!("session-{ordinal}")),
            root_session_id: Some(format!("session-{ordinal}")),
            delegation_id: None,
            agent_role: Some(WireAgentRole::Root),
            context_generation: Some(1),
            submission_boot_id: Some(first.runtime.boot_id().to_string()),
            attestation_ref: Some(format!("capability:session-{ordinal}")),
            attestation_digest: Some(digest.clone()),
            grant: Some(ScopedGrantWire::default()),
            identity_digest: Some(digest.clone()),
            workspace_mode: WorkspaceMode::Legacy,
            inventory_generation: workspace.inventory_generation,
            entrypoint: "toolset.required".to_owned(),
            arguments: json!({}),
            submission_scope_digest: digest.clone(),
            submission_idempotency_key: format!("submit-{ordinal}"),
            submission_idempotency_key_digest: digest.clone(),
            request_digest: digest.clone(),
            source_revision: Some(1),
            input_fingerprint: Some(digest.clone()),
            deadline_unix_ms: 2_000,
            status: RuntimeJobStatus::Queued,
            row_version: 0,
            result: None,
            error_code: None,
            mutation_id: None,
            receipt_locator: None,
            project_receipt_digest: None,
            submitted_event_seq: 0,
            last_event_seq: 0,
            created_at_unix_ms: 900,
            started_at_unix_ms: None,
            finished_at_unix_ms: None,
            updated_at_unix_ms: 900,
        };
        record = persistence
            .commit_job_transition(RuntimeJobTransition {
                record,
                expected_status: None,
                expected_row_version: None,
                event: job_event(&job_id, "job.submitted"),
            })
            .expect("queued job persists");
        if initial_status == RuntimeJobStatus::Running {
            let expected_version = record.row_version;
            record.status = RuntimeJobStatus::Running;
            record.row_version += 1;
            record.started_at_unix_ms = Some(950);
            record.updated_at_unix_ms = 950;
            persistence
                .commit_job_transition(RuntimeJobTransition {
                    record,
                    expected_status: Some(RuntimeJobStatus::Queued),
                    expected_row_version: Some(expected_version),
                    event: job_event(&job_id, "job.started"),
                })
                .expect("running job persists");
        }
    }

    let second = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        203,
        "second-token".to_owned(),
    );
    second.runtime.recover().expect("runtime recovers");
    assert!(
        !second
            .runtime
            .run_one_pending_job()
            .expect("queue stays empty")
    );

    for ordinal in 0..2 {
        let job = persistence
            .load_job(&format!("old-boot-{ordinal}"))
            .expect("job loads")
            .expect("job exists");
        assert_eq!(job.status, RuntimeJobStatus::Stale);
        assert_eq!(job.result, Some(json!({"errorCode":"SESSION_EXPIRED"})));
        assert_eq!(job.error_code, None);
        assert_eq!(job.finished_at_unix_ms, Some(1_000));
        assert_eq!(
            job.started_at_unix_ms,
            Some(if ordinal == 0 { 1_000 } else { 950 })
        );
    }
}

fn job_event(job_id: &str, kind: &str) -> DurableEvent {
    DurableEvent {
        event_store_id: String::new(),
        event_seq: 0,
        boot_id: Uuid::from_u128(202).to_string(),
        kind: kind.to_owned(),
        workspace_id: None,
        session_id: None,
        work_item_id: None,
        payload: json!({"jobId":job_id}),
        payload_digest: "b".repeat(64),
    }
}
