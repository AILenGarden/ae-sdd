use ae_sdd_domain::AgentRole;
use ae_sdd_policy::{RoleOperation, RolePolicy};

#[test]
fn only_root_owns_global_transition_and_plan_approval() {
    for role in [
        AgentRole::Root,
        AgentRole::Series,
        AgentRole::Task,
        AgentRole::Reviewer,
    ] {
        let expected = role == AgentRole::Root;
        assert_eq!(
            RolePolicy::permits(role, RoleOperation::RequestGlobalTransition),
            expected
        );
        assert_eq!(
            RolePolicy::permits(role, RoleOperation::ApproveExecutionPlan),
            expected
        );
    }
}

#[test]
fn task_and_reviewer_permissions_are_separated() {
    assert!(RolePolicy::permits(
        AgentRole::Task,
        RoleOperation::ModifyAssignedPaths
    ));
    assert!(!RolePolicy::permits(
        AgentRole::Reviewer,
        RoleOperation::ModifyAssignedPaths
    ));
    assert!(RolePolicy::permits(
        AgentRole::Reviewer,
        RoleOperation::ReviewAssignedDiff
    ));
    assert!(!RolePolicy::permits(
        AgentRole::Task,
        RoleOperation::ReviewAssignedDiff
    ));
}

#[test]
fn no_agent_role_may_break_a_lease() {
    for role in [
        AgentRole::Root,
        AgentRole::Series,
        AgentRole::Task,
        AgentRole::Reviewer,
    ] {
        assert!(!RolePolicy::permits(role, RoleOperation::BreakLease));
    }
}

#[test]
fn root_reads_bounded_projection_not_child_artifact_bodies() {
    assert!(RolePolicy::permits(
        AgentRole::Root,
        RoleOperation::ReadBoundedProjection
    ));
    assert!(!RolePolicy::permits(
        AgentRole::Root,
        RoleOperation::ReadAuthorizedArtifacts
    ));
}
