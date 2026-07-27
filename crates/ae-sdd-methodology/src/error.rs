use thiserror::Error;

/// Fail-closed errors emitted by the pure Methodology compiler and Catalog.
#[derive(Debug, Error)]
pub enum MethodologyError {
    /// JSON input could not be decoded under the strict v1 schema.
    #[error("Methodology JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    /// The Catalog or bundle schema is not supported.
    #[error("unsupported Methodology schema {0}")]
    UnsupportedSchema(String),
    /// The Catalog source exceeded its fixed byte budget.
    #[error("Methodology Catalog source exceeds its byte budget")]
    SourceTooLarge,
    /// No Methodology entries were supplied.
    #[error("Methodology Catalog must contain at least one entry")]
    EmptyCatalog,
    /// A bounded Catalog collection exceeded its v1 limit.
    #[error("Methodology collection {field} exceeds its {limit}-item limit")]
    CollectionLimit {
        /// Collection field.
        field: &'static str,
        /// Frozen item limit.
        limit: usize,
    },
    /// A portable identifier or semantic version was malformed.
    #[error("invalid Methodology field {field}: {value}")]
    InvalidField {
        /// Field name.
        field: &'static str,
        /// Redacted invalid value.
        value: String,
    },
    /// A set-like list contained a duplicate value.
    #[error("Methodology field {field} contains duplicate value {value}")]
    DuplicateListValue {
        /// Field name.
        field: &'static str,
        /// Duplicate value.
        value: String,
    },
    /// Two entries used the same skill and variant identity.
    #[error("duplicate Methodology skill/variant identity: {skill_id}/{variant}")]
    DuplicateEntry {
        /// Skill identity.
        skill_id: String,
        /// Variant identity.
        variant: String,
    },
    /// Activation and spawn policy violated the v1 matrix.
    #[error("invalid activation/spawnPolicy combination")]
    InvalidActivationPolicy,
    /// A workflow entry omitted routing or deliverable metadata.
    #[error("workflow Methodology requires route predicates and deliverable kinds")]
    IncompleteWorkflow,
    /// A project-relative artifact path was malformed or escaped containment.
    #[error("invalid Methodology artifact path {0}")]
    InvalidPath(String),
    /// Compact and fallback paths were identical.
    #[error("compact and fallback references must be different")]
    DuplicateArtifactReference,
    /// A required compact artifact was absent.
    #[error("required compact Methodology artifact is missing: {0}")]
    CompactMissing(String),
    /// A declared fallback artifact was absent.
    #[error("declared fallback Methodology artifact is missing: {0}")]
    FallbackMissing(String),
    /// An artifact exceeded its type-specific byte budget or was empty.
    #[error("Methodology artifact {path} has invalid byte length {actual}")]
    InvalidArtifactSize {
        /// Project-relative artifact path.
        path: String,
        /// Observed length.
        actual: usize,
    },
    /// Content no longer matched its compiled digest or size.
    #[error("Methodology artifact digest or size mismatch: {0}")]
    ArtifactTampered(String),
    /// A compiled entry digest did not match its canonical metadata.
    #[error("Methodology entry digest mismatch: {0}")]
    EntryDigestMismatch(String),
    /// The bundle Catalog digest did not match its canonical entries.
    #[error("Methodology bundle Catalog digest mismatch")]
    CatalogDigestMismatch,
    /// Compiled entries were not in canonical identity order.
    #[error("Methodology bundle entries are not in canonical order")]
    NonCanonicalEntryOrder,
    /// One compiled set-like field was not in strict canonical order.
    #[error("Methodology compiled field is not canonical: {0}")]
    NonCanonicalMetadata(&'static str),
    /// The production built-in inventory was incomplete or misclassified.
    #[error("Methodology built-in coverage mismatch: {0}")]
    CoverageMismatch(&'static str),
    /// Built-in is a terminal fallback and cannot be registered as an override.
    #[error("built-in Methodology entries cannot be registered as overrides")]
    InvalidOverrideLayer,
    /// The override scope did not match its registry layer.
    #[error("Methodology override scope does not match its layer")]
    InvalidOverrideScope,
    /// An override targeted a skill absent from the built-in Catalog.
    #[error("Methodology override target is absent from the built-in Catalog: {0}")]
    OverrideTargetMissing(String),
    /// An override changed the frozen Series kind of its target.
    #[error("Methodology override Series kind differs from target {0}")]
    OverrideSeriesMismatch(String),
    /// The complete contender trace would exceed the frozen contract budget.
    #[error("Methodology override contenders exceed the frozen trace budget")]
    OverrideTraceLimit,
}
