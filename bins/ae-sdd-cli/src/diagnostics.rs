//! Reader for the daemon diagnostic tracks.
//!
//! The whole point of this side is to keep the answer small.  The tracks are
//! written cheaply and grow fast, so a query that returns the file is a query
//! that costs more than the finding is worth.  Filtering happens here, and the
//! aggregate formats exist so a reader can locate a problem before paying for
//! any individual line.
//!
//! Segments are read newest-first and scanning stops once the requested count is
//! satisfied, so a narrow query does not pay for the whole retained window.

use std::path::{Path, PathBuf};

use ae_sdd_contracts::diagnostics::{DIAGNOSTICS_DIR, DiagnosticRecord, DiagnosticTrack};

/// Which tracks a query reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackSelector {
    /// Hook invocations and answers.
    Trace,
    /// Node transitions and defects.
    Ops,
    /// Both tracks, merged chronologically.
    All,
}

impl std::str::FromStr for TrackSelector {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "trace" => Ok(Self::Trace),
            "ops" => Ok(Self::Ops),
            "all" => Ok(Self::All),
            other => Err(format!(
                "unknown track `{other}`; expected trace, ops or all"
            )),
        }
    }
}

/// How results are rendered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    /// One JSON record per line.
    Lines,
    /// Counts grouped by record kind and outcome.
    Count,
    /// Hook invocations that never recorded an answer.
    Gaps,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "lines" => Ok(Self::Lines),
            "count" => Ok(Self::Count),
            "gaps" => Ok(Self::Gaps),
            other => Err(format!(
                "unknown format `{other}`; expected lines, count or gaps"
            )),
        }
    }
}

/// Filters applied while scanning.
#[derive(Clone, Debug, Default)]
pub struct Query {
    /// Oldest timestamp to include, in epoch milliseconds.
    pub since_ms: Option<i64>,
    /// Exact turn identity.
    pub turn: Option<String>,
    /// Exact Hook event identity.
    pub hook: Option<String>,
    /// Substring match against the method or operation name.
    pub name: Option<String>,
    /// Restrict to records that failed.
    pub failed: bool,
    /// Maximum records to return.
    pub limit: usize,
}

/// Returns the segment paths for `track`, newest first.
fn segments(dir: &Path, track: DiagnosticTrack) -> Vec<PathBuf> {
    let mut paths = vec![dir.join(format!("{}.jsonl", track.stem()))];
    for index in 1..=track.retained_segments() {
        paths.push(dir.join(format!("{}.{index}.jsonl", track.stem())));
    }
    paths
}

/// Returns the timestamp a record carries.
fn timestamp_of(record: &DiagnosticRecord) -> i64 {
    match record {
        DiagnosticRecord::HookIn(inner) => inner.ts,
        DiagnosticRecord::HookOut(inner) => inner.ts,
        DiagnosticRecord::Node(inner) => inner.ts,
        DiagnosticRecord::Bug(inner) => inner.ts,
        DiagnosticRecord::BugRepeat(inner) => inner.ts,
        DiagnosticRecord::Dropped(inner) => inner.ts,
    }
}

/// Returns the method or operation name a record carries.
fn name_of(record: &DiagnosticRecord) -> Option<&str> {
    match record {
        DiagnosticRecord::HookIn(inner) => Some(inner.m.as_str()),
        DiagnosticRecord::Node(inner) => Some(inner.op.as_str()),
        DiagnosticRecord::Bug(inner) => Some(inner.site.as_str()),
        DiagnosticRecord::HookOut(_)
        | DiagnosticRecord::BugRepeat(_)
        | DiagnosticRecord::Dropped(_) => None,
    }
}

