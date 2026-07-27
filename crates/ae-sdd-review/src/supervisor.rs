//! Pure Review-supervisor state machine.

use std::collections::BTreeSet;

use ae_sdd_contracts::review::{
    ReviewAttemptV2, ReviewBatchReceiptV2, ReviewBatchStatusV2, ReviewBatchV2,
    ReviewContributionOutcomeV2, ReviewEvaluationV2, ReviewExitDisposition,
    ReviewExitDispositionV2, ReviewExitReceipt, ReviewExitReceiptV2, ReviewFinalProofKind,
    ReviewNextActionV2, ReviewSession, ReviewSessionStatusV2, ReviewSessionV2, ReviewStatus,
    ReviewerContributionV2, ReviewerSpecialty,
};
use ae_sdd_contracts::{ReviewerRole, SchemaVersion};
use ae_sdd_domain::ArtifactDigest;

use crate::{
    error::{IdentityViolation, InfraFault, ReviewSupervisorError},
    fingerprint::{dedup_findings, finding_artifact_digest},
    model::{CollectedReview, MAX_REVIEWER_LINEAGE_DEPTH},
};

/// Stateless, deterministic review supervisor.
///
/// The supervisor consumes a frozen [`ReviewSession`] and one
/// [`CollectedReview`] input and produces either a validated
/// [`ReviewExitReceipt`] or a stable [`ReviewSupervisorError`]. It reads no
/// clock, filesystem, database, random source or global state. Round/budget
/// accounting is owned by the C1 adapter and surfaced via
/// [`CollectedReview::asserted_status`].
#[derive(Clone, Copy, Debug, Default)]
pub struct ReviewSupervisor;

