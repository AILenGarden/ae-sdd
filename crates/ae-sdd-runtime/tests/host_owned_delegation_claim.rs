//! Physical child claims are daemon-issued and delivered only on the Host lane.

mod support;

use std::sync::Arc;

use ae_sdd_domain::EventStoreId;
use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::{MemoryPersistence, PersistencePort, RuntimeConfig, RuntimeIdentityKind};
use serde_json::{Value, json};
use uuid::Uuid;

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

    let accept_params = |claim_id: &str, key: &str, work_item_id: Option<&str>| {
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
        accept.work_item_id = work_item_id.map(str::to_owned);
        accept.idempotency_key = Some(key.to_owned());
        accept
    };

    let forged = harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        accept_params(
            "00000000-0000-0000-0000-000000001103",
            "host-owned-claim-forged",
            Some("WORK"),
        ),
    );
    assert_eq!(
        stable_error(&forged),
        "DELEGATION_ATTESTATION_FAILED",
        "a caller-minted UUID must not be accepted: {forged}"
    );

    let mismatched_work_item = harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        accept_params(
            &claim_id,
            "host-owned-claim-work-item-mismatch",
            Some("OTHER"),
        ),
    );
    assert_eq!(
        stable_error(&mismatched_work_item),
        "DELEGATION_ATTESTATION_FAILED",
        "an explicit Work Item remains an equality assertion: {mismatched_work_item}"
    );

    let mut legacy_record = harness
        .persistence
        .load_record("delegation/v1", &delegation_id)
        .expect("delegation record")
        .expect("delegation record exists");
    legacy_record
        .as_object_mut()
        .expect("delegation record is an object")
        .remove("workItemId");
    harness
        .persistence
        .store_record("delegation/v1", &delegation_id, &legacy_record)
        .expect("legacy delegation fixture persists");
    let missing_authority = harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        accept_params(
            &claim_id,
            "host-owned-claim-missing-work-item-authority",
            Some("WORK"),
        ),
    );
    assert_eq!(
        stable_error(&missing_authority),
        "DELEGATION_ATTESTATION_FAILED",
        "caller input must not repair missing durable Work Item authority: {missing_authority}"
    );
    legacy_record["workItemId"] = json!("WORK");
    harness
        .persistence
        .store_record("delegation/v1", &delegation_id, &legacy_record)
        .expect("frozen Work Item authority is restored");

    let accepted = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        accept_params(&claim_id, "host-owned-claim-accept", None),
    ));
    assert_eq!(accepted["status"], "running");
    let child = harness
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Delegation)
        .expect("durable session snapshots")
        .into_iter()
        .filter_map(|snapshot| snapshot.session)
        .find(|session| session.session_id == CHILD)
        .expect("accepted child session is durable");
    assert_eq!(
        child.current_work_item.as_deref(),
        Some("WORK"),
        "the child Work Item must derive from delegation.create authority when \
         delegation.accept omits workItemId"
    );
}

