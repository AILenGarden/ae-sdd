//! Rotating JSONL sink for the daemon diagnostic tracks.
//!
//! The sink is process-global and starts disabled.  Every emit is a no-op until
//! [`init`] runs, which keeps the many `RuntimeService` test constructors free of
//! logging setup and means a test never writes stray files.
//!
//! Retention is expressed purely in bytes: a segment that fills rotates and the
//! oldest is dropped.  There is no expiry timer and no background sweeper — the
//! window self-adjusts to traffic, which is what a daemon optimization pass
//! actually asks of it, and it adds no new failure surface.
//!
//! Nothing on this path may fail the daemon.  A dead writer thread, a full disk
//! or a saturated queue degrades to lost lines, never to a returned error.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ae_sdd_contracts::diagnostics::{
    BugKind, BugRecord, BugRepeatRecord, DiagnosticRecord, DiagnosticTrack, DroppedRecord,
};
use sha2::{Digest, Sha256};

/// Queue depth before trace emits start shedding.
const QUEUE_DEPTH: usize = 4_096;

/// How often the writer flushes accumulated defect repeat counts.
const REPEAT_FLUSH: Duration = Duration::from_secs(60);

/// Distinct defect fingerprints tracked before the dedup table is reset.
///
/// Normalization only collapses digit runs, so a message carrying a varying path
/// or identifier still yields a fresh fingerprint each time.  Without a ceiling
/// the table would grow for the lifetime of the daemon; resetting costs nothing
/// worse than one extra full record per fingerprint still in flight.
const MAX_TRACKED_DEFECTS: usize = 512;

/// Global sink, absent until [`init`] installs one.
static SINK: OnceLock<Sink> = OnceLock::new();

/// Trace lines dropped since the writer last reported a loss.
static DROPPED: AtomicU64 = AtomicU64::new(0);

/// Message accepted by the writer thread.
enum Message {
    /// Persist one record.
    Record(Box<DiagnosticRecord>),
    /// Drain everything queued, then acknowledge.
    Flush(SyncSender<()>),
}

/// Handle to the writer thread.
struct Sink {
    sender: SyncSender<Message>,
}

/// Returns milliseconds since the Unix epoch, saturating at zero.
///
/// A clock before the epoch is not worth a typed error on a logging path.
#[must_use]
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

