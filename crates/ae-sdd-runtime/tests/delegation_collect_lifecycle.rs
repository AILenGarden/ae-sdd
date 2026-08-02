mod support;

#[allow(dead_code, unused_imports)]
#[path = "../../../bins/ae-sdd-cli/src/legacy/mod.rs"]
mod legacy;

use std::sync::atomic::Ordering;

use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::{
    ConnectionState, PersistencePort, RuntimeConfig, SessionResult, WorkspaceResult,
};
use serde_json::{Value, json};

use support::{
    Harness, create_root_series_delegation, flow_decision_digest, open_root_session, params,
    register_workspace, result, session_params, stable_error,
};

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
    let delegation = create_root_series_delegation(
        &harness,
        &mut root_connection,
        &workspace,
        &root,
        "root-agent",
        "WORK",
        "requirement-analysis",
        &["RA"],
        "delegation-create",
    );
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
            "claimId":action["claimId"],
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
            "inputFingerprint":flow_decision_digest("delegation-create"),
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
    legacy::adapt_passthrough_request("review collect", RpcMethod::DelegationCollect, &mut collect)
        .expect("review collect alias adapts");
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

    let mut replay = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"delegationId":delegation_id}),
        1_000,
    );
    replay.work_item_id = Some("WORK".to_owned());
    replay.idempotency_key = Some("delegation-collect".to_owned());
    legacy::adapt_passthrough_request(
        "review-loop collect",
        RpcMethod::DelegationCollect,
        &mut replay,
    )
    .expect("review-loop collect alias adapts");
    let replayed =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCollect, replay));
    assert_eq!(replayed, collected);
}

struct SeriesLifecycle {
    root_connection: ConnectionState,
    workspace: WorkspaceResult,
    root: SessionResult,
    delegation_id: String,
    create_response: Value,
}

/// Drives one Root-to-Series delegation from create to memory-cleaned so a
/// test only has to collect. `create_extra` is merged into the create payload.
fn run_series_lifecycle(harness: &Harness) -> SeriesLifecycle {
    let mut host = harness.connection(ClientKind::HostAdapter);
    let mut host_register = params(
        json!({"adapterId":"host-a","capabilities":["create","attest"]}),
        1_000,
    );
    host_register.capability_token = Some(harness.host_credential());
    host_register.idempotency_key = Some("host-register".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostRegister, host_register));

    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(harness, &mut root_connection, "delegation");
    let root = open_root_session(
        harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );
    let create_response = create_root_series_delegation(
        harness,
        &mut root_connection,
        &workspace,
        &root,
        "root-agent",
        "WORK",
        "requirement-analysis",
        &["RA"],
        "delegation-create",
    );
    let delegation_id = create_response["delegationId"]
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
            "claimId":action["claimId"],
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
    let child: SessionResult = serde_json::from_value(result(&harness.call(
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
            "inputFingerprint":flow_decision_digest("delegation-create"),
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

    SeriesLifecycle {
        root_connection,
        workspace,
        root,
        delegation_id,
        create_response,
    }
}

fn collect(life: &mut SeriesLifecycle, key: &str) -> ae_sdd_protocol::RequestParams<Value> {
    let mut collect = session_params(
        &life.workspace,
        &life.root,
        "root-agent",
        json!({"delegationId":life.delegation_id}),
        1_000,
    );
    collect.work_item_id = Some("WORK".to_owned());
    collect.idempotency_key = Some(key.to_owned());
    collect
}

#[test]
fn series_boundary_collect_records_the_flow_event_once() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut life = run_series_lifecycle(&harness);

    let mut request = collect(&mut life, "delegation-collect");
    legacy::adapt_passthrough_request("review collect", RpcMethod::DelegationCollect, &mut request)
        .expect("review collect alias adapts");
    let collected = result(&harness.call(
        &mut life.root_connection,
        RpcMethod::DelegationCollect,
        request,
    ));
    assert_eq!(collected["status"], "completed");
    assert_eq!(
        collected["compactAdvice"],
        json!({"kind":"suggest-compact","reason":"series-boundary","advisory":true})
    );
    assert_eq!(
        harness
            .business
            .series_completed_calls
            .load(Ordering::Acquire),
        1,
        "the series boundary must enter the flow event stream exactly once"
    );

    let mut replay = collect(&mut life, "delegation-collect");
    legacy::adapt_passthrough_request("review collect", RpcMethod::DelegationCollect, &mut replay)
        .expect("review collect alias adapts");
    let replayed = result(&harness.call(
        &mut life.root_connection,
        RpcMethod::DelegationCollect,
        replay,
    ));
    assert_eq!(replayed, collected);
    assert_eq!(
        harness
            .business
            .series_completed_calls
            .load(Ordering::Acquire),
        1,
        "an idempotent collect replay must not record the boundary twice"
    );
}

#[test]
fn daemon_derived_briefing_survives_create_status_and_collect() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut life = run_series_lifecycle(&harness);

    assert_eq!(
        life.create_response["briefing"],
        json!("Execute the daemon-committed requirement-analysis Series and produce RA")
    );
    assert!(life.create_response.get("assetRefs").is_none());

    let mut status = session_params(
        &life.workspace,
        &life.root,
        "root-agent",
        json!({"delegationId":life.delegation_id}),
        1_000,
    );
    status.work_item_id = Some("WORK".to_owned());
    let status = result(&harness.call(
        &mut life.root_connection,
        RpcMethod::DelegationStatus,
        status,
    ));
    assert_eq!(
        status["briefing"],
        json!("Execute the daemon-committed requirement-analysis Series and produce RA")
    );
    assert!(status.get("assetRefs").is_none());

    let mut request = collect(&mut life, "delegation-collect");
    legacy::adapt_passthrough_request("review collect", RpcMethod::DelegationCollect, &mut request)
        .expect("review collect alias adapts");
    let collected = result(&harness.call(
        &mut life.root_connection,
        RpcMethod::DelegationCollect,
        request,
    ));
    assert_eq!(
        collected["briefing"],
        json!("Execute the daemon-committed requirement-analysis Series and produce RA")
    );
    assert!(collected["assetRefs"].is_null());
}

#[test]
fn oversized_briefing_is_rejected_with_a_structured_error() {
    let harness = Harness::new(RuntimeConfig::default());
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
            "flowDecisionDigest":"a".repeat(64),
            "briefing":"x".repeat(8_193)
        }),
        1_000,
    );
    create.work_item_id = Some("WORK".to_owned());
    create.idempotency_key = Some("delegation-create".to_owned());
    let response = harness.call(&mut root_connection, RpcMethod::DelegationCreate, create);

    assert_eq!(stable_error(&response), "OPERATION_SCHEMA_INVALID");
}
