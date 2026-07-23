#![allow(dead_code)]

use ae_sdd_delegation::{Delegation, DelegationRequest};
use ae_sdd_domain::{
    AgentLineage, AgentRole, CapabilityId, DelegationId, DeliverableContract, HostAckId,
    HostActionId, InputFingerprint, OperationId, ProjectPathScope, ScopedGrant, SessionId,
    StateRevision,
};
use ae_sdd_host::{
    ChildClaim, HostAck, HostAckOutcome, HostAction, HostActionKind, HostAdapterId, HostTaskId,
    PhysicalSessionProof,
};
use uuid::Uuid;

pub fn session(seed: u128) -> SessionId {
    SessionId::from_uuid(Uuid::from_u128(seed))
}

pub fn delegation(seed: u128) -> DelegationId {
    DelegationId::from_uuid(Uuid::from_u128(seed))
}

pub fn grant() -> ScopedGrant {
    ScopedGrant::new(
        [OperationId::new("state.transition").expect("valid operation")],
        [CapabilityId::new("host.create").expect("valid capability")],
        [ProjectPathScope::ProjectRoot],
    )
}

pub fn requested_delegation() -> Delegation {
    let parent_grant = grant();
    Delegation::new(
        DelegationRequest::new(
            delegation(10),
            AgentLineage::root(session(1)),
            AgentRole::Series,
            &parent_grant,
            grant(),
            DeliverableContract::bounded_default([]).expect("valid contract"),
            StateRevision::new(7),
            InputFingerprint::digest(b"assignment"),
            2_000,
        )
        .expect("valid delegation request"),
    )
}

pub fn create_action() -> HostAction {
    HostAction::new(
        HostActionId::from_uuid(Uuid::from_u128(20)),
        HostAdapterId::new("codex").expect("valid adapter"),
        1,
        HostActionKind::Create,
        Some(delegation(10)),
        None,
        None,
        None,
        2_000,
        [9; 32],
    )
    .expect("valid create action")
}

pub fn physical_proof(action: &HostAction) -> PhysicalSessionProof {
    let ack = HostAck::new(
        HostAckId::from_uuid(Uuid::from_u128(21)),
        action.action_id(),
        action.adapter_id().clone(),
        action.command_seq(),
        HostAckOutcome::Accepted,
        Some(HostTaskId::new("physical-task-1").expect("valid host task")),
        Some(session(2)),
    )
    .expect("valid ACK");
    let claim = ChildClaim::new(
        ae_sdd_domain::ClaimId::from_uuid(Uuid::from_u128(22)),
        delegation(10),
        action.action_id(),
        session(2),
        1_900,
    )
    .expect("valid claim");
    PhysicalSessionProof::establish(action, &ack, &claim, 1_500).expect("physical proof")
}
