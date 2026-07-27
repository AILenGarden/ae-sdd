//! Tier matrix and identity-independence truth-table tests for the
//! `ReviewSupervisor`.

use ae_sdd_contracts::review::{ReviewBudget, ReviewStatus, ReviewTier};
use ae_sdd_contracts::{BoundedText, ReasonCode, ReviewId, ReviewerRole, SchemaVersion};
use ae_sdd_domain::InputFingerprint;
use ae_sdd_review::{
    CollectedReview, IdentityViolation, InfraFault, ReviewFinding, ReviewFindingSeverity,
    ReviewSession, ReviewSupervisor, ReviewSupervisorError, ReviewerIdentity, required_roles_for,
};

const INPUT: InputFingerprint = InputFingerprint::from_array([1u8; 32]);
const RULESET: InputFingerprint = InputFingerprint::from_array([2u8; 32]);

fn budget() -> ReviewBudget {
    ReviewBudget::new(3, 16, 60_000).unwrap()
}

fn session(tier: ReviewTier) -> ReviewSession {
    let roles = required_roles_for(tier);
    ReviewSession::new(
        SchemaVersion::V1,
        ReviewId::new("review-001").unwrap(),
        tier,
        roles,
        INPUT,
        RULESET,
        1,
        0,
        budget(),
    )
    .unwrap()
}

fn identity(role: &str, session_suffix: &str, depth: u8) -> ReviewerIdentity {
    ReviewerIdentity::new(
        ReviewerRole::new(role).unwrap(),
        format!("reviewer-session-{session_suffix}"),
        "author-session",
        depth,
        true,
    )
    .unwrap()
}

fn collected(
    completed: Vec<ReviewerRole>,
    identities: Vec<ReviewerIdentity>,
    status: ReviewStatus,
    findings: Vec<ReviewFinding>,
) -> CollectedReview {
    CollectedReview::new(completed, identities, INPUT, status, findings)
}

fn finding(code: &str) -> ReviewFinding {
    ReviewFinding::new(
        ReasonCode::new(code).unwrap(),
        ReviewFindingSeverity::Major,
        BoundedText::<1024>::new("finding").unwrap(),
    )
}

#[test]
fn tier1_passes_with_one_attested_engineering_reviewer_and_no_findings() {
    let session = session(ReviewTier::Tier1);
    let role = required_roles_for(ReviewTier::Tier1)[0].clone();
    let collected = collected(
        vec![role.clone()],
        vec![identity(role.as_str(), "1", 1)],
        ReviewStatus::Completed,
        vec![],
    );
    let receipt = ReviewSupervisor::evaluate_legacy(&session, &collected).expect("tier1 PASS");
    assert!(receipt.is_pass());
}

#[test]
fn tier2_requires_two_distinct_attested_reviewers() {
    let session = session(ReviewTier::Tier2);
    let roles = required_roles_for(ReviewTier::Tier2);
    let collected = collected(
        roles.clone(),
        vec![
            identity("engineering", "1", 1),
            identity("security", "2", 1),
        ],
        ReviewStatus::Completed,
        vec![],
    );
    let receipt = ReviewSupervisor::evaluate_legacy(&session, &collected).expect("tier2 PASS");
    assert!(receipt.is_pass());
}

#[test]
fn tier2_with_single_reviewer_fails_due_to_unbacked_role() {
    let session = session(ReviewTier::Tier2);
    let roles = required_roles_for(ReviewTier::Tier2);
    let collected = collected(
        roles.clone(),
        vec![identity("engineering", "1", 1)],
        ReviewStatus::Completed,
        vec![],
    );
    let err = ReviewSupervisor::evaluate_legacy(&session, &collected).unwrap_err();
    assert_eq!(
        err,
        ReviewSupervisorError::IdentityIndependenceViolated(
            IdentityViolation::UnbackedCompletedRole
        )
    );
}

#[test]
fn self_review_is_rejected() {
    let session = session(ReviewTier::Tier1);
    let role = required_roles_for(ReviewTier::Tier1)[0].clone();
    let self_identity =
        ReviewerIdentity::new(role.clone(), "author-session", "author-session", 1, true).unwrap();
    let collected = collected(
        vec![role],
        vec![self_identity],
        ReviewStatus::Completed,
        vec![],
    );
    let err = ReviewSupervisor::evaluate_legacy(&session, &collected).unwrap_err();
    assert_eq!(
        err,
        ReviewSupervisorError::IdentityIndependenceViolated(IdentityViolation::SelfReview)
    );
}

#[test]
fn duplicate_physical_session_is_rejected() {
    let session = session(ReviewTier::Tier2);
    let roles = required_roles_for(ReviewTier::Tier2);
    let collected = collected(
        roles.clone(),
        vec![
            identity("engineering", "shared", 1),
            identity("security", "shared", 1),
        ],
        ReviewStatus::Completed,
        vec![],
    );
    let err = ReviewSupervisor::evaluate_legacy(&session, &collected).unwrap_err();
    assert_eq!(
        err,
        ReviewSupervisorError::IdentityIndependenceViolated(
            IdentityViolation::DuplicatePhysicalSession
        )
    );
}

#[test]
fn root_depth_reviewer_is_rejected() {
    let session = session(ReviewTier::Tier1);
    let role = required_roles_for(ReviewTier::Tier1)[0].clone();
    let collected = collected(
        vec![role.clone()],
        vec![identity(role.as_str(), "1", 0)],
        ReviewStatus::Completed,
        vec![],
    );
    let err = ReviewSupervisor::evaluate_legacy(&session, &collected).unwrap_err();
    assert_eq!(
        err,
        ReviewSupervisorError::IdentityIndependenceViolated(IdentityViolation::RootReviewer)
    );
}

#[test]
fn unattested_identity_is_rejected_as_invalid_infra() {
    let session = session(ReviewTier::Tier1);
    let role = required_roles_for(ReviewTier::Tier1)[0].clone();
    let unattested = ReviewerIdentity::new(
        role.clone(),
        "reviewer-session-1",
        "author-session",
        1,
        false,
    )
    .unwrap();
    let collected = collected(
        vec![role],
        vec![unattested],
        ReviewStatus::Completed,
        vec![],
    );
    let err = ReviewSupervisor::evaluate_legacy(&session, &collected).unwrap_err();
    assert_eq!(
        err,
        ReviewSupervisorError::InvalidInfra(InfraFault::MissingAttestation)
    );
}

#[test]
fn stalled_status_cannot_produce_pass() {
    let session = session(ReviewTier::Tier1);
    let role = required_roles_for(ReviewTier::Tier1)[0].clone();
    let collected = collected(
        vec![role.clone()],
        vec![identity(role.as_str(), "1", 1)],
        ReviewStatus::Stalled,
        vec![],
    );
    let err = ReviewSupervisor::evaluate_legacy(&session, &collected).unwrap_err();
    assert_eq!(
        err,
        ReviewSupervisorError::InvalidInfra(InfraFault::BudgetExhausted)
    );
}

#[test]
fn completed_with_finding_produces_findings_disposition_not_pass() {
    let session = session(ReviewTier::Tier1);
    let role = required_roles_for(ReviewTier::Tier1)[0].clone();
    let collected = collected(
        vec![role.clone()],
        vec![identity(role.as_str(), "1", 1)],
        ReviewStatus::Completed,
        vec![finding("RC-A")],
    );
    let receipt =
        ReviewSupervisor::evaluate_legacy(&session, &collected).expect("findings receipt");
    assert!(!receipt.is_pass());
}