/// Returns milliseconds elapsed since `started`, saturating.
#[must_use]
pub fn elapsed_ms(started: std::time::Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Starts the diagnostic writer under `dir`.
///
/// Repeat calls are ignored, so a second daemon lifecycle inside one process
/// cannot install a competing writer.  The directory is expected to sit inside
/// the protected daemon state directory; the sink relies on that protection
/// rather than setting its own mode, matching the existing daemon log.
pub fn init(dir: PathBuf) {
    let _ = SINK.get_or_init(|| {
        let (sender, receiver) = sync_channel::<Message>(QUEUE_DEPTH);
        let spawned = std::thread::Builder::new()
            .name("ae-sdd-diagnostics".to_owned())
            .spawn(move || run_writer(&dir, &receiver));
        if spawned.is_err() {
            // Keep the sink installed anyway: sends will fail and every emit
            // degrades to a no-op, which is the intended failure mode.
        }
        Sink { sender }
    });
}

/// Persists one record, or drops it if that would block or the sink is absent.
///
/// The [`DiagnosticTrack::Ops`] track blocks rather than sheds: node
/// transitions and defects are low volume and are the records a later
/// investigation depends on, so paying a bounded wait beats losing them.  The
/// trace track sheds instead, because a Hook burst must never slow the RPC path
/// it is describing.
pub fn emit(record: DiagnosticRecord) {
    let Some(sink) = SINK.get() else {
        return;
    };
    let track = record.track();
    let message = Message::Record(Box::new(record));
    match track {
        DiagnosticTrack::Ops => {
            let _ = sink.sender.send(message);
        }
        DiagnosticTrack::Trace => {
            if sink.sender.try_send(message).is_err() {
                DROPPED.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// Blocks until the writer has drained, bounded by `timeout`.
///
/// Used on daemon stop and on the panic path, where the process may be about to
/// disappear and an unflushed queue would silently lose exactly the record that
/// explains why.
pub fn flush(timeout: Duration) {
    let Some(sink) = SINK.get() else {
        return;
    };
    let (ack, wait) = sync_channel::<()>(1);
    if sink.sender.send(Message::Flush(ack)).is_ok() {
        let _ = wait.recv_timeout(timeout);
    }
}

/// Records a defect, computing its dedup fingerprint.
///
/// `site` should be a `file:line` literal from the call site.  The message is
/// normalized before fingerprinting so the same defect carrying different
/// indices or identifiers still collapses to one fingerprint.
pub fn emit_bug(kind: BugKind, site: &str, message: &str, chain: Vec<String>, ids: BugIds<'_>) {
    let normalized = normalize(message);
    let mut hasher = Sha256::new();
    hasher.update(format!("{kind:?}").as_bytes());
    hasher.update(b"\0");
    hasher.update(site.as_bytes());
    hasher.update(b"\0");
    hasher.update(normalized.as_bytes());
    let fingerprint = hex::encode(&hasher.finalize()[..6]);
    emit(DiagnosticRecord::Bug(BugRecord {
        ts: now_ms(),
        fp: fingerprint,
        kind,
        site: site.to_owned(),
        msg: normalized,
        chain,
        sid: ids.session_id.map(str::to_owned),
        tid: ids.turn_id.map(str::to_owned),
        hid: ids.hook_event_id.map(str::to_owned),
    }));
}

/// Optional correlation identities attached to a defect.
#[derive(Clone, Copy, Debug, Default)]
pub struct BugIds<'a> {
    /// Session the defect was attributable to.
    pub session_id: Option<&'a str>,
    /// Turn the defect was attributable to.
    pub turn_id: Option<&'a str>,
    /// Hook event the defect was attributable to.
    pub hook_event_id: Option<&'a str>,
}

/// Live segment for one track.
struct TrackState {
    /// Open live segment, absent while a rotation or open is failing.
    file: Option<File>,
    /// Bytes written to the live segment.
    bytes: u64,
}

impl TrackState {
    /// Opens the live segment for `track`, tolerating failure.
    fn open(dir: &Path, track: DiagnosticTrack) -> Self {
        let path = live_path(dir, track);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok();
        let bytes = file
            .as_ref()
            .and_then(|handle| handle.metadata().ok())
            .map_or(0, |metadata| metadata.len());
        Self { file, bytes }
    }
}

/// Returns the live segment path for `track`.
fn live_path(dir: &Path, track: DiagnosticTrack) -> PathBuf {
    dir.join(format!("{}.jsonl", track.stem()))
}

/// Returns the rotated segment path for `track` at depth `index`.
fn rotated_path(dir: &Path, track: DiagnosticTrack, index: u32) -> PathBuf {
    dir.join(format!("{}.{index}.jsonl", track.stem()))
}

/// Rotates `track`, discarding the oldest retained segment.
///
/// The handle is closed before any rename.  On Windows a file opened without
/// `FILE_SHARE_DELETE` cannot be renamed while a handle is held, so renaming
/// first would leave rotation silently failing and the segment growing without
/// bound — a symptom that reads nothing like its cause.
fn rotate(dir: &Path, track: DiagnosticTrack, state: &mut TrackState) {
    state.file = None;
    let retained = track.retained_segments();
    let _ = std::fs::remove_file(rotated_path(dir, track, retained));
    for index in (1..retained).rev() {
        let _ = std::fs::rename(
            rotated_path(dir, track, index),
            rotated_path(dir, track, index + 1),
        );
    }
    let _ = std::fs::rename(live_path(dir, track), rotated_path(dir, track, 1));
    *state = TrackState::open(dir, track);
}

/// Appends one serialized line to `track`, rotating first when it would overrun.
fn append(dir: &Path, track: DiagnosticTrack, state: &mut TrackState, line: &[u8]) {
    if state.bytes.saturating_add(line.len() as u64) > track.max_segment_bytes() && state.bytes > 0
    {
        rotate(dir, track, state);
    }
    let Some(handle) = state.file.as_mut() else {
        return;
    };
    if handle.write_all(line).is_ok() {
        state.bytes = state.bytes.saturating_add(line.len() as u64);
        let _ = handle.flush();
    } else {
        // A failed write leaves the byte count untrustworthy; reopen so the
        // next line either lands or is dropped cleanly.
        *state = TrackState::open(dir, track);
    }
}

/// Serializes `record` into one JSONL line.
///
/// Serialization of an owned record should not fail; if it does, that is itself
/// a defect, and it is reported through the same channel rather than swallowed.
fn encode(record: &DiagnosticRecord) -> Option<Vec<u8>> {
    let mut line = serde_json::to_vec(record).ok()?;
    line.push(b'\n');
    Some(line)
}

/// Owns both track files and serializes every write.
///
/// Defect dedup lives here rather than at the call sites: only the writer sees
/// the whole stream, and a call site inside a loop has no way to know it is
/// repeating itself.
fn run_writer(dir: &Path, receiver: &Receiver<Message>) {
    let _ = std::fs::create_dir_all(dir);
    let mut trace = TrackState::open(dir, DiagnosticTrack::Trace);
    let mut ops = TrackState::open(dir, DiagnosticTrack::Ops);
    let mut seen: HashMap<String, u64> = HashMap::new();
    loop {
        match receiver.recv_timeout(REPEAT_FLUSH) {
            Ok(Message::Record(record)) => {
                report_drops(dir, &mut ops);
                if let DiagnosticRecord::Bug(bug) = record.as_ref()
                    && let Some(count) = seen.get_mut(&bug.fp)
                {
                    *count = count.saturating_add(1);
                    continue;
                }
                if let DiagnosticRecord::Bug(bug) = record.as_ref() {
                    if seen.len() >= MAX_TRACKED_DEFECTS {
                        // Outstanding counts are written before the table is
                        // dropped, so resetting loses no sightings — only the
                        // memory that these fingerprints were already reported.
                        flush_repeats(dir, &mut ops, &mut seen);
                        seen.clear();
                    }
                    seen.insert(bug.fp.clone(), 0);
                }
                let track = record.track();
                if let Some(line) = encode(&record) {
                    match track {
                        DiagnosticTrack::Trace => append(dir, track, &mut trace, &line),
                        DiagnosticTrack::Ops => append(dir, track, &mut ops, &line),
                    }
                }
            }
            Ok(Message::Flush(ack)) => {
                flush_repeats(dir, &mut ops, &mut seen);
                report_drops(dir, &mut ops);
                let _ = ack.send(());
            }
            Err(RecvTimeoutError::Timeout) => {
                flush_repeats(dir, &mut ops, &mut seen);
                report_drops(dir, &mut ops);
            }
            Err(RecvTimeoutError::Disconnected) => {
                flush_repeats(dir, &mut ops, &mut seen);
                report_drops(dir, &mut ops);
                return;
            }
        }
    }
}

/// Writes one repeat line per fingerprint that accumulated sightings.
fn flush_repeats(dir: &Path, ops: &mut TrackState, seen: &mut HashMap<String, u64>) {
    for (fingerprint, count) in seen.iter_mut() {
        if *count == 0 {
            continue;
        }
        let record = DiagnosticRecord::BugRepeat(BugRepeatRecord {
            ts: now_ms(),
            fp: fingerprint.clone(),
            n: *count,
        });
        if let Some(line) = encode(&record) {
            append(dir, DiagnosticTrack::Ops, ops, &line);
        }
        *count = 0;
    }
}

/// Writes a loss line when trace emits have been shed.
fn report_drops(dir: &Path, ops: &mut TrackState) {
    let dropped = DROPPED.swap(0, Ordering::Relaxed);
    if dropped == 0 {
        return;
    }
    let record = DiagnosticRecord::Dropped(DroppedRecord {
        ts: now_ms(),
        n: dropped,
    });
    if let Some(line) = encode(&record) {
        append(dir, DiagnosticTrack::Ops, ops, &line);
    }
}

/// Collapses variable parts of a message so fingerprints stay stable.
///
/// Digit runs become `#`; without this, a defect inside a loop produces a fresh
/// fingerprint per iteration and dedup buys nothing.
fn normalize(message: &str) -> String {
    let mut output = String::with_capacity(message.len());
    let mut in_digits = false;
    for character in message.chars() {
        if character.is_ascii_digit() {
            if !in_digits {
                output.push('#');
                in_digits = true;
            }
        } else {
            in_digits = false;
            output.push(character);
        }
    }
    output.truncate(240);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use ae_sdd_contracts::diagnostics::NodeRecord;

    fn node_line(operation: &str) -> Vec<u8> {
        let record = DiagnosticRecord::Node(NodeRecord {
            ts: 1,
            op: operation.to_owned(),
            wsid: "ws".to_owned(),
            wid: None,
            to: None,
            sid: None,
            tid: None,
            hid: None,
            rev: None,
            es: None,
            actor: None,
            reason: None,
            conf: None,
            ok: true,
            err: None,
            ms: 0,
        });
        encode(&record).expect("a node record serializes")
    }

    #[test]
    fn rotation_keeps_the_retained_window_and_drops_the_oldest() {
        let directory = tempfile::tempdir().expect("a temp dir");
        let path = directory.path();
        let track = DiagnosticTrack::Ops;
        let mut state = TrackState::open(path, track);
        for generation in 0..=track.retained_segments() + 1 {
            let line = node_line(&format!("generation.{generation}"));
            state.bytes = track.max_segment_bytes();
            append(path, track, &mut state, &line);
        }
        assert!(live_path(path, track).exists(), "the live segment exists");
        for index in 1..=track.retained_segments() {
            assert!(
                rotated_path(path, track, index).exists(),
                "retained segment {index} exists"
            );
        }
        assert!(
            !rotated_path(path, track, track.retained_segments() + 1).exists(),
            "no segment is kept beyond the retained window"
        );
    }

    #[test]
    fn a_full_segment_rotates_before_the_line_that_would_overrun_it() {
        let directory = tempfile::tempdir().expect("a temp dir");
        let path = directory.path();
        let track = DiagnosticTrack::Ops;
        let mut state = TrackState::open(path, track);
        let line = node_line("first");
        append(path, track, &mut state, &line);
        let written = state.bytes;
        assert!(written > 0, "the first line lands in the live segment");
        assert!(
            !rotated_path(path, track, 1).exists(),
            "a segment under the ceiling does not rotate"
        );
        state.bytes = track.max_segment_bytes();
        append(path, track, &mut state, &line);
        assert!(
            rotated_path(path, track, 1).exists(),
            "crossing the ceiling rotates rather than truncating"
        );
        assert_eq!(
            state.bytes, written,
            "the new live segment holds only the line that triggered rotation"
        );
    }

    #[test]
    fn normalization_collapses_digits_so_a_looping_defect_shares_one_fingerprint() {
        assert_eq!(normalize("index 47 out of bounds"), "index # out of bounds");
        assert_eq!(
            normalize("index 1024 out of bounds"),
            normalize("index 7 out of bounds"),
            "the same defect at different indices normalizes identically"
        );
        assert_ne!(
            normalize("index out of bounds"),
            normalize("lease expired"),
            "distinct messages stay distinct"
        );
    }

    #[test]
    fn emitting_without_an_initialized_sink_is_inert() {
        // The global sink is absent in a unit test, which is what keeps the many
        // service constructors free of logging setup; this asserts that contract
        // rather than leaving it to chance.
        emit(DiagnosticRecord::Node(NodeRecord {
            ts: 0,
            op: "state.transition".to_owned(),
            wsid: String::new(),
            wid: None,
            to: None,
            sid: None,
            tid: None,
            hid: None,
            rev: None,
            es: None,
            actor: None,
            reason: None,
            conf: None,
            ok: true,
            err: None,
            ms: 0,
        }));
        flush(Duration::from_millis(1));
    }
}