/// Returns the Hook event identity a record carries.
fn hook_of(record: &DiagnosticRecord) -> Option<&str> {
    match record {
        DiagnosticRecord::HookIn(inner) => Some(inner.hid.as_str()),
        DiagnosticRecord::HookOut(inner) => Some(inner.hid.as_str()),
        DiagnosticRecord::Node(inner) => inner.hid.as_deref(),
        DiagnosticRecord::Bug(inner) => inner.hid.as_deref(),
        DiagnosticRecord::BugRepeat(_) | DiagnosticRecord::Dropped(_) => None,
    }
}

/// Reports whether a record represents a failure.
///
/// Defects are failures by construction; the other kinds carry an explicit flag.
fn failed(record: &DiagnosticRecord) -> bool {
    match record {
        DiagnosticRecord::HookOut(inner) => !inner.ok,
        DiagnosticRecord::Node(inner) => !inner.ok,
        DiagnosticRecord::Bug(_) | DiagnosticRecord::BugRepeat(_) => true,
        DiagnosticRecord::HookIn(_) | DiagnosticRecord::Dropped(_) => false,
    }
}

/// Reports whether `record` satisfies every active filter.
fn matches(record: &DiagnosticRecord, query: &Query) -> bool {
    if let Some(since) = query.since_ms
        && timestamp_of(record) < since
    {
        return false;
    }
    if let Some(turn) = query.turn.as_deref()
        && record.turn_id() != Some(turn)
    {
        return false;
    }
    if let Some(hook) = query.hook.as_deref()
        && hook_of(record) != Some(hook)
    {
        return false;
    }
    if let Some(name) = query.name.as_deref()
        && !name_of(record).is_some_and(|candidate| candidate.contains(name))
    {
        return false;
    }
    if query.failed && !failed(record) {
        return false;
    }
    true
}

/// Collects matching records from `track`, newest first, stopping at the limit.
///
/// Lines that fail to decode are skipped rather than reported.  A crash or an
/// abort can leave a partial final line, and one torn line is not worth failing
/// a query over.
fn collect(dir: &Path, track: DiagnosticTrack, query: &Query) -> Vec<DiagnosticRecord> {
    let mut collected = Vec::new();
    for path in segments(dir, track) {
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in contents.lines().rev() {
            if collected.len() >= query.limit {
                return collected;
            }
            if let Ok(record) = serde_json::from_str::<DiagnosticRecord>(line)
                && matches(&record, query)
            {
                collected.push(record);
            }
        }
    }
    collected
}

/// Runs one query and prints the result.
pub fn run(
    state_dir: &Path,
    selector: TrackSelector,
    format: OutputFormat,
    query: &Query,
) -> Result<(), String> {
    let dir = state_dir.join(DIAGNOSTICS_DIR);
    // Aggregate and gap formats must see the whole window, not the newest N
    // records, or the summary would describe an arbitrary slice of it.
    let scan = match format {
        OutputFormat::Lines => query.clone(),
        OutputFormat::Count | OutputFormat::Gaps => Query {
            limit: usize::MAX,
            ..query.clone()
        },
    };
    let mut records = Vec::new();
    if selector != TrackSelector::Ops {
        records.extend(collect(&dir, DiagnosticTrack::Trace, &scan));
    }
    if selector != TrackSelector::Trace {
        records.extend(collect(&dir, DiagnosticTrack::Ops, &scan));
    }
    records.sort_by_key(timestamp_of);
    match format {
        OutputFormat::Lines => print_lines(&records, query.limit),
        OutputFormat::Count => print_counts(&records),
        OutputFormat::Gaps => print_gaps(&records),
    }
    Ok(())
}

/// Prints the newest `limit` records in chronological order.
fn print_lines(records: &[DiagnosticRecord], limit: usize) {
    let start = records.len().saturating_sub(limit);
    for record in &records[start..] {
        if let Ok(line) = serde_json::to_string(record) {
            println!("{line}");
        }
    }
}

