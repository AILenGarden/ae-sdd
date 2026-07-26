use std::collections::{BTreeMap, BTreeSet};

use ae_sdd_contracts::execution_runtime::ExecutionCapsuleV1;
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, ArtifactRef, ContextDigest, ContextProjectionId, ContextRevision,
    DelegationId, DeliverableContract, InventoryGeneration, OperationId, PolicyDigest,
    ProjectPathScope, SessionId, StateRevision,
};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::service::ExecutionCapsuleKey;

pub const DEFAULT_ROOT_PROJECTION_MAX_BYTES: u32 = 65_536;
pub const DEFAULT_CHILD_PROJECTION_MAX_BYTES: u32 = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectionBudget {
    root_max_bytes: u32,
    child_max_bytes: u32,
}

impl ProjectionBudget {
    pub fn new(root_max_bytes: u32, child_max_bytes: u32) -> Result<Self, ContextProjectionError> {
        if root_max_bytes == 0 || child_max_bytes == 0 {
            return Err(ContextProjectionError::InvalidBudget);
        }
        Ok(Self {
            root_max_bytes,
            child_max_bytes,
        })
    }

    #[must_use]
    pub const fn maximum_for(self, role: AgentRole) -> u32 {
        if matches!(role, AgentRole::Root) {
            self.root_max_bytes
        } else {
            self.child_max_bytes
        }
    }
}

