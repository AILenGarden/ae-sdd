//! Execution capsule full/delta/no-change projection contract tests.
//!
//! The projection serializes the typed `ExecutionCapsuleV1` once, enforces the
//! frozen capsule byte budget and binds its cache key to the approved-plan,
//! queue and capsule digests, so the existing runtime context cache can serve
//! full/delta/no-change resumes without a second delta algorithm.

use std::collections::BTreeSet;

use ae_sdd_context::{
    ContextFreshness, ContextFreshnessDimension, ContextProjectionError, ContextSelector,
    ExecutionCapsuleKey, ExecutionCapsuleProjection,
};
use ae_sdd_contracts::SchemaVersion;
use ae_sdd_contracts::execution_runtime::{
    ExecutionBudgetsV1, ExecutionCapsuleV1, ExecutionQueueRefV1, ExecutionSliceV1,
    MAX_CAPSULE_BYTES, SourceReadSpecV1,
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, ExecutionSliceId, InventoryGeneration, PolicyDigest,
    ProjectRelativePath, StateRevision, StoryId, VerificationId, WorkItemId,
};

const CONTEXT_REF_FIELDS: [&str; 4] = [
    "storyRef",
    "constraintsRef",
    "thinkingEngineRef",
    "verificationRef",
];

fn artifact(path: &str, contents: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new("execution-fixture").expect("artifact kind"),
        ProjectRelativePath::new(path).expect("project-relative path"),
        ArtifactDigest::digest(contents),
        u64::try_from(contents.len()).expect("fixture length fits u64"),
    )
}

fn slice_id(value: &str) -> ExecutionSliceId {
    ExecutionSliceId::new(value).expect("slice id")
}

fn slice(
    ordinal: u32,
    id_suffix: &str,
    objective: &str,
    depends_on: Vec<ExecutionSliceId>,
) -> ExecutionSliceV1 {
    ExecutionSliceV1::new(
        slice_id(&format!("slice-{id_suffix}")),
        ordinal,
        objective,
        depends_on,
        vec![ProjectRelativePath::new("crates/ae-sdd-context").expect("path scope")],
        vec![
            SourceReadSpecV1::new(
                ProjectRelativePath::new("crates/ae-sdd-context/src/lib.rs").expect("source path"),
                Some(1),
                Some(24),
            )
            .expect("source read spec"),
        ],
        VerificationId::new("V-EFF-002").expect("verification id"),
        vec![VerificationId::new("V-EFF-001b").expect("verification id")],
        format!("execution-capsule/slice-{id_suffix}"),
    )
    .expect("valid slice")
}

fn three_slices() -> Vec<ExecutionSliceV1> {
    vec![
        slice(
            1,
            "contract",
            "freeze execution capsule contracts",
            Vec::new(),
        ),
        slice(
            2,
            "runtime-wiring",
            "wire flow and operations",
            vec![slice_id("slice-contract")],
        ),
        slice(
            3,
            "process-test",
            "project capsule and run e2e",
            vec![slice_id("slice-runtime-wiring")],
        ),
    ]
}

fn capsule_with_budgets(
    plan_marker: &[u8],
    story_marker: &[u8],
    active_ordinal: u32,
    budgets: ExecutionBudgetsV1,
) -> ExecutionCapsuleV1 {
    let slices = three_slices();
    let approved_plan_digest = ArtifactDigest::digest([plan_marker, b"/plan"].concat());
    let queue_contents = [plan_marker, b"/queue"].concat();
    let queue_digest = ArtifactDigest::digest(&queue_contents);
    let queue_ref = ExecutionQueueRefV1::new(
        artifact(
            ".auto-engineering/PRD-AE-SDD-EXECUTION-EFFICIENCY-001/execution/queue.json",
            &queue_contents,
        ),
        queue_digest,
        3,
        active_ordinal - 1,
        active_ordinal,
    )
    .expect("queue ref");
    let active_slice = slices
        .iter()
        .find(|slice| slice.ordinal() == active_ordinal)
        .expect("active slice exists")
        .clone();
    ExecutionCapsuleV1::new(
        SchemaVersion::V1,
        WorkItemId::new("PRD-AE-SDD-EXECUTION-EFFICIENCY-001").expect("work item id"),
        StoryId::new("STORY-AE-SDD-EXECUTION-CAPSULE-001").expect("story id"),
        StateRevision::new(12),
        approved_plan_digest,
        PolicyDigest::digest(b"projection policy"),
        InventoryGeneration::new(3),
        artifact(
            "ae-sdd-doc/Story/STORY-AE-SDD-EXECUTION-CAPSULE-001.md",
            story_marker,
        ),
        artifact("constraints/README.md", b"constraints body"),
        artifact(
            "source/standards/thinking/be-coding-thinking-engine.md",
            b"thinking body",
        ),
        artifact(
            "ae-sdd-doc/Story/execution-capsule.verification.json",
            b"verification body",
        ),
        queue_ref,
        active_slice,
        budgets,
    )
    .expect("valid capsule")
}

fn capsule(plan_marker: &[u8], story_marker: &[u8], active_ordinal: u32) -> ExecutionCapsuleV1 {
    capsule_with_budgets(
        plan_marker,
        story_marker,
        active_ordinal,
        ExecutionBudgetsV1::default(),
    )
}

