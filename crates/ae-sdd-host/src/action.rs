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

/// Kind of pending work the daemon posts for a host to pick up.
///
/// These are errands, not daemon operations: the daemon enqueues them, the host
/// pulls them and carries them out in its own process. Whether a host can carry
/// one out is therefore not knowable in advance — the answer arrives as the ACK
/// outcome. There is deliberately no capability declaration gating dispatch; a
/// host's self-description could not be verified against anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostActionKind {
    Create,
    Send,
    Wait,
    Cancel,
    Attest,
    Compact,
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
    /// Dispatches `action` to the host and returns its real result as a
    /// local [`HostAck`]. Exit 0 maps to `Ok(HostAck { outcome: Accepted, .. })`;
    /// a non-zero exit still maps to `Ok(HostAck { outcome: Rejected { error_code }, .. })`
    /// — both are host-delivered outcomes carrying full correlation
    /// (`action_id`/`adapter_id`/`command_seq`). Only conditions that mean the
    /// action was never delivered (spawn failure, timeout) return
    /// `Err(HostAdapterError)`.
    fn dispatch(&self, action: &HostAction) -> Result<HostAck, HostAdapterError>;
}

// Contract added at commit fda... (ROUTE-a4574dca U-2):
//
// The ae-sdd review sub-flow (`G-REVIEW-DEPTH` and any later review gates
// that read `state.reviewSession`) requires the host runtime to be able to
// spawn a physically isolated child process with its own agentId and own
// daemon connection, declare the issued identity itself, and execute
// `review.record` from that child. The daemon never creates this child; it
// only inspects the resulting `state.reviewSession`. A host that cannot
// satisfy this requirement cannot complete any route that ends with a review
// gate. ROUTE-a4574dca RA §6 U-2 records this as a handoff to a future D-7/D-8
// that explicitly widens this trait with a review capability or adds it to
// the host adapter onboarding standard.

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum HostAdapterError {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn adapter() -> HostAdapterId {
        HostAdapterId::new("adapter-1").expect("adapter id")
    }

    fn action_id() -> HostActionId {
        HostActionId::from_uuid(uuid::Uuid::from_u128(1))
    }

    /// Builds an otherwise-valid `Create` action, letting each test vary one
    /// field to isolate a single rejection rule.
    fn create_action(
        command_seq: u64,
        deadline_unix_ms: u64,
        delegation_id: Option<DelegationId>,
    ) -> Result<HostAction, HostActionError> {
        HostAction::new(
            action_id(),
            adapter(),
            command_seq,
            HostActionKind::Create,
            delegation_id,
            None,
            None,
            None,
            deadline_unix_ms,
            [7; 32],
        )
    }

    fn delegation() -> DelegationId {
        DelegationId::from_uuid(uuid::Uuid::from_u128(2))
    }

    #[test]
    fn action_rejects_zero_command_sequence_and_zero_deadline_independently() {
        assert_eq!(
            create_action(0, 2_000, Some(delegation())),
            Err(HostActionError::ZeroCommandSequence)
        );
        assert_eq!(
            create_action(1, 0, Some(delegation())),
            Err(HostActionError::ZeroDeadline)
        );
        // Both valid: proves the two guards above are the only thing that
        // rejected the cases, not some unrelated field.
        assert!(create_action(1, 2_000, Some(delegation())).is_ok());
    }

    #[test]
    fn create_action_requires_a_delegation_binding() {
        assert_eq!(
            create_action(1, 2_000, None),
            Err(HostActionError::DelegationBindingRequired)
        );
    }

    #[test]
    fn compact_action_requires_compact_session_and_generation_bindings() {
        let compact_id = CompactId::from_uuid(uuid::Uuid::from_u128(3));
        let session_id = SessionId::from_uuid(uuid::Uuid::from_u128(4));
        let generation = ContextGeneration::default();
        let build = |compact, session, generation_arg| {
            HostAction::new(
                action_id(),
                adapter(),
                1,
                HostActionKind::Compact,
                None,
                compact,
                session,
                generation_arg,
                2_000,
                [7; 32],
            )
        };

        // Each of the three bindings is individually required.
        for (compact, session, generation_arg) in [
            (None, Some(session_id), Some(generation)),
            (Some(compact_id), None, Some(generation)),
            (Some(compact_id), Some(session_id), None),
        ] {
            assert_eq!(
                build(compact, session, generation_arg),
                Err(HostActionError::CompactBindingRequired),
                "missing one compact binding must be rejected"
            );
        }
        assert!(build(Some(compact_id), Some(session_id), Some(generation)).is_ok());
    }

    #[test]
    fn ack_rejects_zero_command_sequence() {
        let build = |command_seq| {
            HostAck::new(
                HostAckId::from_uuid(uuid::Uuid::from_u128(5)),
                action_id(),
                adapter(),
                command_seq,
                HostAckOutcome::Accepted,
                None,
                None,
            )
        };

        assert_eq!(build(0), Err(HostActionError::ZeroCommandSequence));
        assert!(build(1).is_ok());
    }

    #[test]
    fn opaque_ids_reject_empty_overlong_and_control_characters() {
        // Empty.
        assert_eq!(
            HostAdapterId::new(""),
            Err(HostActionError::InvalidOpaqueId { kind: "adapter" })
        );
        // Over the 256-byte bound.
        assert_eq!(
            HostAdapterId::new("a".repeat(257)),
            Err(HostActionError::InvalidOpaqueId { kind: "adapter" })
        );
        // Control character embedded mid-value.
        assert_eq!(
            HostAdapterId::new("adapter\u{7}1"),
            Err(HostActionError::InvalidOpaqueId { kind: "adapter" })
        );
        // The `kind` label is per-type, so a host task id reports its own.
        assert_eq!(
            HostTaskId::new(""),
            Err(HostActionError::InvalidOpaqueId { kind: "host task" })
        );
        // Exactly at the bound is accepted.
        assert!(HostAdapterId::new("a".repeat(256)).is_ok());
    }

    #[test]
    fn every_error_variant_renders_a_distinct_nonempty_message() {
        let variants = [
            HostActionError::InvalidOpaqueId { kind: "adapter" },
            HostActionError::ZeroCommandSequence,
            HostActionError::ZeroDeadline,
            HostActionError::DelegationBindingRequired,
            HostActionError::CompactBindingRequired,
            HostActionError::AckCorrelationMismatch,
        ];
        let rendered = variants.iter().map(ToString::to_string).collect::<Vec<_>>();

        assert!(
            rendered.iter().all(|message| !message.trim().is_empty()),
            "no error variant may render an empty message"
        );
        let unique = rendered.iter().collect::<BTreeSet<_>>();
        assert_eq!(
            unique.len(),
            rendered.len(),
            "each variant must be distinguishable in logs: {rendered:?}"
        );
    }
}