/// Prints counts by record kind, then by name for the kinds that carry one.
///
/// This is the format to reach for first: it says whether there is a problem and
/// roughly where, for a fixed handful of output lines regardless of window size.
fn print_counts(records: &[DiagnosticRecord]) {
    let mut kinds: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
    let mut failures: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    let mut repeats = 0_u64;
    for record in records {
        let kind = match record {
            DiagnosticRecord::HookIn(_) => "hook_in",
            DiagnosticRecord::HookOut(_) => "hook_out",
            DiagnosticRecord::Node(_) => "node",
            DiagnosticRecord::Bug(_) => "bug",
            DiagnosticRecord::BugRepeat(inner) => {
                repeats = repeats.saturating_add(inner.n);
                "bug_repeat"
            }
            DiagnosticRecord::Dropped(_) => "dropped",
        };
        *kinds.entry(kind).or_default() += 1;
        if failed(record)
            && let Some(name) = name_of(record)
        {
            *failures.entry(name.to_owned()).or_default() += 1;
        }
    }
    for (kind, count) in &kinds {
        println!("{kind} {count}");
    }
    if repeats > 0 {
        println!("bug_repeat_total {repeats}");
    }
    for (name, count) in &failures {
        println!("failed:{name} {count}");
    }
}

/// Prints Hook invocations with no recorded answer.
///
/// An unanswered invocation means the daemon never returned — it could not have
/// written its own defect record, so this pairing is the only way the event
/// surfaces at all.
fn print_gaps(records: &[DiagnosticRecord]) {
    let gaps = unanswered(records);
    if gaps.is_empty() {
        println!("no unanswered hook invocations in the scanned window");
        return;
    }
    for record in gaps {
        if let Ok(line) = serde_json::to_string(record) {
            println!("{line}");
        }
    }
}

/// Returns the Hook invocations in `records` that have no matching answer.
fn unanswered(records: &[DiagnosticRecord]) -> Vec<&DiagnosticRecord> {
    let answered: std::collections::BTreeSet<&str> = records
        .iter()
        .filter_map(|record| match record {
            DiagnosticRecord::HookOut(inner) => Some(inner.hid.as_str()),
            _ => None,
        })
        .collect();
    records
        .iter()
        .filter(|record| match record {
            DiagnosticRecord::HookIn(inner) => !answered.contains(inner.hid.as_str()),
            _ => false,
        })
        .collect()
}

