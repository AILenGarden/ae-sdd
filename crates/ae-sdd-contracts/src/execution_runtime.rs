//! Frozen execution-capsule and execution-slice contracts.
//!
//! An [`ExecutionCapsuleV1`] is the bounded recovery unit the daemon derives
//! from an approved `state.executionPlan`.  Constructors canonicalize every
//! collection (stable sort plus deduplication) so identical semantic input
//! produces byte-identical encodings, and they fail closed on empty
//! identities, out-of-scope source reads, broken ordinal contiguity or
//! budgets above the frozen v1 hard limits.

use ae_sdd_domain::{
    ArtifactDigest, ArtifactRef, ExecutionSliceId, InventoryGeneration, PolicyDigest,
    ProjectRelativePath, StateRevision, StoryId, VerificationId, WorkItemId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{SchemaVersion, serde_domain};

/// Hard encoded-size limit for one execution capsule (16 KiB); exceeding it fails closed.
pub const MAX_CAPSULE_BYTES: u32 = 16 * 1024;
/// Default retained-output budget for one tool call (64 KiB).
pub const DEFAULT_MAX_TOOL_OUTPUT_BYTES: u32 = 64 * 1024;
/// Default source bytes one inspection batch may return (24 KiB).
pub const DEFAULT_MAX_SOURCE_READ_BYTES_PER_BATCH: u32 = 24 * 1024;
/// Default source-file count one inspection batch may touch.
pub const DEFAULT_MAX_SOURCE_FILES_PER_BATCH: u16 = 12;
/// Default tool calls closed by one inspection batch.
pub const DEFAULT_INSPECTION_CALLS_PER_BATCH: u8 = 4;
/// Default consecutive no-progress batches before investigation stops.
pub const DEFAULT_MAX_NO_PROGRESS_BATCHES: u8 = 3;
/// Default authority refreshes allowed per resume.
pub const DEFAULT_MAX_AUTHORITY_REFRESHES_PER_RESUME: u8 = 1;
/// Maximum slice dependencies per slice.
pub const MAX_SLICE_DEPENDENCIES: usize = 32;
/// Maximum path-scope entries per slice.
pub const MAX_SLICE_PATH_SCOPE: usize = 32;
/// Maximum source-read specs per slice.
pub const MAX_SLICE_SOURCE_READS: usize = 32;
/// Maximum broad verification bindings per slice.
pub const MAX_SLICE_BROAD_VERIFICATIONS: usize = 16;
/// Maximum slice objective length in bytes.
pub const MAX_OBJECTIVE_BYTES: usize = 4096;
/// Maximum evidence logical-key length in bytes.
pub const MAX_EVIDENCE_LOGICAL_KEY_BYTES: usize = 512;

/// Validation errors for execution capsules, slices, queues and budgets.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ExecutionCapsuleError {
    /// A collection exceeded its frozen v1 limit.
    #[error("execution collection exceeds its frozen v1 limit")]
    CollectionLimitExceeded,
    /// The slice objective was empty or exceeded its byte limit.
    #[error("execution slice objective must be non-empty and within its byte limit")]
    InvalidObjective,
    /// The evidence logical key was empty or exceeded its byte limit.
    #[error("execution evidence logical key must be non-empty and within its byte limit")]
    InvalidEvidenceLogicalKey,
    /// A slice ordinal was not a positive 1-based position.
    #[error("execution slice ordinal must be a positive 1-based position")]
    InvalidOrdinal,
    /// A source-read line range was zero-based, reversed or half-open.
    #[error("source read line range must be 1-based, complete and non-decreasing")]
    InvalidLineRange,
    /// A slice declared no path scope.
    #[error("execution slice must declare at least one path scope entry")]
    EmptyPathScope,
    /// A source read escaped the declared slice path scope.
    #[error("source read path escapes the declared slice path scope")]
    SourceReadOutOfScope,
    /// The queue declared no slices.
    #[error("execution queue must contain at least one slice")]
    EmptyQueue,
    /// Queue progress counters did not form a contiguous executed prefix.
    #[error("queue active ordinal must equal completed slices plus one within total slices")]
    NonContiguousOrdinal,
    /// The active slice ordinal did not match the queue cursor.
    #[error("active slice ordinal does not match the queue active ordinal")]
    ActiveOrdinalMismatch,
    /// A budget was zero or exceeded its frozen v1 hard limit.
    #[error("execution budget must be non-zero and within its frozen v1 hard limit")]
    InvalidBudget,
    /// An encoded capsule exceeded the configured capsule byte budget.
    #[error(
        "encoded execution capsule exceeds the {max_bytes}-byte budget (actual: {actual_bytes})"
    )]
    CapsuleBudgetExceeded {
        /// Configured capsule byte budget.
        max_bytes: u32,
        /// Observed encoded byte length.
        actual_bytes: usize,
    },
}

