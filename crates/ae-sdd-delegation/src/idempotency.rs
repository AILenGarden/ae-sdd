use ae_sdd_domain::{DelegationId, ResultDigest, SessionId, WorkspaceId};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DelegationCreateReceipt {
    workspace_id: WorkspaceId,
    parent_session_id: SessionId,
    idempotency_key: Box<str>,
    request_digest: ResultDigest,
    delegation_id: DelegationId,
    response_digest: ResultDigest,
}

impl DelegationCreateReceipt {
    pub fn new(
        workspace_id: WorkspaceId,
        parent_session_id: SessionId,
        idempotency_key: impl Into<Box<str>>,
        request_digest: ResultDigest,
        delegation_id: DelegationId,
        response_digest: ResultDigest,
    ) -> Result<Self, DelegationIdempotencyError> {
        let idempotency_key = idempotency_key.into();
        if idempotency_key.is_empty() || idempotency_key.len() > 256 {
            return Err(DelegationIdempotencyError::InvalidKey);
        }
        Ok(Self {
            workspace_id,
            parent_session_id,
            idempotency_key,
            request_digest,
            delegation_id,
            response_digest,
        })
    }

    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub const fn parent_session_id(&self) -> SessionId {
        self.parent_session_id
    }

    #[must_use]
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    #[must_use]
    pub const fn delegation_id(&self) -> DelegationId {
        self.delegation_id
    }

    #[must_use]
    pub const fn response_digest(&self) -> ResultDigest {
        self.response_digest
    }

    pub fn replay(
        &self,
        workspace_id: WorkspaceId,
        parent_session_id: SessionId,
        idempotency_key: &str,
        request_digest: ResultDigest,
    ) -> Result<DelegationReplayDecision, DelegationIdempotencyError> {
        if self.workspace_id != workspace_id
            || self.parent_session_id != parent_session_id
            || self.idempotency_key() != idempotency_key
        {
            return Ok(DelegationReplayDecision::NewRequest);
        }
        if self.request_digest != request_digest {
            return Err(DelegationIdempotencyError::KeyReused);
        }
        Ok(DelegationReplayDecision::Replay {
            delegation_id: self.delegation_id,
            response_digest: self.response_digest,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelegationReplayDecision {
    NewRequest,
    Replay {
        delegation_id: DelegationId,
        response_digest: ResultDigest,
    },
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DelegationIdempotencyError {
    #[error("delegation idempotency key must be in 1..=256 bytes")]
    InvalidKey,
    #[error("delegation idempotency key was reused with a different request")]
    KeyReused,
}
