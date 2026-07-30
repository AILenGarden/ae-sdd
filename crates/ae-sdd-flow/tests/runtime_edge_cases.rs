mod support;

use ae_sdd_domain::{
    AgentRole, DesignRoute, EventSequence, FindingCode, GateFailure, GateFinding, GateOutcome,
    InputFingerprint, PolicyDigest, ProcessPhase, StateRevision, WorkScale,
};
use ae_sdd_flow::{
    EventCursor, EventProvenance, FlowEnvironment, FlowError, FlowEvent, FlowEventKind, FlowInput,
    FlowRuntime, FlowSnapshot, NextAction, RouteSelection, SupervisorFault,
};
use ae_sdd_policy::{RequiredGate, TransitionPolicy};

fn provenance(
    sequence: u64,
    policy: PolicyDigest,
    input: InputFingerprint,
    label: &[u8],
) -> EventProvenance {
    EventProvenance::new(
        EventCursor::new(support::event_store(), EventSequence::new(sequence)),
        policy,
        input,
        InputFingerprint::digest(label),
    )
}

fn custom_input(phase: ProcessPhase, scale: WorkScale, route: DesignRoute) -> FlowInput {
    FlowInput::new(
        FlowSnapshot::new(phase, StateRevision::new(7), 0),
        FlowEnvironment::new(
            support::event_store(),
            InputFingerprint::digest(b"work-item-input-v1"),
            RouteSelection::new(scale, route),
        ),
    )
}

#[test]
fn event_provenance_mismatches_and_old_events_fail_closed_or_noop() {
    let checkpoint = FlowRuntime::start(support::input());
    let zero = FlowEvent::new(
        provenance(
            0,
            TransitionPolicy::digest(),
            InputFingerprint::digest(b"work-item-input-v1"),
            b"zero",
        ),
        FlowEventKind::PromptAccepted,
    );
    assert_eq!(
        FlowRuntime::apply(&checkpoint, &zero),
        Err(FlowError::InvalidEventSequence)
    );

    let other_store = support::event_in_store(
        support::other_event_store(),
        1,
        b"other-store",
        FlowEventKind::PromptAccepted,
    );
    assert!(matches!(
        FlowRuntime::apply(&checkpoint, &other_store),
        Err(FlowError::EventStoreMismatch { .. })
    ));

    let wrong_policy = FlowEvent::new(
        provenance(
            1,
            PolicyDigest::digest(b"other-policy"),
            InputFingerprint::digest(b"work-item-input-v1"),
            b"wrong-policy",
        ),
        FlowEventKind::PromptAccepted,
    );
    assert!(matches!(
        FlowRuntime::apply(&checkpoint, &wrong_policy),
        Err(FlowError::PolicyDigestMismatch { .. })
    ));

    let wrong_input = FlowEvent::new(
        provenance(
            1,
            TransitionPolicy::digest(),
            InputFingerprint::digest(b"other-input"),
            b"wrong-input",
        ),
        FlowEventKind::PromptAccepted,
    );
    assert!(matches!(
        FlowRuntime::apply(&checkpoint, &wrong_input),
        Err(FlowError::InputFingerprintMismatch { .. })
    ));

    let after_two = FlowRuntime::apply(
        &checkpoint,
        &support::event(2, b"two", FlowEventKind::PromptAccepted),
    )
    .expect("new event applies");
    let unchanged = FlowRuntime::apply(
        &after_two,
        &support::event(1, b"old", FlowEventKind::PromptAccepted),
    )
    .expect("older event is an already-consumed noop");
    assert_eq!(unchanged, after_two);
}

