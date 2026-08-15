//! The execution flow tree of `ae-sdd-daemon-design.md` §4.2.
//!
//! §4.2 requires three relation families be kept separate — execution flow,
//! Spec relations and the Agent delegation tree — and §1.2 goal 4 states why:
//! each must be independently queryable and auditable. Line 767 adds the
//! sharper constraint: they use independent IDs and 不会因 Series 重试互相污染.
//!
//! This module owns the execution family only. Spec relations are D-04's Document
//! registry, and the delegation tree is already durable as `delegation/v1`.
//!
//! The shape is fixed by §4.2's diagram: `Work Item -> Flow Run -> Series Run`,
//! with Series Runs nesting for the Story subchain
//! (`Story -> TestCase -> CodingPlan`). §4.2 rule 1 states what the tree must
//! answer — "当前任务运行到哪个主节点、哪个 Series 和哪个子节点" — which is why a
//! run carries its main node and sub node rather than leaving position implicit.

use ae_sdd_domain::{EpochMillis, FlowRunId, SeriesRunId, StateRevision, WorkItemId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    MAIN_NODE_SERIES_KINDS, SchemaVersion, SeriesId, SeriesKind, SeriesLifecycleState,
    SeriesSubNode, serde_domain,
};

/// Why a run projection could not be built.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RunGraphError {
    /// `main_node` was well-formed but is not a frozen main node.
    #[error("{value} is not a frozen main node")]
    MainNodeNotFrozen {
        /// The rejected value.
        value: String,
    },
    /// A Series Run named itself as the attempt it replaces.
    #[error("a Series Run cannot be its own retry predecessor")]
    SelfRetry,
    /// A Series Run named itself as its own parent.
    #[error("a Series Run cannot be its own parent")]
    SelfParent,
    /// A first attempt claimed a retry predecessor, or a retry claimed none.
    #[error("attempt ordinal {ordinal} disagrees with the presence of retryOf")]
    RetryOrdinalMismatch {
        /// The attempt ordinal that disagreed.
        ordinal: u32,
    },
}

/// One run of the main flow for a Work Item (§4.2 `WI -> FR`).
///
/// Keyed by [`FlowRunId`] rather than [`WorkItemId`] because §4.1 makes a Work
/// Item stable business identity while a Flow Run is one *instance* of running
/// it: 重试不复用，恢复同一运行时保持不变. A projection keyed by Work Item could
/// therefore hold only the newest run, which is exactly the pollution line 767
/// forbids.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "FlowRunProjectionWire", into = "FlowRunProjectionWire")]
pub struct FlowRunProjection {
    schema_version: SchemaVersion,
    flow_run_id: FlowRunId,
    work_item_id: WorkItemId,
    current_main_node: SeriesKind,
    state_revision: StateRevision,
    started_at: EpochMillis,
    updated_at: EpochMillis,
}

impl FlowRunProjection {
    /// Records a Flow Run at its current main node.
    ///
    /// `current_main_node` is checked against [`MAIN_NODE_SERIES_KINDS`] for the
    /// same reason [`crate::InstructionEnvelope`] checks it: §11.1 makes
    /// `currentMainNode` and the envelope's `mainNode` draw from one frozen list,
    /// so a projection accepting `story-generate` would let the two disagree
    /// about where the flow is.
    pub fn open(
        flow_run_id: FlowRunId,
        work_item_id: WorkItemId,
        current_main_node: SeriesKind,
        state_revision: StateRevision,
        started_at: EpochMillis,
        updated_at: EpochMillis,
    ) -> Result<Self, RunGraphError> {
        if !MAIN_NODE_SERIES_KINDS.contains(&current_main_node.as_str()) {
            return Err(RunGraphError::MainNodeNotFrozen {
                value: current_main_node.to_string(),
            });
        }
        Ok(Self {
            schema_version: SchemaVersion::V1,
            flow_run_id,
            work_item_id,
            current_main_node,
            state_revision,
            started_at,
            updated_at,
        })
    }

    /// Returns the contract schema version.
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns this run's identity.
    pub const fn flow_run_id(&self) -> FlowRunId {
        self.flow_run_id
    }

    /// Returns the stable Work Item this run executes.
    pub const fn work_item_id(&self) -> &WorkItemId {
        &self.work_item_id
    }

    /// Returns the current main node (§4.2 rule 1).
    pub const fn current_main_node(&self) -> &SeriesKind {
        &self.current_main_node
    }

    /// Returns the revision this projection was written at.
    pub const fn state_revision(&self) -> StateRevision {
        self.state_revision
    }

    /// Returns when the run started.
    pub const fn started_at(&self) -> EpochMillis {
        self.started_at
    }