/// Machine lifecycle of one execution slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionSliceStatus {
    /// Approved but not started.
    Pending,
    /// Claimed by the supervisor.
    Running,
    /// Focused RED observed.
    RedObserved,
    /// Minimal patch applied.
    Patched,
    /// Focused verification is green.
    FocusedGreen,
    /// Evidence appended and bound to the slice.
    EvidenceBound,
    /// Slice closed.
    Completed,
    /// Blocked on an external decision.
    Blocked,
}

/// Stable classification of execution-surface tool calls.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionToolClass {
    /// Reading project source.
    SourceRead,
    /// Searching the workspace.
    Search,
    /// Applying a patch.
    Patch,
    /// Running focused verification.
    FocusedTest,
    /// Running broad verification.
    BroadTest,
    /// Appending evidence.
    Evidence,
    /// Anything not classified above.
    Other,
}

/// Bounded source-read intent: one project-relative path plus an optional
/// inclusive 1-based line range.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "SourceReadSpecV1Wire", into = "SourceReadSpecV1Wire")]
pub struct SourceReadSpecV1 {
    path: ProjectRelativePath,
    start_line: Option<u32>,
    end_line: Option<u32>,
}

impl SourceReadSpecV1 {
    /// Constructs a read spec; the line range must be complete or absent.
    pub fn new(
        path: ProjectRelativePath,
        start_line: Option<u32>,
        end_line: Option<u32>,
    ) -> Result<Self, ExecutionCapsuleError> {
        match (start_line, end_line) {
            (None, None) => {}
            (Some(start), Some(end)) if start >= 1 && end >= start => {}
            _ => return Err(ExecutionCapsuleError::InvalidLineRange),
        }
        Ok(Self {
            path,
            start_line,
            end_line,
        })
    }

    /// Returns the project-relative source path.
    pub const fn path(&self) -> &ProjectRelativePath {
        &self.path
    }

    /// Returns the inclusive 1-based first line, when ranged.
    pub const fn start_line(&self) -> Option<u32> {
        self.start_line
    }

    /// Returns the inclusive 1-based last line, when ranged.
    pub const fn end_line(&self) -> Option<u32> {
        self.end_line
    }
}

impl<'de> Deserialize<'de> for SourceReadSpecV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        SourceReadSpecV1Wire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceReadSpecV1Wire {
    #[serde(with = "serde_domain::project_relative_path")]
    path: ProjectRelativePath,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    start_line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end_line: Option<u32>,
}

impl TryFrom<SourceReadSpecV1Wire> for SourceReadSpecV1 {
    type Error = ExecutionCapsuleError;

    fn try_from(value: SourceReadSpecV1Wire) -> Result<Self, Self::Error> {
        Self::new(value.path, value.start_line, value.end_line)
    }
}

impl From<SourceReadSpecV1> for SourceReadSpecV1Wire {
    fn from(value: SourceReadSpecV1) -> Self {
        Self {
            path: value.path,
            start_line: value.start_line,
            end_line: value.end_line,
        }
    }
}

