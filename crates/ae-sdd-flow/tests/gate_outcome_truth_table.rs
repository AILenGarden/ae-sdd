mod support;

use ae_sdd_domain::{
    AgentRole, CancellationCode, ErrorCode, FindingCode, FreshnessDimension, GateCancellation,
    GateError, GateFailure, GateFinding, GateOutcome, GateTimeout, ProcessPhase, StaleGate,
};
use ae_sdd_flow::{FlowRuntime, NextAction, SupervisorDegradation, SupervisorHealth};

use ae_sdd_policy::RequiredGate;

use support::{commit, gate, gate_for, input, input_at, transition_request, transition_request_to};

fn fail() -> GateOutcome {
    let finding = GateFinding::new(
        FindingCode::new("GATE_FAILED").expect("test code is valid"),
        [],
    );
    GateOutcome::Fail(GateFailure::new([finding]).expect("FAIL has findings"))
}

fn decision(outcome: GateOutcome) -> ae_sdd_flow::FlowDecision {
    FlowRuntime::replay(
        input(),
        [transition_request(1, AgentRole::Root), gate(2, outcome)],
    )
    .expect("truth-table case is a valid event sequence")
}

#[test]
fn only_fail_corrects_and_infrastructure_outcomes_only_degrade_health() {
    let pass = decision(GateOutcome::Pass);
    assert_eq!(pass.snapshot().correction_count(), 0);
    assert!(matches!(
        pass.next_action(),
        NextAction::ApplyTransition { .. }
    ));

    let failed = decision(fail());
    assert_eq!(failed.snapshot().correction_count(), 1);
    assert_eq!(failed.next_action(), &NextAction::ProvideCorrection);
    assert_eq!(failed.health(), SupervisorHealth::Healthy);

    let error = decision(GateOutcome::Error(GateError::new(
        ErrorCode::new("GATE_ERROR").expect("test code is valid"),
        true,
    )));
    assert_eq!(error.snapshot().correction_count(), 0);
    assert_eq!(error.next_action(), &NextAction::RetryGate);
    assert_eq!(
        error.health(),
        SupervisorHealth::Degraded(SupervisorDegradation::GateError)
    );

    let timeout = decision(GateOutcome::Timeout(
        GateTimeout::new(250).expect("positive deadline"),
    ));
    assert_eq!(timeout.snapshot().correction_count(), 0);
    assert_eq!(timeout.next_action(), &NextAction::RetryGate);
    assert_eq!(
        timeout.health(),
        SupervisorHealth::Degraded(SupervisorDegradation::GateTimeout)
    );

    let cancelled = decision(GateOutcome::Cancelled(GateCancellation::new(
        CancellationCode::new("CALLER_CANCELLED").expect("test code is valid"),
    )));
    assert_eq!(cancelled.snapshot().correction_count(), 0);
    assert_eq!(
        cancelled.next_action(),
        &NextAction::AwaitCancellationResolution
    );

    let stale = decision(GateOutcome::Stale(
        StaleGate::new([FreshnessDimension::Policy]).expect("STALE has a changed dimension"),
    ));
    assert_eq!(stale.snapshot().correction_count(), 0);
    assert_eq!(stale.next_action(), &NextAction::ReevaluateGate);
}

#[test]
fn transition_waits_until_every_required_gate_passes() {
    let input = input_at(ProcessPhase::TestcaseGenerated, 0);
    let request = transition_request_to(1, AgentRole::Root, ProcessPhase::CodingProcess);
    let first_pass = gate_for(2, RequiredGate::G00, GateOutcome::Pass);
    let partial = FlowRuntime::replay(input, [request.clone(), first_pass])
        .expect("partial Gate set reduces");

    let NextAction::EvaluateGates { required_gates, .. } = partial.next_action() else {
        panic!("partial Gate set must continue evaluation");
    };
    assert!(!required_gates.contains(&RequiredGate::G00));
    assert_eq!(required_gates.len(), 4);

    let complete = FlowRuntime::replay(
        input,
        [
            request,
            gate_for(2, RequiredGate::G00, GateOutcome::Pass),
            gate_for(3, RequiredGate::G02, GateOutcome::Pass),
            gate_for(4, RequiredGate::G03, GateOutcome::Pass),
            gate_for(5, RequiredGate::G04, GateOutcome::Pass),
            gate_for(6, RequiredGate::GStoryContext, GateOutcome::Pass),
        ],
    )
    .expect("complete Gate set reduces");
    assert!(matches!(
        complete.next_action(),
        NextAction::ApplyTransition {
            target: ProcessPhase::CodingProcess
        }
    ));
}

#[test]
fn non_pass_gate_cannot_be_bypassed_by_commit_event() {
    let checkpoint = FlowRuntime::replay(
        input(),
        [transition_request(1, AgentRole::Root), gate(2, fail())],
    )
    .expect("fresh FAIL produces a correction decision");

    let result = FlowRuntime::apply(&checkpoint, &commit(3, 8));
    assert!(matches!(
        result,
        Err(ae_sdd_flow::FlowError::TransitionNotReady { .. })
    ));
    assert_eq!(checkpoint.snapshot().correction_count(), 1);
}
