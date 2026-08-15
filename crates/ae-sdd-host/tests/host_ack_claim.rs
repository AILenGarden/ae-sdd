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

/// A claim minted for a different delegation must never establish proof for
/// this one, even when every other field (action, ACK, session) matches
/// exactly (ROUTE-702d576a Task 2 Admission RED: wrong-delegation case).
#[test]
fn a_claim_for_a_different_delegation_is_rejected() {
    let delegation = DelegationId::from_uuid(Uuid::from_u128(1));
    let other_delegation = DelegationId::from_uuid(Uuid::from_u128(11));
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
    // Correct session, correct action -- only the delegation identity is
    // wrong, isolating this from the session-mismatch case above.
    let wrong_delegation_claim = ChildClaim::new(
        ClaimId::from_uuid(Uuid::from_u128(6)),
        other_delegation,
        action_id,
        expected_session,
        1_900,
    )
    .expect("well-formed but wrong-delegation claim");

    assert!(matches!(
        PhysicalSessionProof::establish(&action, &ack, &wrong_delegation_claim, 1_500),
        Err(AttestationError::DelegationMismatch)
    ));
}

/// A claim past its own expiry must never establish proof, even when action,
/// ACK, session, and delegation all match exactly (ROUTE-702d576a Task 2
/// Admission RED: expired-claim case).
#[test]
fn a_claim_past_its_expiry_is_rejected() {
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
    let claim = ChildClaim::new(
        ClaimId::from_uuid(Uuid::from_u128(7)),
        delegation,
        action_id,
        expected_session,
        1_900,
    )
    .expect("well-formed claim expiring at 1_900");

    // `now_unix_ms` at exactly the expiry boundary must also reject: a claim
    // is valid only strictly before its own expiry, never at or after it.
    assert!(matches!(
        PhysicalSessionProof::establish(&action, &ack, &claim, 1_900),
        Err(AttestationError::ClaimExpired)
    ));
    assert!(matches!(
        PhysicalSessionProof::establish(&action, &ack, &claim, 5_000),
        Err(AttestationError::ClaimExpired)
    ));
}
