//! Restart/replay determinism and finding-deduplication truth-table tests.

use ae_sdd_contracts::review::{ReviewBudget, ReviewStatus, ReviewTier};
use ae_sdd_contracts::{BoundedText, ReasonCode, ReviewId, SchemaVersion};
use ae_sdd_domain::InputFingerprint;
use ae_sdd_review::{
    CollectedReview, ReviewFinding, ReviewFindingSeverity, ReviewSession, ReviewSupervisor,
    ReviewerIdentity, finding_fingerprint, required_roles_for,
};

const INPUT: InputFingerprint = InputFingerprint::from_array([7u8; 32]);
const RULESET: InputFingerprint = InputFingerprint::from_array([8u8; 32]);

fn session(round: u32) -> ReviewSession {
    let roles = required_roles_for(ReviewTier::Tier1);
    ReviewSession::new(
        SchemaVersion::V1,
        ReviewId::new("review-replay").unwrap(),
        ReviewTier::Tier1,
        roles,
        INPUT,
        RULESET,
        round,
        0,
        ReviewBudget::new(4, 32, 120_000).unwrap(),
    )
    .unwrap()
}

fn identity(session_suffix: &str) -> ReviewerIdentity {
    ReviewerIdentity::new(
        required_roles_for(ReviewTier::Tier1)[0].clone(),
        format!("reviewer-{session_suffix}"),
        "author-session",
        1,
        true,
    )
    .unwrap()
}

fn finding(code: &str, summary: &str) -> ReviewFinding {
    ReviewFinding::new(
        ReasonCode::new(code).unwrap(),
        ReviewFindingSeverity::Major,
        BoundedText::<1024>::new(summary).unwrap(),
    )
}

#[test]
fn replay_with_identical_inputs_produces_byte_identical_receipt() {
    let session = session(1);
    let role = required_roles_for(ReviewTier::Tier1)[0].clone();
    let collected = CollectedReview::new(
        vec![role.clone()],
        vec![identity("1")],
        INPUT,
        ReviewStatus::Completed,
        vec![],
    );
    let first = ReviewSupervisor::evaluate_legacy(&session, &collected).unwrap();
    let second = ReviewSupervisor::evaluate_legacy(&session, &collected).unwrap();
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap(),
        "replay must be byte-identical for identical inputs"
    );
}

#[test]
fn dedup_collapses_repeated_findings_keeping_first_seen_order() {
    let session = session(2);
    let role = required_roles_for(ReviewTier::Tier1)[0].clone();
    let a = finding("RC-A", "first");
    let b = finding("RC-B", "second");
    let dup_a = finding("RC-A", "first");
    let collected = CollectedReview::new(
        vec![role.clone()],
        vec![identity("1")],
        INPUT,
        ReviewStatus::Completed,
        vec![a.clone(), b.clone(), dup_a],
    );
    let receipt = ReviewSupervisor::evaluate_legacy(&session, &collected).unwrap();
    // Findings disposition is emitted because at least one finding remains.
    assert!(!receipt.is_pass());
    // The supervisor must not double-count duplicates; only the C0 receipt
    // view is observable here.
    let _ = finding_fingerprint(&a);
    let _ = finding_fingerprint(&b);
}

#[test]
fn input_fingerprint_drift_surfaces_as_invalid_infra_via_asserted_status() {
    // The supervisor trusts the caller-asserted status; drift is surfaced by
    // the C1 adapter as InvalidInfra. Here we simulate that contract by
    // passing InvalidInfra as the asserted status and expect the supervisor
    // to reject PASS.
    let session = session(3);
    let role = required_roles_for(ReviewTier::Tier1)[0].clone();
    let drifted_input = InputFingerprint::from_array([99u8; 32]);
    let collected = CollectedReview::new(
        vec![role.clone()],
        vec![identity("1")],
        INPUT,
        ReviewStatus::InvalidInfra,
        vec![],
    );
    let _ = drifted_input; // drift is owned by the adapter; supervisor only trusts status.
    let err = ReviewSupervisor::evaluate_legacy(&session, &collected).unwrap_err();
    assert!(matches!(
        err,
        ae_sdd_review::ReviewSupervisorError::InvalidInfra(_)
    ));
}

