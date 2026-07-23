use ae_sdd_protocol::{HookDecision, WorkspaceMode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Daemon request admission lifecycle.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonLifecycle {
    /// New sessions and work are admitted.
    Running,
    /// New work is rejected while admitted requests finish.
    Draining,
    /// Stop was requested and the listener should exit.
    Stopping,
}

/// Runtime health and capacity projection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    /// Current daemon lifecycle.
    pub lifecycle: DaemonLifecycle,
    /// Boot identity.
    pub boot_id: String,
    /// Durable event-store epoch.
    pub event_store_id: String,
    /// Latest global event sequence.
    pub event_seq: u64,
    /// Registered workspace count.
    pub workspace_count: usize,
    /// Active session count.
    pub session_count: usize,
    /// Current policy digest.
    pub policy_digest: String,
}

/// Stable wire representation of an Agent role.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WireAgentRole {
    /// Root coordinator.
    Root,
    /// Series coordinator.
    Series,
    /// Scoped implementation task.
    Task,
    /// Independent reviewer.
    Reviewer,
}

/// Workspace registration payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceRegisterPayload {
    /// Absolute or resolvable project root.
    pub project_root: String,
    /// Exact project identity.
    pub project_key: String,
    /// Initial migration mode; defaults to shadow.
    #[serde(default)]
    pub mode: Option<WorkspaceMode>,
}

/// Admin-only, drain-only migration mode transition payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceModeTransitionPayload {
    /// Exact next migration mode.
    pub target_mode: WorkspaceMode,
    /// Bounded auditable reason.
    pub reason: String,
    /// Digest of the shadow/canary parity evidence.
    pub parity_digest: String,
    /// Typed parity observation whose canonical digest must equal `parityDigest`.
    pub parity: WorkspaceParityEvidence,
}

/// Bounded, typed evidence used for a writer-mode cutover.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceParityEvidence {
    /// Number of legacy/Rust observations compared.
    pub comparison_count: u64,
    /// Number of observations that differed.
    pub mismatch_count: u64,
    /// Authoritative project revision at which the comparison was made.
    pub source_revision: u64,
    /// Digest of the legacy observation set.
    pub legacy_digest: String,
    /// Digest of the Rust observation set.
    pub rust_digest: String,
    /// Unix timestamp at which the evidence was observed.
    pub observed_at_unix_ms: u64,
}

/// Workspace registration/snapshot projection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResult {
    /// Stable workspace identity.
    pub workspace_id: String,
    /// Canonical project root.
    pub canonical_root: String,
    /// Exact project identity.
    pub project_key: String,
    /// Current migration mode.
    pub mode: WorkspaceMode,
    /// Monotonic inventory generation.
    pub inventory_generation: u64,
}

/// Session open payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionOpenPayload {
    /// Host-stable conversation identity used for idempotent open.
    pub external_key: String,
    /// Daemon-derived role requested by the trusted host boundary.
    pub role: WireAgentRole,
    /// Whether fail-closed Hook control is engaged.
    pub engaged: bool,
    /// Required physical delegation binding for non-root sessions.
    #[serde(default)]
    pub delegation_id: Option<String>,
    /// Optional precomputed role-aware context payload.
    #[serde(default)]
    pub context: Option<Value>,
}

/// Session lifecycle response.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResult {
    /// Stable session identity.
    pub session_id: String,
    /// Session role.
    pub role: WireAgentRole,
    /// Engaged fail-closed status.
    pub engaged: bool,
    /// Absolute expiry timestamp.
    pub expires_at_unix_ms: u64,
    /// Current context generation.
    pub context_generation: u64,
    /// Boot-signed capability token for offline fail-closed Hook decisions.
    pub capability_token: String,
}

/// Hook event payload shared by the four Hook methods.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookPayload {
    /// Session-unique Hook event identity.
    pub hook_event_id: String,
    /// Monotonic turn sequence.
    pub turn_seq: u64,
    /// Host-specific bounded payload.
    pub host_payload: Value,
}

