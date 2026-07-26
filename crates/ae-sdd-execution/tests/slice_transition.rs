//! Execution slice lifecycle transition truth-table tests.
//!
//! The legal path is `pending -> running -> red-observed -> patched ->
//! focused-green -> evidence-bound -> completed`; any non-terminal status may
//! move to `blocked` and only `Resumed` re-enters the loop at `running`.
//! Everything else — in particular `pending -> focused-green`,
//! `red-observed -> completed` and binding evidence before the focused
//! GREEN — must be rejected.

use ae_sdd_contracts::execution_runtime::ExecutionSliceStatus;
use ae_sdd_execution::{
    ExecutionSliceEvent, ExecutionSliceTransitionError, transition_slice_status,
};

use ExecutionSliceEvent as Event;
use ExecutionSliceStatus as Status;

const STATUSES: [Status; 8] = [
    Status::Pending,
    Status::Running,
    Status::RedObserved,
    Status::Patched,
    Status::FocusedGreen,
    Status::EvidenceBound,
    Status::Completed,
    Status::Blocked,
];

const EVENTS: [Event; 8] = [
    Event::Claimed,
    Event::RedObserved,
    Event::PatchApplied,
    Event::FocusedTestGreen,
    Event::EvidenceBound,
    Event::Completed,
    Event::Blocked,
    Event::Resumed,
];

/// The one authoritative transition table the reducer must enforce.
fn expected_next(from: Status, event: Event) -> Option<Status> {
    match (from, event) {
        (Status::Pending, Event::Claimed) => Some(Status::Running),
        (Status::Running, Event::RedObserved) => Some(Status::RedObserved),
        (Status::RedObserved, Event::PatchApplied) => Some(Status::Patched),
        (Status::Patched, Event::FocusedTestGreen) => Some(Status::FocusedGreen),
        (Status::FocusedGreen, Event::EvidenceBound) => Some(Status::EvidenceBound),
        (Status::EvidenceBound, Event::Completed) => Some(Status::Completed),
        (Status::Blocked, Event::Resumed) => Some(Status::Running),
        (_, Event::Blocked) if from != Status::Completed => Some(Status::Blocked),
        _ => None,
    }
}

#[test]
fn exhaustive_transition_table_is_enforced() {
    for from in STATUSES {
        for event in EVENTS {
            let outcome = transition_slice_status(from, event);
            match expected_next(from, event) {
                Some(next) => assert_eq!(outcome, Ok(next), "{from:?} + {event:?}"),
                None => assert_eq!(
                    outcome,
                    Err(ExecutionSliceTransitionError::IllegalTransition { from, event }),
                    "{from:?} + {event:?}"
                ),
            }
        }
    }
}

#[test]
fn happy_path_walks_pending_to_completed() {
    let path = [
        (Event::Claimed, Status::Running),
        (Event::RedObserved, Status::RedObserved),
        (Event::PatchApplied, Status::Patched),
        (Event::FocusedTestGreen, Status::FocusedGreen),
        (Event::EvidenceBound, Status::EvidenceBound),
        (Event::Completed, Status::Completed),
    ];
    let mut status = Status::Pending;
    for (event, expected) in path {
        status = transition_slice_status(status, event).expect("legal transition");
        assert_eq!(status, expected);
    }
}

#[test]
fn pending_cannot_jump_to_focused_green() {
    assert_eq!(
        transition_slice_status(Status::Pending, Event::FocusedTestGreen),
        Err(ExecutionSliceTransitionError::IllegalTransition {
            from: Status::Pending,
            event: Event::FocusedTestGreen,
        })
    );
}

#[test]
fn red_observed_cannot_jump_to_completed() {
    assert_eq!(
        transition_slice_status(Status::RedObserved, Event::Completed),
        Err(ExecutionSliceTransitionError::IllegalTransition {
            from: Status::RedObserved,
            event: Event::Completed,
        })
    );
}

#[test]
fn evidence_cannot_be_bound_before_focused_green() {
    for from in [
        Status::Pending,
        Status::Running,
        Status::RedObserved,
        Status::Patched,
        Status::Blocked,
    ] {
        assert!(
            matches!(
                transition_slice_status(from, Event::EvidenceBound),
                Err(ExecutionSliceTransitionError::IllegalTransition { .. })
            ),
            "{from:?} must reject evidence binding before the focused GREEN"
        );
    }
}

#[test]
fn completed_is_terminal_for_every_event() {
    for event in EVENTS {
        assert!(
            matches!(
                transition_slice_status(Status::Completed, event),
                Err(ExecutionSliceTransitionError::IllegalTransition {
                    from: Status::Completed,
                    ..
                })
            ),
            "completed must reject {event:?}"
        );
    }
}

#[test]
fn any_non_terminal_status_can_block_and_only_resume_unblocks() {
    for from in STATUSES {
        if from == Status::Completed {
            continue;
        }
        assert_eq!(
            transition_slice_status(from, Event::Blocked),
            Ok(Status::Blocked),
            "{from:?} must be able to block"
        );
    }

    assert_eq!(
        transition_slice_status(Status::Blocked, Event::Resumed),
        Ok(Status::Running)
    );
    assert!(matches!(
        transition_slice_status(Status::Blocked, Event::Claimed),
        Err(ExecutionSliceTransitionError::IllegalTransition { .. })
    ));

    // After a resume the slice re-enters the loop: RED must be observed again
    // before a patch is accepted.
    let resumed = transition_slice_status(Status::Blocked, Event::Resumed).expect("resume");
    assert!(matches!(
        transition_slice_status(resumed, Event::PatchApplied),
        Err(ExecutionSliceTransitionError::IllegalTransition { .. })
    ));
    assert_eq!(
        transition_slice_status(resumed, Event::RedObserved),
        Ok(Status::RedObserved)
    );
}

#[test]
fn skipped_steps_are_rejected() {
    let illegal = [
        (Status::Pending, Event::PatchApplied),
        (Status::Pending, Event::Completed),
        (Status::Running, Event::FocusedTestGreen),
        (Status::Running, Event::Completed),
        (Status::RedObserved, Event::FocusedTestGreen),
        (Status::Patched, Event::Completed),
        (Status::FocusedGreen, Event::Completed),
    ];
    for (from, event) in illegal {
        assert!(
            matches!(
                transition_slice_status(from, event),
                Err(ExecutionSliceTransitionError::IllegalTransition { .. })
            ),
            "{from:?} + {event:?} must be rejected"
        );
    }
}