    /// Returns when the projection was last advanced.
    pub const fn updated_at(&self) -> EpochMillis {
        self.updated_at
    }
}

/// Wire form of [`FlowRunProjection`], validated on the way in.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FlowRunProjectionWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::flow_run_id")]
    flow_run_id: FlowRunId,
    #[serde(with = "serde_domain::work_item_id")]
    work_item_id: WorkItemId,
    current_main_node: SeriesKind,
    #[serde(with = "serde_domain::state_revision")]
    state_revision: StateRevision,
    #[serde(with = "serde_domain::epoch_millis")]
    started_at: EpochMillis,
    #[serde(with = "serde_domain::epoch_millis")]
    updated_at: EpochMillis,
}

impl From<FlowRunProjection> for FlowRunProjectionWire {
    fn from(value: FlowRunProjection) -> Self {
        Self {
            schema_version: value.schema_version,
            flow_run_id: value.flow_run_id,
            work_item_id: value.work_item_id,
            current_main_node: value.current_main_node,
            state_revision: value.state_revision,
            started_at: value.started_at,
            updated_at: value.updated_at,
        }
    }
}

impl TryFrom<FlowRunProjectionWire> for FlowRunProjection {
    type Error = RunGraphError;

    fn try_from(value: FlowRunProjectionWire) -> Result<Self, Self::Error> {
        Self::open(
            value.flow_run_id,
            value.work_item_id,
            value.current_main_node,
            value.state_revision,
            value.started_at,
            value.updated_at,
        )
    }
}

/// One physical attempt at a logical Series (§4.2 `FR -> Series Run`).
///
/// Keyed by [`SeriesRunId`], not [`SeriesId`]. §4.1 fixes the distinction:
/// 重试产生新 run，并关联同一 `SeriesId`. Keying by the logical Series would make
/// each retry overwrite the previous attempt, which is the failure the existing
/// `series_plan_projection` table still has — its primary key is
/// `(workspace_id, series_id)`, so two attempts collapse into one row.
///
/// `parent_series_run_id` exists because §4.2's diagram nests Series Runs rather
/// than flattening them: `ST --> TC --> CP` puts a Story's TestCase and
/// CodingPlan runs under that Story's run. Without the parent edge, a Work Item
/// with three Stories has three indistinguishable TestCase runs hanging off the
/// Flow Run, and §9.2's rule that a CodingPlan bind 同一 Story 的已批准 TestCase
/// becomes unverifiable from the projection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "SeriesRunProjectionWire", into = "SeriesRunProjectionWire")]
pub struct SeriesRunProjection {
    schema_version: SchemaVersion,
    series_run_id: SeriesRunId,
    series_id: SeriesId,
    flow_run_id: FlowRunId,
    parent_series_run_id: Option<SeriesRunId>,
    main_node: SeriesKind,
    sub_node: SeriesSubNode,
    lifecycle_state: SeriesLifecycleState,
    attempt_ordinal: u32,
    retry_of: Option<SeriesRunId>,
    state_revision: StateRevision,
    updated_at: EpochMillis,
}

