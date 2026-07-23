mod support;

use std::sync::atomic::Ordering;

use ae_sdd_protocol::{ClientKind, ConfirmationRef, RpcMethod, WorkspaceMode};
use ae_sdd_runtime::RuntimeConfig;
use serde_json::json;

use support::{
    Harness, open_root_session, params, parity_transition_payload, register_workspace, result,
    session_params, stable_error,
};

#[test]
fn shadow_is_read_only_and_canary_invalidates_unengaged_sessions_before_writes() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut hook_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut hook_connection, "writer-mode");
    let shadow_session = open_root_session(
        &harness,
        &mut hook_connection,
        &workspace,
        "agent",
        "external",
        Some("WORK"),
    );

    let mut read = session_params(
        &workspace,
        &shadow_session,
        "agent",
        json!({"operation":"workitem.get","payload":{}}),
        1_000,
    );
    read.work_item_id = Some("WORK".to_owned());
    assert!(
        harness
            .call(&mut hook_connection, RpcMethod::OperationExecute, read)
            .get("result")
            .is_some()
    );

    let mut write = session_params(
        &workspace,
        &shadow_session,
        "agent",
        json!({"operation":"lease.acquire","payload":{}}),
        1_000,
    );
    write.work_item_id = Some("WORK".to_owned());
    write.idempotency_key = Some("shadow-write".to_owned());
    let denied = harness.call(&mut hook_connection, RpcMethod::OperationExecute, write);
    assert_eq!(stable_error(&denied), "ROLE_OPERATION_FORBIDDEN");
    assert_eq!(harness.business.operation_calls.load(Ordering::Acquire), 1);

    let mut admin = harness.connection(ClientKind::Admin);
    let mut drain = params(json!({"stop":false}), 1_000);
    drain.idempotency_key = Some("drain-canary".to_owned());
    drain.confirmation = Some(confirmation());
    assert!(
        harness
            .call(&mut admin, RpcMethod::RuntimeDrain, drain)
            .get("result")
            .is_some()
    );
    let mut during_drain = session_params(
        &workspace,
        &shadow_session,
        "agent",
        json!({"operation":"workitem.get","payload":{}}),
        1_000,
    );
    during_drain.work_item_id = Some("WORK".to_owned());
    let draining = harness.call(
        &mut hook_connection,
        RpcMethod::OperationExecute,
        during_drain,
    );
    assert_eq!(stable_error(&draining), "DAEMON_DRAINING");
    let mut transition = params(
        parity_transition_payload(WorkspaceMode::RustCanary, 1_000),
        1_000,
    );
    transition.workspace_id = Some(workspace.workspace_id.clone());
    transition.idempotency_key = Some("mode-canary".to_owned());
    transition.confirmation = Some(confirmation());
    let canary: ae_sdd_runtime::WorkspaceResult = serde_json::from_value(result(&harness.call(
        &mut admin,
        RpcMethod::WorkspaceModeTransition,
        transition,
    )))
    .expect("canary workspace decodes");
    assert_eq!(canary.mode, WorkspaceMode::RustCanary);

    let mut stale_read = session_params(
        &canary,
        &shadow_session,
        "agent",
        json!({"operation":"workitem.get","payload":{}}),
        1_000,
    );
    stale_read.work_item_id = Some("WORK".to_owned());
    let stale = harness.call(
        &mut hook_connection,
        RpcMethod::OperationExecute,
        stale_read,
    );
    assert_eq!(stable_error(&stale), "SESSION_EXPIRED");

    let canary_session = open_root_session(
        &harness,
        &mut hook_connection,
        &canary,
        "agent",
        "external",
        Some("WORK"),
    );
    assert!(canary_session.engaged);
    let mut canary_write = session_params(
        &canary,
        &canary_session,
        "agent",
        json!({"operation":"lease.acquire","payload":{}}),
        1_000,
    );
    canary_write.work_item_id = Some("WORK".to_owned());
    canary_write.idempotency_key = Some("canary-write".to_owned());
    assert!(
        harness
            .call(
                &mut hook_connection,
                RpcMethod::OperationExecute,
                canary_write,
            )
            .get("result")
            .is_some()
    );
    assert_eq!(harness.business.operation_calls.load(Ordering::Acquire), 2);
}

#[test]
fn cutover_rejects_untyped_or_mismatched_parity_digest() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut cli, "bad-parity");
    let mut admin = harness.connection(ClientKind::Admin);
    let mut drain = params(json!({"stop":false}), 1_000);
    drain.idempotency_key = Some("drain-bad".to_owned());
    drain.confirmation = Some(confirmation());
    let _ = harness.call(&mut admin, RpcMethod::RuntimeDrain, drain);

    let mut payload = parity_transition_payload(WorkspaceMode::RustCanary, 1_000);
    payload["parityDigest"] = json!("b".repeat(64));
    let mut transition = params(payload, 1_000);
    transition.workspace_id = Some(workspace.workspace_id);
    transition.idempotency_key = Some("mode-bad".to_owned());
    transition.confirmation = Some(confirmation());
    let response = harness.call(&mut admin, RpcMethod::WorkspaceModeTransition, transition);
    assert_eq!(stable_error(&response), "EXTERNAL_STATE_CONFLICT");
}

fn confirmation() -> ConfirmationRef {
    ConfirmationRef {
        confirmation_id: "test-confirmation".to_owned(),
        approved_by: "test-user".to_owned(),
        approved_at: "2026-01-01T00:00:00Z".to_owned(),
    }
}
