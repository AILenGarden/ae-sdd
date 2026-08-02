use ae_sdd_contracts::{
    BoundedText, ReasonCode, SchemaVersion, SeriesKind, SpecKind, TaskKind,
    series::{ImpactFact, ImpactLevel, RouteDecisionError, RouteDisposition, RouteInput},
};
use ae_sdd_domain::{DesignRoute, InputFingerprint, WorkItemId, WorkScale};
use ae_sdd_flow::{RouteEngine, RouteEngineError};
use ae_sdd_protocol::ConfirmationRef;

fn route_input(confidence_bps: u16, impacts: Vec<ImpactFact>) -> RouteInput {
    route_input_with_approval(confidence_bps, impacts, None)
}

fn route_input_with_approval(
    confidence_bps: u16,
    impacts: Vec<ImpactFact>,
    user_approval_ref: Option<ConfirmationRef>,
) -> RouteInput {
    RouteInput::new(
        SchemaVersion::V1,
        WorkItemId::new("STORY-FLOW-001").expect("work item"),
        ReasonCode::new("entry.feature").expect("entry node"),
        BoundedText::new("implement the typed control-plane flow").expect("bounded intent"),
        TaskKind::Implementation,
        Vec::new(),
        impacts,
        confidence_bps,
        InputFingerprint::digest(b"typed route facts"),
        user_approval_ref,
    )
    .expect("valid route input")
}

#[test]
fn low_confidence_waits_for_approval_without_downgrading_typed_impact() {
    let input = route_input(
        4_999,
        vec![ImpactFact::new(
            ReasonCode::new("impact.cross_module").expect("impact code"),
            ImpactLevel::Medium,
            None,
        )],
    );

    let first = RouteEngine::default()
        .decide(&input)
        .expect("route decision");
    let replay = RouteEngine::default()
        .decide(&input)
        .expect("deterministic replay");

    assert_eq!(first.disposition(), RouteDisposition::AwaitUserApproval);
    assert_eq!(first.scale(), WorkScale::Medium);
    assert_eq!(first, replay);
    assert_eq!(first.decision_digest(), replay.decision_digest());
}

#[test]
fn high_impact_waits_for_explicit_bound_approval() {
    let input = route_input(
        9_000,
        vec![ImpactFact::new(
            ReasonCode::new("impact.security_boundary").expect("impact code"),
            ImpactLevel::High,
            None,
        )],
    );

    let decision = RouteEngine::default()
        .decide(&input)
        .expect("route decision");

    assert_eq!(decision.scale(), WorkScale::Large);
    assert_eq!(decision.disposition(), RouteDisposition::AwaitUserApproval);
}

#[test]
fn matching_high_impact_approval_unlocks_the_exact_candidate() {
    let impacts = vec![ImpactFact::new(
        ReasonCode::new("impact.security_boundary").expect("impact code"),
        ImpactLevel::High,
        None,
    )];
    let engine = RouteEngine::default();
    let candidate = route_input(9_000, impacts.clone());
    let binding = engine
        .approval_binding(&candidate)
        .expect("approval binding");
    let approved = route_input_with_approval(
        9_000,
        impacts,
        Some(ConfirmationRef {
            confirmation_id: format!("route:{binding}"),
            approved_by: "user".to_owned(),
            approved_at: "2026-07-24T00:00:00Z".to_owned(),
        }),
    );

    let decision = engine.decide(&approved).expect("approved route decision");

    assert_eq!(decision.disposition(), RouteDisposition::Approved);
    assert_eq!(decision.scale(), WorkScale::Large);
}

#[test]
fn conflicting_typed_facts_wait_even_when_an_approval_is_present() {
    let code = ReasonCode::new("impact.boundary").expect("impact code");
    let impacts = vec![
        ImpactFact::new(code.clone(), ImpactLevel::Low, None),
        ImpactFact::new(code, ImpactLevel::High, None),
    ];
    let engine = RouteEngine::default();
    let candidate = route_input(9_000, impacts.clone());
    let binding = engine
        .approval_binding(&candidate)
        .expect("approval binding");
    let input = route_input_with_approval(
        9_000,
        impacts,
        Some(ConfirmationRef {
            confirmation_id: format!("route:{binding}"),
            approved_by: "user".to_owned(),
            approved_at: "2026-07-24T00:00:00Z".to_owned(),
        }),
    );

    let decision = engine.decide(&input).expect("route decision");

    assert_eq!(decision.scale(), WorkScale::Large);
    assert_eq!(decision.disposition(), RouteDisposition::AwaitUserApproval);
}

