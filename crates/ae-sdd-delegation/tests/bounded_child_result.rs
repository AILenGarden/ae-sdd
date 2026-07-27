use ae_sdd_delegation::{
    ChildDeliverable, ChildFinding, ChildOutcome, ChildResult, ChildResultError,
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, DeliverableContract, DeliverableId, EvidenceDigest,
    EvidenceId, EvidenceRef, FindingCode, OperationId, ProjectRelativePath, VerificationId,
};

fn artifact(path: &str, content: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new("report").expect("valid artifact kind"),
        ProjectRelativePath::new(path).expect("valid project-relative path"),
        ArtifactDigest::digest(content),
        u64::try_from(content.len()).expect("fixture length"),
    )
}

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

    let empty = ChildResult::new(
        ChildOutcome::Succeeded,
        "",
        vec![],
        vec![],
        vec![],
        None,
        ArtifactDigest::digest(b"memory"),
        &contract,
    );
    assert!(matches!(empty, Err(ChildResultError::EmptySummary)));
}

#[test]
fn rich_child_result_round_trips_all_bounded_fields() {
    let contract = DeliverableContract::bounded_default([]).expect("default contract");
    let finding = ChildFinding::new(
        FindingCode::new("F-001").expect("valid finding code"),
        "a concrete finding",
    )
    .expect("valid finding");
    let deliverable = ChildDeliverable::new(
        DeliverableId::new("report").expect("valid deliverable id"),
        artifact("out/report.json", b"report"),
    );
    let evidence = EvidenceRef::new(
        EvidenceId::new("evidence-1").expect("valid evidence id"),
        VerificationId::new("V-021").expect("valid verification id"),
        ProjectRelativePath::new("target/evidence.json").expect("valid evidence path"),
        EvidenceDigest::digest(b"evidence"),
        8,
    );
    let requested_action = OperationId::new("state.transition").expect("valid operation");
    let memory_snapshot = ArtifactDigest::digest(b"memory");

    let result = ChildResult::new(
        ChildOutcome::Blocked,
        "bounded result",
        vec![finding.clone()],
        vec![deliverable.clone()],
        vec![evidence.clone()],
        Some(requested_action.clone()),
        memory_snapshot,
        &contract,
    )
    .expect("valid rich result");

    assert_eq!(finding.code().as_str(), "F-001");
    assert_eq!(finding.message(), "a concrete finding");
    assert_eq!(deliverable.id().as_str(), "report");
    assert_eq!(deliverable.artifact().path().as_str(), "out/report.json");
    assert_eq!(result.outcome(), ChildOutcome::Blocked);
    assert_eq!(result.summary(), "bounded result");
    assert_eq!(result.findings(), &[finding]);
    assert_eq!(result.deliverables(), &[deliverable]);
    assert_eq!(result.evidence(), &[evidence]);
    assert_eq!(result.requested_action(), Some(&requested_action));
    assert_eq!(result.memory_snapshot_digest(), memory_snapshot);
    assert!(result.canonical_bytes() > 0);
    assert_ne!(result.digest(), ae_sdd_domain::ResultDigest::digest([]));
    assert_eq!(result.schema_version(), 1);
}

#[test]
fn canonical_result_distinguishes_every_outcome() {
    let contract = DeliverableContract::bounded_default([]).expect("default contract");
    let digests = [
        ChildOutcome::Succeeded,
        ChildOutcome::Blocked,
        ChildOutcome::Failed,
        ChildOutcome::Cancelled,
    ]
    .into_iter()
    .map(|outcome| {
        ChildResult::new(
            outcome,
            "same payload",
            vec![],
            vec![],
            vec![],
            None,
            ArtifactDigest::digest(b"memory"),
            &contract,
        )
        .expect("valid outcome")
        .digest()
    })
    .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(digests.len(), 4);
}

#[test]
fn findings_and_deliverables_reject_invalid_or_duplicate_content() {
    assert!(matches!(
        ChildFinding::new(FindingCode::new("F-EMPTY").expect("valid finding code"), ""),
        Err(ChildResultError::InvalidFindingMessage)
    ));
    assert!(matches!(
        ChildFinding::new(
            FindingCode::new("F-LARGE").expect("valid finding code"),
            "x".repeat(2_049)
        ),
        Err(ChildResultError::InvalidFindingMessage)
    ));

    let contract = DeliverableContract::bounded_default([]).expect("default contract");
    let duplicate_id = DeliverableId::new("report").expect("valid deliverable id");
    let duplicate = ChildResult::new(
        ChildOutcome::Succeeded,
        "duplicate deliverables",
        vec![],
        vec![
            ChildDeliverable::new(duplicate_id.clone(), artifact("out/first.json", b"first")),
            ChildDeliverable::new(duplicate_id, artifact("out/second.json", b"second")),
        ],
        vec![],
        None,
        ArtifactDigest::digest(b"memory"),
        &contract,
    );
    assert!(matches!(
        duplicate,
        Err(ChildResultError::DuplicateDeliverable(id)) if id.as_str() == "report"
    ));
}
