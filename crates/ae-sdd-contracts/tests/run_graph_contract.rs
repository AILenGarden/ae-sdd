//! Execution flow tree contract tests (`ae-sdd-daemon-design.md` §4.2, line 767).

use ae_sdd_contracts::{
    FlowRunProjection, RunGraphError, SchemaVersion, SeriesId, SeriesKind, SeriesLifecycleState,
    SeriesRunProjection, SeriesSubNode,
};
use ae_sdd_domain::{EpochMillis, FlowRunId, SeriesRunId, StateRevision, WorkItemId};
use uuid::Uuid;

fn flow_run(n: u128) -> FlowRunId {
    FlowRunId::from_uuid(Uuid::from_u128(n))
}

fn series_run(n: u128) -> SeriesRunId {
    SeriesRunId::from_uuid(Uuid::from_u128(n))
}

fn attempt(
    run: SeriesRunId,
    ordinal: u32,
    retry_of: Option<SeriesRunId>,
) -> Result<SeriesRunProjection, RunGraphError> {
    SeriesRunProjection::record(
        run,
        SeriesId::new("SER-STORY-1").expect("series id"),
        flow_run(0x10),
        None,
        SeriesKind::new("story").expect("series kind"),
        SeriesSubNode::Draft,
        SeriesLifecycleState::Running,
        ordinal,
        retry_of,
        StateRevision::new(7),
        EpochMillis::new(1_700_000_000_000),
    )
}

/// Line 767 requires the three relation families use independent IDs and 不会因
/// Series 重试互相污染. A projection keyed by the logical `SeriesId` cannot satisfy
/// that: two attempts collapse onto one key. This is the concrete defect in the
/// existing `series_plan_projection` table, whose primary key is
/// `(workspace_id, series_id)`.
#[test]
fn two_attempts_at_one_series_are_separately_addressable() {
    let first = attempt(series_run(0x01), 1, None).expect("first attempt");
    let second = attempt(series_run(0x02), 2, Some(series_run(0x01))).expect("retry");

    assert_eq!(
        first.series_id(),
        second.series_id(),
        "§4.1: a retry keeps the same logical SeriesId"
    );
    assert_ne!(
        first.series_run_id(),
        second.series_run_id(),
        "§4.1: a retry mints a new run identity, so the two are distinguishable"
    );
    assert_eq!(
        second.retry_of(),
        Some(first.series_run_id()),
        "F-06 requires the replaced attempt stay named, or the retry history is a \
         flat list of unrelated runs"
    );
    assert_eq!(first.retry_of(), None, "a first attempt replaces nothing");
}

/// §4.2's diagram nests Series Runs (`ST --> TC --> CP`) rather than flattening
/// them under the Flow Run. Without the parent edge, a Work Item with several
/// Stories has indistinguishable TestCase runs, and §9.2's requirement that a
/// CodingPlan bind 同一 Story 的已批准 TestCase cannot be checked from the
/// projection.
#[test]
fn a_story_subchain_nests_under_its_story_run() {
    let story = series_run(0x11);
    let testcase = SeriesRunProjection::record(
        series_run(0x12),
        SeriesId::new("SER-TESTCASE-1").expect("series id"),
        flow_run(0x10),
        Some(story),
        SeriesKind::new("testcase").expect("series kind"),
        SeriesSubNode::Draft,
        SeriesLifecycleState::Running,
        1,
        None,
        StateRevision::new(7),
        EpochMillis::new(1),
    )
    .expect("testcase run");

    assert_eq!(
        testcase.parent_series_run_id(),
        Some(story),
        "a TestCase run hangs off its Story run, not off the Flow Run"
    );
    assert_eq!(
        testcase.flow_run_id(),
        flow_run(0x10),
        "it still belongs to the Flow Run, so the tree stays connected"
    );
}