#[test]
fn missing_typed_impact_facts_never_fall_back_to_prompt_inference() {
    let input = route_input(9_000, Vec::new());

    let decision = RouteEngine::default()
        .decide(&input)
        .expect("route decision");

    assert_eq!(decision.disposition(), RouteDisposition::AwaitUserApproval);
}

#[test]
fn malformed_confirmation_is_rejected_instead_of_treated_as_absent() {
    let impacts = vec![ImpactFact::new(
        ReasonCode::new("impact.security_boundary").expect("impact code"),
        ImpactLevel::High,
        None,
    )];
    let engine = RouteEngine::default();
    let candidate = route_input(9_000, impacts.clone());
    let binding = engine
        .approval_binding(&candidate)
        .expect("approval binding");
    let input = route_input_with_approval(
        9_000,
        impacts,
        Some(ConfirmationRef {
            confirmation_id: format!("route:{binding}"),
            approved_by: String::new(),
            approved_at: "2026-07-24T00:00:00Z".to_owned(),
        }),
    );

    assert_eq!(
        engine.decide(&input),
        Err(RouteEngineError::InvalidConfirmation)
    );

    for approval in [
        ConfirmationRef {
            confirmation_id: "x".repeat(129),
            approved_by: "user".to_owned(),
            approved_at: "2026-07-24T00:00:00Z".to_owned(),
        },
        ConfirmationRef {
            confirmation_id: format!("route:{binding}"),
            approved_by: "user".to_owned(),
            approved_at: "2026-07-24 00:00:00".to_owned(),
        },
    ] {
        let invalid = route_input_with_approval(
            9_000,
            vec![ImpactFact::new(
                ReasonCode::new("impact.security_boundary").expect("impact code"),
                ImpactLevel::High,
                None,
            )],
            Some(approval),
        );
        assert_eq!(
            engine.decide(&invalid),
            Err(RouteEngineError::InvalidConfirmation)
        );
    }
}

#[test]
fn impact_fact_permutation_preserves_binding_and_decision_digest() {
    let first_fact = ImpactFact::new(
        ReasonCode::new("impact.api").expect("impact code"),
        ImpactLevel::Medium,
        Some(ae_sdd_domain::ArtifactDigest::digest(b"api evidence")),
    );
    let second_fact = ImpactFact::new(
        ReasonCode::new("impact.storage").expect("impact code"),
        ImpactLevel::Low,
        Some(ae_sdd_domain::ArtifactDigest::digest(b"storage evidence")),
    );
    let micro_fact = ImpactFact::new(
        ReasonCode::new("impact.local_rename").expect("impact code"),
        ImpactLevel::Micro,
        Some(ae_sdd_domain::ArtifactDigest::digest(b"rename evidence")),
    );
    let forward = route_input(
        9_000,
        vec![first_fact.clone(), second_fact.clone(), micro_fact.clone()],
    );
    let reversed = route_input(9_000, vec![micro_fact, second_fact, first_fact]);
    let engine = RouteEngine::default();

    let forward_binding = engine.approval_binding(&forward).expect("forward binding");
    let reversed_binding = engine
        .approval_binding(&reversed)
        .expect("reversed binding");
    let forward_decision = engine.decide(&forward).expect("forward decision");
    let reversed_decision = engine.decide(&reversed).expect("reversed decision");

    assert_eq!(forward_binding, reversed_binding);
    assert_eq!(forward_decision, reversed_decision);
    assert_eq!(
        forward_decision.decision_digest(),
        reversed_decision.decision_digest()
    );
    // The higher level still wins when micro participates in the fact set.
    assert_eq!(forward_decision.scale(), WorkScale::Medium);
}

/// §7.1 line 342 gives micro the route `RA -> executionPlan -> Coding` and states
/// its minimum persisted artifacts are "RA Spec + daemon state 中经批准的
/// `executionPlan`；不要求独立 CodingPlan Markdown". Delegating a CodingPlan Series
/// here produced exactly the Spec the design says must not be required, and left
/// micro with the same `requiredSeries` as small — so the two tiers became
/// indistinguishable from the decision alone.
#[test]
fn all_micro_facts_select_the_micro_coding_plan_route() {
    let impacts = vec![
        ImpactFact::new(
            ReasonCode::new("impact.local_rename").expect("impact code"),
            ImpactLevel::Micro,
            None,
        ),
        ImpactFact::new(
            ReasonCode::new("impact.comment_tweak").expect("impact code"),
            ImpactLevel::Micro,
            None,
        ),
    ];

    let decision = RouteEngine::default()
        .decide(&route_input(9_000, impacts))
        .expect("route decision");

    assert_eq!(decision.scale(), WorkScale::Micro);
    assert_eq!(decision.design_route(), DesignRoute::CodingPlan);
    assert_eq!(decision.disposition(), RouteDisposition::Approved);
    let series: Vec<&str> = decision
        .required_series()
        .iter()
        .map(SeriesKind::as_str)
        .collect();
    assert_eq!(
        series,
        ["requirement-analysis"],
        "§7.1 line 342 does not require a standalone CodingPlan for micro"
    );
    assert_eq!(
        decision.required_spec_kinds(),
        [SpecKind::RequirementAnalysis],
        "micro binds the RA Spec only; the approved executionPlan is its plan"
    );
    assert_eq!(
        decision.design_route(),
        DesignRoute::CodingPlan,
        "design *depth* stays the shallowest tier even though no CodingPlan Spec is          required: `DesignRoute` and `requiredSpecKinds` answer different questions"
    );
}

