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
fn accepted_host_ack_returns_exact_child_binding() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = host_connection(&harness);
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "ack-recovery-facts");
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
        "create-for-ack-recovery",
    );
    let action = next_action(&harness, &mut host);
    let ack_payload = json!({
        "adapterId":ADAPTER,
        "ack": {
            "ackId":"00000000-0000-0000-0000-000000000402",
            "actionId":action["actionId"],
            "commandSeq":action["commandSeq"],
            "outcome":"accepted",
            "hostTaskId":"/root/requirement_analysis_series",
            "sessionId":"00000000-0000-0000-0000-000000000403"
        }
    });
    let mut ack = params(ack_payload.clone(), 1_000);
    ack.idempotency_key = Some("ack-recovery-facts".to_owned());
    let accepted = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));

    assert_eq!(accepted["actionId"], action["actionId"]);
    assert_eq!(accepted["delegationId"], delegation["delegationId"]);
    assert_eq!(accepted["ackId"], "00000000-0000-0000-0000-000000000402");
    assert_eq!(accepted["outcome"], "accepted");
    assert_eq!(accepted["hostTaskId"], "/root/requirement_analysis_series");
    assert_eq!(
        accepted["childSessionId"],
        "00000000-0000-0000-0000-000000000403"
    );
    assert!(
        accepted.get("claimId").is_none(),
        "boot-local claims must never enter a durable ACK response: {accepted}"
    );

    let mut replay = params(ack_payload, 1_000);
    replay.idempotency_key = Some("ack-recovery-facts".to_owned());
    let replayed = result(&harness.call(&mut host, RpcMethod::HostActionAck, replay));
    assert_eq!(
        replayed, accepted,
        "ACK receipt replay must preserve recovery facts"
    );
}

#[test]
fn unknown_ack_outcome_does_not_consume_the_create_action() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = host_connection(&harness);
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "ack-outcome-validation");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );
    let _ = create_root_series_delegation(
        &harness,
        &mut root_connection,
        &workspace,
        &root,
        "root-agent",
        "WORK",
        "requirement-analysis",
        &["RA"],
        "create-for-outcome-validation",
    );
    let action = next_action(&harness, &mut host);
    let child_session_id = "00000000-0000-0000-0000-000000000406";
    let mut unknown = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000000407",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"maybe",
                "hostTaskId":"host-task-unknown",
                "sessionId":child_session_id
            }
        }),
        1_000,
    );
    unknown.idempotency_key = Some("ack-outcome-unknown".to_owned());
    let rejected = harness.call(&mut host, RpcMethod::HostActionAck, unknown);
    assert_eq!(stable_error(&rejected), "DELEGATION_ATTESTATION_FAILED");

    let mut accepted = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000000408",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-valid",
                "sessionId":child_session_id
            }
        }),
        1_000,
    );
    accepted.idempotency_key = Some("ack-outcome-valid".to_owned());
    let accepted = result(&harness.call(&mut host, RpcMethod::HostActionAck, accepted));
    assert_eq!(accepted["outcome"], "accepted");
}

#[test]
fn rejected_create_ack_moves_the_delegation_to_a_terminal_state() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = host_connection(&harness);
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "ack-rejected-terminal");
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
        "create-for-rejection",
    );
    let action = next_action(&harness, &mut host);
    let mut rejected = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {"ackId":"00000000-0000-0000-0000-000000000409","actionId":action["actionId"],"commandSeq":action["commandSeq"],"outcome":"rejected"}
        }),
        1_000,
    );
    rejected.idempotency_key = Some("ack-rejected".to_owned());
    let ack = result(&harness.call(&mut host, RpcMethod::HostActionAck, rejected));
    assert_eq!(ack["outcome"], "rejected");
    assert_eq!(
        harness
            .runtime
            .delegation_supervisor()
            .status(
                &root.session_id,
                delegation["delegationId"].as_str().expect("delegation id")
            )
            .expect("delegation status")
            .status,
        "cancelled"
    );
}

#[test]
fn accepted_create_ack_requires_the_complete_child_binding() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = host_connection(&harness);
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "ack-binding-required");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );
    let _ = create_root_series_delegation(
        &harness,
        &mut root_connection,
        &workspace,
        &root,
        "root-agent",
        "WORK",
        "requirement-analysis",
        &["RA"],
        "create-for-ack-binding",
    );
    let action = next_action(&harness, &mut host);
    let mut incomplete = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000000404",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "sessionId":"00000000-0000-0000-0000-000000000405"
            }
        }),
        1_000,
    );
    incomplete.idempotency_key = Some("ack-binding-incomplete".to_owned());
    let rejected = harness.call(&mut host, RpcMethod::HostActionAck, incomplete);
    assert_eq!(stable_error(&rejected), "DELEGATION_ATTESTATION_FAILED");
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

    let mut first = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-0000000000a1",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-first",
                "sessionId":child_session_id
            }
        }),
        1_000,
    );
    first.idempotency_key = Some("ack-first".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, first));

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
                "hostTaskId":"host-task-first",
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
