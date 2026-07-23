use std::collections::BTreeSet;

use ae_sdd_domain::{
    AgentRole, ArtifactRef, ContextDigest, ContextProjectionId, ContextRevision, DelegationId,
    DeliverableContract, InventoryGeneration, OperationId, PolicyDigest, ProjectPathScope,
    SessionId, StateRevision,
};
use serde::Serialize;
use thiserror::Error;

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
        input_refs: Vec<ArtifactRef>,
        constraint_refs: Vec<ArtifactRef>,
        memory_refs: Vec<RoleMemoryRef>,
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
            });
        }
        let can_delta = previous.is_some_and(|old| {
            old.context_revision == known_revision
                && old.digest == known_digest
                && old.session_id == self.session_id
                && old.role == self.role
                && old.delegation_id == self.delegation_id
        });
        Ok(ContextDelta {
            kind: if can_delta {
                ProjectionKind::Delta
            } else {
                ProjectionKind::Full
            },
            base_revision: known_revision,
            target_revision: self.context_revision,
            target_digest: self.digest,
            view: Some(self.view.clone()),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionKind {
    Full,
    Delta,
    NoChange,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextDelta {
    kind: ProjectionKind,
    base_revision: ContextRevision,
    target_revision: ContextRevision,
    target_digest: ContextDigest,
    view: Option<ContextView>,
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

#[derive(Debug, Error)]
pub enum ContextProjectionError {
    #[error("projection budgets must be greater than zero")]
    InvalidBudget,
    #[error("context summary must not be empty")]
    EmptySummary,
    #[error("memory reference is not visible to the target session/delegation")]
    MemoryVisibilityViolation,
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
