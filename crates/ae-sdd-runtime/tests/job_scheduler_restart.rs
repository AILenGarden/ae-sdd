mod support;

use std::sync::Arc;

use ae_sdd_domain::EventStoreId;
use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::{MemoryPersistence, RuntimeConfig};
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
