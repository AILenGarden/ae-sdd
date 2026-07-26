#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Frozen cross-Part contract vocabulary.
//!
//! This crate owns only published, transport-safe values shared by more than
//! one control-plane Part. Internal aggregate state remains in its owning
//! crate so this boundary does not become an unstructured `common` module.

pub mod compact;
pub mod error;
pub mod evidence;
pub mod execution;
pub mod execution_runtime;
pub mod host;
pub mod lifecycle;
pub mod methodology;
pub mod ports;
pub mod resource;
pub mod review;
mod serde_domain;
pub mod series;
pub mod session;
mod value;

pub use error::{
    ContractValidationError, ControlPlaneError, ControlPlaneErrorCode, Remediation, RetryClass,
};
pub use evidence::{
    EvidenceLedgerError, EvidenceLedgerEventKind, EvidenceLedgerEventV1, MAX_LEDGER_ARTIFACT_REFS,
    MAX_LEDGER_EVENTS,
};
pub use lifecycle::{
    ConfirmationRequirement, EventIntent, FileLockSnapshot, LifecycleCommand, LifecycleDisposition,
    LifecycleInput, LifecycleInputError, LifecyclePlan, LifecyclePlanError, MAX_FILE_LOCKS,
    MAX_LIFECYCLE_REFS, MAX_LIFECYCLE_STORIES, MAX_MUTATION_INTENTS, MutationIntent,
    MutationOperation, MutationTarget, PrdSummary, StorySummary,
};
pub use methodology::{
    MAX_OVERRIDE_TRACE, MethodologyCatalogPort, MethodologyQuery, MethodologyRef,
    MethodologyRefError, MethodologyResolution, MethodologyResolutionError, OverrideDisposition,
    OverrideLayer, OverrideTrace, ProjectScope,
};
pub use series::{
    ImpactFact, ImpactLevel, MAX_IMPACT_FACTS, MAX_REQUIRED_SERIES, MAX_ROUTE_ARTIFACTS,
    MAX_ROUTE_REASON_CODES, MAX_SERIES_GRANT_ITEMS, ProcessSnapshot, RetryPolicy, RouteDecision,
    RouteDecisionError, RouteDisposition, RouteInput, RouteInputError, SeriesInput,
    SeriesInputError, SeriesPlan, SeriesPlanDecision, SeriesPlanError, SeriesReceipt,
    SeriesReceiptError, SeriesReceiptStatus,
};
pub use value::{
    AdapterId, BoundedText, ContextBundleId, ContractValueError, DocumentTxnId, ExecutionId,
    ExecutionStepId, ExternalSessionKey, HostTaskId, IdempotencyKey, LogicalKey, LogicalNamespace,
    MessageKey, MethodologyVariant, MutationIntentId, OperationName, PrdId, ReasonCode, ReviewId,
    ReviewerRole, RouteDecisionId, RuntimeModuleKey, RuntimeModuleName, SchemaVersion, SeriesId,
    SeriesKind, SkillId, VerificationContractId, WorkerId,
};