#[test]
fn a_pending_create_action_receives_a_fresh_claim_after_daemon_restart() {
    let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
        Uuid::from_u128(1_201),
    )));
    let first = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        1_202,
        "first-claim-recovery-token".to_owned(),
    );
    let mut first_host = first.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = first.connection(ClientKind::Hook);
    let workspace = register_workspace(&first, &mut root_connection, "claim-recovery");
    let root = open_root_session(
        &first,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );

    first.business.set_flow_next_result(json!({
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
    let _ = result(&first.call(&mut root_connection, RpcMethod::FlowNext, next));

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some("WORK".to_owned());
    create.idempotency_key = Some("claim-recovery-create".to_owned());
    let delegation = result(&first.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    let first_delivery = result(&first.call(
        &mut first_host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let first_claim = first_delivery["claimId"]
        .as_str()
        .expect("first boot delivers a claim")
        .to_owned();

    let second = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence,
        1_203,
        "second-claim-recovery-token".to_owned(),
    );
    second.runtime.recover().expect("runtime recovers");
    let mut second_host = second.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let recovered_delivery = result(&second.call(
        &mut second_host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let recovered_claim = recovered_delivery["claimId"]
        .as_str()
        .expect("recovered pending create receives a fresh claim");
    assert_ne!(
        recovered_claim, first_claim,
        "boot rotation invalidates the old claim"
    );
    assert_eq!(recovered_delivery["actionId"], first_delivery["actionId"]);

    let child_session = "00000000-0000-0000-0000-000000001204";
    let mut ack = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000001205",
                "actionId":recovered_delivery["actionId"],
                "commandSeq":recovered_delivery["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-claim-recovery",
                "sessionId":child_session
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("claim-recovery-ack".to_owned());
    let _ = result(&second.call(&mut second_host, RpcMethod::HostActionAck, ack));

    let mut accept = params(
        json!({
            "delegationId":delegation["delegationId"],
            "claimId":recovered_claim,
            "actionId":recovered_delivery["actionId"],
            "childSessionId":child_session,
            "expiresAtUnixMs":4_900
        }),
        1_000,
    );
    accept.workspace_id = Some(workspace.workspace_id);
    accept.work_item_id = Some("WORK".to_owned());
    accept.idempotency_key = Some("claim-recovery-accept".to_owned());
    let accepted = result(&second.call(
        &mut second.connection(ClientKind::Hook),
        RpcMethod::DelegationAccept,
        accept,
    ));
    assert_eq!(accepted["status"], "running");
}

#[test]
fn recovery_reconciles_a_rejected_ack_left_before_delegation_cancellation() {
    let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
        Uuid::from_u128(1_210),
    )));
    let first = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        1_211,
        "first-rejected-recovery-token".to_owned(),
    );
    let mut host = first.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = first.connection(ClientKind::Hook);
    let workspace = register_workspace(&first, &mut root_connection, "rejected-recovery");
    let root = open_root_session(
        &first,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );
    first.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1","decisionDigest":DECISION,"stateRevision":1,"phase":"initialized",
        "nextAction":{"kind":"delegate-series","seriesKind":"requirement-analysis","requiredArtifacts":["RA"]}
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some("WORK".to_owned());
    let _ = result(&first.call(&mut root_connection, RpcMethod::FlowNext, next));
    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some("WORK".to_owned());
    create.idempotency_key = Some("rejected-recovery-create".to_owned());
    let delegation = result(&first.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    let action = result(&first.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let ack_id = "00000000-0000-0000-0000-000000001212";
    persistence.store_record("host-ack/v1", ack_id, &json!({
        "ackId":ack_id,"actionId":action["actionId"],"commandSeq":action["commandSeq"],"outcome":"rejected","hostTaskId":null,"sessionId":null
    })).expect("crash-point rejected ACK stores");

    let second = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence,
        1_213,
        "second-rejected-recovery-token".to_owned(),
    );
    second
        .runtime
        .recover()
        .expect("runtime recovery reconciles rejected ACK");
    let status = second
        .runtime
        .delegation_supervisor()
        .status(
            &root.session_id,
            delegation["delegationId"].as_str().expect("delegation id"),
        )
        .expect("delegation status");
    assert_eq!(status.status, "cancelled");
    let mut recovered_host = second.connection(ClientKind::HostAdapter);
    let mut register = params(json!({"adapterId":ADAPTER}), 1_000);
    register.capability_token = Some(second.host_credential());
    register.idempotency_key = Some("rejected-retry-register-second-boot".to_owned());
    let _ = result(&second.call(&mut recovered_host, RpcMethod::HostRegister, register));
    let next_action = result(&second.call(
        &mut recovered_host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    assert_eq!(next_action, Value::Null);
}

#[test]
fn rejected_create_retry_preserves_the_recovered_terminal_state() {
    let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
        Uuid::from_u128(1_214),
    )));
    let first = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        1_215,
        "first-rejected-retry-token".to_owned(),
    );
    let mut host = first.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = first.connection(ClientKind::Hook);
    let workspace = register_workspace(&first, &mut root_connection, "rejected-retry");
    let root = open_root_session(
        &first,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );
    first.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1","decisionDigest":DECISION,"stateRevision":1,"phase":"initialized",
        "nextAction":{"kind":"delegate-series","seriesKind":"requirement-analysis","requiredArtifacts":["RA"]}
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some("WORK".to_owned());
    let _ = result(&first.call(&mut root_connection, RpcMethod::FlowNext, next));

    persistence.fail_commit_event_and_receipt_after(1);
    let create_request = || {
        let mut create = session_params(
            &workspace,
            &root,
            "root-agent",
            json!({"flowDecisionDigest":DECISION}),
            1_000,
        );
        create.work_item_id = Some("WORK".to_owned());
        create.idempotency_key = Some("rejected-retry-create".to_owned());
        create
    };
    let failed = first.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_request(),
    );
    assert_eq!(stable_error(&failed), "EXTERNAL_STATE_CONFLICT");
    let action = result(&first.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let delegation_id = action["delegationId"]
        .as_str()
        .expect("create action names its delegation")
        .to_owned();
    let ack_id = "00000000-0000-0000-0000-000000001216";
    persistence
        .store_record(
            "host-ack/v1",
            ack_id,
            &json!({
                "ackId":ack_id,"actionId":action["actionId"],"commandSeq":action["commandSeq"],
                "outcome":"rejected","hostTaskId":null,"sessionId":null
            }),
        )
        .expect("crash-point rejected ACK stores");

    let second = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence,
        1_217,
        "second-rejected-retry-token".to_owned(),
    );
    second
        .runtime
        .recover()
        .expect("runtime recovers rejection");
    let mut recovered_host = second.connection(ClientKind::HostAdapter);
    let mut register = params(json!({"adapterId":ADAPTER}), 1_000);
    register.capability_token = Some(second.host_credential());
    register.idempotency_key = Some("rejected-retry-register-current-boot".to_owned());
    let _ = result(&second.call(&mut recovered_host, RpcMethod::HostRegister, register));
    let mut recovered_root_connection = second.connection(ClientKind::Hook);
    let recovered_root = open_root_session(
        &second,
        &mut recovered_root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );
    let mut retry = session_params(
        &workspace,
        &recovered_root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    retry.work_item_id = Some("WORK".to_owned());
    retry.idempotency_key = Some("rejected-retry-create".to_owned());
    let replayed = result(&second.call(
        &mut recovered_root_connection,
        RpcMethod::DelegationCreate,
        retry,
    ));
    assert_eq!(replayed["delegationId"], delegation_id);
    assert_eq!(replayed["status"], "cancelled");
    let status = second
        .runtime
        .delegation_supervisor()
        .status(&recovered_root.session_id, &delegation_id)
        .expect("delegation status");
    assert_eq!(status.status, "cancelled");
    assert_eq!(
        result(&second.call(
            &mut recovered_host,
            RpcMethod::HostActionNext,
            params(json!({"adapterId":ADAPTER}), 1_000),
        )),
        Value::Null
    );
}

#[test]
fn recovery_releases_a_spawning_binding_for_an_already_cancelled_rejected_delegation() {
    let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
        Uuid::from_u128(1_218),
    )));
    let first = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        1_219,
        "first-binding-recovery-token".to_owned(),
    );
    let mut host = first.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = first.connection(ClientKind::Hook);
    let workspace = register_workspace(&first, &mut root_connection, "binding-recovery");
    let root = open_root_session(
        &first,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );
    first.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1","decisionDigest":DECISION,"stateRevision":1,"phase":"initialized",
        "nextAction":{"kind":"delegate-series","seriesKind":"requirement-analysis","requiredArtifacts":["RA"]}
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some("WORK".to_owned());
    let _ = result(&first.call(&mut root_connection, RpcMethod::FlowNext, next));
    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some("WORK".to_owned());
    create.idempotency_key = Some("binding-recovery-create".to_owned());
    let delegation = result(&first.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    let action = result(&first.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let ack_id = "00000000-0000-0000-0000-000000001220";
    persistence
        .store_record(
            "host-ack/v1",
            ack_id,
            &json!({
                "ackId":ack_id,"actionId":action["actionId"],"commandSeq":action["commandSeq"],
                "outcome":"rejected","hostTaskId":null,"sessionId":null
            }),
        )
        .expect("rejected ACK stores");
    let delegation_id = delegation["delegationId"].as_str().expect("delegation id");
    first
        .runtime
        .delegation_supervisor()
        .host_rejected(delegation_id)
        .expect("delegation cancellation commits");

    let (binding_key, mut binding) = persistence
        .list_records("host-execution-binding/v1")
        .expect("binding records")
        .into_iter()
        .next()
        .expect("binding exists");
    binding["status"] = json!("spawning");
    binding["releasedAtUnixMs"] = Value::Null;
    binding["releasedReason"] = Value::Null;
    persistence
        .store_record("host-execution-binding/v1", &binding_key, &binding)
        .expect("model crash before binding release");

    let recovered = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        1_221,
        "second-binding-recovery-token".to_owned(),
    );
    recovered.runtime.recover().expect("runtime recovers");
    let binding = persistence
        .load_record("host-execution-binding/v1", &binding_key)
        .expect("binding read")
        .expect("binding remains durable");
    assert_eq!(binding["status"], "released");
    assert_eq!(binding["releasedReason"], "cancelled");
}

