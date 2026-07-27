use ae_sdd_context::{
    ContextBundleInput, ContextFreshness, ContextFreshnessDimension, ContextPort, ContextSelector,
    ContextService, ContextServiceError, MAX_COMPACT_STATE_DELTA_BYTES,
    MAX_COMPACT_STATE_DELTA_ENTRIES,
};
use ae_sdd_contracts::{
    ContextBundleId, MethodologyRef, MethodologyVariant, SchemaVersion, SeriesKind, SkillId,
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, InventoryGeneration, PolicyDigest,
    ProjectRelativePath, StateRevision, WorkItemId,
};

fn artifact(kind: &str, path: &str, content: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new(kind).expect("valid artifact kind"),
        ProjectRelativePath::new(path).expect("valid project-relative path"),
        ArtifactDigest::digest(content),
        u64::try_from(content.len()).expect("fixture length"),
    )
}

fn methodology(content: &[u8]) -> MethodologyRef {
    let compact = artifact("methodology", "source/skills/coding.compact.md", content);
    MethodologyRef::new(
        SchemaVersion::V1,
        SkillId::new("coding").expect("valid skill"),
        SeriesKind::new("coding").expect("valid series"),
        MethodologyVariant::new("compact").expect("valid variant"),
        compact,
        None,
        ArtifactDigest::digest(content),
        ArtifactDigest::digest(b"catalog"),
    )
    .expect("valid methodology")
}

fn methodology_with_fallback(content: &[u8], fallback_content: &[u8]) -> MethodologyRef {
    let compact = artifact("methodology", "source/skills/coding.compact.md", content);
    let fallback = artifact(
        "methodology",
        "source/skills/coding.fallback.md",
        fallback_content,
    );
    MethodologyRef::new(
        SchemaVersion::V1,
        SkillId::new("coding").expect("valid skill"),
        SeriesKind::new("coding").expect("valid series"),
        MethodologyVariant::new("compact").expect("valid variant"),
        compact,
        Some(fallback),
        ArtifactDigest::digest(content),
        ArtifactDigest::digest(b"changed-catalog"),
    )
    .expect("valid methodology")
}

fn try_input(
    optional_refs: Vec<ArtifactRef>,
    revision: u64,
    computed_at_unix_ms: u64,
) -> Result<ContextBundleInput, ContextServiceError> {
    ContextBundleInput::new(
        SchemaVersion::V1,
        ContextBundleId::new("context-plan-c").expect("valid context id"),
        WorkItemId::new("PRD-AE-SDD-RUST-DAEMON-001").expect("valid work item"),
        artifact(
            "story",
            "ae-sdd-doc/Story/STORY-AE-SDD-RESOURCE-CONTEXT-002.md",
            b"story",
        ),
        artifact("constraints", "constraints/README.md", b"constraints"),
        artifact(
            "thinking",
            "source/standards/thinking/be-coding-thinking-engine.md",
            b"thinking",
        ),
        artifact(
            "verification",
            "ae-sdd-doc/Story/resource-context.verification.json",
            b"verification",
        ),
        methodology(b"methodology"),
        optional_refs,
        StateRevision::new(revision),
        InventoryGeneration::new(9),
        PolicyDigest::digest(b"projection-policy"),
        computed_at_unix_ms,
    )
}

fn input(optional_refs: Vec<ArtifactRef>, revision: u64) -> ContextBundleInput {
    try_input(optional_refs, revision, 1_000).expect("valid context input")
}