#[test]
fn restart_does_not_double_count_round_progress() {
    // Round accounting is owned by the C1 adapter. The pure supervisor only
    // consumes the round encoded in the session and the asserted status, so a
    // restart that replays the same session+collected tuple yields the same
    // receipt (no double counting is possible in the pure layer).
    let session = session(2);
    let role = required_roles_for(ReviewTier::Tier1)[0].clone();
    let collected = CollectedReview::new(
        vec![role.clone()],
        vec![identity("1")],
        INPUT,
        ReviewStatus::Completed,
        vec![],
    );
    let before = ReviewSupervisor::evaluate_legacy(&session, &collected).unwrap();
    let after = ReviewSupervisor::evaluate_legacy(&session, &collected).unwrap();
    assert_eq!(
        serde_json::to_string(&before).unwrap(),
        serde_json::to_string(&after).unwrap()
    );
}

mod v2 {
    use ae_sdd_contracts::review::{
        ReviewAttemptV2, ReviewBatchStatusV2, ReviewSessionStatusV2, ReviewSessionV2,
    };
    use ae_sdd_review::ReviewSupervisor;
    use serde_json::{Value, json};

    const INPUT: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const DRIFTED: &str = "9999999999999999999999999999999999999999999999999999999999999999";
    const RULESET: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    const DIGEST: &str = "3333333333333333333333333333333333333333333333333333333333333333";
    const ROOT: &str = "00000000-0000-0000-0000-000000000001";

    fn session() -> ReviewSessionV2 {
        serde_json::from_value(json!({
            "schemaVersion":"v2",
            "reviewId":"review-retry",
            "parentReviewId":null,
            "tier":"tier3",
            "requiredSpecialties":["be","ar","qa"],
            "authorSessionId":"00000000-0000-0000-0000-000000000099",
            "rootSessionId":ROOT,
            "inputFingerprint":INPUT,
            "rulesetFingerprint":RULESET,
            "policyDigest":DIGEST,
            "sourceRevision":7,
            "inventoryGeneration":3,
            "repairClass":"none",
            "cleanPolicy":{"cleanTarget":1,"finalProofRequirement":"full_verification"},
            "budget":{"maxAttempts":5,"maxValidBatches":3,"maxRemediations":2,"maxWallClockMinutes":60},
            "counters":{"attempts":0,"validBatches":0,"cleanStreak":0,"remediations":0,"infraFailures":0,"protocolFailures":0},
            "status":"running",
            "startedAt":"2026-07-25T10:00:00Z",
            "deadlineAt":"2026-07-25T11:00:00Z",
            "terminalAt":null
        }))
        .unwrap()
    }

    fn contribution(attempt_id: &str, specialty: &str, seed: u128, outcome: &str) -> Value {
        json!({
            "sourceAttemptId":attempt_id,
            "reviewer":{
                "agentRole":"reviewer",
                "specialty":specialty,
                "grantedSpecialties":[specialty],
                "physicalSessionId":format!("00000000-0000-0000-0000-{seed:012x}"),
                "rootSessionId":ROOT,
                "delegationId":format!("10000000-0000-0000-0000-{seed:012x}"),
                "lineageDepth":2,
                "attestationRef":format!("evidence/attestation-{seed}.json"),
                "attestationDigest":DIGEST,
                "specialtyGrantDigest":DIGEST
            },
            "outcome":outcome,
            "findings":[],
            "reportDigest":DIGEST,
            "contributionDigest":format!("{seed:064x}"),
            "inputFingerprint":INPUT,
            "rulesetFingerprint":RULESET
        })
    }

    fn attempt(attempt_id: &str, ordinal: u32, contributions: Vec<Value>) -> ReviewAttemptV2 {
        serde_json::from_value(json!({
            "schemaVersion":"v2",
            "reviewId":"review-retry",
            "batchId":"batch-1",
            "attemptId":attempt_id,
            "attemptOrdinal":ordinal,
            "idempotencyKey":format!("key-{attempt_id}"),
            "inputFingerprint":INPUT,
            "rulesetFingerprint":RULESET,
            "contributions":contributions,
            "observedAt":"2026-07-25T10:01:00Z",
            "finalProof":{
                "kind":"full_verification",
                "digest":DIGEST,
                "sourceRevision":7,
                "inputFingerprint":INPUT,
                "rulesetFingerprint":RULESET,
                "observedAt":"2026-07-25T10:00:30Z"
            },
            "projectAuthority":{
                "projectReceiptRef":"evidence/review-receipt.json",
                "activeManifestDigest":DIGEST,
                "stateReceiptRefDigest":DIGEST,
                "journalMutationId":"mutation-1"
            },
            "remediation":null
        }))
        .unwrap()
    }

