use std::collections::BTreeSet;
use std::str::FromStr;

use ae_sdd_domain::{
    AgentRole, CapabilityId, OperationId, ProjectPathScope, ProjectRelativePath, ScopedGrant,
};
use ae_sdd_operations::OperationName;
use ae_sdd_policy::{RoleOperation, RolePolicy};
use ae_sdd_protocol::StableErrorCode;
use serde::{Deserialize, Serialize};

use crate::{RuntimeError, RuntimeResult, WireAgentRole};

const MAX_GRANT_CAPABILITIES: usize = 64;
const MAX_GRANT_PATHS: usize = 64;
const REVIEW_SPECIALTY_CAPABILITIES: [&str; 4] = [
    "review.specialty.general",
    "review.specialty.be",
    "review.specialty.ar",
    "review.specialty.qa",
];

/// Stable wire form of one project path scope in an Agent grant.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GrantPathWire {
    /// The complete registered project root.
    ProjectRoot,
    /// One canonical project-relative subtree.
    Subtree {
        /// Canonical project-relative subtree path.
        path: String,
    },
}

/// Canonical, persistable representation of a daemon-authoritative scoped grant.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScopedGrantWire {
    /// Registered typed operations carried by the grant.
    pub operations: Vec<String>,
    /// Additional named runtime capabilities carried by the grant.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Project path scopes carried by the grant.
    #[serde(default)]
    pub paths: Vec<GrantPathWire>,
}

impl ScopedGrantWire {
    /// Converts a domain grant into its sorted canonical wire representation.
    #[must_use]
    pub fn from_domain(grant: &ScopedGrant) -> Self {
        Self {
            operations: grant
                .operations()
                .iter()
                .map(|operation| operation.as_str().to_owned())
                .collect(),
            capabilities: grant
                .capabilities()
                .iter()
                .map(|capability| capability.as_str().to_owned())
                .collect(),
            paths: grant
                .paths()
                .iter()
                .map(|path| match path {
                    ProjectPathScope::ProjectRoot => GrantPathWire::ProjectRoot,
                    ProjectPathScope::Subtree(path) => GrantPathWire::Subtree {
                        path: path.as_str().to_owned(),
                    },
                })
                .collect(),
        }
    }

    /// Validates and converts this wire grant to the domain representation.
    pub fn to_domain(&self) -> RuntimeResult<ScopedGrant> {
        if self.operations.len() > OperationName::ALL.len()
            || self.capabilities.len() > MAX_GRANT_CAPABILITIES
            || self.paths.len() > MAX_GRANT_PATHS
        {
            return Err(grant_error("scoped grant exceeds its bounded cardinality"));
        }

        let mut operations = BTreeSet::new();
        for value in &self.operations {
            OperationName::from_str(value)
                .map_err(|_| grant_error("scoped grant contains an unregistered operation"))?;
            let operation = OperationId::new(value.clone())
                .map_err(|_| grant_error("scoped grant operation identity is invalid"))?;
            if !operations.insert(operation) {
                return Err(grant_error("scoped grant contains a duplicate operation"));
            }
        }

        let mut capabilities = BTreeSet::new();
        for value in &self.capabilities {
            let capability = CapabilityId::new(value.clone())
                .map_err(|_| grant_error("scoped grant capability identity is invalid"))?;
            if !capabilities.insert(capability) {
                return Err(grant_error("scoped grant contains a duplicate capability"));
            }
        }

        let mut paths = BTreeSet::new();
        for value in &self.paths {
            let path = match value {
                GrantPathWire::ProjectRoot => ProjectPathScope::ProjectRoot,
                GrantPathWire::Subtree { path } => ProjectPathScope::Subtree(
                    ProjectRelativePath::new(path.clone())
                        .map_err(|_| grant_error("scoped grant path is not canonical"))?,
                ),
            };
            if !paths.insert(path) {
                return Err(grant_error("scoped grant contains a duplicate path scope"));
            }
        }

        Ok(ScopedGrant::new(operations, capabilities, paths))
    }

    /// Returns the normalized sorted wire representation after strict validation.
    pub fn normalized(&self) -> RuntimeResult<Self> {
        self.to_domain().map(|grant| Self::from_domain(&grant))
    }
}

pub(crate) fn root_grant() -> ScopedGrantWire {
    let operations = OperationName::ALL
        .into_iter()
        .filter(|operation| *operation != OperationName::LeaseBreak)
        .filter_map(|operation| OperationId::new(operation.as_str()).ok());
    let capabilities = REVIEW_SPECIALTY_CAPABILITIES
        .into_iter()
        .filter_map(|capability| CapabilityId::new(capability).ok());
    ScopedGrantWire::from_domain(&ScopedGrant::new(
        operations,
        capabilities,
        [ProjectPathScope::ProjectRoot],
    ))
}

