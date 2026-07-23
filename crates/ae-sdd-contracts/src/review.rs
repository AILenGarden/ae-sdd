//! Frozen Review Supervisor session and exit-receipt contracts.

use std::collections::BTreeSet;

use ae_sdd_domain::InputFingerprint;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{BoundedText, ReasonCode, ReviewId, ReviewerRole, SchemaVersion, serde_domain};

/// Maximum reviewers participating in one review session.
pub const MAX_REVIEWERS: usize = 8;
/// Maximum findings carried by one exit receipt.
pub const MAX_REVIEW_FINDINGS: usize = 128;
/// Maximum review rounds represented by a v1 budget.
pub const MAX_REVIEW_ROUNDS: u32 = 32;
/// Maximum review duration represented by a v1 budget.
pub const MAX_REVIEW_DURATION_MS: u64 = 24 * 60 * 60 * 1000;

/// Review depth selected by policy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewTier {
    /// Fast, focused review for a narrow change.
    Tier1,
    /// Normal independent engineering review.
    Tier2,
    /// Adversarial or cross-domain review for high-risk work.
    Tier3,
}

/// Authoritative lifecycle status of a review session.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    /// Review is accepted but no reviewer has started.
    Queued,
    /// One or more required reviewers are active.
    Running,
    /// Reviewer results are being validated and aggregated.
    Collecting,
    /// A valid terminal review result was collected.
    Completed,
    /// Progress or budget was exhausted without a valid exit.
    Stalled,
    /// Review infrastructure or reviewer independence was invalid.
    InvalidInfra,
    /// Review was explicitly aborted.
    Aborted,
}

/// Semantic exit disposition of a completed or halted review.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewExitDisposition {
    /// All required independent reviewers completed with no findings.
    Pass,
    /// Valid reviewers produced one or more findings.
    Findings,
    /// Review must be retried against repaired or refreshed inputs.
    Retry,
    /// Review cannot proceed until an external blocker is removed.
    Blocked,
    /// Review was aborted by an authorized actor.
    Aborted,
}

/// Stable finding severity used by review receipts.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFindingSeverity {
    /// Completion-blocking correctness or security issue.
    Blocker,
    /// Material issue requiring repair before completion.
    Major,
    /// Non-blocking quality issue.
    Minor,
}

/// Bounded review budget selected before a session starts.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "ReviewBudgetWire", into = "ReviewBudgetWire")]
pub struct ReviewBudget {
    max_rounds: u32,
    max_findings: u32,
    max_duration_ms: u64,
}

impl<'de> Deserialize<'de> for ReviewBudget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ReviewBudgetWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewBudgetWire {
    max_rounds: u32,
    max_findings: u32,
    max_duration_ms: u64,
}

impl TryFrom<ReviewBudgetWire> for ReviewBudget {
    type Error = ReviewContractError;

    fn try_from(value: ReviewBudgetWire) -> Result<Self, Self::Error> {
        Self::new(value.max_rounds, value.max_findings, value.max_duration_ms)
    }
}

impl From<ReviewBudget> for ReviewBudgetWire {
    fn from(value: ReviewBudget) -> Self {
        Self {
            max_rounds: value.max_rounds,
            max_findings: value.max_findings,
            max_duration_ms: value.max_duration_ms,
        }
    }
}

impl ReviewBudget {
    /// Constructs a bounded, non-zero review budget.
    pub fn new(
        max_rounds: u32,
        max_findings: u32,
        max_duration_ms: u64,
    ) -> Result<Self, ReviewContractError> {
        if max_rounds == 0
            || max_rounds > MAX_REVIEW_ROUNDS
            || max_findings == 0
            || max_findings > MAX_REVIEW_FINDINGS as u32
            || max_duration_ms == 0
            || max_duration_ms > MAX_REVIEW_DURATION_MS
        {
            return Err(ReviewContractError::InvalidBudget);
        }
        Ok(Self {
            max_rounds,
            max_findings,
            max_duration_ms,
        })
    }

    /// Returns the maximum round count.
    pub const fn max_rounds(self) -> u32 {
        self.max_rounds
    }
}

