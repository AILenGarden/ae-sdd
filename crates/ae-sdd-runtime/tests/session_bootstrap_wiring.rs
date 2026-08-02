//! AC-C08 coverage for the runtime-to-session-bootstrap production wiring.

mod support;

use ae_sdd_protocol::{ClientKind, RequestParams, RpcMethod, WorkspaceMode};
use ae_sdd_runtime::{
    PersistencePort, RuntimeConfig, RuntimeIdentityKind, RuntimeIdentityTransition, SessionResult,
    WorkspaceResult,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use support::{
    Harness, create_root_series_delegation, params, register_workspace, session_params,
    stable_error,
};

fn session_open_params(
    workspace: &WorkspaceResult,
    external_key: &str,
    idempotency_key: &str,
) -> RequestParams<Value> {
    let mut request = params(
        json!({
            "externalKey": external_key,
            "role": "root",
            "engaged": false,
        }),
        1_000,
    );
    request.workspace_id = Some(workspace.workspace_id.clone());
    request.agent_id = Some("agent-session-bootstrap".to_owned());
    request.idempotency_key = Some(idempotency_key.to_owned());
    request
}

fn session_result(response: &Value) -> &Value {
    response
        .get("result")
        .unwrap_or_else(|| panic!("expected session.open success: {response}"))
}

fn bootstrap_plan_digest(response: &Value) -> &str {
    session_result(response)["bootstrapPlanDigest"]
        .as_str()
        .unwrap_or_else(|| panic!("session receipt lacks bootstrapPlanDigest: {response}"))
}

/// The receipt a replay returns is byte-identical to the original response
/// except for the capability token, which is re-issued from the live session
/// so the proof matches the session's current grant, engagement and expiry.
fn without_capability(response: &Value) -> Value {
    let mut body = response.clone();
    body["result"]
        .as_object_mut()
        .unwrap_or_else(|| panic!("session result is an object: {response}"))
        .remove("capabilityToken");
    body
}

fn fresh_capability(response: &Value) -> &str {
    session_result(response)["capabilityToken"]
        .as_str()
        .unwrap_or_else(|| panic!("replayed receipt carries a fresh capability: {response}"))
}

#[test]
fn existing_external_session_cannot_be_reused_across_workspaces() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let first_workspace = register_workspace(&harness, &mut connection, "bootstrap-first");
    let second_workspace = register_workspace(&harness, &mut connection, "bootstrap-second");

    let first = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&first_workspace, "shared-host-session", "open-first"),
    );
    assert!(first.get("result").is_some(), "{first}");

    let cross_workspace = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&second_workspace, "shared-host-session", "open-second"),
    );

    assert_eq!(stable_error(&cross_workspace), "PROJECT_MISMATCH");
}

#[test]
fn recovered_duplicate_external_session_is_rejected_even_for_an_exact_workspace_match() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let first_workspace = register_workspace(&harness, &mut connection, "bootstrap-recover-first");
    let second_workspace =
        register_workspace(&harness, &mut connection, "bootstrap-recover-second");
    let external_key = "recovered-duplicate-host-session";

    let opened = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&first_workspace, external_key, "open-before-recover"),
    );
    assert!(opened.get("result").is_some(), "{opened}");

    let mut duplicate = harness
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Session)
        .expect("durable sessions")
        .into_iter()
        .find(|snapshot| {
            snapshot
                .session
                .as_ref()
                .is_some_and(|record| record.workspace_id == first_workspace.workspace_id)
        })
        .expect("opened session is durable");
    let duplicate_session_id = Uuid::new_v4().to_string();
    duplicate.workspace = harness
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Workspace)
        .expect("durable workspaces")
        .into_iter()
        .find(|snapshot| snapshot.workspace.workspace_id == second_workspace.workspace_id)
        .expect("second workspace is durable")
        .workspace;
    let duplicate_session = duplicate.session.as_mut().expect("session row");
    duplicate_session.workspace_id = second_workspace.workspace_id.clone();
    duplicate_session.session_id = duplicate_session_id;
    duplicate.response = json!({"fixture":"cross-workspace-duplicate"});
    harness
        .persistence
        .commit_identity_bundle(RuntimeIdentityTransition {
            operation: "test.session.inject".to_owned(),
            scope_digest: "b".repeat(64),
            idempotency_key: "duplicate-session".to_owned(),
            request_digest: "c".repeat(64),
            expected_workspace_mode: None,
            expected_inventory_generation: None,
            expected_session_status: None,
            expected_delegation_status: None,
            expected_context_generation: None,
            snapshot: duplicate,
            committed_at_unix_ms: 1_000,
        })
        .expect("legacy duplicate session fixture");

    let recovered = Harness::with_persistence(
        RuntimeConfig::default(),
        harness.persistence.clone(),
        12,
        "recovered-bootstrap-token".to_owned(),
    );
    recovered.runtime.recover().expect("runtime recovery");
    let mut recovered_connection = recovered.connection(ClientKind::Cli);
    let response = recovered.call(
        &mut recovered_connection,
        RpcMethod::SessionOpen,
        session_open_params(&first_workspace, external_key, "open-after-recover"),
    );

    assert_eq!(stable_error(&response), "PROJECT_MISMATCH");
}