impl ReviewSupervisor {
    /// Evaluates one authenticated v2 attempt against a session and optional
    /// same-batch aggregate.
    pub fn evaluate(
        session: &ReviewSessionV2,
        prior_batch: Option<&ReviewBatchV2>,
        attempt: ReviewAttemptV2,
    ) -> Result<ReviewEvaluationV2, ReviewSupervisorError> {
        let attempt_digest = canonical_digest(&attempt)?;
        validate_attempt_envelope(session, prior_batch, &attempt, attempt_digest)?;

        if let Some(replayed) = replay_evaluation(session, prior_batch, &attempt, attempt_digest)? {
            return Ok(replayed);
        }
        if session.status().is_terminal() {
            return Err(ReviewSupervisorError::InvalidCollectedInput(
                "terminal review session rejects new attempts",
            ));
        }
        if attempt.attempt_ordinal() != session.counters().attempts() + 1 {
            return Err(ReviewSupervisorError::InvalidCollectedInput(
                "attempt ordinal is not the next session ordinal",
            ));
        }

        let mut retained =
            prior_batch.map_or_else(Vec::new, |batch| batch.retained_contributions().to_vec());
        let status = evaluate_attempt_status(session, &attempt, &mut retained);
        if status == ReviewBatchStatusV2::InvalidInputDrift {
            retained.clear();
        }

        let findings = retained
            .iter()
            .flat_map(|contribution| contribution.findings().iter().cloned())
            .collect::<Vec<_>>();
        let deduped_findings = dedup_findings(&findings)?;
        let finding_fingerprints = deduped_findings
            .iter()
            .map(finding_artifact_digest)
            .collect::<Vec<_>>();
        let completed_specialties = canonical_completed(session, &retained);

        let mut counters = session
            .counters()
            .after_attempt(status)
            .map_err(contract_error)?;
        if status == ReviewBatchStatusV2::InvalidInputDrift {
            counters = ae_sdd_contracts::review::ReviewCountersV2::new(
                counters.attempts(),
                counters.valid_batches(),
                0,
                counters.remediations(),
                counters.infra_failures(),
                counters.protocol_failures(),
            )
            .map_err(contract_error)?;
        }

        let pass = status == ReviewBatchStatusV2::ValidClean
            && counters.clean_streak() >= session.clean_policy().clean_target()
            && finding_fingerprints.is_empty()
            && final_proof_valid(session, &attempt)
            && attempt.observed_at() <= session.deadline_at();
        let exhausted = session.budget().count_exhausted(counters)
            || attempt.observed_at() >= session.deadline_at();
        let (next_status, terminal_at) = if pass {
            (
                ReviewSessionStatusV2::Completed,
                Some(attempt.observed_at().clone()),
            )
        } else {
            match status {
                ReviewBatchStatusV2::InvalidInputDrift => (
                    ReviewSessionStatusV2::Invalidated,
                    Some(attempt.observed_at().clone()),
                ),
                ReviewBatchStatusV2::Cancelled => (
                    ReviewSessionStatusV2::Aborted,
                    Some(attempt.observed_at().clone()),
                ),
                _ if exhausted => (
                    ReviewSessionStatusV2::Stalled,
                    Some(attempt.observed_at().clone()),
                ),
                ReviewBatchStatusV2::ValidFindings => {
                    (ReviewSessionStatusV2::RemediationRequired, None)
                }
                _ => (ReviewSessionStatusV2::Running, None),
            }
        };
        let next_session = session
            .transition(counters, next_status, terminal_at)
            .map_err(contract_error)?;

        let zero_finding_digest = (status == ReviewBatchStatusV2::ValidClean)
            .then(|| canonical_zero_finding_digest(&retained));
        let batch_receipt = build_batch_receipt(
            session,
            &attempt,
            attempt_digest,
            status,
            completed_specialties.clone(),
            counters,
            finding_fingerprints.clone(),
            zero_finding_digest,
        )?;
        let exit_receipt = build_exit_receipt(
            &next_session,
            &attempt,
            &batch_receipt,
            status,
            completed_specialties,
            finding_fingerprints.len() as u32,
        )?;

        let mut attempt_receipts =
            prior_batch.map_or_else(Vec::new, |batch| batch.attempt_receipts().to_vec());
        attempt_receipts.push(batch_receipt.clone());
        let valid_batch_ordinal = status.is_valid().then_some(counters.valid_batches());
        let next_batch = ReviewBatchV2::new(
            session.review_id().clone(),
            attempt.batch_id().clone(),
            session.input_fingerprint(),
            session.ruleset_fingerprint(),
            attempt.attempt_id().clone(),
            status,
            retained,
            finding_fingerprints,
            batch_receipt.clone(),
            attempt_receipts,
            valid_batch_ordinal,
            exit_receipt.clone(),
        )
        .map_err(contract_error)?;
        let next_action = next_action(&next_session, &next_batch);

        Ok(ReviewEvaluationV2::new(
            next_session,
            next_batch,
            batch_receipt,
            exit_receipt,
            next_action,
            false,
        ))
    }

    /// Evaluates a legacy v1 collection without upgrading it to v2.
    pub fn evaluate_legacy(
        session: &ReviewSession,
        collected: &CollectedReview,
    ) -> Result<ReviewExitReceipt, ReviewSupervisorError> {
        validate_identity_independence(session.required_roles(), collected)?;
        let deduped = dedup_findings(collected.findings())?;

        let status = collected.asserted_status();
        if !is_terminal(status) {
            return Err(ReviewSupervisorError::InvalidCollectedInput(
                "asserted status is not terminal",
            ));
        }
        let disposition = match status {
            ReviewStatus::Completed if deduped.is_empty() => ReviewExitDisposition::Pass,
            ReviewStatus::Completed => ReviewExitDisposition::Findings,
            ReviewStatus::Stalled => {
                return Err(ReviewSupervisorError::InvalidInfra(
                    InfraFault::BudgetExhausted,
                ));
            }
            ReviewStatus::Aborted => ReviewExitDisposition::Aborted,
            ReviewStatus::InvalidInfra => {
                return Err(ReviewSupervisorError::InvalidInfra(
                    InfraFault::MissingAttestation,
                ));
            }
            _ => {
                return Err(ReviewSupervisorError::InvalidCollectedInput(
                    "asserted status cannot produce a receipt",
                ));
            }
        };

        ReviewExitReceipt::new(
            SchemaVersion::V1,
            session,
            status,
            disposition,
            collected.observed_input_fingerprint(),
            collected.completed_roles().to_vec(),
            deduped,
        )
        .map_err(|err| ReviewSupervisorError::ReceiptRejected(err.to_string()))
    }
}

