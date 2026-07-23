mod support;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use ae_sdd_protocol::{ClientKind, ConfirmationRef, RpcMethod};
use ae_sdd_runtime::RuntimeConfig;
use serde_json::json;

use support::{
    Harness, open_root_session, params, register_workspace, result, session_params, stable_error,
};

#[test]
fn drain_waits_for_admitted_work_and_rejects_new_business_requests() {
    let harness = Arc::new(Harness::new(RuntimeConfig::default()));
    harness
        .business
        .operation_delay_ms
        .store(100, Ordering::Release);
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, "drain");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent",
        "external",
        Some("WORK"),
    );
    let mut operation = session_params(
        &workspace,
        &session,
        "agent",
        json!({"operation":"workitem.get","payload":{}}),
        1_000,
    );
    operation.work_item_id = Some("WORK".to_owned());
    let worker_harness = Arc::clone(&harness);
    let worker = std::thread::spawn(move || {
        worker_harness.call(&mut connection, RpcMethod::OperationExecute, operation)
    });
    let wait_started = Instant::now();
    while harness.business.operation_calls.load(Ordering::Acquire) == 0 {
        assert!(wait_started.elapsed() < Duration::from_secs(1));
        std::thread::yield_now();
    }

    let mut admin = harness.connection(ClientKind::Admin);
    let mut drain = params(json!({"stop":false}), 1_000);
    drain.idempotency_key = Some("drain-quiesce".to_owned());
    drain.confirmation = Some(ConfirmationRef {
        confirmation_id: "confirmation".to_owned(),
        approved_by: "user".to_owned(),
        approved_at: "2026-01-01T00:00:00Z".to_owned(),
    });
    let drain_started = Instant::now();
    let drained = result(&harness.call(&mut admin, RpcMethod::RuntimeDrain, drain));
    assert_eq!(drained["lifecycle"], "draining");
    assert!(drain_started.elapsed() >= Duration::from_millis(50));
    assert!(worker.join().expect("worker joins").get("result").is_some());

    let mut rejected = session_params(
        &workspace,
        &session,
        "agent",
        json!({"operation":"workitem.get","payload":{}}),
        1_000,
    );
    rejected.work_item_id = Some("WORK".to_owned());
    let response = harness.call(
        &mut harness.connection(ClientKind::Hook),
        RpcMethod::OperationExecute,
        rejected,
    );
    assert_eq!(stable_error(&response), "DAEMON_DRAINING");
}