/// One machine-executable slice of an approved execution plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "ExecutionSliceV1Wire", into = "ExecutionSliceV1Wire")]
pub struct ExecutionSliceV1 {
    slice_id: ExecutionSliceId,
    ordinal: u32,
    objective: Box<str>,
    depends_on: Vec<ExecutionSliceId>,
    path_scope: Vec<ProjectRelativePath>,
    source_reads: Vec<SourceReadSpecV1>,
    focused_verification_id: VerificationId,
    broad_verification_ids: Vec<VerificationId>,
    evidence_logical_key: Box<str>,
}

impl ExecutionSliceV1 {
    /// Constructs a validated slice; collections are canonically sorted and deduplicated.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        slice_id: ExecutionSliceId,
        ordinal: u32,
        objective: impl Into<Box<str>>,
        depends_on: Vec<ExecutionSliceId>,
        path_scope: Vec<ProjectRelativePath>,
        source_reads: Vec<SourceReadSpecV1>,
        focused_verification_id: VerificationId,
        broad_verification_ids: Vec<VerificationId>,
        evidence_logical_key: impl Into<Box<str>>,
    ) -> Result<Self, ExecutionCapsuleError> {
        let objective = objective.into();
        if objective.trim().is_empty() || objective.len() > MAX_OBJECTIVE_BYTES {
            return Err(ExecutionCapsuleError::InvalidObjective);
        }
        let evidence_logical_key = evidence_logical_key.into();
        if evidence_logical_key.trim().is_empty()
            || evidence_logical_key.len() > MAX_EVIDENCE_LOGICAL_KEY_BYTES
        {
            return Err(ExecutionCapsuleError::InvalidEvidenceLogicalKey);
        }
        if ordinal == 0 {
            return Err(ExecutionCapsuleError::InvalidOrdinal);
        }
        if path_scope.is_empty() {
            return Err(ExecutionCapsuleError::EmptyPathScope);
        }
        if depends_on.len() > MAX_SLICE_DEPENDENCIES
            || path_scope.len() > MAX_SLICE_PATH_SCOPE
            || source_reads.len() > MAX_SLICE_SOURCE_READS
            || broad_verification_ids.len() > MAX_SLICE_BROAD_VERIFICATIONS
        {
            return Err(ExecutionCapsuleError::CollectionLimitExceeded);
        }
        for read in &source_reads {
            if !path_scope.iter().any(|scope| scope.contains(read.path())) {
                return Err(ExecutionCapsuleError::SourceReadOutOfScope);
            }
        }
        let mut depends_on = depends_on;
        depends_on.sort_unstable();
        depends_on.dedup();
        let mut path_scope = path_scope;
        path_scope.sort_unstable();
        path_scope.dedup();
        let mut source_reads = source_reads;
        source_reads.sort_unstable();
        source_reads.dedup();
        let mut broad_verification_ids = broad_verification_ids;
        broad_verification_ids.sort_unstable();
        broad_verification_ids.dedup();
        Ok(Self {
            slice_id,
            ordinal,
            objective,
            depends_on,
            path_scope,
            source_reads,
            focused_verification_id,
            broad_verification_ids,
            evidence_logical_key,
        })
    }

    /// Returns the slice identity.
    pub const fn slice_id(&self) -> &ExecutionSliceId {
        &self.slice_id
    }

    /// Returns the 1-based position in the queue.
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Returns the bounded slice objective.
    pub fn objective(&self) -> &str {
        &self.objective
    }

    /// Returns the canonically ordered slice dependencies.
    pub fn depends_on(&self) -> &[ExecutionSliceId] {
        &self.depends_on
    }

    /// Returns the canonically ordered writable path scope.
    pub fn path_scope(&self) -> &[ProjectRelativePath] {
        &self.path_scope
    }

    /// Returns the canonically ordered bounded source reads.
    pub fn source_reads(&self) -> &[SourceReadSpecV1] {
        &self.source_reads
    }

    /// Returns the required focused verification binding.
    pub const fn focused_verification_id(&self) -> &VerificationId {
        &self.focused_verification_id
    }

    /// Returns the canonically ordered broad verification bindings.
    pub fn broad_verification_ids(&self) -> &[VerificationId] {
        &self.broad_verification_ids
    }

    /// Returns the evidence logical key binding slice evidence.
    pub fn evidence_logical_key(&self) -> &str {
        &self.evidence_logical_key
    }
}

