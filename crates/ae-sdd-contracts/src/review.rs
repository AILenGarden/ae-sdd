//! Frozen Review Supervisor session and exit-receipt contracts.

use std::{cmp::Ordering, collections::BTreeSet, fmt, str::FromStr};

use ae_sdd_domain::{
    AgentRole, ArtifactDigest, DelegationId, InputFingerprint, PolicyDigest, SessionId,
};
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
/// Maximum attempts, valid batches, or remediations in one v2 session.
pub const MAX_REVIEW_V2_COUNT: u32 = 32;
/// Maximum wall-clock budget represented by a v2 session.
pub const MAX_REVIEW_V2_WALL_CLOCK_MINUTES: u32 = 24 * 60;
/// Maximum contributions presented by one v2 attempt.
pub const MAX_REVIEW_V2_ATTEMPT_CONTRIBUTIONS: usize = 4;

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
    /// A review-local v2 value violated a frozen structural invariant.
    #[error("invalid review v2 contract: {0}")]
    InvalidV2(&'static str),
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

// Review v2 is intentionally local to this module. The global SchemaVersion
// remains frozen at v1 so legacy rows can never be interpreted as v2 PASS.

/// Review-local schema version.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ReviewSchemaVersion {
    /// Batch-based review contract.
    #[serde(rename = "v2")]
    V2,
}

/// Canonical reviewer specialty used by Review Batch v2.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewerSpecialty {
    /// General reviewer used only by Tier 1.
    General,
    /// Backend engineering reviewer.
    Be,
    /// Architecture reviewer.
    Ar,
    /// Quality-assurance reviewer.
    Qa,
}

/// Authoritative v2 review-session status.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewSessionStatusV2 {
    /// Accepted but not yet running.
    Queued,
    /// Accepting attempts.
    Running,
    /// Findings require a committed remediation.
    RemediationRequired,
    /// Clean target and proof were satisfied.
    Completed,
    /// A review budget was exhausted without PASS.
    Stalled,
    /// Input or ruleset drift invalidated the session.
    Invalidated,
    /// The session was explicitly cancelled.
    Aborted,
}

impl ReviewSessionStatusV2 {
    /// Returns whether the status is terminal.
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Stalled | Self::Invalidated | Self::Aborted
        )
    }
}

/// Canonical v2 batch status. JSON uses the frozen upper-snake wire values.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ReviewBatchStatusV2 {
    /// Exact required specialties completed without findings.
    #[serde(rename = "VALID_CLEAN")]
    ValidClean,
    /// Exact required specialties completed with findings.
    #[serde(rename = "VALID_FINDINGS")]
    ValidFindings,
    /// Infrastructure prevented a complete valid batch.
    #[serde(rename = "INVALID_INFRA")]
    InvalidInfra,
    /// The attempt violated protocol or identity constraints.
    #[serde(rename = "INVALID_PROTOCOL")]
    InvalidProtocol,
    /// Attempt fingerprints differ from the session fingerprints.
    #[serde(rename = "INVALID_INPUT_DRIFT")]
    InvalidInputDrift,
    /// The attempt explicitly cancelled the session.
    #[serde(rename = "CANCELLED")]
    Cancelled,
}

impl ReviewBatchStatusV2 {
    /// Returns whether this is a business-valid batch.
    pub const fn is_valid(self) -> bool {
        matches!(self, Self::ValidClean | Self::ValidFindings)
    }

    /// Returns whether this status closes its batch.
    pub const fn closes_batch(self) -> bool {
        matches!(
            self,
            Self::ValidClean | Self::ValidFindings | Self::InvalidInputDrift | Self::Cancelled
        )
    }
}

/// Review repair classification derived by daemon policy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewRepairClass {
    /// No prior repair.
    None,
    /// A non-critical repair.
    NonCritical,
    /// A high-risk repair.
    HighRisk,
    /// A critical shared-contract repair.
    CriticalContract,
}

/// Proof required before a clean session may PASS.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFinalProofKind {
    /// Tier 1 requires no separate final proof.
    None,
    /// Tier 2 requires deterministic gate proof.
    DeterministicGates,
    /// Tier 3 requires final verification proof: exactly one committed PASS
    /// verification job bound to the session fingerprints. Under the
    /// incremental-testing strategy the verification scope is incremental;
    /// the full test suite runs only at release/distribution gates. The wire
    /// name stays `full_verification` for backward compatibility with
    /// persisted receipts and projections.
    #[serde(rename = "full_verification")]
    FinalVerification,
}

/// Outcome asserted by one daemon-attested reviewer contribution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewContributionOutcomeV2 {
    /// Reviewer completed with no findings.
    Clean,
    /// Reviewer completed with one or more findings.
    Findings,
    /// Reviewer execution failed for infrastructure reasons.
    InfraFailure,
    /// Reviewer output violated the contribution protocol.
    ProtocolFailure,
    /// Reviewer cancelled the attempt.
    Cancelled,
}

/// Terminal v2 session disposition.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewExitDispositionV2 {
    /// Review satisfied the complete PASS predicate.
    Pass,
    /// Review exhausted a budget.
    Stalled,
    /// Review was invalidated by input drift.
    Invalidated,
    /// Review was cancelled.
    Aborted,
}

fn validate_v2_identifier(value: &str, kind: &'static str) -> Result<(), ReviewContractError> {
    if value.is_empty() || value.len() > 128 {
        return Err(ReviewContractError::InvalidV2(kind));
    }
    if !value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric)
        || value
            .chars()
            .any(|character| !character.is_ascii_alphanumeric() && !"._:-".contains(character))
    {
        return Err(ReviewContractError::InvalidV2(kind));
    }
    Ok(())
}

macro_rules! review_v2_identifier {
    ($name:ident, $kind:literal) => {
        #[doc = concat!("Validated v2 ", $kind, ".")]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(Box<str>);

        impl $name {
            /// Validates and constructs the identifier.
            pub fn new(value: impl Into<Box<str>>) -> Result<Self, ReviewContractError> {
                let value = value.into();
                validate_v2_identifier(&value, $kind)?;
                Ok(Self(value))
            }

            /// Returns the canonical identifier.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }
    };
}

review_v2_identifier!(ReviewBatchId, "review batch id");
review_v2_identifier!(ReviewAttemptId, "review attempt id");
review_v2_identifier!(ReviewMutationId, "review mutation id");

/// Non-empty bounded authority reference carried by a v2 receipt.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ReviewAuthorityRef(Box<str>);

impl ReviewAuthorityRef {
    /// Maximum encoded reference length.
    pub const MAX_BYTES: usize = 1024;

    /// Validates and constructs a reference.
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, ReviewContractError> {
        let value = value.into();
        if value.is_empty() || value.len() > Self::MAX_BYTES || value.chars().any(char::is_control)
        {
            return Err(ReviewContractError::InvalidV2("review authority reference"));
        }
        Ok(Self(value))
    }

    /// Returns the reference text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ReviewAuthorityRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Canonical UTC RFC3339 timestamp used by the pure supervisor.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ReviewTimestamp(Box<str>);

impl ReviewTimestamp {
    /// Validates a canonical UTC timestamp (`YYYY-MM-DDTHH:MM:SS[.fraction]Z`).
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, ReviewContractError> {
        let value = value.into();
        timestamp_key(&value)?;
        Ok(Self(value))
    }

    /// Returns the canonical timestamp text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn key(&self) -> (u16, u8, u8, u8, u8, u8, u32) {
        timestamp_key(&self.0).expect("validated ReviewTimestamp")
    }
}

impl PartialOrd for ReviewTimestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ReviewTimestamp {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

impl<'de> Deserialize<'de> for ReviewTimestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[allow(clippy::cast_possible_truncation)]
fn timestamp_key(value: &str) -> Result<(u16, u8, u8, u8, u8, u8, u32), ReviewContractError> {
    let bytes = value.as_bytes();
    let shape = bytes.len() >= 20
        && bytes.len() <= 30
        && bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && bytes.get(10) == Some(&b'T')
        && bytes.get(13) == Some(&b':')
        && bytes.get(16) == Some(&b':')
        && bytes.last() == Some(&b'Z')
        && bytes[..19]
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7 | 10 | 13 | 16) || byte.is_ascii_digit())
        && (bytes.len() == 20
            || (bytes.get(19) == Some(&b'.')
                && bytes[20..bytes.len() - 1].len() <= 9
                && !bytes[20..bytes.len() - 1].is_empty()
                && bytes[20..bytes.len() - 1].iter().all(u8::is_ascii_digit)));
    if !shape {
        return Err(ReviewContractError::InvalidV2("review timestamp"));
    }
    let number = |range: std::ops::Range<usize>| -> Result<u32, ReviewContractError> {
        std::str::from_utf8(&bytes[range])
            .ok()
            .and_then(|raw| raw.parse().ok())
            .ok_or(ReviewContractError::InvalidV2("review timestamp"))
    };
    let year = number(0..4)? as u16;
    let month = number(5..7)? as u8;
    let day = number(8..10)? as u8;
    let hour = number(11..13)? as u8;
    let minute = number(14..16)? as u8;
    let second = number(17..19)? as u8;
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    if year == 0 || day == 0 || day > max_day || hour > 23 || minute > 59 || second > 59 {
        return Err(ReviewContractError::InvalidV2("review timestamp"));
    }
    let fraction = if bytes.len() == 20 {
        0
    } else {
        let digits = &value[20..value.len() - 1];
        let parsed: u32 = digits
            .parse()
            .map_err(|_| ReviewContractError::InvalidV2("review timestamp"))?;
        parsed * 10_u32.pow((9 - digits.len()) as u32)
    };
    Ok((year, month, day, hour, minute, second, fraction))
}