fn validate_attempt_envelope(
    session: &ReviewSessionV2,
    prior_batch: Option<&ReviewBatchV2>,
    attempt: &ReviewAttemptV2,
    _attempt_digest: ArtifactDigest,
) -> Result<(), ReviewSupervisorError> {
    if attempt.review_id() != session.review_id() {
        return Err(ReviewSupervisorError::InvalidCollectedInput(
            "attempt reviewId differs from session reviewId",
        ));
    }
    if let Some(batch) = prior_batch
        && (batch.review_id() != session.review_id() || batch.batch_id() != attempt.batch_id())
    {
        return Err(ReviewSupervisorError::InvalidCollectedInput(
            "prior batch identity differs from the attempt",
        ));
    }
    Ok(())
}

fn replay_evaluation(
    session: &ReviewSessionV2,
    prior_batch: Option<&ReviewBatchV2>,
    attempt: &ReviewAttemptV2,
    attempt_digest: ArtifactDigest,
) -> Result<Option<ReviewEvaluationV2>, ReviewSupervisorError> {
    let Some(batch) = prior_batch else {
        return Ok(None);
    };
    let prior = batch.attempt_receipts().iter().find(|receipt| {
        receipt.attempt_id() == attempt.attempt_id()
            || receipt.idempotency_key() == attempt.idempotency_key()
    });
    let Some(receipt) = prior else {
        if batch.is_closed() {
            return Err(ReviewSupervisorError::InvalidCollectedInput(
                "closed review batch accepts exact replay only",
            ));
        }
        return Ok(None);
    };
    if receipt.attempt_id() != attempt.attempt_id()
        || receipt.idempotency_key() != attempt.idempotency_key()
        || receipt.attempt_digest() != attempt_digest
    {
        return Err(ReviewSupervisorError::InvalidCollectedInput(
            "attempt id or idempotency key was reused with a different payload",
        ));
    }
    Ok(Some(ReviewEvaluationV2::new(
        session.clone(),
        batch.clone(),
        receipt.clone(),
        batch.exit_receipt().cloned(),
        next_action(session, batch),
        true,
    )))
}