#[test]
fn bundle_is_order_independent_and_binds_all_freshness_inputs() {
    let service = ContextService;
    let first = service
        .bundle(input(
            vec![
                artifact("memory", ".ae-sdd/memory/z.json", b"z"),
                artifact("memory", ".ae-sdd/memory/a.json", b"a"),
            ],
            7,
        ))
        .expect("bundle succeeds");
    let reordered = service
        .bundle(input(
            vec![
                artifact("memory", ".ae-sdd/memory/a.json", b"a"),
                artifact("memory", ".ae-sdd/memory/z.json", b"z"),
            ],
            7,
        ))
        .expect("reordered bundle succeeds");

    assert_eq!(
        first.bundle_ref().bundle_digest(),
        reordered.bundle_ref().bundle_digest()
    );
    assert_eq!(first.cache_key().digest(), reordered.cache_key().digest());
    assert_eq!(
        first
            .bundle_ref()
            .artifact_refs()
            .iter()
            .map(|reference| reference.path().as_str())
            .collect::<Vec<_>>(),
        vec![
            ".ae-sdd/memory/a.json",
            ".ae-sdd/memory/z.json",
            "ae-sdd-doc/Story/STORY-AE-SDD-RESOURCE-CONTEXT-002.md",
            "ae-sdd-doc/Story/resource-context.verification.json",
            "constraints/README.md",
            "source/skills/coding.compact.md",
            "source/standards/thinking/be-coding-thinking-engine.md",
        ]
    );
    assert_eq!(first.proof().story_ref(), first.cache_key().story_ref());
    assert_eq!(first.proof().state_revision(), StateRevision::new(7));
    assert_eq!(
        first.cache_key().constraints_ref().path().as_str(),
        "constraints/README.md"
    );
    assert_eq!(
        first.cache_key().thinking_engine_ref().path().as_str(),
        "source/standards/thinking/be-coding-thinking-engine.md"
    );
    assert_eq!(
        first.cache_key().verification_ref().path().as_str(),
        "ae-sdd-doc/Story/resource-context.verification.json"
    );
    assert_eq!(first.cache_key().state_revision(), StateRevision::new(7));
    assert_eq!(
        first.cache_key().inventory_generation(),
        InventoryGeneration::new(9)
    );
    assert_eq!(
        first.cache_key().projection_policy_digest(),
        PolicyDigest::digest(b"projection-policy")
    );
    assert_ne!(
        first.cache_key().methodology_digest(),
        ArtifactDigest::digest([])
    );
    assert_eq!(
        first.cache_key().freshness_against(reordered.cache_key()),
        ContextFreshness::Fresh
    );
}

#[test]
fn refresh_returns_only_changed_removed_and_invalidated_entries() {
    let service = ContextService;
    let prior = service
        .bundle(input(
            vec![
                artifact("memory", ".ae-sdd/memory/a.json", b"old-a"),
                artifact("memory", ".ae-sdd/memory/b.json", b"removed"),
                artifact("memory", ".ae-sdd/memory/stable.json", b"stable"),
            ],
            7,
        ))
        .expect("prior bundle");
    let refreshed = service
        .refresh(
            &prior,
            input(
                vec![
                    artifact("memory", ".ae-sdd/memory/a.json", b"new-a"),
                    artifact("memory", ".ae-sdd/memory/new.json", b"new"),
                    artifact("memory", ".ae-sdd/memory/stable.json", b"stable"),
                ],
                8,
            ),
        )
        .expect("refresh succeeds");

    assert_eq!(
        refreshed
            .delta()
            .changed()
            .iter()
            .map(|reference| reference.path().as_str())
            .collect::<Vec<_>>(),
        vec![".ae-sdd/memory/a.json", ".ae-sdd/memory/new.json"]
    );
    assert_eq!(
        refreshed
            .delta()
            .removed()
            .iter()
            .map(ProjectRelativePath::as_str)
            .collect::<Vec<_>>(),
        vec![".ae-sdd/memory/b.json"]
    );
    assert!(
        !refreshed
            .delta()
            .changed()
            .iter()
            .any(|reference| reference.path().as_str().ends_with("stable.json"))
    );
    assert!(
        refreshed
            .delta()
            .invalidated()
            .contains(&ContextSelector::StateRevision)
    );
    assert!(
        refreshed
            .delta()
            .invalidated()
            .contains(&ContextSelector::Optional(
                ProjectRelativePath::new(".ae-sdd/memory/a.json").expect("valid path")
            ))
    );
    assert!(
        refreshed
            .delta()
            .invalidated()
            .contains(&ContextSelector::Optional(
                ProjectRelativePath::new(".ae-sdd/memory/b.json").expect("valid path")
            ))
    );
    assert_eq!(
        prior
            .cache_key()
            .freshness_against(refreshed.context().cache_key()),
        ContextFreshness::Stale(vec![ContextFreshnessDimension::StateRevision])
    );
    assert_eq!(
        refreshed.delta().prior_digest(),
        prior.bundle_ref().bundle_digest()
    );
    assert_eq!(
        refreshed.delta().next_digest(),
        refreshed.context().bundle_ref().bundle_digest()
    );
    assert!(refreshed.delta().byte_length() > 0);
    assert!(!refreshed.delta().is_empty());
}

#[test]
fn conflicting_duplicate_path_fails_closed() {
    let result = ContextService.bundle(input(
        vec![artifact("memory", "constraints/README.md", b"tampered")],
        7,
    ));

    assert!(result.is_err());
}

