use ae_sdd_contracts::compact::{
    CompactAck, CompactContractError, CompactRequest, RehydrateReceipt,
};
use ae_sdd_domain::{
    ArtifactRef, CompactId, ContextDigest, ContextGeneration, HostAckId, HostActionId, SessionId,
};
use ae_sdd_host::{HostAck, HostAckOutcome, HostAction, HostActionKind};
use thiserror::Error;

/// Maximum bytes represented by a compact capsule and optional delta.
pub const MAX_COMPACT_CAPSULE_BYTES: u64 = 64 * 1024;

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

/// Bounded content-addressed state passed to a host compact operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextCapsule {
    snapshot_ref: ArtifactRef,
    restored_projection_digest: ContextDigest,
    delta_ref: Option<ArtifactRef>,
    byte_length: u64,
}

impl ContextCapsule {
    /// Constructs a bounded capsule without reading or writing its refs.
    pub fn new(
        snapshot_ref: ArtifactRef,
        restored_projection_digest: ContextDigest,
        delta_ref: Option<ArtifactRef>,
    ) -> Result<Self, CompactCoordinatorError> {
        let byte_length = snapshot_ref
            .byte_length()
            .checked_add(delta_ref.as_ref().map_or(0, ArtifactRef::byte_length))
            .ok_or(CompactCoordinatorError::CapsuleBudgetExceeded { actual: u64::MAX })?;
        if byte_length == 0 || byte_length > MAX_COMPACT_CAPSULE_BYTES {
            return Err(CompactCoordinatorError::CapsuleBudgetExceeded {
                actual: byte_length,
            });
        }
        Ok(Self {
            snapshot_ref,
            restored_projection_digest,
            delta_ref,
            byte_length,
        })
    }

    /// Maximum capsule byte budget.
    pub const MAX_BYTES: u64 = MAX_COMPACT_CAPSULE_BYTES;

    /// Returns the durable snapshot reference.
    pub const fn snapshot_ref(&self) -> &ArtifactRef {
        &self.snapshot_ref
    }

    /// Returns the expected restored projection digest.
    pub const fn restored_projection_digest(&self) -> ContextDigest {
        self.restored_projection_digest
    }

    /// Returns an optional compact delta reference.
    pub const fn delta_ref(&self) -> Option<&ArtifactRef> {
        self.delta_ref.as_ref()
    }

    /// Returns the bounded encoded byte length.
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

/// Compact coordinator status after a request is created.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompactCoordinatorStatus {
    /// A request awaits a correlated host acknowledgement.
    Requested,
    /// The host acknowledgement was accepted.
    HostAcknowledged,
    /// Projection rehydration was accepted for the next generation.
    Rehydrated,
    /// The host does not support compact.
    Unsupported,
    /// The acknowledgement deadline elapsed.
    TimedOut,
    /// A terminal infrastructure failure occurred.
    Failed,
}

/// Pure correlation state machine for compact request/ack/rehydrate facts.
#[derive(Clone, Debug)]
pub struct CompactCoordinator {
    request: CompactRequest,
    capsule: ContextCapsule,
    status: CompactCoordinatorStatus,
    ack: Option<CompactAck>,
    receipt: Option<RehydrateReceipt>,
}

impl CompactCoordinator {
    /// Creates a coordinator whose capsule binds the request snapshot.
    pub fn new(
        request: CompactRequest,
        capsule: ContextCapsule,
    ) -> Result<Self, CompactCoordinatorError> {
        if request.snapshot_ref() != capsule.snapshot_ref() {
            return Err(CompactCoordinatorError::CapsuleRequestMismatch);
        }
        Ok(Self {
            request,
            capsule,
            status: CompactCoordinatorStatus::Requested,
            ack: None,
            receipt: None,
        })
    }

    /// Returns the current correlation state.
    pub const fn status(&self) -> CompactCoordinatorStatus {
        self.status
    }

    /// Returns the generation committed by a successful receipt.
    pub const fn committed_generation(&self) -> Option<ContextGeneration> {
        match &self.receipt {
            Some(receipt) => Some(receipt.restored_generation()),
            None => None,
        }
    }