impl<'de> Deserialize<'de> for ExecutionSliceV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ExecutionSliceV1Wire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExecutionSliceV1Wire {
    #[serde(with = "execution_slice_id")]
    slice_id: ExecutionSliceId,
    ordinal: u32,
    objective: Box<str>,
    #[serde(with = "execution_slice_ids")]
    depends_on: Vec<ExecutionSliceId>,
    #[serde(with = "project_relative_paths")]
    path_scope: Vec<ProjectRelativePath>,
    source_reads: Vec<SourceReadSpecV1>,
    #[serde(with = "serde_domain::verification_id")]
    focused_verification_id: VerificationId,
    #[serde(with = "verification_ids")]
    broad_verification_ids: Vec<VerificationId>,
    evidence_logical_key: Box<str>,
}

impl TryFrom<ExecutionSliceV1Wire> for ExecutionSliceV1 {
    type Error = ExecutionCapsuleError;

    fn try_from(value: ExecutionSliceV1Wire) -> Result<Self, Self::Error> {
        Self::new(
            value.slice_id,
            value.ordinal,
            value.objective,
            value.depends_on,
            value.path_scope,
            value.source_reads,
            value.focused_verification_id,
            value.broad_verification_ids,
            value.evidence_logical_key,
        )
    }
}

impl From<ExecutionSliceV1> for ExecutionSliceV1Wire {
    fn from(value: ExecutionSliceV1) -> Self {
        Self {
            slice_id: value.slice_id,
            ordinal: value.ordinal,
            objective: value.objective,
            depends_on: value.depends_on,
            path_scope: value.path_scope,
            source_reads: value.source_reads,
            focused_verification_id: value.focused_verification_id,
            broad_verification_ids: value.broad_verification_ids,
            evidence_logical_key: value.evidence_logical_key,
        }
    }
}

/// Bounded execution budgets enforced by the supervisor; the capsule byte
/// limit is a frozen hard ceiling that cannot be raised by configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "ExecutionBudgetsV1Wire", into = "ExecutionBudgetsV1Wire")]
pub struct ExecutionBudgetsV1 {
    max_capsule_bytes: u32,
    max_tool_output_bytes: u32,
    max_source_read_bytes_per_batch: u32,
    max_source_files_per_batch: u16,
    inspection_calls_per_batch: u8,
    max_no_progress_batches: u8,
    max_authority_refreshes_per_resume: u8,
}