/// Hook fast-path response.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookResult {
    /// Whether fail-closed control applies.
    pub engaged: bool,
    /// Host action.
    pub decision: HookDecision,
    /// Optional precomputed role-aware context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
    /// Monotonic durable event sequence.
    pub event_seq: u64,
    /// True when this exact event was replayed from a receipt.
    pub replayed: bool,
}

/// Canonical idempotency receipt persisted before returning success.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdempotencyReceipt {
    /// Identity namespace, including workspace/session/turn where applicable.
    pub scope: String,
    /// Semantic idempotency key.
    pub key: String,
    /// Canonical request digest.
    pub request_digest: String,
    /// Canonical response object.
    pub response_json: String,
    /// Durable event sequence associated with the response.
    pub event_seq: u64,
}

/// Typed durable runtime event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DurableEvent {
    /// Event-store epoch identity.
    pub event_store_id: String,
    /// Global sequence, monotonic across daemon restarts.
    pub event_seq: u64,
    /// Daemon boot that published the event.
    pub boot_id: String,
    /// Versioned event kind.
    pub kind: String,
    /// Optional workspace identity.
    pub workspace_id: Option<String>,
    /// Optional session identity.
    pub session_id: Option<String>,
    /// Optional Work Item identity.
    pub work_item_id: Option<String>,
    /// Bounded typed payload.
    pub payload: Value,
    /// Canonical payload digest.
    pub payload_digest: String,
}

/// Event subscription request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventSubscriptionPayload {
    /// Expected event-store epoch.
    pub event_store_id: String,
    /// Last successfully consumed global sequence.
    pub after_event_seq: u64,
    /// Bounded requested batch length.
    pub limit: usize,
}

/// Bounded event subscription response.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventBatch {
    /// Active event-store epoch.
    pub event_store_id: String,
    /// Ordered events after the requested cursor.
    pub events: Vec<DurableEvent>,
    /// True only when the caller must discard deltas and fetch a snapshot.
    pub snapshot_required: bool,
}

/// Input used to precompute a bounded context projection.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextProjectionInput {
    /// Target session.
    pub session_id: String,
    /// Source authoritative revision.
    pub source_revision: u64,
    /// Role-aware projection body.
    pub projection: Value,
}

/// Context full/delta/no-change request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextProjectPayload {
    /// Known context revision, if any.
    #[serde(default)]
    pub known_revision: u64,
    /// Known canonical digest, if any.
    #[serde(default)]
    pub known_digest: String,
}

/// Bounded context projection response.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextProjectResult {
    /// `full`, `delta`, or `no_change`.
    pub kind: String,
    /// Current context revision.
    pub context_revision: u64,
    /// Current canonical digest.
    pub digest: String,
    /// Source authoritative revision.
    pub source_revision: u64,
    /// Projection body; absent for no-change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projection: Option<Value>,
    /// Serialized projection size.
    pub byte_length: usize,
}

/// Trusted host adapter registration payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostRegisterPayload {
    /// Stable adapter identity.
    pub adapter_id: String,
    /// Exact supported capability names.
    pub capabilities: Vec<String>,
}

/// Durable host action wire payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostActionPayload {
    /// Host action identity.
    pub action_id: String,
    /// Target adapter.
    pub adapter_id: String,
    /// Adapter-scoped monotonic command sequence.
    pub command_seq: u64,
    /// Exact action kind.
    pub kind: String,
    /// Optional delegation binding.
    pub delegation_id: Option<String>,
    /// Optional compact binding.
    pub compact_id: Option<String>,
    /// Optional session binding.
    pub session_id: Option<String>,
    /// Optional context generation binding.
    pub context_generation: Option<u64>,
    /// Absolute deadline.
    pub deadline_unix_ms: u64,
}

