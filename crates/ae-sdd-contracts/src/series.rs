//! Route classification and deterministic Series planning contracts.

use std::collections::BTreeSet;

use ae_sdd_domain::{
    AgentRole, ArtifactDigest, ArtifactRef, DecisionDigest, DelegationId, DeliverableContract,
    DesignRoute, EventSequence, InputFingerprint, OperationId, ProcessPhase, ProjectPathScope,
    ResultDigest, SeriesRunId, SessionId, StateRevision, StoryId, WorkItemId, WorkScale,
};
use ae_sdd_protocol::ConfirmationRef;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    BoundedText, IdempotencyKey, MethodologyRef, MethodologyResolution, ReasonCode,
    RouteDecisionId, SchemaVersion, SeriesId, SeriesKind, SpecKind, intake::TaskKind,
    resource::ContextBundleRef, serde_domain,
};

/// The frozen main-node vocabulary from `ae-sdd-daemon-design.md` §11.1, in
/// flow order. These values *are* the logical `SeriesKind`s, so
/// `currentMainNode` and `SeriesPlan.requiredSeries` draw from one list.
///
/// Two spellings are deliberately absent. `dr` is a [`DesignRoute`] value — a
/// different axis — and the Series producing a DR document is `design-review`.
/// The `{kind}-generate`/`-review`/`-update` split is a *sub-node* activity
/// inside one Series, not a Series identity.
///
/// §11.1 states that rule against `routePredicates`, but the observed drift was
/// in `seriesKind`: every predicate value in the frozen catalog was already a
/// legal main node, while 13 of 15 Series entries spelled `seriesKind` as
/// `{kind}-generate`. Both fields must draw from this list; only one had
/// actually diverged.
pub const MAIN_NODE_SERIES_KINDS: [&str; 8] = [
    "requirement-analysis",
    "design-review",
    "story",
    "testcase",
    "coding-plan",
    "coding",
    "test",
    "review",
];

/// The Series sub-node vocabulary, frozen here as the typed contract.
///
/// `ae-sdd-daemon-design.md` §11.1 lists these five values with "例如", so the
/// design document presents them as illustrative rather than closed, and states
/// no traversal order. The closure and the order are decided *here*: §11.2
/// requires that concrete enumerations live in the typed contract rather than
/// being copied from the conceptual text as unversioned strings, so a reader
/// needing the authoritative list must find it in code. Extending this array is
/// a contract change, not a documentation change.
///
/// These are activities *inside* one Series, not Series identities. That
/// distinction is the reason `{kind}-generate`/`-review`/`-update` must never
/// appear in [`MAIN_NODE_SERIES_KINDS`]: generating and reviewing a Story are
/// two sub-nodes of the `story` Series, not two Series.
///
/// §11.1 does fix who may advance them: the main node moves only by FlowRuntime
/// transition, while a sub-node advances on a valid Series event and still
/// requires daemon validation.
pub const SERIES_SUB_NODES: [&str; 5] = [
    "resolve-spec",
    "collect-context",
    "draft",
    "validate",
    "await-user",
];

/// The frozen methodology-slice activity vocabulary.
///
/// This is the third axis, and it exists because the other two cannot express
/// what a catalog entry is. A main node says *which* Series
/// ([`MAIN_NODE_SERIES_KINDS`]); a sub-node says *where inside* one Series
/// execution is ([`SERIES_SUB_NODES`]); an activity says *which skill role*
/// serves that Series.
///
/// Three Story skills — generate, review, update — all serve the one `story`
/// main node. Reusing [`SERIES_SUB_NODES`] here cannot separate them: both
/// `story-generate` and `story-update` would map to `draft`, so
/// `(seriesKind, subNode)` would not be unique and slice selection would fall
/// back to catalog order. `(seriesKind, activity)` is unique across the frozen
/// catalog.
///
/// `execute` covers a Series whose skill is the whole Series rather than one
/// role within it (`requirement-analysis`, `coding`).
pub const SERIES_ACTIVITIES: [&str; 5] = ["generate", "review", "update", "fix", "execute"];

/// A skill role within one Series, parsed from [`SERIES_ACTIVITIES`].
///
/// This is an enum rather than a validated string because the vocabulary is
/// closed and frozen: an unrecognised activity is a contract violation, not a
/// value to carry forward.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SeriesActivity {
    /// Produces the Series' primary artifact.
    Generate,
    /// Reviews an artifact this Series already produced.
    Review,
    /// Revises an artifact after review.
    Update,
    /// Remediates findings raised against an artifact.
    Fix,
    /// The skill is the whole Series rather than one role inside it.
    Execute,
}

impl SeriesActivity {
    /// Returns the frozen wire encoding.
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Generate => "generate",
            Self::Review => "review",
            Self::Update => "update",
            Self::Fix => "fix",
            Self::Execute => "execute",
        }
    }

    /// Parses a frozen wire value.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "generate" => Some(Self::Generate),
            "review" => Some(Self::Review),
            "update" => Some(Self::Update),
            "fix" => Some(Self::Fix),
            "execute" => Some(Self::Execute),
            _ => None,
        }
    }
}

/// A position inside one Series, parsed from [`SERIES_SUB_NODES`].
///
/// Kept as a separate axis from [`SeriesActivity`] because the two answer
/// different questions and §11.1 keeps them apart: a sub-node is *where* a
/// Series currently is, an activity is *which skill role* serves it. Collapsing
/// them would make `draft` ambiguous between "the drafting position" and "the
/// generating role", which is the confusion that produced `story-generate` as a
/// `seriesKind`.
///
/// Serialized in `kebab-case`, matching the wire values in
/// [`SERIES_SUB_NODES`] — `resolve-spec`, not `resolvespec`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SeriesSubNode {
    /// Binding this Series to an existing Spec or reserving a new one (§8.1).
    ResolveSpec,
    /// Gathering the minimum premise context for the transaction (§9.2).
    CollectContext,
    /// Producing the Series' primary artifact.
    Draft,
    /// Checking the draft against the transaction and Gates.
    Validate,
    /// Blocked pending a user decision (§12.1).
    AwaitUser,
}

impl SeriesSubNode {
    /// Returns the frozen wire encoding.
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::ResolveSpec => "resolve-spec",
            Self::CollectContext => "collect-context",
            Self::Draft => "draft",
            Self::Validate => "validate",
            Self::AwaitUser => "await-user",
        }
    }

    /// Parses a frozen wire value.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "resolve-spec" => Some(Self::ResolveSpec),
            "collect-context" => Some(Self::CollectContext),
            "draft" => Some(Self::Draft),
            "validate" => Some(Self::Validate),
            "await-user" => Some(Self::AwaitUser),
            _ => None,
        }
    }
}

/// Maximum number of available artifacts supplied to Route classification.
pub const MAX_ROUTE_ARTIFACTS: usize = 64;
/// Maximum number of typed impact facts supplied to Route classification.
pub const MAX_IMPACT_FACTS: usize = 64;
/// Maximum number of dependencies, operations, or path scopes in a Series plan.
pub const MAX_SERIES_GRANT_ITEMS: usize = 64;

/// Bounded authoritative process snapshot shared by Series and Lifecycle planners.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessSnapshot {
    /// Contract schema version.
    pub schema_version: SchemaVersion,
    /// Work Item whose process state is represented.
    #[serde(with = "serde_domain::work_item_id")]
    pub work_item_id: WorkItemId,
    /// Current authoritative phase.
    #[serde(with = "serde_domain::process_phase")]
    pub phase: ProcessPhase,
    /// Exact source phase while paused.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_process_phase"
    )]
    pub paused_from: Option<ProcessPhase>,
    /// Authoritative state revision.
    #[serde(with = "serde_domain::state_revision")]
    pub state_revision: StateRevision,
    /// Digest of the authoritative bounded snapshot.
    #[serde(with = "serde_domain::artifact_digest")]
    pub state_digest: ArtifactDigest,
}

impl ProcessSnapshot {
    /// Constructs a bounded process snapshot.
    pub const fn new(
        schema_version: SchemaVersion,
        work_item_id: WorkItemId,
        phase: ProcessPhase,
        paused_from: Option<ProcessPhase>,
        state_revision: StateRevision,
        state_digest: ArtifactDigest,
    ) -> Self {
        Self {
            schema_version,
            work_item_id,
            phase,
            paused_from,
            state_revision,
            state_digest,
        }
    }
}

