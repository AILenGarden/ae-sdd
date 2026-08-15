mod support;

use ae_sdd_contracts::execution_runtime::ExecutionSliceStatus;
use ae_sdd_domain::{
    ArtifactDigest, DesignRoute, InputFingerprint, ProcessPhase, StateRevision, WorkScale,
};
use ae_sdd_flow::{
    ExecutionCursor, FlowEnvironment, FlowEventKind, FlowInput, FlowRuntime, FlowSnapshot,
    NextAction, RouteLifecycle, RouteSelection,
};
use support::{event, event_store};

fn cursor(ordinal: u32, digest: ArtifactDigest, status: ExecutionSliceStatus) -> ExecutionCursor {
    ExecutionCursor::new(
        ordinal,
        digest,
        ArtifactDigest::digest(b"approved-capsule-v1"),
        status,
    )
}

fn execution_input(phase: ProcessPhase, cursor: ExecutionCursor) -> FlowInput {
    let snapshot = FlowSnapshot::new(phase, StateRevision::new(7), 0);
    let environment = FlowEnvironment::new(
        event_store(),
        InputFingerprint::digest(b"work-item-input-v1"),
        RouteLifecycle::Frozen(RouteSelection::new(WorkScale::Large, DesignRoute::Story)),
    )
    .with_execution_cursor(cursor);
    FlowInput::new(snapshot, environment)
}

#[test]
fn coding_with_an_approved_queue_and_pending_slice_executes_the_approved_slice() {
    let queue_digest = ArtifactDigest::digest(b"approved-queue-v1");
    let input = execution_input(
        ProcessPhase::Coding,
        cursor(2, queue_digest, ExecutionSliceStatus::Pending),
    );

    let decision = FlowRuntime::start(input);

    assert_eq!(
        decision.next_action(),
        &NextAction::ExecuteApprovedSlice {
            active_ordinal: 2,
            queue_digest,
            capsule_digest: ArtifactDigest::digest(b"approved-capsule-v1"),
            active_slice_status: ExecutionSliceStatus::Pending,
            next_slice_transition: ExecutionSliceStatus::Running,
        }
    );
}

#[test]
fn a_changed_queue_digest_resumes_approved_execution() {
    let original = ArtifactDigest::digest(b"approved-queue-v1");
    let changed = ArtifactDigest::digest(b"approved-queue-v2");
    let input = execution_input(
        ProcessPhase::Coding,
        cursor(2, original, ExecutionSliceStatus::Pending),
    );
    let started = FlowRuntime::start(input);

    let decision = FlowRuntime::apply(
        &started,
        &event(
            11,
            b"queue-approved-v2",
            FlowEventKind::ExecutionQueueApproved {
                cursor: cursor(3, changed, ExecutionSliceStatus::Pending),
            },
        ),
    )
    .expect("a queue re-approval event is valid");

    assert_eq!(decision.next_action(), &NextAction::ResumeApprovedExecution);
    assert_eq!(
        decision.execution_cursor(),
        Some(cursor(3, changed, ExecutionSliceStatus::Pending))
    );
}

#[test]
fn reapproving_the_same_queue_digest_keeps_executing_the_active_slice() {
    let queue_digest = ArtifactDigest::digest(b"approved-queue-v1");
    let input = execution_input(
        ProcessPhase::Coding,
        cursor(2, queue_digest, ExecutionSliceStatus::Pending),
    );
    let started = FlowRuntime::start(input);

    let decision = FlowRuntime::apply(
        &started,
        &event(
            13,
            b"queue-approved-v1-running",
            FlowEventKind::ExecutionQueueApproved {
                cursor: cursor(2, queue_digest, ExecutionSliceStatus::Running),
            },
        ),
    )
    .expect("a same-digest re-approval event is valid");

    assert_eq!(
        decision.next_action(),
        &NextAction::ExecuteApprovedSlice {
            active_ordinal: 2,
            queue_digest,
            capsule_digest: ArtifactDigest::digest(b"approved-capsule-v1"),
            active_slice_status: ExecutionSliceStatus::Running,
            next_slice_transition: ExecutionSliceStatus::RedObserved,
        }
    );
}

#[test]
fn an_open_mid_slice_status_keeps_executing_the_approved_slice() {
    let queue_digest = ArtifactDigest::digest(b"approved-queue-v1");
    for (status, next_status) in [
        (
            ExecutionSliceStatus::Running,
            ExecutionSliceStatus::RedObserved,
        ),
        (
            ExecutionSliceStatus::RedObserved,
            ExecutionSliceStatus::Patched,
        ),
        (
            ExecutionSliceStatus::Patched,
            ExecutionSliceStatus::FocusedGreen,
        ),
        (
            ExecutionSliceStatus::FocusedGreen,
            ExecutionSliceStatus::EvidenceBound,
        ),
        (
            ExecutionSliceStatus::EvidenceBound,
            ExecutionSliceStatus::Completed,
        ),
    ] {
        let input = execution_input(ProcessPhase::Coding, cursor(1, queue_digest, status));
        let decision = FlowRuntime::start(input);
        assert_eq!(
            decision.next_action(),
            &NextAction::ExecuteApprovedSlice {
                active_ordinal: 1,
                queue_digest,
                capsule_digest: ArtifactDigest::digest(b"approved-capsule-v1"),
                active_slice_status: status,
                next_slice_transition: next_status,
            },
            "open status {status:?} must keep executing",
        );
    }
}

#[test]
fn a_closed_slice_waits_for_the_authority() {
    let queue_digest = ArtifactDigest::digest(b"approved-queue-v1");
    for status in [
        ExecutionSliceStatus::Completed,
        ExecutionSliceStatus::Blocked,
    ] {
        let input = execution_input(ProcessPhase::Coding, cursor(1, queue_digest, status));
        let decision = FlowRuntime::start(input);
        assert_eq!(
            decision.next_action(),
            &NextAction::AwaitAgentWork,
            "closed status {status:?} must wait for the authority",
        );
    }
}

#[test]
fn execution_actions_require_the_execution_phase() {
    let queue_digest = ArtifactDigest::digest(b"approved-queue-v1");
    let input = execution_input(
        ProcessPhase::TestRunning,
        cursor(2, queue_digest, ExecutionSliceStatus::Pending),
    );

    let decision = FlowRuntime::start(input);

    assert_eq!(decision.next_action(), &NextAction::AwaitAgentWork);
}
