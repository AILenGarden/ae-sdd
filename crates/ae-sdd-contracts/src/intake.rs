//! Bootstrap intake contracts: the provisional assessment a root Agent reports
//! after the Hook fires, before any authoritative routing exists.
//!
//! `ae-sdd-daemon-design.md` §5.3 makes the split explicit: the Hook records a
//! *provisional* `BootstrapAssessment`, and the authoritative `EngineeringRoute`
//! is frozen only after RA closes its input conflicts. Everything here is a
//! proposal or an observed fact, never authority.

use ae_sdd_domain::{
    ArtifactDigest, ArtifactRef, ProjectRelativePath, SessionId, TurnId, WorkScale,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    BoundedText, DocumentId, SchemaVersion,
    document::{DocumentVersionError, DocumentVersionId},
    serde_domain,
};

/// Maximum typed facts carried by one assessment.
pub const MAX_ASSESSMENT_FACTS: usize = 64;
/// Maximum uncertainties carried by one assessment.
pub const MAX_ASSESSMENT_UNCERTAINTIES: usize = 32;
/// Maximum user questions carried by one assessment.
pub const MAX_ASSESSMENT_QUESTIONS: usize = 32;

/// Task kind a bootstrap assessment may propose.
///
/// `ae-sdd-daemon-design.md` §5.3 admits exactly two values. Self-update is not
/// a privileged path: it still runs RA as its first business Series.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TaskKind {
    /// The task changes ae-sdd itself.
    SelfUpdate,
    /// The task changes the user's project.
    Implementation,
}

impl TaskKind {
    /// Returns the frozen wire encoding.
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::SelfUpdate => "self_update",
            Self::Implementation => "implementation",
        }
    }

    /// Parses the frozen wire encoding.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "self_update" => Some(Self::SelfUpdate),
            "implementation" => Some(Self::Implementation),
            _ => None,
        }
    }
}

/// A requirement input source.
///
/// `ae-sdd-daemon-design.md` §6.1 allows any combination of the three, and §6.2
/// rule 3 forbids a silent override order between them: a semantic clash must
/// become a `RequirementConflict` rather than one source quietly winning.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InputSource {
    /// Spoken or typed user narration, bound to a turn and session.
    Oral,
    /// A prototype or demo, which proves observable behaviour only.
    Prototype,
    /// A PRD, which declares design intent only.
    Prd,
}

impl InputSource {
    /// Returns the frozen wire encoding.
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Oral => "oral",
            Self::Prototype => "prototype",
            Self::Prd => "prd",
        }
    }

    /// Parses the frozen wire encoding.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "oral" => Some(Self::Oral),
            "prototype" => Some(Self::Prototype),
            "prd" => Some(Self::Prd),
            _ => None,
        }
    }
}

/// One typed fact behind an assessment judgement.
///
/// §5.3 requires the Agent to "list the judging facts, not just return an
/// enum", so a bare `scaleProposal` with no facts is an incomplete report.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AssessmentFact {
    dimension: BoundedText<256>,
    value: BoundedText<1024>,
}

impl AssessmentFact {
    /// Builds a typed assessment fact.
    pub const fn new(dimension: BoundedText<256>, value: BoundedText<1024>) -> Self {
        Self { dimension, value }
    }

    /// Returns the judged dimension.
    pub const fn dimension(&self) -> &BoundedText<256> {
        &self.dimension
    }

    /// Returns the observed value.
    pub const fn value(&self) -> &BoundedText<1024> {
        &self.value
    }
}

/// Rejection reasons for a malformed bootstrap assessment.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BootstrapAssessmentError {
    /// The report carried no facts, so the proposal is unsupported.
    #[error("bootstrap assessment must list the facts behind its proposal")]
    MissingFacts,
    /// The report carried no input source.
    #[error("bootstrap assessment must report at least one input source")]
    MissingInputSource,
    /// A bounded collection exceeded its frozen cap.
    #[error("bootstrap assessment collection exceeded its bound")]
    TooManyEntries,
}

/// A provisional intake assessment.
///
/// This is deliberately *not* authority. §5.2 rule 4 and §5.3 keep it
/// `provisional` until RA completes; the daemon records it as an audit fact and
/// mints the `FlowRunId`, but the binding route comes from `EngineeringRoute`.
/// Deserialization routes through [`Self::new`] by `try_from`.
///
/// A plain `derive(Deserialize)` skipped two things, both verified against this
/// type: a payload with `"facts": []` decoded successfully although `new` refuses
/// it, and `["prd","oral","prd","oral"]` decoded verbatim rather than being
/// sorted and deduplicated. The second is the sharper failure — §6.2 rule 3
/// forbids resolving competing sources by precedence, and a preserved input order
/// *is* an implied precedence, so the canonicalization in `new` is a contract
/// rule rather than tidiness.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "BootstrapAssessmentWire", into = "BootstrapAssessmentWire")]
pub struct BootstrapAssessment {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::task_kind")]
    task_kind_proposal: TaskKind,
    #[serde(with = "serde_domain::work_scale")]
    scale_proposal: WorkScale,
    #[serde(with = "serde_domain::input_sources")]
    input_sources: Vec<InputSource>,
    facts: Vec<AssessmentFact>,
    uncertainties: Vec<BoundedText<1024>>,
    user_questions: Vec<BoundedText<1024>>,
}