/// Typed impact severity used by deterministic Route classification.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactLevel {
    /// Smallest single-spot impact; ranks below every ordinary level.
    Micro,
    /// Localized, reversible impact.
    Low,
    /// Cross-module or moderately risky impact.
    Medium,
    /// Cross-agent, security, migration, or high-blast-radius impact.
    High,
}

/// One bounded, machine-readable Route classification fact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImpactFact {
    /// Stable fact code.
    pub code: ReasonCode,
    /// Typed severity.
    pub level: ImpactLevel,
    /// Optional digest of authoritative evidence held elsewhere.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_artifact_digest"
    )]
    pub evidence_digest: Option<ArtifactDigest>,
}

impl ImpactFact {
    /// Constructs a typed impact fact.
    pub const fn new(
        code: ReasonCode,
        level: ImpactLevel,
        evidence_digest: Option<ArtifactDigest>,
    ) -> Self {
        Self {
            code,
            level,
            evidence_digest,
        }
    }
}

/// Error returned when a Route input violates frozen v1 bounds.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RouteInputError {
    /// Confidence basis points exceeded 10,000.
    #[error("classification confidence must be between 0 and 10,000 basis points")]
    ConfidenceOutOfRange,
    /// A bounded collection exceeded its frozen v1 limit.
    #[error("Route input exceeds a frozen v1 collection limit")]
    CollectionLimitExceeded,
}

/// Typed, bounded input to `RouteEngine::decide`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "RouteInputWire", into = "RouteInputWire")]
pub struct RouteInput {
    schema_version: SchemaVersion,
    work_item_id: WorkItemId,
    entry_node: ReasonCode,
    requested_intent: BoundedText<4096>,
    /// The task kind proposed for this route.
    ///
    /// Carried as an *input* because §5.5 requires the decision freeze a
    /// `taskKind`, and §5.3 keeps `BootstrapAssessment.task_kind_proposal`
    /// provisional until RA closes. The engine is pure: if the kind were not an
    /// input, `decide` could only invent it, which would make the frozen
    /// authoritative fact a fabrication rather than a promotion of what the
    /// assessment reported.
    task_kind: TaskKind,
    available_artifacts: Vec<ArtifactRef>,
    impact_facts: Vec<ImpactFact>,
    classification_confidence_bps: u16,
    input_fingerprint: InputFingerprint,
    user_approval_ref: Option<ConfirmationRef>,
}

impl RouteInput {
    /// Constructs a validated Route input without interpreting prose.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        work_item_id: WorkItemId,
        entry_node: ReasonCode,
        requested_intent: BoundedText<4096>,
        task_kind: TaskKind,
        available_artifacts: Vec<ArtifactRef>,
        impact_facts: Vec<ImpactFact>,
        classification_confidence_bps: u16,
        input_fingerprint: InputFingerprint,
        user_approval_ref: Option<ConfirmationRef>,
    ) -> Result<Self, RouteInputError> {
        if classification_confidence_bps > 10_000 {
            return Err(RouteInputError::ConfidenceOutOfRange);
        }
        if available_artifacts.len() > MAX_ROUTE_ARTIFACTS || impact_facts.len() > MAX_IMPACT_FACTS
        {
            return Err(RouteInputError::CollectionLimitExceeded);
        }
        Ok(Self {
            schema_version,
            work_item_id,
            entry_node,
            requested_intent,
            task_kind,
            available_artifacts,
            impact_facts,
            classification_confidence_bps,
            input_fingerprint,
            user_approval_ref,
        })
    }

    /// Returns the frozen contract schema version.
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the Work Item being classified.
    pub const fn work_item_id(&self) -> &WorkItemId {
        &self.work_item_id
    }

    /// Returns the classification confidence in basis points.
    pub const fn classification_confidence_bps(&self) -> u16 {
        self.classification_confidence_bps
    }

    /// Returns typed impact facts.
    pub fn impact_facts(&self) -> &[ImpactFact] {
        &self.impact_facts
    }

    /// Returns the proposed task kind the decision will freeze.
    pub const fn task_kind(&self) -> TaskKind {
        self.task_kind
    }

    /// Returns the optional explicit user approval reference.
    pub const fn user_approval_ref(&self) -> Option<&ConfirmationRef> {
        self.user_approval_ref.as_ref()
    }

    /// Returns the input fingerprint bound to this classification request.
    pub const fn input_fingerprint(&self) -> InputFingerprint {
        self.input_fingerprint
    }
}

impl<'de> Deserialize<'de> for RouteInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        RouteInputWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RouteInputWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::work_item_id")]
    work_item_id: WorkItemId,
    entry_node: ReasonCode,
    requested_intent: BoundedText<4096>,
    #[serde(with = "serde_domain::task_kind")]
    task_kind: TaskKind,
    #[serde(with = "serde_domain::artifact_refs")]
    available_artifacts: Vec<ArtifactRef>,
    impact_facts: Vec<ImpactFact>,
    classification_confidence_bps: u16,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    user_approval_ref: Option<ConfirmationRef>,
}

impl TryFrom<RouteInputWire> for RouteInput {
    type Error = RouteInputError;

    fn try_from(value: RouteInputWire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.work_item_id,
            value.entry_node,
            value.requested_intent,
            value.task_kind,
            value.available_artifacts,
            value.impact_facts,
            value.classification_confidence_bps,
            value.input_fingerprint,
            value.user_approval_ref,
        )
    }
}

impl From<RouteInput> for RouteInputWire {
    fn from(value: RouteInput) -> Self {
        Self {
            schema_version: value.schema_version,
            work_item_id: value.work_item_id,
            entry_node: value.entry_node,
            requested_intent: value.requested_intent,
            task_kind: value.task_kind,
            available_artifacts: value.available_artifacts,
            impact_facts: value.impact_facts,
            classification_confidence_bps: value.classification_confidence_bps,
            input_fingerprint: value.input_fingerprint,
            user_approval_ref: value.user_approval_ref,
        }
    }
}

/// Fail-closed route disposition.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteDisposition {
    /// The route must not execute until the user approves it.
    AwaitUserApproval,
    /// The route may be consumed by the Series planner.
    Approved,
    /// The route was rejected because its inputs conflict or violate policy.
    Denied,
    /// A newer route decision replaced this decision.
    Superseded,
}

/// Error returned when a route decision violates its frozen contract.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RouteDecisionError {
    /// No stable machine reason was supplied.
    #[error("route decision must contain at least one reason code")]
    MissingReason,
    /// No Series was selected for the route.
    #[error("route decision must contain at least one required Series")]
    MissingSeries,
    /// A Series kind appeared more than once.
    #[error("route decision contains a duplicate Series kind")]
    DuplicateSeries,
    /// No Spec kind was required by the route.
    ///
    /// §5.4 makes RA the mandatory Series at every scale, so no route can require
    /// zero Spec documents.
    #[error("route decision must require at least one Spec kind")]
    MissingSpecKinds,
    /// A micro route repeated post-route work that RA has already completed.
    #[error("micro route must have empty required Series and Spec kind lists")]
    MicroMustBeEmpty,
    /// A Spec kind appeared more than once.
    #[error("route decision contains a duplicate Spec kind")]
    DuplicateSpecKind,
    /// A collection exceeded its frozen v1 size budget.
    #[error("route decision exceeds a frozen v1 collection limit")]
    CollectionLimitExceeded,
}

/// Maximum number of reason codes carried by one route decision.
pub const MAX_ROUTE_REASON_CODES: usize = 32;
/// Maximum number of required Series carried by one route decision.
pub const MAX_REQUIRED_SERIES: usize = 32;
/// Maximum number of required Spec kinds carried by one route decision.
///
/// `SpecKind` has five values, so a well-formed list cannot exceed five once
/// duplicates are refused. The limit exists to bound a hostile payload before the
/// duplicate check allocates, matching how the Series limit is applied.
pub const MAX_REQUIRED_SPEC_KINDS: usize = 8;