/// Parses a `30m` / `2h` / `7d` / `45s` window into milliseconds.
pub fn parse_window(value: &str) -> Result<i64, String> {
    let (digits, multiplier) = match value.chars().last() {
        Some('s') => (&value[..value.len() - 1], 1_000),
        Some('m') => (&value[..value.len() - 1], 60 * 1_000),
        Some('h') => (&value[..value.len() - 1], 60 * 60 * 1_000),
        Some('d') => (&value[..value.len() - 1], 24 * 60 * 60 * 1_000),
        _ => (value, 1_000),
    };
    digits
        .parse::<i64>()
        .map_err(|_| format!("could not read `{value}` as a duration such as 30m, 2h or 7d"))?
        .checked_mul(multiplier)
        .ok_or_else(|| format!("duration `{value}` is too large"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ae_sdd_contracts::diagnostics::{
        BugKind, BugRecord, HookInRecord, HookOutRecord, NodeRecord,
    };

    fn hook_in(hid: &str, turn: &str, method: &str, ts: i64) -> DiagnosticRecord {
        DiagnosticRecord::HookIn(HookInRecord {
            ts,
            hid: hid.to_owned(),
            wsid: "ws-1".to_owned(),
            sid: "s-1".to_owned(),
            tid: turn.to_owned(),
            wid: None,
            m: method.to_owned(),
            cls: None,
            seq: 1,
        })
    }

    fn hook_out(hid: &str, turn: &str, ok: bool, ts: i64) -> DiagnosticRecord {
        DiagnosticRecord::HookOut(HookOutRecord {
            ts,
            hid: hid.to_owned(),
            tid: turn.to_owned(),
            dec: "allow".to_owned(),
            dir: None,
            rc: None,
            ctx: None,
            cdg: None,
            es: 1,
            rp: false,
            ok,
            err: (!ok).then(|| "GateTimeout".to_owned()),
            ms: 2,
        })
    }

    fn node(operation: &str, turn: &str, ok: bool) -> DiagnosticRecord {
        DiagnosticRecord::Node(NodeRecord {
            ts: 100,
            op: operation.to_owned(),
            wsid: "ws-1".to_owned(),
            wid: None,
            to: None,
            sid: None,
            tid: Some(turn.to_owned()),
            hid: None,
            rev: None,
            es: None,
            actor: None,
            reason: None,
            conf: None,
            ok,
            err: None,
            ms: 1,
        })
    }

    #[test]
    fn a_turn_filter_selects_every_kind_that_belongs_to_that_turn() {
        let records = [
            hook_in("h-1", "t-1", "hook.preTool", 10),
            hook_out("h-1", "t-1", true, 11),
            node("state.transition", "t-2", true),
        ];
        let query = Query {
            turn: Some("t-1".to_owned()),
            limit: 10,
            ..Query::default()
        };
        let selected = records
            .iter()
            .filter(|record| matches(record, &query))
            .count();
        assert_eq!(
            selected, 2,
            "the turn axis joins the Hook invocation to its answer"
        );
    }

    #[test]
    fn the_failed_filter_keeps_defects_and_unsuccessful_records_only() {
        let bug = DiagnosticRecord::Bug(BugRecord {
            ts: 5,
            fp: "abc".to_owned(),
            kind: BugKind::Panic,
            site: "src/main.rs:1".to_owned(),
            msg: "boom".to_owned(),
            chain: Vec::new(),
            sid: None,
            tid: None,
            hid: None,
        });
        let query = Query {
            failed: true,
            limit: 10,
            ..Query::default()
        };
        assert!(matches(&bug, &query), "a defect is a failure");
        assert!(
            matches(&hook_out("h-2", "t-1", false, 12), &query),
            "an unsuccessful answer is a failure"
        );
        assert!(
            !matches(&hook_out("h-3", "t-1", true, 13), &query),
            "a successful answer is not"
        );
        assert!(
            !matches(&node("state.transition", "t-1", true), &query),
            "a successful transition is not"
        );
    }

    #[test]
    fn the_since_filter_excludes_records_older_than_the_window() {
        let query = Query {
            since_ms: Some(50),
            limit: 10,
            ..Query::default()
        };
        assert!(!matches(&hook_in("h-1", "t-1", "hook.stop", 49), &query));
        assert!(matches(&hook_in("h-2", "t-1", "hook.stop", 50), &query));
    }

    #[test]
    fn an_invocation_without_an_answer_is_reported_as_a_gap() {
        let records = [
            hook_in("h-1", "t-1", "hook.preTool", 10),
            hook_out("h-1", "t-1", true, 11),
            hook_in("h-2", "t-1", "hook.postTool", 12),
        ];
        let gaps = unanswered(&records);
        assert_eq!(gaps.len(), 1, "only the unpaired invocation is a gap");
        assert!(
            matches!(gaps.first(), Some(DiagnosticRecord::HookIn(inner)) if inner.hid == "h-2"),
            "the gap names the invocation the daemon never answered"
        );
    }

    #[test]
    fn windows_parse_in_seconds_minutes_hours_and_days() {
        assert_eq!(parse_window("45s"), Ok(45_000));
        assert_eq!(parse_window("30m"), Ok(1_800_000));
        assert_eq!(parse_window("2h"), Ok(7_200_000));
        assert_eq!(parse_window("7d"), Ok(604_800_000));
        assert_eq!(
            parse_window("90"),
            Ok(90_000),
            "a bare number reads as seconds"
        );
        assert!(parse_window("soon").is_err());
    }
}
