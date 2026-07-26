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

/// Strict `hostPayload.executionEvent` decode target.
///
/// Every field beyond the required `class` is optional; unknown fields or
/// wrong shapes fail closed during decode.  Only bounded metadata crosses
/// this boundary — never the tool output body.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionHookEvent {
    /// Host-reported execution tool class.
    pub class: String,
    /// Focused verification outcome (`pass`/`fail`) once it completed.
    #[serde(default)]
    pub outcome: Option<String>,
    /// Canonical project-relative source path, when the event reads one.
    #[serde(default)]
    pub path: Option<String>,
    /// Digest of the content that was read.
    #[serde(default)]
    pub content_digest: Option<String>,
    /// Digest of the bounded search query identity.
    #[serde(default)]
    pub query_digest: Option<String>,
    /// Digest of the resulting patched content.
    #[serde(default)]
    pub result_digest: Option<String>,
    /// Digest of the appended evidence ledger event.
    #[serde(default)]
    pub event_digest: Option<String>,
    /// Raw output bytes produced by the tool call.
    #[serde(default)]
    pub output_bytes: Option<u32>,
    /// Digest of the full tool output.
    #[serde(default)]
    pub output_digest: Option<String>,
    /// Inclusive 1-based first line, when ranged.
    #[serde(default)]
    pub start_line: Option<u32>,
    /// Inclusive 1-based last line, when ranged.
    #[serde(default)]
    pub end_line: Option<u32>,
}

/// Machine decision carried by the optional `executionDirective`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionHookDirectiveDecision {
    /// The event is admissible.
    Allow,
    /// The event is rejected until machine-verified progress is made.
    RequireProgress,
}

/// Optional execution supervision guidance attached to a Hook result.
///
/// Old clients ignore this field entirely; new clients may honor the stable
/// reason code, the frozen retained-output budget, a bounded retry hint or a
/// cached-read reference.  `retryAfterMs` and `cachedReadRef` are populated
/// by later rollout stages (resource arbitration and the source-read cache).
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionHookDirective {
    /// Supervisor decision for the classified event.
    pub decision: ExecutionHookDirectiveDecision,
    /// Stable machine reason code (for example `EXECUTION_PROGRESS_REQUIRED`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    /// Frozen single-call retained-output budget in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_budget_bytes: Option<u32>,
    /// Bounded retry hint for deferred events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
    /// Reference to a cached source read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_read_ref: Option<String>,
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
    /// Optional execution supervision directive; absent for unclassified or
    /// unbound events so old clients can ignore the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_directive: Option<ExecutionHookDirective>,
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

/// Aggregate family committed by the typed identity repository.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeIdentityKind {
    /// Workspace registration or transition.
    Workspace,
    /// Agent session lifecycle.
    Session,
    /// Delegation lifecycle and physical attestation.
    Delegation,
}

/// Typed durable workspace row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWorkspaceRecord {
    /// Stable workspace identity.
    pub workspace_id: String,
    /// Canonical absolute root.
    pub canonical_root: String,
    /// Exact project identity.
    pub project_key: String,
    /// Daemon-owned migration mode.
    pub mode: WorkspaceMode,
    /// Monotonic inventory generation.
    pub inventory_generation: u64,
    /// Dirty inventory marker.
    pub dirty: bool,
    /// Creation time in Unix milliseconds.
    pub created_at_unix_ms: u64,
    /// Last update time in Unix milliseconds.
    pub updated_at_unix_ms: u64,
}

/// Typed durable session row and secret-free replay material.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSessionRecord {
    /// Physical session identity.
    pub session_id: String,
    /// Authenticated agent identity.
    pub agent_id: String,
    /// Owning workspace.
    pub workspace_id: String,
    /// Hash of the host external session key.
    pub external_key_hash: String,
    /// Daemon-derived role.
    pub role: WireAgentRole,
    /// Root orchestration session.
    pub root_session_id: String,
    /// Parent physical session for a child.
    pub parent_session_id: Option<String>,
    /// Delegation for a child.
    pub delegation_id: Option<String>,
    /// Whether fail-closed control is engaged.
    pub engaged: bool,
    /// Current work item, when bound.
    pub current_work_item: Option<String>,
    /// Canonical scoped grant.
    pub grant: crate::ScopedGrantWire,
    /// Monotonic context generation.
    pub context_generation: u64,
    /// Session expiry in Unix milliseconds.
    pub expires_at_unix_ms: u64,
    /// Durable lifecycle status.
    pub status: String,
    /// Creation time in Unix milliseconds.
    pub created_at_unix_ms: u64,
    /// Last update time in Unix milliseconds.
    pub updated_at_unix_ms: u64,
}