#[test]
fn micro_never_dominates_higher_impact_levels() {
    let engine = RouteEngine::default();
    for (level, scale) in [
        (ImpactLevel::Low, WorkScale::Small),
        (ImpactLevel::Medium, WorkScale::Medium),
        (ImpactLevel::High, WorkScale::Large),
    ] {
        let micro_fact = ImpactFact::new(
            ReasonCode::new("impact.local_rename").expect("impact code"),
            ImpactLevel::Micro,
            None,
        );
        let higher_fact = ImpactFact::new(
            ReasonCode::new("impact.scope").expect("impact code"),
            level,
            None,
        );
        let forward = route_input(9_000, vec![micro_fact.clone(), higher_fact.clone()]);
        let reversed = route_input(9_000, vec![higher_fact, micro_fact]);

        let forward_decision = engine.decide(&forward).expect("forward decision");
        let reversed_decision = engine.decide(&reversed).expect("reversed decision");

        assert_eq!(forward_decision.scale(), scale);
        assert_eq!(forward_decision, reversed_decision);
        assert_eq!(
            forward_decision.decision_digest(),
            reversed_decision.decision_digest()
        );
    }
}

#[test]
fn conflicting_micro_facts_wait_for_approval_instead_of_selecting_micro() {
    let code = ReasonCode::new("impact.boundary").expect("impact code");
    let impacts = vec![
        ImpactFact::new(code.clone(), ImpactLevel::Micro, None),
        ImpactFact::new(code, ImpactLevel::Medium, None),
    ];

    let decision = RouteEngine::default()
        .decide(&route_input(9_000, impacts))
        .expect("route decision");

    assert_eq!(decision.disposition(), RouteDisposition::AwaitUserApproval);
    assert_eq!(decision.scale(), WorkScale::Medium);
}

