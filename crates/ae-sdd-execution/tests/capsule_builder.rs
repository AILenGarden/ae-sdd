//! Deterministic queue/capsule builder contract tests.
//!
//! The builder is a pure function of typed input: identical semantic input
//! must produce byte-identical queue and capsule encodings regardless of the
//! order slices (and their inner collections) were supplied in, and invalid
//! DAGs or cursors must fail closed.

use ae_sdd_contracts::SchemaVersion;
use ae_sdd_contracts::execution_runtime::{
    ExecutionBudgetsV1, ExecutionCapsuleError, ExecutionSliceV1, MAX_CAPSULE_BYTES,
    SourceReadSpecV1,
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, ExecutionSliceId, InventoryGeneration, PolicyDigest,
    ProjectRelativePath, StateRevision, StoryId, VerificationId, WorkItemId,
};
use ae_sdd_execution::{
    CapsuleBuildInputV1, ExecutionCapsuleBuildError, ExecutionQueueV1, ExecutionSliceSpecV1,
    build_execution_capsule,
};
use proptest::prelude::*;

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

fn verification_id(value: &str) -> VerificationId {
    VerificationId::new(value).expect("verification id")
}

fn scope(value: &str) -> ProjectRelativePath {
    ProjectRelativePath::new(value).expect("path scope")
}

fn spec(
    ordinal: u32,
    id_suffix: &str,
    objective: &str,
    depends_on: Vec<ExecutionSliceId>,
) -> ExecutionSliceSpecV1 {
    ExecutionSliceSpecV1 {
        slice_id: slice_id(&format!("slice-{id_suffix}")),
        ordinal,
        objective: objective.into(),
        depends_on,
        path_scope: vec![scope("crates/ae-sdd-execution")],
        source_reads: vec![
            SourceReadSpecV1::new(
                scope("crates/ae-sdd-execution/src/lib.rs"),
                Some(1),
                Some(40),
            )
            .expect("source read spec"),
        ],
        focused_verification_id: verification_id("V-EFF-001c"),
        broad_verification_ids: vec![verification_id("V-EFF-001b")],
        evidence_logical_key: format!("execution-capsule/slice-{id_suffix}").into(),
    }
}

fn three_slices() -> Vec<ExecutionSliceSpecV1> {
    vec![
        spec(
            1,
            "contract",
            "freeze execution capsule contracts",
            Vec::new(),
        ),
        spec(
            2,
            "runtime-wiring",
            "wire flow and operations",
            vec![slice_id("slice-contract")],
        ),
        spec(
            3,
            "process-test",
            "project capsule and run e2e",
            vec![slice_id("slice-runtime-wiring")],
        ),
    ]
}

fn input(slices: Vec<ExecutionSliceSpecV1>, active_ordinal: u32) -> CapsuleBuildInputV1 {
    CapsuleBuildInputV1 {
        work_item_id: WorkItemId::new("PRD-AE-SDD-EXECUTION-EFFICIENCY-001").expect("work item id"),
        story_id: StoryId::new("STORY-AE-SDD-EXECUTION-CAPSULE-001").expect("story id"),
        source_revision: StateRevision::new(12),
        approved_plan_digest: ArtifactDigest::digest(b"approved plan"),
        policy_digest: PolicyDigest::digest(b"policy"),
        inventory_generation: InventoryGeneration::new(3),
        story_ref: artifact(
            "ae-sdd-doc/Story/STORY-AE-SDD-EXECUTION-CAPSULE-001.md",
            b"story",
        ),
        constraints_ref: artifact("constraints/README.md", b"constraints"),
        thinking_engine_ref: artifact(
            "source/standards/thinking/be-coding-thinking-engine.md",
            b"thinking",
        ),
        verification_ref: artifact("ae-sdd-doc/Test/TEST-STORY-001.md", b"verification"),
        queue_artifact_kind: ArtifactKind::new("execution-queue").expect("queue artifact kind"),
        queue_artifact_path: scope(
            ".auto-engineering/PRD-AE-SDD-EXECUTION-EFFICIENCY-001/execution/queue.json",
        ),
        slices,
        active_ordinal,
        budgets: ExecutionBudgetsV1::default(),
    }
}

