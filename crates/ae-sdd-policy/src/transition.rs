use std::{error::Error, fmt};

use ae_sdd_domain::{AgentRole, DesignRoute, PolicyDigest, ProcessPhase, WorkScale};

use crate::{RoleAuthorizationError, RoleOperation, RolePolicy};

const LARGE_DR: &[ProcessPhase] = &[
    ProcessPhase::Initialized,
    ProcessPhase::RouteSelected,
    ProcessPhase::RequirementAnalyzed,
    ProcessPhase::DrGenerated,
    ProcessPhase::TestcaseGenerated,
    ProcessPhase::CodingProcess,
    ProcessPhase::Coding,
    ProcessPhase::TestRunning,
    ProcessPhase::CodeReviewed,
    ProcessPhase::Completed,
];
const STORY: &[ProcessPhase] = &[
    ProcessPhase::Initialized,
    ProcessPhase::RouteSelected,
    ProcessPhase::RequirementAnalyzed,
    ProcessPhase::StoryGenerated,
    ProcessPhase::TestcaseGenerated,
    ProcessPhase::CodingProcess,
    ProcessPhase::Coding,
    ProcessPhase::TestRunning,
    ProcessPhase::CodeReviewed,
    ProcessPhase::Completed,
];
const CODING_PLAN: &[ProcessPhase] = &[
    ProcessPhase::Initialized,
    ProcessPhase::RouteSelected,
    ProcessPhase::RequirementAnalyzed,
    ProcessPhase::CodingProcess,
    ProcessPhase::Coding,
    ProcessPhase::TestRunning,
    ProcessPhase::CodeReviewed,
    ProcessPhase::Completed,
];

const G_NONE: &[RequiredGate] = &[];
const G_00: &[RequiredGate] = &[RequiredGate::G00];
const G_RA: &[RequiredGate] = &[
    RequiredGate::G00,
    RequiredGate::GRa1,
    RequiredGate::GRa2,
    RequiredGate::GRa3,
    RequiredGate::GRa4,
    RequiredGate::GRa5,
    RequiredGate::GRa6,
    RequiredGate::GRaFlowViolation,
];
const G_DR: &[RequiredGate] = &[
    RequiredGate::G00,
    RequiredGate::G01,
    RequiredGate::GDrContext,
];
const G_STORY: &[RequiredGate] = &[
    RequiredGate::G00,
    RequiredGate::G02,
    RequiredGate::G03,
    RequiredGate::GStoryContext,
    RequiredGate::GReviewDepth,
];
const G_TESTCASE: &[RequiredGate] = &[
    RequiredGate::G00,
    RequiredGate::G02,
    RequiredGate::G03,
    RequiredGate::G04,
    RequiredGate::GStoryContext,
];
const G_CODING: &[RequiredGate] = &[
    RequiredGate::G00,
    RequiredGate::G07,
    RequiredGate::GCodePlanSource,
    RequiredGate::G14,
    RequiredGate::G08,
    RequiredGate::GHttp1,
];
const G_REVIEW: &[RequiredGate] = &[
    RequiredGate::G00,
    RequiredGate::G09,
    RequiredGate::GCode1,
    RequiredGate::G10,
    RequiredGate::G11,
    RequiredGate::GReviewDepth,
];
const G_COMPLETED: &[RequiredGate] = &[RequiredGate::G00, RequiredGate::G12, RequiredGate::G13];

/// Typed Gate identities referenced by the transition-entry policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequiredGate {
    G00,
    G01,
    G02,
    G03,
    G04,
    G07,
    G08,
    G09,
    G10,
    G11,
    G12,
    G13,
    G14,
    GCode1,
    GCodePlanSource,
    GDrContext,
    GHttp1,
    GRa1,
    GRa2,
    GRa3,
    GRa4,
    GRa5,
    GRa6,
    GRaFlowViolation,
    GReviewDepth,
    GStoryContext,
}

impl RequiredGate {
    /// Returns the stable Gate identifier used by the Gate registry.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::G00 => "G-00",
            Self::G01 => "G-01",
            Self::G02 => "G-02",
            Self::G03 => "G-03",
            Self::G04 => "G-04",
            Self::G07 => "G-07",
            Self::G08 => "G-08",
            Self::G09 => "G-09",
            Self::G10 => "G-10",
            Self::G11 => "G-11",
            Self::G12 => "G-12",
            Self::G13 => "G-13",
            Self::G14 => "G-14",
            Self::GCode1 => "G-CODE-1",
            Self::GCodePlanSource => "G-CODEPLAN-SRC",
            Self::GDrContext => "G-DR-CTX",
            Self::GHttp1 => "G-HTTP-1",
            Self::GRa1 => "G-RA-1",
            Self::GRa2 => "G-RA-2",
            Self::GRa3 => "G-RA-3",
            Self::GRa4 => "G-RA-4",
            Self::GRa5 => "G-RA-5",
            Self::GRa6 => "G-RA-6",
            Self::GRaFlowViolation => "G-RA-FLOW-VIOLATION",
            Self::GReviewDepth => "G-REVIEW-DEPTH",
            Self::GStoryContext => "G-STORY-CTX",
        }
    }
}

