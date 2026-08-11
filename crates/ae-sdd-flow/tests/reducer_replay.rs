mod support;

use ae_sdd_contracts::execution_runtime::ExecutionSliceStatus;
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, DesignRoute, GateOutcome, InputFingerprint, ProcessPhase,
    StateRevision, WorkScale,
};
use ae_sdd_flow::{
    ExecutionCursor, FlowEnvironment, FlowEventKind, FlowInput, FlowRuntime, FlowSnapshot,
    NextAction, RouteLifecycle, RouteSelection,
};
use proptest::prelude::*;

use support::{commit_to, event, event_store, gate_for, input, transition_request};

use ae_sdd_policy::RequiredGate;

fn execution_input(status: ExecutionSliceStatus) -> FlowInput {
    let snapshot = FlowSnapshot::new(ProcessPhase::Coding, StateRevision::new(7), 0);
    let environment = FlowEnvironment::new(
        event_store(),
        InputFingerprint::digest(b"work-item-input-v1"),
        RouteLifecycle::Frozen(RouteSelection::new(WorkScale::Large, DesignRoute::Story)),
    )
    .with_execution_cursor(ExecutionCursor::new(
        1,
        ArtifactDigest::digest(b"approved-queue-v1"),
        status,
    ));
    FlowInput::new(snapshot, environment)
}

#[test]
fn reordered_complete_log_has_identical_decision_and_next_action() {
    // RA-first: the first transition (Initialized -> RequirementAnalyzed)
    // requires G-RA-1..4. Supply all four PASS events then commit.
    let ordered = vec![
        transition_request(4, AgentRole::Root),
        gate_for(5, RequiredGate::GRa1, GateOutcome::Pass),
        gate_for(6, RequiredGate::GRa2, GateOutcome::Pass),
        gate_for(7, RequiredGate::GRa3, GateOutcome::Pass),
        gate_for(8, RequiredGate::GRa4, GateOutcome::Pass),
        commit_to(17, 8, ProcessPhase::RequirementAnalyzed),
    ];
    let mut reordered = ordered.clone();
    reordered.reverse();

    let first = FlowRuntime::replay(input(), ordered).expect("ordered log is valid");
    let second = FlowRuntime::replay(input(), reordered).expect("reordered log is valid");

    assert_eq!(first, second);
    assert_eq!(first.snapshot().phase(), ProcessPhase::RequirementAnalyzed);
}

#[test]
fn execution_log_replay_digest_is_identical_across_runs_and_orderings() {
    let queue_v1 = ArtifactDigest::digest(b"approved-queue-v1");
    let queue_v2 = ArtifactDigest::digest(b"approved-queue-v2");
    let log = || {
        vec![
            event(
                3,
                b"approved-v1-running",
                FlowEventKind::ExecutionQueueApproved {
                    cursor: ExecutionCursor::new(1, queue_v1, ExecutionSliceStatus::Running),
                },
            ),
            event(
                5,
                b"approved-v2-pending",
                FlowEventKind::ExecutionQueueApproved {
                    cursor: ExecutionCursor::new(2, queue_v2, ExecutionSliceStatus::Pending),
                },
            ),
            event(
                8,
                b"approved-v2-running",
                FlowEventKind::ExecutionQueueApproved {
                    cursor: ExecutionCursor::new(2, queue_v2, ExecutionSliceStatus::Running),
                },
            ),
        ]
    };

    let input = execution_input(ExecutionSliceStatus::Pending);
    let first = FlowRuntime::replay(input, log()).expect("ordered log is valid");
    let second = FlowRuntime::replay(input, log()).expect("the same log replays");
    let reordered =
        FlowRuntime::replay(input, log().into_iter().rev()).expect("reordered delivery converges");

    assert_eq!(first, second);
    assert_eq!(first.decision_digest(), second.decision_digest());
    assert_eq!(first.decision_digest(), reordered.decision_digest());
    assert_eq!(
        first.next_action(),
        &NextAction::ExecuteApprovedSlice {
            active_ordinal: 2,
            queue_digest: queue_v2,
        }
    );
}

proptest! {
    #[test]
    fn prompt_replay_digest_is_independent_of_delivery_order(
        mut sequences in prop::collection::btree_set(1_u64..10_000, 1..64),
    ) {
        let ordered: Vec<_> = sequences
            .iter()
            .map(|sequence| event(
                *sequence,
                &sequence.to_be_bytes(),
                FlowEventKind::PromptAccepted,
            ))
            .collect();
        let mut reversed = ordered.clone();
        reversed.reverse();
        if let Some(first) = ordered.first() {
            reversed.push(first.clone());
        }

        let expected = FlowRuntime::replay(input(), ordered)
            .expect("unique ordered events are valid");
        let actual = FlowRuntime::replay(input(), reversed)
            .expect("reordered duplicates are valid");

        prop_assert_eq!(actual.decision_digest(), expected.decision_digest());
        sequences.clear();
    }
}