/// Bounded counters for one v2 review session.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "ReviewCountersV2Wire", into = "ReviewCountersV2Wire")]
pub struct ReviewCountersV2 {
    attempts: u32,
    valid_batches: u32,
    clean_streak: u32,
    remediations: u32,
    infra_failures: u32,
    protocol_failures: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewCountersV2Wire {
    attempts: u32,
    valid_batches: u32,
    clean_streak: u32,
    remediations: u32,
    infra_failures: u32,
    protocol_failures: u32,
}

impl ReviewCountersV2 {
    /// Zero-valued initial counters.
    pub const ZERO: Self = Self {
        attempts: 0,
        valid_batches: 0,
        clean_streak: 0,
        remediations: 0,
        infra_failures: 0,
        protocol_failures: 0,
    };

    /// Constructs a validated counter snapshot.
    pub fn new(
        attempts: u32,
        valid_batches: u32,
        clean_streak: u32,
        remediations: u32,
        infra_failures: u32,
        protocol_failures: u32,
    ) -> Result<Self, ReviewContractError> {
        let values = [
            attempts,
            valid_batches,
            clean_streak,
            remediations,
            infra_failures,
            protocol_failures,
        ];
        if values.into_iter().any(|value| value > MAX_REVIEW_V2_COUNT)
            || clean_streak > valid_batches
            || infra_failures.saturating_add(protocol_failures) > attempts
        {
            return Err(ReviewContractError::InvalidV2("review v2 counters"));
        }
        Ok(Self {
            attempts,
            valid_batches,
            clean_streak,
            remediations,
            infra_failures,
            protocol_failures,
        })
    }

    /// Advances counters for exactly one non-replayed attempt.
    pub fn after_attempt(self, status: ReviewBatchStatusV2) -> Result<Self, ReviewContractError> {
        let attempts = self
            .attempts
            .checked_add(1)
            .ok_or(ReviewContractError::InvalidV2(
                "review attempt counter overflow",
            ))?;
        let (valid_batches, clean_streak, infra_failures, protocol_failures) = match status {
            ReviewBatchStatusV2::ValidClean => (
                self.valid_batches
                    .checked_add(1)
                    .ok_or(ReviewContractError::InvalidV2(
                        "review valid batch counter overflow",
                    ))?,
                self.clean_streak
                    .checked_add(1)
                    .ok_or(ReviewContractError::InvalidV2(
                        "review clean streak counter overflow",
                    ))?,
                self.infra_failures,
                self.protocol_failures,
            ),
            ReviewBatchStatusV2::ValidFindings => (
                self.valid_batches
                    .checked_add(1)
                    .ok_or(ReviewContractError::InvalidV2(
                        "review valid batch counter overflow",
                    ))?,
                0,
                self.infra_failures,
                self.protocol_failures,
            ),
            ReviewBatchStatusV2::InvalidInfra => (
                self.valid_batches,
                self.clean_streak,
                self.infra_failures
                    .checked_add(1)
                    .ok_or(ReviewContractError::InvalidV2(
                        "review infra failure counter overflow",
                    ))?,
                self.protocol_failures,
            ),
            ReviewBatchStatusV2::InvalidProtocol => (
                self.valid_batches,
                self.clean_streak,
                self.infra_failures,
                self.protocol_failures
                    .checked_add(1)
                    .ok_or(ReviewContractError::InvalidV2(
                        "review protocol failure counter overflow",
                    ))?,
            ),
            ReviewBatchStatusV2::InvalidInputDrift | ReviewBatchStatusV2::Cancelled => (
                self.valid_batches,
                self.clean_streak,
                self.infra_failures,
                self.protocol_failures,
            ),
        };
        Self::new(
            attempts,
            valid_batches,
            clean_streak,
            self.remediations,
            infra_failures,
            protocol_failures,
        )
    }

    /// Returns attempts consumed.
    pub const fn attempts(self) -> u32 {
        self.attempts
    }

    /// Returns valid batches consumed.
    pub const fn valid_batches(self) -> u32 {
        self.valid_batches
    }

    /// Returns the consecutive clean-batch count.
    pub const fn clean_streak(self) -> u32 {
        self.clean_streak
    }

    /// Returns committed remediations.
    pub const fn remediations(self) -> u32 {
        self.remediations
    }

    /// Returns infrastructure failures.
    pub const fn infra_failures(self) -> u32 {
        self.infra_failures
    }

    /// Returns protocol failures.
    pub const fn protocol_failures(self) -> u32 {
        self.protocol_failures
    }
}

impl TryFrom<ReviewCountersV2Wire> for ReviewCountersV2 {
    type Error = ReviewContractError;

    fn try_from(value: ReviewCountersV2Wire) -> Result<Self, Self::Error> {
        Self::new(
            value.attempts,
            value.valid_batches,
            value.clean_streak,
            value.remediations,
            value.infra_failures,
            value.protocol_failures,
        )
    }
}

impl From<ReviewCountersV2> for ReviewCountersV2Wire {
    fn from(value: ReviewCountersV2) -> Self {
        Self {
            attempts: value.attempts,
            valid_batches: value.valid_batches,
            clean_streak: value.clean_streak,
            remediations: value.remediations,
            infra_failures: value.infra_failures,
            protocol_failures: value.protocol_failures,
        }
    }
}

/// Count and wall-clock budgets for one v2 review session.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "ReviewBudgetV2Wire", into = "ReviewBudgetV2Wire")]
pub struct ReviewBudgetV2 {
    max_attempts: u32,
    max_valid_batches: u32,
    max_remediations: u32,
    max_wall_clock_minutes: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewBudgetV2Wire {
    max_attempts: u32,
    max_valid_batches: u32,
    max_remediations: u32,
    max_wall_clock_minutes: u32,
}

impl ReviewBudgetV2 {
    /// Constructs a bounded v2 budget.
    pub fn new(
        max_attempts: u32,
        max_valid_batches: u32,
        max_remediations: u32,
        max_wall_clock_minutes: u32,
    ) -> Result<Self, ReviewContractError> {
        if !(1..=MAX_REVIEW_V2_COUNT).contains(&max_attempts)
            || !(1..=MAX_REVIEW_V2_COUNT).contains(&max_valid_batches)
            || !(1..=MAX_REVIEW_V2_COUNT).contains(&max_remediations)
            || !(1..=MAX_REVIEW_V2_WALL_CLOCK_MINUTES).contains(&max_wall_clock_minutes)
        {
            return Err(ReviewContractError::InvalidV2("review v2 budget"));
        }
        Ok(Self {
            max_attempts,
            max_valid_batches,
            max_remediations,
            max_wall_clock_minutes,
        })
    }

    /// Returns the default frozen budget for a tier.
    pub fn for_tier(tier: ReviewTier) -> Self {
        let values = match tier {
            ReviewTier::Tier1 => (3, 2, 1, 30),
            ReviewTier::Tier2 => (4, 3, 2, 60),
            ReviewTier::Tier3 => (6, 3, 2, 120),
        };
        Self::new(values.0, values.1, values.2, values.3).expect("frozen review budget is valid")
    }

    /// Returns the attempt cap.
    pub const fn max_attempts(self) -> u32 {
        self.max_attempts
    }

    /// Returns the valid-batch cap.
    pub const fn max_valid_batches(self) -> u32 {
        self.max_valid_batches
    }

    /// Returns the remediation cap.
    pub const fn max_remediations(self) -> u32 {
        self.max_remediations
    }

    /// Returns the wall-clock cap in minutes.
    pub const fn max_wall_clock_minutes(self) -> u32 {
        self.max_wall_clock_minutes
    }

    /// Returns whether the supplied counters have consumed a count budget.
    pub const fn count_exhausted(self, counters: ReviewCountersV2) -> bool {
        counters.attempts >= self.max_attempts
            || counters.valid_batches >= self.max_valid_batches
            || counters.remediations >= self.max_remediations
    }
}

impl TryFrom<ReviewBudgetV2Wire> for ReviewBudgetV2 {
    type Error = ReviewContractError;