/// Explicit input required to authorize one process transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransitionContext {
    pub actor_role: AgentRole,
    pub current: ProcessPhase,
    pub target: ProcessPhase,
    pub scale: WorkScale,
    pub design_route: DesignRoute,
    pub paused_from: Option<ProcessPhase>,
}

/// Successful transition authorization and its required Gate set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransitionPermit {
    required_gates: &'static [RequiredGate],
}

impl TransitionPermit {
    /// Returns all Gates that must produce fresh `Pass` before mutation.
    pub const fn required_gates(self) -> &'static [RequiredGate] {
        self.required_gates
    }
}

/// Single deterministic owner of legal phase transitions and entry Gates.
#[derive(Clone, Copy, Debug, Default)]
pub struct TransitionPolicy;

impl TransitionPolicy {
    /// Returns the digest of this policy revision.
    ///
    /// The version changes whenever a transition, Gate, or role rule changes.
    pub fn digest() -> PolicyDigest {
        crate::policy_digest()
    }

    /// Authorizes a direct transition for the selected route.
    ///
    /// Only a root Agent may request the global transition. Pause is a
    /// meta-transition; resume must return to the exact recorded phase.
    pub fn authorize(
        context: TransitionContext,
    ) -> Result<TransitionPermit, TransitionPolicyError> {
        RolePolicy::authorize(context.actor_role, RoleOperation::RequestGlobalTransition)?;

        if context.target == ProcessPhase::Paused {
            if context.current == ProcessPhase::Paused || context.current == ProcessPhase::Completed
            {
                return Err(TransitionPolicyError::IllegalTransition {
                    current: context.current,
                    target: context.target,
                });
            }
            return Ok(TransitionPermit {
                required_gates: G_NONE,
            });
        }

        if context.current == ProcessPhase::Paused {
            if context.paused_from != Some(context.target) {
                return Err(TransitionPolicyError::InvalidResume {
                    recorded: context.paused_from,
                    target: context.target,
                });
            }
            return Ok(TransitionPermit {
                required_gates: G_NONE,
            });
        }

        let chain = route_chain(context.scale, context.design_route)?;
        let Some(index) = chain.iter().position(|phase| *phase == context.current) else {
            return Err(TransitionPolicyError::PhaseOutsideRoute {
                phase: context.current,
            });
        };
        if chain.get(index + 1) != Some(&context.target) {
            return Err(TransitionPolicyError::IllegalTransition {
                current: context.current,
                target: context.target,
            });
        }

        Ok(TransitionPermit {
            required_gates: required_gates(context.scale, context.design_route, context.target),
        })
    }

    /// Returns whether the phase hosts supervised approved-slice execution.
    ///
    /// Only `Coding` executes an approved slice queue; every other phase keeps
    /// the regular transition/Gate actions, so the phase table remains the
    /// single owner of where the execution surface may run.
    pub const fn is_execution_phase(phase: ProcessPhase) -> bool {
        matches!(phase, ProcessPhase::Coding)
    }
}

fn route_chain(
    scale: WorkScale,
    design_route: DesignRoute,
) -> Result<&'static [ProcessPhase], TransitionPolicyError> {
    match (scale, design_route) {
        (WorkScale::Large, DesignRoute::Dr) => Ok(LARGE_DR),
        (WorkScale::Large | WorkScale::Medium, DesignRoute::Story) => Ok(STORY),
        (WorkScale::Large | WorkScale::Medium, DesignRoute::CodingPlan)
        | (WorkScale::Small | WorkScale::Micro, DesignRoute::CodingPlan) => Ok(CODING_PLAN),
        _ => Err(TransitionPolicyError::UnsupportedRoute {
            scale,
            design_route,
        }),
    }
}

fn required_gates(
    _scale: WorkScale,
    design_route: DesignRoute,
    target: ProcessPhase,
) -> &'static [RequiredGate] {
    match target {
        ProcessPhase::RouteSelected => G_00,
        ProcessPhase::RequirementAnalyzed => G_RA,
        ProcessPhase::DrGenerated => G_DR,
        ProcessPhase::StoryGenerated => G_STORY,
        ProcessPhase::TestcaseGenerated => G_NONE,
        ProcessPhase::CodingProcess if design_route != DesignRoute::CodingPlan => G_TESTCASE,
        ProcessPhase::CodingProcess => G_00,
        ProcessPhase::Coding => G_CODING,
        ProcessPhase::TestRunning => G_00,
        ProcessPhase::CodeReviewed => G_REVIEW,
        ProcessPhase::Completed => G_COMPLETED,
        ProcessPhase::Initialized | ProcessPhase::Paused => G_NONE,
    }
}