#[test]
fn recovery_repairs_a_stale_series_projection_for_a_cancelled_rejected_delegation() {
    let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
        Uuid::from_u128(1_222),
    )));
    let first = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        1_223,
        "first-series-recovery-token".to_owned(),
    );
    let mut host = first.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = first.connection(ClientKind::Hook);
    let workspace = register_workspace(&first, &mut root_connection, "series-recovery");
    let root = open_root_session(
        &first,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );
    first.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1","decisionDigest":DECISION,"stateRevision":1,"phase":"initialized",
        "nextAction":{"kind":"delegate-series","seriesKind":"requirement-analysis","requiredArtifacts":["RA"]}
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some("WORK".to_owned());
    let _ = result(&first.call(&mut root_connection, RpcMethod::FlowNext, next));
    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some("WORK".to_owned());
    create.idempotency_key = Some("series-recovery-create".to_owned());
    let delegation = result(&first.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    let action = result(&first.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let ack_id = "00000000-0000-0000-0000-000000001224";
    persistence
        .store_record(
            "host-ack/v1",
            ack_id,
            &json!({
                "ackId":ack_id,"actionId":action["actionId"],"commandSeq":action["commandSeq"],
                "outcome":"rejected","hostTaskId":null,"sessionId":null
            }),
        )
        .expect("rejected ACK stores");
    let delegation_id = delegation["delegationId"].as_str().expect("delegation id");
    first
        .runtime
        .delegation_supervisor()
        .host_rejected(delegation_id)
        .expect("delegation cancellation commits");
    let (series_key, mut series) = persistence
        .list_records("series_run/v1")
        .expect("series projections")
        .into_iter()
        .next()
        .expect("series projection exists");
    series["lifecycleState"] = json!("spawn_requested");
    persistence
        .store_record("series_run/v1", &series_key, &series)
        .expect("model crash before Series projection write");

    let recovered = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        1_225,
        "second-series-recovery-token".to_owned(),
    );
    recovered.runtime.recover().expect("runtime recovers");
    let series = persistence
        .load_record("series_run/v1", &series_key)
        .expect("series read")
        .expect("series projection remains durable");
    assert_eq!(series["lifecycleState"], "cancelled");
}