    fn try_from(value: ReviewBudgetV2Wire) -> Result<Self, Self::Error> {
        Self::new(
            value.max_attempts,
            value.max_valid_batches,
            value.max_remediations,
            value.max_wall_clock_minutes,
        )
    }
}

impl From<ReviewBudgetV2> for ReviewBudgetV2Wire {
    fn from(value: ReviewBudgetV2) -> Self {
        Self {
            max_attempts: value.max_attempts,
            max_valid_batches: value.max_valid_batches,
            max_remediations: value.max_remediations,
            max_wall_clock_minutes: value.max_wall_clock_minutes,
        }
    }
}

/// Frozen clean-target and final-proof policy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewCleanPolicyV2 {
    clean_target: u32,
    final_proof_requirement: ReviewFinalProofKind,
}

/// One valid clean batch closes every review session.
///
/// The target is tier- and repair-class-independent: a single batch in which the
/// complete required reviewer set reports zero findings is the exit condition.
/// Risk is still bounded, but by the required reviewer set and by
/// `final_proof_requirement`, not by repeating an already-clean batch.
const CLEAN_TARGET: u32 = 1;

impl ReviewCleanPolicyV2 {
    /// Derives the exact clean policy from tier and daemon repair class.
    ///
    /// `repair_class` no longer raises the clean target; it stays in the
    /// signature because the frozen wire and projection contracts carry it and
    /// the daemon still records it as review provenance.
    pub const fn derive(tier: ReviewTier, _repair_class: ReviewRepairClass) -> Self {
        match tier {
            ReviewTier::Tier1 => Self {
                clean_target: CLEAN_TARGET,
                final_proof_requirement: ReviewFinalProofKind::None,
            },
            ReviewTier::Tier2 => Self {
                clean_target: CLEAN_TARGET,
                final_proof_requirement: ReviewFinalProofKind::DeterministicGates,
            },
            ReviewTier::Tier3 => Self {
                clean_target: CLEAN_TARGET,
                final_proof_requirement: ReviewFinalProofKind::FinalVerification,
            },
        }
    }

    /// Returns the consecutive clean-batch target.
    pub const fn clean_target(self) -> u32 {
        self.clean_target
    }

    /// Returns the required final proof kind.
    pub const fn final_proof_requirement(self) -> ReviewFinalProofKind {
        self.final_proof_requirement
    }
}

/// Returns the exact canonical specialty set for a tier.
#[must_use]
pub fn required_specialties_for_tier(tier: ReviewTier) -> Vec<ReviewerSpecialty> {
    match tier {
        ReviewTier::Tier1 => vec![ReviewerSpecialty::General],
        ReviewTier::Tier2 => vec![ReviewerSpecialty::Be, ReviewerSpecialty::Ar],
        ReviewTier::Tier3 => vec![
            ReviewerSpecialty::Be,
            ReviewerSpecialty::Ar,
            ReviewerSpecialty::Qa,
        ],
    }
}

mod optional_artifact_digest {
    use super::*;

    pub(super) fn serialize<S>(
        value: &Option<ArtifactDigest>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (*value)
            .map(|digest| digest.to_string())
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<ArtifactDigest>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| ArtifactDigest::from_str(&value).map_err(serde::de::Error::custom))
            .transpose()
    }
}

mod optional_input_fingerprint {
    use super::*;

    pub(super) fn serialize<S>(
        value: &Option<InputFingerprint>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (*value)
            .map(|fingerprint| fingerprint.to_string())
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<InputFingerprint>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| InputFingerprint::from_str(&value).map_err(serde::de::Error::custom))
            .transpose()
    }
}

/// Typed final proof bound to the current review inputs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "ReviewFinalProofV2Wire", into = "ReviewFinalProofV2Wire")]
pub struct ReviewFinalProofV2 {
    kind: ReviewFinalProofKind,
    #[serde(with = "optional_artifact_digest")]
    digest: Option<ArtifactDigest>,
    source_revision: Option<u64>,
    #[serde(with = "optional_input_fingerprint")]
    input_fingerprint: Option<InputFingerprint>,
    #[serde(with = "optional_input_fingerprint")]
    ruleset_fingerprint: Option<InputFingerprint>,
    observed_at: Option<ReviewTimestamp>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewFinalProofV2Wire {
    kind: ReviewFinalProofKind,
    #[serde(with = "optional_artifact_digest")]
    digest: Option<ArtifactDigest>,
    source_revision: Option<u64>,
    #[serde(with = "optional_input_fingerprint")]
    input_fingerprint: Option<InputFingerprint>,
    #[serde(with = "optional_input_fingerprint")]
    ruleset_fingerprint: Option<InputFingerprint>,
    observed_at: Option<ReviewTimestamp>,
}

impl ReviewFinalProofV2 {
    /// Constructs the explicit no-proof value used by Tier 1.
    pub const fn none() -> Self {
        Self {
            kind: ReviewFinalProofKind::None,
            digest: None,
            source_revision: None,
            input_fingerprint: None,
            ruleset_fingerprint: None,
            observed_at: None,
        }
    }

    /// Constructs a bound deterministic or final-verification proof.
    pub fn bound(
        kind: ReviewFinalProofKind,
        digest: ArtifactDigest,
        source_revision: u64,
        input_fingerprint: InputFingerprint,
        ruleset_fingerprint: InputFingerprint,
        observed_at: ReviewTimestamp,
    ) -> Result<Self, ReviewContractError> {
        if kind == ReviewFinalProofKind::None {
            return Err(ReviewContractError::InvalidV2("bound final proof kind"));
        }
        Ok(Self {
            kind,
            digest: Some(digest),
            source_revision: Some(source_revision),
            input_fingerprint: Some(input_fingerprint),
            ruleset_fingerprint: Some(ruleset_fingerprint),
            observed_at: Some(observed_at),
        })
    }

    /// Returns the proof kind.
    pub const fn kind(&self) -> ReviewFinalProofKind {
        self.kind
    }

    /// Returns the proof digest when present.
    pub const fn digest(&self) -> Option<ArtifactDigest> {
        self.digest
    }

    /// Returns the bound source revision.
    pub const fn source_revision(&self) -> Option<u64> {
        self.source_revision
    }

    /// Returns the bound input fingerprint.
    pub const fn input_fingerprint(&self) -> Option<InputFingerprint> {
        self.input_fingerprint
    }

    /// Returns the bound ruleset fingerprint.
    pub const fn ruleset_fingerprint(&self) -> Option<InputFingerprint> {
        self.ruleset_fingerprint
    }

    /// Returns the proof observation time.
    pub const fn observed_at(&self) -> Option<&ReviewTimestamp> {
        self.observed_at.as_ref()
    }
}

impl TryFrom<ReviewFinalProofV2Wire> for ReviewFinalProofV2 {
    type Error = ReviewContractError;

    fn try_from(value: ReviewFinalProofV2Wire) -> Result<Self, Self::Error> {
        let fields_present = value.digest.is_some()
            && value.source_revision.is_some()
            && value.input_fingerprint.is_some()
            && value.ruleset_fingerprint.is_some()
            && value.observed_at.is_some();
        let fields_absent = value.digest.is_none()
            && value.source_revision.is_none()
            && value.input_fingerprint.is_none()
            && value.ruleset_fingerprint.is_none()
            && value.observed_at.is_none();
        if (value.kind == ReviewFinalProofKind::None && !fields_absent)
            || (value.kind != ReviewFinalProofKind::None && !fields_present)
        {
            return Err(ReviewContractError::InvalidV2("review final proof"));
        }
        Ok(Self {
            kind: value.kind,
            digest: value.digest,
            source_revision: value.source_revision,
            input_fingerprint: value.input_fingerprint,
            ruleset_fingerprint: value.ruleset_fingerprint,
            observed_at: value.observed_at,
        })
    }
}

impl From<ReviewFinalProofV2> for ReviewFinalProofV2Wire {
    fn from(value: ReviewFinalProofV2) -> Self {
        Self {
            kind: value.kind,
            digest: value.digest,
            source_revision: value.source_revision,
            input_fingerprint: value.input_fingerprint,
            ruleset_fingerprint: value.ruleset_fingerprint,
            observed_at: value.observed_at,
        }
    }
}

/// Project-authority bindings required by review receipts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewProjectAuthorityV2 {
    project_receipt_ref: ReviewAuthorityRef,
    #[serde(with = "serde_domain::artifact_digest")]
    active_manifest_digest: ArtifactDigest,
    #[serde(with = "serde_domain::artifact_digest")]
    state_receipt_ref_digest: ArtifactDigest,
    journal_mutation_id: ReviewMutationId,
}

impl ReviewProjectAuthorityV2 {
    /// Constructs project-authority bindings.
    pub const fn new(
        project_receipt_ref: ReviewAuthorityRef,
        active_manifest_digest: ArtifactDigest,
        state_receipt_ref_digest: ArtifactDigest,
        journal_mutation_id: ReviewMutationId,
    ) -> Self {
        Self {
            project_receipt_ref,
            active_manifest_digest,
            state_receipt_ref_digest,
            journal_mutation_id,
        }
    }

    /// Returns the project receipt reference.
    pub const fn project_receipt_ref(&self) -> &ReviewAuthorityRef {
        &self.project_receipt_ref
    }
}

