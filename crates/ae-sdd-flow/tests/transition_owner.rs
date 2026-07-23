mod support;

use ae_sdd_domain::AgentRole;
use ae_sdd_flow::{FlowRuntime, NextAction};

use support::{input, transition_request};

#[test]
fn root_is_the_only_agent_role_that_can_request_global_transition() {
    let root = FlowRuntime::replay(input(), [transition_request(1, AgentRole::Root)])
        .expect("root intent reduces");
    assert!(matches!(
        root.next_action(),
        NextAction::EvaluateGates { .. }
    ));
    assert!(root.pending_transition().is_some());

    for role in [AgentRole::Series, AgentRole::Task, AgentRole::Reviewer] {
        let child = FlowRuntime::replay(input(), [transition_request(1, role)])
            .expect("denied child intent is a decision, not a reducer error");
        assert!(matches!(
            child.next_action(),
            NextAction::TransitionDenied { .. }
        ));
        assert_eq!(child.pending_transition(), None);
        assert_eq!(child.snapshot().correction_count(), 0);
    }
}
