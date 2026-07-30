//! The first-turn bootstrap chain, end to end against the real business
//! adapter.
//!
//! The runtime crate's `create_binds_session.rs` proves the bind-after-create
//! contract against a stub `TestBusiness`, so it cannot catch a drift between
//! what the daemon binds and what the real authority actually mints or
//! persists. This module drives the exact sequence a fresh `/ae-sdd` session
//! deadlocked on, with the real `NativeBusinessAdapter` behind the runtime:
//!
//! 1. a root session opens with no Work Item,
//! 2. a `hook.user_prompt` succeeds unattributed instead of demanding a
//!    `workItemId` the caller cannot know,
//! 3. `operation.execute workitem.create` names no Work Item and the business
//!    authority mints the business key,
//! 4. the session attributes its next Hook to the minted key, serves
//!    `context.get` for it, and resolves `flow.snapshot` against the state the
//!    create really wrote — both on the connection that ran the create and
//!    after a real-host-style reopen: a fresh Hook connection replays
//!    `session.open` with the same external and idempotency keys, and that
//!    idempotent replay must not roll the binding back.
//!
//! A caller-supplied name must keep working through the same path, or the fix
//! would have traded the bootstrap deadlock for a broken explicit flow.

use ae_sdd_protocol::{ClientKind, RequestParams, RpcMethod};
use ae_sdd_runtime::{PersistencePort, RuntimeIdentityKind, SessionResult, WorkspaceResult};
use serde_json::{Value, json};
use std::fs;

use super::support::*;

#[test]
fn fresh_workspace_bootstrap_requires_explicit_hook_activation() {
    let harness = Harness::new();
    let mut hook = harness.connection(ClientKind::Hook);
    let mut register = plain_params(json!({
        "projectRoot":harness.workspace_root.path().to_string_lossy(),
        "projectKey":"fresh-bootstrap",
    }));
    register.idempotency_key = Some("fresh-bootstrap-register".to_owned());
    let workspace: WorkspaceResult = serde_json::from_value(success(&call(
        &harness.runtime,
        &mut hook,
        RpcMethod::WorkspaceRegister,
        register,
    )))
    .expect("fresh Hook enrollment decodes");
    assert_eq!(workspace.mode, ae_sdd_protocol::WorkspaceMode::Shadow);

    let mut activate = plain_params(json!({"bootstrapActivation":true}));
    activate.workspace_id = Some(workspace.workspace_id.clone());
    activate.idempotency_key = Some("fresh-bootstrap-activate".to_owned());
    activate.confirmation = Some(confirmation_ref(
        "fresh-bootstrap-command",
        "user:/ae-sdd",
        "2026-07-29T00:00:00Z",
    ));
    let activated: WorkspaceResult = serde_json::from_value(success(&call(
        &harness.runtime,
        &mut hook,
        RpcMethod::WorkspaceModeTransition,
        activate,
    )))
    .expect("explicit Hook activation decodes");
    assert_eq!(activated.mode, ae_sdd_protocol::WorkspaceMode::RustCanary);

    let mut cli = harness.connection(ClientKind::Cli);
    let mut forbidden = plain_params(json!({"bootstrapActivation":true}));
    forbidden.workspace_id = Some(workspace.workspace_id);
    forbidden.idempotency_key = Some("fresh-bootstrap-cli-activate".to_owned());
    forbidden.confirmation = Some(confirmation_ref(
        "fresh-bootstrap-cli",
        "user:cli",
        "2026-07-29T00:00:00Z",
    ));
    let rejected = call(
        &harness.runtime,
        &mut cli,
        RpcMethod::WorkspaceModeTransition,
        forbidden,
    );
    assert_eq!(stable_error(&rejected), "ROLE_OPERATION_FORBIDDEN");
}

