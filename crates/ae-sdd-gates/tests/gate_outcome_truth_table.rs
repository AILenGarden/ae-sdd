use ae_sdd_domain::{
    CancellationCode, ErrorCode, FindingCode, FreshnessDimension, GateCancellation, GateError,
    GateFailure, GateFinding, GateOutcome, GateTimeout, StaleGate,
};
use ae_sdd_policy::{GateDirective, GateTruth};

#[test]
fn six_outcomes_have_one_transition_and_one_correction_class() {
    let outcomes = [
        GateOutcome::Pass,
        GateOutcome::Fail(
            GateFailure::new([GateFinding::new(
                FindingCode::new("FAILED").expect("code"),
                [],
            )])
            .expect("finding"),
        ),
        GateOutcome::Error(GateError::new(
            ErrorCode::new("ERROR").expect("code"),
            false,
        )),
        GateOutcome::Timeout(GateTimeout::new(10).expect("deadline")),
        GateOutcome::Cancelled(GateCancellation::new(
            CancellationCode::new("CANCELLED").expect("code"),
        )),
        GateOutcome::Stale(StaleGate::new([FreshnessDimension::Input]).expect("changed dimension")),
    ];

    let judgements: Vec<_> = outcomes.iter().map(GateTruth::judge).collect();
    assert_eq!(
        judgements
            .iter()
            .filter(|judgement| judgement.transition_permitted())
            .count(),
        1
    );
    assert_eq!(
        judgements
            .iter()
            .filter(|judgement| judgement.correction_delta() == 1)
            .count(),
        1
    );
    assert_eq!(judgements[0].directive(), GateDirective::Proceed);
    assert_eq!(judgements[1].directive(), GateDirective::Correct);
    assert_eq!(judgements[2].directive(), GateDirective::Halt);
    assert_eq!(judgements[3].directive(), GateDirective::Retry);
    assert_eq!(
        judgements[4].directive(),
        GateDirective::AwaitCancellationResolution
    );
    assert_eq!(judgements[5].directive(), GateDirective::Reevaluate);
}