/// Host ACK wire payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostAckPayload {
    /// ACK identity.
    pub ack_id: String,
    /// Correlated action.
    pub action_id: String,
    /// Adapter-scoped command sequence.
    pub command_seq: u64,
    /// `accepted` or `rejected`.
    pub outcome: String,
    /// Physical host task identity for create ACKs.
    pub host_task_id: Option<String>,
    /// Host-injected child session identity.
    pub session_id: Option<String>,
}

/// Authenticated token pressure sample.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostPressurePayload {
    /// Adapter identity.
    pub adapter_id: String,
    /// Context generation being measured.
    pub context_generation: u64,
    /// Adapter/session-scoped monotonic sample sequence.
    pub sample_seq: u64,
    /// Tokens currently used.
    pub used_tokens: u64,
    /// Host-reported context window.
    pub context_window_tokens: u64,
    /// Host observation timestamp.
    pub observed_at_unix_ms: u64,
}

/// Delegation creation payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DelegationCreatePayload {
    /// Requested child role.
    pub child_role: WireAgentRole,
    /// Parent delegation, absent for a root-to-series delegation.
    pub parent_delegation_id: Option<String>,
    /// Input authoritative revision.
    pub input_revision: u64,
    /// Input fingerprint.
    pub input_fingerprint: String,
    /// Absolute claim/report deadline.
    pub deadline_unix_ms: u64,
    /// Host adapter selected by policy.
    pub adapter_id: String,
    /// Parent-requested child scope, validated and narrowed by the daemon.
    pub grant: crate::ScopedGrantWire,
}

/// Child physical claim payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DelegationAcceptPayload {
    /// Delegation identity.
    pub delegation_id: String,
    /// Single-use claim identity.
    pub claim_id: String,
    /// Create action being claimed.
    pub action_id: String,
    /// Host-injected child session identity.
    pub child_session_id: String,
    /// Claim expiry.
    pub expires_at_unix_ms: u64,
}

/// Bounded child result payload.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DelegationReportPayload {
    /// Delegation identity.
    pub delegation_id: String,
    /// Input authoritative revision.
    pub input_revision: u64,
    /// Input fingerprint.
    pub input_fingerprint: String,
    /// Bounded summary.
    pub summary: String,
    /// Typed bounded result body containing refs, not transcripts.
    pub result: Value,
}

/// Durable delegation projection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegationResult {
    /// Delegation identity.
    pub delegation_id: String,
    /// Lifecycle state.
    pub status: String,
    /// Canonical daemon-validated scoped grant for the physical child.
    pub grant: crate::ScopedGrantWire,
    /// Child role.
    pub child_role: WireAgentRole,
    /// Host create action.
    pub action_id: String,
    /// Attested child session, once established.
    pub child_session_id: Option<String>,
    /// Bounded result digest, once reported.
    pub result_digest: Option<String>,
}

/// Explicit compact request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactRequestPayload {
    /// Current context generation expected by CAS.
    pub previous_generation: u64,
    /// Snapshot reference digest.
    pub snapshot_digest: String,
    /// Absolute host ACK deadline.
    pub deadline_unix_ms: u64,
    /// Target host adapter.
    pub adapter_id: String,
}

/// Compact ACK plus rehydrate completion payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactAckPayload {
    /// Compact cycle identity.
    pub compact_id: String,
    /// Correlated host action ACK.
    pub ack: HostAckPayload,
    /// Projection digest restored after ACK.
    pub restored_projection_digest: String,
    /// Generation observed before CAS.
    pub observed_generation: u64,
}

/// Durable compact lifecycle projection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactResult {
    /// Compact cycle identity.
    pub compact_id: String,
    /// Lifecycle status.
    pub status: String,
    /// Previous context generation.
    pub previous_generation: u64,
    /// Next context generation.
    pub next_generation: u64,
    /// Correlated host action.
    pub action_id: String,
    /// Restored projection digest only after ACK and rehydrate.
    pub restored_projection_digest: Option<String>,
}
