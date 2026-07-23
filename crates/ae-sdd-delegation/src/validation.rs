use std::collections::BTreeMap;

use ae_sdd_domain::{
    ArtifactDigest, ArtifactRef, DelegationId, DeliverableContract, DeliverableId,
    ProjectRelativePath, ResultDigest, ScopedGrant,
};
use thiserror::Error;

use crate::{ChildDeliverable, ChildResult};

pub trait ArtifactVerifier {
    fn verify(&self, artifact: &ArtifactRef) -> Result<(), ArtifactValidationError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedArtifact {
    deliverable_id: DeliverableId,
    path: ProjectRelativePath,
    digest: ArtifactDigest,
}

impl ValidatedArtifact {
    #[must_use]
    pub const fn deliverable_id(&self) -> &DeliverableId {
        &self.deliverable_id
    }

    #[must_use]
    pub const fn path(&self) -> &ProjectRelativePath {
        &self.path
    }

    #[must_use]
    pub const fn digest(&self) -> ArtifactDigest {
        self.digest
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactValidationReceipt {
    delegation_id: DelegationId,
    result_digest: ResultDigest,
    artifacts: Vec<ValidatedArtifact>,
}

impl ArtifactValidationReceipt {
    pub fn validate(
        delegation_id: DelegationId,
        result: &ChildResult,
        contract: &DeliverableContract,
        grant: &ScopedGrant,
        verifier: &dyn ArtifactVerifier,
    ) -> Result<Self, ArtifactValidationError> {
        let actual: BTreeMap<_, _> = result
            .deliverables()
            .iter()
            .map(|item| (item.id().clone(), item))
            .collect();
        let mut artifacts = Vec::with_capacity(contract.required().len());

        for (id, required) in contract.required() {
            let supplied = actual
                .get(id)
                .ok_or_else(|| ArtifactValidationError::RequiredDeliverableMissing(id.clone()))?;
            validate_required(supplied, required.kind(), required.path(), grant)?;
            verifier.verify(supplied.artifact())?;
            artifacts.push(ValidatedArtifact {
                deliverable_id: id.clone(),
                path: supplied.artifact().path().clone(),
                digest: supplied.artifact().digest(),
            });
        }

        for supplied in result.deliverables() {
            if !grant.permits_path(supplied.artifact().path()) {
                return Err(ArtifactValidationError::PathOutsideGrant(
                    supplied.artifact().path().clone(),
                ));
            }
            if !contract.required().contains_key(supplied.id()) {
                verifier.verify(supplied.artifact())?;
                artifacts.push(ValidatedArtifact {
                    deliverable_id: supplied.id().clone(),
                    path: supplied.artifact().path().clone(),
                    digest: supplied.artifact().digest(),
                });
            }
        }

        Ok(Self {
            delegation_id,
            result_digest: result.digest(),
            artifacts,
        })
    }

    #[must_use]
    pub const fn delegation_id(&self) -> DelegationId {
        self.delegation_id
    }

    #[must_use]
    pub const fn result_digest(&self) -> ResultDigest {
        self.result_digest
    }

    #[must_use]
    pub fn artifacts(&self) -> &[ValidatedArtifact] {
        &self.artifacts
    }
}

fn validate_required(
    supplied: &ChildDeliverable,
    required_kind: &ae_sdd_domain::ArtifactKind,
    required_path: &ProjectRelativePath,
    grant: &ScopedGrant,
) -> Result<(), ArtifactValidationError> {
    if supplied.artifact().kind() != required_kind {
        return Err(ArtifactValidationError::KindMismatch(supplied.id().clone()));
    }
    if supplied.artifact().path() != required_path {
        return Err(ArtifactValidationError::PathMismatch(supplied.id().clone()));
    }
    if !grant.permits_path(supplied.artifact().path()) {
        return Err(ArtifactValidationError::PathOutsideGrant(
            supplied.artifact().path().clone(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ArtifactValidationError {
    #[error("required deliverable {0} is missing")]
    RequiredDeliverableMissing(DeliverableId),
    #[error("deliverable {0} has the wrong artifact kind")]
    KindMismatch(DeliverableId),
    #[error("deliverable {0} has the wrong path")]
    PathMismatch(DeliverableId),
    #[error("artifact path {0} is outside the delegation grant")]
    PathOutsideGrant(ProjectRelativePath),
    #[error("artifact content digest does not match the referenced file")]
    DigestMismatch,
    #[error("artifact could not be read: {0}")]
    ReadFailed(Box<str>),
}

pub trait MemoryNamespaceCleaner {
    fn clean(
        &self,
        delegation_id: DelegationId,
        expected_snapshot: ArtifactDigest,
    ) -> Result<MemoryCleanupReceipt, CleanupError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryCleanupReceipt {
    delegation_id: DelegationId,
    namespace: Box<str>,
    snapshot_digest: ArtifactDigest,
    cleanup_digest: ArtifactDigest,
    cleaned_at_unix_ms: u64,
}

impl MemoryCleanupReceipt {
    pub fn new(
        delegation_id: DelegationId,
        namespace: impl Into<Box<str>>,
        snapshot_digest: ArtifactDigest,
        cleanup_digest: ArtifactDigest,
        cleaned_at_unix_ms: u64,
    ) -> Result<Self, CleanupError> {
        let namespace = namespace.into();
        if namespace.is_empty() || cleaned_at_unix_ms == 0 {
            return Err(CleanupError::InvalidReceipt);
        }
        Ok(Self {
            delegation_id,
            namespace,
            snapshot_digest,
            cleanup_digest,
            cleaned_at_unix_ms,
        })
    }

    #[must_use]
    pub const fn delegation_id(&self) -> DelegationId {
        self.delegation_id
    }

    #[must_use]
    pub const fn snapshot_digest(&self) -> ArtifactDigest {
        self.snapshot_digest
    }

    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    #[must_use]
    pub const fn cleanup_digest(&self) -> ArtifactDigest {
        self.cleanup_digest
    }

    #[must_use]
    pub const fn cleaned_at_unix_ms(&self) -> u64 {
        self.cleaned_at_unix_ms
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CleanupError {
    #[error("memory cleanup receipt is invalid")]
    InvalidReceipt,
    #[error("memory namespace cleanup failed: {0}")]
    Failed(Box<str>),
}