/// Daemon-attested physical reviewer identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "AttestedReviewerV2Wire", into = "AttestedReviewerV2Wire")]
pub struct AttestedReviewerV2 {
    #[serde(with = "serde_domain::agent_role")]
    agent_role: AgentRole,
    specialty: ReviewerSpecialty,
    granted_specialties: Vec<ReviewerSpecialty>,
    #[serde(with = "serde_domain::session_id")]
    physical_session_id: SessionId,
    #[serde(with = "serde_domain::session_id")]
    root_session_id: SessionId,
    #[serde(with = "serde_domain::delegation_id")]
    delegation_id: DelegationId,
    lineage_depth: u8,
    attestation_ref: ReviewAuthorityRef,
    #[serde(with = "serde_domain::artifact_digest")]
    attestation_digest: ArtifactDigest,
    #[serde(with = "serde_domain::artifact_digest")]
    specialty_grant_digest: ArtifactDigest,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AttestedReviewerV2Wire {
    #[serde(with = "serde_domain::agent_role")]
    agent_role: AgentRole,
    specialty: ReviewerSpecialty,
    granted_specialties: Vec<ReviewerSpecialty>,
    #[serde(with = "serde_domain::session_id")]
    physical_session_id: SessionId,
    #[serde(with = "serde_domain::session_id")]
    root_session_id: SessionId,
    #[serde(with = "serde_domain::delegation_id")]
    delegation_id: DelegationId,
    lineage_depth: u8,
    attestation_ref: ReviewAuthorityRef,
    #[serde(with = "serde_domain::artifact_digest")]
    attestation_digest: ArtifactDigest,
    #[serde(with = "serde_domain::artifact_digest")]
    specialty_grant_digest: ArtifactDigest,
}

impl AttestedReviewerV2 {
    /// Constructs and validates a reviewer identity.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        agent_role: AgentRole,
        specialty: ReviewerSpecialty,
        granted_specialties: Vec<ReviewerSpecialty>,
        physical_session_id: SessionId,
        root_session_id: SessionId,
        delegation_id: DelegationId,
        lineage_depth: u8,
        attestation_ref: ReviewAuthorityRef,
        attestation_digest: ArtifactDigest,
        specialty_grant_digest: ArtifactDigest,
    ) -> Result<Self, ReviewContractError> {
        if agent_role != AgentRole::Reviewer
            || lineage_depth != 2
            || physical_session_id == root_session_id
            || granted_specialties.as_slice() != [specialty]
        {
            return Err(ReviewContractError::InvalidV2("attested reviewer identity"));
        }
        Ok(Self {
            agent_role,
            specialty,
            granted_specialties,
            physical_session_id,
            root_session_id,
            delegation_id,
            lineage_depth,
            attestation_ref,
            attestation_digest,
            specialty_grant_digest,
        })
    }

    /// Returns the reviewer specialty.
    pub const fn specialty(&self) -> ReviewerSpecialty {
        self.specialty
    }

    /// Returns the physical reviewer session.
    pub const fn physical_session_id(&self) -> SessionId {
        self.physical_session_id
    }

    /// Returns the root session.
    pub const fn root_session_id(&self) -> SessionId {
        self.root_session_id
    }

    /// Returns the delegation identity.
    pub const fn delegation_id(&self) -> DelegationId {
        self.delegation_id
    }
}

impl TryFrom<AttestedReviewerV2Wire> for AttestedReviewerV2 {
    type Error = ReviewContractError;

    fn try_from(value: AttestedReviewerV2Wire) -> Result<Self, Self::Error> {
        Self::new(
            value.agent_role,
            value.specialty,
            value.granted_specialties,
            value.physical_session_id,
            value.root_session_id,
            value.delegation_id,
            value.lineage_depth,
            value.attestation_ref,
            value.attestation_digest,
            value.specialty_grant_digest,
        )
    }
}

impl From<AttestedReviewerV2> for AttestedReviewerV2Wire {
    fn from(value: AttestedReviewerV2) -> Self {
        Self {
            agent_role: value.agent_role,
            specialty: value.specialty,
            granted_specialties: value.granted_specialties,
            physical_session_id: value.physical_session_id,
            root_session_id: value.root_session_id,
            delegation_id: value.delegation_id,
            lineage_depth: value.lineage_depth,
            attestation_ref: value.attestation_ref,
            attestation_digest: value.attestation_digest,
            specialty_grant_digest: value.specialty_grant_digest,
        }
    }
}

/// One presented or retained reviewer contribution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    try_from = "ReviewerContributionV2Wire",
    into = "ReviewerContributionV2Wire"
)]
pub struct ReviewerContributionV2 {
    source_attempt_id: ReviewAttemptId,
    reviewer: AttestedReviewerV2,
    outcome: ReviewContributionOutcomeV2,
    findings: Vec<ReviewFinding>,
    #[serde(with = "serde_domain::artifact_digest")]
    report_digest: ArtifactDigest,
    #[serde(with = "serde_domain::artifact_digest")]
    contribution_digest: ArtifactDigest,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::input_fingerprint")]
    ruleset_fingerprint: InputFingerprint,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewerContributionV2Wire {
    source_attempt_id: ReviewAttemptId,
    reviewer: AttestedReviewerV2,
    outcome: ReviewContributionOutcomeV2,
    findings: Vec<ReviewFinding>,
    #[serde(with = "serde_domain::artifact_digest")]
    report_digest: ArtifactDigest,
    #[serde(with = "serde_domain::artifact_digest")]
    contribution_digest: ArtifactDigest,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::input_fingerprint")]
    ruleset_fingerprint: InputFingerprint,
}

impl ReviewerContributionV2 {
    /// Constructs a structurally valid contribution.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_attempt_id: ReviewAttemptId,
        reviewer: AttestedReviewerV2,
        outcome: ReviewContributionOutcomeV2,
        findings: Vec<ReviewFinding>,
        report_digest: ArtifactDigest,
        contribution_digest: ArtifactDigest,
        input_fingerprint: InputFingerprint,
        ruleset_fingerprint: InputFingerprint,
    ) -> Result<Self, ReviewContractError> {
        let valid_findings = findings.len() <= MAX_REVIEW_FINDINGS
            && match outcome {
                ReviewContributionOutcomeV2::Clean => findings.is_empty(),
                ReviewContributionOutcomeV2::Findings => !findings.is_empty(),
                ReviewContributionOutcomeV2::InfraFailure
                | ReviewContributionOutcomeV2::ProtocolFailure
                | ReviewContributionOutcomeV2::Cancelled => findings.is_empty(),
            };
        if !valid_findings {
            return Err(ReviewContractError::InvalidV2("reviewer contribution"));
        }
        Ok(Self {
            source_attempt_id,
            reviewer,
            outcome,
            findings,
            report_digest,
            contribution_digest,
            input_fingerprint,
            ruleset_fingerprint,
        })
    }

    /// Returns the source attempt identity.
    pub const fn source_attempt_id(&self) -> &ReviewAttemptId {
        &self.source_attempt_id
    }

    /// Returns the attested reviewer.
    pub const fn reviewer(&self) -> &AttestedReviewerV2 {
        &self.reviewer
    }

    /// Returns the contribution outcome.
    pub const fn outcome(&self) -> ReviewContributionOutcomeV2 {
        self.outcome
    }

    /// Returns findings in first-seen order.
    pub fn findings(&self) -> &[ReviewFinding] {
        &self.findings
    }

    /// Returns the input fingerprint observed by the reviewer.
    pub const fn input_fingerprint(&self) -> InputFingerprint {
        self.input_fingerprint
    }

    /// Returns the ruleset fingerprint observed by the reviewer.
    pub const fn ruleset_fingerprint(&self) -> InputFingerprint {
        self.ruleset_fingerprint
    }
}

impl TryFrom<ReviewerContributionV2Wire> for ReviewerContributionV2 {
    type Error = ReviewContractError;

    fn try_from(value: ReviewerContributionV2Wire) -> Result<Self, Self::Error> {
        Self::new(
            value.source_attempt_id,
            value.reviewer,
            value.outcome,
            value.findings,
            value.report_digest,
            value.contribution_digest,
            value.input_fingerprint,
            value.ruleset_fingerprint,
        )
    }
}

impl From<ReviewerContributionV2> for ReviewerContributionV2Wire {
    fn from(value: ReviewerContributionV2) -> Self {
        Self {
            source_attempt_id: value.source_attempt_id,
            reviewer: value.reviewer,
            outcome: value.outcome,
            findings: value.findings,
            report_digest: value.report_digest,
            contribution_digest: value.contribution_digest,
            input_fingerprint: value.input_fingerprint,
            ruleset_fingerprint: value.ruleset_fingerprint,
        }
    }
}

/// Idempotent committed remediation key.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewRemediationV2 {
    finding_batch_id: ReviewBatchId,
    #[serde(with = "serde_domain::input_fingerprint")]
    plan_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::input_fingerprint")]
    new_input_fingerprint: InputFingerprint,
    next_review_id: ReviewId,
    #[serde(with = "serde_domain::artifact_digest")]
    remediation_digest: ArtifactDigest,
}

impl ReviewRemediationV2 {
    /// Constructs the immutable key for one committed input-changing
    /// remediation. Cross-session invariants are checked by the authority
    /// adapter that owns the parent and next review sessions.
    pub const fn new(
        finding_batch_id: ReviewBatchId,
        plan_fingerprint: InputFingerprint,
        new_input_fingerprint: InputFingerprint,
        next_review_id: ReviewId,
        remediation_digest: ArtifactDigest,
    ) -> Self {
        Self {
            finding_batch_id,
            plan_fingerprint,
            new_input_fingerprint,
            next_review_id,
            remediation_digest,
        }
    }

    /// Returns the findings batch remediated by this key.
    pub const fn finding_batch_id(&self) -> &ReviewBatchId {
        &self.finding_batch_id
    }