fn evaluate_attempt_status(
    session: &ReviewSessionV2,
    attempt: &ReviewAttemptV2,
    retained: &mut Vec<ReviewerContributionV2>,
) -> ReviewBatchStatusV2 {
    if attempt.input_fingerprint() != session.input_fingerprint()
        || attempt.ruleset_fingerprint() != session.ruleset_fingerprint()
        || attempt.contributions().iter().any(|contribution| {
            contribution.input_fingerprint() != session.input_fingerprint()
                || contribution.ruleset_fingerprint() != session.ruleset_fingerprint()
        })
    {
        return ReviewBatchStatusV2::InvalidInputDrift;
    }

    let required: BTreeSet<_> = session.required_specialties().iter().copied().collect();
    let mut specialties: BTreeSet<_> = retained
        .iter()
        .map(|item| item.reviewer().specialty())
        .collect();
    let mut physical_sessions: BTreeSet<_> = retained
        .iter()
        .map(|item| item.reviewer().physical_session_id())
        .collect();
    let mut delegations: BTreeSet<_> = retained
        .iter()
        .map(|item| item.reviewer().delegation_id())
        .collect();
    let mut protocol_failure = retained.iter().any(|item| {
        !required.contains(&item.reviewer().specialty())
            || item.reviewer().root_session_id() != session.root_session_id()
            || item.reviewer().physical_session_id() == session.author_session_id()
            || !matches!(
                item.outcome(),
                ReviewContributionOutcomeV2::Clean | ReviewContributionOutcomeV2::Findings
            )
    });
    let mut infra_failure = false;
    let mut cancelled = false;
    let mut presented_specialties = BTreeSet::new();

    for contribution in attempt.contributions() {
        let reviewer = contribution.reviewer();
        if !presented_specialties.insert(reviewer.specialty())
            || !required.contains(&reviewer.specialty())
            || reviewer.root_session_id() != session.root_session_id()
            || reviewer.physical_session_id() == session.author_session_id()
            || !physical_sessions.insert(reviewer.physical_session_id())
            || !delegations.insert(reviewer.delegation_id())
        {
            protocol_failure = true;
            continue;
        }
        match contribution.outcome() {
            ReviewContributionOutcomeV2::Clean | ReviewContributionOutcomeV2::Findings => {
                if !specialties.insert(reviewer.specialty()) {
                    protocol_failure = true;
                } else {
                    retained.push(contribution.clone());
                }
            }
            ReviewContributionOutcomeV2::InfraFailure => infra_failure = true,
            ReviewContributionOutcomeV2::ProtocolFailure => protocol_failure = true,
            ReviewContributionOutcomeV2::Cancelled => cancelled = true,
        }
    }

    if protocol_failure {
        ReviewBatchStatusV2::InvalidProtocol
    } else if cancelled {
        ReviewBatchStatusV2::Cancelled
    } else if infra_failure || specialties != required {
        ReviewBatchStatusV2::InvalidInfra
    } else if retained.iter().any(|item| !item.findings().is_empty()) {
        ReviewBatchStatusV2::ValidFindings
    } else {
        ReviewBatchStatusV2::ValidClean
    }
}

fn canonical_completed(
    session: &ReviewSessionV2,
    retained: &[ReviewerContributionV2],
) -> Vec<ReviewerSpecialty> {
    let completed: BTreeSet<_> = retained
        .iter()
        .map(|item| item.reviewer().specialty())
        .collect();
    session
        .required_specialties()
        .iter()
        .copied()
        .filter(|specialty| completed.contains(specialty))
        .collect()
}