#[test]
fn an_ordinary_shadow_registration_can_be_enrolled_by_the_later_ae_sdd_hook() {
    let harness = Harness::new();
    let mut cli = harness.connection(ClientKind::Cli);
    let mut register = plain_params(json!({
        "projectRoot":harness.workspace_root.path().to_string_lossy(),
        "projectKey":"shadow-then-bootstrap",
    }));
    register.idempotency_key = Some("shadow-first".to_owned());
    let shadow: WorkspaceResult = serde_json::from_value(success(&call(
        &harness.runtime,
        &mut cli,
        RpcMethod::WorkspaceRegister,
        register,
    )))
    .expect("shadow registration decodes");
    assert_eq!(shadow.mode, ae_sdd_protocol::WorkspaceMode::Shadow);

    let mut hook = harness.connection(ClientKind::Hook);
    let mut enroll = plain_params(json!({"bootstrapActivation":true}));
    enroll.workspace_id = Some(shadow.workspace_id.clone());
    enroll.idempotency_key = Some("shadow-bootstrap-enroll".to_owned());
    enroll.confirmation = Some(confirmation_ref(
        "shadow-bootstrap-command",
        "user:/ae-sdd",
        "2026-07-29T00:00:00Z",
    ));
    let canary: WorkspaceResult = serde_json::from_value(success(&call(
        &harness.runtime,
        &mut hook,
        RpcMethod::WorkspaceModeTransition,
        enroll,
    )))
    .expect("bootstrap enrollment decodes");
    assert_eq!(canary.mode, ae_sdd_protocol::WorkspaceMode::RustCanary);
    assert_eq!(canary.inventory_generation, shadow.inventory_generation + 1);
}

/// Builds a request that carries the session identity and nothing else: the
/// bootstrap caller cannot name a Work Item that does not exist yet, so
/// `workItemId` stays unset unless the test sets it deliberately.
fn session_params(
    workspace: &WorkspaceResult,
    session: &SessionResult,
    agent_id: &str,
    payload: Value,
) -> RequestParams<Value> {
    let mut request = plain_params(payload);
    request.workspace_id = Some(workspace.workspace_id.clone());
    request.session_id = Some(session.session_id.clone());
    request.agent_id = Some(agent_id.to_owned());
    request.capability_token = Some(session.capability_token.clone());
    request
}

/// Opens the root session the way bootstrap must: engaged, but bound to no
/// Work Item, because none exists to bind to.
fn open_unbound_root(
    harness: &Harness,
    connection: &mut ae_sdd_runtime::ConnectionState,
    workspace: &WorkspaceResult,
    external_key: &str,
    agent_id: &str,
) -> SessionResult {
    let mut open = plain_params(json!({
        "externalKey":external_key,
        "role":"root",
        "engaged":true,
    }));
    open.workspace_id = Some(workspace.workspace_id.clone());
    open.agent_id = Some(agent_id.to_owned());
    open.idempotency_key = Some(format!("bootstrap-open-{external_key}"));
    serde_json::from_value(success(&call(
        &harness.runtime,
        connection,
        RpcMethod::SessionOpen,
        open,
    )))
    .expect("root session opens without a Work Item")
}

/// Fires one host-shaped `hook.user_prompt`: no turn, no Work Item, exactly
/// the event a fresh session emits before it has created anything.
fn user_prompt(
    harness: &Harness,
    connection: &mut ae_sdd_runtime::ConnectionState,
    workspace: &WorkspaceResult,
    session: &SessionResult,
    agent_id: &str,
    event_id: &str,
) -> Value {
    let mut hook = session_params(
        workspace,
        session,
        agent_id,
        json!({
            "hookEventId":event_id,
            "hostPayload":{"prompt":"implement the story"},
        }),
    );
    hook.idempotency_key = Some(format!("bootstrap-hook-{event_id}"));
    // Hooks run on the negotiated fast path, so the request deadline must fit
    // its budget rather than the generous default the other calls use.
    hook.deadline_ms = 100;
    success(&call(
        &harness.runtime,
        connection,
        RpcMethod::HookUserPrompt,
        hook,
    ))
}

/// Runs `workitem.create` through `operation.execute`; `work_item_id` is the
/// optional caller-chosen business name, absent for the daemon-minted path.
fn create_work_item(
    harness: &Harness,
    connection: &mut ae_sdd_runtime::ConnectionState,
    workspace: &WorkspaceResult,
    session: &SessionResult,
    agent_id: &str,
    work_item_id: Option<&str>,
    idempotency_key: &str,
) -> Value {
    let mut create = session_params(
        workspace,
        session,
        agent_id,
        json!({
            "operation":"workitem.create",
            "payload":{"entryNode":"STORY"},
        }),
    );
    create.work_item_id = work_item_id.map(str::to_owned);
    create.idempotency_key = Some(idempotency_key.to_owned());
    success(&call(
        &harness.runtime,
        connection,
        RpcMethod::OperationExecute,
        create,
    ))
}