impl ExecutionBudgetsV1 {
    /// Constructs validated budgets; every value must be non-zero and within its hard limit.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        max_capsule_bytes: u32,
        max_tool_output_bytes: u32,
        max_source_read_bytes_per_batch: u32,
        max_source_files_per_batch: u16,
        inspection_calls_per_batch: u8,
        max_no_progress_batches: u8,
        max_authority_refreshes_per_resume: u8,
    ) -> Result<Self, ExecutionCapsuleError> {
        if max_capsule_bytes == 0
            || max_capsule_bytes > MAX_CAPSULE_BYTES
            || max_tool_output_bytes == 0
            || max_source_read_bytes_per_batch == 0
            || max_source_files_per_batch == 0
            || inspection_calls_per_batch == 0
            || max_no_progress_batches == 0
            || max_authority_refreshes_per_resume == 0
        {
            return Err(ExecutionCapsuleError::InvalidBudget);
        }
        Ok(Self {
            max_capsule_bytes,
            max_tool_output_bytes,
            max_source_read_bytes_per_batch,
            max_source_files_per_batch,
            inspection_calls_per_batch,
            max_no_progress_batches,
            max_authority_refreshes_per_resume,
        })
    }

    /// Fails closed when an encoded capsule exceeds the configured capsule budget.
    pub fn check_capsule_len(&self, encoded_bytes: usize) -> Result<(), ExecutionCapsuleError> {
        if encoded_bytes > self.max_capsule_bytes as usize {
            return Err(ExecutionCapsuleError::CapsuleBudgetExceeded {
                max_bytes: self.max_capsule_bytes,
                actual_bytes: encoded_bytes,
            });
        }
        Ok(())
    }

    /// Returns the capsule encoded-size budget.
    pub const fn max_capsule_bytes(&self) -> u32 {
        self.max_capsule_bytes
    }

    /// Returns the retained-output budget for one tool call.
    pub const fn max_tool_output_bytes(&self) -> u32 {
        self.max_tool_output_bytes
    }

    /// Returns the source-byte budget for one inspection batch.
    pub const fn max_source_read_bytes_per_batch(&self) -> u32 {
        self.max_source_read_bytes_per_batch
    }

    /// Returns the source-file budget for one inspection batch.
    pub const fn max_source_files_per_batch(&self) -> u16 {
        self.max_source_files_per_batch
    }

    /// Returns the tool-call budget closed by one inspection batch.
    pub const fn inspection_calls_per_batch(&self) -> u8 {
        self.inspection_calls_per_batch
    }

    /// Returns the consecutive no-progress batch budget.
    pub const fn max_no_progress_batches(&self) -> u8 {
        self.max_no_progress_batches
    }

    /// Returns the authority-refresh budget per resume.
    pub const fn max_authority_refreshes_per_resume(&self) -> u8 {
        self.max_authority_refreshes_per_resume
    }
}

impl Default for ExecutionBudgetsV1 {
    fn default() -> Self {
        Self {
            max_capsule_bytes: MAX_CAPSULE_BYTES,
            max_tool_output_bytes: DEFAULT_MAX_TOOL_OUTPUT_BYTES,
            max_source_read_bytes_per_batch: DEFAULT_MAX_SOURCE_READ_BYTES_PER_BATCH,
            max_source_files_per_batch: DEFAULT_MAX_SOURCE_FILES_PER_BATCH,
            inspection_calls_per_batch: DEFAULT_INSPECTION_CALLS_PER_BATCH,
            max_no_progress_batches: DEFAULT_MAX_NO_PROGRESS_BATCHES,
            max_authority_refreshes_per_resume: DEFAULT_MAX_AUTHORITY_REFRESHES_PER_RESUME,
        }
    }
}

impl<'de> Deserialize<'de> for ExecutionBudgetsV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ExecutionBudgetsV1Wire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExecutionBudgetsV1Wire {
    max_capsule_bytes: u32,
    max_tool_output_bytes: u32,
    max_source_read_bytes_per_batch: u32,
    max_source_files_per_batch: u16,
    inspection_calls_per_batch: u8,
    max_no_progress_batches: u8,
    max_authority_refreshes_per_resume: u8,
}

impl TryFrom<ExecutionBudgetsV1Wire> for ExecutionBudgetsV1 {
    type Error = ExecutionCapsuleError;

    fn try_from(value: ExecutionBudgetsV1Wire) -> Result<Self, Self::Error> {
        Self::new(
            value.max_capsule_bytes,
            value.max_tool_output_bytes,
            value.max_source_read_bytes_per_batch,
            value.max_source_files_per_batch,
            value.inspection_calls_per_batch,
            value.max_no_progress_batches,
            value.max_authority_refreshes_per_resume,
        )
    }
}

