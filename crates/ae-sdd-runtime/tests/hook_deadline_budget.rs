mod support;

use std::sync::atomic::Ordering;

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

fn execution_request(
    workspace: &ae_sdd_runtime::WorkspaceResult,
    session: &ae_sdd_runtime::SessionResult,
    deadline_ms: u64,
    event: &str,
    execution_event: serde_json::Value,
) -> ae_sdd_protocol::RequestParams<serde_json::Value> {
    let mut request = session_params(
        workspace,
        session,
        "agent",
        json!({
            "hookEventId":event,
            "turnSeq":1,
            "hostPayload":{"executionEvent":execution_event},
        }),
        deadline_ms,
    );
    request.turn_id = Some("turn".to_owned());
    request.work_item_id = Some("WORK".to_owned());
    request.idempotency_key = Some(format!("request-{event}"));
    request
}

#[test]
fn execution_events_stay_on_the_bounded_business_free_fast_path() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, "deadline-execution");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent",
        "external",
        Some("WORK"),
    );

    let business_calls = || harness.business.operation_calls.load(Ordering::Acquire);
    let before = business_calls();

    let bounded = result(&harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        execution_request(
            &workspace,
            &session,
            250,
            "bounded",
            json!({"class":"source-read","outputBytes":128,"contentDigest":"a".repeat(64)}),
        ),
    ));
    assert_eq!(bounded["decision"], "allow");
    assert!(
        bounded.get("executionDirective").is_none(),
        "an unbound session stays shadow without a directive: {bounded}"
    );

    let over = harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        execution_request(
            &workspace,
            &session,
            251,
            "over",
            json!({"class":"source-read","outputBytes":128}),
        ),
    );
    assert_eq!(stable_error(&over), "OPERATION_SCHEMA_INVALID");

    let malformed = harness.call(
        &mut connection,
        RpcMethod::HookPreTool,
        execution_request(
            &workspace,
            &session,
            100,
            "malformed",
            json!({"class":"source-read","mystery":true}),
        ),
    );
    assert_eq!(stable_error(&malformed), "OPERATION_SCHEMA_INVALID");

    assert_eq!(
        business_calls(),
        before,
        "execution hook adjudication must not call the business authority or run Gates"
    );
}
