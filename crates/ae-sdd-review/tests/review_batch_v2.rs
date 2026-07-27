use ae_sdd_contracts::review::{
    ReviewAttemptV2, ReviewBatchStatusV2, ReviewBatchV2, ReviewBudgetV2, ReviewRepairClass,
    ReviewSessionStatusV2, ReviewSessionV2, ReviewTier,
};
use ae_sdd_review::ReviewSupervisor;
use serde_json::{Value, json};

const INPUT: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const DRIFTED: &str = "9999999999999999999999999999999999999999999999999999999999999999";
const RULESET: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const DIGEST: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const AUTHOR: &str = "00000000-0000-0000-0000-000000000099";
const ROOT: &str = "00000000-0000-0000-0000-000000000001";

fn tier_wire(tier: ReviewTier) -> (&'static str, Vec<&'static str>, &'static str) {
    match tier {
        ReviewTier::Tier1 => ("tier1", vec!["general"], "none"),
        ReviewTier::Tier2 => ("tier2", vec!["be", "ar"], "deterministic_gates"),
        ReviewTier::Tier3 => ("tier3", vec!["be", "ar", "qa"], "full_verification"),
    }
}

fn session(
    tier: ReviewTier,
    repair: ReviewRepairClass,
    attempts: u32,
    valid_batches: u32,
    clean_streak: u32,
    max_attempts: u32,
) -> ReviewSessionV2 {
    let (tier, specialties, proof) = tier_wire(tier);
    let repair = match repair {
        ReviewRepairClass::None => "none",
        ReviewRepairClass::NonCritical => "non_critical",
        ReviewRepairClass::HighRisk => "high_risk",
        ReviewRepairClass::CriticalContract => "critical_contract",
    };
    let clean_target = if (tier == "tier2" && repair == "critical_contract")
        || (tier == "tier3" && matches!(repair, "high_risk" | "critical_contract"))
    {
        2
    } else {
        1
    };
    serde_json::from_value(json!({
        "schemaVersion": "v2",
        "reviewId": "review-v2",
        "parentReviewId": null,
        "tier": tier,
        "requiredSpecialties": specialties,
        "authorSessionId": AUTHOR,
        "rootSessionId": ROOT,
        "inputFingerprint": INPUT,
        "rulesetFingerprint": RULESET,
        "policyDigest": DIGEST,
        "sourceRevision": 7,
        "inventoryGeneration": 3,
        "repairClass": repair,
        "cleanPolicy": {"cleanTarget": clean_target, "finalProofRequirement": proof},
        "budget": {
            "maxAttempts": max_attempts,
            "maxValidBatches": 3,
            "maxRemediations": 2,
            "maxWallClockMinutes": 60
        },
        "counters": {
            "attempts": attempts,
            "validBatches": valid_batches,
            "cleanStreak": clean_streak,
            "remediations": 0,
            "infraFailures": 0,
            "protocolFailures": 0
        },
        "status": "running",
        "startedAt": "2026-07-25T10:00:00Z",
        "deadlineAt": "2026-07-25T11:00:00Z",
        "terminalAt": null
    }))
    .unwrap()
}

fn proof(kind: &str) -> Value {
    if kind == "none" {
        json!({
            "kind": "none",
            "digest": null,
            "sourceRevision": null,
            "inputFingerprint": null,
            "rulesetFingerprint": null,
            "observedAt": null
        })
    } else {
        json!({
            "kind": kind,
            "digest": DIGEST,
            "sourceRevision": 7,
            "inputFingerprint": INPUT,
            "rulesetFingerprint": RULESET,
            "observedAt": "2026-07-25T10:00:30Z"
        })
    }
}

