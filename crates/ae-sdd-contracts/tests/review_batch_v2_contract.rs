use ae_sdd_contracts::review::{
    MAX_REVIEW_V2_COUNT, ReviewAttemptV2, ReviewBatchStatusV2, ReviewBudgetV2, ReviewCountersV2,
    ReviewSchemaVersion, ReviewSession, ReviewSessionV2, ReviewTier,
};
use serde_json::{Value, json};

const INPUT: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const RULESET: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const DIGEST: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const AUTHOR: &str = "00000000-0000-0000-0000-000000000099";
const ROOT: &str = "00000000-0000-0000-0000-000000000001";

fn session_fixture() -> Value {
    json!({
        "schemaVersion": "v2",
        "reviewId": "review-v2-1",
        "parentReviewId": null,
        "tier": "tier2",
        "requiredSpecialties": ["be", "ar"],
        "authorSessionId": AUTHOR,
        "rootSessionId": ROOT,
        "inputFingerprint": INPUT,
        "rulesetFingerprint": RULESET,
        "policyDigest": DIGEST,
        "sourceRevision": 7,
        "inventoryGeneration": 3,
        "repairClass": "none",
        "cleanPolicy": {
            "cleanTarget": 1,
            "finalProofRequirement": "deterministic_gates"
        },
        "budget": {
            "maxAttempts": 4,
            "maxValidBatches": 3,
            "maxRemediations": 2,
            "maxWallClockMinutes": 60
        },
        "counters": {
            "attempts": 0,
            "validBatches": 0,
            "cleanStreak": 0,
            "remediations": 0,
            "infraFailures": 0,
            "protocolFailures": 0
        },
        "status": "running",
        "startedAt": "2026-07-25T10:00:00Z",
        "deadlineAt": "2026-07-25T11:00:00Z",
        "terminalAt": null
    })
}

fn contribution(specialty: &str, session: &str, delegation: &str) -> Value {
    json!({
        "sourceAttemptId": "attempt-1",
        "reviewer": {
            "agentRole": "reviewer",
            "specialty": specialty,
            "grantedSpecialties": [specialty],
            "physicalSessionId": session,
            "rootSessionId": ROOT,
            "delegationId": delegation,
            "lineageDepth": 2,
            "attestationRef": "evidence/reviewer-attestation.json",
            "attestationDigest": DIGEST,
            "specialtyGrantDigest": DIGEST
        },
        "outcome": "clean",
        "findings": [],
        "reportDigest": DIGEST,
        "contributionDigest": DIGEST,
        "inputFingerprint": INPUT,
        "rulesetFingerprint": RULESET
    })
}

fn attempt_fixture() -> Value {
    json!({
        "schemaVersion": "v2",
        "reviewId": "review-v2-1",
        "batchId": "batch-1",
        "attemptId": "attempt-1",
        "attemptOrdinal": 1,
        "idempotencyKey": "review-attempt-1",
        "inputFingerprint": INPUT,
        "rulesetFingerprint": RULESET,
        "contributions": [
            contribution(
                "be",
                "00000000-0000-0000-0000-000000000010",
                "00000000-0000-0000-0000-000000000110"
            ),
            contribution(
                "ar",
                "00000000-0000-0000-0000-000000000011",
                "00000000-0000-0000-0000-000000000111"
            )
        ],
        "observedAt": "2026-07-25T10:01:00Z",
        "finalProof": {
            "kind": "deterministic_gates",
            "digest": DIGEST,
            "sourceRevision": 7,
            "inputFingerprint": INPUT,
            "rulesetFingerprint": RULESET,
            "observedAt": "2026-07-25T10:00:30Z"
        },
        "projectAuthority": {
            "projectReceiptRef": "evidence/review-receipt.json",
            "activeManifestDigest": DIGEST,
            "stateReceiptRefDigest": DIGEST,
            "journalMutationId": "mutation-1"
        },
        "remediation": null
    })
}

#[test]
fn v2_session_and_attempt_round_trip_losslessly() {
    let session: ReviewSessionV2 = serde_json::from_value(session_fixture()).expect("v2 session");
    let attempt: ReviewAttemptV2 = serde_json::from_value(attempt_fixture()).expect("v2 attempt");

    assert_eq!(serde_json::to_value(session).unwrap(), session_fixture());
    assert_eq!(serde_json::to_value(attempt).unwrap(), attempt_fixture());
    assert_eq!(
        ReviewSchemaVersion::V2,
        serde_json::from_str("\"v2\"").unwrap()
    );
}

