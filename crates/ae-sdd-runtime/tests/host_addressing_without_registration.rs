//! A host is reachable from the connection it opens, with nothing to register.
//!
//! The daemon does not create child sessions; it posts errands and a host picks
//! them up, runs them in its own process, and says what happened in the ACK.
//! So there is nothing about a host to authorize in advance and nothing it
//! could usefully declare: the only question the daemon can answer before
//! dispatch is whether it knows where to deliver.
//!
//! That question is settled by the handshake, which already proves the host
//! holds the boot credential. What used to be an explicit registration step is
//! now implied by connecting, and `host adapter is not registered` means what it
//! always really meant -- no such recipient.

mod support;

use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::RuntimeConfig;
use serde_json::{Value, json};

use support::{
    Harness, create_root_series_delegation, flow_decision_digest, open_root_session, params,
    register_workspace, result, session_params, stable_error,
};

const ADAPTER: &str = "host-fresh";
const CHILD: &str = "00000000-0000-0000-0000-000000000601";

/// Drives create → action_next → ack → accept without ever calling
/// `host.register`, which is the whole claim of S4-1.
#[test]
fn a_freshly_connected_host_completes_the_delegation_chain() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));

    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "fresh");
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
        "fresh-create",
    );
    let delegation_id = delegation["delegationId"]
        .as_str()
        .expect("delegation id")
        .to_owned();

    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let mut ack = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000000602",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-fresh",
                "sessionId":CHILD
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("fresh-ack".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));

    let mut accept = params(
        json!({
            "delegationId":delegation_id,
            "claimId":action["claimId"],
            "actionId":action["actionId"],
            "childSessionId":CHILD,
            "expiresAtUnixMs":4_900
        }),
        1_000,
    );
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.work_item_id = Some("WORK".to_owned());
    accept.idempotency_key = Some("fresh-accept".to_owned());
    let accepted = result(&harness.call(&mut root_connection, RpcMethod::DelegationAccept, accept));
    assert_eq!(
        accepted["status"], "running",
        "the chain must complete with no registration step"
    );
}

/// S4-4: Root does not choose a Host recipient. With no Host attached, the
/// daemon must fail closed instead of accepting caller-supplied authority.
#[test]
fn delegation_fails_when_no_host_is_attached() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "unknown");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );

    let decision_digest = flow_decision_digest("unknown-create");
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":decision_digest,
        "stateRevision":1,
        "phase":"initialized",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"requirement-analysis",
            "requiredArtifacts":["RA"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some("WORK".to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":decision_digest}),
        1_000,
    );
    create.work_item_id = Some("WORK".to_owned());
    create.idempotency_key = Some("unknown-create".to_owned());
    let response = harness.call(&mut root_connection, RpcMethod::DelegationCreate, create);
    assert_eq!(stable_error(&response), "HOST_CAPABILITY_UNSUPPORTED");
    let message = response["error"]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("{response}"));
    assert!(
        message.contains("no Host adapter is attached"),
        "the missing Host must be explicit: {message}"
    );
}

/// S4-2: hosts built against the older shape still send `capabilities`. The
/// field no longer means anything, but rejecting it would break a host that has
/// done nothing wrong, so it is ignored.
#[test]
fn a_registration_carrying_capabilities_still_succeeds() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut register = params(
        json!({"adapterId":ADAPTER,"capabilities":["create","attest","ack"]}),
        1_000,
    );
    register.capability_token = Some(harness.host_credential());
    register.idempotency_key = Some("legacy-register".to_owned());
    let registered: Value = result(&harness.call(&mut host, RpcMethod::HostRegister, register));
    assert_eq!(registered["adapterId"], ADAPTER);
    assert!(
        registered.get("capabilities").is_none(),
        "the ignored field must not be echoed back as if it meant something: {registered}"
    );
}

/// S4-5: the removed method is genuinely gone rather than returning an empty
/// shell that would read as "this host declares nothing".
#[test]
fn the_capability_matrix_method_no_longer_exists() {
    let payload = json!({
        "jsonrpc":"2.0",
        "id":"gone",
        "method":"host.capabilities",
        "params":{"adapterId":ADAPTER}
    });
    assert!(
        serde_json::from_value::<ae_sdd_protocol::JsonRpcRequest<Value>>(payload).is_err(),
        "an unknown method must fail to decode rather than acquire a default profile"
    );
}