fn contribution(
    attempt_id: &str,
    specialty: &str,
    index: u128,
    outcome: &str,
    findings: Value,
) -> Value {
    json!({
        "sourceAttemptId": attempt_id,
        "reviewer": {
            "agentRole": "reviewer",
            "specialty": specialty,
            "grantedSpecialties": [specialty],
            "physicalSessionId": format!("00000000-0000-0000-0000-{index:012x}"),
            "rootSessionId": ROOT,
            "delegationId": format!("10000000-0000-0000-0000-{index:012x}"),
            "lineageDepth": 2,
            "attestationRef": format!("evidence/attestation-{index}.json"),
            "attestationDigest": DIGEST,
            "specialtyGrantDigest": DIGEST
        },
        "outcome": outcome,
        "findings": findings,
        "reportDigest": DIGEST,
        "contributionDigest": format!("{index:064x}"),
        "inputFingerprint": INPUT,
        "rulesetFingerprint": RULESET
    })
}

fn attempt(
    tier: ReviewTier,
    ordinal: u32,
    batch_id: &str,
    attempt_id: &str,
    outcomes: &[&str],
    proof_override: Option<&str>,
) -> ReviewAttemptV2 {
    let (_, specialties, required_proof) = tier_wire(tier);
    let contributions = specialties
        .iter()
        .zip(outcomes)
        .enumerate()
        .map(|(index, (specialty, outcome))| {
            let findings = if *outcome == "findings" {
                json!([{
                    "code": "review.defect",
                    "severity": "major",
                    "summary": "material defect"
                }])
            } else {
                json!([])
            };
            contribution(attempt_id, specialty, index as u128 + 10, outcome, findings)
        })
        .collect::<Vec<_>>();
    serde_json::from_value(json!({
        "schemaVersion": "v2",
        "reviewId": "review-v2",
        "batchId": batch_id,
        "attemptId": attempt_id,
        "attemptOrdinal": ordinal,
        "idempotencyKey": format!("key-{attempt_id}"),
        "inputFingerprint": INPUT,
        "rulesetFingerprint": RULESET,
        "contributions": contributions,
        "observedAt": "2026-07-25T10:01:00Z",
        "finalProof": proof(proof_override.unwrap_or(required_proof)),
        "projectAuthority": {
            "projectReceiptRef": "evidence/review-receipt.json",
            "activeManifestDigest": DIGEST,
            "stateReceiptRefDigest": DIGEST,
            "journalMutationId": "mutation-1"
        },
        "remediation": null
    }))
    .unwrap()
}

fn single_specialty_attempt(
    tier: ReviewTier,
    ordinal: u32,
    batch_id: &str,
    attempt_id: &str,
    specialty: &str,
) -> ReviewAttemptV2 {
    let (_, _, required_proof) = tier_wire(tier);
    serde_json::from_value(json!({
        "schemaVersion": "v2",
        "reviewId": "review-v2",
        "batchId": batch_id,
        "attemptId": attempt_id,
        "attemptOrdinal": ordinal,
        "idempotencyKey": format!("key-{attempt_id}"),
        "inputFingerprint": INPUT,
        "rulesetFingerprint": RULESET,
        "contributions": [contribution(
            attempt_id,
            specialty,
            u128::from(ordinal) + 100,
            "clean",
            json!([]),
        )],
        "observedAt": "2026-07-25T10:01:00Z",
        "finalProof": proof(required_proof),
        "projectAuthority": {
            "projectReceiptRef": "evidence/review-receipt.json",
            "activeManifestDigest": DIGEST,
            "stateReceiptRefDigest": DIGEST,
            "journalMutationId": "mutation-1"
        },
        "remediation": null
    }))
    .unwrap()
}

#[test]
fn exact_tier_specialties_and_proof_produce_pass() {
    for tier in [ReviewTier::Tier1, ReviewTier::Tier2, ReviewTier::Tier3] {
        let session = session(tier, ReviewRepairClass::None, 0, 0, 0, 5);
        let count = tier_wire(tier).1.len();
        let attempt = attempt(tier, 1, "batch-1", "attempt-1", &vec!["clean"; count], None);
        let evaluation = ReviewSupervisor::evaluate(&session, None, attempt).unwrap();

        assert_eq!(
            evaluation.next_session().status(),
            ReviewSessionStatusV2::Completed
        );
        assert_eq!(
            evaluation.batch_receipt().status(),
            ReviewBatchStatusV2::ValidClean
        );
        assert!(
            evaluation
                .exit_receipt()
                .is_some_and(|receipt| receipt.is_pass())
        );
    }
}

