//! Execution slice lifecycle transition truth-table tests.
//!
//! The legal path is `pending -> running -> red-observed -> patched ->
//! focused-green -> evidence-bound -> completed` with an optional refactor
//! loop between `focused-green` and `evidence-bound`: `RefactorStarted` opens
//! the loop and only `RefactorGreen` — the re-observed focused GREEN after
//! refactoring — closes it.  While the loop is open, binding evidence is
//! rejected; a slice without refactoring needs may bind evidence directly.
//! Any non-terminal status may move to `blocked` and only `Resumed` re-enters
//! the loop at `running`, keeping an open refactor loop open.  Everything
//! else — in particular `pending -> focused-green`,
//! `red-observed -> completed` and binding evidence before the focused
//! GREEN — must be rejected.

use ae_sdd_contracts::execution_runtime::ExecutionSliceStatus;
use ae_sdd_execution::{
    ExecutionSliceEvent, ExecutionSliceTransitionError, RefactorCycleV1, transition_slice_status,
};

use ExecutionSliceEvent as Event;
use ExecutionSliceStatus as Status;
use RefactorCycleV1 as Refactor;

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

const REFACTOR_CYCLES: [Refactor; 2] = [Refactor::Idle, Refactor::Open];

const EVENTS: [Event; 10] = [
    Event::Claimed,
    Event::RedObserved,
    Event::PatchApplied,
    Event::FocusedTestGreen,
    Event::RefactorStarted,
    Event::RefactorGreen,
    Event::EvidenceBound,
    Event::Completed,
    Event::Blocked,
    Event::Resumed,
];

/// The one authoritative transition table the reducer must enforce.
fn expected_next(from: Status, refactor: Refactor, event: Event) -> Option<(Status, Refactor)> {
    match (from, refactor, event) {
        (Status::Pending, _, Event::Claimed) => Some((Status::Running, refactor)),
        (Status::Running, _, Event::RedObserved) => Some((Status::RedObserved, refactor)),
        (Status::RedObserved, _, Event::PatchApplied) => Some((Status::Patched, refactor)),
        (Status::Patched, _, Event::FocusedTestGreen) => Some((Status::FocusedGreen, refactor)),
        (Status::FocusedGreen, Refactor::Idle, Event::RefactorStarted) => {
            Some((Status::FocusedGreen, Refactor::Open))
        }
        (Status::FocusedGreen, Refactor::Open, Event::RefactorGreen) => {
            Some((Status::FocusedGreen, Refactor::Idle))
        }
        (Status::FocusedGreen, Refactor::Idle, Event::EvidenceBound) => {
            Some((Status::EvidenceBound, Refactor::Idle))
        }
        (Status::EvidenceBound, _, Event::Completed) => Some((Status::Completed, refactor)),
        (Status::Blocked, _, Event::Resumed) => Some((Status::Running, refactor)),
        (_, _, Event::Blocked) if from != Status::Completed => Some((Status::Blocked, refactor)),
        _ => None,
    }
}

