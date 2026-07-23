use ae_sdd_contracts::{
    BoundedText, ConfirmationRequirement, ControlPlaneErrorCode, EventIntent, ImpactFact,
    ImpactLevel, LifecycleCommand, LifecycleDisposition, LifecycleInput, LifecyclePlan,
    LogicalNamespace, MethodologyQuery, MethodologyRef, MethodologyResolution, MethodologyVariant,
    MutationIntent, MutationIntentId, MutationOperation, MutationTarget, OverrideDisposition,
    OverrideLayer, OverrideTrace, ProcessSnapshot, ProjectScope, ReasonCode, RouteInput,
    SchemaVersion, SeriesKind, SkillId,
};
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, ArtifactKind, ArtifactRef, DecisionDigest, DesignRoute,
    InputFingerprint, ProcessPhase, ProjectKey, ProjectRelativePath, StateRevision, WorkItemId,
    WorkScale,
};

fn artifact(path: &str, contents: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new("control-plane-fixture").expect("artifact kind"),
        ProjectRelativePath::new(path).expect("project-relative path"),
        ArtifactDigest::digest(contents),
        u64::try_from(contents.len()).expect("fixture length fits u64"),
    )
}

fn methodology() -> MethodologyRef {
    MethodologyRef::new(
        SchemaVersion::V1,
        SkillId::new("phase1-design.requirement-analysis").expect("skill id"),
        SeriesKind::new("requirement-analysis").expect("series kind"),
        MethodologyVariant::new("builtin-v1").expect("variant"),
        artifact("runtime/skills/requirement-analysis/compact.md", b"compact"),
        None,
        ArtifactDigest::digest(b"entry"),
        ArtifactDigest::digest(b"catalog"),
    )
    .expect("methodology ref")
}

#[test]
fn methodology_resolution_has_one_auditable_winner_and_strict_wire_shape() {
    let query = MethodologyQuery::new(
        SchemaVersion::V1,
        SeriesKind::new("requirement-analysis").expect("series kind"),
        ProjectScope::new(
            SchemaVersion::V1,
            ProjectKey::new("ae-sdd").expect("project key"),
            Some(ProjectRelativePath::new("crates").expect("relative scope")),
        ),
        None,
        ArtifactDigest::digest(b"catalog"),
        None,
    );
    let query_json = serde_json::to_string(&query).expect("serialize query");
    assert_eq!(
        serde_json::from_str::<MethodologyQuery>(&query_json).expect("deserialize query"),
        query
    );

    let trace = OverrideTrace::new(
        SchemaVersion::V1,
        OverrideLayer::BuiltIn,
        SkillId::new("phase1-design.requirement-analysis").expect("skill id"),
        OverrideDisposition::Selected,
        ReasonCode::new("methodology.builtin-selected").expect("reason code"),
        ArtifactDigest::digest(b"candidate"),
    );
    let resolution = MethodologyResolution::new(
        SchemaVersion::V1,
        methodology(),
        OverrideLayer::BuiltIn,
        vec![trace],
        DecisionDigest::digest(b"resolution"),
    )
    .expect("valid resolution");

    let json = serde_json::to_string(&resolution).expect("serialize");
    let decoded: MethodologyResolution = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, resolution);
    assert!(
        serde_json::from_str::<MethodologyResolution>(&json.replacen('{', "{\"extra\":1,", 1))
            .is_err()
    );
}

#[test]
fn route_input_is_bounded_and_uses_typed_facts_only() {
    let input = RouteInput::new(
        SchemaVersion::V1,
        WorkItemId::new("STORY-001").expect("work item id"),
        ReasonCode::new("route.entry.standard").expect("entry node"),
        BoundedText::<4096>::new("Move workflow control into the daemon").expect("intent"),
        vec![artifact("ae-sdd-doc/RA/RA-001.md", b"ra")],
        vec![ImpactFact::new(
            ReasonCode::new("impact.cross-agent-concurrency").expect("fact code"),
            ImpactLevel::High,
            Some(ArtifactDigest::digest(b"fact evidence")),
        )],
        4_999,
        InputFingerprint::digest(b"route input"),
        None,
    )
    .expect("valid route input");

    let json = serde_json::to_string(&input).expect("serialize");
    let decoded: RouteInput = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, input);
    assert!(
        RouteInput::new(
            SchemaVersion::V1,
            WorkItemId::new("STORY-001").expect("work item id"),
            ReasonCode::new("route.entry.standard").expect("entry node"),
            BoundedText::<4096>::new("intent").expect("intent"),
            Vec::new(),
            Vec::new(),
            10_001,
            InputFingerprint::digest(b"route input"),
            None,
        )
        .is_err()
    );
}

#[test]
fn lifecycle_plan_contains_only_ordered_typed_mutation_intents() {
    let lifecycle_input = LifecycleInput::new(
        SchemaVersion::V1,
        LifecycleCommand::Transition {
            target_phase: ProcessPhase::Coding,
        },
        ProcessSnapshot::new(
            SchemaVersion::V1,
            WorkItemId::new("STORY-001").expect("work item id"),
            ProcessPhase::CodingProcess,
            None,
            StateRevision::new(7),
            ArtifactDigest::digest(b"state"),
        ),
        StateRevision::new(7),
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Dr,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        1_785_000_000_000,
        InputFingerprint::digest(b"lifecycle input"),
    )
    .expect("valid lifecycle input");
    let input_json = serde_json::to_string(&lifecycle_input).expect("serialize lifecycle input");
    assert_eq!(
        serde_json::from_str::<LifecycleInput>(&input_json).expect("deserialize lifecycle input"),
        lifecycle_input
    );

    let target = MutationTarget::project_file(
        LogicalNamespace::new("work-item-state").expect("namespace"),
        ProjectRelativePath::new(".auto-engineering/state.json").expect("relative path"),
    );
    let intent = MutationIntent::new(
        SchemaVersion::V1,
        MutationIntentId::new("intent-0001").expect("intent id"),
        target,
        MutationOperation::Replace,
        StateRevision::new(7),
        Some(ArtifactDigest::digest(b"before")),
        EventIntent::new(
            ReasonCode::new("lifecycle.phase-transitioned").expect("event kind"),
            ArtifactDigest::digest(b"event payload"),
        ),
    );
    let binding = DecisionDigest::digest(b"lifecycle input");
    let plan = LifecyclePlan::new(
        SchemaVersion::V1,
        LifecycleDisposition::Permitted,
        vec![intent],
        StateRevision::new(7),
        ConfirmationRequirement::not_required(binding),
        DecisionDigest::digest(b"lifecycle plan"),
        Vec::new(),
    )
    .expect("valid lifecycle plan");

    let json = serde_json::to_string(&plan).expect("serialize");
    let decoded: LifecyclePlan = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, plan);
    assert_eq!(
        serde_json::to_string(&ControlPlaneErrorCode::RouteApprovalRequired)
            .expect("serialize code"),
        "\"ROUTE_APPROVAL_REQUIRED\""
    );
}
