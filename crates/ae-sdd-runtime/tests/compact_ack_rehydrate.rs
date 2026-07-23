mod support;

use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::RuntimeConfig;
use serde_json::json;

use support::{Harness, open_root_session, params, register_workspace, result, session_params};

#[test]
fn compact_requires_correlated_host_ack_then_rehydrate_cas_and_emits_one_completion() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection(ClientKind::HostAdapter);
    let mut register = params(
        json!({"adapterId":"host-a","capabilities":["compact"]}),
        1_000,
    );
    register.capability_token = Some(harness.host_credential());
    register.idempotency_key = Some("host-register".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostRegister, register));

    let mut hook = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut hook, "compact");
    let session = open_root_session(
        &harness,
        &mut hook,
        &workspace,
        "agent",
        "external",
        Some("WORK"),
    );
    let mut context_get = session_params(&workspace, &session, "agent", json!({}), 1_000);
    context_get.work_item_id = Some("WORK".to_owned());
    let projection = result(&harness.call(&mut hook, RpcMethod::ContextGet, context_get));

    let mut compact = session_params(
        &workspace,
        &session,
        "agent",
        json!({
            "previousGeneration":0,
            "snapshotDigest":"a".repeat(64),
            "deadlineUnixMs":2_000,
            "adapterId":"host-a"
        }),
        1_000,
    );
    compact.work_item_id = Some("WORK".to_owned());
    compact.idempotency_key = Some("compact-request".to_owned());
    let cycle = result(&harness.call(&mut hook, RpcMethod::CompactRequest, compact));
    assert_eq!(cycle["status"], "compact-requested");

    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":"host-a"}), 1_000),
    ));
    assert_eq!(action["actionId"], cycle["actionId"]);
    let mut ack = params(
        json!({
            "adapterId":"host-a",
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000000201",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":null,
                "sessionId":null
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("compact-ack".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));

    let mut before_rehydrate = session_params(
        &workspace,
        &session,
        "agent",
        json!({"compactId":cycle["compactId"]}),
        1_000,
    );
    before_rehydrate.work_item_id = Some("WORK".to_owned());
    let acknowledged = result(&harness.call(&mut hook, RpcMethod::CompactStatus, before_rehydrate));
    assert_eq!(acknowledged["status"], "host-acknowledged");

    let mut rehydrate = session_params(
        &workspace,
        &session,
        "agent",
        json!({
            "knownRevision":projection["contextRevision"],
            "knownDigest":projection["digest"]
        }),
        1_000,
    );
    rehydrate.work_item_id = Some("WORK".to_owned());
    let restored = result(&harness.call(&mut hook, RpcMethod::ContextProject, rehydrate));
    assert_eq!(restored["kind"], "no_change");

    let mut compact_status = session_params(
        &workspace,
        &session,
        "agent",
        json!({"compactId":cycle["compactId"]}),
        1_000,
    );
    compact_status.work_item_id = Some("WORK".to_owned());
    let completed = result(&harness.call(&mut hook, RpcMethod::CompactStatus, compact_status));
    assert_eq!(completed["status"], "context-restored");
    assert_eq!(completed["restoredProjectionDigest"], projection["digest"]);

    let mut heartbeat = session_params(&workspace, &session, "agent", json!({}), 1_000);
    heartbeat.idempotency_key = Some("heartbeat-after-compact".to_owned());
    let heartbeat = result(&harness.call(&mut hook, RpcMethod::SessionHeartbeat, heartbeat));
    assert_eq!(heartbeat["contextGeneration"], 1);

    let mut events = params(
        json!({
            "eventStoreId":harness.runtime.event_store_id().expect("store id").to_string(),
            "afterEventSeq":0,
            "limit":32
        }),
        1_000,
    );
    events.workspace_id = Some(workspace.workspace_id);
    let batch = result(&harness.call(&mut hook, RpcMethod::EventsSubscribe, events));
    let completion_count = batch["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter(|event| event["kind"] == "compact.context_restored")
        .count();
    assert_eq!(completion_count, 1);
}
