use ae_sdd_domain::{
    CancellationCode, ErrorCode, FindingCode, FreshnessDimension, GateCancellation, GateError,
    GateFailure, GateFinding, GateOutcome, GateTimeout, StaleGate,
};
use ae_sdd_policy::GateTruth;

#[test]
fn every_non_pass_outcome_blocks_transition() {
    let finding = GateFinding::new(
        FindingCode::new("GATE_FAILED").expect("test code is valid"),
        [],
    );
    let outcomes = [
        GateOutcome::Fail(GateFailure::new([finding]).expect("FAIL has findings")),
        GateOutcome::Error(GateError::new(
            ErrorCode::new("GATE_ERROR").expect("test code is valid"),
            true,
        )),
        GateOutcome::Timeout(GateTimeout::new(250).expect("deadline is positive")),
        GateOutcome::Cancelled(GateCancellation::new(
            CancellationCode::new("CALLER_CANCELLED").expect("test code is valid"),
        )),
        GateOutcome::Stale(
            StaleGate::new([FreshnessDimension::Input]).expect("STALE has a changed dimension"),
        ),
    ];

    for outcome in outcomes {
        assert!(!GateTruth::judge(&outcome).transition_permitted());
    }
}
