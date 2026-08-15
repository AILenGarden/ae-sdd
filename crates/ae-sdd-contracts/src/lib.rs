#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Frozen cross-Part contract vocabulary.
//!
//! This crate owns only published, transport-safe values shared by more than
//! one control-plane Part. Internal aggregate state remains in its owning
//! crate so this boundary does not become an unstructured `common` module.

pub mod compact;
pub mod diagnostics;
pub mod document;
pub mod engineering_route;
pub mod error;
pub mod evidence;
pub mod execution;
pub mod execution_runtime;
pub mod host;
pub mod instruction;
pub mod intake;
pub mod lifecycle;
pub mod methodology;
pub mod ports;
pub mod provenance;
pub mod resource;
pub mod review;
pub mod run_graph;
mod serde_domain;
pub mod series;
pub mod session;
pub mod supervision;
mod value;

pub use diagnostics::{
    BugKind, BugRecord, BugRepeatRecord, DIAGNOSTICS_DIR, DiagnosticRecord, DiagnosticTrack,
    DroppedRecord, HookInRecord, HookOutRecord, NodeRecord,
};
pub use document::{DocumentVersionError, DocumentVersionId, SpecKind};
pub use engineering_route::{
    EngineeringRoute, EngineeringRouteError, ReceiptStatus, RequirementAnalysisEvidence,
    RouteApprovalReceipt, RouteBindingInput, RouteMappingVersion,
};
pub use error::{
    ContractValidationError, ControlPlaneError, ControlPlaneErrorCode, Remediation, RetryClass,
};
pub use evidence::{
    EvidenceLedgerError, EvidenceLedgerEventKind, EvidenceLedgerEventV1, MAX_LEDGER_ARTIFACT_REFS,
    MAX_LEDGER_EVENTS,
};
pub use instruction::{
    ContextProjectionRef, InstructionEnvelope, InstructionError, InstructionIdentity,
    InstructionTransaction, SkillRef,
};
pub use intake::{
    AssessmentFact, BootstrapAssessment, BootstrapAssessmentError, ConflictDimension, InputSource,
    MAX_ASSESSMENT_FACTS, MAX_ASSESSMENT_QUESTIONS, MAX_ASSESSMENT_UNCERTAINTIES,
    MAX_CONFLICT_SOURCES, RequirementConflict, RequirementConflictError, RequirementSourceRef,
    TaskKind,
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
pub use provenance::FingerprintInputs;
pub use run_graph::{FlowRunProjection, RunGraphError, SeriesRunProjection};
pub use series::{
    ImpactFact, ImpactLevel, MAIN_NODE_SERIES_KINDS, MAX_IMPACT_FACTS, MAX_REQUIRED_SERIES,
    MAX_ROUTE_ARTIFACTS, MAX_ROUTE_REASON_CODES, MAX_SERIES_GRANT_ITEMS, ProcessSnapshot,
    RequirementAnalysisSeriesInput, RetryPolicy, RouteDecision, RouteDecisionError,
    RouteDisposition, RouteInput, RouteInputError, SERIES_ACTIVITIES, SERIES_SUB_NODES,
    SeriesActivity, SeriesInput, SeriesInputError, SeriesLifecycleState, SeriesPlan,
    SeriesPlanDecision, SeriesPlanError, SeriesReceipt, SeriesReceiptError, SeriesReceiptStatus,
    SeriesSubNode,
};
pub use supervision::{RequirementRulingEvent, SeriesProgressEvent, SupervisionEventError};
pub use value::{
    AdapterId, BoundedText, ContextBundleId, ContractValueError, DocumentId, DocumentTxnId,
    ExecutionId, ExecutionStepId, ExternalSessionKey, HostTaskId, IdempotencyKey, LogicalKey,
    LogicalNamespace, MessageKey, MethodologyVariant, MutationIntentId, OperationName, PrdId,
    ReasonCode, ReviewId, ReviewerRole, RouteDecisionId, RuntimeModuleKey, RuntimeModuleName,
    SchemaVersion, SeriesId, SeriesKind, SkillId, SpecGraphId, VerificationContractId, WorkerId,
};
