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

    let validations_before = harness
        .business
        .artifact_validation_calls
        .load(Ordering::Acquire);
    let mut stale_report = session_params(
        &workspace,
        &child,
        "series-agent",
        json!({
            "delegationId":delegation_id,
            "inputRevision":2,
            "inputFingerprint":flow_decision_digest("delegation-create"),
            "summary":"stale child input",
            "result": {
                "outcome":"succeeded",
                "findings":[],
                "deliverables":[],
                "requestedAction":null,
                "memorySnapshotDigest":"a".repeat(64)
            }
        }),
        1_000,
    );
    stale_report.work_item_id = Some("WORK".to_owned());
    stale_report.idempotency_key = Some("stale-series-report".to_owned());
    assert_eq!(
        stable_error(&harness.call(
            &mut root_connection,
            RpcMethod::DelegationReport,
            stale_report,
        )),
        "CHILD_RESULT_INVALID"
    );
    assert_eq!(
        harness
            .business
            .artifact_validation_calls
            .load(Ordering::Acquire),
        validations_before,
        "stale child input must be rejected before artifact validation"
    );

    let mut invalid_report = session_params(
        &workspace,
        &child,
        "series-agent",
        json!({
            "delegationId":delegation_id,
            "inputRevision":1,
            "inputFingerprint":flow_decision_digest("delegation-create"),
            "summary":"invalid deliverable shape",
            "result": {
                "outcome":"succeeded",
                "findings":[],
                "deliverables":["RA"],
                "requestedAction":null,
                "memorySnapshotDigest":"b".repeat(64)
            }
        }),
        1_000,
    );
    invalid_report.work_item_id = Some("WORK".to_owned());
    invalid_report.idempotency_key = Some("invalid-series-report".to_owned());
    assert_eq!(
        stable_error(&harness.call(
            &mut root_connection,
            RpcMethod::DelegationReport,
            invalid_report,
        )),
        "CHILD_RESULT_INVALID"
    );
    assert_eq!(
        harness
            .runtime
            .delegation_supervisor()
            .status(&child.session_id, &delegation_id)
            .expect("invalid report must leave the delegation retryable")
            .status,
        "running"
    );

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
fn collect_projection_exposes_the_root_project_lease_prerequisite() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut life = run_series_lifecycle(&harness);
    let request = collect(&mut life, "collect-prerequisite");
    let projected = result(&harness.call(
        &mut life.root_connection,
        RpcMethod::DelegationCollect,
        request,
    ));

    assert_eq!(projected["requiresRootProjectLease"], true);
    assert_eq!(
        projected["rootProjectLeaseSubmit"],
        json!({
            "method":"operation.execute",
            "operation":"lease.acquire",
            "arguments":{
                "owner":{"purpose":"delegation-collect"},
                "ttlSeconds":300
            }
        })
    );
    assert_eq!(
        projected["collectSubmit"],
        json!({
            "method":"delegation.collect",
            "arguments":{"delegationId":life.delegation_id},
            "leaseBinding":{
                "leaseIdFrom":"rootProjectLeaseSubmit.result.data.leaseId",
                "fencingTokenFrom":"rootProjectLeaseSubmit.result.data.fencingToken"
            }
        })
    );
    let remediation = projected["rootProjectLeaseRemediation"]
        .as_str()
        .expect("collect projection carries lease remediation");
    assert_eq!(
        remediation,
        "call operation.execute with operation=lease.acquire as the Root session before delegation.collect"
    );
}

#[test]
fn collect_rejects_a_caller_work_item_mismatch_before_completion() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut life = run_series_lifecycle(&harness);
    let calls_before = harness
        .business
        .series_completed_calls
        .load(Ordering::Acquire);
    let mut request = collect(&mut life, "collect-work-item-mismatch");
    request.work_item_id = Some("OTHER-WORK".to_owned());

    let response = harness.call(
        &mut life.root_connection,
        RpcMethod::DelegationCollect,
        request,
    );

    assert_eq!(
        stable_error(&response),
        "DELEGATION_ATTESTATION_FAILED",
        "the caller Work Item is an equality assertion against durable authority: {response}"
    );
    assert_eq!(
        harness
            .runtime
            .delegation_supervisor()
            .status(&life.root.session_id, &life.delegation_id)
            .expect("delegation status after rejected collect")
            .status,
        "memory-cleaned",
        "a Work Item mismatch must be rejected before collect mutation"
    );
    assert_eq!(
        harness
            .business
            .series_completed_calls
            .load(Ordering::Acquire),
        calls_before,
        "a Work Item mismatch must not enter the flow mutation"
    );
}