    /// Returns the committed remediation-plan fingerprint.
    pub const fn plan_fingerprint(&self) -> InputFingerprint {
        self.plan_fingerprint
    }

    /// Returns the post-remediation Review input fingerprint.
    pub const fn new_input_fingerprint(&self) -> InputFingerprint {
        self.new_input_fingerprint
    }

    /// Returns the child Review session created for the repaired input.
    pub const fn next_review_id(&self) -> &ReviewId {
        &self.next_review_id
    }

    /// Returns the canonical remediation digest.
    pub const fn remediation_digest(&self) -> ArtifactDigest {
        self.remediation_digest
    }
}

/// One authenticated v2 review attempt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "ReviewAttemptV2Wire", into = "ReviewAttemptV2Wire")]
pub struct ReviewAttemptV2 {
    schema_version: ReviewSchemaVersion,
    review_id: ReviewId,
    batch_id: ReviewBatchId,
    attempt_id: ReviewAttemptId,
    attempt_ordinal: u32,
    idempotency_key: crate::IdempotencyKey,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::input_fingerprint")]
    ruleset_fingerprint: InputFingerprint,
    contributions: Vec<ReviewerContributionV2>,
    observed_at: ReviewTimestamp,
    final_proof: ReviewFinalProofV2,
    project_authority: ReviewProjectAuthorityV2,
    remediation: Option<ReviewRemediationV2>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewAttemptV2Wire {
    schema_version: ReviewSchemaVersion,
    review_id: ReviewId,
    batch_id: ReviewBatchId,
    attempt_id: ReviewAttemptId,
    attempt_ordinal: u32,
    idempotency_key: crate::IdempotencyKey,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::input_fingerprint")]
    ruleset_fingerprint: InputFingerprint,
    contributions: Vec<ReviewerContributionV2>,
    observed_at: ReviewTimestamp,
    final_proof: ReviewFinalProofV2,
    project_authority: ReviewProjectAuthorityV2,
    remediation: Option<ReviewRemediationV2>,
}

impl ReviewAttemptV2 {
    /// Constructs a bounded authenticated attempt.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        review_id: ReviewId,
        batch_id: ReviewBatchId,
        attempt_id: ReviewAttemptId,
        attempt_ordinal: u32,
        idempotency_key: crate::IdempotencyKey,
        input_fingerprint: InputFingerprint,
        ruleset_fingerprint: InputFingerprint,
        contributions: Vec<ReviewerContributionV2>,
        observed_at: ReviewTimestamp,
        final_proof: ReviewFinalProofV2,
        project_authority: ReviewProjectAuthorityV2,
        remediation: Option<ReviewRemediationV2>,
    ) -> Result<Self, ReviewContractError> {
        if !(1..=MAX_REVIEW_V2_COUNT).contains(&attempt_ordinal)
            || !(1..=MAX_REVIEW_V2_ATTEMPT_CONTRIBUTIONS).contains(&contributions.len())
            || contributions
                .iter()
                .any(|item| item.source_attempt_id() != &attempt_id)
        {
            return Err(ReviewContractError::InvalidV2("review v2 attempt"));
        }
        Ok(Self {
            schema_version: ReviewSchemaVersion::V2,
            review_id,
            batch_id,
            attempt_id,
            attempt_ordinal,
            idempotency_key,
            input_fingerprint,
            ruleset_fingerprint,
            contributions,
            observed_at,
            final_proof,
            project_authority,
            remediation,
        })
    }

    /// Returns the owning review ID.
    pub const fn review_id(&self) -> &ReviewId {
        &self.review_id
    }

    /// Returns the batch ID.
    pub const fn batch_id(&self) -> &ReviewBatchId {
        &self.batch_id
    }

    /// Returns the attempt ID.
    pub const fn attempt_id(&self) -> &ReviewAttemptId {
        &self.attempt_id
    }

    /// Returns the global session attempt ordinal.
    pub const fn attempt_ordinal(&self) -> u32 {
        self.attempt_ordinal
    }

    /// Returns the idempotency key.
    pub const fn idempotency_key(&self) -> &crate::IdempotencyKey {
        &self.idempotency_key
    }

    /// Returns the attempt input fingerprint.
    pub const fn input_fingerprint(&self) -> InputFingerprint {
        self.input_fingerprint
    }

    /// Returns the attempt ruleset fingerprint.
    pub const fn ruleset_fingerprint(&self) -> InputFingerprint {
        self.ruleset_fingerprint
    }

    /// Returns presented contributions.
    pub fn contributions(&self) -> &[ReviewerContributionV2] {
        &self.contributions
    }

    /// Returns the trusted observation time.
    pub const fn observed_at(&self) -> &ReviewTimestamp {
        &self.observed_at
    }

    /// Returns the final proof supplied by the adapter.
    pub const fn final_proof(&self) -> &ReviewFinalProofV2 {
        &self.final_proof
    }

    /// Returns project-authority bindings.
    pub const fn project_authority(&self) -> &ReviewProjectAuthorityV2 {
        &self.project_authority
    }

    /// Returns the committed remediation carried by the first child-session
    /// attempt, when this attempt follows a findings batch.
    pub const fn remediation(&self) -> Option<&ReviewRemediationV2> {
        self.remediation.as_ref()
    }
}

impl TryFrom<ReviewAttemptV2Wire> for ReviewAttemptV2 {
    type Error = ReviewContractError;

    fn try_from(value: ReviewAttemptV2Wire) -> Result<Self, Self::Error> {
        if value.schema_version != ReviewSchemaVersion::V2 {
            return Err(ReviewContractError::InvalidV2("review schema version"));
        }
        Self::new(
            value.review_id,
            value.batch_id,
            value.attempt_id,
            value.attempt_ordinal,
            value.idempotency_key,
            value.input_fingerprint,
            value.ruleset_fingerprint,
            value.contributions,
            value.observed_at,
            value.final_proof,
            value.project_authority,
            value.remediation,
        )
    }
}

impl From<ReviewAttemptV2> for ReviewAttemptV2Wire {
    fn from(value: ReviewAttemptV2) -> Self {
        Self {
            schema_version: value.schema_version,
            review_id: value.review_id,
            batch_id: value.batch_id,
            attempt_id: value.attempt_id,
            attempt_ordinal: value.attempt_ordinal,
            idempotency_key: value.idempotency_key,
            input_fingerprint: value.input_fingerprint,
            ruleset_fingerprint: value.ruleset_fingerprint,
            contributions: value.contributions,
            observed_at: value.observed_at,
            final_proof: value.final_proof,
            project_authority: value.project_authority,
            remediation: value.remediation,
        }
    }
}