#[test]
fn six_batch_statuses_have_the_frozen_counter_effects() {
    let cases = [
        ("clean", ReviewBatchStatusV2::ValidClean),
        ("findings", ReviewBatchStatusV2::ValidFindings),
        ("infra_failure", ReviewBatchStatusV2::InvalidInfra),
        ("protocol_failure", ReviewBatchStatusV2::InvalidProtocol),
        ("cancelled", ReviewBatchStatusV2::Cancelled),
    ];
    for (outcome, expected) in cases {
        let session = session(ReviewTier::Tier1, ReviewRepairClass::None, 0, 0, 0, 5);
        let evaluation = ReviewSupervisor::evaluate(
            &session,
            None,
            attempt(
                ReviewTier::Tier1,
                1,
                "batch-1",
                "attempt-1",
                &[outcome],
                None,
            ),
        )
        .unwrap();
        assert_eq!(evaluation.batch_receipt().status(), expected);
        assert_eq!(evaluation.next_session().counters().attempts(), 1);
        assert_eq!(
            evaluation.next_session().counters().valid_batches(),
            u32::from(expected.is_valid())
        );
    }

    let session = session(ReviewTier::Tier1, ReviewRepairClass::None, 0, 0, 0, 5);
    let mut drift = serde_json::to_value(attempt(
        ReviewTier::Tier1,
        1,
        "batch-drift",
        "attempt-drift",
        &["clean"],
        None,
    ))
    .unwrap();
    drift["inputFingerprint"] = json!(DRIFTED);
    let evaluation =
        ReviewSupervisor::evaluate(&session, None, serde_json::from_value(drift).unwrap()).unwrap();
    assert_eq!(
        evaluation.batch_receipt().status(),
        ReviewBatchStatusV2::InvalidInputDrift
    );
    assert_eq!(
        evaluation.next_session().status(),
        ReviewSessionStatusV2::Invalidated
    );
}

#[test]
fn last_allowed_attempt_passes_before_exhaustion_but_nonpass_stalls() {
    let session = session(ReviewTier::Tier1, ReviewRepairClass::None, 2, 0, 0, 3);
    let pass = ReviewSupervisor::evaluate(
        &session,
        None,
        attempt(
            ReviewTier::Tier1,
            3,
            "batch-pass",
            "attempt-pass",
            &["clean"],
            None,
        ),
    )
    .unwrap();
    assert_eq!(
        pass.next_session().status(),
        ReviewSessionStatusV2::Completed
    );

    let stalled = ReviewSupervisor::evaluate(
        &session,
        None,
        attempt(
            ReviewTier::Tier1,
            3,
            "batch-stalled",
            "attempt-stalled",
            &["infra_failure"],
            None,
        ),
    )
    .unwrap();
    assert_eq!(
        stalled.next_session().status(),
        ReviewSessionStatusV2::Stalled
    );
    assert!(!stalled.exit_receipt().unwrap().is_pass());
}

#[test]
fn high_risk_tier3_requires_two_consecutive_clean_batches() {
    let session = session(ReviewTier::Tier3, ReviewRepairClass::HighRisk, 0, 0, 0, 5);
    let first = ReviewSupervisor::evaluate(
        &session,
        None,
        attempt(
            ReviewTier::Tier3,
            1,
            "batch-1",
            "attempt-1",
            &["clean", "clean", "clean"],
            None,
        ),
    )
    .unwrap();
    assert_eq!(
        first.next_session().status(),
        ReviewSessionStatusV2::Running
    );
    assert!(first.exit_receipt().is_none());

    let second = ReviewSupervisor::evaluate(
        first.next_session(),
        None,
        attempt(
            ReviewTier::Tier3,
            2,
            "batch-2",
            "attempt-2",
            &["clean", "clean", "clean"],
            None,
        ),
    )
    .unwrap();
    assert_eq!(
        second.next_session().status(),
        ReviewSessionStatusV2::Completed
    );
}

