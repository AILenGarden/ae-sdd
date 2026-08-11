use std::{fmt, str::FromStr};

use ae_sdd_protocol::OperationScope;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const OPERATION_COUNT: usize = 25;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum OperationName {
    DocumentResolve,
    DocumentSave,
    EvidenceFinalize,
    EvidenceRecord,
    ExecutionPlanApprove,
    ExecutionPlanSet,
    ExecutionResume,
    ExecutionSliceRecord,
    ExecutionSliceStart,
    GateCheck,
    LeaseAcquire,
    LeaseBreak,
    LeaseRelease,
    LeaseRenew,
    LeaseStatus,
    ReviewContribute,
    ReviewFinalize,
    ReviewRecord,
    RouteDecide,
    StateNextActions,
    StateTransition,
    VerificationPlan,
    WorkItemComplete,
    WorkItemCreate,
    WorkItemGet,
}

impl OperationName {
    pub const ALL: [Self; OPERATION_COUNT] = [
        Self::DocumentResolve,
        Self::DocumentSave,
        Self::EvidenceFinalize,
        Self::EvidenceRecord,
        Self::ExecutionPlanApprove,
        Self::ExecutionPlanSet,
        Self::ExecutionResume,
        Self::ExecutionSliceRecord,
        Self::ExecutionSliceStart,
        Self::GateCheck,
        Self::LeaseAcquire,
        Self::LeaseBreak,
        Self::LeaseRelease,
        Self::LeaseRenew,
        Self::LeaseStatus,
        Self::ReviewContribute,
        Self::ReviewFinalize,
        Self::ReviewRecord,
        Self::RouteDecide,
        Self::StateNextActions,
        Self::StateTransition,
        Self::VerificationPlan,
        Self::WorkItemComplete,
        Self::WorkItemCreate,
        Self::WorkItemGet,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DocumentResolve => "document.resolve",
            Self::DocumentSave => "document.save",
            Self::EvidenceFinalize => "evidence.finalize",
            Self::EvidenceRecord => "evidence.record",
            Self::ExecutionPlanApprove => "execution.plan.approve",
            Self::ExecutionPlanSet => "execution.plan.set",
            Self::ExecutionResume => "execution.resume",
            Self::ExecutionSliceRecord => "execution.slice.record",
            Self::ExecutionSliceStart => "execution.slice.start",
            Self::GateCheck => "gate.check",
            Self::LeaseAcquire => "lease.acquire",
            Self::LeaseBreak => "lease.break",
            Self::LeaseRelease => "lease.release",
            Self::LeaseRenew => "lease.renew",
            Self::LeaseStatus => "lease.status",
            Self::ReviewContribute => "review.contribute",
            Self::ReviewFinalize => "review.finalize",
            Self::ReviewRecord => "review.record",
            Self::RouteDecide => "route.decide",
            Self::StateNextActions => "state.next_actions",
            Self::StateTransition => "state.transition",
            Self::VerificationPlan => "verification.plan",
            Self::WorkItemComplete => "workitem.complete",
            Self::WorkItemCreate => "workitem.create",
            Self::WorkItemGet => "workitem.get",
        }
    }

    #[must_use]
    pub const fn spec(self) -> &'static OperationSpec {
        &OPERATION_REGISTRY[self as usize]
    }
}

