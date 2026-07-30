#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Pure verification execution boundary for Part D (Assurance Plane).
//!
//! Consumes frozen [`ae_sdd_contracts::execution`] types and validates worker
//! isolation, plan↔receipt identity and toolset requirements without any I/O,
//! clock, random or filesystem access.
//!
//! It also owns the deterministic approved-plan execution surface:
//! [`build_execution_capsule`] derives a byte-identical full queue artifact
//! plus a bounded active-slice capsule from typed input, and
//! [`transition_slice_status`] is the pure slice lifecycle reducer.  Neither
//! reads the filesystem, a clock, randomness or a database.
//!
//! On top of that surface [`ExecutionSupervisor`] is the pure slice-progress
//! policy: it adjudicates every bounded [`ExecutionToolEventV1`] against a
//! restartable checkpoint — RED/GREEN cadence, investigation batches, output
//! budgets and broad-test timing — with zero I/O, clock or randomness.

mod capsule;
mod error;
mod plan;
mod policy;
mod receipt;
mod slice;
mod supervisor;

pub use capsule::{
    CapsuleBuildInputV1, CapsuleBuildOutcome, ExecutionQueueV1, ExecutionSliceSpecV1,
    build_execution_capsule,
};
pub use error::{
    ExecutionCapsuleBuildError, ExecutionPolicyError, ExecutionPolicyFault,
    ExecutionSliceTransitionError, ExecutionSupervisorError, ExecutionSupervisorFault,
};
pub use plan::{ToolsetPort, ToolsetQuery, ToolsetRequirement};
pub use policy::{
    ExecutionAllowanceV1, ExecutionDecisionV1, ExecutionDeferralV1, ExecutionOutputDirectiveV1,
    ExecutionPolicy, ExecutionProgressKindV1, reject_shell_program_path,
    shell_executable_blocklist,
};
pub use receipt::validate_against_plan;
pub use slice::{ExecutionSliceEvent, RefactorCycleV1, transition_slice_status};
pub use supervisor::{
    ExecutionSupervisor, ExecutionSupervisorCheckpointV1, ExecutionToolEventV1,
    ExecutionToolOutputV1, FocusedTestOutcomeV1, FocusedTestStateV1,
};

/// Re-export of frozen contract types so consumers depend only on this crate.
pub use ae_sdd_contracts::execution::{
    DEFAULT_EXECUTION_TIMEOUT_MS, DEFAULT_OUTPUT_BYTES, EnvironmentRef, ExecutionLimits,
    ExecutionStep, ExecutionStepError, MAX_ENVIRONMENT_REFS, MAX_EXECUTION_ARGS,
    MAX_EXECUTION_STEPS, MAX_EXECUTION_TIMEOUT_MS, MAX_OUTPUT_BYTES, VerificationExecutionPlan,
    VerificationReceipt,
};
pub use ae_sdd_contracts::{
    ExecutionId, ExecutionStepId, SchemaVersion, VerificationContractId, WorkerId,
};