#[test]
fn collect_validates_work_item_before_same_key_receipt_replay() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut life = run_series_lifecycle(&harness);
    let key = "collect-cross-work-item-replay";
    let first_request = collect(&mut life, key);
    let first = result(&harness.call(
        &mut life.root_connection,
        RpcMethod::DelegationCollect,
        first_request,
    ));

    let mut mismatch = collect(&mut life, key);
    mismatch.work_item_id = Some("OTHER-WORK".to_owned());
    let rejected = harness.call(
        &mut life.root_connection,
        RpcMethod::DelegationCollect,
        mismatch,
    );
    assert_eq!(
        stable_error(&rejected),
        "DELEGATION_ATTESTATION_FAILED",
        "durable Work Item authority must be checked before receipt replay: {rejected}"
    );

    let corrected = collect(&mut life, key);
    assert_eq!(
        result(&harness.call(
            &mut life.root_connection,
            RpcMethod::DelegationCollect,
            corrected,
        )),
        first,
        "a rejected cross-Work-Item assertion must not displace the valid receipt"
    );
}

#[test]
fn collect_fails_closed_without_frozen_legacy_work_item_authority() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut life = run_series_lifecycle(&harness);
    let mut record = harness
        .persistence
        .load_record("delegation/v1", &life.delegation_id)
        .expect("delegation record")
        .expect("delegation record exists");
    record
        .as_object_mut()
        .expect("delegation record is an object")
        .remove("workItemId");
    harness
        .persistence
        .store_record("delegation/v1", &life.delegation_id, &record)
        .expect("legacy delegation fixture persists");
    let calls_before = harness
        .business
        .series_completed_calls
        .load(Ordering::Acquire);

    let request = collect(&mut life, "collect-missing-legacy-authority");
    let response = harness.call(
        &mut life.root_connection,
        RpcMethod::DelegationCollect,
        request,
    );

    assert_eq!(
        stable_error(&response),
        "DELEGATION_ATTESTATION_FAILED",
        "caller input must not repair missing durable Work Item authority: {response}"
    );
    assert_eq!(
        harness
            .runtime
            .delegation_supervisor()
            .status(&life.root.session_id, &life.delegation_id)
            .expect("delegation status after rejected legacy collect")
            .status,
        "memory-cleaned"
    );
    assert_eq!(
        harness
            .business
            .series_completed_calls
            .load(Ordering::Acquire),
        calls_before
    );
}

#[test]
fn nested_series_collect_does_not_require_the_root_project_lease() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut life = run_series_lifecycle(&harness);
    let record = harness
        .persistence
        .load_record("delegation/v1", &life.delegation_id)
        .expect("delegation record")
        .expect("delegation record exists");
    let mut nested = record;
    nested["parentDelegationId"] = json!("parent-series-delegation");
    harness
        .persistence
        .store_record("delegation/v1", &life.delegation_id, &nested)
        .expect("nested delegation fixture persists");

    let request = collect(&mut life, "nested-collect");
    let projected = result(&harness.call(
        &mut life.root_connection,
        RpcMethod::DelegationCollect,
        request,
    ));

    assert_eq!(projected["requiresRootProjectLease"], false);
    assert!(
        projected["rootProjectLeaseSubmit"].is_null(),
        "a nested Series collect must not advertise a Root project lease prerequisite"
    );
    assert!(
        projected["collectSubmit"]["leaseBinding"].is_null(),
        "a non-boundary collect must not advertise an inert lease binding"
    );
}

