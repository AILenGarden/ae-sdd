//! AC-B12 coverage for `build_session_bootstrap_request`: legal/illegal
//! combinations of every `SessionBootstrapRequest::new` required field,
//! proving the pure composition function neither adds nor loosens the
//! frozen `SessionContractError` validation (root/delegation invariant,
//! capability bound and uniqueness).

#[allow(dead_code, unused_imports)]
#[path = "../src/bootstrap.rs"]
mod bootstrap;

use std::str::FromStr;

use ae_sdd_contracts::session::SessionContractError;
use ae_sdd_contracts::{AdapterId, ContextBundleId, ExternalSessionKey};
use ae_sdd_domain::{AgentRole, CapabilityId, DelegationId, WorkspaceId};
use bootstrap::build_session_bootstrap_request;

/// Fixed identities keep these assertions deterministic and keep the CLI's
/// dependency surface unchanged (no `uuid` dev-dependency needed just to
/// mint throwaway ids).
fn workspace_id() -> WorkspaceId {
    WorkspaceId::from_str("11111111-1111-4111-8111-111111111111").expect("workspace id")
}

fn delegation_id() -> DelegationId {
    DelegationId::from_str("22222222-2222-4222-8222-222222222222").expect("delegation id")
}

fn external_session_key() -> ExternalSessionKey {
    ExternalSessionKey::new("host-conversation-1").expect("external session key")
}

fn adapter_id() -> AdapterId {
    AdapterId::new("adapter-1").expect("adapter id")
}

fn capability(value: &str) -> CapabilityId {
    CapabilityId::new(value).expect("capability id")
}

#[test]
fn root_role_without_delegation_and_no_capabilities_builds_a_valid_request() {
    let workspace_id = workspace_id();
    let request = build_session_bootstrap_request(
        workspace_id,
        external_session_key(),
        adapter_id(),
        AgentRole::Root,
        true,
        None,
        Vec::new(),
        None,
    )
    .expect("root without delegation and no capabilities is a legal combination");

    assert_eq!(request.workspace_id(), workspace_id);
    assert_eq!(request.role(), AgentRole::Root);
    assert!(request.engaged());
    assert_eq!(request.delegation_id(), None);
    assert!(request.capabilities().is_empty());
    assert_eq!(request.context_bundle_id(), None);
}

#[test]
fn non_root_role_with_delegation_and_capabilities_builds_a_valid_request() {
    let delegation_id = delegation_id();
    let context_bundle_id = ContextBundleId::new("bundle-1").expect("context bundle id");
    let request = build_session_bootstrap_request(
        workspace_id(),
        external_session_key(),
        adapter_id(),
        AgentRole::Task,
        false,
        Some(delegation_id),
        vec![capability("write"), capability("read")],
        Some(context_bundle_id.clone()),
    )
    .expect("non-root with delegation and capabilities is a legal combination");

    assert_eq!(request.role(), AgentRole::Task);
    assert!(!request.engaged());
    assert_eq!(request.delegation_id(), Some(delegation_id));
    // `SessionBootstrapRequest::new` sorts capabilities; assert the
    // deterministic sorted order rather than input order.
    assert_eq!(
        request.capabilities(),
        &[capability("read"), capability("write")]
    );
    assert_eq!(request.context_bundle_id(), Some(&context_bundle_id));
}

#[test]
fn root_role_with_a_delegation_is_rejected() {
    let outcome = build_session_bootstrap_request(
        workspace_id(),
        external_session_key(),
        adapter_id(),
        AgentRole::Root,
        true,
        Some(delegation_id()),
        Vec::new(),
        None,
    );

    assert_eq!(outcome, Err(SessionContractError::RootDelegationForbidden));
}

#[test]
fn non_root_role_without_a_delegation_is_rejected() {
    for role in [AgentRole::Series, AgentRole::Task, AgentRole::Reviewer] {
        let outcome = build_session_bootstrap_request(
            workspace_id(),
            external_session_key(),
            adapter_id(),
            role,
            true,
            None,
            Vec::new(),
            None,
        );

        assert_eq!(
            outcome,
            Err(SessionContractError::DelegationRequired),
            "role {role:?} without a delegation must be rejected"
        );
    }
}

#[test]
fn capability_count_over_the_bound_is_rejected() {
    let capabilities = (0..=ae_sdd_contracts::session::MAX_SESSION_CAPABILITIES)
        .map(|index| capability(&format!("capability-{index}")))
        .collect::<Vec<_>>();
    let expected_actual = capabilities.len();

    let outcome = build_session_bootstrap_request(
        workspace_id(),
        external_session_key(),
        adapter_id(),
        AgentRole::Root,
        true,
        None,
        capabilities,
        None,
    );

    assert_eq!(
        outcome,
        Err(SessionContractError::TooManyCapabilities {
            maximum: ae_sdd_contracts::session::MAX_SESSION_CAPABILITIES,
            actual: expected_actual,
        })
    );
}

#[test]
fn duplicate_capabilities_are_rejected() {
    let outcome = build_session_bootstrap_request(
        workspace_id(),
        external_session_key(),
        adapter_id(),
        AgentRole::Root,
        true,
        None,
        vec![capability("write"), capability("write")],
        None,
    );

    assert_eq!(outcome, Err(SessionContractError::DuplicateCapability));
}

#[test]
fn schema_version_is_fixed_to_v1_and_not_a_caller_input() {
    let request = build_session_bootstrap_request(
        workspace_id(),
        external_session_key(),
        adapter_id(),
        AgentRole::Root,
        true,
        None,
        Vec::new(),
        None,
    )
    .expect("valid root request");

    assert_eq!(
        request.schema_version(),
        ae_sdd_contracts::SchemaVersion::V1
    );
}