#[test]
fn rejected_retry_repairs_series_projection_without_a_restart() {
    let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
        Uuid::from_u128(1_230),
    )));
    let harness = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        1_231,
        "same-process-series-retry-token".to_owned(),
    );
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "same-process-series-retry");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1","decisionDigest":DECISION,"stateRevision":1,"phase":"initialized",
        "nextAction":{"kind":"delegate-series","seriesKind":"requirement-analysis","requiredArtifacts":["RA"]}
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
    create.idempotency_key = Some("same-process-series-retry-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    let _ = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let delegation_id = delegation["delegationId"].as_str().unwrap();

    persistence.fail_store_record_after("series_run/v1", 1);
    let first = harness
        .runtime
        .delegation_supervisor()
        .host_rejected(delegation_id);
    assert_eq!(
        first.expect_err("Series projection write fails").code(),
        ae_sdd_protocol::StableErrorCode::ExternalStateConflict
    );
    harness
        .runtime
        .delegation_supervisor()
        .host_rejected(delegation_id)
        .expect("same-process retry repairs Series projection");

    let series = persistence
        .list_records("series_run/v1")
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .1;
    assert_eq!(series["lifecycleState"], "cancelled");
}

#[test]
fn delayed_rejected_ack_recovery_persists_a_cancelled_binding_reason() {
    let persistence = Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(
        Uuid::from_u128(1_226),
    )));
    let first = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        1_227,
        "first-delayed-recovery-token".to_owned(),
    );
    let mut host = first.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = first.connection(ClientKind::Hook);
    let workspace = register_workspace(&first, &mut root_connection, "delayed-recovery");
    let root = open_root_session(
        &first,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );
    first.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1","decisionDigest":DECISION,"stateRevision":1,"phase":"initialized",
        "nextAction":{"kind":"delegate-series","seriesKind":"requirement-analysis","requiredArtifacts":["RA"]}
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some("WORK".to_owned());
    let _ = result(&first.call(&mut root_connection, RpcMethod::FlowNext, next));
    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some("WORK".to_owned());
    create.idempotency_key = Some("delayed-recovery-create".to_owned());
    let _ = result(&first.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    let action = result(&first.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let ack_id = "00000000-0000-0000-0000-000000001228";
    persistence
        .store_record(
            "host-ack/v1",
            ack_id,
            &json!({
                "ackId":ack_id,"actionId":action["actionId"],"commandSeq":action["commandSeq"],
                "outcome":"rejected","hostTaskId":null,"sessionId":null
            }),
        )
        .expect("rejected ACK stores");
    let (binding_key, _) = persistence
        .list_records("host-execution-binding/v1")
        .expect("binding records")
        .into_iter()
        .next()
        .expect("binding exists");

    let recovered = Harness::with_persistence(
        RuntimeConfig::default(),
        persistence.clone(),
        1_229,
        "second-delayed-recovery-token".to_owned(),
    );
    recovered.clock.set(1_000 + 12 * 60 * 60 * 1_000 + 1);
    recovered.runtime.recover().expect("runtime recovers");
    let binding = persistence
        .load_record("host-execution-binding/v1", &binding_key)
        .expect("binding read")
        .expect("binding remains durable");
    assert_eq!(binding["status"], "released");
    assert_eq!(binding["releasedReason"], "cancelled");
}

/// A2 host-native delegation has no field in the host's `SubagentStart`
/// payload that could disambiguate which of two concurrently pending Create
/// actions a given child belongs to (`hook_event_name`/`agent_id`/`agent_type`
/// only). Rather than guess via queue order, a second concurrent create from
/// the same root session must be rejected outright, which keeps the existing
/// FIFO `host.action_next` delivery exact instead of a heuristic.
#[test]
fn a_second_concurrent_delegation_create_from_the_same_root_session_is_allowed() {
    // ROUTE-C (Plan §2.4): the "at most one spawning delegation per root
    // session" guard is gone. Child Self-Claim makes concurrency safe because
    // each delegation carries its own daemon-minted claim_id, so FIFO queue
    // disambiguation is no longer the concurrency model. The same root session
    // may now open several spawning delegations at once; liveness and
    // preemption are owned by ROUTE-A's binding ledger, not by this gate.
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "concurrent-claim");
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

    let create_params = |idempotency_key: &str| {
        let mut create = session_params(
            &workspace,
            &root,
            "root-agent",
            json!({"flowDecisionDigest":DECISION}),
            1_000,
        );
        create.work_item_id = Some("WORK".to_owned());
        create.idempotency_key = Some(idempotency_key.to_owned());
        create
    };

    let first = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_params("concurrent-claim-create-1"),
    ));
    assert_eq!(first["status"], "spawning");
    let first_delegation_id = first["delegationId"].as_str().unwrap().to_owned();

    // A distinct idempotency key makes this a genuinely new create attempt,
    // not an idempotent replay of the first. It must now succeed: the
    // concurrency ceiling is lifted.
    let second = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_params("concurrent-claim-create-2"),
    ));
    assert_eq!(
        second["status"], "spawning",
        "a root session may hold multiple spawning delegations: {second}"
    );
    let second_delegation_id = second["delegationId"].as_str().unwrap().to_owned();
    assert_ne!(
        first_delegation_id, second_delegation_id,
        "the two concurrent creates must be distinct delegations"
    );

    // The first delegation is then accepted into `running`; the second stays
    // spawning. This proves sequence and concurrency coexist under the new
    // model — exactly the regression §2.4 / ROUTE-A revision 1 calls for.
    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let claim_id = action["claimId"]
        .as_str()
        .expect("Host delivery carries the daemon-issued claim")
        .to_owned();
    let mut ack = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000001104",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-concurrent-claim",
                "sessionId":CHILD
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("concurrent-claim-ack".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));

    let mut accept = params(
        json!({
            "delegationId":first_delegation_id,
            "claimId":claim_id,
            "actionId":action["actionId"],
            "childSessionId":CHILD,
            "expiresAtUnixMs":4_900
        }),
        1_000,
    );
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.work_item_id = Some("WORK".to_owned());
    accept.idempotency_key = Some("concurrent-claim-accept".to_owned());
    let accepted = result(&harness.call(&mut root_connection, RpcMethod::DelegationAccept, accept));
    assert_eq!(accepted["status"], "running");

    // A third create must also succeed now that the guard is gone — regardless
    // of whether an earlier delegation is still active. This is the
    // multi-active-binding regression pinned by ROUTE-A's ledger design.
    let third = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_params("concurrent-claim-create-3"),
    ));
    assert_eq!(third["status"], "spawning");
}