#[test]
fn delegation_status_projects_collect_prerequisites_before_mutation() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut life = run_series_lifecycle(&harness);
    let mut status = session_params(
        &life.workspace,
        &life.root,
        "root-agent",
        json!({"delegationId":life.delegation_id}),
        1_000,
    );
    status.work_item_id = Some("WORK".to_owned());
    status.idempotency_key = Some("collect-status-prerequisite".to_owned());

    let projected = result(&harness.call(
        &mut life.root_connection,
        RpcMethod::DelegationStatus,
        status,
    ));
    assert_eq!(projected["status"], "memory-cleaned");
    let prerequisite = projected["collectPrerequisite"]
        .as_object()
        .expect("status exposes collect prerequisite before mutation");
    assert_eq!(prerequisite["requiresRootProjectLease"], true);
    assert_eq!(
        prerequisite["rootProjectLeaseSubmit"]["method"],
        "operation.execute"
    );
    assert_eq!(
        prerequisite["rootProjectLeaseSubmit"]["payload"],
        json!({
            "operation":"lease.acquire",
            "payload":{
                "owner":{"purpose":"delegation-collect"},
                "ttlSeconds":300
            }
        })
    );
    assert_eq!(
        prerequisite["rootProjectLeaseSubmit"]["requestContext"]["workspaceId"],
        life.workspace.workspace_id
    );
    assert_eq!(
        prerequisite["rootProjectLeaseSubmit"]["requestContext"]["workItemId"],
        "WORK"
    );
    assert_eq!(
        prerequisite["collectSubmit"]["payload"],
        json!({"delegationId":life.delegation_id})
    );
    assert_eq!(
        prerequisite["collectSubmit"]["requestContext"]["leaseIdFrom"],
        "rootProjectLeaseSubmit.result.data.leaseId"
    );
    assert_eq!(
        prerequisite["collectSubmit"]["requestContext"]["fencingTokenFrom"],
        "rootProjectLeaseSubmit.result.data.fencingToken"
    );
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
fn completed_collect_retry_repairs_a_failed_binding_release() {
    let harness = Harness::new(RuntimeConfig::default());
    let life = run_series_lifecycle(&harness);
    harness
        .persistence
        .fail_store_record_after("host-execution-binding/v1", 1);

    let first = harness
        .runtime
        .delegation_supervisor()
        .collect(&life.root.session_id, &life.delegation_id);
    assert_eq!(
        first.expect_err("binding release fails").code(),
        ae_sdd_protocol::StableErrorCode::ExternalStateConflict
    );
    harness
        .runtime
        .delegation_supervisor()
        .collect(&life.root.session_id, &life.delegation_id)
        .expect("completed collect retry repairs binding");

    let binding = harness
        .persistence
        .list_records("host-execution-binding/v1")
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .1;
    assert_eq!(binding["status"], "released");
    assert_eq!(binding["releasedReason"], "collected");
}

#[test]
fn recovery_reconciles_a_completed_delegation_with_a_live_binding() {
    let persistence = std::sync::Arc::new(ae_sdd_runtime::MemoryPersistence::new(
        ae_sdd_domain::EventStoreId::from_uuid(uuid::Uuid::from_u128(1_240)),
    ));
    let first = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        1_241,
        "completed-binding-recovery-first".to_owned(),
    );
    let life = run_series_lifecycle(&first);
    first
        .runtime
        .delegation_supervisor()
        .collect(&life.root.session_id, &life.delegation_id)
        .expect("collect completes");
    let (binding_key, mut binding) = persistence
        .list_records("host-execution-binding/v1")
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    binding["status"] = json!("active");
    binding["releasedAtUnixMs"] = Value::Null;
    binding["releasedReason"] = Value::Null;
    persistence
        .store_record("host-execution-binding/v1", &binding_key, &binding)
        .unwrap();

    let recovered = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        1_242,
        "completed-binding-recovery-second".to_owned(),
    );
    recovered.runtime.recover().expect("runtime recovery");
    let binding = persistence
        .load_record("host-execution-binding/v1", &binding_key)
        .unwrap()
        .unwrap();
    assert_eq!(binding["status"], "released");
    assert_eq!(binding["releasedReason"], "collected");
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
