use ae_sdd_domain::{ClaimId, DelegationId, HostAckId, HostActionId, SessionId};
use ae_sdd_host::{
    AttestationError, ChildClaim, HostAck, HostAckOutcome, HostAction, HostActionKind,
    HostAdapterId, HostTaskId, PhysicalSessionProof,
};
use uuid::Uuid;

#[test]
fn ack_is_command_receipt_not_physical_child_proof() {
    let delegation = DelegationId::from_uuid(Uuid::from_u128(1));
    let action_id = HostActionId::from_uuid(Uuid::from_u128(2));
    let expected_session = SessionId::from_uuid(Uuid::from_u128(3));
    let adapter = HostAdapterId::new("codex").expect("valid adapter");
    let action = HostAction::new(
        action_id,
        adapter.clone(),
        1,
        HostActionKind::Create,
        Some(delegation),
        None,
        None,
        None,
        2_000,
        [1; 32],
    )
    .expect("valid action");
    let ack = HostAck::new(
        HostAckId::from_uuid(Uuid::from_u128(4)),
        action_id,
        adapter,
        1,
        HostAckOutcome::Accepted,
        Some(HostTaskId::new("host-task").expect("valid task")),
        Some(expected_session),
    )
    .expect("valid ACK");
    let forged_claim = ChildClaim::new(
        ClaimId::from_uuid(Uuid::from_u128(5)),
        delegation,
        action_id,
        SessionId::from_uuid(Uuid::from_u128(99)),
        1_900,
    )
    .expect("well-formed but mismatched claim");

    assert!(matches!(
        PhysicalSessionProof::establish(&action, &ack, &forged_claim, 1_500),
        Err(AttestationError::SessionMismatch)
    ));
}
