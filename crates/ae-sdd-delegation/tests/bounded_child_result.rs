use ae_sdd_delegation::{ChildOutcome, ChildResult, ChildResultError};
use ae_sdd_domain::{ArtifactDigest, DeliverableContract};

#[test]
fn child_result_enforces_summary_and_canonical_payload_limits() {
    let contract = DeliverableContract::bounded_default([]).expect("default contract");
    let exact_summary = "x".repeat(8_192);
    let accepted = ChildResult::new(
        ChildOutcome::Succeeded,
        exact_summary,
        vec![],
        vec![],
        vec![],
        None,
        ArtifactDigest::digest(b"memory"),
        &contract,
    )
    .expect("8 KiB summary remains within 64 KiB result");
    assert!(accepted.canonical_bytes() <= 65_536);

    let rejected = ChildResult::new(
        ChildOutcome::Succeeded,
        "x".repeat(8_193),
        vec![],
        vec![],
        vec![],
        None,
        ArtifactDigest::digest(b"memory"),
        &contract,
    );
    assert!(matches!(
        rejected,
        Err(ChildResultError::SummaryTooLarge { .. })
    ));
}
