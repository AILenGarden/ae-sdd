//! Committed supervision events consumed by FlowSupervisor.
//!
//! `ae-sdd-daemon-design.md` §11.3 requires these events be committed,
//! replayable and globally ordered, and lists the fact categories they must be
//! able to express. Its closing line draws the hard boundary: an event records
//! business facts and references only — never Agent reasoning, a full prompt, a
//! whole document, or source text.
//!
//! That boundary is enforced structurally here rather than by review. Every
//! field on these types is an identifier, an enum, a revision, a digest or a
//! bounded summary, so there is nowhere for a transcript to be stored even by a
//! caller who wants to.

use ae_sdd_domain::{EpochMillis, EventSequence, SeriesRunId, StateRevision};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    BoundedText, ConflictDimension, RequirementSourceRef, SchemaVersion, SeriesId, SeriesSubNode,
    serde_domain,
};

/// Why a supervision event could not be built.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SupervisionEventError {
    /// A ruling adopted a source it also rejected.
    #[error("a ruling cannot both adopt and reject the same source")]
    ChosenSourceAlsoRejected,
    /// A ruling rejected nothing, so there was no clash to rule on.
    #[error("a ruling must record the branch it rejected")]
    NoRejectedSources,
    /// The rationale summary was empty.
    #[error("a ruling must record why it was decided")]
    EmptyRationale,
}

/// One committed Series progress fact (§11.3, "Series ... progress ...").
///
/// Carries [`EventSequence`] because §11.3 requires global ordering: a replay
/// that only knows per-Series order cannot reconstruct how two Series
/// interleaved, which is what supervising a parallel Story branch needs.
///
/// `sub_node` is a [`SeriesSubNode`] rather than free text so the event cannot
/// describe a position outside the frozen vocabulary. §11.1 also fixes who may
/// move it: a sub-node advances on a valid Series event and still requires
/// daemon validation, so this type records an *observation*, never an authority
/// to advance.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SeriesProgressEvent {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::event_sequence")]
    sequence: EventSequence,
    series_id: SeriesId,
    /// The physical attempt this observation came from.
    ///
    /// §9.1 line 452 requires a Series transaction define `seriesId/seriesRunId/
    /// workItemId`, and §4.1 makes a retry a *new* `SeriesRunId` under the same
    /// `SeriesId`. With only `series_id`, two attempts of one Series emitted
    /// indistinguishable progress: a replay could not tell a retry's advance from
    /// the failed attempt's, which is precisely what §11.4's stale-result marking
    /// needs to distinguish.
    #[serde(with = "serde_domain::series_run_id")]
    series_run_id: SeriesRunId,
    sub_node: SeriesSubNode,
    #[serde(with = "serde_domain::state_revision")]
    state_revision: StateRevision,
    #[serde(with = "serde_domain::epoch_millis")]
    observed_at: EpochMillis,
}

impl SeriesProgressEvent {
    /// Records a Series reaching `sub_node` at `state_revision`.
    ///
    /// Infallible: every field is already constrained by its own type, and there
    /// is no cross-field invariant to check. A progress observation that cannot
    /// be recorded would be worse than one recorded and later superseded — §11.4
    /// marks stale results rather than dropping the events that produced them.
    pub const fn observe(
        sequence: EventSequence,
        series_id: SeriesId,
        series_run_id: SeriesRunId,
        sub_node: SeriesSubNode,
        state_revision: StateRevision,
        observed_at: EpochMillis,
    ) -> Self {
        Self {
            schema_version: SchemaVersion::V1,
            sequence,
            series_id,
            series_run_id,
            sub_node,
            state_revision,
            observed_at,
        }
    }

    /// Returns the physical attempt this observation belongs to.
    pub const fn series_run_id(&self) -> &SeriesRunId {
        &self.series_run_id
    }

    /// Returns the contract schema version.
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the global order position.
    pub const fn sequence(&self) -> EventSequence {
        self.sequence
    }

    /// Returns the Series this progress belongs to.
    pub const fn series_id(&self) -> &SeriesId {
        &self.series_id
    }

    /// Returns the observed position inside the Series.
    pub const fn sub_node(&self) -> SeriesSubNode {
        self.sub_node
    }

    /// Returns the revision this observation was made against.
    pub const fn state_revision(&self) -> StateRevision {
        self.state_revision
    }

    /// Returns when the observation was made.
    pub const fn observed_at(&self) -> EpochMillis {
        self.observed_at
    }
}