/// ROUTE-702d576a Task 2 Admission RED: duplicate-event case. A claim that
/// already produced a `running` delegation must replay the same receipt when
/// the exact same accept request arrives again (the host's `SubagentStart`
/// hook retrying after a lost response), never mint a second physical child
/// or otherwise mutate state a second time.
#[test]
fn a_duplicate_accept_of_the_same_claim_replays_the_original_receipt() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "duplicate-accept");
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
    create.idempotency_key = Some("duplicate-accept-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    let delegation_id = delegation["delegationId"]
        .as_str()
        .expect("delegation id")
        .to_owned();

    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let claim_id = action["claimId"]
        .as_str()
        .expect("Host delivery carries the daemon-issued claim")
        .to_owned();
    let mut ack = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000001105",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-duplicate-accept",
                "sessionId":CHILD
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("duplicate-accept-ack".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));

    let accept_once = || {
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
        // Same idempotency key both times: this is the exact same request
        // arriving twice, not a second distinct accept attempt.
        accept.idempotency_key = Some("duplicate-accept-accept".to_owned());
        accept
    };

    let first = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        accept_once(),
    ));
    assert_eq!(first["status"], "running");

    let replay = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        accept_once(),
    ));
    assert_eq!(
        replay, first,
        "a replayed accept of the same claim must return the identical receipt, \
         never mint a second child or advance any further state"
    );
}

