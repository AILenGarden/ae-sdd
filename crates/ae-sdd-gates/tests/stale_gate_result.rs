mod support;

use std::time::Duration;

use ae_sdd_domain::{GateKey, GateOutcome, GateResult};
use ae_sdd_gates::{
    CancellationToken, GateExecutor, GateFreshnessSource, GateInputError, GateRunRequest,
    GateScheduler, GateSpec,
};
use ae_sdd_policy::GateTruth;

struct PassingExecutor;

impl GateExecutor for PassingExecutor {
    fn evaluate(
        &self,
        _specification: &'static GateSpec,
        _key: &GateKey,
        _cancellation: &CancellationToken,
    ) -> GateOutcome {
        GateOutcome::Pass
    }
}

struct NewerRevision;

impl GateFreshnessSource for NewerRevision {
    fn current_key(&self, snapshot: &GateKey) -> Result<GateKey, GateInputError> {
        Ok(support::gate_key(
            snapshot.gate_id().as_str(),
            snapshot.state_revision().get() + 1,
        ))
    }
}

#[test]
fn passing_evaluation_becomes_stale_when_commit_snapshot_changes() {
    let scheduler = GateScheduler::new(PassingExecutor, NewerRevision);
    let result: GateResult = scheduler.run(
        GateRunRequest::new(
            support::gate_key("G-14", 12),
            Duration::from_secs(1),
            CancellationToken::caller(),
        )
        .expect("valid request"),
    );

    assert!(matches!(result.outcome(), GateOutcome::Stale(_)));
    let judgement = GateTruth::judge(result.outcome());
    assert!(!judgement.transition_permitted());
    assert_eq!(judgement.correction_delta(), 0);
}
