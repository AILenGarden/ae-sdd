//! ExecutionSupervisor progress-policy contract tests.
//!
//! The supervisor is a pure reducer: `ExecutionSupervisor::decide` maps the
//! current checkpoint plus one bounded tool event to a decision
//! (`Allow`/`Deny`/`Defer`/`RequireProgress`) and a new checkpoint without
//! any I/O, clock, randomness or global state.  Progress is produced only by
//! the six machine events of plan §4.5 (new patch digest, first focused run,
//! focused non-green -> green, new blocker code + locator, new evidence
//! ledger event, legal slice state advance); repeated reads of the same
//! path/digest/range, repeated failing runs and cache hits never count.

use ae_sdd_contracts::execution_runtime::{
    ExecutionBudgetsV1, ExecutionSliceStatus, MAX_CAPSULE_BYTES,
};
use ae_sdd_domain::{ArtifactDigest, ArtifactKind, ArtifactRef, ProjectRelativePath};
use ae_sdd_execution::{
    ExecutionDecisionV1, ExecutionProgressKindV1, ExecutionSliceEvent, ExecutionSupervisor,
    ExecutionSupervisorCheckpointV1, ExecutionSupervisorError, ExecutionSupervisorFault,
    ExecutionToolEventV1, ExecutionToolOutputV1, FocusedTestOutcomeV1, FocusedTestStateV1,
};
use ae_sdd_protocol::StableErrorCode;
use proptest::prelude::*;

const KIB: u32 = 1024;
const DEFAULT_MAX_OUTPUT: u32 = 64 * KIB;

fn path(value: &str) -> ProjectRelativePath {
    ProjectRelativePath::new(value).expect("project-relative path")
}

fn artifact(name: &str) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new("tool-output").expect("artifact kind"),
        ProjectRelativePath::new(format!(".auto-engineering/tool-output/{name}.bin"))
            .expect("artifact path"),
        ArtifactDigest::digest(name.as_bytes()),
        128,
    )
}

fn output(bytes: u32, tag: &[u8]) -> ExecutionToolOutputV1 {
    ExecutionToolOutputV1 {
        bytes,
        digest: ArtifactDigest::digest(tag),
        locator: None,
    }
}

fn located_output(bytes: u32, tag: &[u8], locator: ArtifactRef) -> ExecutionToolOutputV1 {
    ExecutionToolOutputV1 {
        bytes,
        digest: ArtifactDigest::digest(tag),
        locator: Some(locator),
    }
}

fn read(
    path_value: &str,
    range: Option<(u32, u32)>,
    bytes: u32,
    tag: &[u8],
) -> ExecutionToolEventV1 {
    ExecutionToolEventV1::SourceRead {
        path: path(path_value),
        content_digest: ArtifactDigest::digest(tag),
        start_line: range.map(|range| range.0),
        end_line: range.map(|range| range.1),
        output: output(bytes, tag),
    }
}

fn search(tag: &[u8]) -> ExecutionToolEventV1 {
    ExecutionToolEventV1::Search {
        query_digest: ArtifactDigest::digest(tag),
        output: output(KIB, tag),
    }
}

fn patch(tag: &[u8]) -> ExecutionToolEventV1 {
    ExecutionToolEventV1::Patch {
        result_digest: ArtifactDigest::digest(tag),
        output: output(2 * KIB, tag),
    }
}

fn focused(outcome: FocusedTestOutcomeV1) -> ExecutionToolEventV1 {
    ExecutionToolEventV1::FocusedTest {
        outcome,
        output: output(4 * KIB, b"focused-output"),
    }
}

fn broad() -> ExecutionToolEventV1 {
    ExecutionToolEventV1::BroadTest {
        output: output(8 * KIB, b"broad-output"),
    }
}

fn evidence(tag: &[u8]) -> ExecutionToolEventV1 {
    ExecutionToolEventV1::Evidence {
        event_digest: ArtifactDigest::digest(tag),
        output: output(KIB, tag),
    }
}

fn blocker(code: &str, locator_name: &str) -> ExecutionToolEventV1 {
    ExecutionToolEventV1::Blocker {
        code: code.into(),
        locator: artifact(locator_name),
    }
}

fn other(bytes: u32) -> ExecutionToolEventV1 {
    ExecutionToolEventV1::Other {
        output: output(bytes, b"other-output"),
    }
}

fn default_budgets() -> ExecutionBudgetsV1 {
    ExecutionBudgetsV1::default()
}

fn budgets_with(files: u16, source_bytes: u32, calls: u8) -> ExecutionBudgetsV1 {
    ExecutionBudgetsV1::new(
        MAX_CAPSULE_BYTES,
        DEFAULT_MAX_OUTPUT,
        source_bytes,
        files,
        calls,
        3,
        1,
    )
    .expect("budgets within frozen v1 limits")
}