#[test]
fn slice_order_permutation_produces_identical_queue_and_capsule_digests() {
    let baseline = build_execution_capsule(&input(three_slices(), 1)).expect("baseline build");

    let reversed: Vec<_> = three_slices().into_iter().rev().collect();
    let mut rotated = three_slices();
    rotated.rotate_left(1);

    for permuted in [reversed, rotated] {
        let outcome = build_execution_capsule(&input(permuted, 1)).expect("permuted build");
        assert_eq!(outcome.queue_bytes(), baseline.queue_bytes());
        assert_eq!(outcome.queue_digest(), baseline.queue_digest());
        assert_eq!(outcome.capsule_bytes(), baseline.capsule_bytes());
        assert_eq!(outcome.capsule_digest(), baseline.capsule_digest());
    }
}

#[test]
fn inner_collection_order_permutation_produces_identical_digests() {
    let mut ordered = spec(1, "contract", "freeze contracts", Vec::new());
    ordered.path_scope = vec![scope("crates/alpha"), scope("crates/beta")];
    ordered.source_reads = vec![
        SourceReadSpecV1::new(scope("crates/alpha/src/lib.rs"), None, None)
            .expect("source read spec"),
    ];
    ordered.broad_verification_ids = vec![verification_id("V-1"), verification_id("V-2")];

    let mut shuffled = spec(1, "contract", "freeze contracts", Vec::new());
    shuffled.path_scope = vec![scope("crates/beta"), scope("crates/alpha")];
    shuffled.source_reads = vec![
        SourceReadSpecV1::new(scope("crates/alpha/src/lib.rs"), None, None)
            .expect("source read spec"),
    ];
    shuffled.broad_verification_ids = vec![verification_id("V-2"), verification_id("V-1")];

    let first = build_execution_capsule(&input(vec![ordered], 1)).expect("first build");
    let second = build_execution_capsule(&input(vec![shuffled], 1)).expect("second build");
    assert_eq!(first.queue_bytes(), second.queue_bytes());
    assert_eq!(first.queue_digest(), second.queue_digest());
    assert_eq!(first.capsule_bytes(), second.capsule_bytes());
    assert_eq!(first.capsule_digest(), second.capsule_digest());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn any_slice_order_produces_the_same_digests(permutation in Just(vec![0_usize, 1, 2]).prop_shuffle()) {
        let base = three_slices();
        let shuffled: Vec<_> = permutation.iter().map(|&index| base[index].clone()).collect();
        let baseline = build_execution_capsule(&input(three_slices(), 1)).expect("baseline build");
        let outcome = build_execution_capsule(&input(shuffled, 1)).expect("shuffled build");

        prop_assert_eq!(outcome.queue_bytes(), baseline.queue_bytes());
        prop_assert_eq!(outcome.queue_digest(), baseline.queue_digest());
        prop_assert_eq!(outcome.capsule_bytes(), baseline.capsule_bytes());
        prop_assert_eq!(outcome.capsule_digest(), baseline.capsule_digest());
    }
}

#[test]
fn queue_contains_all_slices_while_capsule_contains_only_the_active_slice() {
    let outcome = build_execution_capsule(&input(three_slices(), 2)).expect("build");

    assert_eq!(outcome.queue().slices().len(), 3);
    assert_eq!(outcome.queue().total_slices(), 3);
    assert_eq!(outcome.queue().schema_version(), SchemaVersion::V1);
    assert_eq!(outcome.capsule().active_slice().ordinal(), 2);
    assert_eq!(outcome.capsule().queue().total_slices(), 3);
    assert_eq!(outcome.capsule().queue().completed_slices(), 1);
    assert_eq!(outcome.capsule().queue().active_ordinal(), 2);
    assert_eq!(
        outcome.capsule().queue().queue_digest(),
        outcome.queue_digest()
    );
    assert_eq!(
        outcome.capsule().queue().artifact().digest(),
        outcome.queue_digest()
    );
    assert_eq!(
        outcome.capsule().queue().artifact().byte_length(),
        u64::try_from(outcome.queue_bytes().len()).expect("queue length fits u64")
    );

    let capsule_text = String::from_utf8(outcome.capsule_bytes().to_vec()).expect("utf8 capsule");
    assert!(capsule_text.contains("wire flow and operations"));
    assert!(!capsule_text.contains("freeze execution capsule contracts"));
    assert!(!capsule_text.contains("project capsule and run e2e"));

    let queue_text = String::from_utf8(outcome.queue_bytes().to_vec()).expect("utf8 queue");
    assert!(queue_text.contains("freeze execution capsule contracts"));
    assert!(queue_text.contains("wire flow and operations"));
    assert!(queue_text.contains("project capsule and run e2e"));
}

