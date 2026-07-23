//! Frozen resource, context-proof, and document-transaction contracts.
//!
//! These values are intentionally transport-only.  They carry content-addressed
//! references and bounded plans, but never open files or mutate a workspace.

use std::collections::BTreeSet;

use ae_sdd_domain::{
    ArtifactDigest, ArtifactRef, ContextDigest, InputFingerprint, InventoryGeneration,
    ProjectRelativePath, StateRevision, WorkItemId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ContextBundleId, DocumentTxnId, MethodologyRef, SchemaVersion, serde_domain};

/// Maximum number of artifact references in one context bundle.
pub const MAX_CONTEXT_ARTIFACTS: usize = 64;
/// Maximum byte budget represented by one context bundle.
pub const MAX_CONTEXT_BYTES: u64 = 64 * 1024;
/// Maximum number of operations in one document transaction plan.
pub const MAX_DOCUMENT_OPERATIONS: usize = 64;
/// Maximum document size accepted by a transaction operation.
pub const MAX_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;

/// Structural validation errors for resource contracts.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResourceContractError {
    /// A context bundle had no content references.
    #[error("context bundle must contain at least one artifact")]
    EmptyArtifacts,
    /// A bounded collection exceeded its v1 limit.
    #[error("{field} exceeds its {max_items}-item limit")]
    CollectionLimitExceeded {
        /// Collection name.
        field: &'static str,
        /// Maximum item count.
        max_items: usize,
    },
    /// The context byte budget was exceeded.
    #[error("context bundle exceeds the {max_bytes}-byte budget")]
    ContextByteBudgetExceeded {
        /// Maximum accepted context size.
        max_bytes: u64,
    },
    /// Two references targeted the same path.
    #[error("context bundle contains duplicate artifact paths")]
    DuplicateArtifactPath,
    /// A nested proof belonged to a different Work Item.
    #[error("resource contract Work Item does not match its nested reference")]
    WorkItemMismatch,
    /// The proof repeated a digest that did not match its context reference.
    #[error("loaded context proof bundle digest does not match its context reference")]
    BundleDigestMismatch,
    /// The proof did not bind the Story artifact into the context bundle.
    #[error("loaded context proof Story artifact is not present in the bundle")]
    StoryNotInBundle,
    /// A transaction had no operations.
    #[error("document transaction must contain at least one operation")]
    EmptyOperations,
    /// Two operations targeted the same path.
    #[error("document transaction contains duplicate target paths")]
    DuplicateOperationPath,
    /// One operation exceeded the document byte budget.
    #[error("document operation exceeds the {max_bytes}-byte budget")]
    DocumentByteBudgetExceeded {
        /// Maximum accepted document size.
        max_bytes: u64,
    },
}

/// Content-addressed, bounded context input selected for one Work Item.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "ContextBundleRefWire", into = "ContextBundleRefWire")]
pub struct ContextBundleRef {
    schema_version: SchemaVersion,
    context_id: ContextBundleId,
    work_item_id: WorkItemId,
    artifact_refs: Vec<ArtifactRef>,
    bundle_digest: ContextDigest,
    byte_length: u64,
}

impl ContextBundleRef {
    /// Maximum number of artifacts accepted by this contract.
    pub const MAX_ARTIFACTS: usize = MAX_CONTEXT_ARTIFACTS;
    /// Maximum byte budget accepted by this contract.
    pub const MAX_BYTES: u64 = MAX_CONTEXT_BYTES;

    /// Constructs and validates a context bundle reference.
    pub fn new(
        schema_version: SchemaVersion,
        context_id: ContextBundleId,
        work_item_id: WorkItemId,
        artifact_refs: Vec<ArtifactRef>,
        bundle_digest: ContextDigest,
        byte_length: u64,
    ) -> Result<Self, ResourceContractError> {
        if artifact_refs.is_empty() {
            return Err(ResourceContractError::EmptyArtifacts);
        }
        if artifact_refs.len() > MAX_CONTEXT_ARTIFACTS {
            return Err(ResourceContractError::CollectionLimitExceeded {
                field: "artifactRefs",
                max_items: MAX_CONTEXT_ARTIFACTS,
            });
        }
        if byte_length == 0 || byte_length > MAX_CONTEXT_BYTES {
            return Err(ResourceContractError::ContextByteBudgetExceeded {
                max_bytes: MAX_CONTEXT_BYTES,
            });
        }
        let mut paths = BTreeSet::new();
        if artifact_refs
            .iter()
            .any(|reference| !paths.insert(reference.path().to_string()))
        {
            return Err(ResourceContractError::DuplicateArtifactPath);
        }
        Ok(Self {
            schema_version,
            context_id,
            work_item_id,
            artifact_refs,
            bundle_digest,
            byte_length,
        })
    }