/// A user's ruling on a requirement conflict (§6.2 rule 5, §11.3 "输入来源
/// 注册、冲突发现和用户裁决").
///
/// Three clauses of §6.2 rule 5 shape this type, and each maps to a structural
/// decision rather than a convention:
///
/// The ruling forms a *new* committed fact, so this event references the
/// competing sources and never carries a mutable handle to the original input.
/// There is deliberately no field here through which an original source could be
/// edited — §6.2 rule 5's "不改写原始输入" is unforgeable rather than advised.
///
/// The rejected branch is *retained*, so `rejected` is stored rather than
/// inferred as the complement of `chosen`. A reader six months later needs to see
/// what was turned down, not reconstruct it from a source list that may since
/// have grown.
///
/// The rationale is a *summary*, which is also the most §11.3's closing line
/// permits: a bounded reason, never the deliberation that produced it.
///
/// `dimension` is carried because [`crate::RequirementConflict`] has no identity
/// of its own — it is `dimension` + `statement` + `sources`. Naming the dimension
/// is what lets a supervisor tell a security ruling from a scope ruling without
/// re-reading the conflict, and §6.2 rule 4 keys route-blocking on exactly that
/// dimension.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "RequirementRulingWire", into = "RequirementRulingWire")]
pub struct RequirementRulingEvent {
    schema_version: SchemaVersion,
    sequence: EventSequence,
    dimension: ConflictDimension,
    chosen: RequirementSourceRef,
    rejected: Vec<RequirementSourceRef>,
    rationale_summary: BoundedText<1024>,
    state_revision: StateRevision,
    decided_at: EpochMillis,
}

impl RequirementRulingEvent {
    /// Records a user ruling, refusing one that decides nothing.
    ///
    /// A ruling whose `chosen` also appears in `rejected` is self-contradictory,
    /// and one that rejects nothing had no clash to resolve. Both matter because
    /// §6.2 rule 4 holds the flow in `awaiting_user` until a real ruling arrives:
    /// accepting either shape would release that hold on a fact that settles
    /// nothing, letting routing continue past an unresolved conflict.
    pub fn decide(
        sequence: EventSequence,
        dimension: ConflictDimension,
        chosen: RequirementSourceRef,
        rejected: Vec<RequirementSourceRef>,
        rationale_summary: BoundedText<1024>,
        state_revision: StateRevision,
        decided_at: EpochMillis,
    ) -> Result<Self, SupervisionEventError> {
        if rejected.is_empty() {
            return Err(SupervisionEventError::NoRejectedSources);
        }
        if rejected.contains(&chosen) {
            return Err(SupervisionEventError::ChosenSourceAlsoRejected);
        }
        if rationale_summary.as_str().trim().is_empty() {
            return Err(SupervisionEventError::EmptyRationale);
        }
        Ok(Self {
            schema_version: SchemaVersion::V1,
            sequence,
            dimension,
            chosen,
            rejected,
            rationale_summary,
            state_revision,
            decided_at,
        })
    }

    /// Returns the contract schema version.
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the global order position.
    pub const fn sequence(&self) -> EventSequence {
        self.sequence
    }

    /// Returns the dimension the conflict fell on.
    pub const fn dimension(&self) -> ConflictDimension {
        self.dimension
    }

    /// Returns the adopted source.
    pub const fn chosen(&self) -> &RequirementSourceRef {
        &self.chosen
    }

    /// Returns the rejected branch, retained per §6.2 rule 5.
    pub fn rejected(&self) -> &[RequirementSourceRef] {
        &self.rejected
    }

    /// Returns the bounded reason for the ruling.
    pub const fn rationale_summary(&self) -> &BoundedText<1024> {
        &self.rationale_summary
    }

    /// Returns the revision this ruling was made against.
    pub const fn state_revision(&self) -> StateRevision {
        self.state_revision
    }

    /// Returns when the ruling was made.
    pub const fn decided_at(&self) -> EpochMillis {
        self.decided_at
    }
}

/// Wire form of [`RequirementRulingEvent`], validated on the way in.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequirementRulingWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::event_sequence")]
    sequence: EventSequence,
    dimension: ConflictDimension,
    chosen: RequirementSourceRef,
    rejected: Vec<RequirementSourceRef>,
    rationale_summary: BoundedText<1024>,
    #[serde(with = "serde_domain::state_revision")]
    state_revision: StateRevision,
    #[serde(with = "serde_domain::epoch_millis")]
    decided_at: EpochMillis,
}

impl From<RequirementRulingEvent> for RequirementRulingWire {
    fn from(value: RequirementRulingEvent) -> Self {
        Self {
            schema_version: value.schema_version,
            sequence: value.sequence,
            dimension: value.dimension,
            chosen: value.chosen,
            rejected: value.rejected,
            rationale_summary: value.rationale_summary,
            state_revision: value.state_revision,
            decided_at: value.decided_at,
        }
    }
}

impl TryFrom<RequirementRulingWire> for RequirementRulingEvent {
    type Error = SupervisionEventError;

    fn try_from(value: RequirementRulingWire) -> Result<Self, Self::Error> {
        Self::decide(
            value.sequence,
            value.dimension,
            value.chosen,
            value.rejected,
            value.rationale_summary,
            value.state_revision,
            value.decided_at,
        )
    }
}
