use ae_sdd_domain::{GateKey, GateOutcome, GateResult};

/// The deterministic action selected from one Gate outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateDirective {
    /// The evaluated mutation may proceed to its commit preconditions.
    Proceed,
    /// Return the business findings to the responsible Agent.
    Correct,
    /// Retry the infrastructure-backed Gate evaluation.
    Retry,
    /// Stop automatic retry because the infrastructure error is terminal.
    Halt,
    /// Await an explicit caller decision after cancellation.
    AwaitCancellationResolution,
    /// Evaluate the Gate again from a fresh snapshot.
    Reevaluate,
}

/// Effect of a Gate outcome on supervisor health.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfrastructureImpact {
    /// The outcome does not change infrastructure health.
    Unchanged,
    /// The outcome marks the supervisor degraded until recovery evidence arrives.
    Degraded,
}

/// Complete policy judgement for one Gate result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GateJudgement {
    transition_permitted: bool,
    correction_delta: u64,
    infrastructure_impact: InfrastructureImpact,
    directive: GateDirective,
}

impl GateJudgement {
    /// Returns whether this Gate result permits the guarded transition.
    pub const fn transition_permitted(self) -> bool {
        self.transition_permitted
    }

    /// Returns the business correction increment selected by policy.
    pub const fn correction_delta(self) -> u64 {
        self.correction_delta
    }

    /// Returns the effect on supervisor infrastructure health.
    pub const fn infrastructure_impact(self) -> InfrastructureImpact {
        self.infrastructure_impact
    }

    /// Returns the deterministic follow-up action.
    pub const fn directive(self) -> GateDirective {
        self.directive
    }
}

/// The single Gate truth table used by transitions and supervisors.
#[derive(Clone, Copy, Debug, Default)]
pub struct GateTruth;

impl GateTruth {
    /// Judges an already freshness-normalized six-state Gate outcome.
    ///
    /// Only `Pass` permits a transition. Only `Fail` increments business
    /// correction. Infrastructure and freshness states retain their distinct
    /// recovery actions.
    pub const fn judge(outcome: &GateOutcome) -> GateJudgement {
        match outcome {
            GateOutcome::Pass => GateJudgement {
                transition_permitted: true,
                correction_delta: 0,
                infrastructure_impact: InfrastructureImpact::Unchanged,
                directive: GateDirective::Proceed,
            },
            GateOutcome::Fail(_) => GateJudgement {
                transition_permitted: false,
                correction_delta: 1,
                infrastructure_impact: InfrastructureImpact::Unchanged,
                directive: GateDirective::Correct,
            },
            GateOutcome::Error(error) => GateJudgement {
                transition_permitted: false,
                correction_delta: 0,
                infrastructure_impact: InfrastructureImpact::Degraded,
                directive: if error.retryable() {
                    GateDirective::Retry
                } else {
                    GateDirective::Halt
                },
            },
            GateOutcome::Timeout(_) => GateJudgement {
                transition_permitted: false,
                correction_delta: 0,
                infrastructure_impact: InfrastructureImpact::Degraded,
                directive: GateDirective::Retry,
            },
            GateOutcome::Cancelled(_) => GateJudgement {
                transition_permitted: false,
                correction_delta: 0,
                infrastructure_impact: InfrastructureImpact::Unchanged,
                directive: GateDirective::AwaitCancellationResolution,
            },
            GateOutcome::Stale(_) => GateJudgement {
                transition_permitted: false,
                correction_delta: 0,
                infrastructure_impact: InfrastructureImpact::Unchanged,
                directive: GateDirective::Reevaluate,
            },
        }
    }

    /// Revalidates a recorded result against the current snapshot before judging it.
    ///
    /// A formerly passing result whose key changed is converted to `Stale` by
    /// the domain contract and can therefore never permit a transition here.
    pub fn judge_result(result: &GateResult, current: &GateKey) -> GateJudgement {
        Self::judge(&result.outcome_against(current))
    }
}
