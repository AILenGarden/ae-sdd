//! A session that creates a Work Item is bound to it without reopening.
//!
//! `workitem.create` is Workspace-scoped: the request cannot name the Work
//! Item it creates, so the business authority mints the business key and
//! returns it in the operation result. The daemon must then bind the calling
//! session to that key — durably and with the same context projection
//! `session.open` installs — or the bootstrap turn deadlocks: the session
//! could never attribute its own Hooks to the Work Item it just created.

mod support;

use ae_sdd_protocol::{ClientKind, ConfirmationRef, RequestParams, RpcMethod, WorkspaceMode};
use ae_sdd_runtime::{
    PersistencePort, RuntimeConfig, RuntimeIdentityKind, SessionResult, WorkspaceResult,
};
use serde_json::{Value, json};

use support::{
    Harness, open_root_session, params, parity_transition_payload, register_workspace, result,
    session_params,
};

/// Registers a workspace and moves it into the Rust writer mode, the way the
/// bootstrap flow finds it once the daemon owns writes.
fn canary_workspace(harness: &Harness, suffix: &str) -> WorkspaceResult {
    let mut connection = harness.connection(ClientKind::Admin);
    let workspace = register_workspace(harness, &mut connection, suffix);
    let confirmation = || ConfirmationRef {
        confirmation_id: format!("confirmation-{suffix}"),
        approved_by: "user".to_owned(),
        approved_at: "2026-01-01T00:00:00Z".to_owned(),
    };
    let mut drain = params(json!({"stop":false}), 1_000);
    drain.idempotency_key = Some(format!("drain-{suffix}"));
    drain.confirmation = Some(confirmation());
    result(&harness.call(&mut connection, RpcMethod::RuntimeDrain, drain));
    let mut transition = params(
        parity_transition_payload(WorkspaceMode::RustCanary, 1_000),
        1_000,
    );
    transition.workspace_id = Some(workspace.workspace_id.clone());
    transition.idempotency_key = Some(format!("mode-{suffix}"));
    transition.confirmation = Some(confirmation());
    serde_json::from_value(result(&harness.call(
        &mut connection,
        RpcMethod::WorkspaceModeTransition,
        transition,
    )))
    .expect("canary workspace result decodes")
}

/// Builds a `workitem.create` call the way the bootstrap agent flow does:
/// the session identity is present, the Work Item is not yet known.
fn create_request(
    workspace: &WorkspaceResult,
    session: &SessionResult,
    agent_id: &str,
    work_item_id: Option<&str>,
) -> RequestParams<Value> {
    let mut request = session_params(
        workspace,
        session,
        agent_id,
        json!({
            "operation": "workitem.create",
            "payload": {"entryNode":"Story"},
        }),
        1_000,
    );
    request.work_item_id = work_item_id.map(str::to_owned);
    request.idempotency_key = Some(format!("create-{}", work_item_id.unwrap_or("minted")));
    request
}

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
            "hostPayload": {"prompt":"implement the story"},
        }),
        100,
    );
    request.idempotency_key = Some(format!("request-{event_id}"));
    request
}

#[test]
fn an_explicitly_named_create_binds_the_calling_session() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = canary_workspace(&harness, "create-bind-named");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-a",
        "external-a",
        None,
    );

    let created = result(&harness.call(
        &mut connection,
        RpcMethod::OperationExecute,
        create_request(&workspace, &session, "agent-a", Some("WORK-NEW")),
    ));
    assert_eq!(created["data"]["workItemId"], "WORK-NEW", "{created}");

    // A later Hook carries neither turn nor Work Item; attribution must come
    // from the binding the create installed.
    let prompt = result(&harness.call(
        &mut connection,
        RpcMethod::HookUserPrompt,
        unbound_hook_request(&workspace, &session, "agent-a", "event-1"),
    ));
    assert_eq!(prompt["workItemId"], "WORK-NEW", "{prompt}");

    // The binding went through the durable session-identity path, so a later
    // boot recovers it instead of silently dropping attribution.
    let durable = harness
        .persistence
        .list_identity_snapshots(RuntimeIdentityKind::Session)
        .expect("session identity snapshots load")
        .into_iter()
        .filter_map(|snapshot| snapshot.session)
        .find(|record| record.session_id == session.session_id)
        .expect("the session has a durable identity record");
    assert_eq!(durable.current_work_item.as_deref(), Some("WORK-NEW"));

    // The create also installed the same context projection `session.open`
    // would have, so `context.get` works without reopening the session.
    let mut get = session_params(&workspace, &session, "agent-a", json!({}), 1_000);
    get.work_item_id = Some("WORK-NEW".to_owned());
    let projection = result(&harness.call(&mut connection, RpcMethod::ContextGet, get));
    assert_eq!(
        projection["projection"]["workItemId"], "WORK-NEW",
        "{projection}"
    );
}