impl From<ExecutionBudgetsV1> for ExecutionBudgetsV1Wire {
    fn from(value: ExecutionBudgetsV1) -> Self {
        Self {
            max_capsule_bytes: value.max_capsule_bytes,
            max_tool_output_bytes: value.max_tool_output_bytes,
            max_source_read_bytes_per_batch: value.max_source_read_bytes_per_batch,
            max_source_files_per_batch: value.max_source_files_per_batch,
            inspection_calls_per_batch: value.inspection_calls_per_batch,
            max_no_progress_batches: value.max_no_progress_batches,
            max_authority_refreshes_per_resume: value.max_authority_refreshes_per_resume,
        }
    }
}

/// Content-addressed reference to the full slice queue artifact plus the
/// contiguous execution cursor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "ExecutionQueueRefV1Wire", into = "ExecutionQueueRefV1Wire")]
pub struct ExecutionQueueRefV1 {
    artifact: ArtifactRef,
    queue_digest: ArtifactDigest,
    total_slices: u32,
    completed_slices: u32,
    active_ordinal: u32,
}

impl ExecutionQueueRefV1 {
    /// Constructs a queue cursor; the active ordinal must continue the completed prefix.
    pub fn new(
        artifact: ArtifactRef,
        queue_digest: ArtifactDigest,
        total_slices: u32,
        completed_slices: u32,
        active_ordinal: u32,
    ) -> Result<Self, ExecutionCapsuleError> {
        if total_slices == 0 {
            return Err(ExecutionCapsuleError::EmptyQueue);
        }
        if completed_slices >= total_slices || active_ordinal != completed_slices + 1 {
            return Err(ExecutionCapsuleError::NonContiguousOrdinal);
        }
        Ok(Self {
            artifact,
            queue_digest,
            total_slices,
            completed_slices,
            active_ordinal,
        })
    }

    /// Returns the content-addressed queue artifact.
    pub const fn artifact(&self) -> &ArtifactRef {
        &self.artifact
    }

    /// Returns the digest of the canonical queue encoding.
    pub const fn queue_digest(&self) -> ArtifactDigest {
        self.queue_digest
    }

    /// Returns the total slice count.
    pub const fn total_slices(&self) -> u32 {
        self.total_slices
    }

    /// Returns the completed slice count.
    pub const fn completed_slices(&self) -> u32 {
        self.completed_slices
    }

    /// Returns the 1-based ordinal of the active slice.
    pub const fn active_ordinal(&self) -> u32 {
        self.active_ordinal
    }
}

impl<'de> Deserialize<'de> for ExecutionQueueRefV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ExecutionQueueRefV1Wire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExecutionQueueRefV1Wire {
    #[serde(with = "serde_domain::artifact_ref")]
    artifact: ArtifactRef,
    #[serde(with = "serde_domain::artifact_digest")]
    queue_digest: ArtifactDigest,
    total_slices: u32,
    completed_slices: u32,
    active_ordinal: u32,
}

impl TryFrom<ExecutionQueueRefV1Wire> for ExecutionQueueRefV1 {
    type Error = ExecutionCapsuleError;

    fn try_from(value: ExecutionQueueRefV1Wire) -> Result<Self, Self::Error> {
        Self::new(
            value.artifact,
            value.queue_digest,
            value.total_slices,
            value.completed_slices,
            value.active_ordinal,
        )
    }
}

impl From<ExecutionQueueRefV1> for ExecutionQueueRefV1Wire {
    fn from(value: ExecutionQueueRefV1) -> Self {
        Self {
            artifact: value.artifact,
            queue_digest: value.queue_digest,
            total_slices: value.total_slices,
            completed_slices: value.completed_slices,
            active_ordinal: value.active_ordinal,
        }
    }
}