fn final_proof_valid(session: &ReviewSessionV2, attempt: &ReviewAttemptV2) -> bool {
    let proof = attempt.final_proof();
    if proof.kind() != session.clean_policy().final_proof_requirement() {
        return false;
    }
    match proof.kind() {
        ReviewFinalProofKind::None => true,
        ReviewFinalProofKind::DeterministicGates | ReviewFinalProofKind::FullVerification => {
            proof.digest().is_some()
                && proof.source_revision() == Some(session.source_revision())
                && proof.input_fingerprint() == Some(session.input_fingerprint())
                && proof.ruleset_fingerprint() == Some(session.ruleset_fingerprint())
                && proof.observed_at().is_some_and(|observed| {
                    observed <= attempt.observed_at() && observed <= session.deadline_at()
                })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_batch_receipt(
    session: &ReviewSessionV2,
    attempt: &ReviewAttemptV2,
    attempt_digest: ArtifactDigest,
    status: ReviewBatchStatusV2,
    completed_specialties: Vec<ReviewerSpecialty>,
    counters: ae_sdd_contracts::review::ReviewCountersV2,
    finding_fingerprints: Vec<ArtifactDigest>,
    zero_finding_digest: Option<ArtifactDigest>,
) -> Result<ReviewBatchReceiptV2, ReviewSupervisorError> {
    let provisional = ReviewBatchReceiptV2::new(
        session.review_id().clone(),
        attempt.batch_id().clone(),
        attempt.attempt_id().clone(),
        attempt.attempt_ordinal(),
        attempt.idempotency_key().clone(),
        attempt_digest,
        status,
        session.required_specialties().to_vec(),
        completed_specialties.clone(),
        counters,
        finding_fingerprints.clone(),
        zero_finding_digest,
        attempt.final_proof().clone(),
        attempt.project_authority().clone(),
        attempt.observed_at().clone(),
        ArtifactDigest::from_array([0; 32]),
    )
    .map_err(contract_error)?;
    let receipt_digest = canonical_digest_without_field(&provisional, "receiptDigest")?;
    ReviewBatchReceiptV2::new(
        session.review_id().clone(),
        attempt.batch_id().clone(),
        attempt.attempt_id().clone(),
        attempt.attempt_ordinal(),
        attempt.idempotency_key().clone(),
        attempt_digest,
        status,
        session.required_specialties().to_vec(),
        completed_specialties,
        counters,
        finding_fingerprints,
        zero_finding_digest,
        attempt.final_proof().clone(),
        attempt.project_authority().clone(),
        attempt.observed_at().clone(),
        receipt_digest,
    )
    .map_err(contract_error)
}

#[allow(clippy::too_many_arguments)]
fn build_exit_receipt(
    session: &ReviewSessionV2,
    attempt: &ReviewAttemptV2,
    batch_receipt: &ReviewBatchReceiptV2,
    status: ReviewBatchStatusV2,
    completed_specialties: Vec<ReviewerSpecialty>,
    finding_count: u32,
) -> Result<Option<ReviewExitReceiptV2>, ReviewSupervisorError> {
    let disposition = match session.status() {
        ReviewSessionStatusV2::Completed => ReviewExitDispositionV2::Pass,
        ReviewSessionStatusV2::Stalled => ReviewExitDispositionV2::Stalled,
        ReviewSessionStatusV2::Invalidated => ReviewExitDispositionV2::Invalidated,
        ReviewSessionStatusV2::Aborted => ReviewExitDispositionV2::Aborted,
        _ => return Ok(None),
    };
    let zero_receipt =
        (disposition == ReviewExitDispositionV2::Pass).then_some(batch_receipt.receipt_digest());
    let provisional = ReviewExitReceiptV2::new(
        session,
        disposition,
        attempt.input_fingerprint(),
        completed_specialties.clone(),
        finding_count,
        attempt.batch_id().clone(),
        attempt.attempt_id().clone(),
        status,
        zero_receipt,
        attempt.final_proof().clone(),
        attempt.project_authority().clone(),
        attempt.observed_at().clone(),
        ArtifactDigest::from_array([0; 32]),
    )
    .map_err(contract_error)?;
    let receipt_digest = canonical_digest_without_field(&provisional, "receiptDigest")?;
    ReviewExitReceiptV2::new(
        session,
        disposition,
        attempt.input_fingerprint(),
        completed_specialties,
        finding_count,
        attempt.batch_id().clone(),
        attempt.attempt_id().clone(),
        status,
        zero_receipt,
        attempt.final_proof().clone(),
        attempt.project_authority().clone(),
        attempt.observed_at().clone(),
        receipt_digest,
    )
    .map(Some)
    .map_err(contract_error)
}

fn next_action(session: &ReviewSessionV2, batch: &ReviewBatchV2) -> ReviewNextActionV2 {
    match session.status() {
        ReviewSessionStatusV2::Completed => ReviewNextActionV2::Complete,
        ReviewSessionStatusV2::Stalled => ReviewNextActionV2::Stalled,
        ReviewSessionStatusV2::Invalidated => ReviewNextActionV2::StartNewSession,
        ReviewSessionStatusV2::Aborted => ReviewNextActionV2::Aborted,
        ReviewSessionStatusV2::RemediationRequired => ReviewNextActionV2::Remediate,
        ReviewSessionStatusV2::Queued | ReviewSessionStatusV2::Running
            if batch.latest_receipt().status().is_valid() =>
        {
            ReviewNextActionV2::ContinueCleanStreak
        }
        ReviewSessionStatusV2::Queued | ReviewSessionStatusV2::Running => {
            let completed: BTreeSet<_> = batch
                .retained_contributions()
                .iter()
                .map(|item| item.reviewer().specialty())
                .collect();
            ReviewNextActionV2::RetryMissing {
                specialties: session
                    .required_specialties()
                    .iter()
                    .copied()
                    .filter(|specialty| !completed.contains(specialty))
                    .collect(),
            }
        }
    }
}

fn canonical_zero_finding_digest(retained: &[ReviewerContributionV2]) -> ArtifactDigest {
    serde_json::to_vec(retained).map_or_else(|_| ArtifactDigest::digest([]), ArtifactDigest::digest)
}

fn canonical_digest<T: serde::Serialize>(
    value: &T,
) -> Result<ArtifactDigest, ReviewSupervisorError> {
    serde_json::to_vec(value)
        .map(ArtifactDigest::digest)
        .map_err(|_| ReviewSupervisorError::InvalidCollectedInput("canonical serialization failed"))
}

fn canonical_digest_without_field<T: serde::Serialize>(
    value: &T,
    field: &str,
) -> Result<ArtifactDigest, ReviewSupervisorError> {
    let mut value = serde_json::to_value(value).map_err(|_| {
        ReviewSupervisorError::InvalidCollectedInput("canonical serialization failed")
    })?;
    value
        .as_object_mut()
        .ok_or(ReviewSupervisorError::InvalidCollectedInput(
            "canonical receipt is not an object",
        ))?
        .remove(field);
    canonical_digest(&value)
}

fn contract_error(error: ae_sdd_contracts::review::ReviewContractError) -> ReviewSupervisorError {
    ReviewSupervisorError::ReceiptRejected(error.to_string())
}

fn is_terminal(status: ReviewStatus) -> bool {
    matches!(
        status,
        ReviewStatus::Completed
            | ReviewStatus::Stalled
            | ReviewStatus::InvalidInfra
            | ReviewStatus::Aborted
    )
}

fn validate_identity_independence(
    required_roles: &[ReviewerRole],
    collected: &CollectedReview,
) -> Result<(), ReviewSupervisorError> {
    let required: BTreeSet<&str> = required_roles.iter().map(ReviewerRole::as_str).collect();
    if required.is_empty() {
        return Err(ReviewSupervisorError::InvalidInfra(
            InfraFault::MissingAttestation,
        ));
    }

    let mut roles_backed: BTreeSet<&str> = BTreeSet::new();
    for identity in collected.reviewer_identities() {
        if !identity.attested() {
            return Err(ReviewSupervisorError::InvalidInfra(
                InfraFault::MissingAttestation,
            ));
        }
        if identity.lineage_depth() == 0 {
            return Err(ReviewSupervisorError::IdentityIndependenceViolated(
                IdentityViolation::RootReviewer,
            ));
        }
        if identity.lineage_depth() > MAX_REVIEWER_LINEAGE_DEPTH {
            return Err(ReviewSupervisorError::IdentityIndependenceViolated(
                IdentityViolation::ExcessiveLineageDepth,
            ));
        }
        if identity.physical_session_id() == identity.author_session_id() {
            return Err(ReviewSupervisorError::IdentityIndependenceViolated(
                IdentityViolation::SelfReview,
            ));
        }
        roles_backed.insert(identity.reviewer_role().as_str());
    }

    // Duplicate physical sessions are rejected even when roles differ.
    let physical: BTreeSet<&str> = collected.physical_sessions();
    if physical.len() != collected.reviewer_identities().len() {
        return Err(ReviewSupervisorError::IdentityIndependenceViolated(
            IdentityViolation::DuplicatePhysicalSession,
        ));
    }

    // Every required role must be backed by an attested identity.
    if !required.is_subset(&roles_backed) {
        return Err(ReviewSupervisorError::IdentityIndependenceViolated(
            IdentityViolation::UnbackedCompletedRole,
        ));
    }
    Ok(())
}