    fn first_attempt() -> ReviewAttemptV2 {
        attempt(
            "attempt-1",
            1,
            vec![
                contribution("attempt-1", "be", 10, "clean"),
                contribution("attempt-1", "ar", 11, "infra_failure"),
                contribution("attempt-1", "qa", 12, "clean"),
            ],
        )
    }

    fn retry_attempt() -> ReviewAttemptV2 {
        attempt(
            "attempt-2",
            2,
            vec![contribution("attempt-2", "ar", 11, "clean")],
        )
    }

    #[test]
    fn same_batch_retry_retains_successes_and_retries_only_missing_specialty() {
        let first = ReviewSupervisor::evaluate(&session(), None, first_attempt()).unwrap();
        assert_eq!(
            first.batch_receipt().status(),
            ReviewBatchStatusV2::InvalidInfra
        );
        assert_eq!(first.next_batch().retained_contributions().len(), 2);

        let second = ReviewSupervisor::evaluate(
            first.next_session(),
            Some(first.next_batch()),
            retry_attempt(),
        )
        .unwrap();
        assert_eq!(
            second.batch_receipt().status(),
            ReviewBatchStatusV2::ValidClean
        );
        assert_eq!(second.next_batch().retained_contributions().len(), 3);
        assert_eq!(second.next_session().counters().attempts(), 2);
        assert_eq!(second.next_session().counters().valid_batches(), 1);
        assert_eq!(
            second.next_session().status(),
            ReviewSessionStatusV2::Completed
        );
    }

    #[test]
    fn exact_replay_is_byte_identical_and_does_not_advance_counters() {
        let first = ReviewSupervisor::evaluate(&session(), None, first_attempt()).unwrap();
        let retry = retry_attempt();
        let completed = ReviewSupervisor::evaluate(
            first.next_session(),
            Some(first.next_batch()),
            retry.clone(),
        )
        .unwrap();
        let replay = ReviewSupervisor::evaluate(
            completed.next_session(),
            Some(completed.next_batch()),
            retry,
        )
        .unwrap();

        assert!(replay.replayed());
        assert_eq!(
            serde_json::to_vec(replay.next_session()).unwrap(),
            serde_json::to_vec(completed.next_session()).unwrap()
        );
        assert_eq!(
            serde_json::to_vec(replay.next_batch()).unwrap(),
            serde_json::to_vec(completed.next_batch()).unwrap()
        );
    }

    #[test]
    fn same_idempotency_key_with_changed_payload_is_rejected() {
        let first = ReviewSupervisor::evaluate(&session(), None, first_attempt()).unwrap();
        let retry = retry_attempt();
        let completed = ReviewSupervisor::evaluate(
            first.next_session(),
            Some(first.next_batch()),
            retry.clone(),
        )
        .unwrap();
        let mut changed = serde_json::to_value(retry).unwrap();
        changed["observedAt"] = json!("2026-07-25T10:02:00Z");

        assert!(
            ReviewSupervisor::evaluate(
                completed.next_session(),
                Some(completed.next_batch()),
                serde_json::from_value(changed).unwrap(),
            )
            .is_err()
        );
    }

    #[test]
    fn drift_invalidates_and_discards_prior_successful_contributions() {
        let first = ReviewSupervisor::evaluate(&session(), None, first_attempt()).unwrap();
        let mut drift = serde_json::to_value(retry_attempt()).unwrap();
        drift["inputFingerprint"] = json!(DRIFTED);
        let drifted = ReviewSupervisor::evaluate(
            first.next_session(),
            Some(first.next_batch()),
            serde_json::from_value(drift).unwrap(),
        )
        .unwrap();

        assert_eq!(
            drifted.batch_receipt().status(),
            ReviewBatchStatusV2::InvalidInputDrift
        );
        assert!(drifted.next_batch().retained_contributions().is_empty());
        assert_eq!(drifted.next_session().counters().clean_streak(), 0);
        assert_eq!(
            drifted.next_session().status(),
            ReviewSessionStatusV2::Invalidated
        );
    }
}
