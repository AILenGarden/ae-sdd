//! Frozen resource, context-proof, and document-transaction contracts.
//!
//! These values are intentionally transport-only.  They carry content-addressed
//! references and bounded plans, but never open files or mutate a workspace.

use std::collections::BTreeSet;

use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, ContextDigest, InputFingerprint,
    InventoryGeneration, ProjectRelativePath, StateRevision, WorkItemId,
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
    /// A caller-provided bundle digest did not match canonical content.
    #[error("context bundle digest does not match canonical content")]
    ContextBundleDigestMismatch,
    /// A caller-provided byte length did not match the referenced artifacts.
    #[error("context bundle byte length mismatch: claimed {claimed}, actual {actual}")]
    ContextBundleByteLengthMismatch {
        /// Caller-provided length.
        claimed: u64,
        /// Canonically computed length.
        actual: u64,
    },
    /// A nested proof belonged to a different Work Item.
    #[error("resource contract Work Item does not match its nested reference")]
    WorkItemMismatch,
    /// The proof repeated a digest that did not match its context reference.
    #[error("loaded context proof bundle digest does not match its context reference")]
    BundleDigestMismatch,
    /// The proof did not bind the Story artifact into the context bundle.
    #[error("loaded context proof Story artifact is not present in the bundle")]
    StoryNotInBundle,
    /// The proof did not bind another mandatory Coding artifact into the bundle.
    #[error("loaded context proof required artifact {field} is not present in the bundle")]
    RequiredArtifactNotInBundle {
        /// Stable mandatory-artifact field name.
        field: &'static str,
    },
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
    /// A document save operation did not reference any content bytes.
    #[error("document save operation must reference non-empty staged content")]
    EmptyDocumentContent,
    /// A wire-level optional digest was not canonical lower-case SHA-256 hex.
    #[error("document expected-before digest is not canonical")]
    InvalidExpectedBeforeDigest,
    /// A document plan's encoded digest did not match its canonical content.
    #[error("document transaction plan digest does not match canonical content")]
    DocumentPlanDigestMismatch,
    /// A compatibility save operation could not construct its fixed artifact kind.
    #[error("document save operation artifact kind is invalid")]
    InvalidDocumentArtifactKind,
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

    /// Constructs a context bundle by canonicalizing references and computing
    /// its byte length and digest inside the contract owner.
    pub fn from_artifacts(
        schema_version: SchemaVersion,
        context_id: ContextBundleId,
        work_item_id: WorkItemId,
        mut artifact_refs: Vec<ArtifactRef>,
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
        artifact_refs.sort_by(|left, right| left.path().as_str().cmp(right.path().as_str()));
        let mut paths = BTreeSet::new();
        if artifact_refs
            .iter()
            .any(|reference| !paths.insert(reference.path().to_string()))
        {
            return Err(ResourceContractError::DuplicateArtifactPath);
        }
        let byte_length = artifact_refs.iter().try_fold(0_u64, |total, reference| {
            total.checked_add(reference.byte_length())
        });
        let Some(byte_length) = byte_length else {
            return Err(ResourceContractError::ContextByteBudgetExceeded {
                max_bytes: MAX_CONTEXT_BYTES,
            });
        };
        if byte_length == 0 || byte_length > MAX_CONTEXT_BYTES {
            return Err(ResourceContractError::ContextByteBudgetExceeded {
                max_bytes: MAX_CONTEXT_BYTES,
            });
        }
        let bundle_digest = canonical_context_digest(
            schema_version,
            &context_id,
            &work_item_id,
            &artifact_refs,
            byte_length,
        );
        Ok(Self {
            schema_version,
            context_id,
            work_item_id,
            artifact_refs,
            bundle_digest,
            byte_length,
        })
    }

    /// Constructs and validates a context bundle reference.
    pub fn new(
        schema_version: SchemaVersion,
        context_id: ContextBundleId,
        work_item_id: WorkItemId,
        artifact_refs: Vec<ArtifactRef>,
        _bundle_digest: ContextDigest,
        _byte_length: u64,
    ) -> Result<Self, ResourceContractError> {
        Self::from_artifacts(schema_version, context_id, work_item_id, artifact_refs)
    }

    /// Returns the wire schema version.
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
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