/// Bounded finding carried in a review exit receipt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewFinding {
    code: ReasonCode,
    severity: ReviewFindingSeverity,
    summary: BoundedText<1024>,
}

impl ReviewFinding {
    /// Constructs a stable, bounded finding.
    pub const fn new(
        code: ReasonCode,
        severity: ReviewFindingSeverity,
        summary: BoundedText<1024>,
    ) -> Self {
        Self {
            code,
            severity,
            summary,
        }
    }
}

/// Structural validation errors for review contracts.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ReviewContractError {
    /// No required reviewer roles were supplied.
    #[error("review session must require at least one reviewer role")]
    MissingRequiredReviewer,
    /// A bounded review collection exceeded its v1 limit.
    #[error("review collection exceeds its frozen v1 limit")]
    CollectionLimitExceeded,
    /// A reviewer role appeared more than once.
    #[error("review contract contains a duplicate reviewer role")]
    DuplicateReviewerRole,
    /// Review budget values were zero or outside v1 bounds.
    #[error("review budget is outside its frozen v1 bounds")]
    InvalidBudget,
    /// The requested round exceeded the configured budget.
    #[error("review round exceeds its configured budget")]
    RoundExceedsBudget,
    /// An exit receipt represented a zero round.
    #[error("review exit round must be greater than zero")]
    InvalidRound,
    /// A non-completed session attempted to produce PASS.
    #[error("only a completed review session may produce PASS")]
    InvalidPassStatus,
    /// A stalled review attempted to produce PASS.
    #[error("a stalled review cannot produce PASS")]
    StalledCannotPass,
    /// Invalid review infrastructure attempted to produce PASS.
    #[error("invalid review infrastructure cannot produce PASS")]
    InvalidInfrastructureCannotPass,
    /// The observed input fingerprint differs from the session input.
    #[error("review input fingerprint drifted during the session")]
    FingerprintDrift,
    /// Not every required reviewer role completed.
    #[error("review exit is missing one or more required reviewer roles")]
    MissingCompletedReviewer,
    /// PASS was paired with findings.
    #[error("review PASS cannot contain findings")]
    PassWithFindings,
    /// A findings disposition had no findings.
    #[error("review findings disposition must contain at least one finding")]
    FindingsMissing,
}

/// Immutable parameters and budget for an independent review session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "ReviewSessionWire", into = "ReviewSessionWire")]
pub struct ReviewSession {
    schema_version: SchemaVersion,
    review_id: ReviewId,
    tier: ReviewTier,
    required_roles: Vec<ReviewerRole>,
    input_fingerprint: InputFingerprint,
    ruleset_fingerprint: InputFingerprint,
    round: u32,
    clean_streak: u32,
    budget: ReviewBudget,
    status: ReviewStatus,
}

impl ReviewSession {
    /// Constructs a running review session.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        review_id: ReviewId,
        tier: ReviewTier,
        required_roles: Vec<ReviewerRole>,
        input_fingerprint: InputFingerprint,
        ruleset_fingerprint: InputFingerprint,
        round: u32,
        clean_streak: u32,
        budget: ReviewBudget,
    ) -> Result<Self, ReviewContractError> {
        Self::from_parts(
            schema_version,
            review_id,
            tier,
            required_roles,
            input_fingerprint,
            ruleset_fingerprint,
            round,
            clean_streak,
            budget,
            ReviewStatus::Running,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        schema_version: SchemaVersion,
        review_id: ReviewId,
        tier: ReviewTier,
        required_roles: Vec<ReviewerRole>,
        input_fingerprint: InputFingerprint,
        ruleset_fingerprint: InputFingerprint,
        round: u32,
        clean_streak: u32,
        budget: ReviewBudget,
        status: ReviewStatus,
    ) -> Result<Self, ReviewContractError> {
        validate_roles(&required_roles, true)?;
        if round == 0 || round > budget.max_rounds() {
            return Err(ReviewContractError::RoundExceedsBudget);
        }
        Ok(Self {
            schema_version,
            review_id,
            tier,
            required_roles,
            input_fingerprint,
            ruleset_fingerprint,
            round,
            clean_streak,
            budget,
            status,
        })
    }

    /// Returns the review identity.
    pub const fn review_id(&self) -> &ReviewId {
        &self.review_id
    }

    /// Returns the session status.
    pub const fn status(&self) -> ReviewStatus {
        self.status
    }

    /// Returns required reviewer roles.
    pub fn required_roles(&self) -> &[ReviewerRole] {
        &self.required_roles
    }
}

