use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{
    ArtifactKind, CapabilityId, DelegationId, DeliverableId, OperationId, ProjectRelativePath,
    SessionId,
};

pub const MAX_DELEGATION_DEPTH: u8 = 2;
pub const DEFAULT_CHILD_RESULT_MAX_BYTES: u32 = 65_536;
pub const DEFAULT_CHILD_SUMMARY_MAX_BYTES: u32 = 8_192;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AgentRole {
    Root,
    Series,
    Task,
    Reviewer,
}

impl AgentRole {
    pub const fn depth(self) -> u8 {
        match self {
            Self::Root => 0,
            Self::Series => 1,
            Self::Task | Self::Reviewer => 2,
        }
    }

    pub const fn may_spawn(self, child: Self) -> bool {
        matches!(
            (self, child),
            (Self::Root, Self::Series) | (Self::Series, Self::Task | Self::Reviewer)
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AgentIdentity {
    session_id: SessionId,
    role: AgentRole,
}

impl AgentIdentity {
    pub const fn new(session_id: SessionId, role: AgentRole) -> Self {
        Self { session_id, role }
    }

    pub const fn session_id(self) -> SessionId {
        self.session_id
    }

    pub const fn role(self) -> AgentRole {
        self.role
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineageNode {
    identity: AgentIdentity,
    via_delegation: Option<DelegationId>,
}

impl LineageNode {
    pub const fn identity(self) -> AgentIdentity {
        self.identity
    }

    pub const fn via_delegation(self) -> Option<DelegationId> {
        self.via_delegation
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentLineage {
    nodes: Vec<LineageNode>,
}

impl AgentLineage {
    pub fn root(session_id: SessionId) -> Self {
        Self {
            nodes: vec![LineageNode {
                identity: AgentIdentity::new(session_id, AgentRole::Root),
                via_delegation: None,
            }],
        }
    }

    pub fn depth(&self) -> u8 {
        u8::try_from(self.nodes.len() - 1).expect("lineage is bounded to three nodes")
    }

    pub fn current(&self) -> AgentIdentity {
        self.nodes
            .last()
            .expect("AgentLineage always contains its root")
            .identity
    }

    pub fn root_identity(&self) -> AgentIdentity {
        self.nodes[0].identity
    }

    pub fn nodes(&self) -> &[LineageNode] {
        &self.nodes
    }

    pub fn spawn_child(
        &self,
        session_id: SessionId,
        delegation_id: DelegationId,
        child_role: AgentRole,
    ) -> Result<Self, LineageError> {
        let parent = self.current();
        if self.depth() >= MAX_DELEGATION_DEPTH || !parent.role().may_spawn(child_role) {
            return Err(LineageError::RoleTransitionDenied {
                parent: parent.role(),
                child: child_role,
            });
        }
        if child_role.depth() != self.depth() + 1 {
            return Err(LineageError::DepthMismatch {
                role: child_role,
                expected: child_role.depth(),
                actual: self.depth() + 1,
            });
        }
        if self
            .nodes
            .iter()
            .any(|node| node.identity.session_id() == session_id)
        {
            return Err(LineageError::DuplicateSession(session_id));
        }
        if self
            .nodes
            .iter()
            .any(|node| node.via_delegation == Some(delegation_id))
        {
            return Err(LineageError::DuplicateDelegation(delegation_id));
        }

        let mut nodes = self.nodes.clone();
        nodes.push(LineageNode {
            identity: AgentIdentity::new(session_id, child_role),
            via_delegation: Some(delegation_id),
        });
        Ok(Self { nodes })
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LineageError {
    #[error("{parent:?} cannot spawn {child:?}")]
    RoleTransitionDenied { parent: AgentRole, child: AgentRole },
    #[error("{role:?} must appear at depth {expected}, not {actual}")]
    DepthMismatch {
        role: AgentRole,
        expected: u8,
        actual: u8,
    },
    #[error("session {0} already appears in this lineage")]
    DuplicateSession(SessionId),
    #[error("delegation {0} already appears in this lineage")]
    DuplicateDelegation(DelegationId),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProjectPathScope {
    ProjectRoot,
    Subtree(ProjectRelativePath),
}

impl ProjectPathScope {
    pub fn contains(&self, child: &Self) -> bool {
        match (self, child) {
            (Self::ProjectRoot, _) => true,
            (Self::Subtree(_), Self::ProjectRoot) => false,
            (Self::Subtree(parent), Self::Subtree(child)) => parent.contains(child),
        }
    }

    pub fn contains_path(&self, path: &ProjectRelativePath) -> bool {
        match self {
            Self::ProjectRoot => true,
            Self::Subtree(parent) => parent.contains(path),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScopedGrant {
    operations: BTreeSet<OperationId>,
    capabilities: BTreeSet<CapabilityId>,
    paths: BTreeSet<ProjectPathScope>,
}

impl ScopedGrant {
    pub fn new(
        operations: impl IntoIterator<Item = OperationId>,
        capabilities: impl IntoIterator<Item = CapabilityId>,
        paths: impl IntoIterator<Item = ProjectPathScope>,
    ) -> Self {
        Self {
            operations: operations.into_iter().collect(),
            capabilities: capabilities.into_iter().collect(),
            paths: paths.into_iter().collect(),
        }
    }

    pub fn operations(&self) -> &BTreeSet<OperationId> {
        &self.operations
    }

    pub fn capabilities(&self) -> &BTreeSet<CapabilityId> {
        &self.capabilities
    }

    pub fn paths(&self) -> &BTreeSet<ProjectPathScope> {
        &self.paths
    }

    pub fn permits_path(&self, path: &ProjectRelativePath) -> bool {
        self.paths.iter().any(|scope| scope.contains_path(path))
    }

    pub fn validate_child(&self, child: &Self) -> Result<(), GrantViolation> {
        if let Some(operation) = child.operations.difference(&self.operations).next() {
            return Err(GrantViolation::OperationNotGranted(operation.clone()));
        }
        if let Some(capability) = child.capabilities.difference(&self.capabilities).next() {
            return Err(GrantViolation::CapabilityNotGranted(capability.clone()));
        }
        if let Some(path) = child
            .paths
            .iter()
            .find(|child_path| !self.paths.iter().any(|parent| parent.contains(child_path)))
        {
            return Err(GrantViolation::PathNotGranted(path.clone()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum GrantViolation {
    #[error("operation {0} is not present in the parent grant")]
    OperationNotGranted(OperationId),
    #[error("capability {0} is not present in the parent grant")]
    CapabilityNotGranted(CapabilityId),
    #[error("path scope {0:?} is not contained by the parent grant")]
    PathNotGranted(ProjectPathScope),
    #[error("required deliverable {0} is outside the grant's path scope")]
    DeliverableOutsideScope(ProjectRelativePath),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliverableRequirement {
    id: DeliverableId,
    kind: ArtifactKind,
    path: ProjectRelativePath,
}

impl DeliverableRequirement {
    pub const fn new(id: DeliverableId, kind: ArtifactKind, path: ProjectRelativePath) -> Self {
        Self { id, kind, path }
    }

    pub const fn id(&self) -> &DeliverableId {
        &self.id
    }

    pub const fn kind(&self) -> &ArtifactKind {
        &self.kind
    }

    pub const fn path(&self) -> &ProjectRelativePath {
        &self.path
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliverableContract {
    required: BTreeMap<DeliverableId, DeliverableRequirement>,
    max_result_bytes: u32,
    max_summary_bytes: u32,
}

impl DeliverableContract {
    pub fn new(
        required: impl IntoIterator<Item = DeliverableRequirement>,
        max_result_bytes: u32,
        max_summary_bytes: u32,
    ) -> Result<Self, DeliverableContractError> {
        if max_result_bytes == 0 {
            return Err(DeliverableContractError::ZeroResultBudget);
        }
        if max_summary_bytes == 0 || max_summary_bytes > max_result_bytes {
            return Err(DeliverableContractError::InvalidSummaryBudget {
                summary: max_summary_bytes,
                result: max_result_bytes,
            });
        }

        let mut required_by_id = BTreeMap::new();
        let mut paths = BTreeSet::new();
        for requirement in required {
            if required_by_id.contains_key(requirement.id()) {
                return Err(DeliverableContractError::DuplicateId(
                    requirement.id().clone(),
                ));
            }
            if !paths.insert(requirement.path().clone()) {
                return Err(DeliverableContractError::DuplicatePath(
                    requirement.path().clone(),
                ));
            }
            required_by_id.insert(requirement.id().clone(), requirement);
        }

        Ok(Self {
            required: required_by_id,
            max_result_bytes,
            max_summary_bytes,
        })
    }

    pub fn bounded_default(
        required: impl IntoIterator<Item = DeliverableRequirement>,
    ) -> Result<Self, DeliverableContractError> {
        Self::new(
            required,
            DEFAULT_CHILD_RESULT_MAX_BYTES,
            DEFAULT_CHILD_SUMMARY_MAX_BYTES,
        )
    }

    pub fn required(&self) -> &BTreeMap<DeliverableId, DeliverableRequirement> {
        &self.required
    }

    pub const fn max_result_bytes(&self) -> u32 {
        self.max_result_bytes
    }

    pub const fn max_summary_bytes(&self) -> u32 {
        self.max_summary_bytes
    }

    pub fn validate_scope(&self, grant: &ScopedGrant) -> Result<(), GrantViolation> {
        if let Some(requirement) = self
            .required
            .values()
            .find(|requirement| !grant.permits_path(requirement.path()))
        {
            return Err(GrantViolation::DeliverableOutsideScope(
                requirement.path().clone(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DeliverableContractError {
    #[error("child result byte budget must be greater than zero")]
    ZeroResultBudget,
    #[error("summary budget {summary} must be in 1..={result}")]
    InvalidSummaryBudget { summary: u32, result: u32 },
    #[error("deliverable ID {0} is duplicated")]
    DuplicateId(DeliverableId),
    #[error("deliverable path {0} is duplicated")]
    DuplicatePath(ProjectRelativePath),
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use uuid::Uuid;

    use super::*;

    fn session(seed: u128) -> SessionId {
        SessionId::from_uuid(Uuid::from_u128(seed))
    }

    fn delegation(seed: u128) -> DelegationId {
        DelegationId::from_uuid(Uuid::from_u128(seed))
    }

    fn operation(value: &str) -> OperationId {
        OperationId::new(value).expect("test operation is valid")
    }

    fn capability(value: &str) -> CapabilityId {
        CapabilityId::new(value).expect("test capability is valid")
    }

    fn subtree(value: &str) -> ProjectPathScope {
        ProjectPathScope::Subtree(ProjectRelativePath::new(value).expect("test path is valid"))
    }

    #[test]
    fn root_series_task_or_reviewer_max_depth_is_enforced() {
        let root = AgentLineage::root(session(1));
        let series = root
            .spawn_child(session(2), delegation(11), AgentRole::Series)
            .expect("root may spawn series");
        let task = series
            .spawn_child(session(3), delegation(12), AgentRole::Task)
            .expect("series may spawn task");
        let reviewer = series
            .spawn_child(session(4), delegation(13), AgentRole::Reviewer)
            .expect("series may spawn reviewer");

        assert_eq!(task.depth(), MAX_DELEGATION_DEPTH);
        assert_eq!(reviewer.depth(), MAX_DELEGATION_DEPTH);
        assert!(
            task.spawn_child(session(5), delegation(14), AgentRole::Task)
                .is_err()
        );
        assert!(
            root.spawn_child(session(6), delegation(15), AgentRole::Task)
                .is_err()
        );
        assert_ne!(task.current().session_id(), reviewer.current().session_id());
    }

    #[test]
    fn child_scope_cannot_widen_parent_operations_or_paths() {
        let parent = ScopedGrant::new(
            [operation("artifact.read"), operation("artifact.write")],
            [capability("host.report")],
            [subtree("crates/ae-sdd-domain")],
        );
        let valid_child = ScopedGrant::new(
            [operation("artifact.read")],
            [capability("host.report")],
            [subtree("crates/ae-sdd-domain/src")],
        );
        let wider_operation = ScopedGrant::new(
            [operation("state.transition")],
            [capability("host.report")],
            [subtree("crates/ae-sdd-domain/src")],
        );
        let wider_path = ScopedGrant::new(
            [operation("artifact.read")],
            [capability("host.report")],
            [subtree("crates")],
        );

        assert_eq!(parent.validate_child(&valid_child), Ok(()));
        assert!(matches!(
            parent.validate_child(&wider_operation),
            Err(GrantViolation::OperationNotGranted(_))
        ));
        assert!(matches!(
            parent.validate_child(&wider_path),
            Err(GrantViolation::PathNotGranted(_))
        ));
    }

    proptest! {
        #[test]
        fn child_scope_narrowing_is_transitive(
            leaf_a in "[a-z][a-z0-9]{0,12}",
            leaf_b in "[a-z][a-z0-9]{0,12}",
        ) {
            let root = ScopedGrant::new(
                [operation("artifact.read")],
                [capability("host.report")],
                [ProjectPathScope::ProjectRoot],
            );
            let series_path = format!("crates/{leaf_a}");
            let task_path = format!("{series_path}/{leaf_b}");
            let series = ScopedGrant::new(
                [operation("artifact.read")],
                [capability("host.report")],
                [subtree(&series_path)],
            );
            let task = ScopedGrant::new(
                [operation("artifact.read")],
                [capability("host.report")],
                [subtree(&task_path)],
            );

            prop_assert_eq!(root.validate_child(&series), Ok(()));
            prop_assert_eq!(series.validate_child(&task), Ok(()));
            prop_assert_eq!(root.validate_child(&task), Ok(()));
        }
    }

    #[test]
    fn deliverable_contract_is_bounded_and_scoped() {
        let requirement = DeliverableRequirement::new(
            DeliverableId::new("domain-source").expect("valid deliverable ID"),
            ArtifactKind::new("rust-source").expect("valid artifact kind"),
            ProjectRelativePath::new("crates/ae-sdd-domain/src/lib.rs").expect("valid path"),
        );
        let contract =
            DeliverableContract::bounded_default([requirement]).expect("bounded contract is valid");
        let allowed = ScopedGrant::new([], [], [subtree("crates/ae-sdd-domain")]);
        let denied = ScopedGrant::new([], [], [subtree("crates/ae-sdd-flow")]);

        assert_eq!(contract.max_result_bytes(), 65_536);
        assert_eq!(contract.max_summary_bytes(), 8_192);
        assert_eq!(contract.validate_scope(&allowed), Ok(()));
        assert!(matches!(
            contract.validate_scope(&denied),
            Err(GrantViolation::DeliverableOutsideScope(_))
        ));
    }
}
