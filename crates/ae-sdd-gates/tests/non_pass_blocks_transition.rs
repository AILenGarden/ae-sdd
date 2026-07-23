use ae_sdd_domain::{
    CancellationCode, ErrorCode, FindingCode, FreshnessDimension, GateCancellation, GateError,
    GateFailure, GateFinding, GateOutcome, GateTimeout, StaleGate,
};
use ae_sdd_policy::{GateDirective, GateTruth, InfrastructureImpact};

fn non_pass_outcomes() -> [GateOutcome; 5] {
    [
        GateOutcome::Fail(
            GateFailure::new([GateFinding::new(
                FindingCode::new("BUSINESS_FINDING").expect("valid code"),
                [],
            )])
            .expect("finding required"),
        ),
        GateOutcome::Error(GateError::new(
            ErrorCode::new("INFRA_ERROR").expect("valid code"),
            true,
        )),
        GateOutcome::Timeout(GateTimeout::new(250).expect("deadline")),
        GateOutcome::Cancelled(GateCancellation::new(
            CancellationCode::new("CALLER_CANCELLED").expect("valid code"),
        )),
        GateOutcome::Stale(
            StaleGate::new([FreshnessDimension::StateRevision]).expect("changed dimension"),
        ),
    ]
}

#[test]
fn every_non_pass_outcome_blocks_transition_and_only_fail_corrects() {
    for (index, outcome) in non_pass_outcomes().iter().enumerate() {
        let judgement = GateTruth::judge(outcome);
        assert!(!judgement.transition_permitted());
        assert_eq!(judgement.correction_delta(), u64::from(index == 0));
    }
}

#[test]
fn recovery_directives_preserve_error_timeout_cancelled_and_stale_semantics() {
    let outcomes = non_pass_outcomes();
    let expected = [
        (GateDirective::Correct, InfrastructureImpact::Unchanged),
        (GateDirective::Retry, InfrastructureImpact::Degraded),
        (GateDirective::Retry, InfrastructureImpact::Degraded),
        (
            GateDirective::AwaitCancellationResolution,
            InfrastructureImpact::Unchanged,
        ),
        (GateDirective::Reevaluate, InfrastructureImpact::Unchanged),
    ];
    for (outcome, (directive, impact)) in outcomes.iter().zip(expected) {
        let judgement = GateTruth::judge(outcome);
        assert_eq!(judgement.directive(), directive);
        assert_eq!(judgement.infrastructure_impact(), impact);
    }
}
