//! A running delegation's deadline can be legitimately extended.
//!
//! The liveness judgement itself is correct: past its deadline a delegation
//! stops accepting work. What was missing is any way to express a legitimate
//! extension, so a long series that outlived its original deadline could only
//! be cancelled and rebuilt, discarding work already done.
//!
//! Renewal is deliberately narrow. Only the parent may ask, because a child
//! able to extend its own deadline would hold its grant for as long as it
//! liked. The bound is a total lifetime measured from creation rather than from
//! the deadline in force, so repeated renewals cannot walk the deadline forward
//! indefinitely.

mod support;

use ae_sdd_protocol::{ClientKind, RpcMethod, StableErrorCode};
use ae_sdd_runtime::{DurableEvent, IdempotencyReceipt, PersistencePort, RuntimeConfig};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use support::{
    Harness, create_root_series_delegation, open_root_session, params, register_workspace, result,
    session_params, stable_error,
};

const ADAPTER: &str = "host-a";
const CHILD: &str = "00000000-0000-0000-0000-000000000501";

struct Fixture {
    harness: Harness,
    workspace: ae_sdd_runtime::WorkspaceResult,
    root: ae_sdd_runtime::SessionResult,
    root_connection: ae_sdd_runtime::ConnectionState,
    delegation_id: String,
    created_deadline: u64,
}

/// Drives create → ACK → accept so the delegation is `running`, which is the
/// only state renewal applies to.
fn running_delegation(suffix: &str, _deadline_unix_ms: u64) -> Fixture {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection(ClientKind::HostAdapter);
    let mut register = params(
        json!({"adapterId":ADAPTER,"capabilities":["create","attest","ack"]}),
        1_000,
    );
    register.capability_token = Some(harness.host_credential());
    register.idempotency_key = Some(format!("host-register-{suffix}"));
    let _ = result(&harness.call(&mut host, RpcMethod::HostRegister, register));

    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, suffix);
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
        &format!("create-{suffix}"),
    );
    let deadline_unix_ms = delegation["deadlineUnixMs"]
        .as_u64()
        .expect("daemon-derived deadline");
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
                "ackId":"00000000-0000-0000-0000-000000000502",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-1",
                "sessionId":CHILD
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some(format!("ack-{suffix}"));
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));

    let mut accept = params(
        json!({
            "delegationId":delegation_id,
            "claimId":action["claimId"],
            "actionId":action["actionId"],
            "childSessionId":CHILD,
            "expiresAtUnixMs":deadline_unix_ms - 100
        }),
        1_000,
    );
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.work_item_id = Some("WORK".to_owned());
    accept.idempotency_key = Some(format!("accept-{suffix}"));
    let accepted = result(&harness.call(&mut root_connection, RpcMethod::DelegationAccept, accept));
    assert_eq!(accepted["status"], "running");

    Fixture {
        harness,
        workspace,
        root,
        root_connection,
        delegation_id,
        created_deadline: deadline_unix_ms,
    }
}

/// Issues a renewal as the parent session unless `as_child` is set.
fn renew(fixture: &mut Fixture, deadline: u64, key: &str) -> Value {
    let mut request = session_params(
        &fixture.workspace,
        &fixture.root,
        "root-agent",
        json!({"delegationId": fixture.delegation_id, "deadlineUnixMs": deadline}),
        1_000,
    );
    request.work_item_id = Some("WORK".to_owned());
    request.idempotency_key = Some(key.to_owned());
    fixture.harness.call(
        &mut fixture.root_connection,
        RpcMethod::DelegationRenew,
        request,
    )
}