impl SeriesRunProjection {
    /// Records one attempt, rejecting a self-referential or inconsistent chain.
    ///
    /// Three guards, each pinning a fact the projection would otherwise be able
    /// to contradict:
    ///
    /// A run naming itself in `retry_of` or `parent_series_run_id` would make the
    /// execution tree cyclic, so "all attempts of this Series" and "the runs under
    /// this Story" become non-terminating walks.
    ///
    /// `attempt_ordinal` and `retry_of` must agree. Ordinal 1 is a first attempt
    /// and has nothing to replace; any later ordinal replaces something. §4.1
    /// requires a retry keep the same `SeriesId` while minting a new run, so an
    /// ordinal-3 run with no predecessor would claim to be a retry of nothing and
    /// break the chain F-06 requires to stay walkable.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        series_run_id: SeriesRunId,
        series_id: SeriesId,
        flow_run_id: FlowRunId,
        parent_series_run_id: Option<SeriesRunId>,
        main_node: SeriesKind,
        sub_node: SeriesSubNode,
        lifecycle_state: SeriesLifecycleState,
        attempt_ordinal: u32,
        retry_of: Option<SeriesRunId>,
        state_revision: StateRevision,
        updated_at: EpochMillis,
    ) -> Result<Self, RunGraphError> {
        if !MAIN_NODE_SERIES_KINDS.contains(&main_node.as_str()) {
            return Err(RunGraphError::MainNodeNotFrozen {
                value: main_node.to_string(),
            });
        }
        if retry_of == Some(series_run_id) {
            return Err(RunGraphError::SelfRetry);
        }
        if parent_series_run_id == Some(series_run_id) {
            return Err(RunGraphError::SelfParent);
        }
        let first_attempt = attempt_ordinal <= 1;
        if first_attempt != retry_of.is_none() {
            return Err(RunGraphError::RetryOrdinalMismatch {
                ordinal: attempt_ordinal,
            });
        }
        Ok(Self {
            schema_version: SchemaVersion::V1,
            series_run_id,
            series_id,
            flow_run_id,
            parent_series_run_id,
            main_node,
            sub_node,
            lifecycle_state,
            attempt_ordinal,
            retry_of,
            state_revision,
            updated_at,
        })
    }

    /// Returns the contract schema version.
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns this attempt's identity.
    pub const fn series_run_id(&self) -> SeriesRunId {
        self.series_run_id
    }

    /// Returns the logical Series every attempt of it shares.
    pub const fn series_id(&self) -> &SeriesId {
        &self.series_id
    }

    /// Returns the Flow Run this attempt belongs to.
    pub const fn flow_run_id(&self) -> FlowRunId {
        self.flow_run_id
    }

    /// Returns the enclosing Series Run for a Story subchain, if any.
    pub const fn parent_series_run_id(&self) -> Option<SeriesRunId> {
        self.parent_series_run_id
    }

    /// Returns the main node this attempt serves.
    pub const fn main_node(&self) -> &SeriesKind {
        &self.main_node
    }

    /// Returns the position inside the Series (§4.2 rule 1).
    pub const fn sub_node(&self) -> SeriesSubNode {
        self.sub_node
    }

    /// Returns the conceptual lifecycle state (§11.2).
    pub const fn lifecycle_state(&self) -> SeriesLifecycleState {
        self.lifecycle_state
    }

    /// Returns which attempt this is, counting from 1.
    pub const fn attempt_ordinal(&self) -> u32 {
        self.attempt_ordinal
    }

    /// Returns the attempt this one replaces, absent on a first attempt.
    pub const fn retry_of(&self) -> Option<SeriesRunId> {
        self.retry_of
    }

    /// Returns the revision this projection was written at.
    pub const fn state_revision(&self) -> StateRevision {
        self.state_revision
    }

    /// Returns when the projection was last advanced.
    pub const fn updated_at(&self) -> EpochMillis {
        self.updated_at
    }
}

/// Wire form of [`SeriesRunProjection`], validated on the way in.
///
/// `parentSeriesRunId` and `retryOf` are omitted when absent rather than encoded
/// as null, because a first attempt has no predecessor at all — a present-but-null
/// field invites a reader to treat "no retry" and "unknown retry" as the same
/// thing, and D-03 forbids reading missing data as an empty rebuild.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SeriesRunProjectionWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::series_run_id")]
    series_run_id: SeriesRunId,
    series_id: SeriesId,
    #[serde(with = "serde_domain::flow_run_id")]
    flow_run_id: FlowRunId,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_domain::optional_series_run_id"
    )]
    parent_series_run_id: Option<SeriesRunId>,
    main_node: SeriesKind,
    sub_node: SeriesSubNode,
    lifecycle_state: SeriesLifecycleState,
    attempt_ordinal: u32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_domain::optional_series_run_id"
    )]
    retry_of: Option<SeriesRunId>,
    #[serde(with = "serde_domain::state_revision")]
    state_revision: StateRevision,
    #[serde(with = "serde_domain::epoch_millis")]
    updated_at: EpochMillis,
}

impl From<SeriesRunProjection> for SeriesRunProjectionWire {
    fn from(value: SeriesRunProjection) -> Self {
        Self {
            schema_version: value.schema_version,
            series_run_id: value.series_run_id,
            series_id: value.series_id,
            flow_run_id: value.flow_run_id,
            parent_series_run_id: value.parent_series_run_id,
            main_node: value.main_node,
            sub_node: value.sub_node,
            lifecycle_state: value.lifecycle_state,
            attempt_ordinal: value.attempt_ordinal,
            retry_of: value.retry_of,
            state_revision: value.state_revision,
            updated_at: value.updated_at,
        }
    }
}

impl TryFrom<SeriesRunProjectionWire> for SeriesRunProjection {
    type Error = RunGraphError;

    fn try_from(value: SeriesRunProjectionWire) -> Result<Self, Self::Error> {
        Self::record(
            value.series_run_id,
            value.series_id,
            value.flow_run_id,
            value.parent_series_run_id,
            value.main_node,
            value.sub_node,
            value.lifecycle_state,
            value.attempt_ordinal,
            value.retry_of,
            value.state_revision,
            value.updated_at,
        )
    }
}
