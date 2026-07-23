mod support;

use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::RuntimeConfig;
use serde_json::json;

use support::{
    Harness, open_root_session, register_workspace, result, session_params, stable_error,
};

fn hook_request(
    workspace: &ae_sdd_runtime::WorkspaceResult,
    session: &ae_sdd_runtime::SessionResult,
    agent_id: &str,
    event_id: &str,
) -> ae_sdd_protocol::RequestParams<serde_json::Value> {
    let mut request = session_params(
        workspace,
        session,
        agent_id,
        json!({
            "hookEventId": event_id,
            "turnSeq": 1,
            "hostPayload": {"tool":"read"},
        }),
        100,
    );
    request.turn_id = Some("turn-1".to_owned());
    request.work_item_id = Some("WORK-1".to_owned());
    request.idempotency_key = Some(format!("request-{event_id}"));
    request
}

#[test]
fn duplicate_hook_event_replays_the_original_receipt() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, "hook-replay");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-a",
        "external-a",
        Some("WORK-1"),
    );

    let first = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(&workspace, &session, "agent-a", "event-1"),
    ));
    let replay = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        hook_request(&workspace, &session, "agent-a", "event-1"),
    ));

    assert_eq!(first["eventSeq"], replay["eventSeq"]);
    assert_eq!(first["replayed"], false);
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["decision"], "allow");
}

#[test]
fn replayed_capability_cannot_cross_session_identity() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, "capability-binding");
    let session_a = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-a",
        "external-a",
        Some("WORK-1"),
    );
    let session_b = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-b",
        "external-b",
        Some("WORK-1"),
    );
    let mut forged = hook_request(&workspace, &session_b, "agent-b", "event-forged");
    forged.capability_token = Some(session_a.capability_token);

    let response = harness.call(&mut connection, RpcMethod::HookPreTool, forged);
    assert_eq!(stable_error(&response), "TURN_IDENTITY_MISMATCH");
}

#[test]
fn duplicate_event_id_with_mutated_payload_is_rejected() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, "hook-conflict");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-a",
        "external-a",
        Some("WORK-1"),
    );
    let first = hook_request(&workspace, &session, "agent-a", "event-1");
    assert!(
        harness
            .call(&mut connection, RpcMethod::HookPreTool, first)
            .get("result")
            .is_some()
    );
    let mut changed = hook_request(&workspace, &session, "agent-a", "event-1");
    changed.payload["hostPayload"] = json!({"tool":"write"});
    let response = harness.call(&mut connection, RpcMethod::HookPreTool, changed);
    assert_eq!(stable_error(&response), "IDEMPOTENCY_KEY_REUSED");
}
