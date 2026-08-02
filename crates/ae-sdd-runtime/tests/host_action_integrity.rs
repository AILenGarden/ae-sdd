//! Host actions stay consistent with the delegations they belong to.
//!
//! Two integrity properties are asserted here, both of which were observed to
//! fail against a live daemon:
//!
//! - A committed `delegation.create` still publishes its action, so deferring
//!   publication past the authoritative commit did not cost liveness. The
//!   converse property, that a failed commit publishes nothing, is asserted
//!   directly against the coordinator in `host_coordinator`'s own tests, where
//!   the commit boundary can be exercised without a store that enforces every
//!   column constraint.
//! - One action carries at most one ACK. Recovery already enforced this over
//!   the durable records, but the write path did not: a host that re-acknowledged
//!   under a fresh identity wrote state the daemon then refused to load, so every
//!   later start failed permanently on the duplicate. The second ACK has to be
//!   refused at write time, while replaying the recorded one stays idempotent.

mod support;

use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::RuntimeConfig;
use serde_json::{Value, json};

use support::{
    Harness, create_root_series_delegation, open_root_session, params, register_workspace, result,
    stable_error,
};

const ADAPTER: &str = "host-a";

/// Registers the adapter on a host-adapter connection, the way a real host does
/// before it may take actions or acknowledge them.
fn host_connection(harness: &Harness) -> ae_sdd_runtime::ConnectionState {
    let mut host = harness.connection(ClientKind::HostAdapter);
    let mut register = params(
        json!({"adapterId":ADAPTER,"capabilities":["create","attest","ack"]}),
        1_000,
    );
    register.capability_token = Some(harness.host_credential());
    register.idempotency_key = Some("host-register".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostRegister, register));
    host
}

fn next_action(harness: &Harness, host: &mut ae_sdd_runtime::ConnectionState) -> Value {
    result(&harness.call(
        host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ))
}

#[test]
fn a_successful_create_still_publishes_its_action() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = host_connection(&harness);
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "published-action");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );

    let delegation = create_root_series_delegation(
        &harness,
        &mut root_connection,
        &workspace,
        &root,
        "root-agent",
        "WORK",
        "requirement-analysis",
        &["RA"],
        "create-valid",
    );

    let action = next_action(&harness, &mut host);
    assert_eq!(
        action["delegationId"], delegation["delegationId"],
        "a committed create must publish its own action"
    );
}

#[test]
fn a_second_ack_identity_for_one_action_is_refused() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = host_connection(&harness);
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "ack-order");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );

    let delegation = create_root_series_delegation(
        &harness,
        &mut root_connection,
        &workspace,
        &root,
        "root-agent",
        "WORK",
        "requirement-analysis",
        &["RA"],
        "create-for-ack",
    );
    let delegation_id = delegation["delegationId"]
        .as_str()
        .expect("delegation id")
        .to_owned();
    let action = next_action(&harness, &mut host);
    let child_session_id = "00000000-0000-0000-0000-000000000401";

    // First ACK omits the host task binding, so it cannot establish physical
    // proof. Its identity sorts before the usable ACK that follows.
    let mut incomplete = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-0000000000a1",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "sessionId":child_session_id
            }
        }),
        1_000,
    );
    incomplete.idempotency_key = Some("ack-incomplete".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, incomplete));

    let mut complete = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"ffffffff-ffff-4fff-8fff-ffffffffffff",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-1",
                "sessionId":child_session_id
            }
        }),
        1_000,
    );
    complete.idempotency_key = Some("ack-complete".to_owned());
    let response = harness.call(&mut host, RpcMethod::HostActionAck, complete);
    assert_eq!(
        stable_error(&response),
        "IDEMPOTENCY_KEY_REUSED",
        "a second ACK identity for one action must be refused, not silently stored"
    );

    // The refusal is what keeps the durable state loadable: recovery admits one
    // ACK per action, so a stored second one would make every later daemon
    // start fail on the duplicate.
    let _ = delegation_id;
    let mut replay = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-0000000000a1",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "sessionId":child_session_id
            }
        }),
        1_000,
    );
    replay.idempotency_key = Some("ack-replay".to_owned());
    let replayed = harness.call(&mut host, RpcMethod::HostActionAck, replay);
    assert!(
        replayed.get("error").is_none(),
        "replaying the recorded ACK must stay idempotent, got {}",
        stable_error(&replayed)
    );
}
