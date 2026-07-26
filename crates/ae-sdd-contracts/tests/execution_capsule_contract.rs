use ae_sdd_contracts::SchemaVersion;
use ae_sdd_contracts::execution_runtime::{
    DEFAULT_INSPECTION_CALLS_PER_BATCH, DEFAULT_MAX_AUTHORITY_REFRESHES_PER_RESUME,
    DEFAULT_MAX_NO_PROGRESS_BATCHES, DEFAULT_MAX_SOURCE_FILES_PER_BATCH,
    DEFAULT_MAX_SOURCE_READ_BYTES_PER_BATCH, DEFAULT_MAX_TOOL_OUTPUT_BYTES, ExecutionBudgetsV1,
    ExecutionCapsuleError, ExecutionCapsuleV1, ExecutionQueueRefV1, ExecutionSliceStatus,
    ExecutionSliceV1, ExecutionToolClass, MAX_CAPSULE_BYTES, MAX_OBJECTIVE_BYTES, SourceReadSpecV1,
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, ExecutionSliceId, InventoryGeneration, PolicyDigest,
    ProjectRelativePath, StateRevision, StoryId, VerificationId, WorkItemId,
};

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

fn read_spec(path: &str) -> SourceReadSpecV1 {
    SourceReadSpecV1::new(scope(path), Some(1), Some(40)).expect("source read spec")
}

fn slice(ordinal: u32) -> ExecutionSliceV1 {
    slice_with_objective(ordinal, &format!("implement slice {ordinal}")).expect("valid slice")
}

fn slice_with_objective(
    ordinal: u32,
    objective: &str,
) -> Result<ExecutionSliceV1, ExecutionCapsuleError> {
    ExecutionSliceV1::new(
        slice_id(&format!("slice-{ordinal:03}")),
        ordinal,
        objective,
        Vec::new(),
        vec![scope("crates/ae-sdd-contracts")],
        vec![read_spec("crates/ae-sdd-contracts/src/lib.rs")],
        verification_id("V-EFF-001b"),
        vec![verification_id("V-EFF-001c")],
        format!("execution/slice-{ordinal:03}"),
    )
}

fn try_queue(
    total_slices: u32,
    completed_slices: u32,
    active_ordinal: u32,
) -> Result<ExecutionQueueRefV1, ExecutionCapsuleError> {
    ExecutionQueueRefV1::new(
        artifact(
            ".auto-engineering/PRD-AE-SDD-EXECUTION-EFFICIENCY-001/execution/queue.json",
            b"queue",
        ),
        ArtifactDigest::digest(b"queue"),
        total_slices,
        completed_slices,
        active_ordinal,
    )
}

fn queue(total_slices: u32, completed_slices: u32, active_ordinal: u32) -> ExecutionQueueRefV1 {
    try_queue(total_slices, completed_slices, active_ordinal).expect("valid queue")
}

fn try_capsule(
    active_slice: ExecutionSliceV1,
    queue: ExecutionQueueRefV1,
) -> Result<ExecutionCapsuleV1, ExecutionCapsuleError> {
    ExecutionCapsuleV1::new(
        SchemaVersion::V1,
        WorkItemId::new("PRD-AE-SDD-EXECUTION-EFFICIENCY-001").expect("work item id"),
        StoryId::new("STORY-AE-SDD-EXECUTION-CAPSULE-001").expect("story id"),
        StateRevision::new(7),
        ArtifactDigest::digest(b"approved plan"),
        PolicyDigest::digest(b"policy"),
        InventoryGeneration::new(3),
        artifact(
            "ae-sdd-doc/Story/STORY-AE-SDD-EXECUTION-CAPSULE-001.md",
            b"story",
        ),
        artifact("constraints/README.md", b"constraints"),
        artifact(
            "source/standards/thinking/be-coding-thinking-engine.md",
            b"thinking",
        ),
        artifact("ae-sdd-doc/Test/TEST-STORY-001.md", b"verification"),
        queue,
        active_slice,
        ExecutionBudgetsV1::default(),
    )
}

fn capsule(active_slice: ExecutionSliceV1, queue: ExecutionQueueRefV1) -> ExecutionCapsuleV1 {
    try_capsule(active_slice, queue).expect("valid capsule")
}

