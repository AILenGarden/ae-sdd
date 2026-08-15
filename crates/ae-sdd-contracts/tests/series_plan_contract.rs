use ae_sdd_contracts::resource::ContextBundleRef;
use ae_sdd_contracts::series::{ImpactFact, ImpactLevel};
use ae_sdd_contracts::{
    IdempotencyKey, MethodologyRef, MethodologyResolution, MethodologyVariant, OverrideDisposition,
    OverrideLayer, OverrideTrace, ProcessSnapshot, ReasonCode, RetryPolicy, RouteDecision,
    RouteDecisionId, RouteDisposition, SchemaVersion, SeriesId, SeriesInput, SeriesKind,
    SeriesPlan, SeriesPlanDecision, SkillId, SpecKind, TaskKind,
};
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, ArtifactKind, ArtifactRef, ContextDigest, DecisionDigest,
    DeliverableContract, DeliverableId, DeliverableRequirement, DesignRoute, InputFingerprint,
    OperationId, ProcessPhase, ProjectPathScope, ProjectRelativePath, StateRevision, WorkItemId,
    WorkScale,
};

fn artifact(path: &str, contents: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new("series-fixture").expect("artifact kind"),
        ProjectRelativePath::new(path).expect("project-relative path"),
        ArtifactDigest::digest(contents),
        u64::try_from(contents.len()).expect("fixture length fits u64"),
    )
}

#[test]
fn series_plan_round_trips_with_bounded_context_and_grant_contracts() {
    let work_item_id = WorkItemId::new("STORY-001").expect("work item id");
    let methodology_ref = MethodologyRef::new(
        SchemaVersion::V1,
        SkillId::new("phase1-design.requirement-analysis").expect("skill id"),
        SeriesKind::new("requirement-analysis").expect("series kind"),
        MethodologyVariant::new("builtin-v1").expect("variant"),
        artifact("runtime/skills/requirement-analysis/compact.md", b"compact"),
        None,
        ArtifactDigest::digest(b"methodology entry"),
        ArtifactDigest::digest(b"catalog"),
    )
    .expect("methodology ref");
    let context_ref = ContextBundleRef::new(
        SchemaVersion::V1,
        ae_sdd_contracts::ContextBundleId::new("context-STORY-001-r7").expect("context id"),
        work_item_id.clone(),
        vec![artifact("ae-sdd-doc/Story/STORY-001.md", b"story")],
        ContextDigest::digest(b"context bundle"),
        5,
    )
    .expect("context bundle ref");
    let deliverable_contract = DeliverableContract::bounded_default([DeliverableRequirement::new(
        DeliverableId::new("requirement-analysis").expect("deliverable id"),
        ArtifactKind::new("requirement-analysis").expect("artifact kind"),
        ProjectRelativePath::new("ae-sdd-doc/RA/RA-STORY-001.md").expect("deliverable path"),
    )])
    .expect("deliverable contract");
    let plan = SeriesPlan::new(
        SchemaVersion::V1,
        SeriesId::new("series-STORY-001-ra-r7").expect("series id"),
        work_item_id,
        None,
        SeriesKind::new("requirement-analysis").expect("series kind"),
        AgentRole::Series,
        methodology_ref,
        context_ref,
        deliverable_contract,
        vec![OperationId::new("document.save").expect("operation id")],
        vec![ProjectPathScope::Subtree(
            ProjectRelativePath::new("ae-sdd-doc/RA").expect("path scope"),
        )],
        Vec::new(),
        StateRevision::new(7),
        InputFingerprint::digest(b"series input"),
        1_785_000_900_000,
        RetryPolicy::new(2, 250, 2_000).expect("retry policy"),
        DecisionDigest::digest(b"series plan"),
    )
    .expect("series plan");
    let route = RouteDecision::new(
        SchemaVersion::V1,
        RouteDecisionId::new("route-STORY-001-r7").expect("route id"),
        WorkItemId::new("STORY-001").expect("work item id"),
        TaskKind::Implementation,
        WorkScale::Large,
        DesignRoute::Dr,
        RouteDisposition::Approved,
        vec![ReasonCode::new("route.large-dr").expect("reason")],
        vec![SeriesKind::new("requirement-analysis").expect("series kind")],
        vec![SpecKind::RequirementAnalysis],
        InputFingerprint::digest(b"route input"),
        Some(ArtifactDigest::digest(b"approval binding")),
        DecisionDigest::digest(b"route decision"),
    )
    .expect("route decision");
    let resolution = MethodologyResolution::new(
        SchemaVersion::V1,
        plan.methodology_ref().clone(),
        OverrideLayer::BuiltIn,
        vec![OverrideTrace::new(
            SchemaVersion::V1,
            OverrideLayer::BuiltIn,
            SkillId::new("phase1-design.requirement-analysis").expect("skill id"),
            OverrideDisposition::Selected,
            ReasonCode::new("methodology.builtin-selected").expect("reason"),
            ArtifactDigest::digest(b"methodology entry"),
        )],
        DecisionDigest::digest(b"resolution"),
    )
    .expect("resolution");
    let input = SeriesInput::new(
        SchemaVersion::V1,
        route,
        ProcessSnapshot::new(
            SchemaVersion::V1,
            WorkItemId::new("STORY-001").expect("work item id"),
            ProcessPhase::RequirementAnalyzed,
            None,
            StateRevision::new(7),
            ArtifactDigest::digest(b"state"),
        ),
        Vec::new(),
        vec![resolution],
        vec![plan.clone()],
        AgentRole::Root,
        StateRevision::new(7),
        InputFingerprint::digest(b"series input"),
        IdempotencyKey::new("series-next-STORY-001-r7").expect("idempotency key"),
    )
    .expect("series input");
    let input_json = serde_json::to_string(&input).expect("serialize series input");
    assert_eq!(
        serde_json::from_str::<SeriesInput>(&input_json).expect("deserialize series input"),
        input
    );
    let decision = SeriesPlanDecision::RunSeries {
        schema_version: SchemaVersion::V1,
        idempotency_key: IdempotencyKey::new("series-STORY-001-ra-r7").expect("idempotency key"),
        plan: Box::new(plan),
    };

    let json = serde_json::to_string(&decision).expect("serialize");
    let decoded: SeriesPlanDecision = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, decision);
    assert!(!json.contains("C:\\"));
}

#[test]
fn micro_impact_level_round_trips_the_wire_token() {
    let fact = ImpactFact::new(
        ReasonCode::new("impact.local_rename").expect("fact code"),
        ImpactLevel::Micro,
        None,
    );

    let json = serde_json::to_value(&fact).expect("serialize impact fact");
    assert_eq!(json["level"], "micro");
    let decoded: ImpactFact = serde_json::from_value(json).expect("deserialize impact fact");

    assert_eq!(decoded, fact);
    // The total order is frozen: micro ranks below every ordinary impact level.
    assert!(ImpactLevel::Micro < ImpactLevel::Low);
    assert!(ImpactLevel::Low < ImpactLevel::Medium);
    assert!(ImpactLevel::Medium < ImpactLevel::High);
}

#[test]
fn unknown_impact_level_token_fails_closed() {
    for token in ["tiny", "Micro", "MICRO", "", "low "] {
        assert!(
            serde_json::from_value::<ImpactLevel>(serde_json::json!(token)).is_err(),
            "unknown impact level token must be rejected: {token:?}"
        );
    }
    let fact = serde_json::json!({"code":"impact.local_rename","level":"tiny"});
    assert!(serde_json::from_value::<ImpactFact>(fact).is_err());
}