/// A cyclic execution tree makes "all attempts of this Series" and "the runs under
/// this Story" non-terminating walks.
#[test]
fn a_run_cannot_be_its_own_parent_or_predecessor() {
    let run = series_run(0x21);

    assert!(matches!(
        attempt(run, 2, Some(run)),
        Err(RunGraphError::SelfRetry)
    ));

    let self_parent = SeriesRunProjection::record(
        run,
        SeriesId::new("SER-STORY-1").expect("series id"),
        flow_run(0x10),
        Some(run),
        SeriesKind::new("story").expect("series kind"),
        SeriesSubNode::Draft,
        SeriesLifecycleState::Running,
        1,
        None,
        StateRevision::new(7),
        EpochMillis::new(1),
    );
    assert!(matches!(self_parent, Err(RunGraphError::SelfParent)));
}

/// `attempt_ordinal` and `retry_of` are two statements about the same fact, so
/// they can disagree. An ordinal-3 run with no predecessor claims to be a retry of
/// nothing, which breaks the chain F-06 requires to stay walkable.
#[test]
fn the_attempt_ordinal_must_agree_with_the_retry_edge() {
    assert!(
        matches!(
            attempt(series_run(0x31), 3, None),
            Err(RunGraphError::RetryOrdinalMismatch { ordinal: 3 })
        ),
        "a later attempt must name what it replaces"
    );
    assert!(
        matches!(
            attempt(series_run(0x32), 1, Some(series_run(0x33))),
            Err(RunGraphError::RetryOrdinalMismatch { ordinal: 1 })
        ),
        "a first attempt cannot replace anything"
    );
}

/// §11.1 makes `currentMainNode` and the envelope's `mainNode` draw from one
/// frozen list, so a projection must not be able to disagree with the envelope
/// about where the flow is.
#[test]
fn run_projections_refuse_a_main_node_outside_the_frozen_vocabulary() {
    let flow = FlowRunProjection::open(
        flow_run(0x41),
        WorkItemId::new("WI-1").expect("work item"),
        SeriesKind::new("story-generate").expect("series kind"),
        StateRevision::new(1),
        EpochMillis::new(1),
        EpochMillis::new(1),
    );
    assert!(matches!(flow, Err(RunGraphError::MainNodeNotFrozen { .. })));

    let ok = FlowRunProjection::open(
        flow_run(0x41),
        WorkItemId::new("WI-1").expect("work item"),
        SeriesKind::new("story").expect("series kind"),
        StateRevision::new(1),
        EpochMillis::new(1),
        EpochMillis::new(1),
    )
    .expect("frozen main node");
    assert_eq!(ok.schema_version(), SchemaVersion::V1);
}

/// Both projections cross the wire, and a payload the constructor would refuse
/// must not decode — otherwise the guarantees stop at the process boundary.
#[test]
fn run_projections_round_trip_and_the_wire_enforces_the_same_guards() {
    let run = attempt(series_run(0x51), 2, Some(series_run(0x50))).expect("retry");
    let encoded = serde_json::to_value(&run).expect("serialize");

    assert_eq!(encoded["attemptOrdinal"], serde_json::json!(2));
    assert_eq!(
        encoded["lifecycleState"],
        serde_json::json!("running"),
        "the §11.2 state keeps its documented snake_case spelling"
    );
    assert!(
        encoded.get("parentSeriesRunId").is_none(),
        "an absent parent is omitted, not encoded as null: D-03 forbids reading \
         missing data as an empty rebuild, so the two must stay distinguishable"
    );
    assert_eq!(
        serde_json::from_value::<SeriesRunProjection>(encoded.clone()).expect("round trip"),
        run
    );

    let mut broken = encoded;
    broken["retryOf"] = serde_json::Value::Null;
    assert!(
        serde_json::from_value::<SeriesRunProjection>(broken).is_err(),
        "ordinal 2 with a null retryOf must fail on the wire exactly as the \
         constructor fails it"
    );

    let first = attempt(series_run(0x52), 1, None).expect("first attempt");
    let flat = serde_json::to_value(&first).expect("serialize");
    assert!(
        flat.get("retryOf").is_none(),
        "a first attempt omits retryOf entirely"
    );
    assert_eq!(
        serde_json::from_value::<SeriesRunProjection>(flat).expect("round trip"),
        first
    );
}