/// ROUTE-702d576a Task 2 Admission RED: wrong-parent case, corrected. The
/// first attempt at this test asserted that `DelegationAccept` must reject a
/// caller-supplied `sessionId`/`capabilityToken` that names a different root
/// session -- that assertion was wrong and has been withdrawn (see below);
/// this replacement asserts the actual security boundary that protects
/// against parent impersonation.
///
/// `DelegationAcceptPayload` (`model.rs`) carries no session/agent identity
/// field at all, and `delegation_accept` (`service_host.rs`) never calls
/// `session_identity()` -- unlike `delegation.collect`, which does check
/// `record.parent_session_id` against the caller. Accept's authority is the
/// `claimId` alone: `delegation_claim_digest` is computed from the *daemon's
/// own stored* `record.parent_session_id`, deadline, role, and other fields
/// baked in at `create` time, never from anything the accept caller supplies.
/// A caller cannot "impersonate the parent" by changing `sessionId` because
/// accept never reads that field; the actual protection is that the raw
/// `claimId` is delivered only through the authenticated `host.action_next`
/// lane and is never observable to an ordinary session (constraints/security.md
/// §四: claims never enter argv/env/logs/transcript).
///
/// So the real wrong-parent boundary to prove is: a claim minted for
/// delegation A must never authorize accepting delegation B, even when B's
/// own `delegationId`/`actionId`/`childSessionId` are supplied verbatim
/// alongside A's `claimId`. This is the cross-delegation confusion case,
/// already covered at the pure-function layer by
/// `a_claim_for_a_different_delegation_is_rejected` in
/// `ae-sdd-host/tests/host_ack_claim.rs`; this test proves the same
/// invariant holds through the full daemon-integration path.
#[test]
fn a_claim_minted_for_one_delegation_cannot_accept_a_different_one() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "wrong-parent");
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

    let create_params = |idempotency_key: &str| {
        let mut create = session_params(
            &workspace,
            &root,
            "root-agent",
            json!({"flowDecisionDigest":DECISION}),
            1_000,
        );
        create.work_item_id = Some("WORK".to_owned());
        create.idempotency_key = Some(idempotency_key.to_owned());
        create
    };

    // Delegation A: create, deliver, ACK -- but never accepted. Its claim is
    // the one the impersonation attempt below tries to reuse.
    let first = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_params("wrong-parent-create-a"),
    ));
    let delegation_a_id = first["delegationId"]
        .as_str()
        .expect("delegation A id")
        .to_owned();
    let action_a = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let claim_a_id = action_a["claimId"]
        .as_str()
        .expect("Host delivery carries the daemon-issued claim")
        .to_owned();
    let mut ack_a = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000001107",
                "actionId":action_a["actionId"],
                "commandSeq":action_a["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-wrong-parent-a",
                "sessionId":CHILD
            }
        }),
        1_000,
    );
    ack_a.idempotency_key = Some("wrong-parent-ack-a".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack_a));

    // Complete delegation A so the concurrent-pending invariant does not
    // block delegation B's create below; A's claim remains valid to attempt
    // reuse against B regardless of A's own final state.
    let mut accept_a = params(
        json!({
            "delegationId":delegation_a_id,
            "claimId":claim_a_id,
            "actionId":action_a["actionId"],
            "childSessionId":CHILD,
            "expiresAtUnixMs":4_900
        }),
        1_000,
    );
    accept_a.workspace_id = Some(workspace.workspace_id.clone());
    accept_a.work_item_id = Some("WORK".to_owned());
    accept_a.idempotency_key = Some("wrong-parent-accept-a".to_owned());
    let accepted_a =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationAccept, accept_a));
    assert_eq!(accepted_a["status"], "running");

    // Delegation B: a second, independent create/deliver/ACK cycle with its
    // own real claim.
    let second = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_params("wrong-parent-create-b"),
    ));
    let delegation_b_id = second["delegationId"]
        .as_str()
        .expect("delegation B id")
        .to_owned();
    let action_b = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let mut ack_b = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000001108",
                "actionId":action_b["actionId"],
                "commandSeq":action_b["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-wrong-parent-b",
                "sessionId":CHILD
            }
        }),
        1_000,
    );
    ack_b.idempotency_key = Some("wrong-parent-ack-b".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack_b));

    // The impersonation attempt: accept delegation B using delegation A's
    // claim, with B's own actionId/childSessionId supplied verbatim.
    let mut confused_accept = params(
        json!({
            "delegationId":delegation_b_id,
            "claimId":claim_a_id,
            "actionId":action_b["actionId"],
            "childSessionId":CHILD,
            "expiresAtUnixMs":4_900
        }),
        1_000,
    );
    confused_accept.workspace_id = Some(workspace.workspace_id.clone());
    confused_accept.work_item_id = Some("WORK".to_owned());
    confused_accept.idempotency_key = Some("wrong-parent-accept-confused".to_owned());

    let rejected = harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        confused_accept,
    );
    assert_eq!(
        stable_error(&rejected),
        "DELEGATION_ATTESTATION_FAILED",
        "delegation A's claim must never authorize accepting delegation B: {rejected}"
    );
}