/// Pins the impact-to-route mapping and each route's `decisionDigest`.
///
/// The four expected rows are transcribed from the §7.1 table, not from what the
/// code happened to produce: micro `RA -> executionPlan -> Coding` (no CodingPlan
/// Spec), small `RA -> CodingPlan`, medium `RA -> Story -> TestCase -> CodingPlan`,
/// large `RA -> DR -> N x (Story -> TestCase -> CodingPlan)`.
///
/// All four digests moved on 2026-08-02 when `taskKind` and `requiredSpecKinds`
/// entered `digest_route`, and medium/large moved additionally because
/// `coding-plan` joined their `requiredSeries`. `digest_route` folds each list as
/// count-then-elements, so any list change necessarily moves the digest. A digest
/// moving *without* a matching change here is a real regression; treat a surprise
/// diff as one until proven otherwise.
#[test]
fn impact_mappings_and_decision_digests_stay_pinned() {
    let engine = RouteEngine::default();
    let cases: [(
        ImpactLevel,
        WorkScale,
        DesignRoute,
        &[&str],
        &[SpecKind],
        &str,
    ); 4] = [
        (
            ImpactLevel::Micro,
            WorkScale::Micro,
            DesignRoute::CodingPlan,
            &["requirement-analysis"],
            &[SpecKind::RequirementAnalysis],
            "b087c87c4b7009fdd894589691c0373a6b19232d03f5ac610b1029d5b4410f86",
        ),
        (
            ImpactLevel::Low,
            WorkScale::Small,
            DesignRoute::CodingPlan,
            &["requirement-analysis", "coding-plan"],
            &[SpecKind::RequirementAnalysis, SpecKind::CodingPlan],
            "f48a40ea13aea1acc4f7a9a6a1c2f4fb7600610303896f6f61008e9c4b596c10",
        ),
        (
            ImpactLevel::Medium,
            WorkScale::Medium,
            DesignRoute::Story,
            &["requirement-analysis", "story", "testcase", "coding-plan"],
            &[
                SpecKind::RequirementAnalysis,
                SpecKind::Story,
                SpecKind::TestCase,
                SpecKind::CodingPlan,
            ],
            "354d47fe9c27f4354a95f233734f72e931b74b62e65a93cff7d7cbd624319908",
        ),
        (
            ImpactLevel::High,
            WorkScale::Large,
            DesignRoute::Dr,
            &[
                "requirement-analysis",
                "design-review",
                "story",
                "testcase",
                "coding-plan",
            ],
            &[
                SpecKind::RequirementAnalysis,
                SpecKind::DesignReview,
                SpecKind::Story,
                SpecKind::TestCase,
                SpecKind::CodingPlan,
            ],
            "90d5a2be7b8fb140c9f392ed19117791bb72d145fa7200072f4c77d1f7b1b696",
        ),
    ];
    for (level, scale, design_route, series, spec_kinds, digest) in cases {
        let decision = engine
            .decide(&route_input(
                9_000,
                vec![ImpactFact::new(
                    ReasonCode::new("impact.scope").expect("impact code"),
                    level,
                    None,
                )],
            ))
            .expect("route decision");
        assert_eq!(decision.scale(), scale);
        assert_eq!(decision.design_route(), design_route);
        let names: Vec<&str> = decision
            .required_series()
            .iter()
            .map(SeriesKind::as_str)
            .collect();
        assert_eq!(names, series, "{level:?} requiredSeries matches §7.1");
        assert_eq!(
            decision.required_spec_kinds(),
            spec_kinds,
            "{level:?} requiredSpecKinds matches the §7.1 最低持久化设计产物 column"
        );
        assert_eq!(
            decision.task_kind(),
            TaskKind::Implementation,
            "the decision freezes the task kind it was given, it does not invent one"
        );
        assert_eq!(decision.decision_digest().to_string(), digest);
    }
    // Missing facts keep the frozen low mapping and never default to micro.
    let missing = engine
        .decide(&route_input(9_000, Vec::new()))
        .expect("route decision");
    assert_eq!(missing.disposition(), RouteDisposition::AwaitUserApproval);
    assert_eq!(missing.scale(), WorkScale::Small);
    assert_eq!(
        missing.decision_digest().to_string(),
        "074932c8b3738fd177015c8fc1a5a3ce5af6a7aaeb2251a520f37b76f4e07707"
    );
}

#[test]
fn route_engine_configuration_and_errors_are_explicit() {
    assert_eq!(
        RouteEngine::new(10_001),
        Err(RouteEngineError::InvalidConfidenceThreshold)
    );
    let engine = RouteEngine::new(5_000).expect("threshold at 50 percent is valid");
    assert_eq!(
        engine
            .decide(&route_input(9_000, Vec::new()))
            .unwrap()
            .scale(),
        WorkScale::Small
    );

    for error in [
        RouteEngineError::InvalidConfidenceThreshold,
        RouteEngineError::ContractEncoding,
        RouteEngineError::InvalidConfirmation,
        RouteEngineError::InvariantViolation,
        RouteEngineError::DecisionContract(RouteDecisionError::MissingReason),
    ] {
        assert!(!error.to_string().is_empty());
    }
}

/// The baseline flow is `RA → DR → N × (Story → TestCase → CodingPlan)`, so a
/// route that requires a Story requires its TestCase too. `classify_impacts`
/// omitted `testcase`, and the handoff gates that Series behind
/// `requires("testcase")` reading this very field — so a medium or large route
/// went straight from Story to CodingPlan and no delegation ever produced a
/// TestCase. Backfilling stored state patched existing Work Items only; new
/// ones need the mapping itself fixed.
#[test]
fn story_bearing_routes_require_a_testcase_series() {
    let engine = RouteEngine::default();
    for (level, expected) in [
        (
            ImpactLevel::Medium,
            vec!["requirement-analysis", "story", "testcase", "coding-plan"],
        ),
        (
            ImpactLevel::High,
            vec![
                "requirement-analysis",
                "design-review",
                "story",
                "testcase",
                "coding-plan",
            ],
        ),
    ] {
        let decision = engine
            .decide(&route_input(
                9_000,
                vec![ImpactFact::new(
                    ReasonCode::new("impact.scope").expect("impact code"),
                    level,
                    None,
                )],
            ))
            .expect("route decision");
        let names: Vec<&str> = decision
            .required_series()
            .iter()
            .map(SeriesKind::as_str)
            .collect();
        assert_eq!(
            names, expected,
            "a route that requires a Story must require its TestCase: {level:?}"
        );
    }
}
