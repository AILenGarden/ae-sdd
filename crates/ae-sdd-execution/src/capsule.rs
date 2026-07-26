//! Deterministic execution queue and capsule builder.
//!
//! The builder consumes only typed input: the approved-plan identity, the
//! four required context references, the caller-resolved queue artifact
//! locator and the slice specs.  It never reads the filesystem, a clock,
//! randomness or a database.  Slices are canonically ordered by ordinal and
//! every inner collection is canonicalised by the frozen
//! [`ExecutionSliceV1`] constructor, so identical semantic input produces
//! byte-identical queue and capsule encodings no matter which order the
//! caller supplied the specs in.
//!
//! The full queue is written as its own content-addressed artifact and is
//! cursor-free: the execution cursor lives only in the
//! [`ExecutionQueueRefV1`] embedded in the capsule (and in project state), so
//! the queue digest stays stable while slices advance.  The capsule carries
//! only the active slice and fails closed when its encoding exceeds the
//! configured capsule budget.

use std::collections::BTreeMap;

use ae_sdd_contracts::SchemaVersion;
use ae_sdd_contracts::execution_runtime::{
    ExecutionBudgetsV1, ExecutionCapsuleError, ExecutionCapsuleV1, ExecutionQueueRefV1,
    ExecutionSliceV1, SourceReadSpecV1,
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, ExecutionSliceId, InventoryGeneration, PolicyDigest,
    ProjectRelativePath, StateRevision, StoryId, VerificationId, WorkItemId,
};
use serde::{Deserialize, Serialize};

use crate::error::ExecutionCapsuleBuildError;

/// One approved-plan slice as supplied to [`build_execution_capsule`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionSliceSpecV1 {
    /// Unique slice identity referenced by `depends_on` edges.
    pub slice_id: ExecutionSliceId,
    /// Plan-declared 1-based queue position.
    pub ordinal: u32,
    /// Bounded slice objective.
    pub objective: Box<str>,
    /// Identities of slices that must complete first.
    pub depends_on: Vec<ExecutionSliceId>,
    /// Writable project-relative path scope.
    pub path_scope: Vec<ProjectRelativePath>,
    /// Bounded source reads the slice may perform.
    pub source_reads: Vec<SourceReadSpecV1>,
    /// Focused verification bound to the slice.
    pub focused_verification_id: VerificationId,
    /// Broad verifications allowed after the focused GREEN.
    pub broad_verification_ids: Vec<VerificationId>,
    /// Logical key binding slice evidence.
    pub evidence_logical_key: Box<str>,
}

/// Full typed input for one deterministic capsule build.
#[derive(Clone, Debug)]
pub struct CapsuleBuildInputV1 {
    /// Owning work item identity.
    pub work_item_id: WorkItemId,
    /// Owning story identity.
    pub story_id: StoryId,
    /// State revision the capsule is derived from.
    pub source_revision: StateRevision,
    /// Digest of the approved execution plan.
    pub approved_plan_digest: ArtifactDigest,
    /// Policy digest the capsule is derived under.
    pub policy_digest: PolicyDigest,
    /// Inventory generation the capsule is derived from.
    pub inventory_generation: InventoryGeneration,
    /// Content-addressed story reference.
    pub story_ref: ArtifactRef,
    /// Content-addressed constraints reference.
    pub constraints_ref: ArtifactRef,
    /// Content-addressed thinking-engine reference.
    pub thinking_engine_ref: ArtifactRef,
    /// Content-addressed verification-contract reference.
    pub verification_ref: ArtifactRef,
    /// Artifact kind under which the queue artifact is published.
    pub queue_artifact_kind: ArtifactKind,
    /// Project-relative queue artifact locator, resolved by the caller and
    /// never guessed inside the builder.
    pub queue_artifact_path: ProjectRelativePath,
    /// Slice specs in any order; the builder canonicalises them by ordinal.
    pub slices: Vec<ExecutionSliceSpecV1>,
    /// 1-based ordinal of the active slice; the completed prefix precedes it.
    pub active_ordinal: u32,
    /// Frozen execution budgets; the capsule byte budget is enforced.
    pub budgets: ExecutionBudgetsV1,
}

