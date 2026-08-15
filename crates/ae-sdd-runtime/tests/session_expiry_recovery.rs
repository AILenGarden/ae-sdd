//! A root session whose TTL elapsed is recoverable under a new agent label.
//!
//! Nothing clears `active` when a session's TTL simply runs out: the flag is
//! only lowered by an explicit `session.close`. The recovery drift allowance
//! therefore has to treat an elapsed deadline as inactive too. Otherwise the
//! host, whose agent label may legitimately change between turns, can never
//! reattach to its own `externalKey` — `session.open` fails the turn-mismatch
//! check and the workflow deadlocks with no way forward.

mod support;

use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::RuntimeConfig;
use serde_json::json;

use support::{Harness, canary_workspace, open_root_session, params, result, stable_error};

/// Reopens the same external key under a caller-chosen agent label.
fn reopen_as(
    harness: &Harness,
    connection: &mut ae_sdd_runtime::ConnectionState,
    workspace: &ae_sdd_runtime::WorkspaceResult,
    agent_id: &str,
    external_key: &str,
    idempotency: &str,
) -> serde_json::Value {
    let mut request = params(
        json!({"externalKey": external_key, "role": "root", "engaged": true}),
        1_000,
    );
    request.workspace_id = Some(workspace.workspace_id.clone());
    request.agent_id = Some(agent_id.to_owned());
    request.idempotency_key = Some(idempotency.to_owned());
    harness.call(connection, RpcMethod::SessionOpen, request)
}

#[test]
fn expired_root_session_recovers_under_a_new_agent_label() {
    let harness = Harness::new(RuntimeConfig::default());
    let workspace = canary_workspace(&harness, "expiry-recovery");
    let mut connection = harness.connection(ClientKind::Cli);

    let opened = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "host-a",
        "conversation-1",
        None,
    );

    // Let the session's own TTL elapse. `active` stays true because only
    // session.close lowers it, which is exactly the state the fix must accept.
    harness.clock.set(opened.expires_at_unix_ms + 1);

    let response = reopen_as(
        &harness,
        &mut connection,
        &workspace,
        "host-b",
        "conversation-1",
        "reopen-after-expiry",
    );

    let recovered: ae_sdd_runtime::SessionResult =
        serde_json::from_value(result(&response)).expect("recovered session decodes");
    assert_eq!(
        recovered.session_id, opened.session_id,
        "recovery must reattach the same durable session, not mint a second one"
    );
    assert!(
        recovered.expires_at_unix_ms > opened.expires_at_unix_ms,
        "recovery must issue a fresh capability deadline"
    );
}

#[test]
fn live_root_session_still_rejects_a_competing_agent_label() {
    let harness = Harness::new(RuntimeConfig::default());
    let workspace = canary_workspace(&harness, "expiry-guard");
    let mut connection = harness.connection(ClientKind::Cli);

    open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "host-a",
        "conversation-2",
        None,
    );

    // No clock advance: the session is genuinely live, so the isolation
    // boundary must still hold. Recovery may not become a way to hijack an
    // active session by presenting a different agent label.
    let response = reopen_as(
        &harness,
        &mut connection,
        &workspace,
        "host-b",
        "conversation-2",
        "reopen-while-live",
    );

    assert_eq!(
        stable_error(&response),
        "TURN_IDENTITY_MISMATCH",
        "a live session must not be reattachable under a competing agent label"
    );
}
