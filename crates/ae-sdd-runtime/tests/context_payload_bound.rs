mod support;

use std::sync::atomic::Ordering;

use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::RuntimeConfig;
use serde_json::json;

use support::{Harness, params, register_workspace, result, stable_error};

#[test]
fn oversized_authoritative_projection_is_rejected_before_it_can_be_injected() {
    let config = RuntimeConfig {
        max_context_projection_bytes: 128,
        ..RuntimeConfig::default()
    };
    let harness = Harness::new(config);
    harness
        .business
        .projection_bytes
        .store(129, Ordering::Release);
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, "context-bound");
    let mut open = params(
        json!({"externalKey":"external","role":"root","engaged":false}),
        1_000,
    );
    open.workspace_id = Some(workspace.workspace_id.clone());
    open.agent_id = Some("agent".to_owned());
    open.work_item_id = Some("WORK".to_owned());
    open.idempotency_key = Some("session-open".to_owned());

    let response = harness.call(&mut connection, RpcMethod::SessionOpen, open);
    assert_eq!(stable_error(&response), "CONTEXT_BUDGET_EXCEEDED");
    let status = result(&harness.call(
        &mut connection,
        RpcMethod::RuntimeStatus,
        params(json!({}), 1_000),
    ));
    assert_eq!(status["sessionCount"], 0);
}
