use ae_sdd_contracts::{
    MethodologyRef, MethodologyVariant, ReasonCode, RouteDecision, RouteDecisionId,
    RouteDisposition, SchemaVersion, SeriesKind, SkillId,
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, DecisionDigest, DesignRoute, InputFingerprint,
    ProjectRelativePath, WorkItemId, WorkScale,
};

fn artifact(path: &str, contents: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new("methodology-slice").expect("artifact kind"),
        ProjectRelativePath::new(path).expect("project-relative path"),
        ArtifactDigest::digest(contents),
        u64::try_from(contents.len()).expect("fixture length fits u64"),
    )
}

#[test]
fn methodology_reference_round_trips_and_rejects_unknown_fields() {
    let reference = MethodologyRef::new(
        SchemaVersion::V1,
        SkillId::new("phase1-design.requirement-analysis").expect("valid skill id"),
        SeriesKind::new("requirement-analysis").expect("series kind"),
        MethodologyVariant::new("builtin-v1").expect("variant"),
        artifact("runtime/skills/requirement-analysis/compact.md", b"compact"),
        Some(artifact(
            "runtime/skills/requirement-analysis/fallback.md",
            b"fallback",
        )),
        ArtifactDigest::digest(b"entry"),
        ArtifactDigest::digest(b"catalog"),
    )
    .expect("valid methodology reference");

    let json = serde_json::to_string(&reference).expect("serialize");
    let decoded: MethodologyRef = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, reference);

    let with_unknown = json.replacen('{', "{\"unexpected\":true,", 1);
    assert!(serde_json::from_str::<MethodologyRef>(&with_unknown).is_err());
    assert!(SkillId::new("../escape").is_err());
}

#[test]
fn route_decision_round_trips_using_domain_owned_scale_and_digests() {
    let decision = RouteDecision::new(
        SchemaVersion::V1,
        RouteDecisionId::new("route-STORY-001-r7").expect("route decision id"),
        WorkItemId::new("STORY-001").expect("work item id"),
        WorkScale::Large,
        DesignRoute::Dr,
        RouteDisposition::AwaitUserApproval,
        vec![ReasonCode::new("route.low-confidence").expect("reason code")],
        vec![
            SeriesKind::new("requirement-analysis").expect("series kind"),
            SeriesKind::new("design-review").expect("series kind"),
        ],
        InputFingerprint::digest(b"typed route facts"),
        None,
        DecisionDigest::digest(b"route decision"),
    )
    .expect("route decision");

    assert_eq!(decision.disposition(), RouteDisposition::AwaitUserApproval);
    assert!(!decision.is_approved());

    let json = serde_json::to_string(&decision).expect("serialize");
    let decoded: RouteDecision = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, decision);

    let micro = json.replace("\"large\"", "\"micro\"");
    let decoded_micro: RouteDecision = serde_json::from_str(&micro).expect("micro is supported");
    assert_eq!(decoded_micro.scale(), WorkScale::Micro);
}
