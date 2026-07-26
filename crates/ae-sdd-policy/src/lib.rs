#![forbid(unsafe_code)]

//! Deterministic ae-sdd process policy.
//!
//! This crate is the single owner of transition, Gate interpretation, and
//! Agent-role permission rules. It deliberately contains no I/O or runtime
//! integration.

use ae_sdd_domain::PolicyDigest;

mod gate;
mod hook;
mod role;
mod transition;

pub use gate::{GateDirective, GateJudgement, GateTruth, InfrastructureImpact};
pub use hook::{
    ExecutionHookDenialReason, ExecutionHookGuard, ExecutionHookGuardInput, ExecutionHookToolClass,
    ExecutionHookVerdict, HookAction, HookContextProof, HookGuard, HookGuardDecision,
    HookGuardDisposition, HookGuardInput, HookGuardPort, HookGuardReason, HookPoint, HookPolicy,
};
pub use role::{RoleAuthorizationError, RoleOperation, RolePolicy};
pub use transition::{
    RequiredGate, TransitionContext, TransitionPermit, TransitionPolicy, TransitionPolicyError,
};

/// Returns the digest of the complete transition, Gate, role, and Hook policy.
///
/// The explicit revision marker is bumped whenever any policy table changes;
/// daemon manifests must publish this value rather than a placeholder digest.
#[must_use]
pub fn policy_digest() -> PolicyDigest {
    PolicyDigest::digest(b"ae-sdd-policy/v3:transition+gate+role+hook-guard")
}
