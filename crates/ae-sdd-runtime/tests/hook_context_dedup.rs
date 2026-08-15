//! Hook user-prompt projection delivery dedup.
//!
//! An engaged `hook.user_prompt` injects the precomputed projection exactly
//! once per digest: a repeated prompt against an unchanged projection answers
//! `contextKind: "no_change"` without the body, a moved projection or a
//! completed compact rehydrate forces a full redelivery, and a client that
//! echoes the daemon revision/digest cursor negotiates the no-change itself.

mod support;

use std::sync::atomic::Ordering;

use ae_sdd_protocol::{ClientKind, ConfirmationRef, RpcMethod, WorkspaceMode};
use ae_sdd_runtime::{ConnectionState, RuntimeConfig, SessionResult, WorkspaceResult};
use serde_json::{Value, json};

use support::{
    Harness, open_root_session, params, parity_transition_payload, register_workspace, result,
    session_params,
};

#[test]
fn repeated_user_prompt_hooks_deliver_the_projection_once() {
    let (harness, workspace, mut hook, session) = engaged_session("dedup-repeat");

    let first = hook_user_prompt(
        &harness,
        &mut hook,
        &workspace,
        &session,
        "event-1",
        json!({}),
    );
    assert_eq!(first["decision"], "context");
    assert_eq!(first["contextKind"], "full");
    assert!(
        first.get("context").is_some(),
        "the first delivery carries the full projection body"
    );
    let digest = first["contextDigest"]
        .as_str()
        .expect("full delivery reports the projection digest")
        .to_owned();

    let second = hook_user_prompt(
        &harness,
        &mut hook,
        &workspace,
        &session,
        "event-2",
        json!({}),
    );
    assert_eq!(second["decision"], "context");
    assert_eq!(second["contextKind"], "no_change");
    assert!(
        second.get("context").is_none(),
        "an unchanged projection must not be re-delivered: {second}"
    );
    assert_eq!(second["contextDigest"], digest);
}

#[test]
fn a_projection_move_redelivers_the_full_body() {
    let (harness, workspace, mut hook, session) = engaged_session("dedup-move");
    let first = hook_user_prompt(
        &harness,
        &mut hook,
        &workspace,
        &session,
        "event-1",
        json!({}),
    );
    let first_digest = first["contextDigest"].as_str().expect("digest").to_owned();
    let second = hook_user_prompt(
        &harness,
        &mut hook,
        &workspace,
        &session,
        "event-2",
        json!({}),
    );
    assert_eq!(second["contextKind"], "no_change");

    harness
        .business
        .projection_bytes
        .store(64, Ordering::Release);
    assert_eq!(
        harness
            .runtime
            .refresh_active_contexts()
            .expect("context refresh"),
        1
    );

    let third = hook_user_prompt(
        &harness,
        &mut hook,
        &workspace,
        &session,
        "event-3",
        json!({}),
    );
    assert_eq!(third["contextKind"], "full");
    assert!(
        third.get("context").is_some(),
        "a moved projection must be delivered in full again"
    );
    assert_ne!(third["contextDigest"], first_digest);
}

#[test]
fn a_client_supplied_known_digest_skips_the_first_delivery() {
    let (harness, workspace, mut hook, session) = engaged_session("dedup-cursor");
    let mut context_get = session_params(&workspace, &session, "agent", json!({}), 1_000);
    context_get.work_item_id = Some("WORK".to_owned());
    let projection = result(&harness.call(&mut hook, RpcMethod::ContextGet, context_get));

    // A fresh session has never been delivered to; a client echoing the exact
    // daemon cursor still receives no body.
    let negotiated = hook_user_prompt(
        &harness,
        &mut hook,
        &workspace,
        &session,
        "event-known",
        json!({
            "knownRevision": projection["contextRevision"],
            "knownDigest": projection["digest"],
        }),
    );
    assert_eq!(negotiated["decision"], "context");
    assert_eq!(negotiated["contextKind"], "no_change");
    assert!(negotiated.get("context").is_none());

    // A digest that does not match the daemon projection is not a cursor.
    let (harness2, workspace2, mut hook2, session2) = engaged_session("dedup-cursor-stale");
    let stale = hook_user_prompt(
        &harness2,
        &mut hook2,
        &workspace2,
        &session2,
        "event-stale",
        json!({"knownRevision": 1, "knownDigest": "f".repeat(64)}),
    );
    assert_eq!(stale["contextKind"], "full");
    assert!(stale.get("context").is_some());
}

