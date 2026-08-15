//! Daemon diagnostic trace vocabulary.
//!
//! These records exist to optimize the daemon: Hook invocations, the decisions
//! the daemon answered them with, task node transitions and defects.  They are
//! disposable diagnostics, not business truth — the authoritative carriers stay
//! `runtime_event`, `operation_receipt` and the project files.
//!
//! Field names are deliberately short.  The primary reader is an agent paying
//! per token, and the Hook tracks dominate volume, so every persisted key is
//! spent on something a query filters or a human reads.
//!
//! Nothing here may carry prompt text, transcripts, tool output bodies, tokens
//! or secrets; bounded identifiers, stable codes and digests only.

use serde::{Deserialize, Serialize};

/// Directory under the daemon state directory holding the diagnostic tracks.
///
/// Shared so the writing and reading sides cannot drift apart on where the
/// files live.
pub const DIAGNOSTICS_DIR: &str = "logs";

/// Rotating file family a record is routed to.
///
/// The split exists to stop eviction bleeding across value densities: Hook
/// traffic is the overwhelming majority of lines, so sharing one file would let
/// a burst of Hook activity evict the defect and node records that a daemon
/// optimization pass actually depends on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticTrack {
    /// Hook invocations paired with the daemon's answer.
    Trace,
    /// Task node transitions and defects.
    Ops,
}

/// One diagnostic line.
///
/// `t` discriminates the shape so a reader can decode a mixed file, and the
/// enum keeps the field set a compile-time contract instead of a convention
/// that drifts as call sites are added.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum DiagnosticRecord {
    /// A Hook invocation entered the daemon.
    HookIn(HookInRecord),
    /// The daemon answered a Hook invocation.
    HookOut(HookOutRecord),
    /// A task node transition was attempted.
    Node(NodeRecord),
    /// A defect was observed.
    Bug(BugRecord),
    /// Repeat count for a defect already reported in full.
    BugRepeat(BugRepeatRecord),
    /// Trace lines discarded under write backpressure.
    Dropped(DroppedRecord),
}

impl DiagnosticRecord {
    /// Returns the track this record is persisted to.
    #[must_use]
    pub const fn track(&self) -> DiagnosticTrack {
        match self {
            Self::HookIn(_) | Self::HookOut(_) => DiagnosticTrack::Trace,
            Self::Node(_) | Self::Bug(_) | Self::BugRepeat(_) | Self::Dropped(_) => {
                DiagnosticTrack::Ops
            }
        }
    }

    /// Returns the turn this record belongs to, when it carries one.
    ///
    /// `turn_id` is the axis that joins the four record kinds into one causal
    /// story, so the reader exposes it independently of shape.
    #[must_use]
    pub fn turn_id(&self) -> Option<&str> {
        match self {
            Self::HookIn(record) => Some(record.tid.as_str()),
            Self::HookOut(record) => Some(record.tid.as_str()),
            Self::Node(record) => record.tid.as_deref(),
            Self::Bug(record) => record.tid.as_deref(),
            Self::BugRepeat(_) | Self::Dropped(_) => None,
        }
    }
}

impl DiagnosticTrack {
    /// Returns the file stem used for this track's rotating segments.
    #[must_use]
    pub const fn stem(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Ops => "ops",
        }
    }

    /// Returns the byte ceiling one segment may reach before rotation.
    #[must_use]
    pub const fn max_segment_bytes(self) -> u64 {
        match self {
            Self::Trace => 4 * 1024 * 1024,
            Self::Ops => 2 * 1024 * 1024,
        }
    }

    /// Returns how many rotated segments are kept behind the live file.
    #[must_use]
    pub const fn retained_segments(self) -> u32 {
        match self {
            Self::Trace => 3,
            Self::Ops => 2,
        }
    }
}

/// A Hook invocation as it entered the daemon.
///
/// Written before the daemon does the work, so a crash or a deadline overrun
/// leaves this line without its [`HookOutRecord`].  That gap is the intended
/// signal: it is the only cheap evidence of a Hook the daemon never answered.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HookInRecord {
    /// Milliseconds since the Unix epoch.
    pub ts: i64,
    /// Session-unique Hook event identity; pairs this line with its answer.
    pub hid: String,
    /// Workspace identity.
    pub wsid: String,
    /// Session identity.
    pub sid: String,
    /// Turn identity.
    pub tid: String,
    /// Work item identity; absent for a Hook not bound to one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wid: Option<String>,
    /// RPC method name.
    pub m: String,
    /// Host-reported execution tool class, when the event carried one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cls: Option<String>,
    /// Monotonic turn sequence.
    pub seq: u64,
}

