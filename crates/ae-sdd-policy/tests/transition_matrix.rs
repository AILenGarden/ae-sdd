use std::collections::BTreeSet;

use ae_sdd_domain::{AgentRole, DesignRoute, ProcessPhase, WorkScale};
use ae_sdd_policy::{
    RequiredGate, RoleOperation, RolePolicy, TransitionContext, TransitionPolicy,
    TransitionPolicyError,
};

const ALL_GATES: [RequiredGate; 26] = [
    RequiredGate::G00,
    RequiredGate::G01,
    RequiredGate::G02,
    RequiredGate::G03,
    RequiredGate::G04,
    RequiredGate::G07,
    RequiredGate::G08,
    RequiredGate::G09,
    RequiredGate::G10,
    RequiredGate::G11,
    RequiredGate::G12,
    RequiredGate::G13,
    RequiredGate::G14,
    RequiredGate::GCode1,
    RequiredGate::GCodePlanSource,
    RequiredGate::GDrContext,
    RequiredGate::GHttp1,
    RequiredGate::GRa1,
    RequiredGate::GRa2,
    RequiredGate::GRa3,
    RequiredGate::GRa4,
    RequiredGate::GRa5,
    RequiredGate::GRa6,
    RequiredGate::GRaFlowViolation,
    RequiredGate::GReviewDepth,
    RequiredGate::GStoryContext,
];

fn context(
    current: ProcessPhase,
    target: ProcessPhase,
    scale: WorkScale,
    design_route: DesignRoute,
) -> TransitionContext {
    TransitionContext {
        actor_role: AgentRole::Root,
        current,
        target,
        scale,
        design_route,
        paused_from: None,
    }
}

#[test]
fn every_required_gate_has_a_unique_stable_registry_identifier() {
    let expected = [
        "G-00",
        "G-01",
        "G-02",
        "G-03",
        "G-04",
        "G-07",
        "G-08",
        "G-09",
        "G-10",
        "G-11",
        "G-12",
        "G-13",
        "G-14",
        "G-CODE-1",
        "G-CODEPLAN-SRC",
        "G-DR-CTX",
        "G-HTTP-1",
        "G-RA-1",
        "G-RA-2",
        "G-RA-3",
        "G-RA-4",
        "G-RA-5",
        "G-RA-6",
        "G-RA-FLOW-VIOLATION",
        "G-REVIEW-DEPTH",
        "G-STORY-CTX",
    ];
    assert_eq!(ALL_GATES.map(RequiredGate::as_str), expected);
    assert_eq!(
        ALL_GATES
            .map(RequiredGate::as_str)
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len(),
        ALL_GATES.len()
    );
}

#[test]
fn every_supported_route_authorizes_only_adjacent_steps() {
    // RA-first: every chain is Initialized -> RequirementAnalyzed -> RouteSelected -> downstream.
    let large_dr = [
        ProcessPhase::Initialized,
        ProcessPhase::RequirementAnalyzed,
        ProcessPhase::RouteSelected,
        ProcessPhase::DrGenerated,
        ProcessPhase::StoryGenerated,
        ProcessPhase::TestcaseGenerated,
        ProcessPhase::CodingProcess,
        ProcessPhase::Coding,
        ProcessPhase::TestRunning,
        ProcessPhase::CodeReviewed,
        ProcessPhase::Completed,
    ];
    let story = [
        ProcessPhase::Initialized,
        ProcessPhase::RequirementAnalyzed,
        ProcessPhase::RouteSelected,
        ProcessPhase::StoryGenerated,
        ProcessPhase::TestcaseGenerated,
        ProcessPhase::CodingProcess,
        ProcessPhase::Coding,
        ProcessPhase::TestRunning,
        ProcessPhase::CodeReviewed,
        ProcessPhase::Completed,
    ];
    let coding_plan = [
        ProcessPhase::Initialized,
        ProcessPhase::RequirementAnalyzed,
        ProcessPhase::RouteSelected,
        ProcessPhase::CodingProcess,
        ProcessPhase::Coding,
        ProcessPhase::TestRunning,
        ProcessPhase::CodeReviewed,
        ProcessPhase::Completed,
    ];
    let routes = [
        (WorkScale::Large, DesignRoute::Dr, large_dr.as_slice()),
        (WorkScale::Large, DesignRoute::Story, story.as_slice()),
        (WorkScale::Medium, DesignRoute::Story, story.as_slice()),
        (
            WorkScale::Large,
            DesignRoute::CodingPlan,
            coding_plan.as_slice(),
        ),
        (
            WorkScale::Medium,
            DesignRoute::CodingPlan,
            coding_plan.as_slice(),
        ),
        (
            WorkScale::Small,
            DesignRoute::CodingPlan,
            coding_plan.as_slice(),
        ),
        (
            WorkScale::Micro,
            DesignRoute::CodingPlan,
            coding_plan.as_slice(),
        ),
    ];

    for (scale, route, phases) in routes {
        for pair in phases.windows(2) {
            let permit = TransitionPolicy::authorize(context(pair[0], pair[1], scale, route))
                .unwrap_or_else(|error| panic!("{scale:?}/{route:?} {pair:?}: {error}"));
            if pair[1] == ProcessPhase::Coding {
                assert!(
                    permit
                        .required_gates()
                        .contains(&RequiredGate::GCodePlanSource)
                );
            }
        }
    }
    assert_eq!(TransitionPolicy::digest(), ae_sdd_policy::policy_digest());
}