#[test]
fn the_first_hook_after_a_compact_rehydrate_forces_a_full_redelivery() {
    let (harness, workspace, mut hook, session) = engaged_session("dedup-compact");
    let mut host = harness.connection(ClientKind::HostAdapter);
    let mut register = params(
        json!({"adapterId":"host-a","capabilities":["compact"]}),
        1_000,
    );
    register.capability_token = Some(harness.host_credential());
    register.idempotency_key = Some("host-register".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostRegister, register));

    let first = hook_user_prompt(
        &harness,
        &mut hook,
        &workspace,
        &session,
        "event-1",
        json!({}),
    );
    assert_eq!(first["contextKind"], "full");
    let digest = first["contextDigest"].as_str().expect("digest").to_owned();
    let second = hook_user_prompt(
        &harness,
        &mut hook,
        &workspace,
        &session,
        "event-2",
        json!({}),
    );
    assert_eq!(second["contextKind"], "no_change");

    let mut context_get = session_params(&workspace, &session, "agent", json!({}), 1_000);
    context_get.work_item_id = Some("WORK".to_owned());
    let projection = result(&harness.call(&mut hook, RpcMethod::ContextGet, context_get));

    let mut compact = session_params(
        &workspace,
        &session,
        "agent",
        json!({
            "previousGeneration":0,
            "snapshotDigest":"a".repeat(64),
            "deadlineUnixMs":2_000,
            "adapterId":"host-a"
        }),
        1_000,
    );
    compact.work_item_id = Some("WORK".to_owned());
    compact.idempotency_key = Some("compact-request".to_owned());
    let cycle = result(&harness.call(&mut hook, RpcMethod::CompactRequest, compact));
    assert_eq!(cycle["status"], "compact-requested");

    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":"host-a"}), 1_000),
    ));
    let mut ack = params(
        json!({
            "adapterId":"host-a",
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000000201",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":null,
                "sessionId":null
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("compact-ack".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));

    let mut rehydrate = session_params(
        &workspace,
        &session,
        "agent",
        json!({
            "knownRevision":projection["contextRevision"],
            "knownDigest":projection["digest"]
        }),
        1_000,
    );
    rehydrate.work_item_id = Some("WORK".to_owned());
    let restored = result(&harness.call(&mut hook, RpcMethod::ContextProject, rehydrate));
    assert_eq!(restored["kind"], "no_change");

    // The compact replaced the host-side context window, so the next prompt
    // must receive the full body even though the daemon digest never moved.
    let third = hook_user_prompt(
        &harness,
        &mut hook,
        &workspace,
        &session,
        "event-3",
        json!({}),
    );
    assert_eq!(third["decision"], "context");
    assert_eq!(third["contextKind"], "full");
    assert!(
        third.get("context").is_some(),
        "the first hook after a compact rehydrate must deliver the full body"
    );
    assert_eq!(third["contextDigest"], digest);
}

fn engaged_session(suffix: &str) -> (Harness, WorkspaceResult, ConnectionState, SessionResult) {
    let harness = Harness::new(RuntimeConfig::default());
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut cli, suffix);
    let mut admin = harness.connection(ClientKind::Admin);
    let mut drain = params(json!({"stop":false}), 1_000);
    drain.idempotency_key = Some(format!("drain-{suffix}"));
    drain.confirmation = Some(confirmation());
    let _ = result(&harness.call(&mut admin, RpcMethod::RuntimeDrain, drain));
    let mut transition = params(
        parity_transition_payload(WorkspaceMode::RustCanary, 1_000),
        1_000,
    );
    transition.workspace_id = Some(workspace.workspace_id.clone());
    transition.idempotency_key = Some(format!("mode-{suffix}"));
    transition.confirmation = Some(confirmation());
    let canary: WorkspaceResult = serde_json::from_value(result(&harness.call(
        &mut admin,
        RpcMethod::WorkspaceModeTransition,
        transition,
    )))
    .expect("canary workspace");

    let mut hook = harness.connection(ClientKind::Hook);
    let session = open_root_session(
        &harness,
        &mut hook,
        &canary,
        "agent",
        "external",
        Some("WORK"),
    );
    (harness, canary, hook, session)
}

fn hook_user_prompt(
    harness: &Harness,
    hook: &mut ConnectionState,
    workspace: &WorkspaceResult,
    session: &SessionResult,
    event_id: &str,
    extra: Value,
) -> Value {
    let mut payload = json!({"hookEventId":event_id,"turnSeq":1,"hostPayload":{}});
    if let (Some(payload), Some(extra)) = (payload.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            payload.insert(key.clone(), value.clone());
        }
    }
    let mut request = session_params(workspace, session, "agent", payload, 100);
    request.turn_id = Some("turn".to_owned());
    request.work_item_id = Some("WORK".to_owned());
    request.idempotency_key = Some(format!("request-{event_id}"));
    result(&harness.call(hook, RpcMethod::HookUserPrompt, request))
}

fn confirmation() -> ConfirmationRef {
    ConfirmationRef {
        confirmation_id: "confirmation".to_owned(),
        approved_by: "user".to_owned(),
        approved_at: "2026-01-01T00:00:00Z".to_owned(),
    }
}