#[test]
fn queue_artifact_wire_round_trip_is_byte_stable_and_camel_case() {
    let outcome = build_execution_capsule(&input(three_slices(), 1)).expect("build");
    let json = String::from_utf8(outcome.queue_bytes().to_vec()).expect("utf8 queue");

    assert!(json.contains("\"schemaVersion\":\"v1\""));
    assert!(json.contains("\"workItemId\""));
    assert!(json.contains("\"approvedPlanDigest\""));
    assert!(json.contains("\"totalSlices\":3"));
    assert!(json.contains("\"focusedVerificationId\""));
    assert!(json.contains("\"evidenceLogicalKey\""));
    assert!(!json.contains("work_item_id"));
    assert!(!json.contains("approved_plan_digest"));

    let decoded: ExecutionQueueV1 = serde_json::from_str(&json).expect("decode queue");
    assert_eq!(&decoded, outcome.queue());
    let reencoded = serde_json::to_string(&decoded).expect("re-encode queue");
    assert_eq!(reencoded, json);

    let mut unknown: serde_json::Value = serde_json::from_str(&json).expect("queue value");
    unknown
        .as_object_mut()
        .expect("queue object")
        .insert("unexpected".to_string(), serde_json::Value::from(1));
    assert!(serde_json::from_value::<ExecutionQueueV1>(unknown).is_err());
}

#[test]
fn queue_slices_are_ordered_by_ordinal() {
    let reversed: Vec<_> = three_slices().into_iter().rev().collect();
    let outcome = build_execution_capsule(&input(reversed, 1)).expect("build");
    let ordinals: Vec<u32> = outcome
        .queue()
        .slices()
        .iter()
        .map(ExecutionSliceV1::ordinal)
        .collect();
    assert_eq!(ordinals, vec![1, 2, 3]);
}

#[test]
fn queue_digest_binds_plan_and_slices_but_not_the_cursor() {
    let first = build_execution_capsule(&input(three_slices(), 1)).expect("first build");
    let second = build_execution_capsule(&input(three_slices(), 2)).expect("second build");

    // Moving the cursor changes the capsule (active slice) but not the queue artifact.
    assert_eq!(first.queue_digest(), second.queue_digest());
    assert_eq!(first.queue_bytes(), second.queue_bytes());
    assert_ne!(first.capsule_digest(), second.capsule_digest());

    // Changing the approved plan digest changes the queue digest.
    let mut replanned = input(three_slices(), 1);
    replanned.approved_plan_digest = ArtifactDigest::digest(b"different plan");
    let replanned = build_execution_capsule(&replanned).expect("replanned build");
    assert_ne!(first.queue_digest(), replanned.queue_digest());

    // Changing slice content changes the queue digest.
    let mut edited = input(three_slices(), 1);
    edited.slices[0].objective = "changed objective".into();
    let edited = build_execution_capsule(&edited).expect("edited build");
    assert_ne!(first.queue_digest(), edited.queue_digest());
}

#[test]
fn default_build_capsule_fits_the_capsule_budget() {
    let outcome = build_execution_capsule(&input(three_slices(), 1)).expect("build");
    assert!(outcome.capsule_bytes().len() <= MAX_CAPSULE_BYTES as usize);
    assert_eq!(
        outcome
            .capsule()
            .budgets()
            .check_capsule_len(outcome.capsule_bytes().len()),
        Ok(())
    );
}

#[test]
fn duplicate_slice_id_fails_closed() {
    let mut slices = three_slices();
    slices[1].slice_id = slice_id("slice-contract");
    assert_eq!(
        build_execution_capsule(&input(slices, 1)).unwrap_err(),
        ExecutionCapsuleBuildError::DuplicateSliceId {
            slice_id: slice_id("slice-contract"),
        }
    );
}