/// Authoritative Review Batch v2 session aggregate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "ReviewSessionV2Wire", into = "ReviewSessionV2Wire")]
pub struct ReviewSessionV2 {
    schema_version: ReviewSchemaVersion,
    review_id: ReviewId,
    parent_review_id: Option<ReviewId>,
    tier: ReviewTier,
    required_specialties: Vec<ReviewerSpecialty>,
    #[serde(with = "serde_domain::session_id")]
    author_session_id: SessionId,
    #[serde(with = "serde_domain::session_id")]
    root_session_id: SessionId,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::input_fingerprint")]
    ruleset_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::policy_digest")]
    policy_digest: PolicyDigest,
    source_revision: u64,
    inventory_generation: u64,
    repair_class: ReviewRepairClass,
    clean_policy: ReviewCleanPolicyV2,
    budget: ReviewBudgetV2,
    counters: ReviewCountersV2,
    status: ReviewSessionStatusV2,
    started_at: ReviewTimestamp,
    deadline_at: ReviewTimestamp,
    terminal_at: Option<ReviewTimestamp>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewSessionV2Wire {
    schema_version: ReviewSchemaVersion,
    review_id: ReviewId,
    parent_review_id: Option<ReviewId>,
    tier: ReviewTier,
    required_specialties: Vec<ReviewerSpecialty>,
    #[serde(with = "serde_domain::session_id")]
    author_session_id: SessionId,
    #[serde(with = "serde_domain::session_id")]
    root_session_id: SessionId,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::input_fingerprint")]
    ruleset_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::policy_digest")]
    policy_digest: PolicyDigest,
    source_revision: u64,
    inventory_generation: u64,
    repair_class: ReviewRepairClass,
    clean_policy: ReviewCleanPolicyV2,
    budget: ReviewBudgetV2,
    counters: ReviewCountersV2,
    status: ReviewSessionStatusV2,
    started_at: ReviewTimestamp,
    deadline_at: ReviewTimestamp,
    terminal_at: Option<ReviewTimestamp>,
}

impl ReviewSessionV2 {
    /// Constructs an initial running v2 session with derived policy.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        review_id: ReviewId,
        parent_review_id: Option<ReviewId>,
        tier: ReviewTier,
        author_session_id: SessionId,
        root_session_id: SessionId,
        input_fingerprint: InputFingerprint,
        ruleset_fingerprint: InputFingerprint,
        policy_digest: PolicyDigest,
        source_revision: u64,
        inventory_generation: u64,
        repair_class: ReviewRepairClass,
        budget: ReviewBudgetV2,
        started_at: ReviewTimestamp,
        deadline_at: ReviewTimestamp,
    ) -> Result<Self, ReviewContractError> {
        Self::from_parts(
            ReviewSchemaVersion::V2,
            review_id,
            parent_review_id,
            tier,
            required_specialties_for_tier(tier),
            author_session_id,
            root_session_id,
            input_fingerprint,
            ruleset_fingerprint,
            policy_digest,
            source_revision,
            inventory_generation,
            repair_class,
            ReviewCleanPolicyV2::derive(tier, repair_class),
            budget,
            ReviewCountersV2::ZERO,
            ReviewSessionStatusV2::Running,
            started_at,
            deadline_at,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        schema_version: ReviewSchemaVersion,
        review_id: ReviewId,
        parent_review_id: Option<ReviewId>,
        tier: ReviewTier,
        required_specialties: Vec<ReviewerSpecialty>,
        author_session_id: SessionId,
        root_session_id: SessionId,
        input_fingerprint: InputFingerprint,
        ruleset_fingerprint: InputFingerprint,
        policy_digest: PolicyDigest,
        source_revision: u64,
        inventory_generation: u64,
        repair_class: ReviewRepairClass,
        clean_policy: ReviewCleanPolicyV2,
        budget: ReviewBudgetV2,
        counters: ReviewCountersV2,
        status: ReviewSessionStatusV2,
        started_at: ReviewTimestamp,
        deadline_at: ReviewTimestamp,
        terminal_at: Option<ReviewTimestamp>,
    ) -> Result<Self, ReviewContractError> {
        if schema_version != ReviewSchemaVersion::V2
            || parent_review_id.as_ref() == Some(&review_id)
            || required_specialties != required_specialties_for_tier(tier)
            || clean_policy != ReviewCleanPolicyV2::derive(tier, repair_class)
            || started_at >= deadline_at
            || status.is_terminal() != terminal_at.is_some()
            || counters.attempts() > budget.max_attempts()
            || counters.valid_batches() > budget.max_valid_batches()
            || counters.remediations() > budget.max_remediations()
            || (status == ReviewSessionStatusV2::Completed
                && counters.clean_streak() < clean_policy.clean_target())
        {
            return Err(ReviewContractError::InvalidV2("review v2 session"));
        }
        if terminal_at
            .as_ref()
            .is_some_and(|terminal| terminal < &started_at)
        {
            return Err(ReviewContractError::InvalidV2("review terminal timestamp"));
        }
        Ok(Self {
            schema_version,
            review_id,
            parent_review_id,
            tier,
            required_specialties,
            author_session_id,
            root_session_id,
            input_fingerprint,
            ruleset_fingerprint,
            policy_digest,
            source_revision,
            inventory_generation,
            repair_class,
            clean_policy,
            budget,
            counters,
            status,
            started_at,
            deadline_at,
            terminal_at,
        })
    }

    /// Produces the next validated status/counter snapshot.
    pub fn transition(
        &self,
        counters: ReviewCountersV2,
        status: ReviewSessionStatusV2,
        terminal_at: Option<ReviewTimestamp>,
    ) -> Result<Self, ReviewContractError> {
        Self::from_parts(
            self.schema_version,
            self.review_id.clone(),
            self.parent_review_id.clone(),
            self.tier,
            self.required_specialties.clone(),
            self.author_session_id,
            self.root_session_id,
            self.input_fingerprint,
            self.ruleset_fingerprint,
            self.policy_digest,
            self.source_revision,
            self.inventory_generation,
            self.repair_class,
            self.clean_policy,
            self.budget,
            counters,
            status,
            self.started_at.clone(),
            self.deadline_at.clone(),
            terminal_at,
        )
    }

    /// Returns the review ID.
    pub const fn review_id(&self) -> &ReviewId {
        &self.review_id
    }

    /// Returns the parent Review session when this session was created after
    /// input/ruleset drift.
    pub const fn parent_review_id(&self) -> Option<&ReviewId> {
        self.parent_review_id.as_ref()
    }

    /// Returns the tier.
    pub const fn tier(&self) -> ReviewTier {
        self.tier
    }

    /// Returns exact required specialties.
    pub fn required_specialties(&self) -> &[ReviewerSpecialty] {
        &self.required_specialties
    }

    /// Returns the author session.
    pub const fn author_session_id(&self) -> SessionId {
        self.author_session_id
    }

    /// Returns the root session.
    pub const fn root_session_id(&self) -> SessionId {
        self.root_session_id
    }

    /// Returns the current input fingerprint.
    pub const fn input_fingerprint(&self) -> InputFingerprint {
        self.input_fingerprint
    }

    /// Returns the current ruleset fingerprint.
    pub const fn ruleset_fingerprint(&self) -> InputFingerprint {
        self.ruleset_fingerprint
    }

    /// Returns the policy digest.
    pub const fn policy_digest(&self) -> PolicyDigest {
        self.policy_digest
    }

    /// Returns the source revision.
    pub const fn source_revision(&self) -> u64 {
        self.source_revision
    }

    /// Returns the inventory generation.
    pub const fn inventory_generation(&self) -> u64 {
        self.inventory_generation
    }

    /// Returns the clean policy.
    pub const fn clean_policy(&self) -> ReviewCleanPolicyV2 {
        self.clean_policy
    }

    /// Returns the budget.
    pub const fn budget(&self) -> ReviewBudgetV2 {
        self.budget
    }

    /// Returns the current counters.
    pub const fn counters(&self) -> ReviewCountersV2 {
        self.counters
    }

    /// Returns the current status.
    pub const fn status(&self) -> ReviewSessionStatusV2 {
        self.status
    }

    /// Returns the deadline.
    pub const fn deadline_at(&self) -> &ReviewTimestamp {
        &self.deadline_at
    }
}

impl TryFrom<ReviewSessionV2Wire> for ReviewSessionV2 {
    type Error = ReviewContractError;

    fn try_from(value: ReviewSessionV2Wire) -> Result<Self, Self::Error> {
        Self::from_parts(
            value.schema_version,
            value.review_id,
            value.parent_review_id,
            value.tier,
            value.required_specialties,
            value.author_session_id,
            value.root_session_id,
            value.input_fingerprint,
            value.ruleset_fingerprint,
            value.policy_digest,
            value.source_revision,
            value.inventory_generation,
            value.repair_class,
            value.clean_policy,
            value.budget,
            value.counters,
            value.status,
            value.started_at,
            value.deadline_at,
            value.terminal_at,
        )
    }
}

impl From<ReviewSessionV2> for ReviewSessionV2Wire {
    fn from(value: ReviewSessionV2) -> Self {
        Self {
            schema_version: value.schema_version,
            review_id: value.review_id,
            parent_review_id: value.parent_review_id,
            tier: value.tier,
            required_specialties: value.required_specialties,
            author_session_id: value.author_session_id,
            root_session_id: value.root_session_id,
            input_fingerprint: value.input_fingerprint,
            ruleset_fingerprint: value.ruleset_fingerprint,
            policy_digest: value.policy_digest,
            source_revision: value.source_revision,
            inventory_generation: value.inventory_generation,
            repair_class: value.repair_class,
            clean_policy: value.clean_policy,
            budget: value.budget,
            counters: value.counters,
            status: value.status,
            started_at: value.started_at,
            deadline_at: value.deadline_at,
            terminal_at: value.terminal_at,
        }
    }
}

mod artifact_digests {
    use super::*;

    pub(super) fn serialize<S>(values: &[ArtifactDigest], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<ArtifactDigest>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Vec::<String>::deserialize(deserializer)?
            .into_iter()
            .map(|value| ArtifactDigest::from_str(&value).map_err(serde::de::Error::custom))
            .collect()
    }
}

/// Canonical receipt for one evaluated attempt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewBatchReceiptV2 {
    schema_version: ReviewSchemaVersion,
    review_id: ReviewId,
    batch_id: ReviewBatchId,
    attempt_id: ReviewAttemptId,
    attempt_ordinal: u32,
    idempotency_key: crate::IdempotencyKey,
    #[serde(with = "serde_domain::artifact_digest")]
    attempt_digest: ArtifactDigest,
    status: ReviewBatchStatusV2,
    required_specialties: Vec<ReviewerSpecialty>,
    completed_specialties: Vec<ReviewerSpecialty>,
    counters: ReviewCountersV2,
    #[serde(with = "artifact_digests")]
    finding_fingerprints: Vec<ArtifactDigest>,
    #[serde(with = "optional_artifact_digest")]
    zero_finding_digest: Option<ArtifactDigest>,
    final_proof: ReviewFinalProofV2,
    project_authority: ReviewProjectAuthorityV2,
    observed_at: ReviewTimestamp,
    #[serde(with = "serde_domain::artifact_digest")]
    receipt_digest: ArtifactDigest,
}