    /// Accepts an exact, timely host ACK; exact replay is a no-op.
    pub fn acknowledge(
        &mut self,
        ack: &CompactAck,
        now_unix_ms: u64,
    ) -> Result<(), CompactCoordinatorError> {
        if self.status == CompactCoordinatorStatus::HostAcknowledged
            && self.ack.as_ref() == Some(ack)
        {
            return Ok(());
        }
        if matches!(
            self.status,
            CompactCoordinatorStatus::Rehydrated
                | CompactCoordinatorStatus::Unsupported
                | CompactCoordinatorStatus::TimedOut
                | CompactCoordinatorStatus::Failed
        ) {
            return Err(CompactCoordinatorError::TerminalState);
        }
        if now_unix_ms > self.request.deadline_unix_ms() {
            return Err(CompactCoordinatorError::DeadlineExpired);
        }
        ack.validate_for(&self.request)
            .map_err(|_| CompactCoordinatorError::CorrelationMismatch)?;
        self.ack = Some(ack.clone());
        self.status = CompactCoordinatorStatus::HostAcknowledged;
        Ok(())
    }

    /// Accepts an exact rehydration receipt after host acknowledgement.
    pub fn rehydrate(&mut self, receipt: &RehydrateReceipt) -> Result<(), CompactCoordinatorError> {
        if self.status == CompactCoordinatorStatus::Rehydrated
            && self.receipt.as_ref() == Some(receipt)
        {
            return Ok(());
        }
        if self.status != CompactCoordinatorStatus::HostAcknowledged {
            return Err(CompactCoordinatorError::InvalidTransition);
        }
        let Some(ack) = self.ack.as_ref() else {
            return Err(CompactCoordinatorError::CorrelationMismatch);
        };
        if receipt.compact_id() != self.request.compact_id()
            || receipt.session_id() != self.request.session_id()
            || receipt.previous_generation() != self.request.previous_generation()
            || receipt.restored_generation() != self.request.next_generation()
            || receipt.action_id() != ack.ack().action_id()
            || receipt.ack_id() != ack.ack().ack_id()
            || receipt.restored_projection_digest() != self.capsule.restored_projection_digest()
        {
            return Err(CompactCoordinatorError::CorrelationMismatch);
        }
        self.receipt = Some(receipt.clone());
        self.status = CompactCoordinatorStatus::Rehydrated;
        Ok(())
    }

    /// Records an explicit unsupported-host result.
    pub fn mark_unsupported(&mut self) -> Result<(), CompactCoordinatorError> {
        if self.status != CompactCoordinatorStatus::Requested {
            return Err(CompactCoordinatorError::TerminalState);
        }
        self.status = CompactCoordinatorStatus::Unsupported;
        Ok(())
    }

    /// Records a deadline timeout without fabricating success.
    pub fn mark_timed_out(&mut self, now_unix_ms: u64) -> Result<(), CompactCoordinatorError> {
        if self.status != CompactCoordinatorStatus::Requested
            || now_unix_ms < self.request.deadline_unix_ms()
        {
            return Err(CompactCoordinatorError::TerminalState);
        }
        self.status = CompactCoordinatorStatus::TimedOut;
        Ok(())
    }
}

/// Stable compact-coordinator validation failures.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CompactCoordinatorError {
    /// Capsule bytes exceeded the bounded budget.
    #[error("compact capsule is {actual} bytes and exceeds the bounded budget")]
    CapsuleBudgetExceeded { actual: u64 },
    /// Request and capsule did not bind the same snapshot.
    #[error("compact capsule does not match the request snapshot")]
    CapsuleRequestMismatch,
    /// Correlation fields did not match the request/ack/receipt identity.
    #[error("compact correlation fields do not match")]
    CorrelationMismatch,
    /// The host acknowledgement arrived after its deadline.
    #[error("compact acknowledgement deadline expired")]
    DeadlineExpired,
    /// A terminal coordinator state cannot receive another fact.
    #[error("compact coordinator is already terminal")]
    TerminalState,
    /// Rehydration was attempted before accepted host acknowledgement.
    #[error("compact rehydration transition is invalid")]
    InvalidTransition,
    /// Frozen contract validation failed.
    #[error(transparent)]
    Contract(#[from] CompactContractError),
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