#[test]
fn legal_bootstrap_digest_is_deterministic_and_external_key_replay_is_unchanged() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut connection, "bootstrap-digest");

    let initial = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&workspace, "stable-host-session", "open-initial"),
    );
    let initial_digest = bootstrap_plan_digest(&initial);
    assert_eq!(initial_digest.len(), 64);
    assert!(initial_digest.bytes().all(|byte| byte.is_ascii_hexdigit()));

    let repeated = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&workspace, "stable-host-session", "open-repeat"),
    );
    let same_input = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&workspace, "stable-host-session", "open-same-input"),
    );

    assert_eq!(
        session_result(&initial)["sessionId"],
        session_result(&repeated)["sessionId"],
        "an existing external key must continue to reopen the same session"
    );
    assert_eq!(
        bootstrap_plan_digest(&repeated),
        bootstrap_plan_digest(&same_input),
        "the same request and bootstrap snapshot must have a byte-identical digest"
    );

    harness.clock.set(1_250);
    let idempotent_replay = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&workspace, "stable-host-session", "open-repeat"),
    );
    assert_eq!(
        without_capability(&repeated),
        without_capability(&idempotent_replay),
        "the existing session.open idempotency receipt must replay unchanged"
    );
    assert!(
        !fresh_capability(&idempotent_replay).is_empty(),
        "the replayed receipt must carry a capability issued from the live session"
    );
}

#[test]
fn recovered_inactive_root_reopens_with_host_label_drift_and_keeps_binding() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut connection, "bootstrap-host-label-drift");
    let mut initial = session_open_params(
        &workspace,
        "host-label-drift-session",
        "open-before-host-restart",
    );
    initial.work_item_id = Some("ROUTE-PERSISTED".to_owned());
    let opened = harness.call(&mut connection, RpcMethod::SessionOpen, initial);
    let original_session_id = session_result(&opened)["sessionId"]
        .as_str()
        .expect("session id")
        .to_owned();

    let recovered = Harness::with_persistence(
        RuntimeConfig::default(),
        harness.persistence.clone(),
        13,
        "recovered-host-label-token".to_owned(),
    );
    recovered.runtime.recover().expect("runtime recovery");
    let mut recovered_connection = recovered.connection(ClientKind::Hook);
    let mut reopen = session_open_params(
        &workspace,
        "host-label-drift-session",
        "open-after-host-restart",
    );
    reopen.agent_id = Some("host-hook".to_owned());
    let reopened = recovered.call(&mut recovered_connection, RpcMethod::SessionOpen, reopen);

    assert_eq!(session_result(&reopened)["sessionId"], original_session_id);
    let durable = recovered
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Session)
        .expect("durable sessions")
        .into_iter()
        .filter_map(|snapshot| snapshot.session)
        .find(|session| session.session_id == original_session_id)
        .expect("reopened durable session");
    assert_eq!(durable.agent_id, "host-hook");
    assert_eq!(
        durable.current_work_item.as_deref(),
        Some("ROUTE-PERSISTED")
    );
}