    /// Returns the context identity.
    pub const fn context_id(&self) -> &ContextBundleId {
        &self.context_id
    }

    /// Returns the bound Work Item identity.
    pub const fn work_item_id(&self) -> &WorkItemId {
        &self.work_item_id
    }

    /// Returns the immutable artifact references in canonical order supplied by the caller.
    pub fn artifact_refs(&self) -> &[ArtifactRef] {
        &self.artifact_refs
    }

    /// Returns the digest of the complete context bundle.
    pub const fn bundle_digest(&self) -> ContextDigest {
        self.bundle_digest
    }

    /// Returns the encoded byte budget claimed by the bundle.
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

impl<'de> Deserialize<'de> for ContextBundleRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ContextBundleRefWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextBundleRefWire {
    schema_version: SchemaVersion,
    context_id: ContextBundleId,
    #[serde(with = "serde_domain::work_item_id")]
    work_item_id: WorkItemId,
    #[serde(with = "serde_domain::artifact_refs")]
    artifact_refs: Vec<ArtifactRef>,
    #[serde(with = "serde_domain::context_digest")]
    bundle_digest: ContextDigest,
    byte_length: u64,
}

impl TryFrom<ContextBundleRefWire> for ContextBundleRef {
    type Error = ResourceContractError;

    fn try_from(value: ContextBundleRefWire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.context_id,
            value.work_item_id,
            value.artifact_refs,
            value.bundle_digest,
            value.byte_length,
        )
    }
}

impl From<ContextBundleRef> for ContextBundleRefWire {
    fn from(value: ContextBundleRef) -> Self {
        Self {
            schema_version: value.schema_version,
            context_id: value.context_id,
            work_item_id: value.work_item_id,
            artifact_refs: value.artifact_refs,
            bundle_digest: value.bundle_digest,
            byte_length: value.byte_length,
        }
    }
}

/// Proof produced after the daemon loaded all mandatory Coding inputs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "LoadedContextProofWire", into = "LoadedContextProofWire")]
pub struct LoadedContextProof {
    schema_version: SchemaVersion,
    work_item_id: WorkItemId,
    context_ref: ContextBundleRef,
    story_ref: ArtifactRef,
    constraints_ref: ArtifactRef,
    thinking_engine_ref: ArtifactRef,
    verification_ref: ArtifactRef,
    methodology_ref: MethodologyRef,
    state_revision: StateRevision,
    inventory_generation: InventoryGeneration,
    bundle_digest: ContextDigest,
    computed_at_unix_ms: u64,
}

impl LoadedContextProof {
    /// Constructs a proof and verifies its cross-reference bindings.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        work_item_id: WorkItemId,
        context_ref: ContextBundleRef,
        story_ref: ArtifactRef,
        constraints_ref: ArtifactRef,
        thinking_engine_ref: ArtifactRef,
        verification_ref: ArtifactRef,
        methodology_ref: MethodologyRef,
        state_revision: StateRevision,
        inventory_generation: InventoryGeneration,
        computed_at_unix_ms: u64,
    ) -> Result<Self, ResourceContractError> {
        if context_ref.work_item_id() != &work_item_id {
            return Err(ResourceContractError::WorkItemMismatch);
        }
        if !context_ref
            .artifact_refs()
            .iter()
            .any(|reference| reference == &story_ref)
        {
            return Err(ResourceContractError::StoryNotInBundle);
        }
        Ok(Self {
            schema_version,
            work_item_id,
            bundle_digest: context_ref.bundle_digest(),
            context_ref,
            story_ref,
            constraints_ref,
            thinking_engine_ref,
            verification_ref,
            methodology_ref,
            state_revision,
            inventory_generation,
            computed_at_unix_ms,
        })
    }

    /// Returns the Work Item identity proved by the load.
    pub const fn work_item_id(&self) -> &WorkItemId {
        &self.work_item_id
    }

    /// Returns the context bundle reference.
    pub const fn context_ref(&self) -> &ContextBundleRef {
        &self.context_ref
    }

    /// Returns the bundle digest bound into the proof.
    pub const fn bundle_digest(&self) -> ContextDigest {
        self.bundle_digest
    }
}