/// ROUTE-702d576a Task 2 Admission RED: replay-with-different-payload case.
/// The same idempotency key reused with a materially different payload
/// (a different `childSessionId`, here) must be rejected outright, never
/// silently accepted as if it were the original request nor treated as a
/// safe replay.
#[test]
fn reusing_an_accept_idempotency_key_with_a_different_payload_is_rejected() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "replay-payload");
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
    create.idempotency_key = Some("replay-payload-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    let delegation_id = delegation["delegationId"]
        .as_str()
        .expect("delegation id")
        .to_owned();

    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let claim_id = action["claimId"]
        .as_str()
        .expect("Host delivery carries the daemon-issued claim")
        .to_owned();
    let mut ack = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000001109",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-replay-payload",
                "sessionId":CHILD
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("replay-payload-ack".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));

    const SHARED_KEY: &str = "replay-payload-accept";
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
    accept.idempotency_key = Some(SHARED_KEY.to_owned());
    let accepted = result(&harness.call(&mut root_connection, RpcMethod::DelegationAccept, accept));
    assert_eq!(accepted["status"], "running");

    // Same key, but a materially different payload (a different, otherwise
    // well-formed, expiresAtUnixMs): this must be rejected as a reused key,
    // not silently accepted and not treated as an idempotent replay of the
    // original request.
    let mut different_payload = params(
        json!({
            "delegationId":delegation_id,
            "claimId":claim_id,
            "actionId":action["actionId"],
            "childSessionId":CHILD,
            "expiresAtUnixMs":4_901
        }),
        1_000,
    );
    different_payload.workspace_id = Some(workspace.workspace_id.clone());
    different_payload.work_item_id = Some("WORK".to_owned());
    different_payload.idempotency_key = Some(SHARED_KEY.to_owned());

    let rejected = harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        different_payload,
    );
    assert_eq!(
        stable_error(&rejected),
        "IDEMPOTENCY_KEY_REUSED",
        "the same idempotency key with a different canonical payload must \
         never be treated as the original request or a safe replay: {rejected}"
    );
}