/// Asserts the minted key matches `^STORY-[0-9a-f]{8}$` by hand: pulling in a
/// regex dependency for one assertion would be heavier than the check.
fn assert_minted_shape(minted: &str) {
    let suffix = minted
        .strip_prefix("STORY-")
        .unwrap_or_else(|| panic!("a minted Work Item is prefixed by its entry node: {minted}"));
    assert_eq!(suffix.len(), 8, "minted suffix is 8 hex digits: {minted}");
    assert!(
        suffix
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "minted suffix is lowercase hex: {minted}"
    );
}

#[test]
fn ae_sdd_hook_bootstraps_route_and_exposes_the_requirement_analysis_handoff() {
    let harness = Harness::new();
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_and_cut_over(&harness, &mut cli);
    let mut hook_connection = harness.connection(ClientKind::Hook);
    let session = open_unbound_root(
        &harness,
        &mut hook_connection,
        &workspace,
        "bootstrap-route-external",
        "bootstrap-route-agent",
    );
    let mut hook = session_params(
        &workspace,
        &session,
        "bootstrap-route-agent",
        json!({
            "hookEventId":"bootstrap-route-command",
            "hostPayload":{"prompt":"/ae-sdd"},
        }),
    );
    hook.idempotency_key = Some("bootstrap-route-command".to_owned());
    hook.deadline_ms = 100;
    let prompt = success(&call(
        &harness.runtime,
        &mut hook_connection,
        RpcMethod::HookUserPrompt,
        hook,
    ));
    let work_item_id = prompt["workItemId"]
        .as_str()
        .unwrap_or_else(|| panic!("/ae-sdd must return its daemon-minted Work Item: {prompt}"))
        .to_owned();
    assert!(work_item_id.starts_with("ROUTE-"), "{prompt}");
    assert_eq!(prompt["context"]["nextAction"]["kind"], "analyze-route");

    let mut acquire = session_params(
        &workspace,
        &session,
        "bootstrap-route-agent",
        json!({
            "operation":"lease.acquire",
            "payload":{"owner":{"role":"root"},"ttlSeconds":300},
        }),
    );
    acquire.work_item_id = Some(work_item_id.clone());
    acquire.idempotency_key = Some("bootstrap-route-lease".to_owned());
    let lease = success(&call(
        &harness.runtime,
        &mut hook_connection,
        RpcMethod::OperationExecute,
        acquire,
    ));

    let mut decide = session_params(
        &workspace,
        &session,
        "bootstrap-route-agent",
        json!({
            "operation":"route.decide",
            "payload":{
                "requestedIntent":"repair the host bootstrap control plane",
                "impactFacts":[{"code":"impact.cross_module","level":"medium"}],
                "classificationConfidenceBps":9000
            },
        }),
    );
    decide.work_item_id = Some(work_item_id.clone());
    decide.lease_id = Some(
        lease["data"]["leaseId"]
            .as_str()
            .expect("lease id")
            .parse()
            .expect("typed lease id"),
    );
    decide.fencing_token = lease["data"]["fencingToken"].as_u64();
    decide.expected_revision = Some(0);
    decide.idempotency_key = Some("bootstrap-route-decide".to_owned());
    let routed = success(&call(
        &harness.runtime,
        &mut hook_connection,
        RpcMethod::OperationExecute,
        decide,
    ));
    assert_eq!(routed["data"]["scale"], "medium", "{routed}");
    assert_eq!(routed["data"]["selectedDesign"], "STORY", "{routed}");

    let mut next = session_params(&workspace, &session, "bootstrap-route-agent", json!({}));
    next.work_item_id = Some(work_item_id);
    let flow = success(&call(
        &harness.runtime,
        &mut hook_connection,
        RpcMethod::FlowNext,
        next,
    ));
    assert_eq!(flow["nextAction"]["kind"], "delegate-series", "{flow}");
    assert_eq!(
        flow["nextAction"]["seriesKind"], "requirement-analysis",
        "{flow}"
    );
}