pub(crate) fn validate_child_grant(
    parent: &ScopedGrant,
    child_role: WireAgentRole,
    requested: &ScopedGrantWire,
) -> RuntimeResult<ScopedGrantWire> {
    let normalized = requested.normalized()?;
    let child = normalized.to_domain()?;
    parent
        .validate_child(&child)
        .map_err(|_| grant_error("child scoped grant widens the parent grant"))?;
    for operation in child.operations() {
        let operation = OperationName::from_str(operation.as_str())
            .map_err(|_| grant_error("child scoped grant operation is not registered"))?;
        if !role_may_receive(child_role, operation) {
            return Err(grant_error(
                "child role cannot execute or delegate one of the requested operations",
            ));
        }
    }
    validate_role_capabilities(child_role, &normalized)?;
    Ok(normalized)
}

pub(crate) fn validate_session_grant(
    role: WireAgentRole,
    grant: &ScopedGrantWire,
) -> RuntimeResult<ScopedGrant> {
    let domain = grant.to_domain()?;
    if role == WireAgentRole::Root {
        if grant.normalized()? != root_grant() {
            return Err(grant_error("root session grant differs from daemon policy"));
        }
        return Ok(domain);
    }
    for operation in domain.operations() {
        let operation = OperationName::from_str(operation.as_str())
            .map_err(|_| grant_error("session scoped grant operation is not registered"))?;
        if !role_may_receive(role, operation) {
            return Err(grant_error(
                "session scoped grant is incompatible with its role",
            ));
        }
    }
    validate_role_capabilities(role, grant)?;
    Ok(domain)
}

fn validate_role_capabilities(role: WireAgentRole, grant: &ScopedGrantWire) -> RuntimeResult<()> {
    if grant
        .capabilities
        .iter()
        .any(|capability| !REVIEW_SPECIALTY_CAPABILITIES.contains(&capability.as_str()))
    {
        return Err(grant_error(
            "scoped grant contains a capability outside daemon policy",
        ));
    }
    let specialty_count = grant
        .capabilities
        .iter()
        .filter(|capability| REVIEW_SPECIALTY_CAPABILITIES.contains(&capability.as_str()))
        .count();
    match role {
        WireAgentRole::Root => {
            if grant.normalized()? != root_grant() {
                return Err(grant_error("root session grant differs from daemon policy"));
            }
        }
        WireAgentRole::Series => {}
        WireAgentRole::Task if specialty_count == 0 => {}
        WireAgentRole::Reviewer
            if specialty_count == 1
                && grant.operations.iter().any(|operation| {
                    operation == OperationName::ReviewContribute.as_str()
                        || operation == OperationName::ReviewRecord.as_str()
                }) => {}
        WireAgentRole::Task => {
            return Err(grant_error(
                "task grant cannot carry a review specialty capability",
            ));
        }
        WireAgentRole::Reviewer => {
            return Err(grant_error(
                "reviewer grant requires review.contribute and exactly one specialty capability",
            ));
        }
    }
    Ok(())
}

fn role_may_receive(role: WireAgentRole, operation: OperationName) -> bool {
    match role {
        WireAgentRole::Root => operation != OperationName::LeaseBreak,
        WireAgentRole::Series => {
            role_may_execute(AgentRole::Series, operation)
                || role_may_execute(AgentRole::Task, operation)
                || role_may_execute(AgentRole::Reviewer, operation)
        }
        WireAgentRole::Task => role_may_execute(AgentRole::Task, operation),
        WireAgentRole::Reviewer => role_may_execute(AgentRole::Reviewer, operation),
    }
}

fn role_may_execute(role: AgentRole, operation: OperationName) -> bool {
    RolePolicy::permits(role, semantic_operation(role, operation))
}

fn semantic_operation(role: AgentRole, operation: OperationName) -> RoleOperation {
    match operation {
        OperationName::StateTransition | OperationName::WorkItemComplete => {
            RoleOperation::RequestGlobalTransition
        }
        OperationName::ExecutionPlanApprove => RoleOperation::ApproveExecutionPlan,
        OperationName::ExecutionPlanSet => RoleOperation::SelectRoute,
        OperationName::DocumentSave => RoleOperation::ModifyAssignedPaths,
        OperationName::EvidenceRecord | OperationName::EvidenceFinalize => {
            RoleOperation::SubmitEvidence
        }
        OperationName::VerificationPlan => RoleOperation::RunAssignedTests,
        OperationName::ReviewContribute | OperationName::ReviewRecord => {
            RoleOperation::SubmitReviewFindings
        }
        OperationName::ReviewFinalize => RoleOperation::RequestGlobalTransition,
        OperationName::LeaseBreak => RoleOperation::BreakLease,
        OperationName::LeaseAcquire | OperationName::LeaseRenew | OperationName::LeaseRelease => {
            RoleOperation::ManageOwnLease
        }
        _ if matches!(role, AgentRole::Root | AgentRole::Series) => {
            RoleOperation::ReadBoundedProjection
        }
        _ => RoleOperation::ReadAuthorizedArtifacts,
    }
}

