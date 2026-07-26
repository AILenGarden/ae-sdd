//! Execution capsule resume projection through the runtime context cache.
//!
//! The typed capsule is serialized once by `ExecutionCapsuleProjection` and
//! fed through the existing `ContextCache` full/delta/no-change machinery with
//! the cache digest bound to the canonical capsule digest: a repeated resume
//! is a bounded no-change, an active-ordinal move is a delta that never
//! re-sends unchanged story/constraints references, and plan or queue drift
//! moves the digest clients resume against.

use ae_sdd_context::ExecutionCapsuleProjection;
use ae_sdd_contracts::SchemaVersion;
use ae_sdd_contracts::execution_runtime::{
    ExecutionBudgetsV1, ExecutionCapsuleV1, ExecutionQueueRefV1, ExecutionSliceV1,
    MAX_CAPSULE_BYTES, SourceReadSpecV1,
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, ExecutionSliceId, InventoryGeneration, PolicyDigest,
    ProjectRelativePath, StateRevision, StoryId, VerificationId, WorkItemId,
};
use ae_sdd_runtime::ContextCache;

const STREAM_KEY: &str = "execution:PRD-AE-SDD-EXECUTION-EFFICIENCY-001";
const CONTEXT_REF_FIELDS: [&str; 5] = [
    "storyRef",
    "constraintsRef",
    "thinkingEngineRef",
    "verificationRef",
    "storyId",
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
        vec![ProjectRelativePath::new("crates/ae-sdd-runtime").expect("path scope")],
        vec![
            SourceReadSpecV1::new(
                ProjectRelativePath::new("crates/ae-sdd-runtime/src/lib.rs").expect("source path"),
                Some(1),
                Some(40),
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

fn capsule(plan_marker: &[u8], story_marker: &[u8], active_ordinal: u32) -> ExecutionCapsuleV1 {
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
        ExecutionBudgetsV1::default(),
    )
    .expect("valid capsule")
}

#[test]
fn full_resume_stays_within_the_capsule_budget_and_repeats_as_no_change() {
    let cache = ContextCache::new(65_536);
    let projection =
        ExecutionCapsuleProjection::new(&capsule(b"plan-v1", b"story-v1", 1)).expect("projection");

    let full = cache
        .put_execution_capsule(STREAM_KEY, 12, &projection)
        .expect("first resume projects the full capsule");
    assert_eq!(full.kind, "full");
    assert_eq!(full.context_revision, 1);
    assert!(full.byte_length <= MAX_CAPSULE_BYTES as usize);
    assert_eq!(full.digest, projection.digest().to_hex());
    let body = full.projection.as_ref().expect("full body");
    assert_eq!(body["activeSlice"]["ordinal"], 1);

    let repeat = cache
        .put_execution_capsule(STREAM_KEY, 12, &projection)
        .expect("repeated resume with the same capsule");
    assert_eq!(repeat.kind, "no_change");
    assert_eq!(repeat.byte_length, 0);
    assert_eq!(repeat.context_revision, full.context_revision);

    let resumed = cache
        .project(STREAM_KEY, full.context_revision, &full.digest)
        .expect("resume against the known digest");
    assert_eq!(resumed.kind, "no_change");
    assert!(resumed.projection.is_none());
    let response_bytes = serde_json::to_vec(&resumed).expect("no-change response encoding");
    assert!(
        response_bytes.len() <= 1_024,
        "no-change resume must stay within 1 KiB, got {} bytes",
        response_bytes.len()
    );
}

#[test]
fn ordinal_advance_projects_a_delta_without_context_bodies() {
    let cache = ContextCache::new(65_536);
    let first =
        ExecutionCapsuleProjection::new(&capsule(b"plan-v1", b"story body", 1)).expect("first");
    let full = cache
        .put_execution_capsule(STREAM_KEY, 12, &first)
        .expect("first resume");
    let second =
        ExecutionCapsuleProjection::new(&capsule(b"plan-v1", b"story body", 2)).expect("second");
    let advanced = cache
        .put_execution_capsule(STREAM_KEY, 12, &second)
        .expect("ordinal advance");
    assert_eq!(advanced.kind, "full");
    assert_eq!(advanced.context_revision, full.context_revision + 1);
    assert_ne!(advanced.digest, full.digest);

    let delta = cache
        .project(STREAM_KEY, full.context_revision, &full.digest)
        .expect("resume from the previous revision projects a delta");
    assert_eq!(delta.kind, "delta");
    assert_eq!(delta.digest, advanced.digest);
    assert!(delta.byte_length < advanced.byte_length);

    let body = delta.projection.as_ref().expect("delta body");
    let set = body["set"].as_object().expect("delta set entries");
    assert!(set.contains_key("activeSlice"));
    assert!(set.contains_key("queue"));
    for field in CONTEXT_REF_FIELDS {
        assert!(
            !set.contains_key(field),
            "delta must not re-send the unchanged {field}"
        );
    }
    let delta_text = serde_json::to_string(body).expect("delta text");
    assert!(!delta_text.contains("story body"));
    assert!(!delta_text.contains("constraints body"));
    assert!(!delta_text.contains("STORY-AE-SDD-EXECUTION-CAPSULE-001.md"));
    assert!(!delta_text.contains("constraints/README.md"));
}

#[test]
fn plan_drift_moves_the_resume_digest_and_reports_the_new_plan() {
    let cache = ContextCache::new(65_536);
    let first =
        ExecutionCapsuleProjection::new(&capsule(b"plan-v1", b"story-v1", 1)).expect("first");
    let full = cache
        .put_execution_capsule(STREAM_KEY, 12, &first)
        .expect("first resume");
    let replanned =
        ExecutionCapsuleProjection::new(&capsule(b"plan-v2", b"story-v1", 1)).expect("replanned");
    let drifted = cache
        .put_execution_capsule(STREAM_KEY, 12, &replanned)
        .expect("plan drift resume");
    assert_ne!(drifted.digest, full.digest);
    assert_eq!(
        drifted.digest,
        replanned.digest().to_hex(),
        "resume digest stays bound to the canonical capsule digest"
    );

    let resumed = cache
        .project(STREAM_KEY, full.context_revision, &full.digest)
        .expect("resume from the pre-drift revision");
    assert!(matches!(resumed.kind.as_str(), "delta" | "full"));
    let body = resumed.projection.as_ref().expect("drift body");
    let text = serde_json::to_string(body).expect("drift body text");
    let new_plan_digest = ArtifactDigest::digest(b"plan-v2/plan").to_hex();
    assert!(
        text.contains(&new_plan_digest),
        "resume after plan drift must carry the new approved plan digest"
    );
    assert!(!text.contains("constraints body"));
}
