use ae_sdd_contracts::{
    BoundedText, ReasonCode, SchemaVersion,
    series::{ImpactFact, ImpactLevel, RouteDecisionError, RouteDisposition, RouteInput},
};
use ae_sdd_domain::{InputFingerprint, WorkItemId, WorkScale};
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
    let forward = route_input(9_000, vec![first_fact.clone(), second_fact.clone()]);
    let reversed = route_input(9_000, vec![second_fact, first_fact]);
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
