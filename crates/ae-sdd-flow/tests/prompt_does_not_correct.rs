mod support;

use ae_sdd_domain::AgentRole;
use ae_sdd_flow::{FlowEventKind, FlowRuntime, NextAction};

use support::{event, input_with_corrections, transition_request};

#[test]
fn prompts_and_prompt_duplicates_never_increment_business_correction() {
    let prompt = event(10, b"prompt-10", FlowEventKind::PromptAccepted);
    let start = FlowRuntime::start(input_with_corrections(4));
    let once = FlowRuntime::apply(&start, &prompt).expect("prompt event is valid");
    let duplicate = FlowRuntime::apply(&once, &prompt).expect("duplicate is a no-op");
    let later = FlowRuntime::apply(
        &duplicate,
        &event(12, b"prompt-12", FlowEventKind::PromptAccepted),
    )
    .expect("later prompt is valid");

    assert_eq!(once.snapshot().correction_count(), 4);
    assert_eq!(duplicate.decision_digest(), once.decision_digest());
    assert_eq!(later.snapshot().correction_count(), 4);
    assert_eq!(later.next_action(), &NextAction::AwaitAgentWork);
}

#[test]
fn prompt_does_not_replace_pending_flow_action() {
    let pending = FlowRuntime::replay(
        input_with_corrections(0),
        [transition_request(1, AgentRole::Root)],
    )
    .expect("root transition request is valid");
    let action_before = pending.next_action().clone();
    let after_prompt = FlowRuntime::apply(
        &pending,
        &event(2, b"prompt-while-pending", FlowEventKind::PromptAccepted),
    )
    .expect("prompt is a valid signal");

    assert_eq!(after_prompt.next_action(), &action_before);
    assert_eq!(
        after_prompt.pending_transition(),
        pending.pending_transition()
    );
    assert_eq!(after_prompt.snapshot().correction_count(), 0);
}