fn grant_error(message: &'static str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::RoleOperationForbidden, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_grant_must_narrow_parent_operations_and_paths() {
        let parent = ScopedGrant::new(
            [OperationId::new("document.save").expect("operation")],
            [],
            [ProjectPathScope::Subtree(
                ProjectRelativePath::new("crates").expect("path"),
            )],
        );
        let valid = ScopedGrantWire {
            operations: vec!["document.save".to_owned()],
            capabilities: Vec::new(),
            paths: vec![GrantPathWire::Subtree {
                path: "crates/ae-sdd-runtime".to_owned(),
            }],
        };
        assert!(validate_child_grant(&parent, WireAgentRole::Task, &valid).is_ok());

        let widened = ScopedGrantWire {
            paths: vec![GrantPathWire::ProjectRoot],
            ..valid
        };
        assert!(validate_child_grant(&parent, WireAgentRole::Task, &widened).is_err());
    }

    #[test]
    fn admin_only_lease_break_never_enters_an_agent_grant() {
        assert!(
            !root_grant()
                .operations
                .iter()
                .any(|operation| operation == "lease.break")
        );
        let requested = ScopedGrantWire {
            operations: vec!["lease.break".to_owned()],
            capabilities: Vec::new(),
            paths: Vec::new(),
        };
        assert!(
            validate_child_grant(
                &root_grant().to_domain().expect("root grant"),
                WireAgentRole::Series,
                &requested,
            )
            .is_err()
        );
    }

    #[test]
    fn root_and_series_can_narrow_to_one_reviewer_specialty() {
        let root = root_grant().to_domain().expect("root grant");
        for capability in REVIEW_SPECIALTY_CAPABILITIES {
            assert!(
                root_grant()
                    .capabilities
                    .iter()
                    .any(|value| value == capability)
            );
        }
        let series = ScopedGrantWire {
            operations: vec![
                "lease.acquire".to_owned(),
                "review.contribute".to_owned(),
                "review.record".to_owned(),
            ],
            capabilities: REVIEW_SPECIALTY_CAPABILITIES
                .into_iter()
                .map(str::to_owned)
                .collect(),
            paths: vec![GrantPathWire::ProjectRoot],
        };
        let series = validate_child_grant(&root, WireAgentRole::Series, &series)
            .expect("root narrows to a review-capable series");
        let reviewer = ScopedGrantWire {
            operations: vec![
                "lease.acquire".to_owned(),
                "review.contribute".to_owned(),
                "review.record".to_owned(),
            ],
            capabilities: vec!["review.specialty.general".to_owned()],
            paths: vec![GrantPathWire::ProjectRoot],
        };
        assert!(
            validate_child_grant(
                &series.to_domain().expect("series grant"),
                WireAgentRole::Reviewer,
                &reviewer,
            )
            .is_ok()
        );

        let missing_specialty = ScopedGrantWire {
            capabilities: Vec::new(),
            ..reviewer.clone()
        };
        assert!(
            validate_child_grant(
                &series.to_domain().expect("series grant"),
                WireAgentRole::Reviewer,
                &missing_specialty,
            )
            .is_err()
        );
        let multiple_specialties = ScopedGrantWire {
            capabilities: vec![
                "review.specialty.general".to_owned(),
                "review.specialty.qa".to_owned(),
            ],
            ..reviewer
        };
        assert!(
            validate_child_grant(
                &series.to_domain().expect("series grant"),
                WireAgentRole::Reviewer,
                &multiple_specialties,
            )
            .is_err()
        );
    }

    #[test]
    fn only_root_receives_review_finalize_and_reviewer_receives_contribute() {
        let root = root_grant();
        assert!(
            root.operations
                .iter()
                .any(|operation| operation == OperationName::ReviewFinalize.as_str()),
            "the root/finalizer grant carries review.finalize"
        );
        assert!(
            root.operations
                .iter()
                .any(|operation| operation == OperationName::ReviewContribute.as_str()),
            "the root grant carries review.contribute"
        );

        let reviewer = ScopedGrantWire {
            operations: vec![
                "lease.acquire".to_owned(),
                "review.contribute".to_owned(),
                "review.finalize".to_owned(),
            ],
            capabilities: vec!["review.specialty.general".to_owned()],
            paths: vec![GrantPathWire::ProjectRoot],
        };
        assert!(
            validate_child_grant(
                &root.to_domain().expect("root grant"),
                WireAgentRole::Reviewer,
                &reviewer
            )
            .is_err(),
            "a reviewer can never receive review.finalize"
        );
        let series = ScopedGrantWire {
            operations: vec!["review.finalize".to_owned()],
            capabilities: Vec::new(),
            paths: vec![GrantPathWire::ProjectRoot],
        };
        assert!(
            validate_child_grant(
                &root.to_domain().expect("root grant"),
                WireAgentRole::Series,
                &series
            )
            .is_err(),
            "a series can never receive review.finalize"
        );

        let contribute_only = ScopedGrantWire {
            operations: vec!["lease.acquire".to_owned(), "review.contribute".to_owned()],
            capabilities: vec!["review.specialty.general".to_owned()],
            paths: vec![GrantPathWire::ProjectRoot],
        };
        assert!(
            validate_child_grant(
                &root.to_domain().expect("root grant"),
                WireAgentRole::Reviewer,
                &contribute_only,
            )
            .is_ok(),
            "reviewer grant carries review.contribute and exactly one specialty"
        );
    }
}
