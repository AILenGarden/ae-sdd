//! `operation.execute` admission preconditions come from the operation
//! registry, not from the method name.
//!
//! constraints/api.md freezes `requiresWorkspace`/`requiresWorkItem` per
//! operation, and `workitem.create` is workspace-scoped because the Work Item
//! is its output, not its input. Applying the method-level blanket
//! (`requiresWorkItem: true`) to every `operation.execute` deadlocks
//! bootstrap: no session can ever create the first Work Item. The gate must
//! therefore resolve the selected operation's `OperationSpec`, while an
//! unresolvable operation name keeps the fail-closed blanket so the dispatch
//! stays the single authority on `OPERATION_NOT_REGISTERED`.

mod support;

use std::sync::atomic::Ordering;

use ae_sdd_protocol::{ClientKind, ConfirmationRef, RequestParams, RpcMethod, WorkspaceMode};
use ae_sdd_runtime::{RuntimeConfig, SessionResult, WorkspaceResult};
use serde_json::{Value, json};

use support::{
    Harness, open_root_session, params, parity_transition_payload, register_workspace,
    session_params, stable_error,
};

/// Builds an `operation.execute` request the way a bootstrapping agent does:
/// bound to a session and a workspace, but with no Work Item yet.
fn operation_params(
    workspace: &WorkspaceResult,
    session: &SessionResult,
    operation: &str,
    payload: Value,
) -> RequestParams<Value> {
    session_params(
        workspace,
        session,
        "agent-a",
        json!({"operation": operation, "payload": payload}),
        1_000,
    )
}

#[test]
fn a_workspace_scoped_operation_needs_no_work_item_id() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut connection, "op-gate-create");
    // A write operation is only admitted once the daemon owns the writer mode;
    // the gate regression this test pins sits before that guard, so the
    // workspace transitions first and the session opens against the canary.
    let canary = transition_to_canary(&harness, &workspace);
    let session = open_root_session(
        &harness,
        &mut connection,
        &canary,
        "agent-a",
        "external-a",
        None,
    );

    let mut request = operation_params(
        &canary,
        &session,
        "workitem.create",
        json!({"entryNode": "STORY"}),
    );
    // The registry still freezes `requiresIdempotency` for creation, so the
    // bootstrap caller owes a key even though it owes no Work Item.
    request.idempotency_key = Some("create-1".to_owned());

    let response = harness.call(&mut connection, RpcMethod::OperationExecute, request);

    assert!(
        response.get("result").is_some(),
        "workitem.create must pass the requirements gate without a workItemId: {response}"
    );
    assert_eq!(
        harness.business.operation_calls.load(Ordering::Acquire),
        1,
        "the operation must reach dispatch, not die at the gate"
    );
}

/// Moves a workspace into the daemon-owned writer mode the way the cutover
/// tests do: quiesce, then a mode transition backed by parity evidence.
fn transition_to_canary(harness: &Harness, workspace: &WorkspaceResult) -> WorkspaceResult {
    let mut admin = harness.connection(ClientKind::Admin);
    let mut drain = params(json!({"stop":false}), 1_000);
    drain.idempotency_key = Some(format!("drain-{}", workspace.workspace_id));
    drain.confirmation = Some(ConfirmationRef {
        confirmation_id: "confirmation".to_owned(),
        approved_by: "admin".to_owned(),
        approved_at: "2026-01-01T00:00:00Z".to_owned(),
    });
    assert!(
        harness
            .call(&mut admin, RpcMethod::RuntimeDrain, drain)
            .get("result")
            .is_some()
    );
    let mut transition = params(
        parity_transition_payload(WorkspaceMode::RustCanary, 1_000),
        1_000,
    );
    transition.workspace_id = Some(workspace.workspace_id.clone());
    transition.idempotency_key = Some(format!("mode-{}", workspace.workspace_id));
    transition.confirmation = Some(ConfirmationRef {
        confirmation_id: "confirmation".to_owned(),
        approved_by: "admin".to_owned(),
        approved_at: "2026-01-01T00:00:00Z".to_owned(),
    });
    serde_json::from_value(
        harness
            .call(&mut admin, RpcMethod::WorkspaceModeTransition, transition)
            .get("result")
            .cloned()
            .expect("canary transition succeeds"),
    )
    .expect("canary workspace decodes")
}