/// Reads `path-a.rs`..`path-d.rs` cyclically so twelve calls stay inside the
/// default per-batch byte and file budgets and only the call budget binds.
fn investigation_reads(count: usize) -> Vec<ExecutionToolEventV1> {
    const PATHS: [&str; 4] = [
        "crates/ae-sdd-execution/src/a.rs",
        "crates/ae-sdd-execution/src/b.rs",
        "crates/ae-sdd-execution/src/c.rs",
        "crates/ae-sdd-execution/src/d.rs",
    ];
    (0..count)
        .map(|index| {
            read(
                PATHS[index % PATHS.len()],
                Some((1, 40)),
                KIB,
                format!("read-{index}").as_bytes(),
            )
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Expect {
    Allow(Option<ExecutionProgressKindV1>),
    Deny(ExecutionSupervisorFault),
    RequireProgress(ExecutionSupervisorFault),
}

struct Case {
    name: &'static str,
    start: ExecutionSliceStatus,
    budgets: ExecutionBudgetsV1,
    events: Vec<ExecutionToolEventV1>,
    /// Expected decision for the LAST event in the sequence.
    decision: Expect,
    final_status: ExecutionSliceStatus,
    focused: FocusedTestStateV1,
    no_progress_batches: u8,
    batch_calls: u8,
}

#[rustfmt::skip]
fn cases() -> Vec<Case> {
    use ExecutionProgressKindV1 as Progress;
    use ExecutionSliceStatus as Status;
    use Expect::{Allow, Deny, RequireProgress};
    use ExecutionSupervisorFault as Fault;
    use FocusedTestOutcomeV1 as Outcome;
    use FocusedTestStateV1 as Focused;

    let running = Status::Running;
    let default = default_budgets();
    let mut cases = Vec::new();

    let mut push = |case: Case| cases.push(case);

    // --- Source reads and searches: allowed, batched, never progress. ---
    push(Case {
        name: "source read is allowed and counts toward the open batch",
        start: running,
        budgets: default,
        events: vec![read("crates/ae-sdd-execution/src/lib.rs", Some((1, 40)), 4 * KIB, b"r1")],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 1,
    });
    push(Case {
        name: "repeated read of the same path digest and range is not progress",
        start: running,
        budgets: default,
        events: vec![
            read("crates/ae-sdd-execution/src/lib.rs", Some((1, 40)), 4 * KIB, b"same"),
            read("crates/ae-sdd-execution/src/lib.rs", Some((1, 40)), 4 * KIB, b"same"),
        ],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 2,
    });
    push(Case {
        name: "rereading the same path with a new range is still not progress",
        start: running,
        budgets: default,
        events: vec![
            read("crates/ae-sdd-execution/src/lib.rs", Some((1, 40)), 4 * KIB, b"same"),
            read("crates/ae-sdd-execution/src/lib.rs", Some((41, 80)), 4 * KIB, b"same"),
        ],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 2,
    });
    push(Case {
        name: "rereading the same path after its digest changed is not progress either",
        start: running,
        budgets: default,
        events: vec![
            read("crates/ae-sdd-execution/src/lib.rs", Some((1, 40)), 4 * KIB, b"old"),
            read("crates/ae-sdd-execution/src/lib.rs", Some((1, 40)), 4 * KIB, b"new"),
        ],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 2,
    });
    push(Case {
        name: "search counts as an inspection call",
        start: running,
        budgets: default,
        events: vec![search(b"q1")],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 1,
    });
    push(Case {
        name: "four inspection calls close one no-progress batch",
        start: running,
        budgets: default,
        events: vec![
            read("crates/ae-sdd-execution/src/a.rs", None, KIB, b"b1"),
            search(b"b2"),
            read("crates/ae-sdd-execution/src/b.rs", None, KIB, b"b3"),
            search(b"b4"),
        ],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 1,
        batch_calls: 0,
    });

    // --- Patches: progress only on a new patch digest. ---
    push(Case {
        name: "a patch with a new digest is progress",
        start: running,
        budgets: default,
        events: vec![patch(b"patch-1")],
        decision: Allow(Some(Progress::NewPatchDigest)),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "reapplying the same patch digest is not progress",
        start: running,
        budgets: default,
        events: vec![patch(b"patch-1"), patch(b"patch-1")],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "a different patch digest is progress again",
        start: running,
        budgets: default,
        events: vec![patch(b"patch-1"), patch(b"patch-2")],
        decision: Allow(Some(Progress::NewPatchDigest)),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });

    // --- Focused tests: first run and non-green -> green are progress. ---
    push(Case {
        name: "the first focused run is progress even when it fails",
        start: running,
        budgets: default,
        events: vec![focused(Outcome::Fail)],
        decision: Allow(Some(Progress::FirstFocusedRun)),
        final_status: running,
        focused: Focused::Failed,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "repeating the same failing focused run is not progress",
        start: running,
        budgets: default,
        events: vec![focused(Outcome::Fail), focused(Outcome::Fail)],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Failed,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "a focused run turning from failed to green is progress",
        start: running,
        budgets: default,
        events: vec![focused(Outcome::Fail), focused(Outcome::Pass)],
        decision: Allow(Some(Progress::FocusedTurnedGreen)),
        final_status: running,
        focused: Focused::Green,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "the first focused run passing immediately is progress",
        start: running,
        budgets: default,
        events: vec![focused(Outcome::Pass)],
        decision: Allow(Some(Progress::FirstFocusedRun)),
        final_status: running,
        focused: Focused::Green,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "repeating a green focused run is not progress",
        start: running,
        budgets: default,
        events: vec![focused(Outcome::Pass), focused(Outcome::Pass)],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Green,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "a focused run failing after green is not progress and drops the green",
        start: running,
        budgets: default,
        events: vec![focused(Outcome::Pass), focused(Outcome::Fail)],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Failed,
        no_progress_batches: 0,
        batch_calls: 0,
    });

    // --- Broad tests: RequireProgress before the focused GREEN. ---
    push(Case {
        name: "a broad test before any focused run requires progress",
        start: running,
        budgets: default,
        events: vec![broad()],
        decision: RequireProgress(Fault::BroadTestBeforeFocusedGreen),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "a broad test after a failing focused run requires progress",
        start: running,
        budgets: default,
        events: vec![focused(Outcome::Fail), broad()],
        decision: RequireProgress(Fault::BroadTestBeforeFocusedGreen),
        final_status: running,
        focused: Focused::Failed,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "a broad test after the focused green is allowed",
        start: running,
        budgets: default,
        events: vec![focused(Outcome::Pass), broad()],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Green,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "a broad test loses its allowance when the focused run fails again",
        start: running,
        budgets: default,
        events: vec![focused(Outcome::Pass), focused(Outcome::Fail), broad()],
        decision: RequireProgress(Fault::BroadTestBeforeFocusedGreen),
        final_status: running,
        focused: Focused::Failed,
        no_progress_batches: 0,
        batch_calls: 0,
    });

    // --- No-progress batches: three consecutive batches stop investigation. ---
    let twelve_reads = investigation_reads(12);
    let mut exhausted = twelve_reads.clone();
    exhausted.push(read(
        "crates/ae-sdd-execution/src/e.rs",
        Some((1, 20)),
        KIB,
        b"thirteenth",
    ));
    push(Case {
        name: "the thirteenth consecutive investigation call is denied",
        start: running,
        budgets: default,
        events: exhausted.clone(),
        decision: Deny(Fault::InvestigationExhausted),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 3,
        batch_calls: 0,
    });
    let mut exhausted_search = twelve_reads.clone();
    exhausted_search.push(search(b"late-search"));
    push(Case {
        name: "after exhaustion a search is denied as well",
        start: running,
        budgets: default,
        events: exhausted_search,
        decision: Deny(Fault::InvestigationExhausted),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 3,
        batch_calls: 0,
    });
    let mut exhausted_patch = twelve_reads.clone();
    exhausted_patch.push(patch(b"recovery-patch"));
    push(Case {
        name: "after exhaustion a patch stays allowed and resets the counter",
        start: running,
        budgets: default,
        events: exhausted_patch,
        decision: Allow(Some(Progress::NewPatchDigest)),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    let mut exhausted_focused = twelve_reads.clone();
    exhausted_focused.push(focused(Outcome::Fail));
    push(Case {
        name: "after exhaustion a focused test stays allowed and resets the counter",
        start: running,
        budgets: default,
        events: exhausted_focused,
        decision: Allow(Some(Progress::FirstFocusedRun)),
        final_status: running,
        focused: Focused::Failed,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    let mut green_then_exhausted = vec![focused(Outcome::Pass)];
    green_then_exhausted.extend(twelve_reads.clone());
    green_then_exhausted.push(broad());
    push(Case {
        name: "after exhaustion even a post-green broad test is denied",
        start: running,
        budgets: default,
        events: green_then_exhausted,
        decision: Deny(Fault::InvestigationExhausted),
        final_status: running,
        focused: Focused::Green,
        no_progress_batches: 3,
        batch_calls: 0,
    });
    let mut exhausted_evidence = twelve_reads.clone();
    exhausted_evidence.push(evidence(b"late-evidence"));
    push(Case {
        name: "after exhaustion an evidence append is denied",
        start: running,
        budgets: default,
        events: exhausted_evidence,
        decision: Deny(Fault::InvestigationExhausted),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 3,
        batch_calls: 0,
    });
    let mut exhausted_other = twelve_reads.clone();
    exhausted_other.push(other(KIB));
    push(Case {
        name: "after exhaustion an unclassified tool is denied",
        start: running,
        budgets: default,
        events: exhausted_other,
        decision: Deny(Fault::InvestigationExhausted),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 3,
        batch_calls: 0,
    });

    // --- Blockers: new code + locator is progress and resets the counter. ---
    let mut blocker_reset = twelve_reads.clone();
    blocker_reset.push(blocker("missing-dependency", "blocker-1"));
    blocker_reset.push(read(
        "crates/ae-sdd-execution/src/a.rs",
        Some((1, 40)),
        KIB,
        b"after-blocker",
    ));
    push(Case {
        name: "a new blocker resets exhaustion so investigation may resume",
        start: running,
        budgets: default,
        events: blocker_reset,
        decision: Allow(None),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 1,
    });
    push(Case {
        name: "a blocker with a new code and locator is progress",
        start: running,
        budgets: default,
        events: vec![blocker("missing-dependency", "blocker-1")],
        decision: Allow(Some(Progress::NewBlocker)),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "the same blocker code with a new locator is progress",
        start: running,
        budgets: default,
        events: vec![
            blocker("missing-dependency", "blocker-1"),
            blocker("missing-dependency", "blocker-2"),
        ],
        decision: Allow(Some(Progress::NewBlocker)),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "repeating the same blocker code and locator is not progress",
        start: running,
        budgets: default,
        events: vec![
            blocker("missing-dependency", "blocker-1"),
            blocker("missing-dependency", "blocker-1"),
        ],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });

    // --- Evidence: a new ledger event is progress. ---
    push(Case {
        name: "a new evidence ledger event is progress",
        start: running,
        budgets: default,
        events: vec![evidence(b"evidence-1")],
        decision: Allow(Some(Progress::NewEvidenceEvent)),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "repeating the same evidence ledger event is not progress",
        start: running,
        budgets: default,
        events: vec![evidence(b"evidence-1"), evidence(b"evidence-1")],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });

    // --- Slice lifecycle: legal advances are progress, illegal ones denied. ---
    push(Case {
        name: "a legal slice advance is progress",
        start: Status::Pending,
        budgets: default,
        events: vec![ExecutionToolEventV1::Slice(ExecutionSliceEvent::Claimed)],
        decision: Allow(Some(Progress::SliceAdvanced)),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "an illegal slice event is denied without touching the checkpoint",
        start: Status::Pending,
        budgets: default,
        events: vec![ExecutionToolEventV1::Slice(
            ExecutionSliceEvent::FocusedTestGreen,
        )],
        decision: Deny(Fault::IllegalSliceEvent),
        final_status: Status::Pending,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "the full legal slice lifecycle advances step by step",
        start: Status::Pending,
        budgets: default,
        events: vec![
            ExecutionToolEventV1::Slice(ExecutionSliceEvent::Claimed),
            ExecutionToolEventV1::Slice(ExecutionSliceEvent::RedObserved),
            ExecutionToolEventV1::Slice(ExecutionSliceEvent::PatchApplied),
            ExecutionToolEventV1::Slice(ExecutionSliceEvent::FocusedTestGreen),
            ExecutionToolEventV1::Slice(ExecutionSliceEvent::EvidenceBound),
            ExecutionToolEventV1::Slice(ExecutionSliceEvent::Completed),
        ],
        decision: Allow(Some(Progress::SliceAdvanced)),
        final_status: Status::Completed,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "blocking and resuming the slice are both legal advances",
        start: running,
        budgets: default,
        events: vec![
            ExecutionToolEventV1::Slice(ExecutionSliceEvent::Blocked),
            ExecutionToolEventV1::Slice(ExecutionSliceEvent::Resumed),
        ],
        decision: Allow(Some(Progress::SliceAdvanced)),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });

    // --- Batch budgets and progress resets. ---
    push(Case {
        name: "progress resets the open batch so earlier reads are forgotten",
        start: running,
        budgets: default,
        events: vec![
            read("crates/ae-sdd-execution/src/a.rs", None, KIB, b"p1"),
            read("crates/ae-sdd-execution/src/b.rs", None, KIB, b"p2"),
            read("crates/ae-sdd-execution/src/c.rs", None, KIB, b"p3"),
            patch(b"mid-patch"),
            read("crates/ae-sdd-execution/src/a.rs", None, KIB, b"p4"),
            read("crates/ae-sdd-execution/src/b.rs", None, KIB, b"p5"),
            read("crates/ae-sdd-execution/src/c.rs", None, KIB, b"p6"),
            read("crates/ae-sdd-execution/src/d.rs", None, KIB, b"p7"),
        ],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 1,
        batch_calls: 0,
    });
    push(Case {
        name: "a read blowing the per-batch source byte budget is denied",
        start: running,
        budgets: default,
        events: vec![read(
            "crates/ae-sdd-execution/src/big.rs",
            None,
            30 * KIB,
            b"big-read",
        )],
        decision: Deny(Fault::BatchSourceBytesExceeded),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });
    push(Case {
        name: "a read beyond the per-batch file budget is denied",
        start: running,
        budgets: budgets_with(2, 24 * KIB, 4),
        events: vec![
            read("crates/ae-sdd-execution/src/a.rs", None, KIB, b"f1"),
            read("crates/ae-sdd-execution/src/b.rs", None, KIB, b"f2"),
            read("crates/ae-sdd-execution/src/c.rs", None, KIB, b"f3"),
        ],
        decision: Deny(Fault::BatchSourceFilesExceeded),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 2,
    });
    push(Case {
        name: "rereading an already counted file stays within the file budget",
        start: running,
        budgets: budgets_with(2, 24 * KIB, 4),
        events: vec![
            read("crates/ae-sdd-execution/src/a.rs", None, KIB, b"g1"),
            read("crates/ae-sdd-execution/src/b.rs", None, KIB, b"g2"),
            read("crates/ae-sdd-execution/src/a.rs", Some((41, 80)), KIB, b"g3"),
        ],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 3,
    });
    push(Case {
        name: "an unclassified tool is allowed and does not count as inspection",
        start: running,
        budgets: default,
        events: vec![other(2 * KIB)],
        decision: Allow(None),
        final_status: running,
        focused: Focused::Never,
        no_progress_batches: 0,
        batch_calls: 0,
    });

    cases
}

#[test]
fn table_driven_supervisor_progress_policy() {
    let cases = cases();
    assert!(
        cases.len() >= 20,
        "the supervisor table must cover at least 20 combinations, got {}",
        cases.len()
    );
    for case in &cases {
        let mut checkpoint = ExecutionSupervisorCheckpointV1::new(case.start, case.budgets);
        let mut decision = None;
        for event in &case.events {
            let (next_decision, next_checkpoint) = ExecutionSupervisor::decide(&checkpoint, event);
            decision = Some(next_decision);
            checkpoint = next_checkpoint;
        }
        let decision = decision.expect("every case drives at least one event");
        match (case.decision, &decision) {
            (Expect::Allow(expected_progress), ExecutionDecisionV1::Allow(allowance)) => {
                assert_eq!(
                    allowance.progress(),
                    expected_progress,
                    "case `{}` progress kind",
                    case.name
                );
            }
            (Expect::Deny(expected_fault), ExecutionDecisionV1::Deny(error))
            | (
                Expect::RequireProgress(expected_fault),
                ExecutionDecisionV1::RequireProgress(error),
            ) => {
                assert_eq!(
                    error.fault(),
                    expected_fault,
                    "case `{}` rejection fault",
                    case.name
                );
            }
            (expected, actual) => panic!(
                "case `{}` expected {:?}, got {:?}",
                case.name, expected, actual
            ),
        }
        assert_eq!(
            checkpoint.slice_status(),
            case.final_status,
            "case `{}` final slice status",
            case.name
        );
        assert_eq!(
            checkpoint.focused_test(),
            case.focused,
            "case `{}` final focused state",
            case.name
        );
        assert_eq!(
            checkpoint.no_progress_batches(),
            case.no_progress_batches,
            "case `{}` no-progress batch counter",
            case.name
        );
        assert_eq!(
            checkpoint.batch_calls(),
            case.batch_calls,
            "case `{}` open batch call counter",
            case.name
        );
    }
}

#[test]
fn rejections_carry_stable_protocol_error_codes() {
    let expectations = [
        (
            ExecutionSupervisorFault::InvestigationExhausted,
            StableErrorCode::ExecutionProgressRequired,
        ),
        (
            ExecutionSupervisorFault::BroadTestBeforeFocusedGreen,
            StableErrorCode::ExecutionProgressRequired,
        ),
        (
            ExecutionSupervisorFault::BatchSourceBytesExceeded,
            StableErrorCode::ExecutionBudgetExceeded,
        ),
        (
            ExecutionSupervisorFault::BatchSourceFilesExceeded,
            StableErrorCode::ExecutionBudgetExceeded,
        ),
        (
            ExecutionSupervisorFault::IllegalSliceEvent,
            StableErrorCode::ExecutionSliceInvalid,
        ),
        (
            ExecutionSupervisorFault::SliceCompleted,
            StableErrorCode::ExecutionSliceInvalid,
        ),
    ];
    for (fault, code) in expectations {
        assert_eq!(
            ExecutionSupervisorError::new(fault).error_code(),
            code,
            "{fault:?} must map to {code:?}"
        );
    }
}

#[test]
fn over_budget_output_is_truncated_and_bound_to_digest_and_locator() {
    let checkpoint =
        ExecutionSupervisorCheckpointV1::new(ExecutionSliceStatus::Running, default_budgets());
    let locator = artifact("full-focused-output");
    let event = ExecutionToolEventV1::FocusedTest {
        outcome: FocusedTestOutcomeV1::Fail,
        output: located_output(100 * KIB, b"focused-full", locator.clone()),
    };
    let (decision, _checkpoint) = ExecutionSupervisor::decide(&checkpoint, &event);
    let ExecutionDecisionV1::Allow(allowance) = decision else {
        panic!("over-budget output must still be allowed, got {decision:?}")
    };
    let directive = allowance.output().expect("output directive");
    assert!(directive.truncated());
    assert_eq!(directive.retained_bytes(), DEFAULT_MAX_OUTPUT);
    assert_eq!(
        directive.output_digest(),
        Some(ArtifactDigest::digest(b"focused-full"))
    );
    assert_eq!(directive.output_locator(), Some(&locator));
}

#[test]
fn output_at_the_budget_boundary_is_not_truncated() {
    let checkpoint =
        ExecutionSupervisorCheckpointV1::new(ExecutionSliceStatus::Running, default_budgets());
    for bytes in [KIB, DEFAULT_MAX_OUTPUT] {
        let event = other(bytes);
        let (decision, _checkpoint) = ExecutionSupervisor::decide(&checkpoint, &event);
        let ExecutionDecisionV1::Allow(allowance) = decision else {
            panic!("within-budget output must be allowed, got {decision:?}")
        };
        let directive = allowance.output().expect("output directive");
        assert!(!directive.truncated(), "{bytes} bytes must not truncate");
        assert_eq!(directive.retained_bytes(), bytes);
        assert_eq!(directive.output_digest(), None);
        assert_eq!(directive.output_locator(), None);
    }
}

#[test]
fn one_byte_over_the_output_budget_truncates() {
    let checkpoint =
        ExecutionSupervisorCheckpointV1::new(ExecutionSliceStatus::Running, default_budgets());
    let event = other(DEFAULT_MAX_OUTPUT + 1);
    let (decision, _checkpoint) = ExecutionSupervisor::decide(&checkpoint, &event);
    let ExecutionDecisionV1::Allow(allowance) = decision else {
        panic!("over-budget output must still be allowed, got {decision:?}")
    };
    let directive = allowance.output().expect("output directive");
    assert!(directive.truncated());
    assert_eq!(directive.retained_bytes(), DEFAULT_MAX_OUTPUT);
}

#[test]
fn an_oversized_source_read_is_denied_by_the_batch_byte_budget_not_truncated() {
    // A 100 KiB read truncates to the 64 KiB retained budget, which still
    // exceeds the 24 KiB per-batch source budget: the read itself is denied.
    let checkpoint =
        ExecutionSupervisorCheckpointV1::new(ExecutionSliceStatus::Running, default_budgets());
    let event = read(
        "crates/ae-sdd-execution/src/huge.rs",
        None,
        100 * KIB,
        b"huge",
    );
    let (decision, next) = ExecutionSupervisor::decide(&checkpoint, &event);
    let ExecutionDecisionV1::Deny(error) = decision else {
        panic!("oversized read must be denied, got {decision:?}")
    };
    assert_eq!(
        error.fault(),
        ExecutionSupervisorFault::BatchSourceBytesExceeded
    );
    assert_eq!(error.error_code(), StableErrorCode::ExecutionBudgetExceeded);
    assert_eq!(
        next, checkpoint,
        "a denied event must not mutate the checkpoint"
    );
}

#[test]
fn completed_slice_denies_every_event_and_never_mutates() {
    let checkpoint =
        ExecutionSupervisorCheckpointV1::new(ExecutionSliceStatus::Completed, default_budgets());
    let mut events = vec![
        read(
            "crates/ae-sdd-execution/src/lib.rs",
            Some((1, 40)),
            KIB,
            b"c1",
        ),
        search(b"c2"),
        patch(b"c3"),
        focused(FocusedTestOutcomeV1::Fail),
        focused(FocusedTestOutcomeV1::Pass),
        broad(),
        evidence(b"c4"),
        blocker("late-blocker", "blocker-late"),
        other(KIB),
    ];
    for slice_event in [
        ExecutionSliceEvent::Claimed,
        ExecutionSliceEvent::RedObserved,
        ExecutionSliceEvent::PatchApplied,
        ExecutionSliceEvent::FocusedTestGreen,
        ExecutionSliceEvent::EvidenceBound,
        ExecutionSliceEvent::Completed,
        ExecutionSliceEvent::Blocked,
        ExecutionSliceEvent::Resumed,
    ] {
        events.push(ExecutionToolEventV1::Slice(slice_event));
    }
    for event in &events {
        let (decision, next) = ExecutionSupervisor::decide(&checkpoint, event);
        let ExecutionDecisionV1::Deny(error) = decision else {
            panic!("completed slice must deny {event:?}, got {decision:?}")
        };
        assert_eq!(error.fault(), ExecutionSupervisorFault::SliceCompleted);
        assert_eq!(error.error_code(), StableErrorCode::ExecutionSliceInvalid);
        assert_eq!(next, checkpoint, "completed slice must never mutate");
    }
}

#[test]
fn identical_input_produces_identical_decisions() {
    let events = investigation_reads(5)
        .into_iter()
        .chain([
            patch(b"det-1"),
            focused(FocusedTestOutcomeV1::Fail),
            focused(FocusedTestOutcomeV1::Pass),
            broad(),
        ])
        .collect::<Vec<_>>();
    let baseline =
        ExecutionSupervisorCheckpointV1::new(ExecutionSliceStatus::Running, default_budgets());
    let mut first = baseline.clone();
    let mut second = baseline;
    for event in &events {
        let (first_decision, first_next) = ExecutionSupervisor::decide(&first, event);
        let (second_decision, second_next) = ExecutionSupervisor::decide(&second, event);
        assert_eq!(first_decision, second_decision);
        assert_eq!(first_next, second_next);
        first = first_next;
        second = second_next;
    }
}

fn arb_budgets() -> impl Strategy<Value = ExecutionBudgetsV1> {
    (
        1_u8..=4,
        1_u16..=6,
        (8 * KIB)..=(64 * KIB),
        1_u8..=6,
        (16 * KIB)..=(64 * KIB),
    )
        .prop_map(
            |(calls, files, source_bytes, max_no_progress, tool_output)| {
                ExecutionBudgetsV1::new(
                    MAX_CAPSULE_BYTES,
                    tool_output,
                    source_bytes,
                    files,
                    calls,
                    max_no_progress,
                    1,
                )
                .expect("strategy stays within frozen v1 limits")
            },
        )
}

fn arb_digest() -> impl Strategy<Value = ArtifactDigest> {
    any::<u8>().prop_map(|seed| ArtifactDigest::digest([seed]))
}

fn arb_path() -> impl Strategy<Value = ProjectRelativePath> {
    prop::sample::select(vec![
        path("crates/ae-sdd-execution/src/lib.rs"),
        path("crates/ae-sdd-execution/src/supervisor.rs"),
        path("crates/ae-sdd-execution/src/policy.rs"),
        path("crates/ae-sdd-execution/src/error.rs"),
    ])
}

fn arb_range() -> impl Strategy<Value = (Option<u32>, Option<u32>)> {
    prop::option::of((1_u32..=200).prop_flat_map(|start| {
        (Just(start), (start..=start.saturating_add(80)))
            .prop_map(|(start, end)| (Some(start), Some(end)))
    }))
    .prop_map(|range| range.unwrap_or((None, None)))
}

fn arb_output() -> impl Strategy<Value = ExecutionToolOutputV1> {
    (0_u32..=(100 * KIB), any::<u8>(), prop::bool::ANY).prop_map(|(bytes, seed, located)| {
        ExecutionToolOutputV1 {
            bytes,
            digest: ArtifactDigest::digest([seed]),
            locator: located.then(|| artifact("prop-output")),
        }
    })
}

fn arb_event() -> impl Strategy<Value = ExecutionToolEventV1> {
    prop_oneof![
        (arb_path(), arb_digest(), arb_range(), arb_output()).prop_map(
            |(path, content_digest, (start_line, end_line), output)| {
                ExecutionToolEventV1::SourceRead {
                    path,
                    content_digest,
                    start_line,
                    end_line,
                    output,
                }
            }
        ),
        (arb_digest(), arb_output()).prop_map(|(query_digest, output)| {
            ExecutionToolEventV1::Search {
                query_digest,
                output,
            }
        }),
        (arb_digest(), arb_output()).prop_map(|(result_digest, output)| {
            ExecutionToolEventV1::Patch {
                result_digest,
                output,
            }
        }),
        (prop::bool::ANY, arb_output()).prop_map(|(pass, output)| {
            ExecutionToolEventV1::FocusedTest {
                outcome: if pass {
                    FocusedTestOutcomeV1::Pass
                } else {
                    FocusedTestOutcomeV1::Fail
                },
                output,
            }
        }),
        arb_output().prop_map(|output| ExecutionToolEventV1::BroadTest { output }),
        (arb_digest(), arb_output()).prop_map(|(event_digest, output)| {
            ExecutionToolEventV1::Evidence {
                event_digest,
                output,
            }
        }),
        (
            prop::sample::select(vec![
                "missing-dependency",
                "needs-decision",
                "environment-broken"
            ]),
            prop::sample::select(vec![artifact("prop-blocker-1"), artifact("prop-blocker-2")]),
        )
            .prop_map(|(code, locator)| ExecutionToolEventV1::Blocker {
                code: code.into(),
                locator,
            }),
        prop::sample::select(vec![
            ExecutionSliceEvent::Claimed,
            ExecutionSliceEvent::RedObserved,
            ExecutionSliceEvent::PatchApplied,
            ExecutionSliceEvent::FocusedTestGreen,
            ExecutionSliceEvent::EvidenceBound,
            ExecutionSliceEvent::Completed,
            ExecutionSliceEvent::Blocked,
            ExecutionSliceEvent::Resumed,
        ])
        .prop_map(ExecutionToolEventV1::Slice),
        arb_output().prop_map(|output| ExecutionToolEventV1::Other { output }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn completed_slice_never_regresses(
        event in arb_event(),
        budgets in arb_budgets(),
    ) {
        let checkpoint =
            ExecutionSupervisorCheckpointV1::new(ExecutionSliceStatus::Completed, budgets);
        let (decision, next) = ExecutionSupervisor::decide(&checkpoint, &event);
        prop_assert!(
            matches!(decision, ExecutionDecisionV1::Deny(_)),
            "completed slice must deny {event:?}, got {decision:?}"
        );
        prop_assert_eq!(next, checkpoint);
    }

    #[test]
    fn broad_test_is_never_allowed_before_the_focused_green(
        script in prop::collection::vec(arb_event(), 0..24),
    ) {
        let mut checkpoint =
            ExecutionSupervisorCheckpointV1::new(ExecutionSliceStatus::Running, default_budgets());
        for event in &script {
            let (decision, next) = ExecutionSupervisor::decide(&checkpoint, event);
            if matches!(event, ExecutionToolEventV1::BroadTest { .. })
                && matches!(decision, ExecutionDecisionV1::Allow(_))
            {
                prop_assert_eq!(
                    checkpoint.focused_test(),
                    FocusedTestStateV1::Green,
                    "a broad test may only be allowed after the focused green"
                );
            }
            checkpoint = next;
        }
    }

    #[test]
    fn no_progress_counter_never_exceeds_the_configured_max(
        script in prop::collection::vec(arb_event(), 0..48),
        budgets in arb_budgets(),
    ) {
        let mut checkpoint =
            ExecutionSupervisorCheckpointV1::new(ExecutionSliceStatus::Running, budgets);
        for event in &script {
            let (_decision, next) = ExecutionSupervisor::decide(&checkpoint, event);
            prop_assert!(
                next.no_progress_batches() <= budgets.max_no_progress_batches(),
                "no-progress counter {} exceeds the configured max {}",
                next.no_progress_batches(),
                budgets.max_no_progress_batches()
            );
            checkpoint = next;
        }
    }

    #[test]
    fn replaying_a_script_yields_identical_decisions_and_checkpoints(
        script in prop::collection::vec(arb_event(), 0..32),
        budgets in arb_budgets(),
    ) {
        let baseline = ExecutionSupervisorCheckpointV1::new(ExecutionSliceStatus::Running, budgets);
        let mut first = baseline.clone();
        let mut second = baseline;
        for event in &script {
            let (first_decision, first_next) = ExecutionSupervisor::decide(&first, event);
            let (second_decision, second_next) = ExecutionSupervisor::decide(&second, event);
            prop_assert_eq!(&first_decision, &second_decision);
            prop_assert_eq!(&first_next, &second_next);
            first = first_next;
            second = second_next;
        }
    }
}
