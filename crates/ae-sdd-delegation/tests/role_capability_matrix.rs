mod support;

use ae_sdd_delegation::{DelegationError, DelegationRequest};
use ae_sdd_domain::{
    AgentLineage, AgentRole, CapabilityId, DeliverableContract, OperationId, ProjectPathScope,
    ScopedGrant, StateRevision,
};

use support::{delegation, grant, session};

#[test]
fn root_can_create_only_series_and_child_grants_only_narrow() {
    let parent = grant();
    let direct_task = DelegationRequest::new(
        delegation(30),
        AgentLineage::root(session(1)),
        AgentRole::Task,
        &parent,
        grant(),
        DeliverableContract::bounded_default([]).expect("valid contract"),
        StateRevision::new(1),
        ae_sdd_domain::InputFingerprint::digest(b"input"),
        100,
    );
    assert!(matches!(
        direct_task,
        Err(DelegationError::RoleCannotDelegate)
    ));

    let expanded = ScopedGrant::new(
        [
            OperationId::new("state.transition").expect("valid operation"),
            OperationId::new("lease.break").expect("valid operation"),
        ],
        [CapabilityId::new("host.create").expect("valid capability")],
        [ProjectPathScope::ProjectRoot],
    );
    let request = DelegationRequest::new(
        delegation(31),
        AgentLineage::root(session(1)),
        AgentRole::Series,
        &parent,
        expanded,
        DeliverableContract::bounded_default([]).expect("valid contract"),
        StateRevision::new(1),
        ae_sdd_domain::InputFingerprint::digest(b"input"),
        100,
    );
    assert!(matches!(request, Err(DelegationError::GrantExpansion)));
}
