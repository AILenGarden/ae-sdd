//! Pure execution-slice lifecycle reducer.
//!
//! The reducer owns the legal
//! `pending -> running -> red-observed -> patched -> focused-green ->
//! evidence-bound -> completed` path plus the optional refactor loop between
//! `focused-green` and `evidence-bound`.  The loop is tracked as
//! [`RefactorCycleV1`] because the frozen contract status has no `refactoring`
//! variant: [`ExecutionSliceEvent::RefactorStarted`] opens the loop at the
//! focused GREEN and only [`ExecutionSliceEvent::RefactorGreen`] — the
//! re-observed focused GREEN after refactoring — closes it again.  While the
//! loop is open, binding evidence is rejected; a slice without refactoring
//! needs may bind evidence directly.  Any non-terminal status may move to
//! `blocked`; a blocked slice re-enters the loop at `running` via
//! [`ExecutionSliceEvent::Resumed`] and must re-observe the focused RED
//! before patching again, while an open refactor loop stays open across the
//! block/resume pair and must still close before evidence binds.  The
//! reducer is a pure function of its arguments: it never reads the
//! filesystem, a clock, randomness or any global state.

use ae_sdd_contracts::execution_runtime::ExecutionSliceStatus;

use crate::error::ExecutionSliceTransitionError;

/// Refactor-loop state of one execution slice.
///
/// The loop lives between `focused-green` and `evidence-bound`: it opens on
/// [`ExecutionSliceEvent::RefactorStarted`] and closes only on
/// [`ExecutionSliceEvent::RefactorGreen`].  A slice whose refactor never
/// started stays `Idle` and may bind evidence directly, keeping the
/// pre-refactor lifecycle backward compatible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefactorCycleV1 {
    /// No refactor is open: either none started or the last one closed with
    /// its re-observed focused GREEN.
    Idle,
    /// A refactor started at the focused GREEN and still awaits its
    /// re-observed focused GREEN before evidence may bind.
    Open,
}

/// Machine event offered to the slice lifecycle reducer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionSliceEvent {
    /// The supervisor claimed the slice (`pending -> running`).
    Claimed,
    /// The focused verification produced the expected failure (`running -> red-observed`).
    RedObserved,
    /// A minimal patch was applied (`red-observed -> patched`).
    PatchApplied,
    /// The focused verification turned green (`patched -> focused-green`).
    FocusedTestGreen,
    /// Refactoring began at the focused GREEN (`focused-green`: refactor loop opens).
    RefactorStarted,
    /// The focused verification was re-observed green after refactoring
    /// (`focused-green` with an open loop: refactor loop closes).
    RefactorGreen,
    /// Evidence was appended and bound to the slice (`focused-green -> evidence-bound`).
    EvidenceBound,
    /// The slice was closed (`evidence-bound -> completed`).
    Completed,
    /// An external decision is required (any non-terminal status -> `blocked`).
    Blocked,
    /// The blocker cleared and the slice is re-claimed (`blocked -> running`).
    Resumed,
}

/// Applies one machine event to a slice status and refactor-loop state.
///
/// Returns the next status and refactor-loop state, or
/// [`ExecutionSliceTransitionError::IllegalTransition`] when the event does
/// not legally advance the lifecycle.  In particular `pending ->
/// focused-green`, `red-observed -> completed`, binding evidence before the
/// focused GREEN and binding evidence while a refactor loop is open are
/// always rejected, and `completed` is terminal.
pub const fn transition_slice_status(
    status: ExecutionSliceStatus,
    refactor: RefactorCycleV1,
    event: ExecutionSliceEvent,
) -> Result<(ExecutionSliceStatus, RefactorCycleV1), ExecutionSliceTransitionError> {
    use ExecutionSliceEvent as Event;
    use ExecutionSliceStatus as Status;
    use RefactorCycleV1 as Refactor;
    let next = match (status, refactor, event) {
        (Status::Pending, _, Event::Claimed) => (Status::Running, refactor),
        (Status::Running, _, Event::RedObserved) => (Status::RedObserved, refactor),
        (Status::RedObserved, _, Event::PatchApplied) => (Status::Patched, refactor),
        (Status::Patched, _, Event::FocusedTestGreen) => (Status::FocusedGreen, refactor),
        (Status::FocusedGreen, Refactor::Idle, Event::RefactorStarted) => {
            (Status::FocusedGreen, Refactor::Open)
        }
        (Status::FocusedGreen, Refactor::Open, Event::RefactorGreen) => {
            (Status::FocusedGreen, Refactor::Idle)
        }
        (Status::FocusedGreen, Refactor::Idle, Event::EvidenceBound) => {
            (Status::EvidenceBound, Refactor::Idle)
        }
        (Status::EvidenceBound, _, Event::Completed) => (Status::Completed, refactor),
        (Status::Blocked, _, Event::Resumed) => (Status::Running, refactor),
        (_, _, Event::Blocked) if !matches!(status, Status::Completed) => {
            (Status::Blocked, refactor)
        }
        _ => {
            return Err(ExecutionSliceTransitionError::IllegalTransition {
                from: status,
                refactor,
                event,
            });
        }
    };
    Ok(next)
}
