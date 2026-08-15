//! A host Hook must reach the daemon before routing exists.
//!
//! The bootstrap turn is the one that establishes routing, so demanding a turn
//! and a Work Item from it made the first host event unreachable: the Hook
//! subprocess holds no durable state and cannot know either value. The daemon
//! therefore allocates the turn and resolves the Work Item from the session.

mod support;

use std::sync::atomic::Ordering;

use ae_sdd_protocol::{ClientKind, RequestParams, RpcMethod};
use ae_sdd_runtime::{RuntimeConfig, SessionResult, WorkspaceResult};
use serde_json::{Value, json};

use support::{
    Harness, canary_workspace, open_root_session, register_workspace, result, session_params,
    stable_error,
};

/// Builds a Hook request shaped like a host subprocess: no turn, no Work Item.
fn unbound_hook_request(
    workspace: &WorkspaceResult,
    session: &SessionResult,
    agent_id: &str,
    event_id: &str,
) -> RequestParams<Value> {
    let mut request = session_params(
        workspace,
        session,
        agent_id,
        json!({
            "hookEventId": event_id,
            "hostPayload": {"prompt":"/ae-sdd"},
        }),
        100,
    );
    request.idempotency_key = Some(format!("request-{event_id}"));
    request
}

#[test]
fn ae_sdd_command_bootstraps_and_binds_a_work_item_idempotently() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = canary_workspace(&harness, "hook-unbound");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-a",
        "external-a",
        None,
    );

    let first = result(&harness.call(
        &mut connection,
        RpcMethod::HookUserPrompt,
        unbound_hook_request(&workspace, &session, "agent-a", "event-1"),
    ));

    let turn_id = first["turnId"]
        .as_str()
        .unwrap_or_else(|| panic!("the daemon must report the turn it allocated: {first}"));
    assert_eq!(
        first["turnSeq"], 1,
        "the first allocated turn is sequence 1: {first}"
    );
    assert_eq!(first["workItemId"], "WORK-MINTED", "{first}");
    assert_eq!(
        harness.business.operation_calls.load(Ordering::Acquire),
        1,
        "the daemon performs exactly one bootstrap intake"
    );

    // The next prompt is a new turn boundary, so the sequence advances. This is
    // the case a stateless client could never satisfy on its own: it would
    // resend sequence 1 and be rejected.
    let second = result(&harness.call(
        &mut connection,
        RpcMethod::HookUserPrompt,
        unbound_hook_request(&workspace, &session, "agent-a", "event-2"),
    ));
    assert_eq!(second["turnSeq"], 2, "{second}");
    assert_ne!(
        second["turnId"].as_str(),
        Some(turn_id),
        "a new prompt opens a new turn"
    );
    assert_eq!(second["workItemId"], "WORK-MINTED", "{second}");
    assert_eq!(
        harness.business.operation_calls.load(Ordering::Acquire),
        1,
        "a bound session must not create a second Work Item"
    );
}

#[test]
fn an_ordinary_prompt_does_not_bootstrap_a_work_item() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = canary_workspace(&harness, "hook-ordinary");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-a",
        "external-a",
        None,
    );
    let mut request = unbound_hook_request(&workspace, &session, "agent-a", "ordinary-1");
    request.payload["hostPayload"]["prompt"] = json!("explain the current state");

    let outcome = result(&harness.call(&mut connection, RpcMethod::HookUserPrompt, request));

    assert!(outcome.get("workItemId").is_none(), "{outcome}");
    assert_eq!(
        harness.business.operation_calls.load(Ordering::Acquire),
        0,
        "session-level context delivery is not command-level bootstrap"
    );
}

#[test]
fn an_ae_sdd_prefix_with_trailing_text_does_not_bootstrap_a_work_item() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = canary_workspace(&harness, "hook-prefixed-command");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-a",
        "external-a",
        None,
    );
    let mut request = unbound_hook_request(&workspace, &session, "agent-a", "prefixed-1");
    request.payload["hostPayload"]["prompt"] = json!("/ae-sdd extra");

    let outcome = result(&harness.call(&mut connection, RpcMethod::HookUserPrompt, request));

    assert!(outcome.get("workItemId").is_none(), "{outcome}");
    assert_eq!(
        harness.business.operation_calls.load(Ordering::Acquire),
        0,
        "only the exact /ae-sdd command may bootstrap"
    );
}

#[test]
fn hooks_inside_one_turn_join_the_turn_the_prompt_opened() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, "hook-turn-join");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-a",
        "external-a",
        None,
    );

    let mut prompt_request = unbound_hook_request(&workspace, &session, "agent-a", "prompt-1");
    prompt_request.payload["hostPayload"]["prompt"] = json!("ordinary prompt");
    let prompt = result(&harness.call(&mut connection, RpcMethod::HookUserPrompt, prompt_request));
    let turn_id = prompt["turnId"]
        .as_str()
        .expect("allocated turn")
        .to_owned();

    // A tool Hook runs inside the turn already in flight; it must not open a
    // new one, or the turn sequence would advance once per tool call.
    for (index, method) in [RpcMethod::HookPreTool, RpcMethod::HookPostTool]
        .into_iter()
        .enumerate()
    {
        let outcome = result(&harness.call(
            &mut connection,
            method,
            unbound_hook_request(&workspace, &session, "agent-a", &format!("tool-{index}")),
        ));
        assert_eq!(
            outcome["turnId"].as_str(),
            Some(turn_id.as_str()),
            "{outcome}"
        );
        assert_eq!(outcome["turnSeq"], 1, "{outcome}");
    }
}

#[test]
fn a_bound_session_attributes_its_hook_to_the_bound_work_item() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, "hook-bound-item");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-a",
        "external-a",
        Some("WORK-1"),
    );

    let outcome = result(&harness.call(
        &mut connection,
        RpcMethod::HookUserPrompt,
        unbound_hook_request(&workspace, &session, "agent-a", "event-1"),
    ));

    // Omitting the field adopts the binding, so a host that never learns the
    // Work Item still produces correctly attributed events.
    assert_eq!(outcome["workItemId"], "WORK-1", "{outcome}");
}

#[test]
fn a_hook_cannot_claim_a_work_item_the_session_is_not_bound_to() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, "hook-item-conflict");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-a",
        "external-a",
        Some("WORK-1"),
    );

    let mut request = unbound_hook_request(&workspace, &session, "agent-a", "event-1");
    request.work_item_id = Some("WORK-OTHER".to_owned());

    let conflict = harness.call(&mut connection, RpcMethod::HookUserPrompt, request);

    assert_eq!(stable_error(&conflict), "TURN_IDENTITY_MISMATCH");
}

#[test]
fn an_explicit_turn_still_requires_its_sequence() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut connection, "hook-explicit-turn");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-a",
        "external-a",
        None,
    );

    // A host that names its own turn is still validated, not allocated for:
    // dropping the sequence would let a client silently reorder turns.
    let mut request = unbound_hook_request(&workspace, &session, "agent-a", "event-1");
    request.turn_id = Some("turn-host-1".to_owned());

    let rejected = harness.call(&mut connection, RpcMethod::HookUserPrompt, request);

    assert_eq!(stable_error(&rejected), "OPERATION_SCHEMA_INVALID");
}