/// Bounded execution recovery unit derived from an approved plan: the four
/// required context references, the queue cursor, the active slice and the
/// frozen v1 budgets.  The encoded form must fit the capsule budget.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "ExecutionCapsuleV1Wire", into = "ExecutionCapsuleV1Wire")]
pub struct ExecutionCapsuleV1 {
    schema_version: SchemaVersion,
    work_item_id: WorkItemId,
    story_id: StoryId,
    source_revision: StateRevision,
    approved_plan_digest: ArtifactDigest,
    policy_digest: PolicyDigest,
    inventory_generation: InventoryGeneration,
    story_ref: ArtifactRef,
    constraints_ref: ArtifactRef,
    thinking_engine_ref: ArtifactRef,
    verification_ref: ArtifactRef,
    queue: ExecutionQueueRefV1,
    active_slice: ExecutionSliceV1,
    budgets: ExecutionBudgetsV1,
}

impl ExecutionCapsuleV1 {
    /// Constructs a capsule; the active slice must match the queue cursor.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        work_item_id: WorkItemId,
        story_id: StoryId,
        source_revision: StateRevision,
        approved_plan_digest: ArtifactDigest,
        policy_digest: PolicyDigest,
        inventory_generation: InventoryGeneration,
        story_ref: ArtifactRef,
        constraints_ref: ArtifactRef,
        thinking_engine_ref: ArtifactRef,
        verification_ref: ArtifactRef,
        queue: ExecutionQueueRefV1,
        active_slice: ExecutionSliceV1,
        budgets: ExecutionBudgetsV1,
    ) -> Result<Self, ExecutionCapsuleError> {
        if active_slice.ordinal() != queue.active_ordinal() {
            return Err(ExecutionCapsuleError::ActiveOrdinalMismatch);
        }
        Ok(Self {
            schema_version,
            work_item_id,
            story_id,
            source_revision,
            approved_plan_digest,
            policy_digest,
            inventory_generation,
            story_ref,
            constraints_ref,
            thinking_engine_ref,
            verification_ref,
            queue,
            active_slice,
            budgets,
        })
    }

    /// Returns the contract schema version.
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the owning work item identity.
    pub const fn work_item_id(&self) -> &WorkItemId {
        &self.work_item_id
    }

    /// Returns the owning story identity.
    pub const fn story_id(&self) -> &StoryId {
        &self.story_id
    }

    /// Returns the state revision the capsule was derived from.
    pub const fn source_revision(&self) -> StateRevision {
        self.source_revision
    }

    /// Returns the digest of the approved execution plan.
    pub const fn approved_plan_digest(&self) -> ArtifactDigest {
        self.approved_plan_digest
    }

    /// Returns the policy digest the capsule was derived under.
    pub const fn policy_digest(&self) -> PolicyDigest {
        self.policy_digest
    }

    /// Returns the inventory generation the capsule was derived from.
    pub const fn inventory_generation(&self) -> InventoryGeneration {
        self.inventory_generation
    }

    /// Returns the content-addressed story reference.
    pub const fn story_ref(&self) -> &ArtifactRef {
        &self.story_ref
    }

    /// Returns the content-addressed constraints reference.
    pub const fn constraints_ref(&self) -> &ArtifactRef {
        &self.constraints_ref
    }

    /// Returns the content-addressed thinking-engine reference.
    pub const fn thinking_engine_ref(&self) -> &ArtifactRef {
        &self.thinking_engine_ref
    }

    /// Returns the content-addressed verification-contract reference.
    pub const fn verification_ref(&self) -> &ArtifactRef {
        &self.verification_ref
    }

    /// Returns the queue reference and execution cursor.
    pub const fn queue(&self) -> &ExecutionQueueRefV1 {
        &self.queue
    }

    /// Returns the active slice.
    pub const fn active_slice(&self) -> &ExecutionSliceV1 {
        &self.active_slice
    }

    /// Returns the execution budgets.
    pub const fn budgets(&self) -> &ExecutionBudgetsV1 {
        &self.budgets
    }
}