#[test]
fn pending_gate_and_commit_invariants_reject_out_of_order_events() {
    let initial = FlowRuntime::start(support::input());
    let pending = FlowRuntime::apply(&initial, &support::transition_request(1, AgentRole::Root))
        .expect("root transition becomes pending");
    assert!(matches!(
        FlowRuntime::apply(
            &pending,
            &support::transition_request_to(2, AgentRole::Root, ProcessPhase::RequirementAnalyzed),
        ),
        Err(FlowError::TransitionAlreadyPending { .. })
    ));
    assert_eq!(
        FlowRuntime::apply(&initial, &support::gate(1, GateOutcome::Pass)),
        Err(FlowError::UnexpectedGateOutcome)
    );
    assert_eq!(
        FlowRuntime::apply(
            &pending,
            &support::gate_for(2, RequiredGate::G01, GateOutcome::Pass),
        ),
        Err(FlowError::UnexpectedGate {
            gate: RequiredGate::G01,
        })
    );
    assert!(matches!(
        FlowRuntime::apply(
            &pending,
            &support::commit_to(2, 8, ProcessPhase::RequirementAnalyzed)
        ),
        Err(FlowError::UnexpectedTransitionCommit { .. })
    ));
    assert_eq!(
        FlowRuntime::apply(&pending, &support::commit(2, 8)),
        Err(FlowError::TransitionNotReady {
            target: ProcessPhase::RouteSelected,
        })
    );

    let ready = FlowRuntime::apply(&pending, &support::gate(2, GateOutcome::Pass))
        .expect("required Gate passes");
    assert!(matches!(
        FlowRuntime::apply(&ready, &support::commit(3, 7)),
        Err(FlowError::NonMonotonicStateRevision { .. })
    ));

    let overflow_start = FlowRuntime::start(support::input_with_corrections(u64::MAX));
    let overflow_pending = FlowRuntime::apply(
        &overflow_start,
        &support::transition_request(1, AgentRole::Root),
    )
    .expect("transition becomes pending");
    let failure = GateOutcome::Fail(
        GateFailure::new([GateFinding::new(
            FindingCode::new("FAILED").expect("finding code"),
            [],
        )])
        .expect("non-empty failure"),
    );
    assert_eq!(
        FlowRuntime::apply(&overflow_pending, &support::gate(2, failure)),
        Err(FlowError::CorrectionOverflow)
    );
}

#[test]
fn repeated_root_request_for_the_same_pending_transition_is_a_replay_noop() {
    let initial = FlowRuntime::start(support::input());
    let pending = FlowRuntime::apply(&initial, &support::transition_request(1, AgentRole::Root))
        .expect("root transition becomes pending");

    let replayed = FlowRuntime::apply(&pending, &support::transition_request(2, AgentRole::Root))
        .expect("same root transition intent replays idempotently");

    assert_eq!(replayed.pending_transition(), pending.pending_transition());
    assert_eq!(replayed.required_gates(), pending.required_gates());
    assert_eq!(replayed.passed_gates(), pending.passed_gates());
    assert_eq!(replayed.next_action(), pending.next_action());
    assert_eq!(
        replayed
            .last_cursor()
            .expect("replayed event advances the cursor")
            .sequence()
            .get(),
        2
    );
}

#[test]
fn pause_commit_records_the_exact_resume_phase() {
    let initial = FlowRuntime::start(support::input_at(ProcessPhase::Coding, 0));
    let pending = FlowRuntime::apply(
        &initial,
        &support::transition_request_to(1, AgentRole::Root, ProcessPhase::Paused),
    )
    .expect("active flow may request pause");
    assert_eq!(
        pending.next_action(),
        &NextAction::ApplyTransition {
            target: ProcessPhase::Paused,
        }
    );
    let paused = FlowRuntime::apply(&pending, &support::commit_to(2, 8, ProcessPhase::Paused))
        .expect("authorized pause commits");
    assert_eq!(paused.snapshot().phase(), ProcessPhase::Paused);
    assert_eq!(paused.snapshot().paused_from(), Some(ProcessPhase::Coding));
}