impl BootstrapAssessment {
    /// Validates and builds a provisional assessment.
    pub fn new(
        schema_version: SchemaVersion,
        task_kind_proposal: TaskKind,
        scale_proposal: WorkScale,
        mut input_sources: Vec<InputSource>,
        facts: Vec<AssessmentFact>,
        uncertainties: Vec<BoundedText<1024>>,
        user_questions: Vec<BoundedText<1024>>,
    ) -> Result<Self, BootstrapAssessmentError> {
        if facts.is_empty() {
            return Err(BootstrapAssessmentError::MissingFacts);
        }
        if input_sources.is_empty() {
            return Err(BootstrapAssessmentError::MissingInputSource);
        }
        if facts.len() > MAX_ASSESSMENT_FACTS
            || uncertainties.len() > MAX_ASSESSMENT_UNCERTAINTIES
            || user_questions.len() > MAX_ASSESSMENT_QUESTIONS
        {
            return Err(BootstrapAssessmentError::TooManyEntries);
        }
        input_sources.sort_unstable();
        input_sources.dedup();
        Ok(Self {
            schema_version,
            task_kind_proposal,
            scale_proposal,
            input_sources,
            facts,
            uncertainties,
            user_questions,
        })
    }

    /// Returns the contract schema version.
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the proposed task kind. This is a proposal, never authority.
    pub const fn task_kind_proposal(&self) -> TaskKind {
        self.task_kind_proposal
    }

    /// Returns the proposed scale. RA may overturn it.
    pub const fn scale_proposal(&self) -> WorkScale {
        self.scale_proposal
    }

    /// Returns the deduplicated, sorted input sources.
    pub fn input_sources(&self) -> &[InputSource] {
        &self.input_sources
    }

    /// Returns the facts behind the proposal.
    pub fn facts(&self) -> &[AssessmentFact] {
        &self.facts
    }

    /// Returns the declared uncertainties.
    pub fn uncertainties(&self) -> &[BoundedText<1024>] {
        &self.uncertainties
    }

    /// Returns the questions that need a user answer.
    pub fn user_questions(&self) -> &[BoundedText<1024>] {
        &self.user_questions
    }
}

/// Per-source tracking for one requirement item.
///
/// `ae-sdd-daemon-design.md` §6.1 fixes *different* mandatory tracking per
/// source, so this is a sum type rather than one struct with optional fields:
/// oral needs its turn/session and confirmation state, a prototype needs an
/// artifact ref plus observed behaviour, a PRD needs `DocumentId`, path and
/// content digest. §6.1 also forbids passing a merged conclusion off as one
/// source's original text.
/// Tagged by `kind` so each variant keeps its own evidence shape on the wire.
/// An untagged encoding would let a `Prd` payload missing its digest fall
/// through to another variant, which is precisely the silent source-substitution
/// §6.1 forbids. `deny_unknown_fields` makes a `telepathy` source fail closed
/// rather than degrade to the first variant that happens to fit.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RequirementSourceRef {
    /// User narration, bound to the turn and session that carried it.
    Oral {
        /// Session the narration arrived on.
        #[serde(with = "serde_domain::session_id")]
        session_id: SessionId,
        /// The turn the narration arrived on.
        ///
        /// §6.1 line 302 requires an oral source cite its *original turn/session*
        /// reference, not the session alone: one session carries many turns, so a
        /// session-only citation cannot locate what the user actually said, and a
        /// later turn revising the requirement becomes indistinguishable from the
        /// turn that first stated it.
        #[serde(with = "serde_domain::turn_id")]
        turn_id: TurnId,
        /// Structured summary of what was said.
        summary: BoundedText<4096>,
        /// Whether the user has confirmed this reading.
        confirmed: bool,
    },
    /// A prototype or demo observation.
    Prototype {
        /// Immutable reference to the artifact observed.
        #[serde(with = "serde_domain::artifact_ref")]
        artifact: ArtifactRef,
        /// The behaviour actually observed, not the behaviour inferred.
        observed_behaviour: BoundedText<4096>,
    },
    /// A PRD extract.
    Prd {
        /// Stable logical document identity.
        document_id: DocumentId,
        /// Where the document was read from.
        ///
        /// §6.1 line 304 lists 路径 among the four facts a PRD source must keep,
        /// alongside the id, digest and version. It is not redundant with
        /// `document_id`: §4.1 line 131 makes the id survive a path change, so the
        /// id alone cannot say where this reading came from, and a reviewer cannot
        /// re-open the source without it.
        #[serde(with = "serde_domain::project_relative_path")]
        path: ProjectRelativePath,
        /// Content digest of the version read.
        #[serde(with = "serde_domain::artifact_digest")]
        content_digest: ArtifactDigest,
        /// 1-based content version of the document read.
        ///
        /// §4.1 line 132 defines `DocumentVersionId` as `DocumentId + contentDigest
        /// + version`. With only the first two, this citation cannot name a
        /// `DocumentVersionId`, so the §7.1 line 410 rule that a content change
        /// produces a new `DocumentVersionId/contentDigest` while staying under the
        /// same `documentId` cannot be checked from the citation alone.
        ///
        /// Held as the raw `u32` the triple's third slot takes rather than as a
        /// [`DocumentVersionId`], because that type carries no `Serialize`/
        /// `Deserialize` (`document.rs:26`). [`Self::document_version`] derives the
        /// identity on demand, so the wire form stays plain fields while callers
        /// still get the frozen type.
        version: u32,
        /// The rule extracted from that version.
        extracted_rule: BoundedText<4096>,
    },
}

