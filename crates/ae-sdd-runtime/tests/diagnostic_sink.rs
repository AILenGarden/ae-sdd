//! End-to-end behaviour of the diagnostic writer thread.
//!
//! The sink is process-global and installs once, so this file owns the single
//! initialization and every assertion here shares it.  Dedup and drop reporting
//! only exist inside the writer thread, so they cannot be reached from a unit
//! test that never starts one.

use std::time::Duration;

use ae_sdd_contracts::diagnostics::{
    BugKind, DiagnosticRecord, DiagnosticTrack, HookInRecord, NodeRecord,
};
use ae_sdd_runtime::diagnostics;

/// Reads every decodable record from one track's live segment.
fn read_live(directory: &std::path::Path, track: DiagnosticTrack) -> Vec<DiagnosticRecord> {
    let path = directory.join(format!("{}.jsonl", track.stem()));
    let contents = std::fs::read_to_string(path).unwrap_or_default();
    contents
        .lines()
        .filter_map(|line| serde_json::from_str::<DiagnosticRecord>(line).ok())
        .collect()
}

fn node(operation: &str) -> DiagnosticRecord {
    DiagnosticRecord::Node(NodeRecord {
        ts: diagnostics::now_ms(),
        op: operation.to_owned(),
        wsid: "ws-1".to_owned(),
        wid: Some("WI-1".to_owned()),
        to: Some("TEST".to_owned()),
        sid: Some("s-1".to_owned()),
        tid: Some("t-1".to_owned()),
        hid: None,
        rev: Some(7),
        es: Some(11),
        actor: Some("cap-1".to_owned()),
        reason: None,
        conf: None,
        ok: true,
        err: None,
        ms: 3,
    })
}

#[test]
fn the_sink_routes_records_by_track_and_collapses_repeated_defects() {
    let directory = tempfile::tempdir().expect("a temp dir");
    let path = directory.path().join("logs");
    diagnostics::init(path.clone());

    diagnostics::emit(DiagnosticRecord::HookIn(HookInRecord {
        ts: diagnostics::now_ms(),
        hid: "h-1".to_owned(),
        wsid: "ws-1".to_owned(),
        sid: "s-1".to_owned(),
        tid: "t-1".to_owned(),
        wid: None,
        m: "hook.preTool".to_owned(),
        cls: Some("patch".to_owned()),
        seq: 1,
    }));
    diagnostics::emit(node("state.transition"));
    for _ in 0..5 {
        diagnostics::emit_bug(
            BugKind::Invariant,
            "crates/ae-sdd-runtime/tests/diagnostic_sink.rs",
            "slice 3 did not hold",
            Vec::new(),
            diagnostics::BugIds::default(),
        );
    }
    diagnostics::flush(Duration::from_secs(5));

    let trace = read_live(&path, DiagnosticTrack::Trace);
    assert_eq!(trace.len(), 1, "the Hook record goes to the trace track");
    assert!(
        matches!(trace.first(), Some(DiagnosticRecord::HookIn(_))),
        "the trace track holds the Hook invocation"
    );

    let ops = read_live(&path, DiagnosticTrack::Ops);
    let bugs: Vec<_> = ops
        .iter()
        .filter_map(|record| match record {
            DiagnosticRecord::Bug(bug) => Some(bug),
            _ => None,
        })
        .collect();
    assert_eq!(
        bugs.len(),
        1,
        "five sightings of one defect write one full record"
    );
    let repeats: u64 = ops
        .iter()
        .filter_map(|record| match record {
            DiagnosticRecord::BugRepeat(repeat) => Some(repeat.n),
            _ => None,
        })
        .sum();
    assert_eq!(repeats, 4, "the remaining sightings survive as a count");
    assert_eq!(
        bugs.first().map(|bug| bug.msg.as_str()),
        Some("slice # did not hold"),
        "the persisted message is the normalized form used for dedup"
    );
    assert!(
        ops.iter()
            .any(|record| matches!(record, DiagnosticRecord::Node(_))),
        "the node transition goes to the ops track"
    );
}
