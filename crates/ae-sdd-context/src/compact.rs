use ae_sdd_domain::{
    ArtifactRef, CompactId, ContextDigest, ContextGeneration, HostAckId, HostActionId, SessionId,
};
use ae_sdd_host::{HostAck, HostAckOutcome, HostAction, HostActionKind};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompactStatus {
    PressureDetected,
    SnapshotReady,
    CompactRequested,
    HostCompacting,
    HostAcknowledged,
    ContextRestored,
    Unsupported,
    TimedOut,
    Failed,
}

#[derive(Clone, Debug)]
pub struct CompactCycle {
    compact_id: CompactId,
    session_id: SessionId,
    previous_generation: ContextGeneration,
    next_generation: ContextGeneration,
    status: CompactStatus,
    deadline_unix_ms: u64,
    snapshot: Option<ArtifactRef>,
    host_action_id: Option<HostActionId>,
    host_ack_id: Option<HostAckId>,
    restored_projection_digest: Option<ContextDigest>,
}

impl CompactCycle {
    pub fn new(
        compact_id: CompactId,
        session_id: SessionId,
        previous_generation: ContextGeneration,
        next_generation: ContextGeneration,
        deadline_unix_ms: u64,
    ) -> Result<Self, CompactCycleError> {
        if previous_generation.checked_next().ok() != Some(next_generation) {
            return Err(CompactCycleError::InvalidGenerationStep);
        }
        if deadline_unix_ms == 0 {
            return Err(CompactCycleError::InvalidDeadline);
        }
        Ok(Self {
            compact_id,
            session_id,
            previous_generation,
            next_generation,
            status: CompactStatus::PressureDetected,
            deadline_unix_ms,
            snapshot: None,
            host_action_id: None,
            host_ack_id: None,
            restored_projection_digest: None,
        })
    }

    #[must_use]
    pub const fn status(&self) -> CompactStatus {
        self.status
    }

    #[must_use]
    pub const fn compact_id(&self) -> CompactId {
        self.compact_id
    }

    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    #[must_use]
    pub const fn previous_generation(&self) -> ContextGeneration {
        self.previous_generation
    }

    #[must_use]
    pub const fn next_generation(&self) -> ContextGeneration {
        self.next_generation
    }

    pub fn snapshot_ready(&mut self, snapshot: ArtifactRef) -> Result<(), CompactCycleError> {
        self.expect(CompactStatus::PressureDetected)?;
        self.snapshot = Some(snapshot);
        self.status = CompactStatus::SnapshotReady;
        Ok(())
    }

    pub fn dispatch(&mut self, action: &HostAction) -> Result<(), CompactCycleError> {
        self.expect(CompactStatus::SnapshotReady)?;
        if action.kind() != HostActionKind::Compact
            || action.compact_id() != Some(self.compact_id)
            || action.session_id() != Some(self.session_id)
            || action.context_generation() != Some(self.previous_generation)
        {
            return Err(CompactCycleError::ActionCorrelationMismatch);
        }
        self.host_action_id = Some(action.action_id());
        self.status = CompactStatus::CompactRequested;
        Ok(())
    }

    pub fn host_began_compacting(&mut self) -> Result<(), CompactCycleError> {
        self.expect(CompactStatus::CompactRequested)?;
        self.status = CompactStatus::HostCompacting;
        Ok(())
    }

    pub fn acknowledge(&mut self, ack: &HostAck) -> Result<(), CompactCycleError> {
        self.expect(CompactStatus::HostCompacting)?;
        if Some(ack.action_id()) != self.host_action_id || ack.session_id() != Some(self.session_id)
        {
            return Err(CompactCycleError::AckCorrelationMismatch);
        }
        if !matches!(ack.outcome(), HostAckOutcome::Accepted) {
            return Err(CompactCycleError::AckRejected);
        }
        self.host_ack_id = Some(ack.ack_id());
        self.status = CompactStatus::HostAcknowledged;
        Ok(())
    }

    pub fn rehydrate(
        &mut self,
        observed_generation: ContextGeneration,
        restored_projection_digest: ContextDigest,
    ) -> Result<ContextGeneration, CompactCycleError> {
        self.expect(CompactStatus::HostAcknowledged)?;
        if observed_generation != self.previous_generation {
            return Err(CompactCycleError::GenerationCasConflict);
        }
        self.restored_projection_digest = Some(restored_projection_digest);
        self.status = CompactStatus::ContextRestored;
        Ok(self.next_generation)
    }

    pub fn mark_unsupported(&mut self) -> Result<(), CompactCycleError> {
        if !matches!(
            self.status,
            CompactStatus::PressureDetected | CompactStatus::SnapshotReady
        ) {
            return Err(CompactCycleError::InvalidTerminalTransition);
        }
        self.status = CompactStatus::Unsupported;
        Ok(())
    }

    pub fn mark_timed_out(&mut self, now_unix_ms: u64) -> Result<(), CompactCycleError> {
        if now_unix_ms < self.deadline_unix_ms || is_terminal(self.status) {
            return Err(CompactCycleError::InvalidTerminalTransition);
        }
        self.status = CompactStatus::TimedOut;
        Ok(())
    }

    pub fn mark_failed(&mut self) -> Result<(), CompactCycleError> {
        if is_terminal(self.status) {
            return Err(CompactCycleError::InvalidTerminalTransition);
        }
        self.status = CompactStatus::Failed;
        Ok(())
    }

    #[must_use]
    pub const fn restored_projection_digest(&self) -> Option<ContextDigest> {
        self.restored_projection_digest
    }

    fn expect(&self, expected: CompactStatus) -> Result<(), CompactCycleError> {
        if self.status == expected {
            Ok(())
        } else {
            Err(CompactCycleError::InvalidTransition {
                from: self.status,
                expected,
            })
        }
    }
}

const fn is_terminal(status: CompactStatus) -> bool {
    matches!(
        status,
        CompactStatus::ContextRestored
            | CompactStatus::Unsupported
            | CompactStatus::TimedOut
            | CompactStatus::Failed
    )
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CompactCycleError {
    #[error("compact next generation must be previous generation plus one")]
    InvalidGenerationStep,
    #[error("compact deadline must be greater than zero")]
    InvalidDeadline,
    #[error("expected compact state {expected:?}, found {from:?}")]
    InvalidTransition {
        from: CompactStatus,
        expected: CompactStatus,
    },
    #[error("compact host action does not match cycle/session/generation")]
    ActionCorrelationMismatch,
    #[error("compact host ACK does not match action/session")]
    AckCorrelationMismatch,
    #[error("compact host ACK was rejected")]
    AckRejected,
    #[error("context generation changed before rehydrate CAS")]
    GenerationCasConflict,
    #[error("compact terminal transition is invalid")]
    InvalidTerminalTransition,
}
