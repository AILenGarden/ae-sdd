use ae_sdd_contracts::resource::{
    DocumentTxnOperation, DocumentTxnPlan, MAX_DOCUMENT_BYTES, ResourceContractError,
};
use ae_sdd_contracts::{DocumentTxnId, SchemaVersion};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactRef, InputFingerprint, ProjectRelativePath, WorkItemId,
};
use thiserror::Error;

use crate::{ResolvedResource, ResourceResolveRequest};

/// A document selected by the resource resolver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedDocument {
    resource: ResolvedResource,
}

impl ResolvedDocument {
    /// Wraps a resolved resource as a document.
    pub const fn new(resource: ResolvedResource) -> Self {
        Self { resource }
    }

    /// Returns the selected document reference.
    pub const fn reference(&self) -> &ArtifactRef {
        self.resource.winner()
    }

    /// Returns the complete resolution result and trace.
    pub const fn resource(&self) -> &ResolvedResource {
        &self.resource
    }
}

/// Validated bounded document-read request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentReadRequest {
    reference: ArtifactRef,
    expected_digest: Option<ArtifactDigest>,
    max_bytes: u64,
}

impl DocumentReadRequest {
    /// Constructs a bounded content-addressed read request.
    pub fn new(
        reference: ArtifactRef,
        expected_digest: Option<ArtifactDigest>,
        max_bytes: u64,
    ) -> Result<Self, DocumentRequestError> {
        if max_bytes == 0 || max_bytes > MAX_DOCUMENT_BYTES {
            return Err(DocumentRequestError::InvalidReadLimit {
                max_bytes: MAX_DOCUMENT_BYTES,
            });
        }
        Ok(Self {
            reference,
            expected_digest,
            max_bytes,
        })
    }

    /// Returns the requested artifact reference.
    pub const fn reference(&self) -> &ArtifactRef {
        &self.reference
    }

    /// Returns an additional caller-owned digest expectation.
    pub const fn expected_digest(&self) -> Option<ArtifactDigest> {
        self.expected_digest
    }

    /// Returns the strict read byte limit.
    pub const fn max_bytes(&self) -> u64 {
        self.max_bytes
    }
}

/// Bounded document bytes whose digest matched the requested reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedDocument {
    reference: ArtifactRef,
    bytes: Vec<u8>,
}

impl BoundedDocument {
    /// Constructs a verified document result.
    pub const fn new(reference: ArtifactRef, bytes: Vec<u8>) -> Self {
        Self { reference, bytes }
    }

    /// Returns the verified artifact reference.
    pub const fn reference(&self) -> &ArtifactRef {
        &self.reference
    }

    /// Returns the bounded document bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Pure inputs required to construct a save plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentSaveRequest {
    transaction_id: DocumentTxnId,
    work_item_id: WorkItemId,
    target_path: ProjectRelativePath,
    staged_content_ref: ArtifactRef,
    expected_before_digest: Option<ArtifactDigest>,
    input_fingerprint: InputFingerprint,
    cleanup_policy: DocumentCleanupPolicy,
}

/// Source-draft cleanup selected explicitly by the document-save caller.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum DocumentCleanupPolicy {
    /// Preserve the staged source after saving the destination.
    #[default]
    PreserveSource,
    /// Delete the staged source in the same v2 transaction.
    DeleteSource,
}

impl DocumentSaveRequest {
    /// Validates a save request without reading or writing the project.
    pub fn new(
        transaction_id: DocumentTxnId,
        work_item_id: WorkItemId,
        target_path: ProjectRelativePath,
        staged_content_ref: ArtifactRef,
        expected_before_digest: Option<ArtifactDigest>,
        input_fingerprint: InputFingerprint,
    ) -> Result<Self, DocumentRequestError> {
        if staged_content_ref.path() == &target_path {
            return Err(DocumentRequestError::StagedContentTargetsDestination);
        }
        DocumentTxnOperation::save_staged(
            target_path.clone(),
            staged_content_ref.clone(),
            expected_before_digest,
        )?;
        Ok(Self {
            transaction_id,
            work_item_id,
            target_path,
            staged_content_ref,
            expected_before_digest,
            input_fingerprint,
            cleanup_policy: DocumentCleanupPolicy::PreserveSource,
        })
    }

    /// Selects source deletion in the same transaction as the destination save.
    pub const fn delete_source_after_save(mut self) -> Self {
        self.cleanup_policy = DocumentCleanupPolicy::DeleteSource;
        self
    }

    /// Returns the stable transaction identity.
    pub const fn transaction_id(&self) -> &DocumentTxnId {
        &self.transaction_id
    }

    /// Returns the bound Work Item.
    pub const fn work_item_id(&self) -> &WorkItemId {
        &self.work_item_id
    }

    /// Returns the project-relative target path.
    pub const fn target_path(&self) -> &ProjectRelativePath {
        &self.target_path
    }

    /// Returns the content-addressed staged input.
    pub const fn staged_content_ref(&self) -> &ArtifactRef {
        &self.staged_content_ref
    }