impl ReviewBatchReceiptV2 {
    /// Constructs a validated attempt receipt.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        review_id: ReviewId,
        batch_id: ReviewBatchId,
        attempt_id: ReviewAttemptId,
        attempt_ordinal: u32,
        idempotency_key: crate::IdempotencyKey,
        attempt_digest: ArtifactDigest,
        status: ReviewBatchStatusV2,
        required_specialties: Vec<ReviewerSpecialty>,
        completed_specialties: Vec<ReviewerSpecialty>,
        counters: ReviewCountersV2,
        finding_fingerprints: Vec<ArtifactDigest>,
        zero_finding_digest: Option<ArtifactDigest>,
        final_proof: ReviewFinalProofV2,
        project_authority: ReviewProjectAuthorityV2,
        observed_at: ReviewTimestamp,
        receipt_digest: ArtifactDigest,
    ) -> Result<Self, ReviewContractError> {
        let required: BTreeSet<_> = required_specialties.iter().copied().collect();
        let completed: BTreeSet<_> = completed_specialties.iter().copied().collect();
        if !(1..=3).contains(&required.len())
            || required.len() != required_specialties.len()
            || completed.len() != completed_specialties.len()
            || !completed.is_subset(&required)
            || finding_fingerprints.len() > MAX_REVIEW_FINDINGS
            || !(1..=MAX_REVIEW_V2_COUNT).contains(&attempt_ordinal)
            || (status == ReviewBatchStatusV2::ValidClean
                && (!finding_fingerprints.is_empty()
                    || completed != required
                    || zero_finding_digest.is_none()))
            || (status == ReviewBatchStatusV2::ValidFindings
                && (finding_fingerprints.is_empty() || completed != required))
            || (status != ReviewBatchStatusV2::ValidClean && zero_finding_digest.is_some())
        {
            return Err(ReviewContractError::InvalidV2("review batch receipt"));
        }
        Ok(Self {
            schema_version: ReviewSchemaVersion::V2,
            review_id,
            batch_id,
            attempt_id,
            attempt_ordinal,
            idempotency_key,
            attempt_digest,
            status,
            required_specialties,
            completed_specialties,
            counters,
            finding_fingerprints,
            zero_finding_digest,
            final_proof,
            project_authority,
            observed_at,
            receipt_digest,
        })
    }

    /// Returns the attempt ID.
    pub const fn attempt_id(&self) -> &ReviewAttemptId {
        &self.attempt_id
    }

    /// Returns the owning review ID.
    pub const fn review_id(&self) -> &ReviewId {
        &self.review_id
    }

    /// Returns the owning batch ID.
    pub const fn batch_id(&self) -> &ReviewBatchId {
        &self.batch_id
    }

    /// Returns the idempotency key.
    pub const fn idempotency_key(&self) -> &crate::IdempotencyKey {
        &self.idempotency_key
    }

    /// Returns the canonical attempt digest.
    pub const fn attempt_digest(&self) -> ArtifactDigest {
        self.attempt_digest
    }

    /// Returns the evaluated status.
    pub const fn status(&self) -> ReviewBatchStatusV2 {
        self.status
    }

    /// Returns the counter snapshot after the attempt.
    pub const fn counters(&self) -> ReviewCountersV2 {
        self.counters
    }

    /// Returns the receipt digest.
    pub const fn receipt_digest(&self) -> ArtifactDigest {
        self.receipt_digest
    }

    /// Returns completed specialties.
    pub fn completed_specialties(&self) -> &[ReviewerSpecialty] {
        &self.completed_specialties
    }

    /// Returns finding fingerprints.
    pub fn finding_fingerprints(&self) -> &[ArtifactDigest] {
        &self.finding_fingerprints
    }
}

/// Terminal v2 exit receipt consumed by Review Gates.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewExitReceiptV2 {
    schema_version: ReviewSchemaVersion,
    review_id: ReviewId,
    tier: ReviewTier,
    session_status: ReviewSessionStatusV2,
    disposition: ReviewExitDispositionV2,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::input_fingerprint")]
    observed_input_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::input_fingerprint")]
    ruleset_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::policy_digest")]
    policy_digest: PolicyDigest,
    source_revision: u64,
    inventory_generation: u64,
    required_specialties: Vec<ReviewerSpecialty>,
    completed_specialties: Vec<ReviewerSpecialty>,
    finding_count: u32,
    counters: ReviewCountersV2,
    clean_target: u32,
    last_batch_id: ReviewBatchId,
    last_attempt_id: ReviewAttemptId,
    last_attempt_status: ReviewBatchStatusV2,
    #[serde(with = "optional_artifact_digest")]
    zero_finding_batch_receipt_digest: Option<ArtifactDigest>,
    final_proof: ReviewFinalProofV2,
    project_authority: ReviewProjectAuthorityV2,
    created_at: ReviewTimestamp,
    #[serde(with = "serde_domain::artifact_digest")]
    receipt_digest: ArtifactDigest,
}

impl ReviewExitReceiptV2 {
    /// Constructs a terminal exit receipt and validates the PASS predicate.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session: &ReviewSessionV2,
        disposition: ReviewExitDispositionV2,
        observed_input_fingerprint: InputFingerprint,
        completed_specialties: Vec<ReviewerSpecialty>,
        finding_count: u32,
        last_batch_id: ReviewBatchId,
        last_attempt_id: ReviewAttemptId,
        last_attempt_status: ReviewBatchStatusV2,
        zero_finding_batch_receipt_digest: Option<ArtifactDigest>,
        final_proof: ReviewFinalProofV2,
        project_authority: ReviewProjectAuthorityV2,
        created_at: ReviewTimestamp,
        receipt_digest: ArtifactDigest,
    ) -> Result<Self, ReviewContractError> {
        let expected = match disposition {
            ReviewExitDispositionV2::Pass => ReviewSessionStatusV2::Completed,
            ReviewExitDispositionV2::Stalled => ReviewSessionStatusV2::Stalled,
            ReviewExitDispositionV2::Invalidated => ReviewSessionStatusV2::Invalidated,
            ReviewExitDispositionV2::Aborted => ReviewSessionStatusV2::Aborted,
        };
        if session.status() != expected
            || finding_count > MAX_REVIEW_FINDINGS as u32
            || (disposition == ReviewExitDispositionV2::Pass
                && (last_attempt_status != ReviewBatchStatusV2::ValidClean
                    || observed_input_fingerprint != session.input_fingerprint()
                    || finding_count != 0
                    || completed_specialties != session.required_specialties()
                    || session.counters().clean_streak() < session.clean_policy().clean_target()
                    || zero_finding_batch_receipt_digest.is_none()
                    || final_proof.kind() != session.clean_policy().final_proof_requirement()))
            || (disposition != ReviewExitDispositionV2::Pass
                && zero_finding_batch_receipt_digest.is_some())
        {
            return Err(ReviewContractError::InvalidV2("review exit receipt"));
        }
        Ok(Self {
            schema_version: ReviewSchemaVersion::V2,
            review_id: session.review_id().clone(),
            tier: session.tier(),
            session_status: session.status(),
            disposition,
            input_fingerprint: session.input_fingerprint(),
            observed_input_fingerprint,
            ruleset_fingerprint: session.ruleset_fingerprint(),
            policy_digest: session.policy_digest(),
            source_revision: session.source_revision(),
            inventory_generation: session.inventory_generation(),
            required_specialties: session.required_specialties().to_vec(),
            completed_specialties,
            finding_count,
            counters: session.counters(),
            clean_target: session.clean_policy().clean_target(),
            last_batch_id,
            last_attempt_id,
            last_attempt_status,
            zero_finding_batch_receipt_digest,
            final_proof,
            project_authority,
            created_at,
            receipt_digest,
        })
    }

    /// Returns true only for a fully validated v2 PASS.
    pub const fn is_pass(&self) -> bool {
        matches!(self.disposition, ReviewExitDispositionV2::Pass)
    }

    /// Returns true only when every PASS field is bound to the supplied
    /// authoritative session. This is intentionally stronger than `is_pass`.
    pub fn valid_pass_for(&self, session: &ReviewSessionV2) -> bool {
        self.disposition == ReviewExitDispositionV2::Pass
            && self.review_id == *session.review_id()
            && self.tier == session.tier()
            && self.session_status == ReviewSessionStatusV2::Completed
            && session.status() == ReviewSessionStatusV2::Completed
            && self.input_fingerprint == session.input_fingerprint()
            && self.observed_input_fingerprint == session.input_fingerprint()
            && self.ruleset_fingerprint == session.ruleset_fingerprint()
            && self.policy_digest == session.policy_digest()
            && self.source_revision == session.source_revision()
            && self.inventory_generation == session.inventory_generation()
            && self.required_specialties == session.required_specialties()
            && self.completed_specialties == session.required_specialties()
            && self.finding_count == 0
            && self.counters == session.counters()
            && self.clean_target == session.clean_policy().clean_target()
            && self.counters.clean_streak() >= self.clean_target
            && self.last_attempt_status == ReviewBatchStatusV2::ValidClean
            && self.zero_finding_batch_receipt_digest.is_some()
            && self.final_proof.kind() == session.clean_policy().final_proof_requirement()
            && match self.final_proof.kind() {
                ReviewFinalProofKind::None => true,
                ReviewFinalProofKind::DeterministicGates
                | ReviewFinalProofKind::FinalVerification => {
                    self.final_proof.digest().is_some()
                        && self.final_proof.source_revision() == Some(session.source_revision())
                        && self.final_proof.input_fingerprint() == Some(session.input_fingerprint())
                        && self.final_proof.ruleset_fingerprint()
                            == Some(session.ruleset_fingerprint())
                        && self
                            .final_proof
                            .observed_at()
                            .is_some_and(|observed| observed <= &self.created_at)
                }
            }
    }

    /// Returns the review ID.
    pub const fn review_id(&self) -> &ReviewId {
        &self.review_id
    }

    /// Returns the inventory generation bound to the receipt.
    pub const fn inventory_generation(&self) -> u64 {
        self.inventory_generation
    }

    /// Returns the zero-finding batch receipt digest.
    pub const fn zero_finding_batch_receipt_digest(&self) -> Option<ArtifactDigest> {
        self.zero_finding_batch_receipt_digest
    }

    /// Returns the final batch ID.
    pub const fn last_batch_id(&self) -> &ReviewBatchId {
        &self.last_batch_id
    }

    /// Returns the final attempt ID.
    pub const fn last_attempt_id(&self) -> &ReviewAttemptId {
        &self.last_attempt_id
    }

    /// Returns the session status.
    pub const fn session_status(&self) -> ReviewSessionStatusV2 {
        self.session_status
    }

    /// Returns the input fingerprint bound to the receipt.
    pub const fn input_fingerprint(&self) -> InputFingerprint {
        self.input_fingerprint
    }

    /// Returns the ruleset fingerprint bound to the receipt.
    pub const fn ruleset_fingerprint(&self) -> InputFingerprint {
        self.ruleset_fingerprint
    }

    /// Returns the source revision bound to the receipt.
    pub const fn source_revision(&self) -> u64 {
        self.source_revision
    }

    /// Returns the policy digest bound to the receipt.
    pub const fn policy_digest(&self) -> PolicyDigest {
        self.policy_digest
    }

    /// Returns the receipt digest.
    pub const fn receipt_digest(&self) -> ArtifactDigest {
        self.receipt_digest
    }

    /// Returns exact required specialties.
    pub fn required_specialties(&self) -> &[ReviewerSpecialty] {
        &self.required_specialties
    }

    /// Returns exact completed specialties.
    pub fn completed_specialties(&self) -> &[ReviewerSpecialty] {
        &self.completed_specialties
    }

    /// Returns the final proof.
    pub const fn final_proof(&self) -> &ReviewFinalProofV2 {
        &self.final_proof
    }
}