fn commit_legacy_renewal(fixture: &Fixture, deadline: u64, key: &str) {
    let scope = format!("delegation-renew\0{}", fixture.delegation_id);
    let event_payload = json!({"scope":scope,"key":key});
    let payload_digest = hex::encode(Sha256::digest(
        serde_json::to_vec(&event_payload).expect("renewal event serializes"),
    ));
    let response = json!({
        "delegationId":fixture.delegation_id,
        "deadlineUnixMs":deadline
    });
    let request_digest = hex::encode(Sha256::digest(
        serde_json::to_vec(&json!({
            "delegationId":fixture.delegation_id,
            "deadlineUnixMs":deadline
        }))
        .expect("renewal request serializes"),
    ));
    fixture
        .harness
        .persistence
        .commit_event_and_receipt(
            DurableEvent {
                event_store_id: fixture
                    .harness
                    .persistence
                    .event_store_id()
                    .expect("event store identity")
                    .to_string(),
                event_seq: 0,
                boot_id: fixture.harness.runtime.boot_id().to_string(),
                kind: "delegation.renewed".to_owned(),
                workspace_id: Some(fixture.workspace.workspace_id.clone()),
                session_id: Some(fixture.root.session_id.clone()),
                work_item_id: Some("WORK".to_owned()),
                payload: event_payload,
                payload_digest,
            },
            IdempotencyReceipt {
                scope,
                key: key.to_owned(),
                request_digest,
                response_json: serde_json::to_string(&response)
                    .expect("renewal response serializes"),
                event_seq: 0,
            },
        )
        .expect("legacy renewal event and receipt commit atomically");
}

#[test]
fn parent_extends_the_deadline_and_liveness_follows_the_new_value() {
    let mut fixture = running_delegation("renew-happy", 5_000);
    let extended = fixture.created_deadline + 60_000;
    let renewed = result(&renew(&mut fixture, extended, "renew-1"));
    assert_eq!(
        renewed["deadlineUnixMs"], extended,
        "the projection must report the extended deadline"
    );

    // Past the original deadline but inside the renewed one: the delegation is
    // still live, which is the whole point of renewal.
    fixture.harness.clock.set(fixture.created_deadline + 1_000);
    let mut reopen = params(
        json!({"externalKey":"root-external","role":"root","engaged":false}),
        1_000,
    );
    reopen.workspace_id = Some(fixture.workspace.workspace_id.clone());
    reopen.agent_id = Some("root-agent".to_owned());
    reopen.work_item_id = Some("WORK".to_owned());
    reopen.idempotency_key = Some("renew-happy-root-refresh".to_owned());
    fixture.root = serde_json::from_value(result(&fixture.harness.call(
        &mut fixture.root_connection,
        RpcMethod::SessionOpen,
        reopen,
    )))
    .expect("root refresh after the original delegation deadline");
    let status = result(&{
        let mut request = session_params(
            &fixture.workspace,
            &fixture.root,
            "root-agent",
            json!({"delegationId": fixture.delegation_id}),
            1_000,
        );
        request.work_item_id = Some("WORK".to_owned());
        fixture.harness.call(
            &mut fixture.root_connection,
            RpcMethod::DelegationStatus,
            request,
        )
    });
    assert_eq!(status["status"], "running");
    assert_eq!(status["deadlineUnixMs"], extended);
}

#[test]
fn a_non_parent_session_cannot_renew() {
    let mut fixture = running_delegation("renew-role", 5_000);
    // The child session holds a real capability token, so this exercises the
    // parent check rather than an authentication failure.
    let mut child_open = params(
        json!({
            "externalKey":"child-external",
            "role":"series",
            "engaged":false,
            "delegationId": fixture.delegation_id
        }),
        1_000,
    );
    child_open.workspace_id = Some(fixture.workspace.workspace_id.clone());
    child_open.agent_id = Some("child-agent".to_owned());
    child_open.session_id = Some(CHILD.to_owned());
    child_open.work_item_id = Some("WORK".to_owned());
    child_open.idempotency_key = Some("child-open".to_owned());
    let child: ae_sdd_runtime::SessionResult =
        serde_json::from_value(result(&fixture.harness.call(
            &mut fixture.root_connection,
            RpcMethod::SessionOpen,
            child_open,
        )))
        .expect("child session decodes");

    let mut request = session_params(
        &fixture.workspace,
        &child,
        "child-agent",
        json!({
            "delegationId": fixture.delegation_id,
            "deadlineUnixMs": fixture.created_deadline + 60_000
        }),
        1_000,
    );
    request.work_item_id = Some("WORK".to_owned());
    request.idempotency_key = Some("renew-as-child".to_owned());
    let response = fixture.harness.call(
        &mut fixture.root_connection,
        RpcMethod::DelegationRenew,
        request,
    );
    assert_eq!(
        stable_error(&response),
        "ROLE_OPERATION_FORBIDDEN",
        "a child must not be able to extend its own deadline"
    );
}