impl RequirementSourceRef {
    /// Returns which source this reference came from.
    pub const fn source(&self) -> InputSource {
        match self {
            Self::Oral { .. } => InputSource::Oral,
            Self::Prototype { .. } => InputSource::Prototype,
            Self::Prd { .. } => InputSource::Prd,
        }
    }

    /// Whether this source proves a backend rule on its own.
    ///
    /// §6.2 rule 6: a prototype proves observable behaviour only, so it never
    /// establishes a backend rule by itself.
    pub const fn proves_backend_rule(&self) -> bool {
        matches!(self, Self::Prd { .. })
    }

    /// Whether this source proves the *existing implementation*.
    ///
    /// §6.2 rule 6: a PRD declares design intent, so it never proves what the
    /// code currently does.
    pub const fn proves_existing_implementation(&self) -> bool {
        matches!(self, Self::Prototype { .. })
    }

    /// The document version a PRD citation names, or `None` for other sources.
    ///
    /// This is what the `version` field is for: assembling the §4.1 line 132
    /// triple that the id and digest alone cannot form. Fails only on a zero
    /// version, which [`DocumentVersionId::derive`] rejects as "not a content
    /// version" — surfaced rather than silently defaulted to 1, since a zero here
    /// means the citation never recorded a real version.
    pub fn document_version(&self) -> Option<Result<DocumentVersionId, DocumentVersionError>> {
        match self {
            Self::Prd {
                document_id,
                content_digest,
                version,
                ..
            } => Some(DocumentVersionId::derive(
                document_id.clone(),
                *content_digest,
                *version,
            )),
            Self::Oral { .. } | Self::Prototype { .. } => None,
        }
    }
}

/// The dimension a requirement conflict falls on.
///
/// §6.2 rule 4 names the dimensions that force `awaiting_user` and forbid
/// routing from continuing. Anything outside that set is still recorded, but it
/// does not by itself block the route.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictDimension {
    /// Conflicting scope claims.
    Scope,
    /// Conflicting acceptance criteria.
    Acceptance,
    /// Conflicting data or model claims.
    Data,
    /// Conflicting security expectations.
    Security,
    /// Conflicting route or scale implications.
    Route,
    /// Any other conflict, recorded but not route-blocking on its own.
    Other,
}

impl ConflictDimension {
    /// Whether a conflict on this dimension must stop routing.
    ///
    /// §6.2 rule 4: scope, acceptance, data, security and route conflicts send
    /// the FlowSupervisor to `awaiting_user`.
    pub const fn blocks_routing(self) -> bool {
        matches!(
            self,
            Self::Scope | Self::Acceptance | Self::Data | Self::Security | Self::Route
        )
    }

    /// Returns the frozen wire encoding.
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Scope => "scope",
            Self::Acceptance => "acceptance",
            Self::Data => "data",
            Self::Security => "security",
            Self::Route => "route",
            Self::Other => "other",
        }
    }

    /// Parses the frozen wire encoding.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "scope" => Some(Self::Scope),
            "acceptance" => Some(Self::Acceptance),
            "data" => Some(Self::Data),
            "security" => Some(Self::Security),
            "route" => Some(Self::Route),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