/// Cursor-free full slice queue, serialized as the content-addressed queue
/// artifact.  Collections are canonically ordered at build time so the
/// encoding is byte-stable for identical semantic input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionQueueV1 {
    schema_version: SchemaVersion,
    #[serde(with = "work_item_id")]
    work_item_id: WorkItemId,
    #[serde(with = "story_id")]
    story_id: StoryId,
    #[serde(with = "artifact_digest")]
    approved_plan_digest: ArtifactDigest,
    total_slices: u32,
    slices: Vec<ExecutionSliceV1>,
    budgets: ExecutionBudgetsV1,
}

impl ExecutionQueueV1 {
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

    /// Returns the digest of the approved execution plan.
    pub const fn approved_plan_digest(&self) -> ArtifactDigest {
        self.approved_plan_digest
    }

    /// Returns the total slice count.
    pub const fn total_slices(&self) -> u32 {
        self.total_slices
    }

    /// Returns the slices in canonical ordinal order.
    pub fn slices(&self) -> &[ExecutionSliceV1] {
        &self.slices
    }

    /// Returns the frozen execution budgets.
    pub const fn budgets(&self) -> &ExecutionBudgetsV1 {
        &self.budgets
    }
}

/// Deterministic result of one capsule build: the full queue artifact plus
/// the bounded capsule that embeds only the active slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapsuleBuildOutcome {
    queue: ExecutionQueueV1,
    queue_bytes: Vec<u8>,
    queue_digest: ArtifactDigest,
    capsule: ExecutionCapsuleV1,
    capsule_bytes: Vec<u8>,
    capsule_digest: ArtifactDigest,
}

impl CapsuleBuildOutcome {
    /// Returns the full cursor-free queue.
    pub const fn queue(&self) -> &ExecutionQueueV1 {
        &self.queue
    }

    /// Returns the canonical queue artifact encoding to persist.
    pub fn queue_bytes(&self) -> &[u8] {
        &self.queue_bytes
    }

    /// Returns the digest of the canonical queue encoding.
    pub const fn queue_digest(&self) -> ArtifactDigest {
        self.queue_digest
    }

    /// Returns the bounded capsule carrying only the active slice.
    pub const fn capsule(&self) -> &ExecutionCapsuleV1 {
        &self.capsule
    }

    /// Returns the canonical capsule encoding to persist.
    pub fn capsule_bytes(&self) -> &[u8] {
        &self.capsule_bytes
    }

    /// Returns the digest of the canonical capsule encoding.
    pub const fn capsule_digest(&self) -> ArtifactDigest {
        self.capsule_digest
    }
}

