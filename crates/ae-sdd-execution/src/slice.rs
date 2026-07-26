//! Pure execution-slice lifecycle reducer.
//!
//! The reducer owns the legal
//! `pending -> running -> red-observed -> patched -> focused-green ->
//! evidence-bound -> completed` path.  Any non-terminal status may move to
//! `blocked`; a blocked slice re-enters the loop at `running` via
//! [`ExecutionSliceEvent::Resumed`] and must re-observe the focused RED
//! before patching again.  The reducer is a pure function of its arguments:
//! it never reads the filesystem, a clock, randomness or any global state.

use ae_sdd_contracts::execution_runtime::ExecutionSliceStatus;

use crate::error::ExecutionSliceTransitionError;

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
    /// Evidence was appended and bound to the slice (`focused-green -> evidence-bound`).
    EvidenceBound,
    /// The slice was closed (`evidence-bound -> completed`).
    Completed,
    /// An external decision is required (any non-terminal status -> `blocked`).
    Blocked,
    /// The blocker cleared and the slice is re-claimed (`blocked -> running`).
    Resumed,
}

/// Applies one machine event to a slice status.
///
/// Returns the next status, or
/// [`ExecutionSliceTransitionError::IllegalTransition`] when the event does
/// not legally advance the lifecycle.  In particular `pending ->
/// focused-green`, `red-observed -> completed` and binding evidence before
/// the focused GREEN are always rejected, and `completed` is terminal.
pub const fn transition_slice_status(
    status: ExecutionSliceStatus,
    event: ExecutionSliceEvent,
) -> Result<ExecutionSliceStatus, ExecutionSliceTransitionError> {
    use ExecutionSliceEvent as Event;
    use ExecutionSliceStatus as Status;
    let next = match (status, event) {
        (Status::Pending, Event::Claimed) => Status::Running,
        (Status::Running, Event::RedObserved) => Status::RedObserved,
        (Status::RedObserved, Event::PatchApplied) => Status::Patched,
        (Status::Patched, Event::FocusedTestGreen) => Status::FocusedGreen,
        (Status::FocusedGreen, Event::EvidenceBound) => Status::EvidenceBound,
        (Status::EvidenceBound, Event::Completed) => Status::Completed,
        (Status::Blocked, Event::Resumed) => Status::Running,
        (_, Event::Blocked) if !matches!(status, Status::Completed) => Status::Blocked,
        _ => {
            return Err(ExecutionSliceTransitionError::IllegalTransition {
                from: status,
                event,
            });
        }
    };
    Ok(next)
}