/// Typed durable delegation row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDelegationRecord {
    /// Stable delegation identity.
    pub delegation_id: String,
    /// Owning workspace.
    pub workspace_id: String,
    /// Root orchestration session.
    pub root_session_id: String,
    /// Parent physical session.
    pub parent_session_id: String,
    /// Attested child physical session.
    pub child_session_id: Option<String>,
    /// Parent delegation, when nested.
    pub parent_delegation_id: Option<String>,
    /// Child role.
    pub role: WireAgentRole,
    /// Source state revision.
    pub input_revision: u64,
    /// Source input fingerprint.
    pub input_fingerprint: String,
    /// Durable lifecycle status.
    pub status: String,
    /// Absolute deadline in Unix milliseconds.
    pub deadline_unix_ms: u64,
    /// Digest of the canonical delegation projection.
    pub receipt_digest: String,
    /// Creation time in Unix milliseconds.
    pub created_at_unix_ms: u64,
    /// Last update time in Unix milliseconds.
    pub updated_at_unix_ms: u64,
}

/// Delegation-scoped host action binding.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDelegationHostActionRecord {
    /// Owning workspace.
    pub workspace_id: String,
    /// Delegation being spawned.
    pub delegation_id: String,
    /// Host action identity.
    pub host_action_id: String,
    /// Parent physical session.
    pub parent_session_id: String,
    /// Canonical action request digest.
    pub action_digest: String,
    /// Creation time in Unix milliseconds.
    pub created_at_unix_ms: u64,
}

/// Secret-free physical delegation attestation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDelegationAttestationRecord {
    /// Owning workspace.
    pub workspace_id: String,
    /// Delegation identity.
    pub delegation_id: String,
    /// Physical child session.
    pub physical_session_id: String,
    /// Bound Host action.
    pub host_action_id: String,
    /// Correlated Host ACK.
    pub host_ack_id: String,
    /// Action digest.
    pub action_digest: String,
    /// ACK digest.
    pub ack_digest: String,
    /// Digest of the single-use claim; the raw claim is never durable.
    pub claim_digest: String,
    /// Canonical scoped grant.
    pub grant: crate::ScopedGrantWire,
    /// Immutable attestation reference.
    pub attestation_ref: String,
    /// Attestation digest.
    pub attestation_digest: String,
    /// Boot that accepted the physical proof.
    pub accepted_boot_id: String,
    /// Acceptance time in Unix milliseconds.
    pub accepted_at_unix_ms: u64,
    /// Expiry time in Unix milliseconds.
    pub expires_at_unix_ms: u64,
}

/// Secret-free identity after-image used for durable replay.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeIdentitySnapshot {
    /// Aggregate family.
    pub identity_kind: RuntimeIdentityKind,
    /// Workspace after-image.
    pub workspace: RuntimeWorkspaceRecord,
    /// Optional session after-image.
    pub session: Option<RuntimeSessionRecord>,
    /// Optional delegation after-image.
    pub delegation: Option<RuntimeDelegationRecord>,
    /// Optional Host action binding.
    pub host_action: Option<RuntimeDelegationHostActionRecord>,
    /// Optional physical attestation.
    pub attestation: Option<RuntimeDelegationAttestationRecord>,
    /// Secret-free operation response material.
    pub response: Value,
    /// True only on the returned value when a receipt was replayed.
    #[serde(skip, default)]
    pub replayed: bool,
}

/// One atomic typed identity mutation and idempotency receipt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeIdentityTransition {
    /// Exact operation name.
    pub operation: String,
    /// Domain-separated scope digest.
    pub scope_digest: String,
    /// Raw bounded idempotency key.
    pub idempotency_key: String,
    /// Digest of the complete trusted request.
    pub request_digest: String,
    /// Expected workspace mode, when performing CAS.
    pub expected_workspace_mode: Option<WorkspaceMode>,
    /// Expected inventory generation, when performing CAS.
    pub expected_inventory_generation: Option<u64>,
    /// Expected session status, when performing CAS.
    pub expected_session_status: Option<String>,
    /// Expected delegation status, when performing CAS.
    pub expected_delegation_status: Option<String>,
    /// Expected context generation, when performing CAS.
    pub expected_context_generation: Option<u64>,
    /// Complete secret-free after-image.
    pub snapshot: RuntimeIdentitySnapshot,
    /// Commit time in Unix milliseconds.
    pub committed_at_unix_ms: u64,
}