impl<'de> Deserialize<'de> for ExecutionCapsuleV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ExecutionCapsuleV1Wire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExecutionCapsuleV1Wire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::work_item_id")]
    work_item_id: WorkItemId,
    #[serde(with = "serde_domain::story_id")]
    story_id: StoryId,
    #[serde(with = "serde_domain::state_revision")]
    source_revision: StateRevision,
    #[serde(with = "serde_domain::artifact_digest")]
    approved_plan_digest: ArtifactDigest,
    #[serde(with = "serde_domain::policy_digest")]
    policy_digest: PolicyDigest,
    #[serde(with = "serde_domain::inventory_generation")]
    inventory_generation: InventoryGeneration,
    #[serde(with = "serde_domain::artifact_ref")]
    story_ref: ArtifactRef,
    #[serde(with = "serde_domain::artifact_ref")]
    constraints_ref: ArtifactRef,
    #[serde(with = "serde_domain::artifact_ref")]
    thinking_engine_ref: ArtifactRef,
    #[serde(with = "serde_domain::artifact_ref")]
    verification_ref: ArtifactRef,
    queue: ExecutionQueueRefV1,
    active_slice: ExecutionSliceV1,
    budgets: ExecutionBudgetsV1,
}

impl TryFrom<ExecutionCapsuleV1Wire> for ExecutionCapsuleV1 {
    type Error = ExecutionCapsuleError;

    fn try_from(value: ExecutionCapsuleV1Wire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.work_item_id,
            value.story_id,
            value.source_revision,
            value.approved_plan_digest,
            value.policy_digest,
            value.inventory_generation,
            value.story_ref,
            value.constraints_ref,
            value.thinking_engine_ref,
            value.verification_ref,
            value.queue,
            value.active_slice,
            value.budgets,
        )
    }
}

impl From<ExecutionCapsuleV1> for ExecutionCapsuleV1Wire {
    fn from(value: ExecutionCapsuleV1) -> Self {
        Self {
            schema_version: value.schema_version,
            work_item_id: value.work_item_id,
            story_id: value.story_id,
            source_revision: value.source_revision,
            approved_plan_digest: value.approved_plan_digest,
            policy_digest: value.policy_digest,
            inventory_generation: value.inventory_generation,
            story_ref: value.story_ref,
            constraints_ref: value.constraints_ref,
            thinking_engine_ref: value.thinking_engine_ref,
            verification_ref: value.verification_ref,
            queue: value.queue,
            active_slice: value.active_slice,
            budgets: value.budgets,
        }
    }
}

mod execution_slice_id {
    use ae_sdd_domain::ExecutionSliceId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(value: &ExecutionSliceId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.as_str().serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<ExecutionSliceId, D::Error>
    where
        D: Deserializer<'de>,
    {
        ExecutionSliceId::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

mod execution_slice_ids {
    use ae_sdd_domain::ExecutionSliceId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(value: &[ExecutionSliceId], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .iter()
            .map(ExecutionSliceId::as_str)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<ExecutionSliceId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<String>::deserialize(deserializer)?
            .into_iter()
            .map(|value| ExecutionSliceId::new(value).map_err(de::Error::custom))
            .collect()
    }
}

mod project_relative_paths {
    use ae_sdd_domain::ProjectRelativePath;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(
        value: &[ProjectRelativePath],
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .iter()
            .map(ProjectRelativePath::as_str)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<ProjectRelativePath>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<String>::deserialize(deserializer)?
            .into_iter()
            .map(|value| ProjectRelativePath::new(value).map_err(de::Error::custom))
            .collect()
    }
}

mod verification_ids {
    use ae_sdd_domain::VerificationId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(value: &[VerificationId], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .iter()
            .map(VerificationId::as_str)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<VerificationId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<String>::deserialize(deserializer)?
            .into_iter()
            .map(|value| VerificationId::new(value).map_err(de::Error::custom))
            .collect()
    }
}