#[test]
fn cache_freshness_reports_every_authoritative_dimension_in_stable_order() {
    let service = ContextService;
    let baseline = service
        .bundle(input(vec![], 7))
        .expect("baseline bundle succeeds");
    let changed = service
        .bundle(
            ContextBundleInput::new(
                SchemaVersion::V1,
                ContextBundleId::new("context-plan-c").expect("valid context id"),
                WorkItemId::new("PRD-AE-SDD-RUST-DAEMON-002").expect("valid work item"),
                artifact(
                    "story",
                    "ae-sdd-doc/Story/STORY-AE-SDD-RESOURCE-CONTEXT-002.md",
                    b"changed story",
                ),
                artifact(
                    "constraints",
                    "constraints/README.md",
                    b"changed constraints",
                ),
                artifact(
                    "thinking",
                    "source/standards/thinking/be-coding-thinking-engine.md",
                    b"changed thinking",
                ),
                artifact(
                    "verification",
                    "ae-sdd-doc/Story/resource-context.verification.json",
                    b"changed verification",
                ),
                methodology_with_fallback(b"changed methodology", b"fallback"),
                vec![],
                StateRevision::new(8),
                InventoryGeneration::new(10),
                PolicyDigest::digest(b"changed projection policy"),
                2_000,
            )
            .expect("valid changed input"),
        )
        .expect("changed bundle succeeds");

    assert_eq!(
        baseline.cache_key().freshness_against(changed.cache_key()),
        ContextFreshness::Stale(vec![
            ContextFreshnessDimension::WorkItem,
            ContextFreshnessDimension::Story,
            ContextFreshnessDimension::Constraints,
            ContextFreshnessDimension::ThinkingEngine,
            ContextFreshnessDimension::Verification,
            ContextFreshnessDimension::Methodology,
            ContextFreshnessDimension::StateRevision,
            ContextFreshnessDimension::InventoryGeneration,
            ContextFreshnessDimension::ProjectionPolicy,
        ])
    );
    assert_ne!(
        baseline.cache_key().methodology_digest(),
        changed.cache_key().methodology_digest()
    );
}

#[test]
fn refresh_enforces_entry_and_byte_budgets_and_empty_delta_identity() {
    assert_eq!(
        try_input(vec![], 7, 0),
        Err(ContextServiceError::InvalidComputedTime)
    );

    let service = ContextService;
    let prior = service
        .bundle(input(vec![], 7))
        .expect("prior bundle succeeds");
    let unchanged = service
        .refresh(&prior, input(vec![], 7))
        .expect("unchanged refresh succeeds");
    assert!(unchanged.delta().is_empty());
    assert_eq!(unchanged.delta().byte_length(), 0);
    assert_eq!(
        unchanged.delta().prior_digest(),
        unchanged.delta().next_digest()
    );

    let prior_entries = (0..32)
        .map(|index| {
            artifact(
                "memory",
                &format!(".ae-sdd/memory/old-{index:03}.json"),
                b"old",
            )
        })
        .collect::<Vec<_>>();
    let next_entries = (0..32)
        .map(|index| {
            artifact(
                "memory",
                &format!(".ae-sdd/memory/new-{index:03}.json"),
                b"new",
            )
        })
        .collect::<Vec<_>>();
    let prior_with_entries = service
        .bundle(input(prior_entries, 7))
        .expect("each bundle remains within its own resource limit");
    assert!(matches!(
        service.refresh(&prior_with_entries, input(next_entries, 7)),
        Err(ContextServiceError::DeltaEntryLimitExceeded {
            actual,
            maximum: MAX_COMPACT_STATE_DELTA_ENTRIES,
        }) if actual > MAX_COMPACT_STATE_DELTA_ENTRIES
    ));

    let oversized = ArtifactRef::new(
        ArtifactKind::new("memory").expect("valid artifact kind"),
        ProjectRelativePath::new(".ae-sdd/memory/oversized.json").expect("valid path"),
        ArtifactDigest::digest(b"oversized"),
        MAX_COMPACT_STATE_DELTA_BYTES - prior.bundle_ref().byte_length(),
    );
    assert!(matches!(
        service.refresh(&prior, input(vec![oversized], 7)),
        Err(ContextServiceError::DeltaBudgetExceeded {
            actual,
            maximum: MAX_COMPACT_STATE_DELTA_BYTES,
        }) if actual > MAX_COMPACT_STATE_DELTA_BYTES
    ));
}