/// Aggregate state of one v2 batch across one or more attempts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewBatchV2 {
    schema_version: ReviewSchemaVersion,
    review_id: ReviewId,
    batch_id: ReviewBatchId,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::input_fingerprint")]
    ruleset_fingerprint: InputFingerprint,
    latest_attempt_id: ReviewAttemptId,
    latest_status: ReviewBatchStatusV2,
    retained_contributions: Vec<ReviewerContributionV2>,
    #[serde(with = "artifact_digests")]
    finding_fingerprints: Vec<ArtifactDigest>,
    latest_receipt: ReviewBatchReceiptV2,
    attempt_receipts: Vec<ReviewBatchReceiptV2>,
    closed: bool,
    valid_batch_ordinal: Option<u32>,
    exit_receipt: Option<ReviewExitReceiptV2>,
}

impl ReviewBatchV2 {
    /// Constructs the next validated batch aggregate.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        review_id: ReviewId,
        batch_id: ReviewBatchId,
        input_fingerprint: InputFingerprint,
        ruleset_fingerprint: InputFingerprint,
        latest_attempt_id: ReviewAttemptId,
        latest_status: ReviewBatchStatusV2,
        retained_contributions: Vec<ReviewerContributionV2>,
        finding_fingerprints: Vec<ArtifactDigest>,
        latest_receipt: ReviewBatchReceiptV2,
        attempt_receipts: Vec<ReviewBatchReceiptV2>,
        valid_batch_ordinal: Option<u32>,
        exit_receipt: Option<ReviewExitReceiptV2>,
    ) -> Result<Self, ReviewContractError> {
        let specialties: BTreeSet<_> = retained_contributions
            .iter()
            .map(|item| item.reviewer().specialty())
            .collect();
        let sessions: BTreeSet<_> = retained_contributions
            .iter()
            .map(|item| item.reviewer().physical_session_id())
            .collect();
        let delegations: BTreeSet<_> = retained_contributions
            .iter()
            .map(|item| item.reviewer().delegation_id())
            .collect();
        if retained_contributions.len() > 3
            || specialties.len() != retained_contributions.len()
            || sessions.len() != retained_contributions.len()
            || delegations.len() != retained_contributions.len()
            || finding_fingerprints.len() > MAX_REVIEW_FINDINGS
            || attempt_receipts.is_empty()
            || attempt_receipts.len() > MAX_REVIEW_V2_COUNT as usize
            || attempt_receipts.last() != Some(&latest_receipt)
            || latest_receipt.attempt_id() != &latest_attempt_id
            || latest_receipt.status() != latest_status
            || latest_status.is_valid() != valid_batch_ordinal.is_some()
        {
            return Err(ReviewContractError::InvalidV2("review v2 batch"));
        }
        Ok(Self {
            schema_version: ReviewSchemaVersion::V2,
            review_id,
            batch_id,
            input_fingerprint,
            ruleset_fingerprint,
            latest_attempt_id,
            latest_status,
            retained_contributions,
            finding_fingerprints,
            latest_receipt,
            attempt_receipts,
            closed: latest_status.closes_batch(),
            valid_batch_ordinal,
            exit_receipt,
        })
    }

    /// Returns the owning review ID.
    pub const fn review_id(&self) -> &ReviewId {
        &self.review_id
    }

    /// Returns the batch ID.
    pub const fn batch_id(&self) -> &ReviewBatchId {
        &self.batch_id
    }

    /// Returns the batch input fingerprint.
    pub const fn input_fingerprint(&self) -> InputFingerprint {
        self.input_fingerprint
    }

    /// Returns the batch ruleset fingerprint.
    pub const fn ruleset_fingerprint(&self) -> InputFingerprint {
        self.ruleset_fingerprint
    }

    /// Returns the latest batch status.
    pub const fn latest_status(&self) -> ReviewBatchStatusV2 {
        self.latest_status
    }

    /// Returns whether the batch is closed.
    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    /// Returns retained successful contributions.
    pub fn retained_contributions(&self) -> &[ReviewerContributionV2] {
        &self.retained_contributions
    }

    /// Returns finding fingerprints.
    pub fn finding_fingerprints(&self) -> &[ArtifactDigest] {
        &self.finding_fingerprints
    }

    /// Returns all attempt receipts in ordinal order.
    pub fn attempt_receipts(&self) -> &[ReviewBatchReceiptV2] {
        &self.attempt_receipts
    }

    /// Returns the latest receipt.
    pub const fn latest_receipt(&self) -> &ReviewBatchReceiptV2 {
        &self.latest_receipt
    }

    /// Returns the terminal exit receipt when one was produced.
    pub const fn exit_receipt(&self) -> Option<&ReviewExitReceiptV2> {
        self.exit_receipt.as_ref()
    }
}

/// Stable next action emitted by the v2 supervisor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReviewNextActionV2 {
    /// Retry only the listed specialties in the same batch.
    RetryMissing {
        /// Specialties still lacking a successful contribution.
        specialties: Vec<ReviewerSpecialty>,
    },
    /// Commit remediation before starting a new session.
    Remediate,
    /// Start the next clean batch in the same session.
    ContinueCleanStreak,
    /// Review reached PASS.
    Complete,
    /// Review exhausted a budget.
    Stalled,
    /// Review input drifted and a new session is required.
    StartNewSession,
    /// Review was cancelled.
    Aborted,
}

/// Deterministic result of one v2 supervisor evaluation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewEvaluationV2 {
    next_session: ReviewSessionV2,
    next_batch: ReviewBatchV2,
    batch_receipt: ReviewBatchReceiptV2,
    exit_receipt: Option<ReviewExitReceiptV2>,
    next_action: ReviewNextActionV2,
    replayed: bool,
}

impl ReviewEvaluationV2 {
    /// Constructs an evaluation result.
    pub const fn new(
        next_session: ReviewSessionV2,
        next_batch: ReviewBatchV2,
        batch_receipt: ReviewBatchReceiptV2,
        exit_receipt: Option<ReviewExitReceiptV2>,
        next_action: ReviewNextActionV2,
        replayed: bool,
    ) -> Self {
        Self {
            next_session,
            next_batch,
            batch_receipt,
            exit_receipt,
            next_action,
            replayed,
        }
    }

    /// Returns the next session snapshot.
    pub const fn next_session(&self) -> &ReviewSessionV2 {
        &self.next_session
    }

    /// Returns the next batch snapshot.
    pub const fn next_batch(&self) -> &ReviewBatchV2 {
        &self.next_batch
    }

    /// Returns the attempt receipt.
    pub const fn batch_receipt(&self) -> &ReviewBatchReceiptV2 {
        &self.batch_receipt
    }

    /// Returns a terminal exit receipt when produced.
    pub const fn exit_receipt(&self) -> Option<&ReviewExitReceiptV2> {
        self.exit_receipt.as_ref()
    }

    /// Returns the stable next action.
    pub const fn next_action(&self) -> &ReviewNextActionV2 {
        &self.next_action
    }

    /// Returns whether this was an exact replay.
    pub const fn replayed(&self) -> bool {
        self.replayed
    }
}
