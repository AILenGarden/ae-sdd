use std::cell::Cell;

use ae_sdd_delegation::{
    ArtifactValidationError, ArtifactValidationReceipt, ArtifactVerifier, ChildDeliverable,
    ChildOutcome, ChildResult, CleanupError, MemoryCleanupReceipt,
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, DelegationId, DeliverableContract, DeliverableId,
    DeliverableRequirement, ProjectPathScope, ProjectRelativePath, ScopedGrant,
};
use uuid::Uuid;

fn delegation(seed: u128) -> DelegationId {
    DelegationId::from_uuid(Uuid::from_u128(seed))
}

fn path(value: &str) -> ProjectRelativePath {
    ProjectRelativePath::new(value).expect("valid project-relative path")
}

fn kind(value: &str) -> ArtifactKind {
    ArtifactKind::new(value).expect("valid artifact kind")
}

fn artifact(kind_name: &str, path_name: &str, content: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        kind(kind_name),
        path(path_name),
        ArtifactDigest::digest(content),
        u64::try_from(content.len()).expect("fixture length"),
    )
}

fn deliverable(id: &str, kind_name: &str, path_name: &str, content: &[u8]) -> ChildDeliverable {
    ChildDeliverable::new(
        DeliverableId::new(id).expect("valid deliverable id"),
        artifact(kind_name, path_name, content),
    )
}

fn contract() -> DeliverableContract {
    DeliverableContract::bounded_default([DeliverableRequirement::new(
        DeliverableId::new("report").expect("valid deliverable id"),
        kind("report"),
        path("out/report.json"),
    )])
    .expect("valid deliverable contract")
}

fn result(contract: &DeliverableContract, deliverables: Vec<ChildDeliverable>) -> ChildResult {
    ChildResult::new(
        ChildOutcome::Succeeded,
        "artifact validation result",
        vec![],
        deliverables,
        vec![],
        None,
        ArtifactDigest::digest(b"memory snapshot"),
        contract,
    )
    .expect("valid child result")
}

struct CountingVerifier {
    calls: Cell<usize>,
    failure: Option<ArtifactValidationError>,
}

impl CountingVerifier {
    fn accepting() -> Self {
        Self {
            calls: Cell::new(0),
            failure: None,
        }
    }

    fn rejecting(failure: ArtifactValidationError) -> Self {
        Self {
            calls: Cell::new(0),
            failure: Some(failure),
        }
    }
}

