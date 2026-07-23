mod support;

use ae_sdd_flow::{FlowError, FlowEventKind, FlowRuntime};

use support::{event, input};

#[test]
fn exact_duplicate_is_noop_and_sequence_reuse_conflicts() {
    let original = event(5, b"original", FlowEventKind::PromptAccepted);
    let checkpoint =
        FlowRuntime::replay(input(), [original.clone()]).expect("original event is valid");
    let duplicate = FlowRuntime::apply(&checkpoint, &original).expect("exact duplicate is a no-op");
    assert_eq!(duplicate, checkpoint);

    let conflicting = event(5, b"mutated", FlowEventKind::PromptAccepted);
    assert!(matches!(
        FlowRuntime::apply(&checkpoint, &conflicting),
        Err(FlowError::EventSequenceConflict { .. })
    ));
}

#[test]
fn batch_replay_rejects_conflicting_duplicate_even_when_reordered() {
    let first = event(9, b"first", FlowEventKind::PromptAccepted);
    let conflicting = event(9, b"second", FlowEventKind::PromptAccepted);

    assert!(matches!(
        FlowRuntime::replay(input(), [conflicting, first]),
        Err(FlowError::EventSequenceConflict { .. })
    ));
}