#[test]
fn capsule_wire_round_trip_is_byte_stable_and_camel_case() {
    let capsule = capsule(slice(2), queue(3, 1, 2));

    let json = serde_json::to_string(&capsule).expect("serialize capsule");
    assert!(json.contains("\"schemaVersion\":\"v1\""));
    assert!(json.contains("\"activeSlice\""));
    assert!(json.contains("\"focusedVerificationId\""));
    assert!(json.contains("\"approvedPlanDigest\""));
    assert!(json.contains("\"maxCapsuleBytes\""));
    assert!(!json.contains("slice_id"));
    assert!(!json.contains("focused_verification_id"));

    let decoded: ExecutionCapsuleV1 = serde_json::from_str(&json).expect("deserialize capsule");
    assert_eq!(decoded, capsule);
    let reencoded = serde_json::to_string(&decoded).expect("re-serialize capsule");
    assert_eq!(reencoded, json);
}

#[test]
fn slice_status_and_tool_class_use_kebab_case_wire_values() {
    let status_cases = [
        (ExecutionSliceStatus::Pending, "pending"),
        (ExecutionSliceStatus::Running, "running"),
        (ExecutionSliceStatus::RedObserved, "red-observed"),
        (ExecutionSliceStatus::Patched, "patched"),
        (ExecutionSliceStatus::FocusedGreen, "focused-green"),
        (ExecutionSliceStatus::EvidenceBound, "evidence-bound"),
        (ExecutionSliceStatus::Completed, "completed"),
        (ExecutionSliceStatus::Blocked, "blocked"),
    ];
    for (status, wire) in status_cases {
        let encoded = serde_json::to_string(&status).expect("serialize status");
        assert_eq!(encoded, format!("\"{wire}\""));
        assert_eq!(
            serde_json::from_str::<ExecutionSliceStatus>(&encoded).expect("decode status"),
            status
        );
    }

    let tool_cases = [
        (ExecutionToolClass::SourceRead, "source-read"),
        (ExecutionToolClass::Search, "search"),
        (ExecutionToolClass::Patch, "patch"),
        (ExecutionToolClass::FocusedTest, "focused-test"),
        (ExecutionToolClass::BroadTest, "broad-test"),
        (ExecutionToolClass::Evidence, "evidence"),
        (ExecutionToolClass::Other, "other"),
    ];
    for (class, wire) in tool_cases {
        let encoded = serde_json::to_string(&class).expect("serialize tool class");
        assert_eq!(encoded, format!("\"{wire}\""));
        assert_eq!(
            serde_json::from_str::<ExecutionToolClass>(&encoded).expect("decode tool class"),
            class
        );
    }

    assert!(serde_json::from_str::<ExecutionSliceStatus>("\"RedObserved\"").is_err());
    assert!(serde_json::from_str::<ExecutionToolClass>("\"focusedTest\"").is_err());
}

#[test]
fn duplicate_slice_id_in_dependencies_is_canonicalized() {
    let first = ExecutionSliceV1::new(
        slice_id("slice-003"),
        3,
        "implement slice 3",
        vec![
            slice_id("slice-002"),
            slice_id("slice-001"),
            slice_id("slice-002"),
        ],
        vec![scope("crates/ae-sdd-contracts")],
        Vec::new(),
        verification_id("V-EFF-001b"),
        vec![
            verification_id("V-EFF-001c"),
            verification_id("V-EFF-001a"),
            verification_id("V-EFF-001c"),
        ],
        "execution/slice-003",
    )
    .expect("valid slice");
    let second = ExecutionSliceV1::new(
        slice_id("slice-003"),
        3,
        "implement slice 3",
        vec![slice_id("slice-001"), slice_id("slice-002")],
        vec![scope("crates/ae-sdd-contracts")],
        Vec::new(),
        verification_id("V-EFF-001b"),
        vec![verification_id("V-EFF-001a"), verification_id("V-EFF-001c")],
        "execution/slice-003",
    )
    .expect("valid slice");

    assert_eq!(first, second);
    assert_eq!(
        first.depends_on(),
        &[slice_id("slice-001"), slice_id("slice-002")]
    );
    assert_eq!(
        first.broad_verification_ids(),
        &[verification_id("V-EFF-001a"), verification_id("V-EFF-001c")]
    );
}