/// What the daemon answered one Hook invocation with.
///
/// This is the record of the daemon's own output: the three fields that
/// actually steer the calling agent are the Hook decision, the execution
/// directive and whether a context projection was delivered.  Projection bodies
/// never appear — `cdg` carries the digest, which is enough to tell whether two
/// deliveries were the same content.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HookOutRecord {
    /// Milliseconds since the Unix epoch.
    pub ts: i64,
    /// Hook event identity of the invocation being answered.
    pub hid: String,
    /// Turn identity, repeated so a turn query needs no join.
    pub tid: String,
    /// Hook decision (`allow`/`deny`/`block`/`context`).
    pub dec: String,
    /// Execution directive decision, when one was attached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    /// Stable directive reason code, when one was attached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rc: Option<String>,
    /// Context delivery kind (`full`/`no_change`), when context was decided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctx: Option<String>,
    /// Context projection digest, when one was decided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cdg: Option<String>,
    /// Durable event sequence the answer committed at.
    pub es: u64,
    /// True when the answer was replayed from an existing receipt.
    #[serde(default, skip_serializing_if = "core::ops::Not::not")]
    pub rp: bool,
    /// Whether the invocation succeeded.
    pub ok: bool,
    /// Stable error code when `ok` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub err: Option<String>,
    /// Wall-clock duration of the invocation.
    pub ms: u64,
}

/// A task node transition attempt.
///
/// Only operations that move a work item through the flow are recorded; reads
/// and queries are not nodes.  High-privilege operations additionally carry
/// `actor`, `reason` and `conf` because `constraints/security.md` requires the
/// authorizing evidence to be recoverable alongside the change.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct NodeRecord {
    /// Milliseconds since the Unix epoch.
    pub ts: i64,
    /// Operation name, for example `state.transition`.
    pub op: String,
    /// Workspace identity.
    pub wsid: String,
    /// Work item identity, when the operation is scoped to one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wid: Option<String>,
    /// Target phase, for operations that name one.
    ///
    /// Only the destination is recorded.  The origin phase would cost a state
    /// read on every transition, and the previous node line for the same work
    /// item already supplies it when the file is read in order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Session identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>,
    /// Turn identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tid: Option<String>,
    /// Hook event identity, when the transition happened inside a Hook.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hid: Option<String>,
    /// Work item revision after the transition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<u64>,
    /// Durable event sequence the transition committed at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub es: Option<u64>,
    /// Capability or actor that authorized the operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    /// Stated reason, required for high-privilege operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Confirmation identity, required for high-privilege operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conf: Option<String>,
    /// Whether the transition succeeded.
    pub ok: bool,
    /// Stable error code when `ok` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub err: Option<String>,
    /// Wall-clock duration of the operation.
    pub ms: u64,
}

/// Class of defect, kept narrow on purpose.
///
/// Policy denials — forbidden roles, blocker Gate failures, lease conflicts —
/// are the daemon working as designed and are deliberately absent.  Recording
/// them here would bury the real defects under expected traffic, which is the
/// failure mode this enum exists to prevent.  They remain queryable through the
/// `ok: false` lines on the other records.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BugKind {
    /// A thread or task panicked.
    Panic,
    /// An internal invariant did not hold.
    Invariant,
    /// A background worker stopped or failed unexpectedly.
    Worker,
    /// Persistence integrity or migration failed.
    Store,
    /// Serialization of a value the daemon owns failed.
    Encode,
}

/// A defect observed by the daemon, reported in full.
///
/// The first sighting of a fingerprint carries the detail; later sightings
/// collapse into [`BugRepeatRecord`] so a defect inside a loop cannot flood the
/// file and drown everything else in it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BugRecord {
    /// Milliseconds since the Unix epoch.
    pub ts: i64,
    /// Fingerprint over kind, site and normalized message.
    pub fp: String,
    /// Defect class.
    pub kind: BugKind,
    /// Source site, as `file:line`.
    pub site: String,
    /// Normalized message with variable parts elided.
    pub msg: String,
    /// Error source chain, outermost first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chain: Vec<String>,
    /// Session identity, when the defect was attributable to one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>,
    /// Turn identity, when the defect was attributable to one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tid: Option<String>,
    /// Hook event identity, when the defect was attributable to one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hid: Option<String>,
}

/// Repeat count for a defect whose full detail was already written.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BugRepeatRecord {
    /// Milliseconds since the Unix epoch.
    pub ts: i64,
    /// Fingerprint of the already-reported defect.
    pub fp: String,
    /// Sightings collapsed into this line.
    pub n: u64,
}

/// Trace lines discarded because the write queue was saturated.
///
/// Silent loss would make the trace file lie by omission, so the count is
/// persisted: how much was lost is itself a finding worth keeping.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DroppedRecord {
    /// Milliseconds since the Unix epoch.
    pub ts: i64,
    /// Lines discarded since the previous report.
    pub n: u64,
}