#[test]
fn a_deadline_in_the_past_or_beyond_the_lifetime_bound_is_refused() {
    let mut fixture = running_delegation("renew-bounds", 5_000);

    let past = fixture
        .harness
        .clock
        .0
        .load(std::sync::atomic::Ordering::Acquire);
    let response = renew(&mut fixture, past, "renew-past");
    assert_eq!(
        stable_error(&response),
        "OPERATION_SCHEMA_INVALID",
        "a deadline at or before now cannot renew anything"
    );

    // Well past the configured total lifetime measured from creation.
    let beyond = RuntimeConfig::default().max_delegation_lifetime_ms + 1_000_000;
    let response = renew(&mut fixture, beyond, "renew-beyond");
    assert_eq!(
        stable_error(&response),
        "OPERATION_SCHEMA_INVALID",
        "the total lifetime bound must hold"
    );
}

#[test]
fn renewal_cannot_shorten_a_deadline() {
    let mut fixture = running_delegation("renew-shorten", 60_000);
    let response = renew(&mut fixture, 30_000, "renew-shorter");
    assert_eq!(
        stable_error(&response),
        "OPERATION_SCHEMA_INVALID",
        "renewal is an extension, not an arbitrary reassignment"
    );
}

#[test]
fn only_a_running_delegation_can_be_renewed() {
    // Stop before accept, so the delegation is still `spawning`.
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection(ClientKind::HostAdapter);
    let mut register = params(
        json!({"adapterId":ADAPTER,"capabilities":["create","attest","ack"]}),
        1_000,
    );
    register.capability_token = Some(harness.host_credential());
    register.idempotency_key = Some("host-register-state".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostRegister, register));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "renew-state");
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
        "create-state",
    );

    let mut request = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"delegationId": delegation["delegationId"], "deadlineUnixMs": 65_000}),
        1_000,
    );
    request.work_item_id = Some("WORK".to_owned());
    request.idempotency_key = Some("renew-spawning".to_owned());
    let response = harness.call(&mut root_connection, RpcMethod::DelegationRenew, request);
    assert_eq!(
        stable_error(&response),
        "DELEGATION_ATTESTATION_FAILED",
        "a delegation without a live child has nothing to renew"
    );
}

#[test]
fn renewal_leaves_the_attestation_and_grant_untouched() {
    let mut fixture = running_delegation("renew-attest", 5_000);
    let before = result(&{
        let mut request = session_params(
            &fixture.workspace,
            &fixture.root,
            "root-agent",
            json!({"delegationId": fixture.delegation_id}),
            1_000,
        );
        request.work_item_id = Some("WORK".to_owned());
        fixture.harness.call(
            &mut fixture.root_connection,
            RpcMethod::DelegationStatus,
            request,
        )
    });

    let extended = fixture.created_deadline + 60_000;
    let renewed = result(&renew(&mut fixture, extended, "renew-attest-1"));

    // Renewal moves the deadline and nothing else. The physical child binding
    // and the grant are what the attestation rests on, so a renewal that could
    // shift either would be a way to launder an unattested claim.
    assert_eq!(renewed["childSessionId"], before["childSessionId"]);
    assert_eq!(renewed["grant"], before["grant"]);
    assert_eq!(renewed["childRole"], before["childRole"]);
    assert_eq!(renewed["actionId"], before["actionId"]);
    assert_ne!(
        renewed["deadlineUnixMs"], before["deadlineUnixMs"],
        "the deadline is the one field renewal is allowed to move"
    );
}

