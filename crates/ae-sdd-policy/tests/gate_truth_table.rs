use ae_sdd_domain::{
    CancellationCode, ErrorCode, FindingCode, FreshnessDimension, GateCancellation, GateError,
    GateFailure, GateFinding, GateOutcome, GateTimeout, StaleGate,
};
use ae_sdd_policy::{GateDirective, GateTruth, InfrastructureImpact};

fn failure() -> GateOutcome {
    let finding = GateFinding::new(
        FindingCode::new("GATE_FAILED").expect("test finding code is valid"),
        [],
    );
    GateOutcome::Fail(GateFailure::new([finding]).expect("FAIL has a finding"))
}

#[test]
fn six_gate_outcomes_keep_distinct_policy_semantics() {
    let cases = [
        (
            GateOutcome::Pass,
            true,
            0,
            InfrastructureImpact::Unchanged,
            GateDirective::Proceed,
        ),
        (
            failure(),
            false,
            1,
            InfrastructureImpact::Unchanged,
            GateDirective::Correct,
        ),
        (
            GateOutcome::Error(GateError::new(
                ErrorCode::new("GATE_ERROR").expect("test error code is valid"),
                true,
            )),
            false,
            0,
            InfrastructureImpact::Degraded,
            GateDirective::Retry,
        ),
        (
            GateOutcome::Timeout(GateTimeout::new(250).expect("deadline is positive")),
            false,
            0,
            InfrastructureImpact::Degraded,
            GateDirective::Retry,
        ),
        (
            GateOutcome::Cancelled(GateCancellation::new(
                CancellationCode::new("CALLER_CANCELLED").expect("test cancellation code is valid"),
            )),
            false,
            0,
            InfrastructureImpact::Unchanged,
            GateDirective::AwaitCancellationResolution,
        ),
        (
            GateOutcome::Stale(
                StaleGate::new([FreshnessDimension::StateRevision])
                    .expect("STALE has a changed dimension"),
            ),
            false,
            0,
            InfrastructureImpact::Unchanged,
            GateDirective::Reevaluate,
        ),
    ];

    for (outcome, permitted, correction, health, directive) in cases {
        let judgement = GateTruth::judge(&outcome);
        assert_eq!(judgement.transition_permitted(), permitted);
        assert_eq!(judgement.correction_delta(), correction);
        assert_eq!(judgement.infrastructure_impact(), health);
        assert_eq!(judgement.directive(), directive);
    }
}

#[test]
fn terminal_gate_error_halts_instead_of_retrying() {
    let judgement = GateTruth::judge(&GateOutcome::Error(GateError::new(
        ErrorCode::new("INVALID_GATE_CONFIGURATION").expect("test code is valid"),
        false,
    )));

    assert_eq!(judgement.directive(), GateDirective::Halt);
    assert_eq!(judgement.correction_delta(), 0);
}