/// Deterministic route decision exchanged between route and Series ownership.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "RouteDecisionWire", into = "RouteDecisionWire")]
pub struct RouteDecision {
    schema_version: SchemaVersion,
    decision_id: RouteDecisionId,
    work_item_id: WorkItemId,
    /// The task nature frozen at route time (§5.5's first frozen fact).
    ///
    /// Frozen here rather than left on the intake proposal because
    /// `BootstrapAssessment` carries it as `task_kind_proposal` — a *proposal* that
    /// §5.3 keeps provisional until RA closes. Without freezing it onto the
    /// decision, the authoritative task kind exists nowhere once RA has closed.
    task_kind: TaskKind,
    scale: WorkScale,
    design_route: DesignRoute,
    disposition: RouteDisposition,
    reason_codes: Vec<ReasonCode>,
    required_series: Vec<SeriesKind>,
    /// Spec documents this route requires be bound (§5.5's fifth frozen fact).
    ///
    /// Distinct from `required_series`, not a restatement of it: §7.1 line 342
    /// gives a micro task Coding work with *no* standalone CodingPlan Spec, so one
    /// list cannot carry both facts. Kept as a separate field for that reason even
    /// where the two happen to agree.
    required_spec_kinds: Vec<SpecKind>,
    input_fingerprint: InputFingerprint,
    approval_binding_digest: Option<ArtifactDigest>,
    decision_digest: DecisionDigest,
}

impl RouteDecision {
    /// Constructs a validated, replay-safe route decision.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        decision_id: RouteDecisionId,
        work_item_id: WorkItemId,
        task_kind: TaskKind,
        scale: WorkScale,
        design_route: DesignRoute,
        disposition: RouteDisposition,
        reason_codes: Vec<ReasonCode>,
        required_series: Vec<SeriesKind>,
        required_spec_kinds: Vec<SpecKind>,
        input_fingerprint: InputFingerprint,
        approval_binding_digest: Option<ArtifactDigest>,
        decision_digest: DecisionDigest,
    ) -> Result<Self, RouteDecisionError> {
        if reason_codes.is_empty() {
            return Err(RouteDecisionError::MissingReason);
        }
        let micro = scale == WorkScale::Micro;
        if !micro && required_series.is_empty() {
            return Err(RouteDecisionError::MissingSeries);
        }
        // §5.4 makes RA 所有规模的必经 Series, so every route requires at least the
        // RA Spec. An empty list would let a decision claim no document need exist.
        if !micro && required_spec_kinds.is_empty() {
            return Err(RouteDecisionError::MissingSpecKinds);
        }
        if micro && (!required_series.is_empty() || !required_spec_kinds.is_empty()) {
            return Err(RouteDecisionError::MicroMustBeEmpty);
        }
        if reason_codes.len() > MAX_ROUTE_REASON_CODES
            || required_series.len() > MAX_REQUIRED_SERIES
            || required_spec_kinds.len() > MAX_REQUIRED_SPEC_KINDS
        {
            return Err(RouteDecisionError::CollectionLimitExceeded);
        }
        let unique_series: BTreeSet<&SeriesKind> = required_series.iter().collect();
        if unique_series.len() != required_series.len() {
            return Err(RouteDecisionError::DuplicateSeries);
        }
        let unique_spec_kinds: BTreeSet<&SpecKind> = required_spec_kinds.iter().collect();
        if unique_spec_kinds.len() != required_spec_kinds.len() {
            return Err(RouteDecisionError::DuplicateSpecKind);
        }
        Ok(Self {
            schema_version,
            decision_id,
            work_item_id,
            task_kind,
            scale,
            design_route,
            disposition,
            reason_codes,
            required_series,
            required_spec_kinds,
            input_fingerprint,
            approval_binding_digest,
            decision_digest,
        })
    }

    /// Returns the route disposition.
    pub const fn disposition(&self) -> RouteDisposition {
        self.disposition
    }

    /// Returns whether this decision may enter Series planning.
    pub const fn is_approved(&self) -> bool {
        matches!(self.disposition, RouteDisposition::Approved)
    }

    /// Returns the domain-owned work scale.
    pub const fn scale(&self) -> WorkScale {
        self.scale
    }

    /// Returns the domain-owned design route.
    pub const fn design_route(&self) -> DesignRoute {
        self.design_route
    }

    /// Returns the decision identity.
    pub const fn decision_id(&self) -> &RouteDecisionId {
        &self.decision_id
    }

    /// Returns the bound Work Item identity.
    pub const fn work_item_id(&self) -> &WorkItemId {
        &self.work_item_id
    }

    /// Returns the required ordered Series kinds.
    pub fn required_series(&self) -> &[SeriesKind] {
        &self.required_series
    }

    /// Returns the frozen task kind (§5.5's first frozen fact).
    pub const fn task_kind(&self) -> TaskKind {
        self.task_kind
    }

    /// Returns the Spec kinds this route requires be bound.
    ///
    /// Read this, not [`Self::required_series`], to answer "which documents must
    /// exist": §7.1 line 342 gives micro a Coding Series with no CodingPlan Spec.
    pub fn required_spec_kinds(&self) -> &[SpecKind] {
        &self.required_spec_kinds
    }

    /// Returns the input fingerprint bound to the decision.
    pub const fn input_fingerprint(&self) -> InputFingerprint {
        self.input_fingerprint
    }

    /// Returns the canonical decision digest.
    pub const fn decision_digest(&self) -> DecisionDigest {
        self.decision_digest
    }
}

impl<'de> Deserialize<'de> for RouteDecision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        RouteDecisionWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RouteDecisionWire {
    schema_version: SchemaVersion,
    decision_id: RouteDecisionId,
    #[serde(with = "serde_domain::work_item_id")]
    work_item_id: WorkItemId,
    #[serde(with = "serde_domain::task_kind")]
    task_kind: TaskKind,
    #[serde(with = "serde_domain::work_scale")]
    scale: WorkScale,
    #[serde(with = "serde_domain::design_route")]
    design_route: DesignRoute,
    disposition: RouteDisposition,
    reason_codes: Vec<ReasonCode>,
    required_series: Vec<SeriesKind>,
    required_spec_kinds: Vec<SpecKind>,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_artifact_digest"
    )]
    approval_binding_digest: Option<ArtifactDigest>,
    #[serde(with = "serde_domain::decision_digest")]
    decision_digest: DecisionDigest,
}

mod optional_artifact_digest {
    use std::str::FromStr;

    use ae_sdd_domain::ArtifactDigest;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(
        value: &Option<ArtifactDigest>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(|digest| digest.to_string()).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<ArtifactDigest>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| ArtifactDigest::from_str(&value).map_err(de::Error::custom))
            .transpose()
    }
}

impl TryFrom<RouteDecisionWire> for RouteDecision {
    type Error = RouteDecisionError;

    fn try_from(value: RouteDecisionWire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.decision_id,
            value.work_item_id,
            value.task_kind,
            value.scale,
            value.design_route,
            value.disposition,
            value.reason_codes,
            value.required_series,
            value.required_spec_kinds,
            value.input_fingerprint,
            value.approval_binding_digest,
            value.decision_digest,
        )
    }
}

impl From<RouteDecision> for RouteDecisionWire {
    fn from(value: RouteDecision) -> Self {
        Self {
            schema_version: value.schema_version,
            decision_id: value.decision_id,
            work_item_id: value.work_item_id,
            task_kind: value.task_kind,
            scale: value.scale,
            design_route: value.design_route,
            disposition: value.disposition,
            reason_codes: value.reason_codes,
            required_series: value.required_series,
            required_spec_kinds: value.required_spec_kinds,
            input_fingerprint: value.input_fingerprint,
            approval_binding_digest: value.approval_binding_digest,
            decision_digest: value.decision_digest,
        }
    }
}

/// Bounded retry policy carried by a Series plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "RetryPolicyWire", into = "RetryPolicyWire")]
pub struct RetryPolicy {
    max_attempts: u8,
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
}

impl RetryPolicy {
    /// Constructs a bounded retry policy.
    pub fn new(
        max_attempts: u8,
        initial_backoff_ms: u64,
        max_backoff_ms: u64,
    ) -> Result<Self, SeriesPlanError> {
        if !(1..=8).contains(&max_attempts)
            || initial_backoff_ms == 0
            || max_backoff_ms < initial_backoff_ms
            || max_backoff_ms > 60_000
        {
            return Err(SeriesPlanError::InvalidRetryPolicy);
        }
        Ok(Self {
            max_attempts,
            initial_backoff_ms,
            max_backoff_ms,
        })
    }

    /// Returns the maximum attempts including the first attempt.
    pub const fn max_attempts(self) -> u8 {
        self.max_attempts
    }
}

impl<'de> Deserialize<'de> for RetryPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        RetryPolicyWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetryPolicyWire {
    max_attempts: u8,
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
}