fn canonical_context_digest(
    schema_version: SchemaVersion,
    context_id: &ContextBundleId,
    work_item_id: &WorkItemId,
    artifact_refs: &[ArtifactRef],
    byte_length: u64,
) -> ContextDigest {
    fn push_field(target: &mut Vec<u8>, value: &[u8]) {
        target.extend_from_slice(&(value.len() as u64).to_be_bytes());
        target.extend_from_slice(value);
    }

    let mut canonical = Vec::new();
    push_field(&mut canonical, b"ae-sdd/context-bundle/v1");
    let schema_tag = match schema_version {
        SchemaVersion::V1 => b"v1".as_slice(),
    };
    push_field(&mut canonical, schema_tag);
    push_field(&mut canonical, context_id.as_str().as_bytes());
    push_field(&mut canonical, work_item_id.as_str().as_bytes());
    canonical.extend_from_slice(&(artifact_refs.len() as u64).to_be_bytes());
    for reference in artifact_refs {
        push_field(&mut canonical, reference.kind().as_str().as_bytes());
        push_field(&mut canonical, reference.path().as_str().as_bytes());
        canonical.extend_from_slice(reference.digest().as_bytes());
        canonical.extend_from_slice(&reference.byte_length().to_be_bytes());
    }
    canonical.extend_from_slice(&byte_length.to_be_bytes());
    ContextDigest::digest(canonical)
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
        let canonical = Self::from_artifacts(
            value.schema_version,
            value.context_id,
            value.work_item_id,
            value.artifact_refs,
        )?;
        if canonical.byte_length != value.byte_length {
            return Err(ResourceContractError::ContextBundleByteLengthMismatch {
                claimed: value.byte_length,
                actual: canonical.byte_length,
            });
        }
        if canonical.bundle_digest != value.bundle_digest {
            return Err(ResourceContractError::ContextBundleDigestMismatch);
        }
        Ok(canonical)
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
        for (field, required) in [
            ("constraintsRef", &constraints_ref),
            ("thinkingEngineRef", &thinking_engine_ref),
            ("verificationRef", &verification_ref),
        ] {
            if !context_ref
                .artifact_refs()
                .iter()
                .any(|reference| reference == required)
            {
                return Err(ResourceContractError::RequiredArtifactNotInBundle { field });
            }
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

    /// Returns the wire schema version.
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the context bundle reference.
    pub const fn context_ref(&self) -> &ContextBundleRef {
        &self.context_ref
    }

    /// Returns the bundle digest bound into the proof.
    pub const fn bundle_digest(&self) -> ContextDigest {
        self.bundle_digest
    }

    /// Returns the exact Story artifact bound into the proof.
    pub const fn story_ref(&self) -> &ArtifactRef {
        &self.story_ref
    }

    /// Returns the exact project-constraints artifact bound into the proof.
    pub const fn constraints_ref(&self) -> &ArtifactRef {
        &self.constraints_ref
    }

    /// Returns the exact Thinking Engine artifact bound into the proof.
    pub const fn thinking_engine_ref(&self) -> &ArtifactRef {
        &self.thinking_engine_ref
    }

    /// Returns the exact verification-contract artifact bound into the proof.
    pub const fn verification_ref(&self) -> &ArtifactRef {
        &self.verification_ref
    }

    /// Returns the methodology selected for the Coding flow.
    pub const fn methodology_ref(&self) -> &MethodologyRef {
        &self.methodology_ref
    }

    /// Returns the project-state revision observed during computation.
    pub const fn state_revision(&self) -> StateRevision {
        self.state_revision
    }

    /// Returns the inventory generation observed during computation.
    pub const fn inventory_generation(&self) -> InventoryGeneration {
        self.inventory_generation
    }

    /// Returns the explicit computation timestamp supplied by the caller.
    pub const fn computed_at_unix_ms(&self) -> u64 {
        self.computed_at_unix_ms
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
        /// Content-addressed staged input consumed by the future applier.
        staged_content_ref: ArtifactRef,
        /// Optional digest that must be observed before applying the save.
        expected_before_digest: Option<ArtifactDigest>,
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
        let kind = ArtifactKind::new("document")
            .map_err(|_| ResourceContractError::InvalidDocumentArtifactKind)?;
        let staged_content_ref = ArtifactRef::new(kind, path.clone(), content_digest, byte_length);
        Self::save_staged(path, staged_content_ref, None)
    }

    /// Constructs a bounded save operation over a content-addressed staged ref
    /// and an optional compare-and-swap expectation.
    pub fn save_staged(
        path: ProjectRelativePath,
        staged_content_ref: ArtifactRef,
        expected_before_digest: Option<ArtifactDigest>,
    ) -> Result<Self, ResourceContractError> {
        if staged_content_ref.byte_length() == 0 {
            return Err(ResourceContractError::EmptyDocumentContent);
        }
        if staged_content_ref.byte_length() > MAX_DOCUMENT_BYTES {
            return Err(ResourceContractError::DocumentByteBudgetExceeded {
                max_bytes: MAX_DOCUMENT_BYTES,
            });
        }
        Ok(Self::Save {
            path,
            staged_content_ref,
            expected_before_digest,
        })
    }

    /// Constructs a finalize operation.
    pub fn finalize(path: ProjectRelativePath, expected_digest: ArtifactDigest) -> Self {
        Self::Finalize {
            path,
            expected_digest,
        }
    }

    /// Returns the project-relative transaction target.
    pub const fn path(&self) -> &ProjectRelativePath {
        match self {
            Self::Save { path, .. } | Self::Finalize { path, .. } => path,
        }
    }

    /// Returns staged content for a save operation.
    pub const fn staged_content_ref(&self) -> Option<&ArtifactRef> {
        match self {
            Self::Save {
                staged_content_ref, ..
            } => Some(staged_content_ref),
            Self::Finalize { .. } => None,
        }
    }

    /// Returns the compare-and-swap digest expected before the operation.
    pub const fn expected_before_digest(&self) -> Option<ArtifactDigest> {
        match self {
            Self::Save {
                expected_before_digest,
                ..
            } => *expected_before_digest,
            Self::Finalize {
                expected_digest, ..
            } => Some(*expected_digest),
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
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum DocumentTxnOperationWire {
    /// Wire representation of [`DocumentTxnOperation::Save`].
    Save {
        #[serde(with = "serde_domain::project_relative_path")]
        path: ProjectRelativePath,
        #[serde(with = "serde_domain::artifact_ref")]
        staged_content_ref: ArtifactRef,
        expected_before_digest: Option<String>,
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
                staged_content_ref,
                expected_before_digest,
            } => {
                let expected_before_digest = expected_before_digest
                    .map(|value| value.parse())
                    .transpose()
                    .map_err(|_| ResourceContractError::InvalidExpectedBeforeDigest)?;
                Self::save_staged(path, staged_content_ref, expected_before_digest)
            }
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
                staged_content_ref,
                expected_before_digest,
            } => Self::Save {
                path,
                staged_content_ref,
                expected_before_digest: expected_before_digest.map(|value| value.to_string()),
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
    plan_digest: ArtifactDigest,
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
        let plan_digest = canonical_document_plan_digest(
            schema_version,
            &transaction_id,
            &work_item_id,
            &operations,
            input_fingerprint,
        );
        Ok(Self {
            schema_version,
            transaction_id,
            work_item_id,
            operations,
            input_fingerprint,
            plan_digest,
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

    /// Returns the wire schema version.
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the transaction identity.
    pub const fn transaction_id(&self) -> &DocumentTxnId {
        &self.transaction_id
    }

    /// Returns the bound Work Item identity.
    pub const fn work_item_id(&self) -> &WorkItemId {
        &self.work_item_id
    }

    /// Returns the fingerprint of the inputs used to build this plan.
    pub const fn input_fingerprint(&self) -> InputFingerprint {
        self.input_fingerprint
    }

    /// Returns the canonical digest of the complete transaction plan.
    pub const fn plan_digest(&self) -> ArtifactDigest {
        self.plan_digest
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
    #[serde(with = "serde_domain::artifact_digest")]
    plan_digest: ArtifactDigest,
}

impl TryFrom<DocumentTxnPlanWire> for DocumentTxnPlan {
    type Error = ResourceContractError;

    fn try_from(value: DocumentTxnPlanWire) -> Result<Self, Self::Error> {
        let plan = Self::new(
            value.schema_version,
            value.transaction_id,
            value.work_item_id,
            value.operations,
            value.input_fingerprint,
        )?;
        if plan.plan_digest != value.plan_digest {
            return Err(ResourceContractError::DocumentPlanDigestMismatch);
        }
        Ok(plan)
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
            plan_digest: value.plan_digest,
        }
    }
}

fn canonical_document_plan_digest(
    schema_version: SchemaVersion,
    transaction_id: &DocumentTxnId,
    work_item_id: &WorkItemId,
    operations: &[DocumentTxnOperation],
    input_fingerprint: InputFingerprint,
) -> ArtifactDigest {
    fn push_field(target: &mut Vec<u8>, value: &[u8]) {
        target.extend_from_slice(&(value.len() as u64).to_be_bytes());
        target.extend_from_slice(value);
    }

    let mut canonical = Vec::new();
    push_field(&mut canonical, b"ae-sdd/document-transaction-plan/v1");
    let schema_tag = match schema_version {
        SchemaVersion::V1 => b"v1".as_slice(),
    };
    push_field(&mut canonical, schema_tag);
    push_field(&mut canonical, transaction_id.as_str().as_bytes());
    push_field(&mut canonical, work_item_id.as_str().as_bytes());
    canonical.extend_from_slice(input_fingerprint.as_bytes());
    canonical.extend_from_slice(&(operations.len() as u64).to_be_bytes());
    for operation in operations {
        match operation {
            DocumentTxnOperation::Save {
                path,
                staged_content_ref,
                expected_before_digest,
            } => {
                push_field(&mut canonical, b"save");
                push_field(&mut canonical, path.as_str().as_bytes());
                push_field(
                    &mut canonical,
                    staged_content_ref.kind().as_str().as_bytes(),
                );
                push_field(
                    &mut canonical,
                    staged_content_ref.path().as_str().as_bytes(),
                );
                canonical.extend_from_slice(staged_content_ref.digest().as_bytes());
                canonical.extend_from_slice(&staged_content_ref.byte_length().to_be_bytes());
                match expected_before_digest {
                    Some(digest) => {
                        canonical.push(1);
                        canonical.extend_from_slice(digest.as_bytes());
                    }
                    None => canonical.push(0),
                }
            }
            DocumentTxnOperation::Finalize {
                path,
                expected_digest,
            } => {
                push_field(&mut canonical, b"finalize");
                push_field(&mut canonical, path.as_str().as_bytes());
                canonical.extend_from_slice(expected_digest.as_bytes());
            }
        }
    }
    ArtifactDigest::digest(canonical)
}