#[test]
fn slice_collections_are_canonically_sorted_and_deduplicated() {
    let slice = ExecutionSliceV1::new(
        slice_id("slice-002"),
        2,
        "implement slice 2",
        Vec::new(),
        vec![scope("crates/b"), scope("crates/a"), scope("crates/b")],
        vec![
            read_spec("crates/b/x.rs"),
            read_spec("crates/a/x.rs"),
            read_spec("crates/b/x.rs"),
        ],
        verification_id("V-EFF-001b"),
        Vec::new(),
        "execution/slice-002",
    )
    .expect("valid slice");

    assert_eq!(slice.path_scope(), &[scope("crates/a"), scope("crates/b")]);
    assert_eq!(
        slice.source_reads(),
        &[read_spec("crates/a/x.rs"), read_spec("crates/b/x.rs")]
    );
}

#[test]
fn non_contiguous_queue_ordinal_fails_closed() {
    assert_eq!(
        try_queue(3, 1, 3),
        Err(ExecutionCapsuleError::NonContiguousOrdinal)
    );
    assert_eq!(
        try_queue(3, 3, 4),
        Err(ExecutionCapsuleError::NonContiguousOrdinal)
    );
    assert_eq!(try_queue(0, 0, 1), Err(ExecutionCapsuleError::EmptyQueue));
    assert!(try_queue(3, 2, 3).is_ok());
    assert!(try_queue(1, 0, 1).is_ok());
}

#[test]
fn active_slice_ordinal_must_match_queue_cursor() {
    assert_eq!(
        try_capsule(slice(1), queue(3, 1, 2)),
        Err(ExecutionCapsuleError::ActiveOrdinalMismatch)
    );
}

#[test]
fn empty_objective_fails_closed() {
    for objective in ["", "   ", "\t\n"] {
        assert_eq!(
            slice_with_objective(1, objective),
            Err(ExecutionCapsuleError::InvalidObjective)
        );
    }
    let oversized = "a".repeat(MAX_OBJECTIVE_BYTES + 1);
    assert_eq!(
        slice_with_objective(1, &oversized),
        Err(ExecutionCapsuleError::InvalidObjective)
    );
}

#[test]
fn slice_rejects_zero_ordinal() {
    assert_eq!(
        slice_with_objective(0, "implement slice 0"),
        Err(ExecutionCapsuleError::InvalidOrdinal)
    );
}

#[test]
fn source_read_outside_path_scope_fails_closed() {
    let outside = ExecutionSliceV1::new(
        slice_id("slice-001"),
        1,
        "implement slice 1",
        Vec::new(),
        vec![scope("crates/ae-sdd-contracts")],
        vec![read_spec("crates/ae-sdd-domain/src/lib.rs")],
        verification_id("V-EFF-001b"),
        Vec::new(),
        "execution/slice-001",
    );
    assert_eq!(outside, Err(ExecutionCapsuleError::SourceReadOutOfScope));

    let sibling_prefix = ExecutionSliceV1::new(
        slice_id("slice-001"),
        1,
        "implement slice 1",
        Vec::new(),
        vec![scope("crates/ae-sdd-contracts")],
        vec![read_spec("crates/ae-sdd-contracts-old/src/lib.rs")],
        verification_id("V-EFF-001b"),
        Vec::new(),
        "execution/slice-001",
    );
    assert_eq!(
        sibling_prefix,
        Err(ExecutionCapsuleError::SourceReadOutOfScope)
    );

    let empty_scope = ExecutionSliceV1::new(
        slice_id("slice-001"),
        1,
        "implement slice 1",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        verification_id("V-EFF-001b"),
        Vec::new(),
        "execution/slice-001",
    );
    assert_eq!(empty_scope, Err(ExecutionCapsuleError::EmptyPathScope));
}

#[test]
fn invalid_source_read_line_range_fails_closed() {
    assert_eq!(
        SourceReadSpecV1::new(scope("src/lib.rs"), Some(4), Some(2)),
        Err(ExecutionCapsuleError::InvalidLineRange)
    );
    assert_eq!(
        SourceReadSpecV1::new(scope("src/lib.rs"), Some(0), Some(2)),
        Err(ExecutionCapsuleError::InvalidLineRange)
    );
    assert_eq!(
        SourceReadSpecV1::new(scope("src/lib.rs"), Some(1), None),
        Err(ExecutionCapsuleError::InvalidLineRange)
    );
    assert_eq!(
        SourceReadSpecV1::new(scope("src/lib.rs"), None, Some(2)),
        Err(ExecutionCapsuleError::InvalidLineRange)
    );
    assert!(SourceReadSpecV1::new(scope("src/lib.rs"), None, None).is_ok());
    assert!(SourceReadSpecV1::new(scope("src/lib.rs"), Some(2), Some(2)).is_ok());
}