#[test]
fn decision_digest_handles_every_phase_route_scale_fault_and_denial_shape() {
    let phases = [
        ProcessPhase::Initialized,
        ProcessPhase::RouteSelected,
        ProcessPhase::RequirementAnalyzed,
        ProcessPhase::DrGenerated,
        ProcessPhase::StoryGenerated,
        ProcessPhase::TestcaseGenerated,
        ProcessPhase::CodingProcess,
        ProcessPhase::Coding,
        ProcessPhase::TestRunning,
        ProcessPhase::CodeReviewed,
        ProcessPhase::Completed,
        ProcessPhase::Paused,
    ];
    for phase in phases {
        let input = if phase == ProcessPhase::Paused {
            FlowInput::new(
                FlowSnapshot::new(phase, StateRevision::new(7), 0)
                    .with_paused_from(ProcessPhase::Coding),
                custom_input(ProcessPhase::Initialized, WorkScale::Large, DesignRoute::Dr)
                    .environment(),
            )
        } else {
            custom_input(phase, WorkScale::Large, DesignRoute::Dr)
        };
        assert_ne!(
            FlowRuntime::start(input).decision_digest(),
            ae_sdd_domain::DecisionDigest::from_array([0; 32])
        );
    }

    for (scale, route) in [
        (WorkScale::Large, DesignRoute::Dr),
        (WorkScale::Medium, DesignRoute::Story),
        (WorkScale::Small, DesignRoute::CodingPlan),
        (WorkScale::Micro, DesignRoute::CodingPlan),
    ] {
        let decision = FlowRuntime::start(custom_input(ProcessPhase::Initialized, scale, route));
        assert_eq!(decision.snapshot().phase(), ProcessPhase::Initialized);
    }

    for (index, fault) in [
        SupervisorFault::GateWorker,
        SupervisorFault::EventStore,
        SupervisorFault::ArtifactProjection,
        SupervisorFault::HostAdapter,
    ]
    .into_iter()
    .enumerate()
    {
        let initial = FlowRuntime::start(support::input());
        let event = support::event(
            u64::try_from(index + 1).expect("small event sequence"),
            &[u8::try_from(index).expect("small label")],
            FlowEventKind::BackgroundFault(fault),
        );
        FlowRuntime::apply(&initial, &event).expect("background fault is reduced");
    }

    let denials = [
        (
            custom_input(
                ProcessPhase::Initialized,
                WorkScale::Small,
                DesignRoute::Story,
            ),
            ProcessPhase::RouteSelected,
        ),
        (
            custom_input(ProcessPhase::DrGenerated, WorkScale::Large, DesignRoute::Dr),
            ProcessPhase::TestcaseGenerated,
        ),
        (
            custom_input(ProcessPhase::Initialized, WorkScale::Large, DesignRoute::Dr),
            ProcessPhase::RequirementAnalyzed,
        ),
        (
            FlowInput::new(
                FlowSnapshot::new(ProcessPhase::Paused, StateRevision::new(7), 0)
                    .with_paused_from(ProcessPhase::TestcaseGenerated),
                custom_input(ProcessPhase::Initialized, WorkScale::Large, DesignRoute::Dr)
                    .environment(),
            ),
            ProcessPhase::Coding,
        ),
    ];
    for (input, target) in denials {
        let initial = FlowRuntime::start(input);
        let denied = FlowRuntime::apply(
            &initial,
            &support::event(
                1,
                &[target as u8],
                FlowEventKind::TransitionRequested {
                    actor_role: AgentRole::Root,
                    target,
                },
            ),
        )
        .expect("policy denial is a deterministic decision");
        assert!(matches!(
            denied.next_action(),
            NextAction::TransitionDenied { .. }
        ));
    }
}

#[test]
fn every_flow_error_has_a_nonempty_diagnostic() {
    let store = support::event_store();
    let other = support::other_event_store();
    let expected_policy = TransitionPolicy::digest();
    let actual_policy = PolicyDigest::digest(b"other");
    let expected_input = InputFingerprint::digest(b"expected");
    let actual_input = InputFingerprint::digest(b"actual");
    let errors = [
        FlowError::InvalidEventSequence,
        FlowError::EventStoreMismatch {
            expected: store,
            actual: other,
        },
        FlowError::EventSequenceConflict {
            sequence: EventSequence::new(1),
        },
        FlowError::PolicyDigestMismatch {
            expected: expected_policy,
            actual: actual_policy,
        },
        FlowError::InputFingerprintMismatch {
            expected: expected_input,
            actual: actual_input,
        },
        FlowError::TransitionAlreadyPending {
            pending: ProcessPhase::Coding,
            requested: ProcessPhase::Paused,
        },
        FlowError::UnexpectedGateOutcome,
        FlowError::UnexpectedGate {
            gate: RequiredGate::G14,
        },
        FlowError::UnexpectedTransitionCommit {
            pending: Some(ProcessPhase::Coding),
            committed: ProcessPhase::Paused,
        },
        FlowError::TransitionNotReady {
            target: ProcessPhase::Coding,
        },
        FlowError::NonMonotonicStateRevision {
            current: StateRevision::new(7),
            committed: StateRevision::new(7),
        },
        FlowError::CorrectionOverflow,
    ];
    for error in errors {
        assert!(!error.to_string().is_empty());
    }
}