#[test]
fn a_daemon_minted_create_bootstraps_the_whole_chain() {
    let harness = Harness::new();
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_and_cut_over(&harness, &mut cli);
    // The bootstrap turn is driven by the host Hook subprocess, so the whole
    // chain runs over one Hook connection and never reopens the session.
    let mut hook_connection = harness.connection(ClientKind::Hook);
    let session = open_unbound_root(
        &harness,
        &mut hook_connection,
        &workspace,
        "bootstrap-minted-external",
        "bootstrap-minted-agent",
    );

    // The first Hook predates any Work Item: admission must not demand a
    // `workItemId` the caller cannot know, and the result stays unattributed.
    let prompt = user_prompt(
        &harness,
        &mut hook_connection,
        &workspace,
        &session,
        "bootstrap-minted-agent",
        "event-before-create",
    );
    assert_eq!(prompt["engaged"], true, "{prompt}");
    assert!(
        prompt.get("workItemId").is_none(),
        "an unbound session attributes no Work Item: {prompt}"
    );

    // The create names no Work Item; the business authority mints the key and
    // reports both the business name and the directory identity.
    let created = create_work_item(
        &harness,
        &mut hook_connection,
        &workspace,
        &session,
        "bootstrap-minted-agent",
        None,
        "bootstrap-create-minted",
    );
    let minted = created["data"]["workItemId"]
        .as_str()
        .unwrap_or_else(|| panic!("the create result carries its business key: {created}"))
        .to_owned();
    assert_minted_shape(&minted);
    let state_machine_id = created["data"]["stateMachineId"]
        .as_str()
        .unwrap_or_else(|| panic!("the create result carries the directory identity: {created}"));
    assert!(
        state_machine_id.ends_with(minted.as_str()),
        "the directory identity embeds the business key: {created}"
    );
    // The state the key resolves to must really exist, or every later read
    // would fail on a name nothing wrote.
    assert!(
        harness
            .workspace_root
            .path()
            .join(format!(".auto-engineering/{state_machine_id}/state.json"))
            .is_file(),
        "the create committed its state.json under {state_machine_id}"
    );

    // Attribution: the next Hook still carries no `workItemId`, so the daemon
    // must resolve it from the binding the create installed.
    let prompt = user_prompt(
        &harness,
        &mut hook_connection,
        &workspace,
        &session,
        "bootstrap-minted-agent",
        "event-after-create",
    );
    assert_eq!(
        prompt["workItemId"].as_str(),
        Some(minted.as_str()),
        "the bound session attributes its Hook to the minted key: {prompt}"
    );

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
    assert_eq!(durable.current_work_item.as_deref(), Some(minted.as_str()));

    // The create installed the same context projection `session.open` would
    // have, so `context.get` serves the minted key without reopening.
    let mut get = session_params(&workspace, &session, "bootstrap-minted-agent", json!({}));
    get.work_item_id = Some(minted.clone());
    let projection = success(&call(
        &harness.runtime,
        &mut hook_connection,
        RpcMethod::ContextGet,
        get,
    ));
    assert_eq!(
        projection["projection"]["workItemId"], minted,
        "{projection}"
    );

    // The minted business key resolves through `read_state`, so the flow
    // authority projects the freshly created state by name.
    let mut snapshot = session_params(&workspace, &session, "bootstrap-minted-agent", json!({}));
    snapshot.work_item_id = Some(minted.clone());
    let flow = success(&call(
        &harness.runtime,
        &mut hook_connection,
        RpcMethod::FlowSnapshot,
        snapshot,
    ));
    assert!(
        flow["phase"].is_string(),
        "flow.snapshot projects the minted Work Item: {flow}"
    );
}