#[test]
fn missing_focused_verification_fails_closed() {
    let capsule = capsule(slice(2), queue(3, 1, 2));
    let value = serde_json::to_value(&capsule).expect("serialize capsule");

    let mut missing = value.clone();
    missing
        .get_mut("activeSlice")
        .and_then(serde_json::Value::as_object_mut)
        .expect("active slice object")
        .remove("focusedVerificationId");
    assert!(serde_json::from_value::<ExecutionCapsuleV1>(missing).is_err());

    let mut unknown = value;
    unknown
        .get_mut("activeSlice")
        .and_then(serde_json::Value::as_object_mut)
        .expect("active slice object")
        .insert("unexpected".to_string(), serde_json::Value::from(1));
    assert!(serde_json::from_value::<ExecutionCapsuleV1>(unknown).is_err());
}

#[test]
fn capsule_budget_hard_limit_fails_closed() {
    assert_eq!(
        ExecutionBudgetsV1::new(MAX_CAPSULE_BYTES + 1, 64 * 1024, 24 * 1024, 12, 4, 3, 1),
        Err(ExecutionCapsuleError::InvalidBudget)
    );
    assert_eq!(
        ExecutionBudgetsV1::new(0, 64 * 1024, 24 * 1024, 12, 4, 3, 1),
        Err(ExecutionCapsuleError::InvalidBudget)
    );
    assert_eq!(
        ExecutionBudgetsV1::new(MAX_CAPSULE_BYTES, 0, 24 * 1024, 12, 4, 3, 1),
        Err(ExecutionCapsuleError::InvalidBudget)
    );
    assert!(ExecutionBudgetsV1::new(MAX_CAPSULE_BYTES, 64 * 1024, 24 * 1024, 12, 4, 3, 1).is_ok());
}

#[test]
fn encoded_capsule_over_budget_fails_closed() {
    let mut fat_scope = Vec::new();
    for index in 0..12_u8 {
        fat_scope.push(scope(&format!("crates/{}{index}", "a".repeat(2_000))));
    }
    let fat_slice = ExecutionSliceV1::new(
        slice_id("slice-001"),
        1,
        "implement slice 1",
        Vec::new(),
        fat_scope,
        Vec::new(),
        verification_id("V-EFF-001b"),
        Vec::new(),
        "execution/slice-001",
    )
    .expect("valid fat slice");
    let fat = capsule(fat_slice, queue(1, 0, 1));
    let encoded = serde_json::to_string(&fat).expect("serialize fat capsule");
    assert!(encoded.len() > MAX_CAPSULE_BYTES as usize);
    assert_eq!(
        fat.budgets().check_capsule_len(encoded.len()),
        Err(ExecutionCapsuleError::CapsuleBudgetExceeded {
            max_bytes: MAX_CAPSULE_BYTES,
            actual_bytes: encoded.len(),
        })
    );

    let lean = capsule(slice(1), queue(1, 0, 1));
    let encoded = serde_json::to_string(&lean).expect("serialize lean capsule");
    assert!(encoded.len() <= MAX_CAPSULE_BYTES as usize);
    assert_eq!(lean.budgets().check_capsule_len(encoded.len()), Ok(()));
}

#[test]
fn default_budgets_match_frozen_v1_table() {
    let budgets = ExecutionBudgetsV1::default();
    assert_eq!(budgets.max_capsule_bytes(), MAX_CAPSULE_BYTES);
    assert_eq!(
        budgets.max_tool_output_bytes(),
        DEFAULT_MAX_TOOL_OUTPUT_BYTES
    );
    assert_eq!(
        budgets.max_source_read_bytes_per_batch(),
        DEFAULT_MAX_SOURCE_READ_BYTES_PER_BATCH
    );
    assert_eq!(
        budgets.max_source_files_per_batch(),
        DEFAULT_MAX_SOURCE_FILES_PER_BATCH
    );
    assert_eq!(
        budgets.inspection_calls_per_batch(),
        DEFAULT_INSPECTION_CALLS_PER_BATCH
    );
    assert_eq!(
        budgets.max_no_progress_batches(),
        DEFAULT_MAX_NO_PROGRESS_BATCHES
    );
    assert_eq!(
        budgets.max_authority_refreshes_per_resume(),
        DEFAULT_MAX_AUTHORITY_REFRESHES_PER_RESUME
    );
}