impl<'de> Deserialize<'de> for LoadedContextProof {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        LoadedContextProofWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LoadedContextProofWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::work_item_id")]
    work_item_id: WorkItemId,
    context_ref: ContextBundleRef,
    #[serde(with = "serde_domain::artifact_ref")]
    story_ref: ArtifactRef,
    #[serde(with = "serde_domain::artifact_ref")]
    constraints_ref: ArtifactRef,
    #[serde(with = "serde_domain::artifact_ref")]
    thinking_engine_ref: ArtifactRef,
    #[serde(with = "serde_domain::artifact_ref")]
    verification_ref: ArtifactRef,
    methodology_ref: MethodologyRef,
    #[serde(with = "serde_domain::state_revision")]
    state_revision: StateRevision,
    #[serde(with = "serde_domain::inventory_generation")]
    inventory_generation: InventoryGeneration,
    #[serde(with = "serde_domain::context_digest")]
    bundle_digest: ContextDigest,
    computed_at_unix_ms: u64,
}

impl TryFrom<LoadedContextProofWire> for LoadedContextProof {
    type Error = ResourceContractError;

    fn try_from(value: LoadedContextProofWire) -> Result<Self, Self::Error> {
        let proof = Self::new(
            value.schema_version,
            value.work_item_id,
            value.context_ref,
            value.story_ref,
            value.constraints_ref,
            value.thinking_engine_ref,
            value.verification_ref,
            value.methodology_ref,
            value.state_revision,
            value.inventory_generation,
            value.computed_at_unix_ms,
        )?;
        if proof.bundle_digest != value.bundle_digest {
            return Err(ResourceContractError::BundleDigestMismatch);
        }
        Ok(proof)
    }
}

impl From<LoadedContextProof> for LoadedContextProofWire {
    fn from(value: LoadedContextProof) -> Self {
        Self {
            schema_version: value.schema_version,
            work_item_id: value.work_item_id,
            context_ref: value.context_ref,
            story_ref: value.story_ref,
            constraints_ref: value.constraints_ref,
            thinking_engine_ref: value.thinking_engine_ref,
            verification_ref: value.verification_ref,
            methodology_ref: value.methodology_ref,
            state_revision: value.state_revision,
            inventory_generation: value.inventory_generation,
            bundle_digest: value.bundle_digest,
            computed_at_unix_ms: value.computed_at_unix_ms,
        }
    }
}

/// Typed document mutation represented inside a transaction plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(
    try_from = "DocumentTxnOperationWire",
    into = "DocumentTxnOperationWire"
)]
pub enum DocumentTxnOperation {
    /// Save a new content-addressed document version.
    Save {
        /// Project-relative document path.
        path: ProjectRelativePath,
        /// Digest of the content to be written by the future applier.
        content_digest: ArtifactDigest,
        /// Number of bytes in the content.
        byte_length: u64,
    },
    /// Finalize a document whose staged digest is already known.
    Finalize {
        /// Project-relative document path.
        path: ProjectRelativePath,
        /// Digest expected at finalization.
        expected_digest: ArtifactDigest,
    },
}

impl DocumentTxnOperation {
    /// Constructs a bounded save operation.
    pub fn save(
        path: ProjectRelativePath,
        content_digest: ArtifactDigest,
        byte_length: u64,
    ) -> Result<Self, ResourceContractError> {
        if byte_length > MAX_DOCUMENT_BYTES {
            return Err(ResourceContractError::DocumentByteBudgetExceeded {
                max_bytes: MAX_DOCUMENT_BYTES,
            });
        }
        Ok(Self::Save {
            path,
            content_digest,
            byte_length,
        })
    }

    /// Constructs a finalize operation.
    pub fn finalize(path: ProjectRelativePath, expected_digest: ArtifactDigest) -> Self {
        Self::Finalize {
            path,
            expected_digest,
        }
    }

    fn path(&self) -> &ProjectRelativePath {
        match self {
            Self::Save { path, .. } | Self::Finalize { path, .. } => path,
        }
    }
}

impl<'de> Deserialize<'de> for DocumentTxnOperation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        DocumentTxnOperationWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum DocumentTxnOperationWire {
    /// Wire representation of [`DocumentTxnOperation::Save`].
    Save {
        #[serde(with = "serde_domain::project_relative_path")]
        path: ProjectRelativePath,
        #[serde(with = "serde_domain::artifact_digest")]
        content_digest: ArtifactDigest,
        byte_length: u64,
    },
    /// Wire representation of [`DocumentTxnOperation::Finalize`].
    Finalize {
        #[serde(with = "serde_domain::project_relative_path")]
        path: ProjectRelativePath,
        #[serde(with = "serde_domain::artifact_digest")]
        expected_digest: ArtifactDigest,
    },
}

