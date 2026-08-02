//! Physical child claims are daemon-issued and delivered only on the Host lane.

mod support;

use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::RuntimeConfig;
use serde_json::json;

use support::{
    Harness, open_root_session, params, register_workspace, result, session_params, stable_error,
};

const ADAPTER: &str = "host-claim-owner";
const CHILD: &str = "00000000-0000-0000-0000-000000001101";
const DECISION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[test]
fn only_the_claim_delivered_to_the_host_can_bootstrap_the_child() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "host-owned-claim");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );

    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
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
    let _ = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some("WORK".to_owned());
    create.idempotency_key = Some("host-owned-claim-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    let delegation_id = delegation["delegationId"]
        .as_str()
        .expect("delegation id")
        .to_owned();
    assert!(
        delegation.get("claimId").is_none(),
        "the Root response must not receive the child bootstrap claim: {delegation}"
    );

    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let claim_id = action["claimId"]
        .as_str()
        .unwrap_or_else(|| panic!("Host delivery must carry the daemon-issued claim: {action}"))
        .to_owned();

    let mut ack = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000001102",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-claim-owner",
                "sessionId":CHILD
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("host-owned-claim-ack".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));

    let accept_params = |claim_id: &str, key: &str| {
        let mut accept = params(
            json!({
                "delegationId":delegation_id,
                "claimId":claim_id,
                "actionId":action["actionId"],
                "childSessionId":CHILD,
                "expiresAtUnixMs":4_900
            }),
            1_000,
        );
        accept.workspace_id = Some(workspace.workspace_id.clone());
        accept.work_item_id = Some("WORK".to_owned());
        accept.idempotency_key = Some(key.to_owned());
        accept
    };

    let forged = harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        accept_params(
            "00000000-0000-0000-0000-000000001103",
            "host-owned-claim-forged",
        ),
    );
    assert_eq!(
        stable_error(&forged),
        "DELEGATION_ATTESTATION_FAILED",
        "a caller-minted UUID must not be accepted: {forged}"
    );

    let accepted = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        accept_params(&claim_id, "host-owned-claim-accept"),
    ));
    assert_eq!(accepted["status"], "running");
}
