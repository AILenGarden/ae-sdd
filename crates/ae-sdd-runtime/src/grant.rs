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
    ScopedGrantWire::from_domain(&ScopedGrant::new(
        operations,
        [],
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
    Ok(domain)
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
        OperationName::ReviewRecord => RoleOperation::SubmitReviewFindings,
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
}