#[test]
fn a_confirmation_requiring_operation_reaches_dispatch_without_confirmation() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut connection, "op-gate-confirm");
    let canary = transition_to_canary(&harness, &workspace);
    let session = open_root_session(
        &harness,
        &mut connection,
        &canary,
        "agent-a",
        "external-a",
        None,
    );

    // `state.transition` freezes every write precondition including
    // confirmation. Confirmation is deliberately NOT a transport admission
    // precondition: the business authority answers a missing confirmation with
    // a remediated challenge whose `error.data.remediation` carries the
    // confirmation binding, and rejecting here with a bare
    // CONFIRMATION_REQUIRED would make that binding unreachable. With every
    // other precondition satisfied, the request must reach dispatch.
    let mut request = operation_params(
        &canary,
        &session,
        "state.transition",
        json!({"target": "DR"}),
    );
    request.work_item_id = Some("work-item-1".to_owned());
    request.lease_id = Some("lease-1".to_owned());
    request.fencing_token = Some(1);
    request.expected_revision = Some(0);
    request.idempotency_key = Some("transition-1".to_owned());

    let response = harness.call(&mut connection, RpcMethod::OperationExecute, request);

    assert_eq!(
        harness.business.operation_calls.load(Ordering::Acquire),
        1,
        "a confirmation-requiring operation with every other field present must reach dispatch, not die at admission: {response}"
    );
}

#[test]
fn a_work_item_scoped_operation_still_requires_work_item_id() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut connection, "op-gate-get");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-a",
        "external-a",
        None,
    );

    let rejected = harness.call(
        &mut connection,
        RpcMethod::OperationExecute,
        operation_params(&workspace, &session, "workitem.get", json!({})),
    );

    assert_eq!(stable_error(&rejected), "OPERATION_SCHEMA_INVALID");
    assert_eq!(
        harness.business.operation_calls.load(Ordering::Acquire),
        0,
        "a rejected request must not reach dispatch"
    );
}

#[test]
fn an_unresolvable_operation_name_fails_closed_as_before() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);
    let workspace = register_workspace(&harness, &mut connection, "op-gate-unknown");
    let session = open_root_session(
        &harness,
        &mut connection,
        &workspace,
        "agent-a",
        "external-a",
        None,
    );

    // Without a spec the gate cannot know the preconditions, so it keeps the
    // blanket requirements: rejecting here exactly as today leaves
    // `OPERATION_NOT_REGISTERED` to the dispatch instead of duplicating it.
    let rejected = harness.call(
        &mut connection,
        RpcMethod::OperationExecute,
        operation_params(&workspace, &session, "not.registered", json!({})),
    );

    assert_eq!(stable_error(&rejected), "OPERATION_SCHEMA_INVALID");
    assert_eq!(harness.business.operation_calls.load(Ordering::Acquire), 0);
}

#[test]
fn method_level_requirements_for_other_methods_are_untouched() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut connection = harness.connection(ClientKind::Cli);

    // WorkspaceRegister is a direct method with `requiresIdempotency`; nothing
    // about the registry-driven resolution may leak into it.
    let mut register = params(
        json!({
            "projectRoot": "C:/ae-sdd-tests/op-gate-method",
            "projectKey": "project-op-gate-method",
        }),
        1_000,
    );
    register.idempotency_key = None;
    let rejected = harness.call(&mut connection, RpcMethod::WorkspaceRegister, register);
    assert_eq!(stable_error(&rejected), "OPERATION_SCHEMA_INVALID");

    // SessionOpen is a direct method with `requiresWorkspace`.
    let mut open = params(
        json!({"externalKey": "external-a", "role": "root", "engaged": false}),
        1_000,
    );
    open.idempotency_key = Some("session-open-a".to_owned());
    let rejected = harness.call(&mut connection, RpcMethod::SessionOpen, open);
    assert_eq!(stable_error(&rejected), "OPERATION_SCHEMA_INVALID");
}
