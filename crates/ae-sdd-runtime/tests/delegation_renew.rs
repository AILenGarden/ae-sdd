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

use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::RuntimeConfig;
use serde_json::{Value, json};

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