#[test]
fn full_projection_stays_within_the_capsule_budget_and_is_deterministic() {
    let fixture = capsule(b"plan-v1", b"story-v1", 1);
    let projection = ExecutionCapsuleProjection::new(&fixture).expect("projection");

    assert!(projection.byte_length() <= MAX_CAPSULE_BYTES);
    let encoded = serde_json::to_vec(&fixture).expect("canonical capsule encoding");
    assert_eq!(projection.byte_length() as usize, encoded.len());
    assert_eq!(projection.digest(), ArtifactDigest::digest(&encoded));

    let rebuilt = ExecutionCapsuleProjection::new(&capsule(b"plan-v1", b"story-v1", 1))
        .expect("rebuilt projection");
    assert_eq!(projection.digest(), rebuilt.digest());
    assert_eq!(projection.byte_length(), rebuilt.byte_length());
    assert_eq!(projection.key(), rebuilt.key());

    let value = projection.value();
    assert_eq!(value["activeSlice"]["ordinal"], 1);
    assert_eq!(value["queue"]["activeOrdinal"], 1);
    for field in CONTEXT_REF_FIELDS {
        assert!(
            value.get(field).is_some(),
            "capsule projection keeps the content-addressed {field} reference"
        );
    }
}

#[test]
fn over_budget_capsule_fails_closed() {
    let budgets = ExecutionBudgetsV1::new(512, 64 * 1024, 24 * 1024, 12, 4, 3, 1)
        .expect("tight capsule budget");
    let oversized = capsule_with_budgets(b"plan-v1", b"story-v1", 1, budgets);

    assert!(matches!(
        ExecutionCapsuleProjection::new(&oversized),
        Err(ContextProjectionError::BudgetExceeded { maximum: 512, .. })
    ));
}

#[test]
fn execution_key_binds_plan_queue_and_capsule_digests() {
    let base = ExecutionCapsuleProjection::new(&capsule(b"plan-v1", b"story-v1", 1))
        .expect("base projection");
    let same = ExecutionCapsuleProjection::new(&capsule(b"plan-v1", b"story-v1", 1))
        .expect("identical projection");
    assert_eq!(
        base.key().freshness_against(same.key()),
        ContextFreshness::Fresh
    );
    assert!(base.key().invalidated_against(same.key()).is_empty());

    let advanced = ExecutionCapsuleProjection::new(&capsule(b"plan-v1", b"story-v1", 2))
        .expect("advanced projection");
    assert_eq!(
        base.key().approved_plan_digest(),
        advanced.key().approved_plan_digest()
    );
    assert_eq!(base.key().queue_digest(), advanced.key().queue_digest());
    assert_ne!(base.key().capsule_digest(), advanced.key().capsule_digest());
    assert_eq!(
        base.key().freshness_against(advanced.key()),
        ContextFreshness::Stale(vec![ContextFreshnessDimension::ExecutionCapsule])
    );
    assert_eq!(
        base.key().invalidated_against(advanced.key()),
        BTreeSet::from([ContextSelector::ActiveSlice])
    );

    let replanned = ExecutionCapsuleProjection::new(&capsule(b"plan-v2", b"story-v1", 1))
        .expect("replanned projection");
    assert_eq!(
        base.key().freshness_against(replanned.key()),
        ContextFreshness::Stale(vec![
            ContextFreshnessDimension::ExecutionPlan,
            ContextFreshnessDimension::ExecutionQueue,
            ContextFreshnessDimension::ExecutionCapsule,
        ])
    );
    assert_eq!(
        base.key().invalidated_against(replanned.key()),
        BTreeSet::from([
            ContextSelector::ExecutionCapsule,
            ContextSelector::ExecutionQueue,
            ContextSelector::ActiveSlice,
        ])
    );

    let story_drift = ExecutionCapsuleProjection::new(&capsule(b"plan-v1", b"story-v2", 1))
        .expect("story drift projection");
    assert_eq!(
        base.key().freshness_against(story_drift.key()),
        ContextFreshness::Stale(vec![ContextFreshnessDimension::ExecutionCapsule])
    );
    assert_eq!(
        base.key().invalidated_against(story_drift.key()),
        BTreeSet::from([ContextSelector::ActiveSlice])
    );

    let swapped_queue = ExecutionCapsuleKey::new(
        base.key().approved_plan_digest(),
        ArtifactDigest::digest(b"swapped queue"),
        ArtifactDigest::digest(b"swapped capsule"),
    );
    assert_eq!(
        base.key().freshness_against(&swapped_queue),
        ContextFreshness::Stale(vec![
            ContextFreshnessDimension::ExecutionQueue,
            ContextFreshnessDimension::ExecutionCapsule,
        ])
    );
    assert_eq!(
        base.key().invalidated_against(&swapped_queue),
        BTreeSet::from([
            ContextSelector::ExecutionQueue,
            ContextSelector::ActiveSlice,
        ])
    );
}

#[test]
fn ordinal_advance_keeps_context_refs_identical_so_the_delta_drops_them() {
    let first = ExecutionCapsuleProjection::new(&capsule(b"plan-v1", b"story body", 1))
        .expect("first projection");
    let second = ExecutionCapsuleProjection::new(&capsule(b"plan-v1", b"story body", 2))
        .expect("second projection");

    for field in CONTEXT_REF_FIELDS {
        assert_eq!(
            first.value().get(field),
            second.value().get(field),
            "{field} must stay identical across an ordinal move so the existing delta omits it"
        );
    }
    assert_ne!(
        first.value().get("activeSlice"),
        second.value().get("activeSlice")
    );
    assert_ne!(first.value().get("queue"), second.value().get("queue"));

    let serialized = serde_json::to_string(first.value()).expect("projection text");
    assert!(!serialized.contains("story body"));
    assert!(!serialized.contains("constraints body"));
}
