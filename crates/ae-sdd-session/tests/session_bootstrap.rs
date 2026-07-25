//! AC-B01/AC-B02 coverage for `SessionBootstrapPort::bootstrap`.

use ae_sdd_contracts::session::SessionBootstrapRequest;
use ae_sdd_contracts::{AdapterId, ContextBundleId, ExternalSessionKey, SchemaVersion};
use ae_sdd_domain::{AgentRole, CapabilityId, DelegationId, SessionId, WorkspaceId};
use ae_sdd_session::{
    BootstrapSnapshot, BootstrapStep, ExistingSessionInfo, PureSessionBootstrap,
    SessionBootstrapError, SessionBootstrapPort,
};
use uuid::Uuid;

fn workspace(seed: u128) -> WorkspaceId {
    WorkspaceId::from_uuid(Uuid::from_u128(seed))
}

fn session(seed: u128) -> SessionId {
    SessionId::from_uuid(Uuid::from_u128(seed))
}

fn delegation(seed: u128) -> DelegationId {
    DelegationId::from_uuid(Uuid::from_u128(seed))
}

fn series_request(workspace_id: WorkspaceId) -> SessionBootstrapRequest {
    SessionBootstrapRequest::new(
        SchemaVersion::V1,
        workspace_id,
        ExternalSessionKey::new("codex-thread-42").expect("external session key"),
        AdapterId::new("codex-app-server").expect("adapter id"),
        AgentRole::Series,
        true,
        Some(delegation(2)),
        vec![CapabilityId::new("host.create").expect("capability")],
        Some(ContextBundleId::new("bundle-story-42").expect("context bundle")),
    )
    .expect("valid bootstrap request")
}

fn root_request(workspace_id: WorkspaceId) -> SessionBootstrapRequest {
    SessionBootstrapRequest::new(
        SchemaVersion::V1,
        workspace_id,
        ExternalSessionKey::new("root-thread").expect("external session key"),
        AdapterId::new("codex-app-server").expect("adapter id"),
        AgentRole::Root,
        true,
        None,
        vec![],
        None,
    )
    .expect("valid root bootstrap request")
}

#[test]
fn bool_combination_matrix_produces_expected_steps_and_deterministic_digest() {
    let workspace_id = workspace(1);
    let request = series_request(workspace_id);

    let combinations = [(false, false), (false, true), (true, false), (true, true)];

    for (workspace_registered, context_ready) in combinations {
        let snapshot = BootstrapSnapshot::new(workspace_registered, None, context_ready);

        let first = ae_sdd_session::bootstrap(&request, &snapshot).expect("plan is decided");
        let second = ae_sdd_session::bootstrap(&request, &snapshot).expect("plan is decided");

        assert_eq!(
            first.plan_digest(),
            second.plan_digest(),
            "same input must produce a byte-identical digest across calls"
        );

        let mut expected = Vec::new();
        if !workspace_registered {
            expected.push(BootstrapStep::RegisterWorkspace);
        }
        expected.push(BootstrapStep::OpenSession);
        expected.push(BootstrapStep::GrantScope);
        if !context_ready {
            expected.push(BootstrapStep::ProjectContext);
        }
        assert_eq!(first.steps(), expected.as_slice());
    }
}

#[test]
fn existing_session_in_same_workspace_skips_open_session_step() {
    let workspace_id = workspace(10);
    let request = series_request(workspace_id);
    let existing = ExistingSessionInfo::new(session(11), workspace_id);
    let snapshot = BootstrapSnapshot::new(true, Some(existing), true);

    let plan = ae_sdd_session::bootstrap(&request, &snapshot).expect("plan is decided");

    assert_eq!(plan.steps(), [BootstrapStep::GrantScope]);
}

#[test]
fn cross_workspace_existing_session_is_rejected_without_guessing() {
    let requested_workspace = workspace(20);
    let other_workspace = workspace(21);
    let request = series_request(requested_workspace);
    let existing = ExistingSessionInfo::new(session(22), other_workspace);
    let snapshot = BootstrapSnapshot::new(true, Some(existing), true);

    let outcome = ae_sdd_session::bootstrap(&request, &snapshot);

    assert_eq!(outcome, Err(SessionBootstrapError::CrossWorkspaceSession));
}

#[test]
fn digest_changes_when_request_or_snapshot_changes() {
    let workspace_id = workspace(30);
    let request_a = series_request(workspace_id);
    let request_b = root_request(workspace_id);
    let snapshot = BootstrapSnapshot::new(false, None, false);

    let plan_a = ae_sdd_session::bootstrap(&request_a, &snapshot).expect("plan a");
    let plan_b = ae_sdd_session::bootstrap(&request_b, &snapshot).expect("plan b");

    assert_ne!(plan_a.plan_digest(), plan_b.plan_digest());

    let other_snapshot = BootstrapSnapshot::new(true, None, false);
    let plan_c = ae_sdd_session::bootstrap(&request_a, &other_snapshot).expect("plan c");
    assert_ne!(plan_a.plan_digest(), plan_c.plan_digest());
}

#[test]
fn root_request_construction_rejects_delegation_via_frozen_c0_contract() {
    // AC-B02: bootstrap does not re-implement this check — it is enforced by
    // `SessionBootstrapRequest::new` before a request can even be built.
    let outcome = SessionBootstrapRequest::new(
        SchemaVersion::V1,
        workspace(40),
        ExternalSessionKey::new("root-thread").expect("external session key"),
        AdapterId::new("codex-app-server").expect("adapter id"),
        AgentRole::Root,
        true,
        Some(delegation(41)),
        vec![],
        None,
    );

    assert!(outcome.is_err(), "root + delegation must be rejected at C0");
}

#[test]
fn non_root_request_construction_requires_delegation_via_frozen_c0_contract() {
    let outcome = SessionBootstrapRequest::new(
        SchemaVersion::V1,
        workspace(42),
        ExternalSessionKey::new("child-thread").expect("external session key"),
        AdapterId::new("codex-app-server").expect("adapter id"),
        AgentRole::Task,
        true,
        None,
        vec![],
        None,
    );

    assert!(
        outcome.is_err(),
        "non-root without delegation must be rejected at C0"
    );
}

#[test]
fn port_trait_default_impl_delegates_to_pure_function() {
    let workspace_id = workspace(50);
    let request = series_request(workspace_id);
    let snapshot = BootstrapSnapshot::new(true, None, true);

    let via_port = PureSessionBootstrap.bootstrap(&request, &snapshot);
    let via_function = ae_sdd_session::bootstrap(&request, &snapshot);

    assert_eq!(via_port, via_function);
}