#[test]
fn a_replayed_session_open_keeps_the_create_binding() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = canary_workspace(&harness, "create-bind-replay");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-a",
        "external-a",
        None,
    );

    let created = result(&harness.call(
        &mut connection,
        RpcMethod::OperationExecute,
        create_request(&workspace, &session, "agent-a", Some("WORK-NEW")),
    ));
    assert_eq!(created["data"]["workItemId"], "WORK-NEW", "{created}");

    // The real host's next Hook is a fresh process that reopens the session
    // with the same external key and idempotency key it used for the first
    // open. That receipt predates the binding and must not roll it back.
    let mut reopen = harness.connection(ClientKind::Hook);
    let replayed = open_root_session(
        &harness,
        &mut reopen,
        &workspace,
        "agent-a",
        "external-a",
        None,
    );
    assert_eq!(replayed.session_id, session.session_id);

    let prompt = result(&harness.call(
        &mut reopen,
        RpcMethod::HookUserPrompt,
        unbound_hook_request(&workspace, &replayed, "agent-a", "event-after-replay"),
    ));
    assert_eq!(prompt["workItemId"], "WORK-NEW", "{prompt}");

    let mut get = session_params(&workspace, &replayed, "agent-a", json!({}), 1_000);
    get.work_item_id = Some("WORK-NEW".to_owned());
    let projection = result(&harness.call(&mut reopen, RpcMethod::ContextGet, get));
    assert_eq!(
        projection["projection"]["workItemId"], "WORK-NEW",
        "{projection}"
    );
}

#[test]
fn a_replayed_session_open_keeps_the_completed_compact_generation() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection(ClientKind::HostAdapter);
    let mut register = params(
        json!({"adapterId":"host-a","capabilities":["compact"]}),
        1_000,
    );
    register.capability_token = Some(harness.host_credential());
    register.idempotency_key = Some("host-register".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostRegister, register));

    let mut hook = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut hook, "open-replay-generation");
    let session = open_root_session(
        &harness,
        &mut hook,
        &workspace,
        "agent-a",
        "external-a",
        Some("WORK"),
    );

    let mut context_get = session_params(&workspace, &session, "agent-a", json!({}), 1_000);
    context_get.work_item_id = Some("WORK".to_owned());
    let projection = result(&harness.call(&mut hook, RpcMethod::ContextGet, context_get));

    let mut compact = session_params(
        &workspace,
        &session,
        "agent-a",
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
    assert_eq!(cycle["status"], "compact-requested", "{cycle}");

    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":"host-a"}), 1_000),
    ));
    assert_eq!(action["actionId"], cycle["actionId"]);
    let mut ack = params(
        json!({
            "adapterId":"host-a",
            "ack":{
                "ackId":"00000000-0000-0000-0000-000000000204",
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

    let mut status = session_params(
        &workspace,
        &session,
        "agent-a",
        json!({"compactId":cycle["compactId"]}),
        1_000,
    );
    status.work_item_id = Some("WORK".to_owned());
    let acknowledged = result(&harness.call(&mut hook, RpcMethod::CompactStatus, status));
    assert_eq!(
        acknowledged["status"], "host-acknowledged",
        "{acknowledged}"
    );

    let mut rehydrate = session_params(
        &workspace,
        &session,
        "agent-a",
        json!({
            "knownRevision":projection["contextRevision"],
            "knownDigest":projection["digest"]
        }),
        1_000,
    );
    rehydrate.work_item_id = Some("WORK".to_owned());
    let restored = result(&harness.call(&mut hook, RpcMethod::ContextProject, rehydrate));
    assert_eq!(restored["kind"], "no_change", "{restored}");

    let mut completed_status = session_params(
        &workspace,
        &session,
        "agent-a",
        json!({"compactId":cycle["compactId"]}),
        1_000,
    );
    completed_status.work_item_id = Some("WORK".to_owned());
    let completed = result(&harness.call(&mut hook, RpcMethod::CompactStatus, completed_status));
    assert_eq!(completed["status"], "context-restored", "{completed}");

    // Replaying the pre-compact open receipt must not roll the live session
    // back to the generation that receipt recorded.
    let replayed = open_root_session(
        &harness,
        &mut hook,
        &workspace,
        "agent-a",
        "external-a",
        Some("WORK"),
    );
    assert_eq!(replayed.session_id, session.session_id);

    let mut heartbeat = session_params(&workspace, &replayed, "agent-a", json!({}), 1_000);
    heartbeat.idempotency_key = Some("heartbeat-after-replay".to_owned());
    let heartbeat = result(&harness.call(&mut hook, RpcMethod::SessionHeartbeat, heartbeat));
    assert_eq!(heartbeat["contextGeneration"], 1, "{heartbeat}");
}

#[test]
fn a_daemon_minted_create_binds_the_calling_session() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Hook);
    let workspace = canary_workspace(&harness, "create-bind-minted");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-a",
        "external-a",
        None,
    );

    // The bootstrap create names no Work Item; the business authority mints
    // the key and the session binds to whatever the result reports.
    let created = result(&harness.call(
        &mut connection,
        RpcMethod::OperationExecute,
        create_request(&workspace, &session, "agent-a", None),
    ));
    let minted = created["data"]["workItemId"]
        .as_str()
        .unwrap_or_else(|| panic!("the create result must carry its business key: {created}"))
        .to_owned();

    let prompt = result(&harness.call(
        &mut connection,
        RpcMethod::HookUserPrompt,
        unbound_hook_request(&workspace, &session, "agent-a", "event-1"),
    ));
    assert_eq!(
        prompt["workItemId"].as_str(),
        Some(minted.as_str()),
        "{prompt}"
    );
}
