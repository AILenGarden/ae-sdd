mod support;

use std::sync::atomic::Ordering;

use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::RuntimeConfig;
use serde_json::json;

use support::{
    Harness, open_root_session, register_workspace, result, session_params, stable_error,
};

#[test]
fn adjacent_digest_bound_projection_returns_a_real_delta() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, "context-delta");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent",
        "external",
        Some("WORK"),
    );
    let mut get = session_params(&workspace, &session, "agent", json!({}), 1_000);
    get.work_item_id = Some("WORK".to_owned());
    let initial = result(&harness.call(&mut connection, RpcMethod::ContextGet, get));

    harness.business.pass_guard.store(1, Ordering::Release);
    assert_eq!(harness.runtime.refresh_active_contexts().unwrap(), 1);
    let mut delta_request = session_params(
        &workspace,
        &session,
        "agent",
        json!({
            "knownRevision":initial["contextRevision"],
            "knownDigest":initial["digest"]
        }),
        1_000,
    );
    delta_request.work_item_id = Some("WORK".to_owned());
    let delta = result(&harness.call(&mut connection, RpcMethod::ContextProject, delta_request));
    assert_eq!(delta["kind"], "delta");
    assert_eq!(delta["projection"]["schemaVersion"], "context-delta/v1");
    assert_eq!(delta["projection"]["set"]["hookGuard"]["outcome"], "PASS");
    assert!(delta["byteLength"].as_u64().is_some_and(|bytes| bytes > 0));

    let mut mismatched = session_params(
        &workspace,
        &session,
        "agent",
        json!({
            "knownRevision":delta["contextRevision"],
            "knownDigest":"f".repeat(64)
        }),
        1_000,
    );
    mismatched.work_item_id = Some("WORK".to_owned());
    let rejected = harness.call(&mut connection, RpcMethod::ContextProject, mismatched);
    assert_eq!(stable_error(&rejected), "CONTEXT_REVISION_STALE");

    let mut wrong_work_item = session_params(&workspace, &session, "agent", json!({}), 1_000);
    wrong_work_item.work_item_id = Some("OTHER-WORK".to_owned());
    let rejected = harness.call(&mut connection, RpcMethod::ContextGet, wrong_work_item);
    assert_eq!(stable_error(&rejected), "TURN_IDENTITY_MISMATCH");
}