impl TryFrom<DocumentTxnOperationWire> for DocumentTxnOperation {
    type Error = ResourceContractError;

    fn try_from(value: DocumentTxnOperationWire) -> Result<Self, Self::Error> {
        match value {
            DocumentTxnOperationWire::Save {
                path,
                content_digest,
                byte_length,
            } => Self::save(path, content_digest, byte_length),
            DocumentTxnOperationWire::Finalize {
                path,
                expected_digest,
            } => Ok(Self::finalize(path, expected_digest)),
        }
    }
}

impl From<DocumentTxnOperation> for DocumentTxnOperationWire {
    fn from(value: DocumentTxnOperation) -> Self {
        match value {
            DocumentTxnOperation::Save {
                path,
                content_digest,
                byte_length,
            } => Self::Save {
                path,
                content_digest,
                byte_length,
            },
            DocumentTxnOperation::Finalize {
                path,
                expected_digest,
            } => Self::Finalize {
                path,
                expected_digest,
            },
        }
    }
}

/// Pure, replay-safe document transaction plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "DocumentTxnPlanWire", into = "DocumentTxnPlanWire")]
pub struct DocumentTxnPlan {
    schema_version: SchemaVersion,
    transaction_id: DocumentTxnId,
    work_item_id: WorkItemId,
    operations: Vec<DocumentTxnOperation>,
    input_fingerprint: InputFingerprint,
}

impl DocumentTxnPlan {
    /// Constructs a validated document transaction plan.
    pub fn new(
        schema_version: SchemaVersion,
        transaction_id: DocumentTxnId,
        work_item_id: WorkItemId,
        operations: Vec<DocumentTxnOperation>,
        input_fingerprint: InputFingerprint,
    ) -> Result<Self, ResourceContractError> {
        if operations.is_empty() {
            return Err(ResourceContractError::EmptyOperations);
        }
        if operations.len() > MAX_DOCUMENT_OPERATIONS {
            return Err(ResourceContractError::CollectionLimitExceeded {
                field: "operations",
                max_items: MAX_DOCUMENT_OPERATIONS,
            });
        }
        let mut paths = BTreeSet::new();
        if operations
            .iter()
            .any(|operation| !paths.insert(operation.path().to_string()))
        {
            return Err(ResourceContractError::DuplicateOperationPath);
        }
        Ok(Self {
            schema_version,
            transaction_id,
            work_item_id,
            operations,
            input_fingerprint,
        })
    }

    /// Constructs a deterministic save-only plan with a caller-supplied identity.
    pub fn save_only(
        schema_version: SchemaVersion,
        transaction_id: DocumentTxnId,
        work_item_id: WorkItemId,
        path: ProjectRelativePath,
        digest: ArtifactDigest,
        byte_length: u64,
        input_fingerprint: InputFingerprint,
    ) -> Result<Self, ResourceContractError> {
        Self::new(
            schema_version,
            transaction_id,
            work_item_id,
            vec![DocumentTxnOperation::save(path, digest, byte_length)?],
            input_fingerprint,
        )
    }

    /// Returns the immutable operation list.
    pub fn operations(&self) -> &[DocumentTxnOperation] {
        &self.operations
    }
}

impl<'de> Deserialize<'de> for DocumentTxnPlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        DocumentTxnPlanWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DocumentTxnPlanWire {
    schema_version: SchemaVersion,
    transaction_id: DocumentTxnId,
    #[serde(with = "serde_domain::work_item_id")]
    work_item_id: WorkItemId,
    operations: Vec<DocumentTxnOperation>,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
}

impl TryFrom<DocumentTxnPlanWire> for DocumentTxnPlan {
    type Error = ResourceContractError;

    fn try_from(value: DocumentTxnPlanWire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.transaction_id,
            value.work_item_id,
            value.operations,
            value.input_fingerprint,
        )
    }
}

impl From<DocumentTxnPlan> for DocumentTxnPlanWire {
    fn from(value: DocumentTxnPlan) -> Self {
        Self {
            schema_version: value.schema_version,
            transaction_id: value.transaction_id,
            work_item_id: value.work_item_id,
            operations: value.operations,
            input_fingerprint: value.input_fingerprint,
        }
    }
}
