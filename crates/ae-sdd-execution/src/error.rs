//! Stable execution-policy, capsule-build and slice-transition errors.

use ae_sdd_contracts::ControlPlaneErrorCode;
use ae_sdd_contracts::execution_runtime::{ExecutionCapsuleError, ExecutionSliceStatus};
use ae_sdd_domain::ExecutionSliceId;
use thiserror::Error;

use crate::slice::ExecutionSliceEvent;

/// Specific reason a verification execution plan was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionPolicyFault {
    /// A program reference contained shell-injection metacharacters.
    ShellInjectionInProgramRef,
    /// A program reference named a known shell executable.
    ShellExecutableProgram,
    /// A receipt claimed PASS without a real successful process result.
    FakePassResult,
    /// A receipt referenced a stale artifact digest.
    StaleArtifactDigest,
    /// A receipt did not match its plan identity.
    IdentityMismatch,
}

/// Error returned by the pure execution policy and receipt validators.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionPolicyError {
    /// The verification plan violated worker isolation rules.
    PlanRejected(ExecutionPolicyFault),
    /// The verification receipt was invalid.
    ReceiptRejected(ExecutionPolicyFault),
}

impl ExecutionPolicyError {
    /// Maps an execution-policy error to a frozen stable error code.
    #[must_use]
    pub const fn error_code(&self) -> ControlPlaneErrorCode {
        match self {
            Self::PlanRejected(_) => ControlPlaneErrorCode::ExecutionPlanInvalid,
            Self::ReceiptRejected(_) => ControlPlaneErrorCode::ExecutionFailed,
        }
    }
}

impl std::fmt::Display for ExecutionPolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlanRejected(fault) => {
                write!(formatter, "verification plan rejected: {fault:?}")
            }
            Self::ReceiptRejected(fault) => {
                write!(formatter, "verification receipt rejected: {fault:?}")
            }
        }
    }
}

impl std::error::Error for ExecutionPolicyError {}

/// Errors returned by the deterministic execution queue/capsule builder.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ExecutionCapsuleBuildError {
    /// A frozen contract rejected a slice, the queue cursor, the capsule or a budget.
    #[error(transparent)]
    Contract(#[from] ExecutionCapsuleError),
    /// Two slices declared the same identity.
    #[error("duplicate execution slice id `{slice_id}`")]
    DuplicateSliceId {
        /// The duplicated slice identity.
        slice_id: ExecutionSliceId,
    },
    /// Slice ordinals did not form the contiguous 1-based queue prefix.
    #[error("execution slice ordinals must form the contiguous 1-based queue prefix")]
    NonContiguousOrdinals,
    /// A dependency referenced a slice absent from the queue.
    #[error("execution slice `{slice_id}` depends on unknown slice `{dependency}`")]
    UnknownDependency {
        /// The slice declaring the dependency.
        slice_id: ExecutionSliceId,
        /// The unknown dependency identity.
        dependency: ExecutionSliceId,
    },
    /// The dependency graph contains a cycle.
    #[error("execution slice dependency graph contains a cycle reaching `{slice_id}`")]
    DependencyCycle {
        /// A slice on the detected cycle.
        slice_id: ExecutionSliceId,
    },
    /// A dependency referenced a slice whose ordinal is not strictly lower.
    #[error("execution slice `{slice_id}` depends on `{dependency}` which is not an earlier slice")]
    DependencyNotLower {
        /// The slice declaring the dependency.
        slice_id: ExecutionSliceId,
        /// The offending dependency identity.
        dependency: ExecutionSliceId,
    },
    /// The active ordinal was zero or beyond the queue length.
    #[error("active ordinal {active_ordinal} is outside the queue of {total_slices} slices")]
    InvalidActiveOrdinal {
        /// The rejected active ordinal.
        active_ordinal: u32,
        /// The total slice count.
        total_slices: u32,
    },
    /// Canonical JSON encoding of a deterministic artifact failed.
    #[error("failed to encode canonical `{artifact}`")]
    CanonicalEncodeFailed {
        /// The artifact schema that failed to encode.
        artifact: &'static str,
    },
}

/// Error returned when an event does not legally advance the slice lifecycle.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ExecutionSliceTransitionError {
    /// The event is not a legal transition from the current status.
    #[error("illegal execution slice transition from {from:?} via {event:?}")]
    IllegalTransition {
        /// Current slice status.
        from: ExecutionSliceStatus,
        /// Rejected machine event.
        event: ExecutionSliceEvent,
    },
}