#[test]
fn transition_denials_preserve_typed_context_and_actionable_messages() {
    let role = RolePolicy::authorize(AgentRole::Task, RoleOperation::RequestGlobalTransition)
        .expect_err("task cannot own global transition");
    assert_eq!(role.role(), AgentRole::Task);
    assert_eq!(role.operation(), RoleOperation::RequestGlobalTransition);
    assert!(role.to_string().contains("Task"));

    let errors = [
        TransitionPolicyError::from(role),
        TransitionPolicy::authorize(context(
            ProcessPhase::Initialized,
            ProcessPhase::RouteSelected,
            WorkScale::Small,
            DesignRoute::Story,
        ))
        .expect_err("small Story route is unsupported"),
        TransitionPolicy::authorize(context(
            ProcessPhase::DrGenerated,
            ProcessPhase::TestcaseGenerated,
            WorkScale::Large,
            DesignRoute::Dr,
        ))
        .expect_err("skipping StoryGenerated on the large DR route is illegal"),
        // RA-first: Initialized -> RouteSelected is now illegal before RA closes.
        TransitionPolicy::authorize(context(
            ProcessPhase::Initialized,
            ProcessPhase::RouteSelected,
            WorkScale::Large,
            DesignRoute::Dr,
        ))
        .expect_err("Initialized -> RouteSelected must be denied before RA"),
        TransitionPolicy::authorize(TransitionContext {
            actor_role: AgentRole::Root,
            current: ProcessPhase::Paused,
            target: ProcessPhase::Coding,
            scale: WorkScale::Large,
            design_route: DesignRoute::Dr,
            paused_from: Some(ProcessPhase::StoryGenerated),
        })
        .expect_err("resume target must match the recorded phase"),
    ];

    for error in errors {
        assert!(!error.to_string().is_empty());
    }

    let pause = TransitionPolicy::authorize(context(
        ProcessPhase::Coding,
        ProcessPhase::Paused,
        WorkScale::Large,
        DesignRoute::Dr,
    ))
    .expect("active flow may pause");
    assert!(pause.required_gates().is_empty());
    let resume = TransitionPolicy::authorize(TransitionContext {
        actor_role: AgentRole::Root,
        current: ProcessPhase::Paused,
        target: ProcessPhase::Coding,
        scale: WorkScale::Large,
        design_route: DesignRoute::Dr,
        paused_from: Some(ProcessPhase::Coding),
    })
    .expect("flow may resume to its exact recorded phase");
    assert!(resume.required_gates().is_empty());
}

/// Task 8: RA-first ordering. Initialized -> RequirementAnalyzed is the legal
/// first step and requires only G-RA-1..4 (no G-00, no G-RA-5/6/FLOW). A route
/// cannot be selected before RA closes.
#[test]
fn initialized_to_requirement_analyzed_is_legal_without_a_route() {
    let permit = TransitionPolicy::authorize(context(
        ProcessPhase::Initialized,
        ProcessPhase::RequirementAnalyzed,
        WorkScale::Large,
        DesignRoute::Dr,
    ))
    .expect("Initialized -> RequirementAnalyzed is the RA-first first step");
    assert_eq!(
        permit.required_gates(),
        [
            RequiredGate::GRa1,
            RequiredGate::GRa2,
            RequiredGate::GRa3,
            RequiredGate::GRa4,
        ]
    );
}

#[test]
fn initialized_to_route_selected_is_illegal_before_ra() {
    let denial = TransitionPolicy::authorize(context(
        ProcessPhase::Initialized,
        ProcessPhase::RouteSelected,
        WorkScale::Large,
        DesignRoute::Dr,
    ))
    .expect_err("Initialized -> RouteSelected must be denied before RA");
    assert!(matches!(
        denial,
        TransitionPolicyError::IllegalTransition { .. }
    ));
}

#[test]
fn requirement_analyzed_to_route_selected_requires_only_flow_gate() {
    let permit = TransitionPolicy::authorize(context(
        ProcessPhase::RequirementAnalyzed,
        ProcessPhase::RouteSelected,
        WorkScale::Large,
        DesignRoute::Dr,
    ))
    .expect("RequirementAnalyzed -> RouteSelected is the RA-first route freeze step");
    assert_eq!(permit.required_gates(), [RequiredGate::GRaFlowViolation]);
}
