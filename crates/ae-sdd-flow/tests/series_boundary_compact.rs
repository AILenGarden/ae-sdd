mod support;

use ae_sdd_domain::{AgentRole, CompletionDigestSet, CompletionMilestone, StateRevision};
use ae_sdd_flow::{
    CompactAdviceReason, FlowEventKind, FlowInput, FlowRuntime, FlowSnapshot, NextAction,
};

use support::{event, input, transition_request};

#[test]
fn collected_series_suggests_compact_at_an_idle_boundary() {
    let decision = FlowRuntime::replay(
        input(),
        [event(
            1,
            b"series-collected",
            FlowEventKind::SeriesCompleted,
        )],
    )
    .expect("series boundary reduces");

    assert_eq!(
        decision.next_action(),
        &NextAction::SuggestCompact {
            reason: CompactAdviceReason::SeriesBoundary,
        }
    );
}

#[test]
fn compact_advice_yields_to_a_pending_transition() {
    let decision = FlowRuntime::replay(
        input(),
        [
            transition_request(1, AgentRole::Root),
            event(2, b"series-collected", FlowEventKind::SeriesCompleted),
        ],
    )
    .expect("series boundary reduces");

    assert!(matches!(
        decision.next_action(),
        NextAction::EvaluateGates { .. }
    ));
    assert!(decision.pending_transition().is_some());
}

#[test]
fn compact_advice_yields_to_completion_chain_work() {
    let input = completion_input();
    let decision = FlowRuntime::replay(
        input,
        [event(
            1,
            b"series-collected",
            FlowEventKind::SeriesCompleted,
        )],
    )
    .expect("series boundary reduces");

    assert_eq!(
        decision.next_action(),
        &NextAction::FinalizeExecutionEvidence
    );
}

#[test]
fn compact_advice_is_replay_deterministic() {
    let events = [event(
        1,
        b"series-collected",
        FlowEventKind::SeriesCompleted,
    )];

    let first = FlowRuntime::replay(input(), events.clone()).expect("first replay");
    let replay = FlowRuntime::replay(input(), events).expect("second replay");

    assert_eq!(first, replay);
    assert_eq!(first.decision_digest(), replay.decision_digest());
}

fn completion_input() -> FlowInput {
    let base = input();
    let snapshot = FlowSnapshot::new(
        base.snapshot().phase(),
        StateRevision::new(7),
        base.snapshot().correction_count(),
    )
    .with_completion_milestone(
        CompletionMilestone::ImplementationVerified,
        CompletionDigestSet::ZERO,
    );
    FlowInput::new(snapshot, base.environment())
}