impl TryFrom<RetryPolicyWire> for RetryPolicy {
    type Error = SeriesPlanError;

    fn try_from(value: RetryPolicyWire) -> Result<Self, Self::Error> {
        Self::new(
            value.max_attempts,
            value.initial_backoff_ms,
            value.max_backoff_ms,
        )
    }
}

impl From<RetryPolicy> for RetryPolicyWire {
    fn from(value: RetryPolicy) -> Self {
        Self {
            max_attempts: value.max_attempts,
            initial_backoff_ms: value.initial_backoff_ms,
            max_backoff_ms: value.max_backoff_ms,
        }
    }
}

/// Error returned when a Series plan violates frozen v1 bounds or identity.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SeriesPlanError {
    /// Retry values were outside the bounded policy.
    #[error("Series retry policy is outside its frozen v1 bounds")]
    InvalidRetryPolicy,
    /// The physical plan was assigned to a role other than Series.
    #[error("Series plan role must be AgentRole::Series")]
    InvalidRole,
    /// Methodology and plan Series kinds differed.
    #[error("Series plan kind does not match its Methodology reference")]
    MethodologyMismatch,
    /// Context and plan Work Item identities differed.
    #[error("Series plan context is bound to a different Work Item")]
    ContextMismatch,
    /// A required operation or path grant was empty.
    #[error("Series plan requires at least one operation and path scope")]
    EmptyGrant,
    /// A bounded collection exceeded its frozen v1 limit.
    #[error("Series plan exceeds a frozen v1 collection limit")]
    CollectionLimitExceeded,
    /// A dependency, operation, or path scope appeared more than once.
    #[error("Series plan contains a duplicate dependency or grant item")]
    DuplicateItem,
    /// The plan depended on itself.
    #[error("Series plan cannot depend on itself")]
    SelfDependency,
    /// The explicit deadline was missing.
    #[error("Series plan requires a non-zero explicit deadline")]
    MissingDeadline,
}

/// Deterministic physical Series plan handed to a Host/Delegation adapter by C1.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "SeriesPlanWire", into = "SeriesPlanWire")]
pub struct SeriesPlan {
    schema_version: SchemaVersion,
    series_id: SeriesId,
    work_item_id: WorkItemId,
    story_id: Option<StoryId>,
    series_kind: SeriesKind,
    role: AgentRole,
    methodology_ref: MethodologyRef,
    context_ref: ContextBundleRef,
    deliverable_contract: DeliverableContract,
    allowed_operations: Vec<OperationId>,
    allowed_paths: Vec<ProjectPathScope>,
    dependency_ids: Vec<SeriesId>,
    source_revision: StateRevision,
    input_fingerprint: InputFingerprint,
    deadline_unix_ms: u64,
    retry_policy: RetryPolicy,
    plan_digest: DecisionDigest,
}

impl SeriesPlan {
    /// Constructs and validates a bounded Series plan.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        series_id: SeriesId,
        work_item_id: WorkItemId,
        story_id: Option<StoryId>,
        series_kind: SeriesKind,
        role: AgentRole,
        methodology_ref: MethodologyRef,
        context_ref: ContextBundleRef,
        deliverable_contract: DeliverableContract,
        allowed_operations: Vec<OperationId>,
        allowed_paths: Vec<ProjectPathScope>,
        dependency_ids: Vec<SeriesId>,
        source_revision: StateRevision,
        input_fingerprint: InputFingerprint,
        deadline_unix_ms: u64,
        retry_policy: RetryPolicy,
        plan_digest: DecisionDigest,
    ) -> Result<Self, SeriesPlanError> {
        if role != AgentRole::Series {
            return Err(SeriesPlanError::InvalidRole);
        }
        if methodology_ref.series_kind() != &series_kind {
            return Err(SeriesPlanError::MethodologyMismatch);
        }
        if context_ref.work_item_id() != &work_item_id {
            return Err(SeriesPlanError::ContextMismatch);
        }
        if allowed_operations.is_empty() || allowed_paths.is_empty() {
            return Err(SeriesPlanError::EmptyGrant);
        }
        if allowed_operations.len() > MAX_SERIES_GRANT_ITEMS
            || allowed_paths.len() > MAX_SERIES_GRANT_ITEMS
            || dependency_ids.len() > MAX_SERIES_GRANT_ITEMS
        {
            return Err(SeriesPlanError::CollectionLimitExceeded);
        }
        let unique_operations: BTreeSet<&OperationId> = allowed_operations.iter().collect();
        let unique_paths: BTreeSet<&ProjectPathScope> = allowed_paths.iter().collect();
        let unique_dependencies: BTreeSet<&SeriesId> = dependency_ids.iter().collect();
        if unique_operations.len() != allowed_operations.len()
            || unique_paths.len() != allowed_paths.len()
            || unique_dependencies.len() != dependency_ids.len()
        {
            return Err(SeriesPlanError::DuplicateItem);
        }
        if dependency_ids
            .iter()
            .any(|dependency| dependency == &series_id)
        {
            return Err(SeriesPlanError::SelfDependency);
        }
        if deadline_unix_ms == 0 {
            return Err(SeriesPlanError::MissingDeadline);
        }
        Ok(Self {
            schema_version,
            series_id,
            work_item_id,
            story_id,
            series_kind,
            role,
            methodology_ref,
            context_ref,
            deliverable_contract,
            allowed_operations,
            allowed_paths,
            dependency_ids,
            source_revision,
            input_fingerprint,
            deadline_unix_ms,
            retry_policy,
            plan_digest,
        })
    }

    /// Returns the Series identity.
    pub const fn series_id(&self) -> &SeriesId {
        &self.series_id
    }

    /// Returns the Series kind.
    pub const fn series_kind(&self) -> &SeriesKind {
        &self.series_kind
    }

    /// Returns the Methodology reference.
    pub const fn methodology_ref(&self) -> &MethodologyRef {
        &self.methodology_ref
    }

    /// Returns the bounded Context Bundle reference.
    pub const fn context_ref(&self) -> &ContextBundleRef {
        &self.context_ref
    }

    /// Returns the canonical plan digest.
    pub const fn plan_digest(&self) -> DecisionDigest {
        self.plan_digest
    }
}

impl<'de> Deserialize<'de> for SeriesPlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        SeriesPlanWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SeriesPlanWire {
    schema_version: SchemaVersion,
    series_id: SeriesId,
    #[serde(with = "serde_domain::work_item_id")]
    work_item_id: WorkItemId,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_story_id"
    )]
    story_id: Option<StoryId>,
    series_kind: SeriesKind,
    #[serde(with = "serde_domain::agent_role")]
    role: AgentRole,
    methodology_ref: MethodologyRef,
    context_ref: ContextBundleRef,
    #[serde(with = "serde_domain::deliverable_contract")]
    deliverable_contract: DeliverableContract,
    #[serde(with = "operation_ids")]
    allowed_operations: Vec<OperationId>,
    #[serde(with = "serde_domain::project_path_scopes")]
    allowed_paths: Vec<ProjectPathScope>,
    dependency_ids: Vec<SeriesId>,
    #[serde(with = "serde_domain::state_revision")]
    source_revision: StateRevision,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    deadline_unix_ms: u64,
    retry_policy: RetryPolicy,
    #[serde(with = "serde_domain::decision_digest")]
    plan_digest: DecisionDigest,
}

impl TryFrom<SeriesPlanWire> for SeriesPlan {
    type Error = SeriesPlanError;

    fn try_from(value: SeriesPlanWire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.series_id,
            value.work_item_id,
            value.story_id,
            value.series_kind,
            value.role,
            value.methodology_ref,
            value.context_ref,
            value.deliverable_contract,
            value.allowed_operations,
            value.allowed_paths,
            value.dependency_ids,
            value.source_revision,
            value.input_fingerprint,
            value.deadline_unix_ms,
            value.retry_policy,
            value.plan_digest,
        )
    }
}

impl From<SeriesPlan> for SeriesPlanWire {
    fn from(value: SeriesPlan) -> Self {
        Self {
            schema_version: value.schema_version,
            series_id: value.series_id,
            work_item_id: value.work_item_id,
            story_id: value.story_id,
            series_kind: value.series_kind,
            role: value.role,
            methodology_ref: value.methodology_ref,
            context_ref: value.context_ref,
            deliverable_contract: value.deliverable_contract,
            allowed_operations: value.allowed_operations,
            allowed_paths: value.allowed_paths,
            dependency_ids: value.dependency_ids,
            source_revision: value.source_revision,
            input_fingerprint: value.input_fingerprint,
            deadline_unix_ms: value.deadline_unix_ms,
            retry_policy: value.retry_policy,
            plan_digest: value.plan_digest,
        }
    }
}