impl fmt::Display for OperationName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for OperationName {
    type Err = UnknownOperation;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|operation| operation.as_str() == value)
            .ok_or_else(|| UnknownOperation(value.to_owned()))
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("operation is not registered: {0}")]
pub struct UnknownOperation(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    String,
    Object,
    Array,
    Boolean,
    Integer,
    StringOrArray,
    StringOrObject,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldSpec {
    pub name: &'static str,
    pub kind: FieldKind,
    pub required: bool,
}

const fn field(name: &'static str, kind: FieldKind, required: bool) -> FieldSpec {
    FieldSpec {
        name,
        kind,
        required,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationSpec {
    pub operation: OperationName,
    pub scope: OperationScope,
    pub requires_workspace: bool,
    pub requires_work_item: bool,
    pub writes: bool,
    pub requires_lease: bool,
    pub requires_revision: bool,
    pub requires_idempotency: bool,
    pub requires_confirmation: bool,
    pub fields: &'static [FieldSpec],
}

const fn spec(
    operation: OperationName,
    writes: bool,
    requires_lease: bool,
    requires_revision: bool,
    requires_idempotency: bool,
    requires_confirmation: bool,
    fields: &'static [FieldSpec],
) -> OperationSpec {
    OperationSpec {
        operation,
        scope: OperationScope::WorkItem,
        requires_workspace: true,
        requires_work_item: true,
        writes,
        requires_lease,
        requires_revision,
        requires_idempotency,
        requires_confirmation,
        fields,
    }
}

/// Spec for an operation that runs before its Work Item exists.
///
/// Every other operation in this registry acts on an already-resolvable Work
/// Item, so `spec()` hardcodes `requires_work_item: true`. Creation cannot: the
/// Work Item is its output, not its input. Lease and revision are likewise
/// Work-Item-level preconditions with nothing to attach to yet, so the guard
/// that remains is idempotency, which makes a retried create return the first
/// result instead of a second directory.
const fn workspace_spec(
    operation: OperationName,
    writes: bool,
    requires_idempotency: bool,
    fields: &'static [FieldSpec],
) -> OperationSpec {
    OperationSpec {
        operation,
        scope: OperationScope::Workspace,
        requires_workspace: true,
        requires_work_item: false,
        writes,
        requires_lease: false,
        requires_revision: false,
        requires_idempotency,
        requires_confirmation: false,
        fields,
    }
}

/// The Work Item's business name MAY arrive as the request-level `workItemId`;
/// a bootstrap caller has no Work Item yet and omits it, so the daemon mints
/// `{entryNode}-{8 lowercase hex}` instead. That is why `workitem.create` is
/// workspace-scoped and the payload carries only what shapes the new state.
///
/// `providedDocuments` registers caller-owned PRD/DR/Story documents at create
/// time. Each entry is an object: `intent` (`PRD`|`DR`|`STORY`), `docId`,
/// project-relative `path`, and an optional `parentDocId` pointing at another
/// entry (a Story to its DR, a DR to its PRD). Adoption only records the
/// mapping; the daemon never writes to or copies the referenced files.
const WORKITEM_CREATE: &[FieldSpec] = &[
    field("entryNode", FieldKind::String, true),
    field("requestedIntent", FieldKind::String, false),
    field("storyName", FieldKind::String, false),
    field("providedDocuments", FieldKind::Array, false),
];
/// `taskKind` is required because §5.5 makes it one of the six facts the route
/// decision freezes. It arrives as an input rather than being inferred: §5.3 keeps
/// `BootstrapAssessment.task_kind_proposal` provisional until RA closes, so the
/// caller reports what the assessment proposed and the decision promotes it. A
/// route engine that invented the value would be fabricating the authoritative
/// fact instead of freezing a reported one.
const ROUTE_DECIDE: &[FieldSpec] = &[
    field("requestedIntent", FieldKind::String, false),
    field("taskKind", FieldKind::String, true),
    field("availableArtifacts", FieldKind::Array, false),
    field("impactFacts", FieldKind::Array, false),
    field("classificationConfidenceBps", FieldKind::Integer, false),
];
const DOCUMENT_RESOLVE: &[FieldSpec] = &[
    field("intent", FieldKind::String, true),
    field("docId", FieldKind::String, false),
    field("version", FieldKind::StringOrObject, false),
];
const DOCUMENT_SAVE: &[FieldSpec] = &[
    field("intent", FieldKind::String, true),
    field("contentFile", FieldKind::String, true),
    field("docId", FieldKind::String, false),
    field("version", FieldKind::StringOrObject, false),
    field("changelogNote", FieldKind::String, false),
];
const EVIDENCE_RECORD: &[FieldSpec] = &[
    field("artifactPath", FieldKind::String, true),
    field("inputFingerprint", FieldKind::String, true),
    field("kind", FieldKind::String, false),
    field("command", FieldKind::StringOrArray, false),
    field("toolchainFingerprint", FieldKind::String, false),
    field("exitCode", FieldKind::Integer, false),
    field("summary", FieldKind::Object, false),
    field("durationMs", FieldKind::Integer, false),
    field("logicalKey", FieldKind::String, false),
];
const EXECUTION_PLAN_APPROVE: &[FieldSpec] = &[field("approvedBy", FieldKind::String, false)];
const EXECUTION_PLAN_SET: &[FieldSpec] = &[
    field("goal", FieldKind::String, true),
    field("changedPaths", FieldKind::Array, true),
    field("verification", FieldKind::Array, true),
    field("risks", FieldKind::Array, false),
    field("sourceReads", FieldKind::Array, false),
];
const EXECUTION_RESUME: &[FieldSpec] = &[
    field("knownCapsuleDigest", FieldKind::String, false),
    field("knownContextRevision", FieldKind::Integer, false),
];
const EXECUTION_SLICE_START: &[FieldSpec] = &[
    field("activeOrdinal", FieldKind::Integer, true),
    field("queueDigest", FieldKind::String, true),
];
const EXECUTION_SLICE_RECORD: &[FieldSpec] = &[
    field("sliceId", FieldKind::String, true),
    field("status", FieldKind::String, true),
    field("progressDigest", FieldKind::String, false),
];
const GATE_CHECK: &[FieldSpec] = &[field("gateIds", FieldKind::Array, false)];
const LEASE_ACQUIRE: &[FieldSpec] = &[
    field("owner", FieldKind::Object, true),
    field("ttlSeconds", FieldKind::Integer, true),
];
const LEASE_BREAK: &[FieldSpec] = &[
    field("actor", FieldKind::Object, true),
    field("reason", FieldKind::String, true),
];
const LEASE_OWNER: &[FieldSpec] = &[field("owner", FieldKind::Object, true)];
const LEASE_RENEW: &[FieldSpec] = &[
    field("owner", FieldKind::Object, true),
    field("ttlSeconds", FieldKind::Integer, true),
];
const REVIEW_RECORD: &[FieldSpec] = &[
    field("status", FieldKind::String, true),
    field("findings", FieldKind::Array, true),
    field("reviewedPaths", FieldKind::Array, false),
    field("evidenceIds", FieldKind::Array, false),
];
const REVIEW_CONTRIBUTE: &[FieldSpec] = REVIEW_RECORD;
const STATE_TRANSITION: &[FieldSpec] = &[field("targetPhase", FieldKind::String, true)];
const VERIFICATION_PLAN: &[FieldSpec] = &[
    field("toolsetJobId", FieldKind::String, true),
    field("plan", FieldKind::Object, true),
    field("receiptId", FieldKind::String, true),
    field("receiptDigest", FieldKind::String, true),
    field("sourceRevision", FieldKind::Integer, true),
    field("planDigest", FieldKind::String, true),
    field("methodologyDigest", FieldKind::String, true),
    field("policyDigest", FieldKind::String, true),
    field("inputFingerprint", FieldKind::String, true),
    field("changedPaths", FieldKind::Array, true),
    field("sinceFingerprint", FieldKind::String, false),
    field("persist", FieldKind::Boolean, true),
];
const NO_FIELDS: &[FieldSpec] = &[];

pub const OPERATION_REGISTRY: [OperationSpec; OPERATION_COUNT] = [
    spec(
        OperationName::DocumentResolve,
        false,
        false,
        false,
        false,
        false,
        DOCUMENT_RESOLVE,
    ),
    spec(
        OperationName::DocumentSave,
        true,
        true,
        true,
        true,
        false,
        DOCUMENT_SAVE,
    ),
    spec(
        OperationName::EvidenceFinalize,
        true,
        true,
        true,
        true,
        false,
        NO_FIELDS,
    ),
    spec(
        OperationName::EvidenceRecord,
        true,
        true,
        true,
        true,
        false,
        EVIDENCE_RECORD,
    ),
    spec(
        OperationName::ExecutionPlanApprove,
        true,
        true,
        true,
        true,
        true,
        EXECUTION_PLAN_APPROVE,
    ),
    spec(
        OperationName::ExecutionPlanSet,
        true,
        true,
        true,
        true,
        false,
        EXECUTION_PLAN_SET,
    ),
    spec(
        OperationName::ExecutionResume,
        false,
        false,
        false,
        false,
        false,
        EXECUTION_RESUME,
    ),
    spec(
        OperationName::ExecutionSliceRecord,
        true,
        true,
        true,
        true,
        false,
        EXECUTION_SLICE_RECORD,
    ),
    spec(
        OperationName::ExecutionSliceStart,
        true,
        true,
        true,
        true,
        false,
        EXECUTION_SLICE_START,
    ),
    spec(
        OperationName::GateCheck,
        false,
        false,
        false,
        false,
        false,
        GATE_CHECK,
    ),
    spec(
        OperationName::LeaseAcquire,
        true,
        false,
        false,
        true,
        false,
        LEASE_ACQUIRE,
    ),
    spec(
        OperationName::LeaseBreak,
        true,
        false,
        false,
        true,
        true,
        LEASE_BREAK,
    ),
    spec(
        OperationName::LeaseRelease,
        true,
        true,
        false,
        true,
        false,
        LEASE_OWNER,
    ),
    spec(
        OperationName::LeaseRenew,
        true,
        true,
        false,
        true,
        false,
        LEASE_RENEW,
    ),
    spec(
        OperationName::LeaseStatus,
        false,
        false,
        false,
        false,
        false,
        NO_FIELDS,
    ),
    spec(
        OperationName::ReviewContribute,
        true,
        false,
        true,
        true,
        false,
        REVIEW_CONTRIBUTE,
    ),
    spec(
        OperationName::ReviewFinalize,
        true,
        true,
        true,
        true,
        false,
        NO_FIELDS,
    ),
    spec(
        OperationName::ReviewRecord,
        true,
        true,
        true,
        true,
        false,
        REVIEW_RECORD,
    ),
    spec(
        OperationName::RouteDecide,
        true,
        true,
        true,
        true,
        false,
        ROUTE_DECIDE,
    ),
    spec(
        OperationName::StateNextActions,
        false,
        false,
        false,
        false,
        false,
        NO_FIELDS,
    ),
    spec(
        OperationName::StateTransition,
        true,
        true,
        true,
        true,
        false,
        STATE_TRANSITION,
    ),
    spec(
        OperationName::VerificationPlan,
        true,
        true,
        true,
        true,
        false,
        VERIFICATION_PLAN,
    ),
    spec(
        OperationName::WorkItemComplete,
        true,
        true,
        true,
        true,
        true,
        NO_FIELDS,
    ),
    workspace_spec(OperationName::WorkItemCreate, true, true, WORKITEM_CREATE),
    spec(
        OperationName::WorkItemGet,
        false,
        false,
        false,
        false,
        false,
        NO_FIELDS,
    ),
];

/// Hashes the complete immutable operation registry into a wire identity.
///
/// The encoding is independent of Rust debug output and JSON map ordering. Any
/// operation, scope, precondition, field, kind, or required flag change
/// produces a new digest.
#[must_use]
pub fn operation_schema_digest() -> String {
    let mut digest = Sha256::new();
    digest.update(b"ae-sdd-operation-registry/v1\0");
    for spec in OPERATION_REGISTRY {
        digest.update(spec.operation.as_str().as_bytes());
        digest.update([0, scope_code(spec.scope)]);
        digest.update([
            u8::from(spec.requires_workspace),
            u8::from(spec.requires_work_item),
            u8::from(spec.writes),
            u8::from(spec.requires_lease),
            u8::from(spec.requires_revision),
            u8::from(spec.requires_idempotency),
            u8::from(spec.requires_confirmation),
        ]);
        for field in spec.fields {
            digest.update(field.name.as_bytes());
            digest.update([0, field_kind_code(field.kind), u8::from(field.required)]);
        }
        digest.update([0xff]);
    }
    hex::encode(digest.finalize())
}

const fn scope_code(scope: OperationScope) -> u8 {
    match scope {
        OperationScope::Runtime => 0,
        OperationScope::Workspace => 1,
        OperationScope::WorkItem => 2,
        OperationScope::Session => 3,
        OperationScope::Delegation => 4,
        OperationScope::Host => 5,
    }
}

const fn field_kind_code(kind: FieldKind) -> u8 {
    match kind {
        FieldKind::String => 0,
        FieldKind::Object => 1,
        FieldKind::Array => 2,
        FieldKind::Boolean => 3,
        FieldKind::Integer => 4,
        FieldKind::StringOrArray => 5,
        FieldKind::StringOrObject => 6,
    }
}
