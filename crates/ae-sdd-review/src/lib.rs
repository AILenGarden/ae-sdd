#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Pure Review supervisor for Part D (Assurance Plane).
//!
//! Consumes frozen [`ae_sdd_contracts::review`] types and emits validated
//! [`ReviewExitReceipt`]s or stable [`ReviewSupervisorError`]s. The supervisor
//! is stateless, deterministic and free of clock/filesystem/network access.

mod error;
mod fingerprint;
mod model;
mod policy;
mod supervisor;

pub use error::{IdentityViolation, InfraFault, ReviewSupervisorError};
pub use fingerprint::{FindingDigest, dedup_findings, finding_fingerprint};
pub use model::{
    CollectedReview, MAX_REVIEWER_LINEAGE_DEPTH, ReviewerIdentity, ReviewerIdentityError,
};
pub use policy::{matches_tier_matrix, min_reviewers, required_roles_for};
pub use supervisor::ReviewSupervisor;

/// Re-export of frozen contract types so consumers depend only on this crate.
pub use ae_sdd_contracts::review::{
    MAX_REVIEW_DURATION_MS, MAX_REVIEW_FINDINGS, MAX_REVIEW_ROUNDS, MAX_REVIEWERS, ReviewBudget,
    ReviewExitDisposition, ReviewExitReceipt, ReviewFinding, ReviewFindingSeverity, ReviewSession,
    ReviewStatus, ReviewTier,
};
pub use ae_sdd_contracts::{ReasonCode, ReviewId, ReviewerRole, SchemaVersion};