/// Pure Series planner action with no side effects.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SeriesPlanDecision {
    /// Route approval is required before any Series can run.
    AwaitRouteApproval {
        /// Contract schema version.
        schema_version: SchemaVersion,
        /// Replay identity.
        idempotency_key: IdempotencyKey,
        /// Route decision awaiting approval.
        decision_id: RouteDecisionId,
    },
    /// Dispatch one bounded physical Series plan.
    RunSeries {
        /// Contract schema version.
        schema_version: SchemaVersion,
        /// Replay identity.
        idempotency_key: IdempotencyKey,
        /// Plan to dispatch through C1.
        plan: Box<SeriesPlan>,
    },
    /// Wait for a running Series receipt.
    AwaitSeries {
        /// Contract schema version.
        schema_version: SchemaVersion,
        /// Replay identity.
        idempotency_key: IdempotencyKey,
        /// Running Series identity.
        series_id: SeriesId,
    },
    /// Collect a staged, validated Series result.
    CollectSeries {
        /// Contract schema version.
        schema_version: SchemaVersion,
        /// Replay identity.
        idempotency_key: IdempotencyKey,
        /// Staged Series identity.
        series_id: SeriesId,
    },
    /// Route-required Series are complete.
    Complete {
        /// Contract schema version.
        schema_version: SchemaVersion,
        /// Replay identity.
        idempotency_key: IdempotencyKey,
        /// Digest of the complete Series projection.
        #[serde(with = "serde_domain::decision_digest")]
        projection_digest: DecisionDigest,
    },
}

/// Durable lifecycle state reported for one physical Series.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SeriesReceiptStatus {
    /// Plan exists but has not been dispatched.
    Planned,
    /// Physical Agent work is in progress.
    Running,
    /// Result artifacts are staged for validation and collection.
    ResultStaged,
    /// Result and cleanup receipts were collected.
    Collected,
    /// Series was cancelled.
    Cancelled,
    /// Series terminated with a bounded failure receipt.
    Failed,
}

/// Error returned when a Series receipt violates its status contract.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SeriesReceiptError {
    /// Result-staged or collected status lacked result identity.
    #[error("staged or collected Series receipt requires result ref and digest")]
    MissingResult,
    /// Collected status lacked successful artifact validation or memory cleanup.
    #[error("collected Series receipt requires artifact validation and memory cleanup receipts")]
    CollectionNotReady,
    /// A non-result status carried result data.
    #[error("planned or running Series receipt cannot carry result data")]
    UnexpectedResult,
}

/// Bounded durable receipt consumed by the pure Series planner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "SeriesReceiptWire", into = "SeriesReceiptWire")]
pub struct SeriesReceipt {
    schema_version: SchemaVersion,
    series_id: SeriesId,
    /// The physical attempt this receipt reports on.
    ///
    /// §9.1 line 452 requires the Series transaction define `seriesId/seriesRunId/
    /// workItemId`. A receipt keyed only by `SeriesId` cannot say *which attempt*
    /// produced the result, so §4.1's retry — a new `SeriesRunId` under the same
    /// `SeriesId` — yielded two receipts that looked like restatements of one fact.
    /// §11.4 then has to mark one stale, which requires telling them apart.
    series_run_id: SeriesRunId,
    plan_digest: DecisionDigest,
    status: SeriesReceiptStatus,
    source_revision: StateRevision,
    input_fingerprint: InputFingerprint,
    result_ref: Option<ArtifactRef>,
    result_digest: Option<ResultDigest>,
    physical_session_id: Option<SessionId>,
    delegation_id: Option<DelegationId>,
    event_cursor: Option<EventSequence>,
    artifact_validation_passed: bool,
    memory_cleanup_passed: bool,
    receipt_digest: ResultDigest,
}

impl SeriesReceipt {
    /// Constructs and validates a Series receipt.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        series_id: SeriesId,
        series_run_id: SeriesRunId,
        plan_digest: DecisionDigest,
        status: SeriesReceiptStatus,
        source_revision: StateRevision,
        input_fingerprint: InputFingerprint,
        result_ref: Option<ArtifactRef>,
        result_digest: Option<ResultDigest>,
        physical_session_id: Option<SessionId>,
        delegation_id: Option<DelegationId>,
        event_cursor: Option<EventSequence>,
        artifact_validation_passed: bool,
        memory_cleanup_passed: bool,
        receipt_digest: ResultDigest,
    ) -> Result<Self, SeriesReceiptError> {
        let has_result = result_ref.is_some() && result_digest.is_some();
        match status {
            SeriesReceiptStatus::ResultStaged | SeriesReceiptStatus::Collected if !has_result => {
                return Err(SeriesReceiptError::MissingResult);
            }
            SeriesReceiptStatus::Collected
                if !artifact_validation_passed || !memory_cleanup_passed =>
            {
                return Err(SeriesReceiptError::CollectionNotReady);
            }
            SeriesReceiptStatus::Planned | SeriesReceiptStatus::Running
                if result_ref.is_some() || result_digest.is_some() =>
            {
                return Err(SeriesReceiptError::UnexpectedResult);
            }
            _ => {}
        }
        Ok(Self {
            schema_version,
            series_id,
            series_run_id,
            plan_digest,
            status,
            source_revision,
            input_fingerprint,
            result_ref,
            result_digest,
            physical_session_id,
            delegation_id,
            event_cursor,
            artifact_validation_passed,
            memory_cleanup_passed,
            receipt_digest,
        })
    }

    /// Returns the Series identity.
    pub const fn series_id(&self) -> &SeriesId {
        &self.series_id
    }

    /// Returns the physical attempt this receipt reports on.
    ///
    /// Read this, not [`Self::series_id`], to attribute a result to an attempt:
    /// a retry shares the `SeriesId` and differs only here.
    pub const fn series_run_id(&self) -> &SeriesRunId {
        &self.series_run_id
    }

    /// Returns the lifecycle status.
    pub const fn status(&self) -> SeriesReceiptStatus {
        self.status
    }

    /// Returns whether collection prerequisites are satisfied.
    pub const fn is_collectable(&self) -> bool {
        matches!(self.status, SeriesReceiptStatus::ResultStaged)
            && self.artifact_validation_passed
            && self.memory_cleanup_passed
    }

    /// Returns the plan digest bound to the receipt.
    pub const fn plan_digest(&self) -> DecisionDigest {
        self.plan_digest
    }
}

impl<'de> Deserialize<'de> for SeriesReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        SeriesReceiptWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SeriesReceiptWire {
    schema_version: SchemaVersion,
    series_id: SeriesId,
    #[serde(with = "serde_domain::series_run_id")]
    series_run_id: SeriesRunId,
    #[serde(with = "serde_domain::decision_digest")]
    plan_digest: DecisionDigest,
    status: SeriesReceiptStatus,
    #[serde(with = "serde_domain::state_revision")]
    source_revision: StateRevision,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_domain::optional_artifact_ref"
    )]
    result_ref: Option<ArtifactRef>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_result_digest"
    )]
    result_digest: Option<ResultDigest>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_session_id"
    )]
    physical_session_id: Option<SessionId>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_delegation_id"
    )]
    delegation_id: Option<DelegationId>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_event_sequence"
    )]
    event_cursor: Option<EventSequence>,
    artifact_validation_passed: bool,
    memory_cleanup_passed: bool,
    #[serde(with = "serde_domain::result_digest")]
    receipt_digest: ResultDigest,
}

impl TryFrom<SeriesReceiptWire> for SeriesReceipt {
    type Error = SeriesReceiptError;

    fn try_from(value: SeriesReceiptWire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.series_id,
            value.series_run_id,
            value.plan_digest,
            value.status,
            value.source_revision,
            value.input_fingerprint,
            value.result_ref,
            value.result_digest,
            value.physical_session_id,
            value.delegation_id,
            value.event_cursor,
            value.artifact_validation_passed,
            value.memory_cleanup_passed,
            value.receipt_digest,
        )
    }
}