#[test]
fn a_new_host_event_durably_refreshes_the_same_root_session() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, "session-refresh");
    let first = harness_session_open(
        &harness,
        &mut connection,
        &workspace,
        "refreshable-host-session",
        "open-event-1",
    );
    let first: SessionResult = serde_json::from_value(first).expect("first session result");

    harness
        .clock
        .set(first.expires_at_unix_ms.saturating_add(1));
    let refreshed = harness_session_open(
        &harness,
        &mut connection,
        &workspace,
        "refreshable-host-session",
        "open-event-2",
    );
    let refreshed: SessionResult =
        serde_json::from_value(refreshed).expect("refreshed session result");

    assert_eq!(refreshed.session_id, first.session_id);
    assert!(refreshed.expires_at_unix_ms > first.expires_at_unix_ms);
    assert_ne!(refreshed.capability_token, first.capability_token);
    let durable_expiry = harness
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Session)
        .expect("typed session snapshots")
        .into_iter()
        .filter_map(|snapshot| snapshot.session)
        .filter(|session| session.session_id == refreshed.session_id)
        .map(|session| session.expires_at_unix_ms)
        .max()
        .expect("durable refreshed session");
    assert_eq!(durable_expiry, refreshed.expires_at_unix_ms);
}

#[test]
fn a_new_host_event_refreshes_a_delegated_session_without_losing_attestation_or_binding() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection(ClientKind::HostAdapter);
    let mut host_register = params(
        json!({"adapterId":"host-refresh","capabilities":["create","attest"]}),
        1_000,
    );
    host_register.capability_token = Some(harness.host_credential());
    host_register.idempotency_key = Some("host-refresh-register".to_owned());
    let _ = harness.call(&mut host, RpcMethod::HostRegister, host_register);

    let mut hook = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut hook, "delegated-refresh");
    let root = support::open_root_session(
        &harness,
        &mut hook,
        &workspace,
        "root-refresh-agent",
        "root-refresh-external",
        Some("WORK"),
    );
    let created = create_root_series_delegation(
        &harness,
        &mut hook,
        &workspace,
        &root,
        "root-refresh-agent",
        "WORK",
        "requirement-analysis",
        &["RA"],
        "delegated-refresh-create",
    );
    let delegation_id = created["delegationId"]
        .as_str()
        .unwrap_or_else(|| panic!("delegation.create failed: {created}"))
        .to_owned();

    let action = harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":"host-refresh"}), 1_000),
    );
    let action = &action["result"];
    let child_session_id = "00000000-0000-0000-0000-000000000501";
    let mut ack = params(
        json!({
            "adapterId":"host-refresh",
            "ack":{
                "ackId":"00000000-0000-0000-0000-000000000502",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-refresh-task",
                "sessionId":child_session_id
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("delegated-refresh-ack".to_owned());
    let acked = harness.call(&mut host, RpcMethod::HostActionAck, ack);
    assert!(acked.get("result").is_some(), "{acked}");

    let mut accept = params(
        json!({
            "delegationId":delegation_id,
            "claimId":action["claimId"],
            "actionId":action["actionId"],
            "childSessionId":child_session_id,
            "expiresAtUnixMs":100_000
        }),
        1_000,
    );
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.work_item_id = Some("WORK".to_owned());
    accept.idempotency_key = Some("delegated-refresh-accept".to_owned());
    let accepted = harness.call(&mut hook, RpcMethod::DelegationAccept, accept);
    assert_eq!(accepted["result"]["status"], "running", "{accepted}");

    let delegated_open = |key: &str| {
        let mut open = params(
            json!({
                "externalKey":"delegated-refresh-external",
                "role":"series",
                "engaged":false,
                "delegationId":delegation_id
            }),
            1_000,
        );
        open.workspace_id = Some(workspace.workspace_id.clone());
        open.agent_id = Some("series-refresh-agent".to_owned());
        open.session_id = Some(child_session_id.to_owned());
        open.work_item_id = Some("WORK".to_owned());
        open.idempotency_key = Some(key.to_owned());
        open
    };
    let first = harness.call(
        &mut hook,
        RpcMethod::SessionOpen,
        delegated_open("delegated-open-event-1"),
    );
    let first: SessionResult = serde_json::from_value(session_result(&first).clone())
        .unwrap_or_else(|error| panic!("first delegated open failed: {error}"));
    harness
        .clock
        .set(first.expires_at_unix_ms.saturating_add(1));
    let refreshed = harness.call(
        &mut hook,
        RpcMethod::SessionOpen,
        delegated_open("delegated-open-event-2"),
    );
    let refreshed: SessionResult = serde_json::from_value(session_result(&refreshed).clone())
        .unwrap_or_else(|error| panic!("delegated refresh failed: {error}"));

    assert_eq!(refreshed.session_id, first.session_id);
    assert!(refreshed.expires_at_unix_ms > first.expires_at_unix_ms);
    let durable = harness
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Session)
        .expect("typed session snapshots")
        .into_iter()
        .filter_map(|snapshot| snapshot.session)
        .filter(|session| session.session_id == child_session_id)
        .max_by_key(|session| session.expires_at_unix_ms)
        .expect("durable delegated session");
    assert_eq!(durable.current_work_item.as_deref(), Some("WORK"));
    assert_eq!(
        durable.delegation_id.as_deref(),
        Some(delegation_id.as_str())
    );
    assert_eq!(durable.expires_at_unix_ms, refreshed.expires_at_unix_ms);
    let attestation = harness
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Delegation)
        .expect("typed delegation snapshots")
        .into_iter()
        .find_map(|snapshot| snapshot.attestation)
        .expect("physical delegation attestation");
    assert_eq!(attestation.delegation_id, delegation_id);
    assert_eq!(attestation.physical_session_id, child_session_id);
    assert!(attestation.expires_at_unix_ms > first.expires_at_unix_ms);
}

/// Regression: a delegated child session must remain reopenable after the
/// physical attestation's accept-time TTL snapshot has expired, as long as the
/// delegation is still `running` and within its own `deadline_unix_ms`. The
/// attestation's `expires_at_unix_ms` is an immutable digest anchor; the
/// delegation deadline and the live session TTL are the real liveness bounds.
#[test]
fn delegated_session_survives_expired_attestation_ttl_while_within_delegation_deadline() {
    // 90s session TTL (default), 100s attestation TTL snapshot, 1_000_000ms
    // delegation deadline -> deadline far outlives the attestation snapshot.
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection(ClientKind::HostAdapter);
    let mut host_register = params(
        json!({"adapterId":"host-attest-expiry","capabilities":["create","attest"]}),
        1_000,
    );
    host_register.capability_token = Some(harness.host_credential());
    host_register.idempotency_key = Some("host-attest-expiry-register".to_owned());
    let _ = harness.call(&mut host, RpcMethod::HostRegister, host_register);

    let mut hook = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut hook, "attest-expiry");
    let root = support::open_root_session(
        &harness,
        &mut hook,
        &workspace,
        "root-attest-expiry",
        "root-attest-expiry-external",
        Some("WORK"),
    );
    let created = create_root_series_delegation(
        &harness,
        &mut hook,
        &workspace,
        &root,
        "root-attest-expiry",
        "WORK",
        "requirement-analysis",
        &["RA"],
        "attest-expiry-create",
    );
    let delegation_id = created["delegationId"]
        .as_str()
        .unwrap_or_else(|| panic!("delegation.create failed: {created}"))
        .to_owned();
    let delegation_deadline = created["deadlineUnixMs"]
        .as_u64()
        .unwrap_or_else(|| panic!("delegation.create lacks deadline: {created}"));

    let action = harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":"host-attest-expiry"}), 1_000),
    );
    let action = &action["result"];
    let child_session_id = "00000000-0000-0000-0000-000000000601";
    let mut ack = params(
        json!({
            "adapterId":"host-attest-expiry",
            "ack":{
                "ackId":"00000000-0000-0000-0000-000000000602",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-attest-expiry-task",
                "sessionId":child_session_id
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("attest-expiry-ack".to_owned());
    let acked = harness.call(&mut host, RpcMethod::HostActionAck, ack);
    assert!(acked.get("result").is_some(), "{acked}");

    // Accept with a 100s attestation TTL snapshot (capped by delegation
    // deadline), strictly shorter than the delegation deadline above.
    let mut accept = params(
        json!({
            "delegationId":delegation_id,
            "claimId":action["claimId"],
            "actionId":action["actionId"],
            "childSessionId":child_session_id,
            "expiresAtUnixMs":100_000
        }),
        1_000,
    );
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.work_item_id = Some("WORK".to_owned());
    accept.idempotency_key = Some("attest-expiry-accept".to_owned());
    let accepted = harness.call(&mut hook, RpcMethod::DelegationAccept, accept);
    assert_eq!(accepted["result"]["status"], "running", "{accepted}");

    let delegated_open = |key: &str| {
        let mut open = params(
            json!({
                "externalKey":"attest-expiry-external",
                "role":"series",
                "engaged":false,
                "delegationId":delegation_id
            }),
            1_000,
        );
        open.workspace_id = Some(workspace.workspace_id.clone());
        open.agent_id = Some("series-attest-expiry".to_owned());
        open.session_id = Some(child_session_id.to_owned());
        open.work_item_id = Some("WORK".to_owned());
        open.idempotency_key = Some(key.to_owned());
        open
    };

    // First open lands the delegated session.
    let first = harness.call(
        &mut hook,
        RpcMethod::SessionOpen,
        delegated_open("attest-expiry-open-1"),
    );
    assert!(
        first.get("result").is_some(),
        "first delegated open failed: {first}"
    );

    // Advance the clock past the attestation's 100s TTL snapshot, but keep it
    // well inside the delegation's 1_000_000ms deadline.
    let now_past_attestation_ttl = 150_000;
    harness.clock.set(now_past_attestation_ttl);

    // Sanity: the durable attestation snapshot really is expired at this clock.
    let expired_attestation = harness
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Delegation)
        .expect("typed delegation snapshots")
        .into_iter()
        .find_map(|snapshot| snapshot.attestation)
        .expect("physical delegation attestation");
    assert_eq!(expired_attestation.expires_at_unix_ms, 100_000);
    assert!(
        expired_attestation.expires_at_unix_ms <= now_past_attestation_ttl,
        "attestation TTL snapshot must be expired by this point"
    );

    // Reopen must succeed: attestation TTL snapshot no longer gates live
    // operations; the delegation deadline is the authoritative upper bound.
    let reopened = harness.call(
        &mut hook,
        RpcMethod::SessionOpen,
        delegated_open("attest-expiry-open-2"),
    );
    assert!(
        reopened.get("result").is_some(),
        "delegated session reopen must succeed after attestation TTL expired but delegation deadline is still live: {reopened}"
    );

    // Negative control: once the clock crosses the delegation deadline, the
    // reopened session must be rejected (the deadline upper bound still bites).
    harness.clock.set(delegation_deadline.saturating_add(1));
    let past_deadline = harness.call(
        &mut hook,
        RpcMethod::SessionOpen,
        delegated_open("attest-expiry-open-3"),
    );
    assert_eq!(
        stable_error(&past_deadline),
        "DELEGATION_ATTESTATION_FAILED",
        "delegated session reopen after the delegation deadline must fail: {past_deadline}"
    );
}

#[test]
fn heartbeat_replay_returns_the_receipt_with_a_current_capability() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut connection, "heartbeat-replay");
    let opened = harness.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&workspace, "heartbeat-host-session", "open-heartbeat"),
    );
    let opened: SessionResult =
        serde_json::from_value(session_result(&opened).clone()).expect("session.open result");

    let mut first_request = session_params(
        &workspace,
        &opened,
        "agent-session-bootstrap",
        json!({}),
        2_000,
    );
    first_request.idempotency_key = Some("heartbeat-stable".to_owned());
    let first = harness.call(&mut connection, RpcMethod::SessionHeartbeat, first_request);
    let first_session: SessionResult =
        serde_json::from_value(session_result(&first).clone()).expect("heartbeat result");

    harness.clock.set(1_250);
    let mut replay_request = session_params(
        &workspace,
        &first_session,
        "agent-session-bootstrap",
        json!({}),
        2_000,
    );
    replay_request.idempotency_key = Some("heartbeat-stable".to_owned());
    let replay = harness.call(&mut connection, RpcMethod::SessionHeartbeat, replay_request);

    assert_eq!(without_capability(&first), without_capability(&replay));
    assert!(
        !fresh_capability(&replay).is_empty(),
        "the replayed receipt must carry a capability issued from the live session"
    );
    let durable = harness
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Session)
        .expect("typed session snapshots")
        .into_iter()
        .find_map(|snapshot| snapshot.session)
        .expect("durable session");
    assert_eq!(durable.expires_at_unix_ms, first_session.expires_at_unix_ms);
}