#[test]
fn exact_batch_status_wire_values_are_frozen() {
    let cases = [
        ("VALID_CLEAN", ReviewBatchStatusV2::ValidClean),
        ("VALID_FINDINGS", ReviewBatchStatusV2::ValidFindings),
        ("INVALID_INFRA", ReviewBatchStatusV2::InvalidInfra),
        ("INVALID_PROTOCOL", ReviewBatchStatusV2::InvalidProtocol),
        (
            "INVALID_INPUT_DRIFT",
            ReviewBatchStatusV2::InvalidInputDrift,
        ),
        ("CANCELLED", ReviewBatchStatusV2::Cancelled),
    ];
    for (wire, expected) in cases {
        assert_eq!(
            serde_json::from_str::<ReviewBatchStatusV2>(&format!("\"{wire}\"")).unwrap(),
            expected
        );
        assert_eq!(
            serde_json::to_string(&expected).unwrap(),
            format!("\"{wire}\"")
        );
    }
    assert!(serde_json::from_str::<ReviewBatchStatusV2>("\"valid_clean\"").is_err());
}

#[test]
fn v2_rejects_unknown_fields_at_every_published_boundary() {
    let mut session = session_fixture();
    session["unexpected"] = json!(true);
    assert!(serde_json::from_value::<ReviewSessionV2>(session).is_err());

    let mut attempt = attempt_fixture();
    attempt["contributions"][0]["reviewer"]["trusted"] = json!(true);
    assert!(serde_json::from_value::<ReviewAttemptV2>(attempt).is_err());
}

#[test]
fn v2_rejects_out_of_range_budget_ordinal_and_contribution_counts() {
    for invalid in [0, 33] {
        let mut session = session_fixture();
        session["budget"]["maxAttempts"] = json!(invalid);
        assert!(serde_json::from_value::<ReviewSessionV2>(session).is_err());

        let mut attempt = attempt_fixture();
        attempt["attemptOrdinal"] = json!(invalid);
        assert!(serde_json::from_value::<ReviewAttemptV2>(attempt).is_err());
    }

    let mut empty = attempt_fixture();
    empty["contributions"] = json!([]);
    assert!(serde_json::from_value::<ReviewAttemptV2>(empty).is_err());

    let mut too_many = attempt_fixture();
    let item = too_many["contributions"][0].clone();
    too_many["contributions"] =
        json!([item.clone(), item.clone(), item.clone(), item.clone(), item]);
    assert!(serde_json::from_value::<ReviewAttemptV2>(too_many).is_err());
}

#[test]
fn v2_rejects_noncanonical_digest_timestamp_and_tier_matrix() {
    let mut uppercase_digest = session_fixture();
    uppercase_digest["inputFingerprint"] = json!("A".repeat(64));
    assert!(serde_json::from_value::<ReviewSessionV2>(uppercase_digest).is_err());

    let mut offset_timestamp = session_fixture();
    offset_timestamp["startedAt"] = json!("2026-07-25T18:00:00+08:00");
    assert!(serde_json::from_value::<ReviewSessionV2>(offset_timestamp).is_err());

    let mut downgraded = session_fixture();
    downgraded["requiredSpecialties"] = json!(["be"]);
    assert!(serde_json::from_value::<ReviewSessionV2>(downgraded).is_err());
}

#[test]
fn legacy_v1_is_read_only_and_never_decodes_as_v2() {
    let v1 = json!({
        "schemaVersion": "v1",
        "reviewId": "legacy-review",
        "tier": "tier1",
        "requiredRoles": ["engineering"],
        "inputFingerprint": INPUT,
        "rulesetFingerprint": RULESET,
        "round": 1,
        "cleanStreak": 0,
        "budget": {"maxRounds": 3, "maxFindings": 16, "maxDurationMs": 60000},
        "status": "running"
    });
    assert!(serde_json::from_value::<ReviewSession>(v1.clone()).is_ok());
    assert!(serde_json::from_value::<ReviewSessionV2>(v1).is_err());
    assert!(serde_json::from_value::<ReviewSession>(session_fixture()).is_err());
}

#[test]
fn v2_counter_advancement_fails_closed_at_the_bound() {
    let counters = ReviewCountersV2::new(
        MAX_REVIEW_V2_COUNT,
        MAX_REVIEW_V2_COUNT,
        MAX_REVIEW_V2_COUNT,
        0,
        0,
        0,
    )
    .expect("bounded counters");

    assert!(
        counters
            .after_attempt(ReviewBatchStatusV2::ValidClean)
            .is_err()
    );
}

#[test]
fn tier3_default_attempt_budget_covers_two_clean_specialty_sets() {
    let budget = ReviewBudgetV2::for_tier(ReviewTier::Tier3);

    assert_eq!(budget.max_attempts(), 6);
}
