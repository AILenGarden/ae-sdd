use std::{error::Error, fmt};

use ae_sdd_domain::AgentRole;

/// A semantic operation governed by the daemon-derived Agent role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoleOperation {
    /// Select the work-item route.
    SelectRoute,
    /// Request a global process transition.
    RequestGlobalTransition,
    /// Acquire, renew, or release the lease owned by this physical session.
    ManageOwnLease,
    /// Approve the compact execution plan.
    ApproveExecutionPlan,
    /// Create a series-level delegation.
    CreateSeriesDelegation,
    /// Create a task-level delegation.
    CreateTaskDelegation,
    /// Create an independent reviewer delegation.
    CreateReviewerDelegation,
    /// Collect a validated bounded child result.
    CollectChildResult,
    /// Report bounded progress to the parent session.
    ReportProgress,
    /// Read only the bounded projection prepared for an orchestration role.
    ReadBoundedProjection,
    /// Read artifacts included in the scoped grant.
    ReadAuthorizedArtifacts,
    /// Submit a bounded child result.
    SubmitChildResult,
    /// Modify implementation paths included in the scoped grant.
    ModifyAssignedPaths,
    /// Run tests included in the assignment.
    RunAssignedTests,
    /// Submit verification evidence for assigned work.
    SubmitEvidence,
    /// Review the authorized diff and evidence in an independent session.
    ReviewAssignedDiff,
    /// Submit reviewer findings and status.
    SubmitReviewFindings,
    /// Break a project lease; no Agent role receives this administrative grant.
    BreakLease,
}

/// Stateless role permission policy.
#[derive(Clone, Copy, Debug, Default)]
pub struct RolePolicy;

impl RolePolicy {
    /// Returns whether a daemon-trusted role may perform the semantic operation.
    pub const fn permits(role: AgentRole, operation: RoleOperation) -> bool {
        match role {
            AgentRole::Root => matches!(
                operation,
                RoleOperation::SelectRoute
                    | RoleOperation::RequestGlobalTransition
                    | RoleOperation::ManageOwnLease
                    | RoleOperation::ApproveExecutionPlan
                    | RoleOperation::CreateSeriesDelegation
                    | RoleOperation::CollectChildResult
                    | RoleOperation::ReportProgress
                    | RoleOperation::ReadBoundedProjection
                    | RoleOperation::ModifyAssignedPaths
                    | RoleOperation::RunAssignedTests
                    | RoleOperation::SubmitEvidence
            ),
            AgentRole::Series => matches!(
                operation,
                RoleOperation::CreateTaskDelegation
                    | RoleOperation::CreateReviewerDelegation
                    | RoleOperation::ManageOwnLease
                    | RoleOperation::CollectChildResult
                    | RoleOperation::ReportProgress
                    | RoleOperation::ReadBoundedProjection
                    | RoleOperation::ReadAuthorizedArtifacts
                    | RoleOperation::SubmitChildResult
            ),
            AgentRole::Task => matches!(
                operation,
                RoleOperation::ReadAuthorizedArtifacts
                    | RoleOperation::ManageOwnLease
                    | RoleOperation::SubmitChildResult
                    | RoleOperation::ModifyAssignedPaths
                    | RoleOperation::RunAssignedTests
                    | RoleOperation::SubmitEvidence
            ),
            AgentRole::Reviewer => matches!(
                operation,
                RoleOperation::ReadAuthorizedArtifacts
                    | RoleOperation::ManageOwnLease
                    | RoleOperation::SubmitChildResult
                    | RoleOperation::ReviewAssignedDiff
                    | RoleOperation::SubmitReviewFindings
            ),
        }
    }

    /// Authorizes an operation or returns a typed denial.
    pub const fn authorize(
        role: AgentRole,
        operation: RoleOperation,
    ) -> Result<(), RoleAuthorizationError> {
        if Self::permits(role, operation) {
            Ok(())
        } else {
            Err(RoleAuthorizationError { role, operation })
        }
    }
}

/// Stable internal denial produced by the role permission matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoleAuthorizationError {
    role: AgentRole,
    operation: RoleOperation,
}

impl RoleAuthorizationError {
    /// Returns the daemon-trusted role that was denied.
    pub const fn role(self) -> AgentRole {
        self.role
    }

    /// Returns the requested semantic operation.
    pub const fn operation(self) -> RoleOperation {
        self.operation
    }
}

impl fmt::Display for RoleAuthorizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "role {:?} is not permitted to perform {:?}",
            self.role, self.operation
        )
    }
}

impl Error for RoleAuthorizationError {}
