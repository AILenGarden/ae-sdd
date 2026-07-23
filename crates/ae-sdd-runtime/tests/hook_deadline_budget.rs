mod support;

use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::RuntimeConfig;
use serde_json::json;

use support::{
    Harness, open_root_session, register_workspace, result, session_params, stable_error,
};

fn request(
    workspace: &ae_sdd_runtime::WorkspaceResult,
    session: &ae_sdd_runtime::SessionResult,
    deadline_ms: u64,
    event: &str,
) -> ae_sdd_protocol::RequestParams<serde_json::Value> {
    let mut request = session_params(
        workspace,
        session,
        "agent",
        json!({"hookEventId":event,"turnSeq":1,"hostPayload":{}}),
        deadline_ms,
    );
    request.turn_id = Some("turn".to_owned());
    request.work_item_id = Some("WORK".to_owned());
    request.idempotency_key = Some(format!("request-{event}"));
    request
}

#[test]
fn hook_deadline_is_capped_by_the_negotiated_fast_path_budget() {
    let config = RuntimeConfig {
        hook_deadline_ms: 25,
        ..RuntimeConfig::default()
    };
    let harness = Harness::new(config);
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, "deadline");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent",
        "external",
        Some("WORK"),
    );

    let over = harness.call(
        &mut connection,
        RpcMethod::HookPostTool,
        request(&workspace, &session, 26, "over"),
    );
    assert_eq!(stable_error(&over), "OPERATION_SCHEMA_INVALID");

    let at_limit = result(&harness.call(
        &mut connection,
        RpcMethod::HookPostTool,
        request(&workspace, &session, 25, "at-limit"),
    ));
    assert_eq!(at_limit["decision"], "allow");

    let zero = harness.call(
        &mut connection,
        RpcMethod::HookPostTool,
        request(&workspace, &session, 0, "zero"),
    );
    assert_eq!(stable_error(&zero), "OPERATION_SCHEMA_INVALID");
}