#[test]
fn replaying_one_renewal_does_not_extend_twice() {
    let mut fixture = running_delegation("renew-replay", 5_000);
    let extended = fixture.created_deadline + 60_000;
    let first = result(&renew(&mut fixture, extended, "renew-once"));
    let replayed = result(&renew(&mut fixture, extended, "renew-once"));
    assert_eq!(
        first["deadlineUnixMs"], replayed["deadlineUnixMs"],
        "the same idempotency key must return the same deadline"
    );
    assert_eq!(replayed["deadlineUnixMs"], extended);
}

#[test]
fn recovery_preserves_the_current_child_session_status() {
    let mut fixture = running_delegation("renew-recovery-session", 5_000);
    let mut child_open = params(
        json!({
            "externalKey":"renew-recovery-child",
            "role":"series",
            "engaged":false,
            "delegationId":fixture.delegation_id
        }),
        1_000,
    );
    child_open.workspace_id = Some(fixture.workspace.workspace_id.clone());
    child_open.agent_id = Some("renew-recovery-child-agent".to_owned());
    child_open.session_id = Some(CHILD.to_owned());
    child_open.work_item_id = Some("WORK".to_owned());
    child_open.idempotency_key = Some("renew-recovery-child-open".to_owned());
    let _child: ae_sdd_runtime::SessionResult =
        serde_json::from_value(result(&fixture.harness.call(
            &mut fixture.root_connection,
            RpcMethod::SessionOpen,
            child_open,
        )))
        .expect("child session opens before recovery");

    let extended = fixture.created_deadline + 60_000;
    let mut operational = fixture
        .harness
        .persistence
        .load_record("delegation/v1", &fixture.delegation_id)
        .expect("operational delegation loads")
        .expect("operational delegation exists");
    operational["deadlineUnixMs"] = json!(extended);
    fixture
        .harness
        .persistence
        .store_record("delegation/v1", &fixture.delegation_id, &operational)
        .expect("legacy operational renewal is staged");

    commit_legacy_renewal(&fixture, extended, "legacy-renewal");

    fixture
        .harness
        .runtime
        .recover()
        .expect("renewal recovery succeeds");

    let mut child_reopen = params(
        json!({
            "externalKey":"renew-recovery-child",
            "role":"series",
            "engaged":false,
            "delegationId":fixture.delegation_id
        }),
        1_000,
    );
    child_reopen.workspace_id = Some(fixture.workspace.workspace_id.clone());
    child_reopen.agent_id = Some("renew-recovery-child-agent".to_owned());
    child_reopen.session_id = Some(CHILD.to_owned());
    child_reopen.work_item_id = Some("WORK".to_owned());
    child_reopen.idempotency_key = Some("renew-recovery-child-reopen".to_owned());
    let reopened = fixture.harness.call(
        &mut fixture.root_connection,
        RpcMethod::SessionOpen,
        child_reopen,
    );
    let child: ae_sdd_runtime::SessionResult = serde_json::from_value(result(&reopened))
        .expect("child session reopens after renewal recovery");
    assert_eq!(child.session_id, CHILD);
}

#[test]
fn recovery_rejects_an_operational_deadline_not_proven_by_the_receipt() {
    let fixture = running_delegation("renew-recovery-conflict", 5_000);
    let proven_deadline = fixture.created_deadline + 60_000;
    commit_legacy_renewal(&fixture, proven_deadline, "legacy-renewal-conflict");

    let unproven_deadline = proven_deadline + 1_000;
    let mut operational = fixture
        .harness
        .persistence
        .load_record("delegation/v1", &fixture.delegation_id)
        .expect("operational delegation loads")
        .expect("operational delegation exists");
    operational["deadlineUnixMs"] = json!(unproven_deadline);
    fixture
        .harness
        .persistence
        .store_record("delegation/v1", &fixture.delegation_id, &operational)
        .expect("external deadline mutation is staged");

    let error = fixture
        .harness
        .runtime
        .recover()
        .expect_err("an unproven operational deadline must fail closed");
    assert_eq!(error.code(), StableErrorCode::ExternalStateConflict);
}

