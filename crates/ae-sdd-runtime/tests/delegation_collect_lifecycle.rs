mod support;

use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::{PersistencePort, RuntimeConfig};
use serde_json::json;

use support::{Harness, open_root_session, params, register_workspace, result, session_params};

#[test]
fn parent_collects_only_after_artifact_and_memory_receipts_are_persisted() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection(ClientKind::HostAdapter);
    let mut host_register = params(
        json!({"adapterId":"host-a","capabilities":["create","attest"]}),
        1_000,
    );
    host_register.capability_token = Some(harness.host_credential());
    host_register.idempotency_key = Some("host-register".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostRegister, host_register));

    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "delegation");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );
    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({
            "childRole":"series",
            "parentDelegationId":null,
            "inputRevision":1,
            "inputFingerprint":"a".repeat(64),
            "deadlineUnixMs":2_000,
            "adapterId":"host-a"
        }),
        1_000,
    );
    create.work_item_id = Some("WORK".to_owned());
    create.idempotency_key = Some("delegation-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    let delegation_id = delegation["delegationId"]
        .as_str()
        .expect("delegation id")
        .to_owned();

    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":"host-a"}), 1_000),
    ));
    let child_session_id = "00000000-0000-0000-0000-000000000401";
    let mut ack = params(
        json!({
            "adapterId":"host-a",
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000000402",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-1",
                "sessionId":child_session_id
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("host-create-ack".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));

    let mut accept = params(
        json!({
            "delegationId":delegation_id,
            "claimId":"00000000-0000-0000-0000-000000000403",
            "actionId":action["actionId"],
            "childSessionId":child_session_id,
            "expiresAtUnixMs":1_900
        }),
        1_000,
    );
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.work_item_id = Some("WORK".to_owned());
    accept.idempotency_key = Some("delegation-accept".to_owned());
    let accepted = result(&harness.call(&mut root_connection, RpcMethod::DelegationAccept, accept));
    assert_eq!(accepted["status"], "running");

    let mut child_open = params(
        json!({
            "externalKey":"series-external",
            "role":"series",
            "engaged":false,
            "delegationId":delegation_id
        }),
        1_000,
    );
    child_open.workspace_id = Some(workspace.workspace_id.clone());
    child_open.agent_id = Some("series-agent".to_owned());
    child_open.session_id = Some(child_session_id.to_owned());
    child_open.work_item_id = Some("WORK".to_owned());
    child_open.idempotency_key = Some("series-open".to_owned());
    let child: ae_sdd_runtime::SessionResult = serde_json::from_value(result(&harness.call(
        &mut root_connection,
        RpcMethod::SessionOpen,
        child_open,
    )))
    .expect("child session");

    let mut report = session_params(
        &workspace,
        &child,
        "series-agent",
        json!({
            "delegationId":delegation_id,
            "inputRevision":1,
            "inputFingerprint":"a".repeat(64),
            "summary":"bounded series result",
            "result": {
                "outcome":"succeeded",
                "findings":[],
                "deliverables":[],
                "requestedAction":null,
                "memorySnapshotDigest":"c".repeat(64)
            }
        }),
        1_000,
    );
    report.work_item_id = Some("WORK".to_owned());
    report.idempotency_key = Some("series-report".to_owned());
    let cleaned = result(&harness.call(&mut root_connection, RpcMethod::DelegationReport, report));
    assert_eq!(cleaned["status"], "memory-cleaned");

    let namespace = harness
        .persistence
        .load_record("delegation-memory/v1", &delegation_id)
        .expect("namespace read")
        .expect("namespace exists");
    assert_eq!(namespace["status"], "cleaned");
    assert_eq!(namespace["payloadPurged"], true);

    let mut collect = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"delegationId":delegation_id}),
        1_000,
    );
    collect.work_item_id = Some("WORK".to_owned());
    collect.idempotency_key = Some("delegation-collect".to_owned());
    let collected =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCollect, collect));
    assert_eq!(collected["status"], "completed");
    assert_eq!(collected["summary"], "bounded series result");
    assert_eq!(
        collected["artifactValidationReceipt"]["schemaVersion"],
        "delegation-artifact-validation/v1"
    );
    assert_eq!(
        collected["memoryCleanupReceipt"]["schemaVersion"],
        "delegation-memory-cleanup/v1"
    );
    assert!(collected.get("result").is_none());
}
