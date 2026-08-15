use std::{error::Error, fmt};

use ae_sdd_domain::{
    EventSequence, EventStoreId, InputFingerprint, PolicyDigest, ProcessPhase, StateRevision,
};
use ae_sdd_policy::RequiredGate;

/// Deterministic rejection produced while validating or reducing events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowError {
    /// Published events use positive sequence numbers; zero is never committed.
    InvalidEventSequence,
    /// The cursor belongs to a rebuilt or different durable event store.
    EventStoreMismatch {
        expected: EventStoreId,
        actual: EventStoreId,
    },
    /// One immutable event sequence was presented with different content.
    EventSequenceConflict { sequence: EventSequence },
    /// The event was evaluated under a different policy revision.
    PolicyDigestMismatch {
        expected: PolicyDigest,
        actual: PolicyDigest,
    },
    /// The event targets a different flow input snapshot.
    InputFingerprintMismatch {
        expected: InputFingerprint,
        actual: InputFingerprint,
    },
    /// A second transition intent arrived while the first was still pending.
    TransitionAlreadyPending {
        pending: ProcessPhase,
        requested: ProcessPhase,
    },
    /// A Gate completed when no transition was waiting for Gate evidence.
    UnexpectedGateOutcome,
    /// The result names a Gate outside the pending transition's required set.
    UnexpectedGate { gate: RequiredGate },
    /// A committed transition does not match the pending root intent.
    UnexpectedTransitionCommit {
        pending: Option<ProcessPhase>,
        committed: ProcessPhase,
    },
    /// The target is pending but its Gate/authorization decision is not ready.
    TransitionNotReady { target: ProcessPhase },
    /// RouteSelected cannot be requested until RA has produced a candidate.
    RouteCandidateMissing { target: ProcessPhase },
    /// Downstream phases cannot consume a route that has not been frozen.
    RouteNotFrozen { target: ProcessPhase },
    /// A commit must advance the authoritative state revision.
    NonMonotonicStateRevision {
        current: StateRevision,
        committed: StateRevision,
    },
    /// The correction counter reached its integer bound.
    CorrectionOverflow,
}

impl fmt::Display for FlowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEventSequence => {
                formatter.write_str("event sequence zero is not a committed event")
            }
            Self::EventStoreMismatch { expected, actual } => write!(
                formatter,
                "event store {actual} does not match checkpoint store {expected}"
            ),
            Self::EventSequenceConflict { sequence } => write!(
                formatter,
                "event sequence {} was reused with different content",
                sequence.get()
            ),
            Self::PolicyDigestMismatch { expected, actual } => write!(
                formatter,
                "event policy digest {actual} does not match flow policy {expected}"
            ),
            Self::InputFingerprintMismatch { expected, actual } => write!(
                formatter,
                "event input fingerprint {actual} does not match flow input {expected}"
            ),
            Self::TransitionAlreadyPending { pending, requested } => write!(
                formatter,
                "transition {requested:?} cannot replace pending transition {pending:?}"
            ),
            Self::UnexpectedGateOutcome => {
                formatter.write_str("Gate outcome arrived without a pending transition")
            }
            Self::UnexpectedGate { gate } => write!(
                formatter,
                "Gate {} is not required by the pending transition",
                gate.as_str()
            ),
            Self::UnexpectedTransitionCommit { pending, committed } => write!(
                formatter,
                "committed transition {committed:?} does not match pending target {pending:?}"
            ),
            Self::TransitionNotReady { target } => write!(
                formatter,
                "transition {target:?} was committed before all policy conditions passed"
            ),
            Self::RouteCandidateMissing { target } => write!(
                formatter,
                "transition {target:?} requires a requirement-analysis route candidate"
            ),
            Self::RouteNotFrozen { target } => write!(
                formatter,
                "transition {target:?} requires a frozen engineering route"
            ),
            Self::NonMonotonicStateRevision { current, committed } => write!(
                formatter,
                "committed revision {} does not advance current revision {}",
                committed.get(),
                current.get()
            ),
            Self::CorrectionOverflow => formatter.write_str("correction counter overflow"),
        }
    }
}

impl Error for FlowError {}