#[test]
fn exhaustive_transition_table_is_enforced() {
    for from in STATUSES {
        for refactor in REFACTOR_CYCLES {
            for event in EVENTS {
                let outcome = transition_slice_status(from, refactor, event);
                match expected_next(from, refactor, event) {
                    Some(next) => {
                        assert_eq!(outcome, Ok(next), "{from:?} + {refactor:?} + {event:?}")
                    }
                    None => assert_eq!(
                        outcome,
                        Err(ExecutionSliceTransitionError::IllegalTransition {
                            from,
                            refactor,
                            event,
                        }),
                        "{from:?} + {refactor:?} + {event:?}"
                    ),
                }
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
    let mut refactor = Refactor::Idle;
    for (event, expected) in path {
        (status, refactor) =
            transition_slice_status(status, refactor, event).expect("legal transition");
        assert_eq!(status, expected);
        assert_eq!(refactor, Refactor::Idle);
    }
}

#[test]
fn happy_path_with_refactor_loop_walks_pending_to_completed() {
    let path = [
        (Event::Claimed, Status::Running, Refactor::Idle),
        (Event::RedObserved, Status::RedObserved, Refactor::Idle),
        (Event::PatchApplied, Status::Patched, Refactor::Idle),
        (
            Event::FocusedTestGreen,
            Status::FocusedGreen,
            Refactor::Idle,
        ),
        (Event::RefactorStarted, Status::FocusedGreen, Refactor::Open),
        (Event::RefactorGreen, Status::FocusedGreen, Refactor::Idle),
        (Event::EvidenceBound, Status::EvidenceBound, Refactor::Idle),
        (Event::Completed, Status::Completed, Refactor::Idle),
    ];
    let mut status = Status::Pending;
    let mut refactor = Refactor::Idle;
    for (event, expected_status, expected_refactor) in path {
        (status, refactor) =
            transition_slice_status(status, refactor, event).expect("legal transition");
        assert_eq!((status, refactor), (expected_status, expected_refactor));
    }
}

#[test]
fn pending_cannot_jump_to_focused_green() {
    assert_eq!(
        transition_slice_status(Status::Pending, Refactor::Idle, Event::FocusedTestGreen),
        Err(ExecutionSliceTransitionError::IllegalTransition {
            from: Status::Pending,
            refactor: Refactor::Idle,
            event: Event::FocusedTestGreen,
        })
    );
}

#[test]
fn red_observed_cannot_jump_to_completed() {
    assert_eq!(
        transition_slice_status(Status::RedObserved, Refactor::Idle, Event::Completed),
        Err(ExecutionSliceTransitionError::IllegalTransition {
            from: Status::RedObserved,
            refactor: Refactor::Idle,
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
                transition_slice_status(from, Refactor::Idle, Event::EvidenceBound),
                Err(ExecutionSliceTransitionError::IllegalTransition { .. })
            ),
            "{from:?} must reject evidence binding before the focused GREEN"
        );
    }
}

#[test]
fn evidence_cannot_be_bound_while_a_refactor_is_open() {
    assert_eq!(
        transition_slice_status(Status::FocusedGreen, Refactor::Open, Event::EvidenceBound),
        Err(ExecutionSliceTransitionError::IllegalTransition {
            from: Status::FocusedGreen,
            refactor: Refactor::Open,
            event: Event::EvidenceBound,
        })
    );
}

#[test]
fn refactor_events_are_only_legal_in_their_window() {
    // Refactoring may only start at the focused GREEN.
    for from in STATUSES {
        if from == Status::FocusedGreen {
            continue;
        }
        assert!(
            matches!(
                transition_slice_status(from, Refactor::Idle, Event::RefactorStarted),
                Err(ExecutionSliceTransitionError::IllegalTransition { .. })
            ),
            "{from:?} must reject a refactor start away from the focused GREEN"
        );
    }
    // An open refactor cannot be started twice.
    assert!(matches!(
        transition_slice_status(Status::FocusedGreen, Refactor::Open, Event::RefactorStarted),
        Err(ExecutionSliceTransitionError::IllegalTransition { .. })
    ));
    // The refactor GREEN only closes an open loop.
    for from in STATUSES {
        assert!(
            matches!(
                transition_slice_status(from, Refactor::Idle, Event::RefactorGreen),
                Err(ExecutionSliceTransitionError::IllegalTransition { .. })
            ),
            "{from:?} must reject refactor-green without an open refactor"
        );
    }
}

#[test]
fn completed_is_terminal_for_every_event() {
    for refactor in REFACTOR_CYCLES {
        for event in EVENTS {
            assert!(
                matches!(
                    transition_slice_status(Status::Completed, refactor, event),
                    Err(ExecutionSliceTransitionError::IllegalTransition {
                        from: Status::Completed,
                        ..
                    })
                ),
                "completed must reject {event:?}"
            );
        }
    }
}

#[test]
fn any_non_terminal_status_can_block_and_only_resume_unblocks() {
    for from in STATUSES {
        if from == Status::Completed {
            continue;
        }
        assert_eq!(
            transition_slice_status(from, Refactor::Idle, Event::Blocked),
            Ok((Status::Blocked, Refactor::Idle)),
            "{from:?} must be able to block"
        );
    }

    assert_eq!(
        transition_slice_status(Status::Blocked, Refactor::Idle, Event::Resumed),
        Ok((Status::Running, Refactor::Idle))
    );
    assert!(matches!(
        transition_slice_status(Status::Blocked, Refactor::Idle, Event::Claimed),
        Err(ExecutionSliceTransitionError::IllegalTransition { .. })
    ));

    // After a resume the slice re-enters the loop: RED must be observed again
    // before a patch is accepted.
    let (resumed, refactor) =
        transition_slice_status(Status::Blocked, Refactor::Idle, Event::Resumed).expect("resume");
    assert!(matches!(
        transition_slice_status(resumed, refactor, Event::PatchApplied),
        Err(ExecutionSliceTransitionError::IllegalTransition { .. })
    ));
    assert_eq!(
        transition_slice_status(resumed, refactor, Event::RedObserved),
        Ok((Status::RedObserved, Refactor::Idle))
    );
}

#[test]
fn blocked_during_refactor_stays_open_across_resume() {
    // Blocking an open refactor keeps the loop open; the resume mirrors the
    // RED re-observation rule and the loop must still close with
    // refactor-green before evidence binds.
    let (blocked, refactor) =
        transition_slice_status(Status::FocusedGreen, Refactor::Open, Event::Blocked)
            .expect("blocking an open refactor is legal");
    assert_eq!((blocked, refactor), (Status::Blocked, Refactor::Open));

    let (resumed, refactor) =
        transition_slice_status(blocked, refactor, Event::Resumed).expect("resume");
    assert_eq!((resumed, refactor), (Status::Running, Refactor::Open));
    assert!(matches!(
        transition_slice_status(resumed, refactor, Event::RefactorGreen),
        Err(ExecutionSliceTransitionError::IllegalTransition { .. })
    ));

    let mut status = resumed;
    let mut refactor = refactor;
    for event in [
        Event::RedObserved,
        Event::PatchApplied,
        Event::FocusedTestGreen,
    ] {
        (status, refactor) =
            transition_slice_status(status, refactor, event).expect("legal transition");
    }
    assert_eq!((status, refactor), (Status::FocusedGreen, Refactor::Open));
    assert!(matches!(
        transition_slice_status(status, refactor, Event::EvidenceBound),
        Err(ExecutionSliceTransitionError::IllegalTransition { .. })
    ));
    assert_eq!(
        transition_slice_status(status, refactor, Event::RefactorGreen),
        Ok((Status::FocusedGreen, Refactor::Idle))
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
                transition_slice_status(from, Refactor::Idle, event),
                Err(ExecutionSliceTransitionError::IllegalTransition { .. })
            ),
            "{from:?} + {event:?} must be rejected"
        );
    }
}
