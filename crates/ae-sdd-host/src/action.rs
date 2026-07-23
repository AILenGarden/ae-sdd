use std::collections::BTreeSet;
use std::fmt;

use ae_sdd_domain::{
    CompactId, ContextGeneration, DelegationId, HostAckId, HostActionId, SessionId,
};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostAdapterId(Box<str>);

impl HostAdapterId {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, HostActionError> {
        let value = value.into();
        validate_opaque_id("adapter", &value)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HostAdapterId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostTaskId(Box<str>);

impl HostTaskId {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, HostActionError> {
        let value = value.into();
        validate_opaque_id("host task", &value)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HostCapability {
    Create,
    Send,
    Wait,
    Cancel,
    Attest,
    Compact,
    PressureTelemetry,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostCapabilitySet(BTreeSet<HostCapability>);

impl HostCapabilitySet {
    #[must_use]
    pub fn new(values: impl IntoIterator<Item = HostCapability>) -> Self {
        Self(values.into_iter().collect())
    }

    #[must_use]
    pub fn supports(&self, capability: HostCapability) -> bool {
        self.0.contains(&capability)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostActionKind {
    Create,
    Send,
    Wait,
    Cancel,
    Attest,
    Compact,
}

impl HostActionKind {
    #[must_use]
    pub const fn required_capability(self) -> HostCapability {
        match self {
            Self::Create => HostCapability::Create,
            Self::Send => HostCapability::Send,
            Self::Wait => HostCapability::Wait,
            Self::Cancel => HostCapability::Cancel,
            Self::Attest => HostCapability::Attest,
            Self::Compact => HostCapability::Compact,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostAction {
    action_id: HostActionId,
    adapter_id: HostAdapterId,
    command_seq: u64,
    kind: HostActionKind,
    delegation_id: Option<DelegationId>,
    compact_id: Option<CompactId>,
    session_id: Option<SessionId>,
    context_generation: Option<ContextGeneration>,
    deadline_unix_ms: u64,
    request_digest: [u8; 32],
}

impl HostAction {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        action_id: HostActionId,
        adapter_id: HostAdapterId,
        command_seq: u64,
        kind: HostActionKind,
        delegation_id: Option<DelegationId>,
        compact_id: Option<CompactId>,
        session_id: Option<SessionId>,
        context_generation: Option<ContextGeneration>,
        deadline_unix_ms: u64,
        request_digest: [u8; 32],
    ) -> Result<Self, HostActionError> {
        if command_seq == 0 {
            return Err(HostActionError::ZeroCommandSequence);
        }
        if deadline_unix_ms == 0 {
            return Err(HostActionError::ZeroDeadline);
        }
        match kind {
            HostActionKind::Create if delegation_id.is_none() => {
                return Err(HostActionError::DelegationBindingRequired);
            }
            HostActionKind::Compact
                if compact_id.is_none() || session_id.is_none() || context_generation.is_none() =>
            {
                return Err(HostActionError::CompactBindingRequired);
            }
            _ => {}
        }
        Ok(Self {
            action_id,
            adapter_id,
            command_seq,
            kind,
            delegation_id,
            compact_id,
            session_id,
            context_generation,
            deadline_unix_ms,
            request_digest,
        })
    }

    #[must_use]
    pub const fn action_id(&self) -> HostActionId {
        self.action_id
    }

    #[must_use]
    pub const fn kind(&self) -> HostActionKind {
        self.kind
    }

    #[must_use]
    pub fn adapter_id(&self) -> &HostAdapterId {
        &self.adapter_id
    }

    #[must_use]
    pub const fn delegation_id(&self) -> Option<DelegationId> {
        self.delegation_id
    }

    #[must_use]
    pub const fn compact_id(&self) -> Option<CompactId> {
        self.compact_id
    }

    #[must_use]
    pub const fn session_id(&self) -> Option<SessionId> {
        self.session_id
    }

    #[must_use]
    pub const fn context_generation(&self) -> Option<ContextGeneration> {
        self.context_generation
    }

    #[must_use]
    pub const fn command_seq(&self) -> u64 {
        self.command_seq
    }

    #[must_use]
    pub const fn deadline_unix_ms(&self) -> u64 {
        self.deadline_unix_ms
    }

    #[must_use]
    pub const fn request_digest(&self) -> &[u8; 32] {
        &self.request_digest
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostAckOutcome {
    Accepted,
    Rejected { error_code: Box<str> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostAck {
    ack_id: HostAckId,
    action_id: HostActionId,
    adapter_id: HostAdapterId,
    command_seq: u64,
    outcome: HostAckOutcome,
    host_task_id: Option<HostTaskId>,
    session_id: Option<SessionId>,
}

impl HostAck {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ack_id: HostAckId,
        action_id: HostActionId,
        adapter_id: HostAdapterId,
        command_seq: u64,
        outcome: HostAckOutcome,
        host_task_id: Option<HostTaskId>,
        session_id: Option<SessionId>,
    ) -> Result<Self, HostActionError> {
        if command_seq == 0 {
            return Err(HostActionError::ZeroCommandSequence);
        }
        Ok(Self {
            ack_id,
            action_id,
            adapter_id,
            command_seq,
            outcome,
            host_task_id,
            session_id,
        })
    }

    #[must_use]
    pub const fn ack_id(&self) -> HostAckId {
        self.ack_id
    }

    #[must_use]
    pub const fn action_id(&self) -> HostActionId {
        self.action_id
    }

    #[must_use]
    pub fn adapter_id(&self) -> &HostAdapterId {
        &self.adapter_id
    }

    #[must_use]
    pub const fn command_seq(&self) -> u64 {
        self.command_seq
    }

    #[must_use]
    pub const fn outcome(&self) -> &HostAckOutcome {
        &self.outcome
    }

    #[must_use]
    pub const fn host_task_id(&self) -> Option<&HostTaskId> {
        self.host_task_id.as_ref()
    }

    #[must_use]
    pub const fn session_id(&self) -> Option<SessionId> {
        self.session_id
    }

    pub fn validate_for(&self, action: &HostAction) -> Result<(), HostActionError> {
        if self.action_id != action.action_id
            || self.adapter_id != action.adapter_id
            || self.command_seq != action.command_seq
        {
            return Err(HostActionError::AckCorrelationMismatch);
        }
        Ok(())
    }
}

pub trait HostRuntimeAdapter: Send + Sync {
    fn adapter_id(&self) -> &HostAdapterId;
    fn capabilities(&self) -> &HostCapabilitySet;
    fn dispatch(&self, action: &HostAction) -> Result<(), HostAdapterError>;
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum HostAdapterError {
    #[error("host capability {0:?} is unsupported")]
    Unsupported(HostCapability),
    #[error("host rejected action dispatch: {0}")]
    Rejected(Box<str>),
    #[error("host action dispatch timed out")]
    Timeout,
    #[error("host adapter is unavailable")]
    Unavailable,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum HostActionError {
    #[error("{kind} ID is empty or exceeds 256 bytes")]
    InvalidOpaqueId { kind: &'static str },
    #[error("host command sequence must be greater than zero")]
    ZeroCommandSequence,
    #[error("host action deadline must be greater than zero")]
    ZeroDeadline,
    #[error("create action requires a delegation binding")]
    DelegationBindingRequired,
    #[error("compact action requires compact, session, and generation bindings")]
    CompactBindingRequired,
    #[error("host ACK does not match action/adapter/command sequence")]
    AckCorrelationMismatch,
}

fn validate_opaque_id(kind: &'static str, value: &str) -> Result<(), HostActionError> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(HostActionError::InvalidOpaqueId { kind });
    }
    Ok(())
}