impl Default for ProjectionBudget {
    fn default() -> Self {
        Self {
            root_max_bytes: DEFAULT_ROOT_PROJECTION_MAX_BYTES,
            child_max_bytes: DEFAULT_CHILD_PROJECTION_MAX_BYTES,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextView {
    target_session_id: SessionId,
    target_role: AgentRole,
    target_delegation_id: Option<DelegationId>,
    summary: Box<str>,
    input_refs: Vec<ArtifactRef>,
    constraint_refs: Vec<ArtifactRef>,
    memory_refs: Vec<RoleMemoryRef>,
    allowed_operations: BTreeSet<OperationId>,
    allowed_paths: BTreeSet<ProjectPathScope>,
    deliverable_contract: Option<DeliverableContract>,
}

impl ContextView {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        target_session_id: SessionId,
        target_role: AgentRole,
        target_delegation_id: Option<DelegationId>,
        summary: impl Into<Box<str>>,
        mut input_refs: Vec<ArtifactRef>,
        mut constraint_refs: Vec<ArtifactRef>,
        mut memory_refs: Vec<RoleMemoryRef>,
        allowed_operations: impl IntoIterator<Item = OperationId>,
        allowed_paths: impl IntoIterator<Item = ProjectPathScope>,
        deliverable_contract: Option<DeliverableContract>,
    ) -> Result<Self, ContextProjectionError> {
        let summary = summary.into();
        if summary.is_empty() {
            return Err(ContextProjectionError::EmptySummary);
        }
        if memory_refs.iter().any(|memory| {
            !memory.is_visible_to(target_session_id, target_role, target_delegation_id)
        }) {
            return Err(ContextProjectionError::MemoryVisibilityViolation);
        }
        canonicalize_artifact_refs(&mut input_refs)?;
        canonicalize_artifact_refs(&mut constraint_refs)?;
        memory_refs.sort_by(|left, right| {
            left.artifact()
                .path()
                .as_str()
                .cmp(right.artifact().path().as_str())
        });
        if memory_refs
            .windows(2)
            .any(|pair| pair[0].artifact().path() == pair[1].artifact().path())
        {
            return Err(ContextProjectionError::DuplicateReferencePath);
        }
        Ok(Self {
            target_session_id,
            target_role,
            target_delegation_id,
            summary,
            input_refs,
            constraint_refs,
            memory_refs,
            allowed_operations: allowed_operations.into_iter().collect(),
            allowed_paths: allowed_paths.into_iter().collect(),
            deliverable_contract,
        })
    }

    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    #[must_use]
    pub fn input_refs(&self) -> &[ArtifactRef] {
        &self.input_refs
    }

    #[must_use]
    pub fn constraint_refs(&self) -> &[ArtifactRef] {
        &self.constraint_refs
    }

    #[must_use]
    pub fn memory_refs(&self) -> &[RoleMemoryRef] {
        &self.memory_refs
    }

    #[must_use]
    pub const fn allowed_operations(&self) -> &BTreeSet<OperationId> {
        &self.allowed_operations
    }

    #[must_use]
    pub const fn allowed_paths(&self) -> &BTreeSet<ProjectPathScope> {
        &self.allowed_paths
    }

    #[must_use]
    pub const fn deliverable_contract(&self) -> Option<&DeliverableContract> {
        self.deliverable_contract.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryVisibility {
    Session(SessionId),
    Delegation(DelegationId),
    RootSummary(SessionId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleMemoryRef {
    artifact: ArtifactRef,
    visibility: MemoryVisibility,
}

impl RoleMemoryRef {
    #[must_use]
    pub const fn new(artifact: ArtifactRef, visibility: MemoryVisibility) -> Self {
        Self {
            artifact,
            visibility,
        }
    }

    #[must_use]
    pub const fn artifact(&self) -> &ArtifactRef {
        &self.artifact
    }

    #[must_use]
    pub const fn visibility(&self) -> MemoryVisibility {
        self.visibility
    }

    #[must_use]
    pub fn is_visible_to(
        &self,
        session_id: SessionId,
        role: AgentRole,
        delegation_id: Option<DelegationId>,
    ) -> bool {
        match self.visibility {
            MemoryVisibility::Session(owner) => owner == session_id,
            MemoryVisibility::Delegation(owner) => delegation_id == Some(owner),
            MemoryVisibility::RootSummary(owner) => {
                matches!(role, AgentRole::Root) && owner == session_id
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextProjection {
    projection_id: ContextProjectionId,
    session_id: SessionId,
    delegation_id: Option<DelegationId>,
    role: AgentRole,
    source_revision: StateRevision,
    context_revision: ContextRevision,
    policy_digest: PolicyDigest,
    inventory_generation: InventoryGeneration,
    view: ContextView,
    byte_length: u32,
    digest: ContextDigest,
    expires_at_unix_ms: u64,
    maximum_bytes: u32,
}

impl ContextProjection {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        projection_id: ContextProjectionId,
        session_id: SessionId,
        delegation_id: Option<DelegationId>,
        role: AgentRole,
        source_revision: StateRevision,
        context_revision: ContextRevision,
        policy_digest: PolicyDigest,
        inventory_generation: InventoryGeneration,
        view: ContextView,
        expires_at_unix_ms: u64,
        budget: ProjectionBudget,
    ) -> Result<Self, ContextProjectionError> {
        match role {
            AgentRole::Root if delegation_id.is_some() => {
                return Err(ContextProjectionError::RootDelegationForbidden);
            }
            AgentRole::Root => {}
            _ if delegation_id.is_none() => {
                return Err(ContextProjectionError::ChildDelegationRequired);
            }
            _ => {}
        }
        if expires_at_unix_ms == 0 {
            return Err(ContextProjectionError::InvalidExpiry);
        }
        if view.target_session_id != session_id
            || view.target_role != role
            || view.target_delegation_id != delegation_id
        {
            return Err(ContextProjectionError::ViewIdentityMismatch);
        }
        let canonical = canonical_bytes(
            projection_id,
            session_id,
            delegation_id,
            role,
            source_revision,
            context_revision,
            policy_digest,
            inventory_generation,
            &view,
            expires_at_unix_ms,
        )?;
        let byte_length = u32::try_from(canonical.len()).unwrap_or(u32::MAX);
        let maximum = budget.maximum_for(role);
        if byte_length > maximum {
            return Err(ContextProjectionError::BudgetExceeded {
                actual: byte_length,
                maximum,
            });
        }
        Ok(Self {
            projection_id,
            session_id,
            delegation_id,
            role,
            source_revision,
            context_revision,
            policy_digest,
            inventory_generation,
            view,
            byte_length,
            digest: ContextDigest::digest(canonical),
            expires_at_unix_ms,
            maximum_bytes: maximum,
        })
    }

    #[must_use]
    pub const fn context_revision(&self) -> ContextRevision {
        self.context_revision
    }

    #[must_use]
    pub const fn digest(&self) -> ContextDigest {
        self.digest
    }

    #[must_use]
    pub const fn byte_length(&self) -> u32 {
        self.byte_length
    }

    #[must_use]
    pub const fn role(&self) -> AgentRole {
        self.role
    }

    #[must_use]
    pub const fn view(&self) -> &ContextView {
        &self.view
    }

    #[must_use]
    pub const fn projection_id(&self) -> ContextProjectionId {
        self.projection_id
    }

    #[must_use]
    pub const fn source_revision(&self) -> StateRevision {
        self.source_revision
    }

    #[must_use]
    pub const fn policy_digest(&self) -> PolicyDigest {
        self.policy_digest
    }

    #[must_use]
    pub const fn inventory_generation(&self) -> InventoryGeneration {
        self.inventory_generation
    }

    #[must_use]
    pub const fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }

    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    #[must_use]
    pub const fn delegation_id(&self) -> Option<DelegationId> {
        self.delegation_id
    }

    pub fn response_from(
        &self,
        previous: Option<&Self>,
        known_revision: ContextRevision,
        known_digest: ContextDigest,
    ) -> Result<ContextDelta, ContextProjectionError> {
        if known_revision > self.context_revision {
            return Err(ContextProjectionError::RevisionStale);
        }
        if known_revision == self.context_revision && known_digest == self.digest {
            return Ok(ContextDelta {
                kind: ProjectionKind::NoChange,
                base_revision: known_revision,
                target_revision: self.context_revision,
                target_digest: self.digest,
                view: None,
                changes: None,
            });
        }
        let delta_source = previous.filter(|old| {
            old.context_revision == known_revision
                && old.digest == known_digest
                && old.session_id == self.session_id
                && old.role == self.role
                && old.delegation_id == self.delegation_id
        });
        let changes = delta_source.map(|old| ContextViewDelta::between(&old.view, &self.view));
        if let Some(changes) = &changes {
            let actual = changes.estimated_bytes();
            if actual > self.maximum_bytes {
                return Err(ContextProjectionError::DeltaBudgetExceeded {
                    actual,
                    maximum: self.maximum_bytes,
                });
            }
        }
        Ok(ContextDelta {
            kind: if delta_source.is_some() {
                ProjectionKind::Delta
            } else {
                ProjectionKind::Full
            },
            base_revision: known_revision,
            target_revision: self.context_revision,
            target_digest: self.digest,
            view: delta_source.is_none().then(|| self.view.clone()),
            changes,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionKind {
    Full,
    Delta,
    NoChange,
}

/// Canonical execution-capsule projection prepared for the runtime context
/// cache.  The typed capsule is serialized exactly once; the encoding is
/// bound by the capsule's own frozen byte budget and its digest, together
/// with the approved-plan and queue digests, forms the
/// [`ExecutionCapsuleKey`] a resume is validated against.  The JSON value is
/// the body fed into the existing full/delta/no-change cache, so an active
/// ordinal move only changes the `queue` and `activeSlice` top-level entries
/// and the delta never re-sends unchanged context references.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionCapsuleProjection {
    key: ExecutionCapsuleKey,
    digest: ArtifactDigest,
    value: Value,
    byte_length: u32,
}

impl ExecutionCapsuleProjection {
    /// Serializes the typed capsule into its canonical encoding and cache
    /// value, failing closed when the encoding exceeds the capsule budget.
    pub fn new(capsule: &ExecutionCapsuleV1) -> Result<Self, ContextProjectionError> {
        let encoded = serde_json::to_vec(capsule).map_err(ContextProjectionError::Canonicalize)?;
        let maximum = capsule.budgets().max_capsule_bytes();
        let byte_length = u32::try_from(encoded.len()).unwrap_or(u32::MAX);
        if byte_length > maximum {
            return Err(ContextProjectionError::BudgetExceeded {
                actual: byte_length,
                maximum,
            });
        }
        let digest = ArtifactDigest::digest(&encoded);
        let value =
            serde_json::from_slice(&encoded).map_err(ContextProjectionError::Canonicalize)?;
        Ok(Self {
            key: ExecutionCapsuleKey::from_capsule(capsule, digest),
            digest,
            value,
            byte_length,
        })
    }

    /// Returns the plan/queue/capsule digest binding for this projection.
    #[must_use]
    pub const fn key(&self) -> &ExecutionCapsuleKey {
        &self.key
    }

    /// Returns the digest of the canonical capsule encoding.
    #[must_use]
    pub const fn digest(&self) -> ArtifactDigest {
        self.digest
    }

    /// Returns the JSON projection body to feed into the context cache.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Returns the canonical capsule encoded length in bytes.
    #[must_use]
    pub const fn byte_length(&self) -> u32 {
        self.byte_length
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextDelta {
    kind: ProjectionKind,
    base_revision: ContextRevision,
    target_revision: ContextRevision,
    target_digest: ContextDigest,
    view: Option<ContextView>,
    changes: Option<ContextViewDelta>,
}

impl ContextDelta {
    #[must_use]
    pub const fn kind(&self) -> ProjectionKind {
        self.kind
    }

    #[must_use]
    pub const fn view(&self) -> Option<&ContextView> {
        self.view.as_ref()
    }

    #[must_use]
    pub const fn changes(&self) -> Option<&ContextViewDelta> {
        self.changes.as_ref()
    }

    #[must_use]
    pub const fn base_revision(&self) -> ContextRevision {
        self.base_revision
    }

    #[must_use]
    pub const fn target_revision(&self) -> ContextRevision {
        self.target_revision
    }

    #[must_use]
    pub const fn target_digest(&self) -> ContextDigest {
        self.target_digest
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeliverableContractDelta {
    Unchanged,
    Set(DeliverableContract),
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextViewDelta {
    summary: Option<Box<str>>,
    changed_input_refs: Vec<ArtifactRef>,
    removed_input_paths: Vec<ae_sdd_domain::ProjectRelativePath>,
    changed_constraint_refs: Vec<ArtifactRef>,
    removed_constraint_paths: Vec<ae_sdd_domain::ProjectRelativePath>,
    changed_memory_refs: Vec<RoleMemoryRef>,
    removed_memory_paths: Vec<ae_sdd_domain::ProjectRelativePath>,
    added_operations: BTreeSet<OperationId>,
    removed_operations: BTreeSet<OperationId>,
    added_paths: BTreeSet<ProjectPathScope>,
    removed_paths: BTreeSet<ProjectPathScope>,
    deliverable_contract: DeliverableContractDelta,
}

impl ContextViewDelta {
    fn between(previous: &ContextView, current: &ContextView) -> Self {
        let (changed_input_refs, removed_input_paths) =
            diff_artifacts(&previous.input_refs, &current.input_refs);
        let (changed_constraint_refs, removed_constraint_paths) =
            diff_artifacts(&previous.constraint_refs, &current.constraint_refs);
        let (changed_memory_refs, removed_memory_paths) =
            diff_memory(&previous.memory_refs, &current.memory_refs);
        Self {
            summary: (previous.summary != current.summary).then(|| current.summary.clone()),
            changed_input_refs,
            removed_input_paths,
            changed_constraint_refs,
            removed_constraint_paths,
            changed_memory_refs,
            removed_memory_paths,
            added_operations: current
                .allowed_operations
                .difference(&previous.allowed_operations)
                .cloned()
                .collect(),
            removed_operations: previous
                .allowed_operations
                .difference(&current.allowed_operations)
                .cloned()
                .collect(),
            added_paths: current
                .allowed_paths
                .difference(&previous.allowed_paths)
                .cloned()
                .collect(),
            removed_paths: previous
                .allowed_paths
                .difference(&current.allowed_paths)
                .cloned()
                .collect(),
            deliverable_contract: match (
                &previous.deliverable_contract,
                &current.deliverable_contract,
            ) {
                (left, right) if left == right => DeliverableContractDelta::Unchanged,
                (_, Some(contract)) => DeliverableContractDelta::Set(contract.clone()),
                (Some(_), None) => DeliverableContractDelta::Removed,
                (None, None) => DeliverableContractDelta::Unchanged,
            },
        }
    }

    #[must_use]
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    #[must_use]
    pub fn changed_input_refs(&self) -> &[ArtifactRef] {
        &self.changed_input_refs
    }

    #[must_use]
    pub fn removed_input_paths(&self) -> &[ae_sdd_domain::ProjectRelativePath] {
        &self.removed_input_paths
    }

    #[must_use]
    pub fn changed_constraint_refs(&self) -> &[ArtifactRef] {
        &self.changed_constraint_refs
    }

    #[must_use]
    pub fn removed_constraint_paths(&self) -> &[ae_sdd_domain::ProjectRelativePath] {
        &self.removed_constraint_paths
    }

    #[must_use]
    pub fn changed_memory_refs(&self) -> &[RoleMemoryRef] {
        &self.changed_memory_refs
    }

    #[must_use]
    pub fn removed_memory_paths(&self) -> &[ae_sdd_domain::ProjectRelativePath] {
        &self.removed_memory_paths
    }

    #[must_use]
    pub const fn added_operations(&self) -> &BTreeSet<OperationId> {
        &self.added_operations
    }

    #[must_use]
    pub const fn removed_operations(&self) -> &BTreeSet<OperationId> {
        &self.removed_operations
    }

    #[must_use]
    pub const fn added_paths(&self) -> &BTreeSet<ProjectPathScope> {
        &self.added_paths
    }

    #[must_use]
    pub const fn removed_paths(&self) -> &BTreeSet<ProjectPathScope> {
        &self.removed_paths
    }

    #[must_use]
    pub const fn deliverable_contract(&self) -> &DeliverableContractDelta {
        &self.deliverable_contract
    }

    fn estimated_bytes(&self) -> u32 {
        let mut bytes = self
            .summary
            .as_ref()
            .map_or(0_u64, |value| value.len() as u64);
        for reference in self
            .changed_input_refs
            .iter()
            .chain(self.changed_constraint_refs.iter())
            .chain(self.changed_memory_refs.iter().map(RoleMemoryRef::artifact))
        {
            bytes = bytes
                .saturating_add(reference.byte_length())
                .saturating_add(reference.path().as_str().len() as u64);
        }
        for path in self
            .removed_input_paths
            .iter()
            .chain(self.removed_constraint_paths.iter())
            .chain(self.removed_memory_paths.iter())
        {
            bytes = bytes.saturating_add(path.as_str().len() as u64);
        }
        for operation in self
            .added_operations
            .iter()
            .chain(self.removed_operations.iter())
        {
            bytes = bytes.saturating_add(operation.as_str().len() as u64);
        }
        for path in self.added_paths.iter().chain(self.removed_paths.iter()) {
            bytes = bytes.saturating_add(path_scope_len(path));
        }
        if let DeliverableContractDelta::Set(contract) = &self.deliverable_contract {
            bytes = bytes
                .saturating_add(u64::from(contract.max_result_bytes()))
                .saturating_add(u64::from(contract.max_summary_bytes()));
            for requirement in contract.required().values() {
                bytes = bytes
                    .saturating_add(requirement.id().as_str().len() as u64)
                    .saturating_add(requirement.kind().as_str().len() as u64)
                    .saturating_add(requirement.path().as_str().len() as u64);
            }
        }
        u32::try_from(bytes).unwrap_or(u32::MAX)
    }
}

fn canonicalize_artifact_refs(refs: &mut [ArtifactRef]) -> Result<(), ContextProjectionError> {
    refs.sort_by(|left, right| {
        left.path()
            .as_str()
            .cmp(right.path().as_str())
            .then_with(|| left.kind().as_str().cmp(right.kind().as_str()))
    });
    if refs.windows(2).any(|pair| pair[0].path() == pair[1].path()) {
        return Err(ContextProjectionError::DuplicateReferencePath);
    }
    Ok(())
}

fn diff_artifacts(
    previous: &[ArtifactRef],
    current: &[ArtifactRef],
) -> (Vec<ArtifactRef>, Vec<ae_sdd_domain::ProjectRelativePath>) {
    let previous_by_path = previous
        .iter()
        .map(|reference| (reference.path().as_str(), reference))
        .collect::<BTreeMap<_, _>>();
    let current_by_path = current
        .iter()
        .map(|reference| (reference.path().as_str(), reference))
        .collect::<BTreeMap<_, _>>();
    let changed = current
        .iter()
        .filter(|reference| previous_by_path.get(reference.path().as_str()) != Some(reference))
        .cloned()
        .collect();
    let removed = previous
        .iter()
        .filter(|reference| !current_by_path.contains_key(reference.path().as_str()))
        .map(|reference| reference.path().clone())
        .collect();
    (changed, removed)
}

fn diff_memory(
    previous: &[RoleMemoryRef],
    current: &[RoleMemoryRef],
) -> (Vec<RoleMemoryRef>, Vec<ae_sdd_domain::ProjectRelativePath>) {
    let previous_by_path = previous
        .iter()
        .map(|reference| (reference.artifact().path().as_str(), reference))
        .collect::<BTreeMap<_, _>>();
    let current_by_path = current
        .iter()
        .map(|reference| (reference.artifact().path().as_str(), reference))
        .collect::<BTreeMap<_, _>>();
    let changed = current
        .iter()
        .filter(|reference| {
            previous_by_path.get(reference.artifact().path().as_str()) != Some(reference)
        })
        .cloned()
        .collect();
    let removed = previous
        .iter()
        .filter(|reference| !current_by_path.contains_key(reference.artifact().path().as_str()))
        .map(|reference| reference.artifact().path().clone())
        .collect();
    (changed, removed)
}

fn path_scope_len(scope: &ProjectPathScope) -> u64 {
    match scope {
        ProjectPathScope::ProjectRoot => 1,
        ProjectPathScope::Subtree(path) => path.as_str().len() as u64,
    }
}

#[derive(Debug, Error)]
pub enum ContextProjectionError {
    #[error("projection budgets must be greater than zero")]
    InvalidBudget,
    #[error("context summary must not be empty")]
    EmptySummary,
    #[error("memory reference is not visible to the target session/delegation")]
    MemoryVisibilityViolation,
    #[error("context view contains duplicate artifact paths within one slice")]
    DuplicateReferencePath,
    #[error("context view identity does not match projection identity")]
    ViewIdentityMismatch,
    #[error("root projection cannot bind a delegation")]
    RootDelegationForbidden,
    #[error("child projection requires its trusted delegation")]
    ChildDelegationRequired,
    #[error("projection expiry must be greater than zero")]
    InvalidExpiry,
    #[error("context projection is {actual} bytes; maximum is {maximum}")]
    BudgetExceeded { actual: u32, maximum: u32 },
    #[error("context projection delta is {actual} bytes; maximum is {maximum}")]
    DeltaBudgetExceeded { actual: u32, maximum: u32 },
    #[error("known context revision is ahead of the current projection")]
    RevisionStale,
    #[error("failed to canonicalize context projection: {0}")]
    Canonicalize(serde_json::Error),
}

#[allow(clippy::too_many_arguments)]
fn canonical_bytes(
    projection_id: ContextProjectionId,
    session_id: SessionId,
    delegation_id: Option<DelegationId>,
    role: AgentRole,
    source_revision: StateRevision,
    context_revision: ContextRevision,
    policy_digest: PolicyDigest,
    inventory_generation: InventoryGeneration,
    view: &ContextView,
    expires_at_unix_ms: u64,
) -> Result<Vec<u8>, ContextProjectionError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Artifact<'a> {
        kind: &'a str,
        path: &'a str,
        digest: String,
        byte_length: u64,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Canonical<'a> {
        schema_version: u16,
        projection_id: String,
        session_id: String,
        delegation_id: Option<String>,
        role: &'static str,
        source_revision: u64,
        context_revision: u64,
        policy_digest: String,
        inventory_generation: u64,
        summary: &'a str,
        input_refs: Vec<Artifact<'a>>,
        constraint_refs: Vec<Artifact<'a>>,
        memory_refs: Vec<Artifact<'a>>,
        allowed_operations: Vec<&'a str>,
        allowed_paths: Vec<String>,
        has_deliverable_contract: bool,
        expires_at_unix_ms: u64,
    }

    fn refs(items: &[ArtifactRef]) -> Vec<Artifact<'_>> {
        items
            .iter()
            .map(|item| Artifact {
                kind: item.kind().as_str(),
                path: item.path().as_str(),
                digest: item.digest().to_hex(),
                byte_length: item.byte_length(),
            })
            .collect()
    }
    fn memory_refs(items: &[RoleMemoryRef]) -> Vec<Artifact<'_>> {
        items
            .iter()
            .map(RoleMemoryRef::artifact)
            .map(|item| Artifact {
                kind: item.kind().as_str(),
                path: item.path().as_str(),
                digest: item.digest().to_hex(),
                byte_length: item.byte_length(),
            })
            .collect()
    }
    fn path(scope: &ProjectPathScope) -> String {
        match scope {
            ProjectPathScope::ProjectRoot => ".".to_owned(),
            ProjectPathScope::Subtree(value) => value.as_str().to_owned(),
        }
    }

    serde_json::to_vec(&Canonical {
        schema_version: 1,
        projection_id: projection_id.to_string(),
        session_id: session_id.to_string(),
        delegation_id: delegation_id.map(|value| value.to_string()),
        role: match role {
            AgentRole::Root => "root",
            AgentRole::Series => "series",
            AgentRole::Task => "task",
            AgentRole::Reviewer => "reviewer",
        },
        source_revision: source_revision.get(),
        context_revision: context_revision.get(),
        policy_digest: policy_digest.to_hex(),
        inventory_generation: inventory_generation.get(),
        summary: view.summary(),
        input_refs: refs(view.input_refs()),
        constraint_refs: refs(view.constraint_refs()),
        memory_refs: memory_refs(view.memory_refs()),
        allowed_operations: view
            .allowed_operations
            .iter()
            .map(OperationId::as_str)
            .collect(),
        allowed_paths: view.allowed_paths.iter().map(path).collect(),
        has_deliverable_contract: view.deliverable_contract.is_some(),
        expires_at_unix_ms,
    })
    .map_err(ContextProjectionError::Canonicalize)
}
