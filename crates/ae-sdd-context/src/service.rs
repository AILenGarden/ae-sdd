use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use ae_sdd_contracts::{
    ContextBundleId, MethodologyRef, SchemaVersion,
    execution_runtime::ExecutionCapsuleV1,
    resource::{ContextBundleRef, LoadedContextProof, ResourceContractError},
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactRef, ContextDigest, InventoryGeneration, PolicyDigest,
    ProjectRelativePath, StateRevision, WorkItemId,
};
use thiserror::Error;

pub const MAX_COMPACT_STATE_DELTA_BYTES: u64 = 64 * 1024;
pub const MAX_COMPACT_STATE_DELTA_ENTRIES: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextBundleInput {
    schema_version: SchemaVersion,
    context_id: ContextBundleId,
    work_item_id: WorkItemId,
    story_ref: ArtifactRef,
    constraints_ref: ArtifactRef,
    thinking_engine_ref: ArtifactRef,
    verification_ref: ArtifactRef,
    methodology_ref: MethodologyRef,
    optional_refs: Vec<ArtifactRef>,
    state_revision: StateRevision,
    inventory_generation: InventoryGeneration,
    projection_policy_digest: PolicyDigest,
    computed_at_unix_ms: u64,
}

impl ContextBundleInput {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        context_id: ContextBundleId,
        work_item_id: WorkItemId,
        story_ref: ArtifactRef,
        constraints_ref: ArtifactRef,
        thinking_engine_ref: ArtifactRef,
        verification_ref: ArtifactRef,
        methodology_ref: MethodologyRef,
        optional_refs: Vec<ArtifactRef>,
        state_revision: StateRevision,
        inventory_generation: InventoryGeneration,
        projection_policy_digest: PolicyDigest,
        computed_at_unix_ms: u64,
    ) -> Result<Self, ContextServiceError> {
        if computed_at_unix_ms == 0 {
            return Err(ContextServiceError::InvalidComputedTime);
        }
        Ok(Self {
            schema_version,
            context_id,
            work_item_id,
            story_ref,
            constraints_ref,
            thinking_engine_ref,
            verification_ref,
            methodology_ref,
            optional_refs,
            state_revision,
            inventory_generation,
            projection_policy_digest,
            computed_at_unix_ms,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextCacheKey {
    work_item_id: WorkItemId,
    story_ref: ArtifactRef,
    constraints_ref: ArtifactRef,
    thinking_engine_ref: ArtifactRef,
    verification_ref: ArtifactRef,
    methodology_digest: ArtifactDigest,
    state_revision: StateRevision,
    inventory_generation: InventoryGeneration,
    projection_policy_digest: PolicyDigest,
    digest: ContextDigest,
}

impl ContextCacheKey {
    fn from_input(input: &ContextBundleInput) -> Self {
        let methodology_digest = methodology_digest(&input.methodology_ref);
        let digest = cache_key_digest(input, methodology_digest);
        Self {
            work_item_id: input.work_item_id.clone(),
            story_ref: input.story_ref.clone(),
            constraints_ref: input.constraints_ref.clone(),
            thinking_engine_ref: input.thinking_engine_ref.clone(),
            verification_ref: input.verification_ref.clone(),
            methodology_digest,
            state_revision: input.state_revision,
            inventory_generation: input.inventory_generation,
            projection_policy_digest: input.projection_policy_digest,
            digest,
        }
    }

    pub const fn story_ref(&self) -> &ArtifactRef {
        &self.story_ref
    }

    pub const fn constraints_ref(&self) -> &ArtifactRef {
        &self.constraints_ref
    }

    pub const fn thinking_engine_ref(&self) -> &ArtifactRef {
        &self.thinking_engine_ref
    }

    pub const fn verification_ref(&self) -> &ArtifactRef {
        &self.verification_ref
    }

    pub const fn methodology_digest(&self) -> ArtifactDigest {
        self.methodology_digest
    }

    pub const fn state_revision(&self) -> StateRevision {
        self.state_revision
    }

    pub const fn inventory_generation(&self) -> InventoryGeneration {
        self.inventory_generation
    }

    pub const fn projection_policy_digest(&self) -> PolicyDigest {
        self.projection_policy_digest
    }

    pub const fn digest(&self) -> ContextDigest {
        self.digest
    }

    #[must_use]
    pub fn freshness_against(&self, current: &Self) -> ContextFreshness {
        let mut changed = Vec::new();
        if self.work_item_id != current.work_item_id {
            changed.push(ContextFreshnessDimension::WorkItem);
        }
        if self.story_ref.digest() != current.story_ref.digest() {
            changed.push(ContextFreshnessDimension::Story);
        }
        if self.constraints_ref.digest() != current.constraints_ref.digest() {
            changed.push(ContextFreshnessDimension::Constraints);
        }
        if self.thinking_engine_ref.digest() != current.thinking_engine_ref.digest() {
            changed.push(ContextFreshnessDimension::ThinkingEngine);
        }
        if self.verification_ref.digest() != current.verification_ref.digest() {
            changed.push(ContextFreshnessDimension::Verification);
        }
        if self.methodology_digest != current.methodology_digest {
            changed.push(ContextFreshnessDimension::Methodology);
        }
        if self.state_revision != current.state_revision {
            changed.push(ContextFreshnessDimension::StateRevision);
        }
        if self.inventory_generation != current.inventory_generation {
            changed.push(ContextFreshnessDimension::InventoryGeneration);
        }
        if self.projection_policy_digest != current.projection_policy_digest {
            changed.push(ContextFreshnessDimension::ProjectionPolicy);
        }
        if changed.is_empty() {
            ContextFreshness::Fresh
        } else {
            ContextFreshness::Stale(changed)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContextFreshnessDimension {
    WorkItem,
    Story,
    Constraints,
    ThinkingEngine,
    Verification,
    Methodology,
    StateRevision,
    InventoryGeneration,
    ProjectionPolicy,
    ExecutionPlan,
    ExecutionQueue,
    ExecutionCapsule,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextFreshness {
    Fresh,
    Stale(Vec<ContextFreshnessDimension>),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContextSelector {
    Story,
    Constraints,
    ThinkingEngine,
    Verification,
    Methodology,
    StateRevision,
    InventoryGeneration,
    ProjectionPolicy,
    ExecutionCapsule,
    ExecutionQueue,
    ActiveSlice,
    Optional(ProjectRelativePath),
}

/// Cache key for one execution-capsule projection stream: it binds the
/// approved-plan, queue and canonical capsule digests so plan, queue or
/// capsule drift is visible to the existing full/delta/no-change machinery.
/// The plan digest is embedded in the queue encoding and the queue reference
/// is embedded in the capsule, so a plan change moves all three digests and a
/// queue change moves the queue and capsule digests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionCapsuleKey {
    approved_plan_digest: ArtifactDigest,
    queue_digest: ArtifactDigest,
    capsule_digest: ArtifactDigest,
}

impl ExecutionCapsuleKey {
    /// Binds the three digests that identify one execution capsule.
    pub const fn new(
        approved_plan_digest: ArtifactDigest,
        queue_digest: ArtifactDigest,
        capsule_digest: ArtifactDigest,
    ) -> Self {
        Self {
            approved_plan_digest,
            queue_digest,
            capsule_digest,
        }
    }

    /// Derives the key from a capsule and the digest of its canonical encoding.
    pub fn from_capsule(capsule: &ExecutionCapsuleV1, capsule_digest: ArtifactDigest) -> Self {
        Self::new(
            capsule.approved_plan_digest(),
            capsule.queue().queue_digest(),
            capsule_digest,
        )
    }

    /// Returns the digest of the approved execution plan.
    pub const fn approved_plan_digest(&self) -> ArtifactDigest {
        self.approved_plan_digest
    }

    /// Returns the digest of the canonical queue encoding.
    pub const fn queue_digest(&self) -> ArtifactDigest {
        self.queue_digest
    }

    /// Returns the digest of the canonical capsule encoding.
    pub const fn capsule_digest(&self) -> ArtifactDigest {
        self.capsule_digest
    }

    /// Reports which bound digests changed, in stable declaration order.
    #[must_use]
    pub fn freshness_against(&self, current: &Self) -> ContextFreshness {
        let mut changed = Vec::new();
        if self.approved_plan_digest != current.approved_plan_digest {
            changed.push(ContextFreshnessDimension::ExecutionPlan);
        }
        if self.queue_digest != current.queue_digest {
            changed.push(ContextFreshnessDimension::ExecutionQueue);
        }
        if self.capsule_digest != current.capsule_digest {
            changed.push(ContextFreshnessDimension::ExecutionCapsule);
        }
        if changed.is_empty() {
            ContextFreshness::Fresh
        } else {
            ContextFreshness::Stale(changed)
        }
    }

    /// Maps drift against a newer key to the context selectors a resume must
    /// refresh: plan drift invalidates the whole capsule, queue drift the
    /// queue, and capsule drift the active slice projection.
    #[must_use]
    pub fn invalidated_against(&self, current: &Self) -> BTreeSet<ContextSelector> {
        let mut invalidated = BTreeSet::new();
        if self.approved_plan_digest != current.approved_plan_digest {
            invalidated.insert(ContextSelector::ExecutionCapsule);
        }
        if self.queue_digest != current.queue_digest {
            invalidated.insert(ContextSelector::ExecutionQueue);
        }
        if self.capsule_digest != current.capsule_digest {
            invalidated.insert(ContextSelector::ActiveSlice);
        }
        invalidated
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundledContext {
    bundle_ref: ContextBundleRef,
    proof: LoadedContextProof,
    cache_key: ContextCacheKey,
}

impl BundledContext {
    pub const fn bundle_ref(&self) -> &ContextBundleRef {
        &self.bundle_ref
    }

    pub const fn proof(&self) -> &LoadedContextProof {
        &self.proof
    }

    pub const fn cache_key(&self) -> &ContextCacheKey {
        &self.cache_key
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactStateDelta {
    prior_digest: ContextDigest,
    next_digest: ContextDigest,
    changed: Vec<ArtifactRef>,
    removed: Vec<ProjectRelativePath>,
    invalidated: BTreeSet<ContextSelector>,
    byte_length: u64,
}

impl CompactStateDelta {
    fn between(
        prior: &BundledContext,
        next: &BundledContext,
        invalidated: BTreeSet<ContextSelector>,
    ) -> Result<Self, ContextServiceError> {
        let prior_by_path = refs_by_path(prior.bundle_ref.artifact_refs());
        let next_by_path = refs_by_path(next.bundle_ref.artifact_refs());
        let changed = next_by_path
            .iter()
            .filter(|(path, reference)| prior_by_path.get(*path) != Some(reference))
            .map(|(_, reference)| (*reference).clone())
            .collect::<Vec<_>>();
        let removed = prior_by_path
            .keys()
            .filter(|path| !next_by_path.contains_key(*path))
            .map(|path| ProjectRelativePath::new((*path).to_owned()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ContextServiceError::InvalidCanonicalPath)?;

        let entry_count = changed.len() + removed.len() + invalidated.len();
        if entry_count > MAX_COMPACT_STATE_DELTA_ENTRIES {
            return Err(ContextServiceError::DeltaEntryLimitExceeded {
                actual: entry_count,
                maximum: MAX_COMPACT_STATE_DELTA_ENTRIES,
            });
        }
        let changed_bytes = changed.iter().try_fold(0_u64, |total, reference| {
            total
                .checked_add(reference.byte_length())
                .and_then(|value| value.checked_add(reference.path().as_str().len() as u64))
        });
        let removed_bytes = removed.iter().try_fold(0_u64, |total, path| {
            total.checked_add(path.as_str().len() as u64)
        });
        let selector_bytes = u64::try_from(invalidated.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(32);
        let byte_length = changed_bytes
            .and_then(|changed| removed_bytes.and_then(|removed| changed.checked_add(removed)))
            .and_then(|value| value.checked_add(selector_bytes))
            .ok_or(ContextServiceError::DeltaBudgetExceeded {
                actual: u64::MAX,
                maximum: MAX_COMPACT_STATE_DELTA_BYTES,
            })?;
        if byte_length > MAX_COMPACT_STATE_DELTA_BYTES {
            return Err(ContextServiceError::DeltaBudgetExceeded {
                actual: byte_length,
                maximum: MAX_COMPACT_STATE_DELTA_BYTES,
            });
        }
        Ok(Self {
            prior_digest: prior.bundle_ref.bundle_digest(),
            next_digest: next.bundle_ref.bundle_digest(),
            changed,
            removed,
            invalidated,
            byte_length,
        })
    }

    pub const fn prior_digest(&self) -> ContextDigest {
        self.prior_digest
    }

    pub const fn next_digest(&self) -> ContextDigest {
        self.next_digest
    }

    pub fn changed(&self) -> &[ArtifactRef] {
        &self.changed
    }

    pub fn removed(&self) -> &[ProjectRelativePath] {
        &self.removed
    }

    pub const fn invalidated(&self) -> &BTreeSet<ContextSelector> {
        &self.invalidated
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.removed.is_empty() && self.invalidated.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextRefresh {
    context: BundledContext,
    delta: CompactStateDelta,
}

impl ContextRefresh {
    pub const fn context(&self) -> &BundledContext {
        &self.context
    }

    pub const fn delta(&self) -> &CompactStateDelta {
        &self.delta
    }
}

pub trait ContextPort {
    fn bundle(&self, input: ContextBundleInput) -> Result<BundledContext, ContextServiceError>;

    fn refresh(
        &self,
        previous: &BundledContext,
        input: ContextBundleInput,
    ) -> Result<ContextRefresh, ContextServiceError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ContextService;

impl ContextPort for ContextService {
    fn bundle(&self, input: ContextBundleInput) -> Result<BundledContext, ContextServiceError> {
        let cache_key = ContextCacheKey::from_input(&input);
        let mut refs = BTreeMap::<Box<str>, ArtifactRef>::new();
        for reference in mandatory_and_optional_refs(&input) {
            insert_ref(&mut refs, reference)?;
        }
        let bundle_ref = ContextBundleRef::from_artifacts(
            input.schema_version,
            input.context_id,
            input.work_item_id.clone(),
            refs.into_values().collect(),
        )?;
        let proof = LoadedContextProof::new(
            input.schema_version,
            input.work_item_id,
            bundle_ref.clone(),
            input.story_ref,
            input.constraints_ref,
            input.thinking_engine_ref,
            input.verification_ref,
            input.methodology_ref,
            input.state_revision,
            input.inventory_generation,
            input.computed_at_unix_ms,
        )?;
        Ok(BundledContext {
            bundle_ref,
            proof,
            cache_key,
        })
    }

    fn refresh(
        &self,
        previous: &BundledContext,
        input: ContextBundleInput,
    ) -> Result<ContextRefresh, ContextServiceError> {
        let next = self.bundle(input)?;
        let invalidated = invalidated_selectors(previous, &next);
        let delta = CompactStateDelta::between(previous, &next, invalidated)?;
        Ok(ContextRefresh {
            context: next,
            delta,
        })
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ContextServiceError {
    #[error("context computation time must be greater than zero")]
    InvalidComputedTime,
    #[error("context references contain conflicting content for one path")]
    ConflictingArtifactPath,
    #[error("context delta contains {actual} entries; maximum is {maximum}")]
    DeltaEntryLimitExceeded { actual: usize, maximum: usize },
    #[error("context delta is {actual} bytes; maximum is {maximum}")]
    DeltaBudgetExceeded { actual: u64, maximum: u64 },
    #[error("contract emitted a non-canonical project-relative path")]
    InvalidCanonicalPath,
    #[error(transparent)]
    Resource(#[from] ResourceContractError),
}

fn mandatory_and_optional_refs(input: &ContextBundleInput) -> Vec<ArtifactRef> {
    let mut refs = vec![
        input.story_ref.clone(),
        input.constraints_ref.clone(),
        input.thinking_engine_ref.clone(),
        input.verification_ref.clone(),
        input.methodology_ref.compact_ref().clone(),
    ];
    if let Some(fallback) = input.methodology_ref.fallback_ref() {
        refs.push(fallback.clone());
    }
    refs.extend(input.optional_refs.iter().cloned());
    refs
}

fn insert_ref(
    refs: &mut BTreeMap<Box<str>, ArtifactRef>,
    reference: ArtifactRef,
) -> Result<(), ContextServiceError> {
    match refs.entry(reference.path().as_str().into()) {
        Entry::Vacant(slot) => {
            slot.insert(reference);
            Ok(())
        }
        Entry::Occupied(slot) if slot.get() == &reference => Ok(()),
        Entry::Occupied(_) => Err(ContextServiceError::ConflictingArtifactPath),
    }
}

fn refs_by_path(refs: &[ArtifactRef]) -> BTreeMap<&str, &ArtifactRef> {
    refs.iter()
        .map(|reference| (reference.path().as_str(), reference))
        .collect()
}

fn invalidated_selectors(
    previous: &BundledContext,
    next: &BundledContext,
) -> BTreeSet<ContextSelector> {
    let mut invalidated = BTreeSet::new();
    let previous_key = previous.cache_key();
    let next_key = next.cache_key();
    let freshness = previous_key.freshness_against(next_key);
    for (dimension, selector) in [
        (ContextFreshnessDimension::Story, ContextSelector::Story),
        (
            ContextFreshnessDimension::Constraints,
            ContextSelector::Constraints,
        ),
        (
            ContextFreshnessDimension::ThinkingEngine,
            ContextSelector::ThinkingEngine,
        ),
        (
            ContextFreshnessDimension::Verification,
            ContextSelector::Verification,
        ),
        (
            ContextFreshnessDimension::Methodology,
            ContextSelector::Methodology,
        ),
        (
            ContextFreshnessDimension::StateRevision,
            ContextSelector::StateRevision,
        ),
        (
            ContextFreshnessDimension::InventoryGeneration,
            ContextSelector::InventoryGeneration,
        ),
        (
            ContextFreshnessDimension::ProjectionPolicy,
            ContextSelector::ProjectionPolicy,
        ),
    ] {
        if matches!(freshness, ContextFreshness::Stale(ref changed) if changed.contains(&dimension))
        {
            invalidated.insert(selector);
        }
    }

    let reserved_paths = [
        previous.proof.story_ref(),
        previous.proof.constraints_ref(),
        previous.proof.thinking_engine_ref(),
        previous.proof.verification_ref(),
        previous.proof.methodology_ref().compact_ref(),
        next.proof.story_ref(),
        next.proof.constraints_ref(),
        next.proof.thinking_engine_ref(),
        next.proof.verification_ref(),
        next.proof.methodology_ref().compact_ref(),
    ]
    .into_iter()
    .map(|reference| reference.path().as_str())
    .chain(
        previous
            .proof
            .methodology_ref()
            .fallback_ref()
            .into_iter()
            .map(|reference| reference.path().as_str()),
    )
    .chain(
        next.proof
            .methodology_ref()
            .fallback_ref()
            .into_iter()
            .map(|reference| reference.path().as_str()),
    )
    .collect::<BTreeSet<_>>();

    let prior_refs = refs_by_path(previous.bundle_ref.artifact_refs());
    let next_refs = refs_by_path(next.bundle_ref.artifact_refs());
    for path in prior_refs.keys().chain(next_refs.keys()) {
        if reserved_paths.contains(path) || prior_refs.get(path) == next_refs.get(path) {
            continue;
        }
        if let Ok(path) = ProjectRelativePath::new((*path).to_owned()) {
            invalidated.insert(ContextSelector::Optional(path));
        }
    }
    invalidated
}

fn methodology_digest(methodology: &MethodologyRef) -> ArtifactDigest {
    let mut canonical = Vec::new();
    push_field(&mut canonical, b"ae-sdd/context-methodology/v1");
    push_field(&mut canonical, methodology.skill_id().as_str().as_bytes());
    push_field(
        &mut canonical,
        methodology.series_kind().as_str().as_bytes(),
    );
    push_artifact(&mut canonical, methodology.compact_ref());
    match methodology.fallback_ref() {
        Some(reference) => {
            canonical.push(1);
            push_artifact(&mut canonical, reference);
        }
        None => canonical.push(0),
    }
    canonical.extend_from_slice(methodology.entry_digest().as_bytes());
    canonical.extend_from_slice(methodology.catalog_digest().as_bytes());
    ArtifactDigest::digest(canonical)
}

fn cache_key_digest(
    input: &ContextBundleInput,
    methodology_digest: ArtifactDigest,
) -> ContextDigest {
    let mut canonical = Vec::new();
    push_field(&mut canonical, b"ae-sdd/context-cache-key/v1");
    push_field(&mut canonical, input.work_item_id.as_str().as_bytes());
    for reference in [
        &input.story_ref,
        &input.constraints_ref,
        &input.thinking_engine_ref,
        &input.verification_ref,
    ] {
        push_field(&mut canonical, reference.path().as_str().as_bytes());
        canonical.extend_from_slice(reference.digest().as_bytes());
    }
    canonical.extend_from_slice(methodology_digest.as_bytes());
    canonical.extend_from_slice(&input.state_revision.get().to_be_bytes());
    canonical.extend_from_slice(&input.inventory_generation.get().to_be_bytes());
    canonical.extend_from_slice(input.projection_policy_digest.as_bytes());
    ContextDigest::digest(canonical)
}

fn push_artifact(target: &mut Vec<u8>, reference: &ArtifactRef) {
    push_field(target, reference.kind().as_str().as_bytes());
    push_field(target, reference.path().as_str().as_bytes());
    target.extend_from_slice(reference.digest().as_bytes());
    target.extend_from_slice(&reference.byte_length().to_be_bytes());
}

fn push_field(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&(value.len() as u64).to_be_bytes());
    target.extend_from_slice(value);
}