/// Typed reason why a requested transition was denied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionPolicyError {
    Role(RoleAuthorizationError),
    UnsupportedRoute {
        scale: WorkScale,
        design_route: DesignRoute,
    },
    PhaseOutsideRoute {
        phase: ProcessPhase,
    },
    IllegalTransition {
        current: ProcessPhase,
        target: ProcessPhase,
    },
    InvalidResume {
        recorded: Option<ProcessPhase>,
        target: ProcessPhase,
    },
}

impl From<RoleAuthorizationError> for TransitionPolicyError {
    fn from(error: RoleAuthorizationError) -> Self {
        Self::Role(error)
    }
}

impl fmt::Display for TransitionPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Role(error) => error.fmt(formatter),
            Self::UnsupportedRoute {
                scale,
                design_route,
            } => write!(
                formatter,
                "design route {design_route:?} is unsupported for scale {scale:?}"
            ),
            Self::PhaseOutsideRoute { phase } => {
                write!(formatter, "phase {phase:?} is not in the selected route")
            }
            Self::IllegalTransition { current, target } => write!(
                formatter,
                "transition from {current:?} to {target:?} is not a direct route step"
            ),
            Self::InvalidResume { recorded, target } => write!(
                formatter,
                "paused flow recorded {recorded:?}, so it cannot resume to {target:?}"
            ),
        }
    }
}

impl Error for TransitionPolicyError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(role: AgentRole, current: ProcessPhase, target: ProcessPhase) -> TransitionContext {
        TransitionContext {
            actor_role: role,
            current,
            target,
            scale: WorkScale::Large,
            design_route: DesignRoute::Story,
            paused_from: None,
        }
    }

    #[test]
    fn direct_root_transition_returns_entry_gates() {
        let permit = TransitionPolicy::authorize(context(
            AgentRole::Root,
            ProcessPhase::StoryGenerated,
            ProcessPhase::TestcaseGenerated,
        ))
        .expect("direct root transition is legal");

        assert_eq!(permit.required_gates(), G_NONE);

        let coding_process = TransitionPolicy::authorize(context(
            AgentRole::Root,
            ProcessPhase::TestcaseGenerated,
            ProcessPhase::CodingProcess,
        ))
        .expect("coding-process transition is legal");
        assert_eq!(coding_process.required_gates(), G_TESTCASE);
    }

    #[test]
    fn child_and_skipped_transition_are_denied() {
        let child = TransitionPolicy::authorize(context(
            AgentRole::Series,
            ProcessPhase::StoryGenerated,
            ProcessPhase::TestcaseGenerated,
        ));
        let skipped = TransitionPolicy::authorize(context(
            AgentRole::Root,
            ProcessPhase::StoryGenerated,
            ProcessPhase::Coding,
        ));

        assert!(matches!(child, Err(TransitionPolicyError::Role(_))));
        assert!(matches!(
            skipped,
            Err(TransitionPolicyError::IllegalTransition { .. })
        ));
    }

    #[test]
    fn resume_must_match_recorded_phase() {
        let valid = TransitionContext {
            actor_role: AgentRole::Root,
            current: ProcessPhase::Paused,
            target: ProcessPhase::Coding,
            scale: WorkScale::Large,
            design_route: DesignRoute::Story,
            paused_from: Some(ProcessPhase::Coding),
        };
        let invalid = TransitionContext {
            target: ProcessPhase::TestRunning,
            ..valid
        };

        assert!(TransitionPolicy::authorize(valid).is_ok());
        assert!(matches!(
            TransitionPolicy::authorize(invalid),
            Err(TransitionPolicyError::InvalidResume { .. })
        ));
    }

    #[test]
    fn policy_digest_is_stable_for_this_revision() {
        assert_eq!(TransitionPolicy::digest(), TransitionPolicy::digest());
    }

    #[test]
    fn coding_entry_contains_all_plan_approval_gates() {
        let permit = TransitionPolicy::authorize(context(
            AgentRole::Root,
            ProcessPhase::CodingProcess,
            ProcessPhase::Coding,
        ))
        .expect("coding is the direct next phase");

        assert!(
            permit
                .required_gates()
                .contains(&RequiredGate::GCodePlanSource)
        );
        assert!(permit.required_gates().contains(&RequiredGate::G14));
        assert!(permit.required_gates().contains(&RequiredGate::G08));
    }

    #[test]
    fn only_coding_hosts_supervised_slice_execution() {
        assert!(TransitionPolicy::is_execution_phase(ProcessPhase::Coding));
        for phase in [
            ProcessPhase::Initialized,
            ProcessPhase::CodingProcess,
            ProcessPhase::TestRunning,
            ProcessPhase::CodeReviewed,
            ProcessPhase::Completed,
            ProcessPhase::Paused,
        ] {
            assert!(
                !TransitionPolicy::is_execution_phase(phase),
                "{phase:?} must not host slice execution",
            );
        }
    }
}