    /// Returns the optional target digest required before application.
    pub const fn expected_before_digest(&self) -> Option<ArtifactDigest> {
        self.expected_before_digest
    }

    /// Returns the complete caller input fingerprint.
    pub const fn input_fingerprint(&self) -> InputFingerprint {
        self.input_fingerprint
    }
}

/// Pure inputs required to construct a finalize plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentFinalizeRequest {
    transaction_id: DocumentTxnId,
    work_item_id: WorkItemId,
    target_path: ProjectRelativePath,
    expected_digest: ArtifactDigest,
    input_fingerprint: InputFingerprint,
}

impl DocumentFinalizeRequest {
    /// Constructs a finalize request.
    pub const fn new(
        transaction_id: DocumentTxnId,
        work_item_id: WorkItemId,
        target_path: ProjectRelativePath,
        expected_digest: ArtifactDigest,
        input_fingerprint: InputFingerprint,
    ) -> Self {
        Self {
            transaction_id,
            work_item_id,
            target_path,
            expected_digest,
            input_fingerprint,
        }
    }

    /// Returns the stable transaction identity.
    pub const fn transaction_id(&self) -> &DocumentTxnId {
        &self.transaction_id
    }

    /// Returns the bound Work Item.
    pub const fn work_item_id(&self) -> &WorkItemId {
        &self.work_item_id
    }

    /// Returns the project-relative target path.
    pub const fn target_path(&self) -> &ProjectRelativePath {
        &self.target_path
    }

    /// Returns the digest expected at finalization.
    pub const fn expected_digest(&self) -> ArtifactDigest {
        self.expected_digest
    }

    /// Returns the complete caller input fingerprint.
    pub const fn input_fingerprint(&self) -> InputFingerprint {
        self.input_fingerprint
    }
}

/// Pure constructor for replay-safe document transaction plans.
#[derive(Clone, Copy, Debug, Default)]
pub struct DocumentPlanner;

impl DocumentPlanner {
    /// Creates a single-operation save plan without project mutation.
    pub fn save(request: &DocumentSaveRequest) -> Result<DocumentTxnPlan, DocumentPlanError> {
        let save = DocumentTxnOperation::save_staged(
            request.target_path.clone(),
            request.staged_content_ref.clone(),
            request.expected_before_digest,
        )?;
        let mut operations = vec![save];
        let schema_version = match request.cleanup_policy {
            DocumentCleanupPolicy::PreserveSource => SchemaVersion::V1,
            DocumentCleanupPolicy::DeleteSource => {
                operations.push(DocumentTxnOperation::delete(
                    request.staged_content_ref.path().clone(),
                    request.staged_content_ref.digest(),
                ));
                SchemaVersion::V2
            }
        };
        Ok(DocumentTxnPlan::new(
            schema_version,
            request.transaction_id.clone(),
            request.work_item_id.clone(),
            operations,
            request.input_fingerprint,
        )?)
    }

    /// Creates a single-operation finalize plan without project mutation.
    pub fn finalize(
        request: &DocumentFinalizeRequest,
    ) -> Result<DocumentTxnPlan, DocumentPlanError> {
        Ok(DocumentTxnPlan::new(
            SchemaVersion::V1,
            request.transaction_id.clone(),
            request.work_item_id.clone(),
            vec![DocumentTxnOperation::finalize(
                request.target_path.clone(),
                request.expected_digest,
            )],
            request.input_fingerprint,
        )?)
    }
}

/// Application port for typed document resolution, bounded reads, and pure plans.
pub trait DocumentPort {
    /// Adapter-specific failure type.
    type Error;

    /// Resolves and validates a document resource.
    fn resolve(&self, request: &ResourceResolveRequest) -> Result<ResolvedDocument, Self::Error>;

    /// Reads a content-addressed document within its byte budget.
    fn read(&self, request: &DocumentReadRequest) -> Result<BoundedDocument, Self::Error>;

    /// Returns a replay-safe save plan without applying it.
    fn save(&self, request: &DocumentSaveRequest) -> Result<DocumentTxnPlan, Self::Error>;

    /// Returns a replay-safe finalize plan without applying it.
    fn finalize(&self, request: &DocumentFinalizeRequest) -> Result<DocumentTxnPlan, Self::Error>;
}

/// Invalid typed document request.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DocumentRequestError {
    /// Read limit was zero or exceeded the contract maximum.
    #[error("document read limit must be between 1 and {max_bytes} bytes")]
    InvalidReadLimit {
        /// Maximum accepted limit.
        max_bytes: u64,
    },
    /// Staged content pointed at the destination and was not independently addressable.
    #[error("staged document content must not use the destination path")]
    StagedContentTargetsDestination,
    /// Frozen transaction-contract validation failed.
    #[error(transparent)]
    Contract(#[from] ResourceContractError),
}

/// Failure to construct a frozen transaction plan.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DocumentPlanError {
    /// Frozen transaction-contract validation failed.
    #[error(transparent)]
    Contract(#[from] ResourceContractError),
}