/// Builds a deterministic queue artifact and active-slice capsule from typed
/// input.
///
/// Validation fails closed on an empty queue, contract-invalid slices,
/// non-contiguous ordinals, duplicate slice identities, unknown or cyclic
/// dependencies, dependencies that do not reference a strictly lower
/// ordinal, an active ordinal outside the queue and capsules over the
/// configured byte budget.
pub fn build_execution_capsule(
    input: &CapsuleBuildInputV1,
) -> Result<CapsuleBuildOutcome, ExecutionCapsuleBuildError> {
    if input.slices.is_empty() {
        return Err(ExecutionCapsuleError::EmptyQueue.into());
    }
    let mut slices = input
        .slices
        .iter()
        .map(build_slice)
        .collect::<Result<Vec<_>, _>>()?;
    validate_ordinal_prefix(&slices)?;
    slices.sort_unstable_by_key(ExecutionSliceV1::ordinal);
    validate_unique_slice_ids(&slices)?;
    validate_acyclic(&slices)?;
    validate_dependencies_point_backwards(&slices)?;

    let total_slices =
        u32::try_from(slices.len()).map_err(|_| ExecutionCapsuleError::CollectionLimitExceeded)?;
    if input.active_ordinal == 0 || input.active_ordinal > total_slices {
        return Err(ExecutionCapsuleBuildError::InvalidActiveOrdinal {
            active_ordinal: input.active_ordinal,
            total_slices,
        });
    }

    let queue = ExecutionQueueV1 {
        schema_version: SchemaVersion::V1,
        work_item_id: input.work_item_id.clone(),
        story_id: input.story_id.clone(),
        approved_plan_digest: input.approved_plan_digest,
        total_slices,
        slices,
        budgets: input.budgets,
    };
    let queue_bytes = serde_json::to_vec(&queue).map_err(|_| {
        ExecutionCapsuleBuildError::CanonicalEncodeFailed {
            artifact: "ae-sdd-execution-queue/v1",
        }
    })?;
    let queue_digest = ArtifactDigest::digest(&queue_bytes);
    let queue_ref = ExecutionQueueRefV1::new(
        ArtifactRef::new(
            input.queue_artifact_kind.clone(),
            input.queue_artifact_path.clone(),
            queue_digest,
            queue_bytes.len() as u64,
        ),
        queue_digest,
        total_slices,
        input.active_ordinal - 1,
        input.active_ordinal,
    )?;
    let active_slice = queue
        .slices()
        .iter()
        .find(|slice| slice.ordinal() == input.active_ordinal)
        .cloned()
        .ok_or(ExecutionCapsuleBuildError::InvalidActiveOrdinal {
            active_ordinal: input.active_ordinal,
            total_slices,
        })?;
    let capsule = ExecutionCapsuleV1::new(
        SchemaVersion::V1,
        input.work_item_id.clone(),
        input.story_id.clone(),
        input.source_revision,
        input.approved_plan_digest,
        input.policy_digest,
        input.inventory_generation,
        input.story_ref.clone(),
        input.constraints_ref.clone(),
        input.thinking_engine_ref.clone(),
        input.verification_ref.clone(),
        queue_ref,
        active_slice,
        input.budgets,
    )?;
    let capsule_bytes = serde_json::to_vec(&capsule).map_err(|_| {
        ExecutionCapsuleBuildError::CanonicalEncodeFailed {
            artifact: "ae-sdd-execution-capsule/v1",
        }
    })?;
    input.budgets.check_capsule_len(capsule_bytes.len())?;
    let capsule_digest = ArtifactDigest::digest(&capsule_bytes);

    Ok(CapsuleBuildOutcome {
        queue,
        queue_bytes,
        queue_digest,
        capsule,
        capsule_bytes,
        capsule_digest,
    })
}

fn build_slice(
    spec: &ExecutionSliceSpecV1,
) -> Result<ExecutionSliceV1, ExecutionCapsuleBuildError> {
    Ok(ExecutionSliceV1::new(
        spec.slice_id.clone(),
        spec.ordinal,
        spec.objective.clone(),
        spec.depends_on.clone(),
        spec.path_scope.clone(),
        spec.source_reads.clone(),
        spec.focused_verification_id.clone(),
        spec.broad_verification_ids.clone(),
        spec.evidence_logical_key.clone(),
    )?)
}

fn validate_ordinal_prefix(slices: &[ExecutionSliceV1]) -> Result<(), ExecutionCapsuleBuildError> {
    let mut ordinals: Vec<u32> = slices.iter().map(ExecutionSliceV1::ordinal).collect();
    ordinals.sort_unstable();
    let mut expected = 1_u32;
    for ordinal in ordinals {
        if ordinal != expected {
            return Err(ExecutionCapsuleBuildError::NonContiguousOrdinals);
        }
        expected = expected.saturating_add(1);
    }
    Ok(())
}