#[test]
fn parent_status_projects_renewal_only_inside_the_expiry_window() {
    let mut fixture = running_delegation("renew-projection", 5_000);
    let status = |fixture: &mut Fixture| {
        let mut request = session_params(
            &fixture.workspace,
            &fixture.root,
            "root-agent",
            json!({"delegationId": fixture.delegation_id}),
            1_000,
        );
        request.work_item_id = Some("WORK".to_owned());
        result(&fixture.harness.call(
            &mut fixture.root_connection,
            RpcMethod::DelegationStatus,
            request,
        ))
    };

    let early = status(&mut fixture);
    assert!(early.get("nextAction").is_none());

    fixture.harness.clock.set(fixture.created_deadline - 100);
    let mut reopen = params(
        json!({"externalKey":"root-external","role":"root","engaged":false}),
        1_000,
    );
    reopen.workspace_id = Some(fixture.workspace.workspace_id.clone());
    reopen.agent_id = Some("root-agent".to_owned());
    reopen.session_id = Some(fixture.root.session_id.clone());
    reopen.work_item_id = Some("WORK".to_owned());
    reopen.idempotency_key = Some("renew-projection-root-reopen".to_owned());
    fixture.root = serde_json::from_value(result(&fixture.harness.call(
        &mut fixture.root_connection,
        RpcMethod::SessionOpen,
        reopen,
    )))
    .expect("root session reopens");
    let near_expiry = status(&mut fixture);
    assert_eq!(near_expiry["nextAction"]["kind"], "renew-delegation");
    assert_eq!(
        near_expiry["nextAction"]["delegationId"],
        fixture.delegation_id
    );
    assert_eq!(
        near_expiry["nextAction"]["currentDeadlineUnixMs"],
        fixture.created_deadline
    );
    assert!(
        near_expiry["nextAction"]["deadlineUnixMs"]
            .as_u64()
            .expect("proposed deadline")
            > fixture.created_deadline
    );
}

#[test]
fn child_status_never_projects_parent_renewal_authority() {
    let mut fixture = running_delegation("renew-child-projection", 5_000);
    let mut child_open = params(
        json!({
            "externalKey":"child-projection-external",
            "role":"series",
            "engaged":false,
            "delegationId":fixture.delegation_id
        }),
        1_000,
    );
    child_open.workspace_id = Some(fixture.workspace.workspace_id.clone());
    child_open.agent_id = Some("child-projection-agent".to_owned());
    child_open.session_id = Some(CHILD.to_owned());
    child_open.work_item_id = Some("WORK".to_owned());
    child_open.idempotency_key = Some("child-projection-open".to_owned());
    let _child: ae_sdd_runtime::SessionResult =
        serde_json::from_value(result(&fixture.harness.call(
            &mut fixture.root_connection,
            RpcMethod::SessionOpen,
            child_open,
        )))
        .expect("child session decodes");

    fixture.harness.clock.set(fixture.created_deadline - 100);
    let mut child_reopen = params(
        json!({
            "externalKey":"child-projection-external",
            "role":"series",
            "engaged":false,
            "delegationId":fixture.delegation_id
        }),
        1_000,
    );
    child_reopen.workspace_id = Some(fixture.workspace.workspace_id.clone());
    child_reopen.agent_id = Some("child-projection-agent".to_owned());
    child_reopen.session_id = Some(CHILD.to_owned());
    child_reopen.work_item_id = Some("WORK".to_owned());
    child_reopen.idempotency_key = Some("child-projection-reopen".to_owned());
    let child: ae_sdd_runtime::SessionResult =
        serde_json::from_value(result(&fixture.harness.call(
            &mut fixture.root_connection,
            RpcMethod::SessionOpen,
            child_reopen,
        )))
        .expect("child session reopens");
    let mut request = session_params(
        &fixture.workspace,
        &child,
        "child-projection-agent",
        json!({"delegationId":fixture.delegation_id}),
        1_000,
    );
    request.work_item_id = Some("WORK".to_owned());
    let status = result(&fixture.harness.call(
        &mut fixture.root_connection,
        RpcMethod::DelegationStatus,
        request,
    ));
    assert!(status.get("nextAction").is_none());
}