/// Rejection reasons for a malformed requirement conflict.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RequirementConflictError {
    /// A conflict needs at least two sources to actually be a conflict.
    #[error("a requirement conflict must cite at least two source references")]
    InsufficientSources,
    /// Sources exceeded the frozen bound.
    #[error("requirement conflict cited too many sources")]
    TooManySources,
}

/// Maximum sources one conflict may cite.
pub const MAX_CONFLICT_SOURCES: usize = 16;

/// A semantic clash between requirement sources.
///
/// §6.2 rule 3 forbids resolving this by precedence, and rule 5 requires the
/// user's ruling to become a new committed fact while the original inputs stay
/// unrewritten. This type therefore carries the competing sources rather than a
/// pre-merged conclusion.
/// Deserialization is routed through [`RequirementConflict::new`] by
/// `try_from` rather than derived directly.
///
/// A plain `derive(Deserialize)` reconstructs private fields without consulting
/// the constructor, so a 1-source conflict would decode successfully even though
/// `new` refuses it — and a conflict with one source cannot be a clash, which is
/// the whole premise of §6.2 rule 3. Routing through the constructor makes the
/// wire and the constructor agree by construction instead of by convention.
/// `review.rs` already uses this idiom for the same reason.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "RequirementConflictWire", into = "RequirementConflictWire")]
pub struct RequirementConflict {
    dimension: ConflictDimension,
    statement: BoundedText<4096>,
    sources: Vec<RequirementSourceRef>,
}

/// Wire form of [`RequirementConflict`], validated on the way in.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequirementConflictWire {
    dimension: ConflictDimension,
    statement: BoundedText<4096>,
    sources: Vec<RequirementSourceRef>,
}

impl From<RequirementConflict> for RequirementConflictWire {
    fn from(value: RequirementConflict) -> Self {
        Self {
            dimension: value.dimension,
            statement: value.statement,
            sources: value.sources,
        }
    }
}

impl TryFrom<RequirementConflictWire> for RequirementConflict {
    type Error = RequirementConflictError;

    fn try_from(value: RequirementConflictWire) -> Result<Self, Self::Error> {
        Self::new(value.dimension, value.statement, value.sources)
    }
}

impl RequirementConflict {
    /// Validates and builds a conflict.
    pub fn new(
        dimension: ConflictDimension,
        statement: BoundedText<4096>,
        sources: Vec<RequirementSourceRef>,
    ) -> Result<Self, RequirementConflictError> {
        if sources.len() < 2 {
            return Err(RequirementConflictError::InsufficientSources);
        }
        if sources.len() > MAX_CONFLICT_SOURCES {
            return Err(RequirementConflictError::TooManySources);
        }
        Ok(Self {
            dimension,
            statement,
            sources,
        })
    }

    /// Returns the conflict dimension.
    pub const fn dimension(&self) -> ConflictDimension {
        self.dimension
    }

    /// Returns the conflict statement.
    pub const fn statement(&self) -> &BoundedText<4096> {
        &self.statement
    }

    /// Returns the competing sources, never a merged conclusion.
    pub fn sources(&self) -> &[RequirementSourceRef] {
        &self.sources
    }

    /// Whether this conflict must stop routing and await the user.
    pub const fn blocks_routing(&self) -> bool {
        self.dimension.blocks_routing()
    }
}

/// Wire form of [`BootstrapAssessment`], validated and canonicalized on the way in.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BootstrapAssessmentWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::task_kind")]
    task_kind_proposal: TaskKind,
    #[serde(with = "serde_domain::work_scale")]
    scale_proposal: WorkScale,
    #[serde(with = "serde_domain::input_sources")]
    input_sources: Vec<InputSource>,
    facts: Vec<AssessmentFact>,
    uncertainties: Vec<BoundedText<1024>>,
    user_questions: Vec<BoundedText<1024>>,
}

impl From<BootstrapAssessment> for BootstrapAssessmentWire {
    fn from(value: BootstrapAssessment) -> Self {
        Self {
            schema_version: value.schema_version,
            task_kind_proposal: value.task_kind_proposal,
            scale_proposal: value.scale_proposal,
            input_sources: value.input_sources,
            facts: value.facts,
            uncertainties: value.uncertainties,
            user_questions: value.user_questions,
        }
    }
}

impl TryFrom<BootstrapAssessmentWire> for BootstrapAssessment {
    type Error = BootstrapAssessmentError;

    fn try_from(value: BootstrapAssessmentWire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.task_kind_proposal,
            value.scale_proposal,
            value.input_sources,
            value.facts,
            value.uncertainties,
            value.user_questions,
        )
    }
}