/// Durable runtime job lifecycle state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeJobStatus {
    /// Accepted and waiting for execution.
    Queued,
    /// Currently executing.
    Running,
    /// Successful PASS result.
    Pass,
    /// Completed FAIL result.
    Fail,
    /// Infrastructure or schema error.
    Error,
    /// Deadline exceeded.
    Timeout,
    /// Cancelled by an authorized caller.
    Cancelled,
    /// Invalidated by restart or freshness drift.
    Stale,
}

/// Typed durable runtime job projection.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeJobRecord {
    /// Stable job identity.
    pub job_id: String,
    /// Owning workspace identity.
    pub workspace_id: String,
    /// Optional work-item binding.
    pub work_item_id: Option<String>,
    /// Optional physical session identity.
    pub session_id: Option<String>,
    /// Optional root session identity.
    pub root_session_id: Option<String>,
    /// Optional delegation identity.
    pub delegation_id: Option<String>,
    /// Optional daemon-derived role.
    pub agent_role: Option<WireAgentRole>,
    /// Optional captured context generation.
    pub context_generation: Option<u64>,
    /// Boot that accepted the job.
    pub submission_boot_id: Option<String>,
    /// Physical attestation reference.
    pub attestation_ref: Option<String>,
    /// Physical attestation digest.
    pub attestation_digest: Option<String>,
    /// Canonical captured grant.
    pub grant: Option<crate::ScopedGrantWire>,
    /// Digest of the complete captured identity.
    pub identity_digest: Option<String>,
    /// Captured workspace mode.
    pub workspace_mode: WorkspaceMode,
    /// Captured inventory generation.
    pub inventory_generation: u64,
    /// Native job entrypoint.
    pub entrypoint: String,
    /// Canonical bounded arguments.
    pub arguments: Value,
    /// Domain-separated submission scope digest.
    pub submission_scope_digest: String,
    /// Raw bounded submission key.
    pub submission_idempotency_key: String,
    /// Digest of the submission key.
    pub submission_idempotency_key_digest: String,
    /// Complete trusted request digest.
    pub request_digest: String,
    /// Optional authoritative source revision.
    pub source_revision: Option<u64>,
    /// Optional source input fingerprint.
    pub input_fingerprint: Option<String>,
    /// Deadline in Unix milliseconds.
    pub deadline_unix_ms: u64,
    /// Lifecycle status.
    pub status: RuntimeJobStatus,
    /// Monotonic row version.
    pub row_version: u64,
    /// Optional bounded result object.
    pub result: Option<Value>,
    /// Optional stable error code.
    pub error_code: Option<String>,
    /// Optional project mutation identity.
    pub mutation_id: Option<String>,
    /// Optional committed receipt locator.
    pub receipt_locator: Option<String>,
    /// Optional committed project receipt digest.
    pub project_receipt_digest: Option<String>,
    /// First durable event sequence.
    pub submitted_event_seq: u64,
    /// Latest durable event sequence.
    pub last_event_seq: u64,
    /// Creation time in Unix milliseconds.
    pub created_at_unix_ms: u64,
    /// Execution start time in Unix milliseconds.
    pub started_at_unix_ms: Option<u64>,
    /// Terminal time in Unix milliseconds.
    pub finished_at_unix_ms: Option<u64>,
    /// Last update time in Unix milliseconds.
    pub updated_at_unix_ms: u64,
}

/// Expected-value CAS transition for one typed runtime job.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeJobTransition {
    /// Complete proposed after-image.
    pub record: RuntimeJobRecord,
    /// Expected status; absent only for initial submission.
    pub expected_status: Option<RuntimeJobStatus>,
    /// Expected row version; absent only for initial submission.
    pub expected_row_version: Option<u64>,
    /// Canonical event committed with this transition.
    pub event: DurableEvent,
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