impl<'de> Deserialize<'de> for ReviewSession {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ReviewSessionWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewSessionWire {
    schema_version: SchemaVersion,
    review_id: ReviewId,
    tier: ReviewTier,
    required_roles: Vec<ReviewerRole>,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::input_fingerprint")]
    ruleset_fingerprint: InputFingerprint,
    round: u32,
    clean_streak: u32,
    budget: ReviewBudget,
    status: ReviewStatus,
}

impl TryFrom<ReviewSessionWire> for ReviewSession {
    type Error = ReviewContractError;

    fn try_from(value: ReviewSessionWire) -> Result<Self, Self::Error> {
        Self::from_parts(
            value.schema_version,
            value.review_id,
            value.tier,
            value.required_roles,
            value.input_fingerprint,
            value.ruleset_fingerprint,
            value.round,
            value.clean_streak,
            value.budget,
            value.status,
        )
    }
}

impl From<ReviewSession> for ReviewSessionWire {
    fn from(value: ReviewSession) -> Self {
        Self {
            schema_version: value.schema_version,
            review_id: value.review_id,
            tier: value.tier,
            required_roles: value.required_roles,
            input_fingerprint: value.input_fingerprint,
            ruleset_fingerprint: value.ruleset_fingerprint,
            round: value.round,
            clean_streak: value.clean_streak,
            budget: value.budget,
            status: value.status,
        }
    }
}

/// Canonical, fail-closed result of validating a review session exit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "ReviewExitReceiptWire", into = "ReviewExitReceiptWire")]
pub struct ReviewExitReceipt {
    schema_version: SchemaVersion,
    review_id: ReviewId,
    tier: ReviewTier,
    required_roles: Vec<ReviewerRole>,
    completed_roles: Vec<ReviewerRole>,
    session_status: ReviewStatus,
    disposition: ReviewExitDisposition,
    session_input_fingerprint: InputFingerprint,
    observed_input_fingerprint: InputFingerprint,
    ruleset_fingerprint: InputFingerprint,
    round: u32,
    findings: Vec<ReviewFinding>,
}

impl ReviewExitReceipt {
    /// Validates an exit against the immutable session contract.
    pub fn new(
        schema_version: SchemaVersion,
        session: &ReviewSession,
        session_status: ReviewStatus,
        disposition: ReviewExitDisposition,
        observed_input_fingerprint: InputFingerprint,
        completed_roles: Vec<ReviewerRole>,
        findings: Vec<ReviewFinding>,
    ) -> Result<Self, ReviewContractError> {
        Self::from_parts(
            schema_version,
            session.review_id.clone(),
            session.tier,
            session.required_roles.clone(),
            completed_roles,
            session_status,
            disposition,
            session.input_fingerprint,
            observed_input_fingerprint,
            session.ruleset_fingerprint,
            session.round,
            findings,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        schema_version: SchemaVersion,
        review_id: ReviewId,
        tier: ReviewTier,
        required_roles: Vec<ReviewerRole>,
        completed_roles: Vec<ReviewerRole>,
        session_status: ReviewStatus,
        disposition: ReviewExitDisposition,
        session_input_fingerprint: InputFingerprint,
        observed_input_fingerprint: InputFingerprint,
        ruleset_fingerprint: InputFingerprint,
        round: u32,
        findings: Vec<ReviewFinding>,
    ) -> Result<Self, ReviewContractError> {
        validate_roles(&required_roles, true)?;
        validate_roles(&completed_roles, false)?;
        if findings.len() > MAX_REVIEW_FINDINGS {
            return Err(ReviewContractError::CollectionLimitExceeded);
        }
        if round == 0 {
            return Err(ReviewContractError::InvalidRound);
        }
        if disposition == ReviewExitDisposition::Pass {
            match session_status {
                ReviewStatus::Stalled => return Err(ReviewContractError::StalledCannotPass),
                ReviewStatus::InvalidInfra => {
                    return Err(ReviewContractError::InvalidInfrastructureCannotPass);
                }
                ReviewStatus::Completed => {}
                _ => return Err(ReviewContractError::InvalidPassStatus),
            }
            if session_input_fingerprint != observed_input_fingerprint {
                return Err(ReviewContractError::FingerprintDrift);
            }
            if !findings.is_empty() {
                return Err(ReviewContractError::PassWithFindings);
            }
            let required: BTreeSet<&ReviewerRole> = required_roles.iter().collect();
            let completed: BTreeSet<&ReviewerRole> = completed_roles.iter().collect();
            if required != completed {
                return Err(ReviewContractError::MissingCompletedReviewer);
            }
        }
        if disposition == ReviewExitDisposition::Findings && findings.is_empty() {
            return Err(ReviewContractError::FindingsMissing);
        }
        Ok(Self {
            schema_version,
            review_id,
            tier,
            required_roles,
            completed_roles,
            session_status,
            disposition,
            session_input_fingerprint,
            observed_input_fingerprint,
            ruleset_fingerprint,
            round,
            findings,
        })
    }

    /// Returns true only for a validated PASS exit.
    pub const fn is_pass(&self) -> bool {
        matches!(self.disposition, ReviewExitDisposition::Pass)
    }
}

impl<'de> Deserialize<'de> for ReviewExitReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ReviewExitReceiptWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewExitReceiptWire {
    schema_version: SchemaVersion,
    review_id: ReviewId,
    tier: ReviewTier,
    required_roles: Vec<ReviewerRole>,
    completed_roles: Vec<ReviewerRole>,
    session_status: ReviewStatus,
    disposition: ReviewExitDisposition,
    #[serde(with = "serde_domain::input_fingerprint")]
    session_input_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::input_fingerprint")]
    observed_input_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::input_fingerprint")]
    ruleset_fingerprint: InputFingerprint,
    round: u32,
    findings: Vec<ReviewFinding>,
}

impl TryFrom<ReviewExitReceiptWire> for ReviewExitReceipt {
    type Error = ReviewContractError;

