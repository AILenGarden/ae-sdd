use ae_sdd_domain::{ClaimId, DelegationId, HostAckId, HostActionId, SessionId};
use thiserror::Error;

use crate::{HostAck, HostAckOutcome, HostAction, HostActionKind, HostAdapterId, HostTaskId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildClaim {
    claim_id: ClaimId,
    delegation_id: DelegationId,
    action_id: HostActionId,
    child_session_id: SessionId,
    expires_at_unix_ms: u64,
}

impl ChildClaim {
    pub fn new(
        claim_id: ClaimId,
        delegation_id: DelegationId,
        action_id: HostActionId,
        child_session_id: SessionId,
        expires_at_unix_ms: u64,
    ) -> Result<Self, AttestationError> {
        if expires_at_unix_ms == 0 {
            return Err(AttestationError::InvalidClaimExpiry);
        }
        Ok(Self {
            claim_id,
            delegation_id,
            action_id,
            child_session_id,
            expires_at_unix_ms,
        })
    }

    #[must_use]
    pub const fn claim_id(&self) -> ClaimId {
        self.claim_id
    }

    #[must_use]
    pub const fn delegation_id(&self) -> DelegationId {
        self.delegation_id
    }

    #[must_use]
    pub const fn child_session_id(&self) -> SessionId {
        self.child_session_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicalSessionProof {
    delegation_id: DelegationId,
    child_session_id: SessionId,
    action_id: HostActionId,
    ack_id: HostAckId,
    adapter_id: HostAdapterId,
    host_task_id: HostTaskId,
    claim_id: ClaimId,
}

impl PhysicalSessionProof {
    pub fn establish(
        action: &HostAction,
        ack: &HostAck,
        claim: &ChildClaim,
        now_unix_ms: u64,
    ) -> Result<Self, AttestationError> {
        if action.kind() != HostActionKind::Create {
            return Err(AttestationError::NotCreateAction);
        }
        ack.validate_for(action)
            .map_err(|_| AttestationError::AckCorrelationMismatch)?;
        if !matches!(ack.outcome(), HostAckOutcome::Accepted) {
            return Err(AttestationError::AckNotAccepted);
        }
        let delegation_id = action
            .delegation_id()
            .ok_or(AttestationError::DelegationMismatch)?;
        if claim.delegation_id != delegation_id || claim.action_id != action.action_id() {
            return Err(AttestationError::DelegationMismatch);
        }
        if now_unix_ms >= claim.expires_at_unix_ms {
            return Err(AttestationError::ClaimExpired);
        }
        if ack.session_id() != Some(claim.child_session_id) {
            return Err(AttestationError::SessionMismatch);
        }
        let host_task_id = ack
            .host_task_id()
            .cloned()
            .ok_or(AttestationError::HostTaskMissing)?;

        Ok(Self {
            delegation_id,
            child_session_id: claim.child_session_id,
            action_id: action.action_id(),
            ack_id: ack.ack_id(),
            adapter_id: action.adapter_id().clone(),
            host_task_id,
            claim_id: claim.claim_id,
        })
    }

    #[must_use]
    pub const fn delegation_id(&self) -> DelegationId {
        self.delegation_id
    }

    #[must_use]
    pub const fn child_session_id(&self) -> SessionId {
        self.child_session_id
    }

    #[must_use]
    pub fn adapter_id(&self) -> &HostAdapterId {
        &self.adapter_id
    }

    #[must_use]
    pub const fn action_id(&self) -> HostActionId {
        self.action_id
    }

    #[must_use]
    pub const fn ack_id(&self) -> HostAckId {
        self.ack_id
    }

    #[must_use]
    pub const fn claim_id(&self) -> ClaimId {
        self.claim_id
    }

    #[must_use]
    pub const fn host_task_id(&self) -> &HostTaskId {
        &self.host_task_id
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AttestationError {
    #[error("claim expiry must be greater than zero")]
    InvalidClaimExpiry,
    #[error("physical session proof requires a create action")]
    NotCreateAction,
    #[error("host ACK does not correlate to the create action")]
    AckCorrelationMismatch,
    #[error("host ACK was not accepted")]
    AckNotAccepted,
    #[error("claim delegation/action does not match the create action")]
    DelegationMismatch,
    #[error("child claim has expired")]
    ClaimExpired,
    #[error("host ACK session does not match child claim")]
    SessionMismatch,
    #[error("accepted create ACK does not include a host task identity")]
    HostTaskMissing,
}

#[cfg(test)]
mod tests {
    use ae_sdd_domain::{ContextGeneration, HostAckId};
    use uuid::Uuid;

    use super::*;

    fn ids() -> (DelegationId, HostActionId, SessionId) {
        (
            DelegationId::from_uuid(Uuid::from_u128(1)),
            HostActionId::from_uuid(Uuid::from_u128(2)),
            SessionId::from_uuid(Uuid::from_u128(3)),
        )
    }

    #[test]
    fn accepted_ack_without_child_claim_is_not_a_physical_proof() {
        let (delegation_id, action_id, session_id) = ids();
        let adapter = HostAdapterId::new("codex").expect("valid adapter");
        let action = HostAction::new(
            action_id,
            adapter.clone(),
            1,
            HostActionKind::Create,
            Some(delegation_id),
            None,
            None,
            None::<ContextGeneration>,
            2_000,
            [7; 32],
        )
        .expect("valid create action");
        let ack = HostAck::new(
            HostAckId::from_uuid(Uuid::from_u128(4)),
            action_id,
            adapter,
            1,
            HostAckOutcome::Accepted,
            Some(HostTaskId::new("task-42").expect("valid task")),
            Some(session_id),
        )
        .expect("valid ack");
        let wrong_claim = ChildClaim::new(
            ClaimId::from_uuid(Uuid::from_u128(5)),
            delegation_id,
            action_id,
            SessionId::from_uuid(Uuid::from_u128(99)),
            1_900,
        )
        .expect("valid claim");

        assert!(matches!(
            PhysicalSessionProof::establish(&action, &ack, &wrong_claim, 1_500),
            Err(AttestationError::SessionMismatch)
        ));
    }

    #[test]
    fn matching_ack_and_live_child_claim_form_physical_proof() {
        let (delegation_id, action_id, session_id) = ids();
        let adapter = HostAdapterId::new("codex").expect("valid adapter");
        let action = HostAction::new(
            action_id,
            adapter.clone(),
            1,
            HostActionKind::Create,
            Some(delegation_id),
            None,
            None,
            None,
            2_000,
            [7; 32],
        )
        .expect("valid create action");
        let ack = HostAck::new(
            HostAckId::from_uuid(Uuid::from_u128(4)),
            action_id,
            adapter,
            1,
            HostAckOutcome::Accepted,
            Some(HostTaskId::new("task-42").expect("valid task")),
            Some(session_id),
        )
        .expect("valid ack");
        let claim = ChildClaim::new(
            ClaimId::from_uuid(Uuid::from_u128(5)),
            delegation_id,
            action_id,
            session_id,
            1_900,
        )
        .expect("valid claim");

        let proof = PhysicalSessionProof::establish(&action, &ack, &claim, 1_500)
            .expect("attested session");
        assert_eq!(proof.child_session_id(), session_id);
        assert_eq!(proof.delegation_id(), delegation_id);
    }
}