#[test]
fn new_boot_reuses_session_id_but_issues_only_a_current_boot_capability() {
    let first = Harness::new(RuntimeConfig::default());
    let mut first_connection = first.connection(ClientKind::Cli);
    let workspace = register_workspace(&first, &mut first_connection, "bootstrap-new-boot");
    let initial = harness_session_open(
        &first,
        &mut first_connection,
        &workspace,
        "stable-across-boots",
        "open-first-boot",
    );

    let second = Harness::with_persistence(
        RuntimeConfig::default(),
        first.persistence.clone(),
        99,
        "second-boot-token".to_owned(),
    );
    second.runtime.recover().expect("typed identity recovers");
    let mut second_connection = second.connection(ClientKind::Cli);
    let reopened = harness_session_open(
        &second,
        &mut second_connection,
        &workspace,
        "stable-across-boots",
        "open-second-boot",
    );

    assert_eq!(initial["sessionId"], reopened["sessionId"]);
    assert_ne!(initial["capabilityToken"], reopened["capabilityToken"]);
}

#[test]
fn recovery_keeps_historical_inactive_sessions_outside_the_active_capacity_limit() {
    let config = RuntimeConfig {
        max_sessions: 1,
        ..RuntimeConfig::default()
    };
    let seeded = Harness::new(config.clone());
    let mut connection = seeded.connection(ClientKind::Cli);
    let workspace = register_workspace(&seeded, &mut connection, "bootstrap-history-capacity");
    let opened = seeded.call(
        &mut connection,
        RpcMethod::SessionOpen,
        session_open_params(&workspace, "historical-session-1", "open-history-1"),
    );
    assert!(opened.get("result").is_some(), "{opened}");

    let mut duplicate = seeded
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Session)
        .expect("durable sessions")
        .into_iter()
        .next()
        .expect("first durable session");
    let duplicate_session = duplicate.session.as_mut().expect("session row");
    duplicate_session.session_id = Uuid::new_v4().to_string();
    duplicate_session.external_key_hash = hex::encode(Sha256::digest(b"historical-session-2"));
    duplicate.response = json!({"fixture":"second historical session"});
    seeded
        .persistence
        .commit_identity_bundle(RuntimeIdentityTransition {
            operation: "test.session.inject".to_owned(),
            scope_digest: "d".repeat(64),
            idempotency_key: "historical-session-2".to_owned(),
            request_digest: "e".repeat(64),
            expected_workspace_mode: None,
            expected_inventory_generation: None,
            expected_session_status: None,
            expected_delegation_status: None,
            expected_context_generation: None,
            snapshot: duplicate,
            committed_at_unix_ms: 1_000,
        })
        .expect("second historical session fixture");

    let recovered = Harness::with_persistence(
        config,
        seeded.persistence.clone(),
        100,
        "historical-session-recovery-token".to_owned(),
    );
    recovered
        .runtime
        .recover()
        .expect("inactive historical sessions do not consume active capacity");
}