    fn try_from(value: ReviewExitReceiptWire) -> Result<Self, Self::Error> {
        Self::from_parts(
            value.schema_version,
            value.review_id,
            value.tier,
            value.required_roles,
            value.completed_roles,
            value.session_status,
            value.disposition,
            value.session_input_fingerprint,
            value.observed_input_fingerprint,
            value.ruleset_fingerprint,
            value.round,
            value.findings,
        )
    }
}

impl From<ReviewExitReceipt> for ReviewExitReceiptWire {
    fn from(value: ReviewExitReceipt) -> Self {
        Self {
            schema_version: value.schema_version,
            review_id: value.review_id,
            tier: value.tier,
            required_roles: value.required_roles,
            completed_roles: value.completed_roles,
            session_status: value.session_status,
            disposition: value.disposition,
            session_input_fingerprint: value.session_input_fingerprint,
            observed_input_fingerprint: value.observed_input_fingerprint,
            ruleset_fingerprint: value.ruleset_fingerprint,
            round: value.round,
            findings: value.findings,
        }
    }
}

fn validate_roles(
    roles: &[ReviewerRole],
    require_non_empty: bool,
) -> Result<(), ReviewContractError> {
    if require_non_empty && roles.is_empty() {
        return Err(ReviewContractError::MissingRequiredReviewer);
    }
    if roles.len() > MAX_REVIEWERS {
        return Err(ReviewContractError::CollectionLimitExceeded);
    }
    let unique: BTreeSet<&ReviewerRole> = roles.iter().collect();
    if unique.len() != roles.len() {
        return Err(ReviewContractError::DuplicateReviewerRole);
    }
    Ok(())
}