#[test]
fn a_real_host_reopen_on_a_fresh_hook_connection_keeps_the_binding() {
    let harness = Harness::new();
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_and_cut_over(&harness, &mut cli);
    // A real host runs every Hook in its own subprocess, so each Hook arrives
    // on its own connection and reopens the session first. The first Hook
    // connection bootstraps exactly like the single-connection chain.
    let mut first_hook = harness.connection(ClientKind::Hook);
    let session = open_unbound_root(
        &harness,
        &mut first_hook,
        &workspace,
        "bootstrap-reopen-external",
        "bootstrap-reopen-agent",
    );

    let prompt = user_prompt(
        &harness,
        &mut first_hook,
        &workspace,
        &session,
        "bootstrap-reopen-agent",
        "reopen-event-before-create",
    );
    assert_eq!(prompt["engaged"], true, "{prompt}");
    assert!(
        prompt.get("workItemId").is_none(),
        "an unbound session attributes no Work Item: {prompt}"
    );

    let created = create_work_item(
        &harness,
        &mut first_hook,
        &workspace,
        &session,
        "bootstrap-reopen-agent",
        None,
        "bootstrap-create-reopen",
    );
    let minted = created["data"]["workItemId"]
        .as_str()
        .unwrap_or_else(|| panic!("the create result carries its business key: {created}"))
        .to_owned();
    assert_minted_shape(&minted);

    // The next Hook is a new subprocess: a fresh connection that replays
    // `session.open` with the same external key and the same idempotency key
    // `open_unbound_root` derives from it. The replay must return the existing
    // session rather than minting or resurrecting anything.
    let mut second_hook = harness.connection(ClientKind::Hook);
    let reopened = open_unbound_root(
        &harness,
        &mut second_hook,
        &workspace,
        "bootstrap-reopen-external",
        "bootstrap-reopen-agent",
    );
    assert_eq!(
        reopened.session_id, session.session_id,
        "the replayed open returns the existing session: {reopened:?}"
    );

    // The idempotent replay must not roll the binding back: the Hook after
    // the reopen is still attributed to the minted key.
    let prompt = user_prompt(
        &harness,
        &mut second_hook,
        &workspace,
        &reopened,
        "bootstrap-reopen-agent",
        "reopen-event-after-create",
    );
    assert_eq!(
        prompt["workItemId"].as_str(),
        Some(minted.as_str()),
        "the binding survives the replayed open: {prompt}"
    );

    let mut get = session_params(&workspace, &reopened, "bootstrap-reopen-agent", json!({}));
    get.work_item_id = Some(minted.clone());
    let projection = success(&call(
        &harness.runtime,
        &mut second_hook,
        RpcMethod::ContextGet,
        get,
    ));
    assert_eq!(
        projection["projection"]["workItemId"], minted,
        "{projection}"
    );

    let mut snapshot = session_params(&workspace, &reopened, "bootstrap-reopen-agent", json!({}));
    snapshot.work_item_id = Some(minted.clone());
    let flow = success(&call(
        &harness.runtime,
        &mut second_hook,
        RpcMethod::FlowSnapshot,
        snapshot,
    ));
    assert!(
        flow["phase"].is_string(),
        "flow.snapshot projects the minted Work Item after the reopen: {flow}"
    );
}

#[test]
fn a_caller_supplied_create_name_still_round_trips() {
    let harness = Harness::new();
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_and_cut_over(&harness, &mut cli);
    let mut hook_connection = harness.connection(ClientKind::Hook);
    let session = open_unbound_root(
        &harness,
        &mut hook_connection,
        &workspace,
        "bootstrap-chosen-external",
        "bootstrap-chosen-agent",
    );

    // An explicit name must keep winning over minting, and the session must
    // bind to it exactly as it binds to a minted key.
    let created = create_work_item(
        &harness,
        &mut hook_connection,
        &workspace,
        &session,
        "bootstrap-chosen-agent",
        Some("STORY-BOOTSTRAP-CHOSEN"),
        "bootstrap-create-chosen",
    );
    assert_eq!(
        created["data"]["workItemId"], "STORY-BOOTSTRAP-CHOSEN",
        "{created}"
    );

    let prompt = user_prompt(
        &harness,
        &mut hook_connection,
        &workspace,
        &session,
        "bootstrap-chosen-agent",
        "event-after-chosen-create",
    );
    assert_eq!(prompt["workItemId"], "STORY-BOOTSTRAP-CHOSEN", "{prompt}");

    let mut snapshot = session_params(&workspace, &session, "bootstrap-chosen-agent", json!({}));
    snapshot.work_item_id = Some("STORY-BOOTSTRAP-CHOSEN".to_owned());
    let flow = success(&call(
        &harness.runtime,
        &mut hook_connection,
        RpcMethod::FlowSnapshot,
        snapshot,
    ));
    assert!(
        flow["phase"].is_string(),
        "flow.snapshot projects the named Work Item: {flow}"
    );
}