impl From<SeriesReceipt> for SeriesReceiptWire {
    fn from(value: SeriesReceipt) -> Self {
        Self {
            schema_version: value.schema_version,
            series_id: value.series_id,
            series_run_id: value.series_run_id,
            plan_digest: value.plan_digest,
            status: value.status,
            source_revision: value.source_revision,
            input_fingerprint: value.input_fingerprint,
            result_ref: value.result_ref,
            result_digest: value.result_digest,
            physical_session_id: value.physical_session_id,
            delegation_id: value.delegation_id,
            event_cursor: value.event_cursor,
            artifact_validation_passed: value.artifact_validation_passed,
            memory_cleanup_passed: value.memory_cleanup_passed,
            receipt_digest: value.receipt_digest,
        }
    }
}

/// Error returned when a Series planner input is stale, ambiguous, or incomplete.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SeriesInputError {
    /// Only the root orchestration role may plan global Series work.
    #[error("Series planning requires AgentRole::Root")]
    InvalidRole,
    /// Process or candidate plan revision did not match the supplied revision.
    #[error("Series input contains a stale revision")]
    RevisionMismatch,
    /// Route and process snapshot belong to different Work Items.
    #[error("Series route and process snapshot belong to different Work Items")]
    WorkItemMismatch,
    /// A bounded collection exceeded its frozen v1 limit.
    #[error("Series input exceeds a frozen v1 collection limit")]
    CollectionLimitExceeded,
    /// A required Series did not have exactly one candidate plan and Methodology resolution.
    #[error(
        "Series input does not provide one plan and Methodology resolution per required Series"
    )]
    IncompleteCandidates,
    /// Candidate plan identity or receipt identity appeared more than once.
    #[error("Series input contains a duplicate candidate or receipt identity")]
    DuplicateIdentity,
    /// Candidate plan and Methodology resolution did not refer to the same entry.
    #[error("Series candidate plan does not match its Methodology resolution")]
    MethodologyMismatch,
    /// Pre-route RA planning is only valid before route selection.
    #[error("Requirement Analysis planning requires Initialized or RequirementAnalyzed phase")]
    InvalidRequirementAnalysisPhase,
    /// The pre-route plan must be the single Requirement Analysis Series.
    #[error("Requirement Analysis input contains a non-RA plan or receipt binding")]
    InvalidRequirementAnalysisBinding,
}

/// Route-less deterministic input for initial or correction Requirement Analysis.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(
    try_from = "RequirementAnalysisSeriesInputWire",
    into = "RequirementAnalysisSeriesInputWire"
)]
pub struct RequirementAnalysisSeriesInput {
    schema_version: SchemaVersion,
    work_item_id: WorkItemId,
    process_snapshot: ProcessSnapshot,
    provisional_intake_fingerprint: InputFingerprint,
    methodology_resolution: MethodologyResolution,
    candidate_plan: SeriesPlan,
    existing_receipt: Option<SeriesReceipt>,
    role: AgentRole,
    state_revision: StateRevision,
    input_fingerprint: InputFingerprint,
    idempotency_key: IdempotencyKey,
}

impl RequirementAnalysisSeriesInput {
    /// Constructs a route-less RA planning input and freezes all freshness bindings.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        work_item_id: WorkItemId,
        process_snapshot: ProcessSnapshot,
        provisional_intake_fingerprint: InputFingerprint,
        methodology_resolution: MethodologyResolution,
        candidate_plan: SeriesPlan,
        existing_receipt: Option<SeriesReceipt>,
        role: AgentRole,
        state_revision: StateRevision,
        input_fingerprint: InputFingerprint,
        idempotency_key: IdempotencyKey,
    ) -> Result<Self, SeriesInputError> {
        if role != AgentRole::Root {
            return Err(SeriesInputError::InvalidRole);
        }
        if !matches!(
            process_snapshot.phase,
            ProcessPhase::Initialized | ProcessPhase::RequirementAnalyzed
        ) {
            return Err(SeriesInputError::InvalidRequirementAnalysisPhase);
        }
        if process_snapshot.work_item_id != work_item_id
            || candidate_plan.work_item_id != work_item_id
        {
            return Err(SeriesInputError::WorkItemMismatch);
        }
        if process_snapshot.state_revision != state_revision
            || candidate_plan.source_revision != state_revision
        {
            return Err(SeriesInputError::RevisionMismatch);
        }
        let is_ra = candidate_plan.series_kind.as_str() == "requirement-analysis"
            && methodology_resolution.methodology_ref() == &candidate_plan.methodology_ref
            && candidate_plan.input_fingerprint == input_fingerprint;
        let receipt_is_bound = existing_receipt.as_ref().is_none_or(|receipt| {
            receipt.series_id == candidate_plan.series_id
                && receipt.plan_digest == candidate_plan.plan_digest
                && receipt.source_revision == state_revision
                && receipt.input_fingerprint == input_fingerprint
        });
        if !is_ra || !receipt_is_bound {
            return Err(SeriesInputError::InvalidRequirementAnalysisBinding);
        }
        Ok(Self {
            schema_version,
            work_item_id,
            process_snapshot,
            provisional_intake_fingerprint,
            methodology_resolution,
            candidate_plan,
            existing_receipt,
            role,
            state_revision,
            input_fingerprint,
            idempotency_key,
        })
    }

    /// Returns the single RA candidate plan.
    pub const fn candidate_plan(&self) -> &SeriesPlan {
        &self.candidate_plan
    }

    /// Returns the optional receipt for the same RA plan.
    pub const fn existing_receipt(&self) -> Option<&SeriesReceipt> {
        self.existing_receipt.as_ref()
    }

    /// Returns the selected RA methodology resolution.
    pub const fn methodology_resolution(&self) -> &MethodologyResolution {
        &self.methodology_resolution
    }

    /// Returns the provisional intake facts fingerprint.
    pub const fn provisional_intake_fingerprint(&self) -> InputFingerprint {
        self.provisional_intake_fingerprint
    }

    /// Returns the replay identity.
    pub const fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }
}

impl<'de> Deserialize<'de> for RequirementAnalysisSeriesInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        RequirementAnalysisSeriesInputWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequirementAnalysisSeriesInputWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::work_item_id")]
    work_item_id: WorkItemId,
    process_snapshot: ProcessSnapshot,
    #[serde(with = "serde_domain::input_fingerprint")]
    provisional_intake_fingerprint: InputFingerprint,
    methodology_resolution: MethodologyResolution,
    candidate_plan: SeriesPlan,
    existing_receipt: Option<SeriesReceipt>,
    #[serde(with = "serde_domain::agent_role")]
    role: AgentRole,
    #[serde(with = "serde_domain::state_revision")]
    state_revision: StateRevision,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    idempotency_key: IdempotencyKey,
}

impl TryFrom<RequirementAnalysisSeriesInputWire> for RequirementAnalysisSeriesInput {
    type Error = SeriesInputError;

    fn try_from(value: RequirementAnalysisSeriesInputWire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.work_item_id,
            value.process_snapshot,
            value.provisional_intake_fingerprint,
            value.methodology_resolution,
            value.candidate_plan,
            value.existing_receipt,
            value.role,
            value.state_revision,
            value.input_fingerprint,
            value.idempotency_key,
        )
    }
}

impl From<RequirementAnalysisSeriesInput> for RequirementAnalysisSeriesInputWire {
    fn from(value: RequirementAnalysisSeriesInput) -> Self {
        Self {
            schema_version: value.schema_version,
            work_item_id: value.work_item_id,
            process_snapshot: value.process_snapshot,
            provisional_intake_fingerprint: value.provisional_intake_fingerprint,
            methodology_resolution: value.methodology_resolution,
            candidate_plan: value.candidate_plan,
            existing_receipt: value.existing_receipt,
            role: value.role,
            state_revision: value.state_revision,
            input_fingerprint: value.input_fingerprint,
            idempotency_key: value.idempotency_key,
        }
    }
}

/// Complete deterministic input to `SeriesPlanner::next`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "SeriesInputWire", into = "SeriesInputWire")]
pub struct SeriesInput {
    schema_version: SchemaVersion,
    route: RouteDecision,
    process_snapshot: ProcessSnapshot,
    existing_receipts: Vec<SeriesReceipt>,
    methodology_resolutions: Vec<MethodologyResolution>,
    candidate_plans: Vec<SeriesPlan>,
    role: AgentRole,
    state_revision: StateRevision,
    input_fingerprint: InputFingerprint,
    idempotency_key: IdempotencyKey,
}