#[test]
fn critical_tier3_reaches_pass_with_one_specialty_per_attempt() {
    let max_attempts = ReviewBudgetV2::for_tier(ReviewTier::Tier3).max_attempts();
    let mut current_session = session(
        ReviewTier::Tier3,
        ReviewRepairClass::CriticalContract,
        0,
        0,
        0,
        max_attempts,
    );
    let mut current_batch: Option<ReviewBatchV2> = None;

    for (index, specialty) in ["be", "ar", "qa", "be", "ar", "qa"].into_iter().enumerate() {
        let ordinal = u32::try_from(index + 1).unwrap();
        let batch_id = if ordinal <= 3 { "batch-1" } else { "batch-2" };
        let attempt_id = format!("attempt-{ordinal}");
        let evaluation = ReviewSupervisor::evaluate(
            &current_session,
            current_batch.as_ref(),
            single_specialty_attempt(ReviewTier::Tier3, ordinal, batch_id, &attempt_id, specialty),
        )
        .unwrap();

        if ordinal == 5 {
            assert_eq!(
                evaluation.next_session().status(),
                ReviewSessionStatusV2::Running
            );
        } else if ordinal == 6 {
            assert_eq!(
                evaluation.next_session().status(),
                ReviewSessionStatusV2::Completed
            );
            assert!(
                evaluation
                    .exit_receipt()
                    .is_some_and(|receipt| receipt.is_pass())
            );
        }
        current_session = evaluation.next_session().clone();
        current_batch =
            (!evaluation.next_batch().is_closed()).then(|| evaluation.next_batch().clone());
    }

    assert_eq!(current_session.counters().attempts(), 6);
    assert_eq!(current_session.status(), ReviewSessionStatusV2::Completed);
    assert!(
        current_batch.is_none(),
        "the terminal clean batch must close"
    );
}

#[test]
fn duplicate_findings_retain_one_and_never_turn_into_pass() {
    let session = session(ReviewTier::Tier1, ReviewRepairClass::None, 0, 0, 0, 5);
    let mut value = serde_json::to_value(attempt(
        ReviewTier::Tier1,
        1,
        "batch-findings",
        "attempt-findings",
        &["findings"],
        None,
    ))
    .unwrap();
    let duplicate = value["contributions"][0]["findings"][0].clone();
    value["contributions"][0]["findings"] = json!([duplicate.clone(), duplicate]);
    let evaluation =
        ReviewSupervisor::evaluate(&session, None, serde_json::from_value(value).unwrap()).unwrap();

    assert_eq!(evaluation.next_batch().finding_fingerprints().len(), 1);
    assert_eq!(
        evaluation.next_session().status(),
        ReviewSessionStatusV2::RemediationRequired
    );
    assert!(evaluation.exit_receipt().is_none());
}

#[test]
fn missing_tier2_final_proof_never_passes() {
    let session = session(ReviewTier::Tier2, ReviewRepairClass::None, 0, 0, 0, 5);
    let evaluation = ReviewSupervisor::evaluate(
        &session,
        None,
        attempt(
            ReviewTier::Tier2,
            1,
            "batch-1",
            "attempt-1",
            &["clean", "clean"],
            Some("none"),
        ),
    )
    .unwrap();

    assert_eq!(
        evaluation.batch_receipt().status(),
        ReviewBatchStatusV2::ValidClean
    );
    assert_eq!(
        evaluation.next_session().status(),
        ReviewSessionStatusV2::Running
    );
    assert!(evaluation.exit_receipt().is_none());
}