fn validate_unique_slice_ids(
    slices: &[ExecutionSliceV1],
) -> Result<(), ExecutionCapsuleBuildError> {
    let mut seen = BTreeMap::new();
    for slice in slices {
        if seen.insert(slice.slice_id(), ()).is_some() {
            return Err(ExecutionCapsuleBuildError::DuplicateSliceId {
                slice_id: slice.slice_id().clone(),
            });
        }
    }
    Ok(())
}

/// Three-color depth-first cycle check.  Entry points are visited in ordinal
/// order and each slice's dependencies in their canonical sorted order, so
/// the first reported cycle is deterministic.
fn validate_acyclic(slices: &[ExecutionSliceV1]) -> Result<(), ExecutionCapsuleBuildError> {
    let by_id: BTreeMap<&ExecutionSliceId, &ExecutionSliceV1> = slices
        .iter()
        .map(|slice| (slice.slice_id(), slice))
        .collect();
    let mut colors: BTreeMap<&ExecutionSliceId, u8> = BTreeMap::new();
    for slice in slices {
        visit_slice(slice, &by_id, &mut colors)?;
    }
    Ok(())
}

fn visit_slice<'a>(
    slice: &'a ExecutionSliceV1,
    by_id: &BTreeMap<&'a ExecutionSliceId, &'a ExecutionSliceV1>,
    colors: &mut BTreeMap<&'a ExecutionSliceId, u8>,
) -> Result<(), ExecutionCapsuleBuildError> {
    // Color 1 marks the active DFS stack; color 2 marks fully explored slices.
    match colors.get(slice.slice_id()) {
        Some(&1) => {
            return Err(ExecutionCapsuleBuildError::DependencyCycle {
                slice_id: slice.slice_id().clone(),
            });
        }
        Some(&2) => return Ok(()),
        _ => {}
    }
    colors.insert(slice.slice_id(), 1);
    for dependency in slice.depends_on() {
        let dependency_slice =
            by_id
                .get(dependency)
                .ok_or_else(|| ExecutionCapsuleBuildError::UnknownDependency {
                    slice_id: slice.slice_id().clone(),
                    dependency: dependency.clone(),
                })?;
        visit_slice(dependency_slice, by_id, colors)?;
    }
    colors.insert(slice.slice_id(), 2);
    Ok(())
}

fn validate_dependencies_point_backwards(
    slices: &[ExecutionSliceV1],
) -> Result<(), ExecutionCapsuleBuildError> {
    let ordinals: BTreeMap<&ExecutionSliceId, u32> = slices
        .iter()
        .map(|slice| (slice.slice_id(), slice.ordinal()))
        .collect();
    for slice in slices {
        for dependency in slice.depends_on() {
            let dependency_ordinal = ordinals.get(dependency).copied().ok_or_else(|| {
                ExecutionCapsuleBuildError::UnknownDependency {
                    slice_id: slice.slice_id().clone(),
                    dependency: dependency.clone(),
                }
            })?;
            if dependency_ordinal >= slice.ordinal() {
                return Err(ExecutionCapsuleBuildError::DependencyNotLower {
                    slice_id: slice.slice_id().clone(),
                    dependency: dependency.clone(),
                });
            }
        }
    }
    Ok(())
}

mod work_item_id {
    use ae_sdd_domain::WorkItemId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(value: &WorkItemId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.as_str().serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<WorkItemId, D::Error>
    where
        D: Deserializer<'de>,
    {
        WorkItemId::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

mod story_id {
    use ae_sdd_domain::StoryId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(value: &StoryId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.as_str().serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<StoryId, D::Error>
    where
        D: Deserializer<'de>,
    {
        StoryId::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

mod artifact_digest {
    use std::str::FromStr;

    use ae_sdd_domain::ArtifactDigest;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(value: &ArtifactDigest, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.to_string().serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<ArtifactDigest, D::Error>
    where
        D: Deserializer<'de>,
    {
        ArtifactDigest::from_str(&String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}