impl SeriesInput {
    /// Constructs and validates planner input.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        route: RouteDecision,
        process_snapshot: ProcessSnapshot,
        existing_receipts: Vec<SeriesReceipt>,
        methodology_resolutions: Vec<MethodologyResolution>,
        candidate_plans: Vec<SeriesPlan>,
        role: AgentRole,
        state_revision: StateRevision,
        input_fingerprint: InputFingerprint,
        idempotency_key: IdempotencyKey,
    ) -> Result<Self, SeriesInputError> {
        if role != AgentRole::Root {
            return Err(SeriesInputError::InvalidRole);
        }
        if state_revision != process_snapshot.state_revision
            || candidate_plans
                .iter()
                .any(|plan| plan.source_revision != state_revision)
        {
            return Err(SeriesInputError::RevisionMismatch);
        }
        if route.work_item_id() != &process_snapshot.work_item_id
            || candidate_plans
                .iter()
                .any(|plan| plan.work_item_id != process_snapshot.work_item_id)
        {
            return Err(SeriesInputError::WorkItemMismatch);
        }
        if existing_receipts.len() > MAX_SERIES_GRANT_ITEMS
            || methodology_resolutions.len() > MAX_REQUIRED_SERIES
            || candidate_plans.len() > MAX_REQUIRED_SERIES
        {
            return Err(SeriesInputError::CollectionLimitExceeded);
        }
        let candidate_ids: BTreeSet<&SeriesId> =
            candidate_plans.iter().map(|plan| &plan.series_id).collect();
        let receipt_ids: BTreeSet<&SeriesId> = existing_receipts
            .iter()
            .map(|receipt| &receipt.series_id)
            .collect();
        if candidate_ids.len() != candidate_plans.len()
            || receipt_ids.len() != existing_receipts.len()
        {
            return Err(SeriesInputError::DuplicateIdentity);
        }
        if route.disposition() == RouteDisposition::Approved {
            let all_candidates_present = route.required_series().iter().all(|required| {
                candidate_plans
                    .iter()
                    .filter(|plan| &plan.series_kind == required)
                    .count()
                    == 1
                    && methodology_resolutions
                        .iter()
                        .filter(|resolution| resolution.methodology_ref().series_kind() == required)
                        .count()
                        == 1
            });
            if !all_candidates_present
                || candidate_plans.len() != route.required_series().len()
                || methodology_resolutions.len() != route.required_series().len()
            {
                return Err(SeriesInputError::IncompleteCandidates);
            }
        }
        if candidate_plans.iter().any(|plan| {
            !methodology_resolutions
                .iter()
                .any(|resolution| resolution.methodology_ref() == &plan.methodology_ref)
        }) {
            return Err(SeriesInputError::MethodologyMismatch);
        }
        Ok(Self {
            schema_version,
            route,
            process_snapshot,
            existing_receipts,
            methodology_resolutions,
            candidate_plans,
            role,
            state_revision,
            input_fingerprint,
            idempotency_key,
        })
    }

    /// Returns the route decision.
    pub const fn route(&self) -> &RouteDecision {
        &self.route
    }

    /// Returns the authoritative process snapshot.
    pub const fn process_snapshot(&self) -> &ProcessSnapshot {
        &self.process_snapshot
    }

    /// Returns existing Series receipts.
    pub fn existing_receipts(&self) -> &[SeriesReceipt] {
        &self.existing_receipts
    }

    /// Returns candidate Series plans in route order.
    pub fn candidate_plans(&self) -> &[SeriesPlan] {
        &self.candidate_plans
    }

    /// Returns the replay identity.
    pub const fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }
}

impl<'de> Deserialize<'de> for SeriesInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        SeriesInputWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SeriesInputWire {
    schema_version: SchemaVersion,
    route: RouteDecision,
    process_snapshot: ProcessSnapshot,
    existing_receipts: Vec<SeriesReceipt>,
    methodology_resolutions: Vec<MethodologyResolution>,
    candidate_plans: Vec<SeriesPlan>,
    #[serde(with = "serde_domain::agent_role")]
    role: AgentRole,
    #[serde(with = "serde_domain::state_revision")]
    state_revision: StateRevision,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    idempotency_key: IdempotencyKey,
}

impl TryFrom<SeriesInputWire> for SeriesInput {
    type Error = SeriesInputError;

    fn try_from(value: SeriesInputWire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.route,
            value.process_snapshot,
            value.existing_receipts,
            value.methodology_resolutions,
            value.candidate_plans,
            value.role,
            value.state_revision,
            value.input_fingerprint,
            value.idempotency_key,
        )
    }
}

impl From<SeriesInput> for SeriesInputWire {
    fn from(value: SeriesInput) -> Self {
        Self {
            schema_version: value.schema_version,
            route: value.route,
            process_snapshot: value.process_snapshot,
            existing_receipts: value.existing_receipts,
            methodology_resolutions: value.methodology_resolutions,
            candidate_plans: value.candidate_plans,
            role: value.role,
            state_revision: value.state_revision,
            input_fingerprint: value.input_fingerprint,
            idempotency_key: value.idempotency_key,
        }
    }
}

mod optional_result_digest {
    use std::str::FromStr;

    use ae_sdd_domain::ResultDigest;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(
        value: &Option<ResultDigest>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(|digest| digest.to_string()).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<ResultDigest>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| ResultDigest::from_str(&value).map_err(de::Error::custom))
            .transpose()
    }
}

mod optional_session_id {
    use std::str::FromStr;

    use ae_sdd_domain::SessionId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(value: &Option<SessionId>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(|id| id.to_string()).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<SessionId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| SessionId::from_str(&value).map_err(de::Error::custom))
            .transpose()
    }
}

mod optional_delegation_id {
    use std::str::FromStr;

    use ae_sdd_domain::DelegationId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(
        value: &Option<DelegationId>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(|id| id.to_string()).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<DelegationId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| DelegationId::from_str(&value).map_err(de::Error::custom))
            .transpose()
    }
}

mod optional_event_sequence {
    use ae_sdd_domain::EventSequence;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(super) fn serialize<S>(
        value: &Option<EventSequence>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(EventSequence::get).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<EventSequence>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Option::<u64>::deserialize(deserializer)?.map(EventSequence::new))
    }
}

mod optional_story_id {
    use ae_sdd_domain::StoryId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(value: &Option<StoryId>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .as_ref()
            .map(ToString::to_string)
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<StoryId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| StoryId::new(value).map_err(de::Error::custom))
            .transpose()
    }
}

mod operation_ids {
    use ae_sdd_domain::OperationId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(value: &[OperationId], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<OperationId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<String>::deserialize(deserializer)?
            .into_iter()
            .map(|value| OperationId::new(value).map_err(de::Error::custom))
            .collect()
    }
}

mod optional_process_phase {
    use ae_sdd_domain::ProcessPhase;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    fn as_str(value: ProcessPhase) -> &'static str {
        match value {
            ProcessPhase::Initialized => "initialized",
            ProcessPhase::RouteSelected => "route_selected",
            ProcessPhase::RequirementAnalyzed => "requirement_analyzed",
            ProcessPhase::DrGenerated => "dr_generated",
            ProcessPhase::StoryGenerated => "story_generated",
            ProcessPhase::TestcaseGenerated => "testcase_generated",
            ProcessPhase::CodingProcess => "coding_process",
            ProcessPhase::Coding => "coding",
            ProcessPhase::TestRunning => "test_running",
            ProcessPhase::CodeReviewed => "code_reviewed",
            ProcessPhase::Completed => "completed",
            ProcessPhase::Paused => "paused",
        }
    }

    pub(super) fn serialize<S>(
        value: &Option<ProcessPhase>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(as_str).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<ProcessPhase>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| match value.as_str() {
                "initialized" => Ok(ProcessPhase::Initialized),
                "route_selected" => Ok(ProcessPhase::RouteSelected),
                "requirement_analyzed" => Ok(ProcessPhase::RequirementAnalyzed),
                "dr_generated" => Ok(ProcessPhase::DrGenerated),
                "story_generated" => Ok(ProcessPhase::StoryGenerated),
                "testcase_generated" => Ok(ProcessPhase::TestcaseGenerated),
                "coding_process" => Ok(ProcessPhase::CodingProcess),
                "coding" => Ok(ProcessPhase::Coding),
                "test_running" => Ok(ProcessPhase::TestRunning),
                "code_reviewed" => Ok(ProcessPhase::CodeReviewed),
                "completed" => Ok(ProcessPhase::Completed),
                "paused" => Ok(ProcessPhase::Paused),
                _ => Err(de::Error::custom("unknown process phase")),
            })
            .transpose()
    }
}

