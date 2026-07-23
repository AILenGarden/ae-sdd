mod support;

use std::sync::atomic::Ordering;

use ae_sdd_protocol::{ClientKind, ConfirmationRef, RpcMethod, WorkspaceMode};
use ae_sdd_runtime::RuntimeConfig;
use serde_json::json;

use support::{
    Harness, open_root_session, params, parity_transition_payload, register_workspace, result,
    session_params,
};

#[test]
fn refreshed_context_revokes_a_previously_passing_hook_guard() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut cli, "context-refresh");
    let mut admin = harness.connection(ClientKind::Admin);
    let mut drain = params(json!({"stop":false}), 1_000);
    drain.idempotency_key = Some("drain-refresh".to_owned());
    drain.confirmation = Some(confirmation());
    let _ = result(&harness.call(&mut admin, RpcMethod::RuntimeDrain, drain));
    let mut transition = params(
        parity_transition_payload(WorkspaceMode::RustCanary, 1_000),
        1_000,
    );
    transition.workspace_id = Some(workspace.workspace_id);
    transition.idempotency_key = Some("mode-refresh".to_owned());
    transition.confirmation = Some(confirmation());
    let canary: ae_sdd_runtime::WorkspaceResult = serde_json::from_value(result(&harness.call(
        &mut admin,
        RpcMethod::WorkspaceModeTransition,
        transition,
    )))
    .expect("canary workspace");

    harness.business.pass_guard.store(1, Ordering::Release);
    let mut hook = harness.connection(ClientKind::Hook);
    let session = open_root_session(
        &harness,
        &mut hook,
        &canary,
        "agent",
        "external",
        Some("WORK"),
    );
    let allowed = result(&harness.call(
        &mut hook,
        RpcMethod::HookPreTool,
        hook_request(&canary, &session, "event-pass"),
    ));
    assert_eq!(allowed["decision"], "allow");

    harness.business.pass_guard.store(0, Ordering::Release);
    assert_eq!(
        harness
            .runtime
            .refresh_active_contexts()
            .expect("context refresh"),
        1
    );
    let denied = result(&harness.call(
        &mut hook,
        RpcMethod::HookPreTool,
        hook_request(&canary, &session, "event-deny"),
    ));
    assert_eq!(denied["decision"], "deny");
}

fn hook_request(
    workspace: &ae_sdd_runtime::WorkspaceResult,
    session: &ae_sdd_runtime::SessionResult,
    event_id: &str,
) -> ae_sdd_protocol::RequestParams<serde_json::Value> {
    let mut request = session_params(
        workspace,
        session,
        "agent",
        json!({"hookEventId":event_id,"turnSeq":1,"hostPayload":{}}),
        100,
    );
    request.turn_id = Some("turn".to_owned());
    request.work_item_id = Some("WORK".to_owned());
    request.idempotency_key = Some(format!("request-{event_id}"));
    request
}

fn confirmation() -> ConfirmationRef {
    ConfirmationRef {
        confirmation_id: "confirmation".to_owned(),
        approved_by: "user".to_owned(),
        approved_at: "2026-01-01T00:00:00Z".to_owned(),
    }
}