#[test]
fn recovery_imports_legacy_root_identity_without_persisting_secrets() {
    let seeded = Harness::new(RuntimeConfig::default());
    let workspace = WorkspaceResult {
        workspace_id: Uuid::new_v4().to_string(),
        canonical_root: "C:/ae-sdd-tests/legacy-import".to_owned(),
        project_key: "project-legacy-import".to_owned(),
        mode: WorkspaceMode::Shadow,
        inventory_generation: 7,
    };
    let session_id = Uuid::new_v4().to_string();
    let external_key = "legacy-physical-session-secret";
    let legacy_capability = "legacy-boot-capability-secret";
    seeded
        .persistence
        .store_record(
            "workspace/v1",
            &workspace.workspace_id,
            &serde_json::to_value(&workspace).expect("legacy workspace serializes"),
        )
        .expect("seed legacy workspace");
    seeded
        .persistence
        .store_record(
            "session/v1",
            &session_id,
            &json!({
                "workspaceId": workspace.workspace_id,
                "agentId": "agent-session-bootstrap",
                "externalKey": external_key,
                "currentWorkItem": null,
                "result": {
                    "sessionId": session_id,
                    "role": "root",
                    "engaged": false,
                    "expiresAtUnixMs": 9_000,
                    "contextGeneration": 4,
                    "capabilityToken": legacy_capability,
                },
                "delegationId": null,
                "currentTurnId": null,
                "currentTurnSeq": 0,
                "active": true,
            }),
        )
        .expect("seed legacy root session");

    let recovered = Harness::with_persistence(
        RuntimeConfig::default(),
        seeded.persistence.clone(),
        101,
        "legacy-import-current-boot-token".to_owned(),
    );
    recovered
        .runtime
        .recover()
        .expect("legacy identity recovers");

    let typed_workspaces = recovered
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Workspace)
        .expect("typed workspaces");
    assert_eq!(typed_workspaces.len(), 1);
    assert_eq!(
        typed_workspaces[0].workspace.workspace_id,
        workspace.workspace_id
    );
    let typed_sessions = recovered
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Session)
        .expect("typed sessions");
    assert_eq!(typed_sessions.len(), 1);
    let typed_session = typed_sessions[0]
        .session
        .as_ref()
        .expect("typed session row");
    assert_eq!(typed_session.session_id, session_id);
    assert_eq!(
        typed_session.external_key_hash,
        hex::encode(Sha256::digest(external_key.as_bytes()))
    );
    let typed_receipt = serde_json::to_string(&typed_sessions[0]).expect("receipt serializes");
    assert!(!typed_receipt.contains(external_key));
    assert!(!typed_receipt.contains(legacy_capability));
    assert!(typed_sessions[0].response.get("capabilityToken").is_none());

    let mut connection = recovered.connection(ClientKind::Cli);
    let reopened = harness_session_open(
        &recovered,
        &mut connection,
        &workspace,
        external_key,
        "open-imported-legacy-session",
    );
    assert_eq!(reopened["sessionId"], session_id);
    assert_ne!(reopened["capabilityToken"], legacy_capability);
    assert!(
        reopened["capabilityToken"]
            .as_str()
            .is_some_and(|token| !token.is_empty())
    );
}

fn harness_session_open(
    harness: &Harness,
    connection: &mut ae_sdd_runtime::ConnectionState,
    workspace: &WorkspaceResult,
    external_key: &str,
    idempotency_key: &str,
) -> Value {
    session_result(&harness.call(
        connection,
        RpcMethod::SessionOpen,
        session_open_params(workspace, external_key, idempotency_key),
    ))
    .clone()
}