/// The conceptual Series lifecycle of `ae-sdd-daemon-design.md` §11.2.
///
/// §11.2 closes with a direct instruction: "具体状态枚举属于 typed contract；实现
/// 不得直接复制本图文本作为未版本化字符串状态机." This enum is that typed
/// contract, which is why the states are variants rather than the diagram's
/// strings.
///
/// Distinct from [`SeriesReceiptStatus`], which is a deliberately coarser
/// *durable projection*: a receipt reports six observable outcomes, while this
/// graph carries the eleven non-terminal positions a Series moves through.
/// Collapsing them would lose the distinction between "planned but unbound" and
/// "bound and ready", which is exactly what §8.1's spec-binding step decides.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SeriesLifecycleState {
    /// A plan exists.
    Planned,
    /// Waiting for the Spec binding of §8.1 to resolve or reserve a document.
    AwaitingSpecBinding,
    /// Bound and dispatchable.
    Ready,
    /// A spawn was requested of the Host adapter.
    SpawnRequested,
    /// A child claimed the delegation.
    Claimed,
    /// Physical Agent work is in progress.
    Running,
    /// Blocked pending a user ruling (§6.2 rule 4, §12.1).
    AwaitingUser,
    /// Blocked pending a Gate outcome.
    AwaitingGate,
    /// A further attempt is being prepared.
    Retrying,
    /// Results are staged for validation.
    ResultStaged,
    /// Results passed daemon validation.
    Validated,
    /// The Series closed successfully.
    Completed,
    /// Terminated by a bounded failure.
    Failed,
    /// Terminated by cancellation.
    Cancelled,
    /// Superseded because its inputs moved.
    Stale,
    /// Terminated by an interruption such as a daemon restart.
    Interrupted,
}

impl SeriesLifecycleState {
    /// The four *failure* terminals of §11.2 line 597, which every non-terminal
    /// state can reach.
    ///
    /// `Completed` is terminal too but is deliberately not here: line 597 grants
    /// every 非终态 an edge to these four, and §10 line 670 requires a Series be
    /// in a 合法终态 to be collected, so `Completed` is a legal terminal that must
    /// *not* acquire an edge to `Failed`. Putting it in this array would let a
    /// collected Series later report failure.
    pub const FAILURE_TERMINAL: [Self; 4] = [
        Self::Failed,
        Self::Cancelled,
        Self::Stale,
        Self::Interrupted,
    ];

    /// Whether this state admits no further transition.
    ///
    /// §11.2 phrases the escape hatch as "任意非终态 -> failed | cancelled |
    /// stale | interrupted", so terminality is what decides which states may
    /// still move at all. Answering it from a list rather than per-call-site
    /// keeps one definition of "done".
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Stale | Self::Interrupted
        )
    }

    /// Returns the frozen wire encoding.
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::AwaitingSpecBinding => "awaiting_spec_binding",
            Self::Ready => "ready",
            Self::SpawnRequested => "spawn_requested",
            Self::Claimed => "claimed",
            Self::Running => "running",
            Self::AwaitingUser => "awaiting_user",
            Self::AwaitingGate => "awaiting_gate",
            Self::Retrying => "retrying",
            Self::ResultStaged => "result_staged",
            Self::Validated => "validated",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Stale => "stale",
            Self::Interrupted => "interrupted",
        }
    }

    /// Parses a frozen wire value.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "planned" => Some(Self::Planned),
            "awaiting_spec_binding" => Some(Self::AwaitingSpecBinding),
            "ready" => Some(Self::Ready),
            "spawn_requested" => Some(Self::SpawnRequested),
            "claimed" => Some(Self::Claimed),
            "running" => Some(Self::Running),
            "awaiting_user" => Some(Self::AwaitingUser),
            "awaiting_gate" => Some(Self::AwaitingGate),
            "retrying" => Some(Self::Retrying),
            "result_staged" => Some(Self::ResultStaged),
            "validated" => Some(Self::Validated),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            "stale" => Some(Self::Stale),
            "interrupted" => Some(Self::Interrupted),
            _ => None,
        }
    }
}

impl SeriesLifecycleState {
    /// The states reachable from `self` in one step.
    ///
    /// The spine follows §11.2's diagram order. Two ambiguities in that diagram
    /// are resolved from elsewhere in the design rather than by preference:
    ///
    /// `awaiting_user` and `awaiting_gate` return to `running`, because both are
    /// blocks on a physical attempt that is still alive — §6.2 rule 4 holds the
    /// flow until a ruling arrives, and holding implies resuming.
    ///
    /// `retrying` goes to `spawn_requested`, not back to `running`. §4.1 fixes
    /// that a retry mints a new `SeriesRunId` while keeping the same `SeriesId`,
    /// so the current physical attempt is over and a fresh one must be spawned
    /// and claimed. Returning to `running` would let one `SeriesRunId` cover two
    /// attempts, which is the conflation §4.1 exists to prevent.
    ///
    /// Every non-terminal state additionally reaches all four terminal states,
    /// per §11.2 line 597.
    #[must_use]
    pub fn next_states(self) -> Vec<Self> {
        if self.is_terminal() {
            return Vec::new();
        }
        let mut next = match self {
            Self::Planned => vec![Self::AwaitingSpecBinding],
            Self::AwaitingSpecBinding => vec![Self::Ready],
            Self::Ready => vec![Self::SpawnRequested],
            Self::SpawnRequested => vec![Self::Claimed],
            Self::Claimed => vec![Self::Running],
            Self::Running => vec![
                Self::AwaitingUser,
                Self::AwaitingGate,
                Self::Retrying,
                Self::ResultStaged,
            ],
            Self::AwaitingUser | Self::AwaitingGate => vec![Self::Running],
            Self::Retrying => vec![Self::SpawnRequested],
            Self::ResultStaged => vec![Self::Validated],
            Self::Validated => vec![Self::Completed],
            Self::Completed => Vec::new(),
            Self::Failed | Self::Cancelled | Self::Stale | Self::Interrupted => Vec::new(),
        };
        next.extend_from_slice(&Self::FAILURE_TERMINAL);
        next.sort();
        next.dedup();
        next
    }

    /// Whether `self -> candidate` is a legal single step.
    pub fn can_advance_to(self, candidate: Self) -> bool {
        self.next_states().contains(&candidate)
    }
}

impl SeriesLifecycleState {
    /// Projects this conceptual state onto the durable [`SeriesReceiptStatus`].
    ///
    /// Two enums describing one lifecycle drift apart unless the mapping between
    /// them is written down once. This is that mapping, and it is deliberately
    /// lossy in one direction only: several conceptual states share a receipt
    /// status, but every conceptual state has exactly one.
    ///
    /// `Validated` maps to `Collected` rather than a status of its own because
    /// §11.4 treats validation and collection as one durable outcome — a receipt
    /// exists once results are collected, and `artifact_validation_passed`
    /// already carries whether validation succeeded.
    ///
    /// `Stale` and `Interrupted` both map to `Cancelled`: neither produced a
    /// result, and `SeriesReceiptStatus` has no variant for them. That is a real
    /// loss of detail, recorded here rather than hidden — a caller needing to
    /// distinguish a superseded Series from an interrupted one must read the
    /// conceptual state, not the receipt.
    pub const fn to_receipt_status(self) -> SeriesReceiptStatus {
        match self {
            Self::Planned | Self::AwaitingSpecBinding | Self::Ready | Self::SpawnRequested => {
                SeriesReceiptStatus::Planned
            }
            Self::Claimed
            | Self::Running
            | Self::AwaitingUser
            | Self::AwaitingGate
            | Self::Retrying => SeriesReceiptStatus::Running,
            Self::ResultStaged => SeriesReceiptStatus::ResultStaged,
            Self::Validated | Self::Completed => SeriesReceiptStatus::Collected,
            Self::Failed => SeriesReceiptStatus::Failed,
            Self::Cancelled | Self::Stale | Self::Interrupted => SeriesReceiptStatus::Cancelled,
        }
    }
}