#[test]
fn non_contiguous_ordinals_fail_closed() {
    let mut gap = three_slices();
    gap[2].ordinal = 4;
    assert_eq!(
        build_execution_capsule(&input(gap, 1)).unwrap_err(),
        ExecutionCapsuleBuildError::NonContiguousOrdinals
    );

    let mut duplicated = three_slices();
    duplicated[1].ordinal = 1;
    assert_eq!(
        build_execution_capsule(&input(duplicated, 1)).unwrap_err(),
        ExecutionCapsuleBuildError::NonContiguousOrdinals
    );
}

#[test]
fn unknown_dependency_fails_closed() {
    let mut slices = three_slices();
    slices[1].depends_on = vec![slice_id("slice-missing")];
    assert_eq!(
        build_execution_capsule(&input(slices, 1)).unwrap_err(),
        ExecutionCapsuleBuildError::UnknownDependency {
            slice_id: slice_id("slice-runtime-wiring"),
            dependency: slice_id("slice-missing"),
        }
    );
}

#[test]
fn dependency_cycle_fails_closed() {
    let mut slices = three_slices();
    slices[0].depends_on = vec![slice_id("slice-process-test")];
    assert!(matches!(
        build_execution_capsule(&input(slices, 1)).unwrap_err(),
        ExecutionCapsuleBuildError::DependencyCycle { .. }
    ));
}

#[test]
fn self_dependency_is_rejected_as_a_cycle() {
    let mut slices = three_slices();
    slices[0].depends_on = vec![slice_id("slice-contract")];
    assert!(matches!(
        build_execution_capsule(&input(slices, 1)).unwrap_err(),
        ExecutionCapsuleBuildError::DependencyCycle { .. }
    ));
}

#[test]
fn dependency_on_later_ordinal_fails_closed() {
    let mut slices = three_slices();
    slices[1].depends_on = vec![slice_id("slice-process-test")];
    slices[2].depends_on = Vec::new();
    assert_eq!(
        build_execution_capsule(&input(slices, 1)).unwrap_err(),
        ExecutionCapsuleBuildError::DependencyNotLower {
            slice_id: slice_id("slice-runtime-wiring"),
            dependency: slice_id("slice-process-test"),
        }
    );
}

#[test]
fn active_ordinal_out_of_range_fails_closed() {
    assert_eq!(
        build_execution_capsule(&input(three_slices(), 0)).unwrap_err(),
        ExecutionCapsuleBuildError::InvalidActiveOrdinal {
            active_ordinal: 0,
            total_slices: 3,
        }
    );
    assert_eq!(
        build_execution_capsule(&input(three_slices(), 4)).unwrap_err(),
        ExecutionCapsuleBuildError::InvalidActiveOrdinal {
            active_ordinal: 4,
            total_slices: 3,
        }
    );
}

#[test]
fn empty_queue_fails_closed() {
    assert_eq!(
        build_execution_capsule(&input(Vec::new(), 1)).unwrap_err(),
        ExecutionCapsuleBuildError::Contract(ExecutionCapsuleError::EmptyQueue)
    );
}

#[test]
fn invalid_slice_spec_fails_closed() {
    let mut slices = three_slices();
    slices[0].objective = "   ".into();
    assert_eq!(
        build_execution_capsule(&input(slices, 1)).unwrap_err(),
        ExecutionCapsuleBuildError::Contract(ExecutionCapsuleError::InvalidObjective)
    );
}

#[test]
fn encoded_capsule_over_budget_fails_closed() {
    let mut fat = spec(1, "contract", "freeze contracts", Vec::new());
    fat.path_scope = (0..12_u8)
        .map(|index| scope(&format!("crates/{}{index}", "a".repeat(2_000))))
        .collect();
    fat.source_reads = Vec::new();

    let error = build_execution_capsule(&input(vec![fat], 1)).unwrap_err();
    assert!(matches!(
        error,
        ExecutionCapsuleBuildError::Contract(ExecutionCapsuleError::CapsuleBudgetExceeded {
            max_bytes,
            ..
        }) if max_bytes == MAX_CAPSULE_BYTES
    ));
}