impl ArtifactVerifier for CountingVerifier {
    fn verify(&self, _artifact: &ArtifactRef) -> Result<(), ArtifactValidationError> {
        self.calls.set(self.calls.get() + 1);
        match &self.failure {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

#[test]
fn required_and_optional_artifacts_are_verified_and_bound_to_the_result() {
    let contract = contract();
    let result = result(
        &contract,
        vec![
            deliverable("report", "report", "out/report.json", b"report"),
            deliverable("trace", "trace", "out/trace.log", b"trace"),
        ],
    );
    let grant = ScopedGrant::new([], [], [ProjectPathScope::ProjectRoot]);
    let verifier = CountingVerifier::accepting();

    let receipt =
        ArtifactValidationReceipt::validate(delegation(1), &result, &contract, &grant, &verifier)
            .expect("all artifacts validate");

    assert_eq!(verifier.calls.get(), 2);
    assert_eq!(receipt.delegation_id(), delegation(1));
    assert_eq!(receipt.result_digest(), result.digest());
    assert_eq!(receipt.artifacts().len(), 2);
    assert_eq!(receipt.artifacts()[0].deliverable_id().as_str(), "report");
    assert_eq!(receipt.artifacts()[0].path().as_str(), "out/report.json");
    assert_eq!(
        receipt.artifacts()[0].digest(),
        ArtifactDigest::digest(b"report")
    );
    assert_eq!(receipt.artifacts()[1].deliverable_id().as_str(), "trace");
}

#[test]
fn required_artifact_contract_rejects_missing_kind_path_and_grant_mismatches() {
    let contract = contract();
    let root = ScopedGrant::new([], [], [ProjectPathScope::ProjectRoot]);
    let verifier = CountingVerifier::accepting();

    let missing = result(&contract, vec![]);
    assert!(matches!(
        ArtifactValidationReceipt::validate(
            delegation(2),
            &missing,
            &contract,
            &root,
            &verifier
        ),
        Err(ArtifactValidationError::RequiredDeliverableMissing(id)) if id.as_str() == "report"
    ));

    let wrong_kind = result(
        &contract,
        vec![deliverable(
            "report",
            "wrong-kind",
            "out/report.json",
            b"report",
        )],
    );
    assert!(matches!(
        ArtifactValidationReceipt::validate(
            delegation(2),
            &wrong_kind,
            &contract,
            &root,
            &verifier
        ),
        Err(ArtifactValidationError::KindMismatch(id)) if id.as_str() == "report"
    ));

    let wrong_path = result(
        &contract,
        vec![deliverable("report", "report", "out/other.json", b"report")],
    );
    assert!(matches!(
        ArtifactValidationReceipt::validate(
            delegation(2),
            &wrong_path,
            &contract,
            &root,
            &verifier
        ),
        Err(ArtifactValidationError::PathMismatch(id)) if id.as_str() == "report"
    ));

    let supplied = result(
        &contract,
        vec![deliverable(
            "report",
            "report",
            "out/report.json",
            b"report",
        )],
    );
    let other_subtree = ScopedGrant::new([], [], [ProjectPathScope::Subtree(path("elsewhere"))]);
    assert!(matches!(
        ArtifactValidationReceipt::validate(
            delegation(2),
            &supplied,
            &contract,
            &other_subtree,
            &verifier
        ),
        Err(ArtifactValidationError::PathOutsideGrant(path))
            if path.as_str() == "out/report.json"
    ));
}

#[test]
fn optional_artifacts_remain_grant_scoped_and_verifier_failures_propagate() {
    let empty_contract = DeliverableContract::bounded_default([]).expect("valid contract");
    let optional = result(
        &empty_contract,
        vec![deliverable("trace", "trace", "outside/trace.log", b"trace")],
    );
    let out_only = ScopedGrant::new([], [], [ProjectPathScope::Subtree(path("out"))]);
    assert!(matches!(
        ArtifactValidationReceipt::validate(
            delegation(3),
            &optional,
            &empty_contract,
            &out_only,
            &CountingVerifier::accepting()
        ),
        Err(ArtifactValidationError::PathOutsideGrant(path))
            if path.as_str() == "outside/trace.log"
    ));

    let contract = contract();
    let required = result(
        &contract,
        vec![deliverable(
            "report",
            "report",
            "out/report.json",
            b"report",
        )],
    );
    for failure in [
        ArtifactValidationError::DigestMismatch,
        ArtifactValidationError::ReadFailed("disk unavailable".into()),
    ] {
        let verifier = CountingVerifier::rejecting(failure.clone());
        assert_eq!(
            ArtifactValidationReceipt::validate(
                delegation(3),
                &required,
                &contract,
                &ScopedGrant::new([], [], [ProjectPathScope::ProjectRoot]),
                &verifier,
            ),
            Err(failure)
        );
        assert_eq!(verifier.calls.get(), 1);
    }
}

#[test]
fn cleanup_receipt_validates_identity_snapshot_namespace_and_timestamp() {
    let snapshot = ArtifactDigest::digest(b"snapshot");
    let cleanup = ArtifactDigest::digest(b"cleanup");
    let receipt = MemoryCleanupReceipt::new(
        delegation(4),
        "delegation/4/private",
        snapshot,
        cleanup,
        9_000,
    )
    .expect("valid cleanup receipt");

    assert_eq!(receipt.delegation_id(), delegation(4));
    assert_eq!(receipt.snapshot_digest(), snapshot);
    assert_eq!(receipt.namespace(), "delegation/4/private");
    assert_eq!(receipt.cleanup_digest(), cleanup);
    assert_eq!(receipt.cleaned_at_unix_ms(), 9_000);
    assert_eq!(
        MemoryCleanupReceipt::new(delegation(4), "", snapshot, cleanup, 9_000),
        Err(CleanupError::InvalidReceipt)
    );
    assert_eq!(
        MemoryCleanupReceipt::new(delegation(4), "delegation/4/private", snapshot, cleanup, 0),
        Err(CleanupError::InvalidReceipt)
    );
}