/// An ordinary typed `route.decide` whose only facts are `micro` must commit
/// the micro scale on the normal RA/CodingPlan chain. This is not the explicit
/// quick command: the committed state binds no user approval and carries no
/// quick marker, and the flow handoff still delegates requirement-analysis.
#[test]
fn ordinary_micro_facts_commit_the_micro_coding_plan_route() {
    let harness = Harness::new();
    let mut cli = harness.connection(ClientKind::Cli);
    let workspace = register_and_cut_over(&harness, &mut cli);
    let mut hook_connection = harness.connection(ClientKind::Hook);
    let session = open_unbound_root(
        &harness,
        &mut hook_connection,
        &workspace,
        "bootstrap-micro-external",
        "bootstrap-micro-agent",
    );
    let mut hook = session_params(
        &workspace,
        &session,
        "bootstrap-micro-agent",
        json!({
            "hookEventId":"bootstrap-micro-command",
            "hostPayload":{"prompt":"/ae-sdd"},
        }),
    );
    hook.idempotency_key = Some("bootstrap-micro-command".to_owned());
    hook.deadline_ms = 100;
    let prompt = success(&call(
        &harness.runtime,
        &mut hook_connection,
        RpcMethod::HookUserPrompt,
        hook,
    ));
    let work_item_id = prompt["workItemId"]
        .as_str()
        .unwrap_or_else(|| panic!("/ae-sdd must return its daemon-minted Work Item: {prompt}"))
        .to_owned();
    assert!(work_item_id.starts_with("ROUTE-"), "{prompt}");

    let mut acquire = session_params(
        &workspace,
        &session,
        "bootstrap-micro-agent",
        json!({
            "operation":"lease.acquire",
            "payload":{"owner":{"role":"root"},"ttlSeconds":300},
        }),
    );
    acquire.work_item_id = Some(work_item_id.clone());
    acquire.idempotency_key = Some("bootstrap-micro-lease".to_owned());
    let lease = success(&call(
        &harness.runtime,
        &mut hook_connection,
        RpcMethod::OperationExecute,
        acquire,
    ));

    let mut decide = session_params(
        &workspace,
        &session,
        "bootstrap-micro-agent",
        json!({
            "operation":"route.decide",
            "payload":{
                "requestedIntent":"rename one local helper and its comment",
                "impactFacts":[{"code":"impact.local_rename","level":"micro"}],
                "classificationConfidenceBps":9000
            },
        }),
    );
    decide.work_item_id = Some(work_item_id.clone());
    decide.lease_id = Some(
        lease["data"]["leaseId"]
            .as_str()
            .expect("lease id")
            .parse()
            .expect("typed lease id"),
    );
    decide.fencing_token = lease["data"]["fencingToken"].as_u64();
    decide.expected_revision = Some(0);
    decide.idempotency_key = Some("bootstrap-micro-decide".to_owned());
    let routed = success(&call(
        &harness.runtime,
        &mut hook_connection,
        RpcMethod::OperationExecute,
        decide,
    ));
    assert_eq!(routed["data"]["scale"], "micro", "{routed}");
    assert_eq!(routed["data"]["selectedDesign"], "CODING_PLAN", "{routed}");
    assert_eq!(routed["data"]["approved"], true, "{routed}");
    assert_eq!(
        routed["data"]["decision"]["designRoute"], "coding_plan",
        "{routed}"
    );
    assert_eq!(
        routed["data"]["decision"]["requiredSeries"],
        json!(["requirement-analysis", "coding-plan"]),
        "{routed}"
    );
    // An ordinary approved micro route binds no user approval: there is no
    // approval shortcut behind the classification.
    assert!(
        routed["data"]["decision"]["approvalBindingDigest"].is_null(),
        "{routed}"
    );

    // The daemon mints the state directory as `{stateUuid}-{workItemId}`, so
    // resolve it by suffix instead of inventing an identity the test cannot
    // know (the bootstrap Hook never returns `stateMachineId`).
    let state_path = fs::read_dir(harness.workspace_root.path().join(".auto-engineering"))
        .expect("state root")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(&format!("-{work_item_id}")))
        })
        .expect("committed route state directory")
        .join("state.json");
    let state: Value =
        serde_json::from_slice(&fs::read(&state_path).expect("committed route state"))
            .expect("committed route state JSON");
    assert_eq!(state["scale"], "micro", "{state}");
    assert_eq!(state["selectedDesign"], "CODING_PLAN", "{state}");
    assert_eq!(state["routeApproved"], true, "{state}");
    assert!(
        !state.to_string().contains("quick"),
        "the committed ordinary micro route carries no quick marker: {state}"
    );

    let mut next = session_params(&workspace, &session, "bootstrap-micro-agent", json!({}));
    next.work_item_id = Some(work_item_id);
    let flow = success(&call(
        &harness.runtime,
        &mut hook_connection,
        RpcMethod::FlowNext,
        next,
    ));
    assert_eq!(flow["nextAction"]["kind"], "delegate-series", "{flow}");
    assert_eq!(
        flow["nextAction"]["seriesKind"], "requirement-analysis",
        "{flow}"
    );
}
