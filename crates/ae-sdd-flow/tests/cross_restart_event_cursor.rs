mod support;

use ae_sdd_flow::{FlowError, FlowEventKind, FlowRuntime};

use support::{event, event_in_store, input, other_event_store};

#[test]
fn durable_cursor_continues_across_runtime_restart() {
    let first = FlowRuntime::replay(
        input(),
        [event(41, b"event-41", FlowEventKind::PromptAccepted)],
    )
    .expect("first runtime checkpoint is valid");

    let restored_checkpoint = first.clone();
    let after_restart = FlowRuntime::apply(
        &restored_checkpoint,
        &event(88, b"event-88", FlowEventKind::PromptAccepted),
    )
    .expect("global cursor continues across process restart");

    assert_eq!(
        after_restart
            .last_cursor()
            .expect("event was applied")
            .sequence()
            .get(),
        88
    );
}

#[test]
fn rebuilt_event_store_epoch_rejects_old_checkpoint() {
    let checkpoint = FlowRuntime::start(input());
    let error = FlowRuntime::apply(
        &checkpoint,
        &event_in_store(
            other_event_store(),
            1,
            b"other-store",
            FlowEventKind::PromptAccepted,
        ),
    )
    .expect_err("event store epoch mismatch must fail closed");

    assert!(matches!(error, FlowError::EventStoreMismatch { .. }));
}
