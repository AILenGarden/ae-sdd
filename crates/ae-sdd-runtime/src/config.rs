use ae_sdd_domain::{DEFAULT_CHILD_RESULT_MAX_BYTES, DEFAULT_CHILD_SUMMARY_MAX_BYTES};
use ae_sdd_protocol::MAX_FRAME_BYTES;

/// Fixed resource and deadline bounds enforced by one daemon boot.
#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    /// Maximum registered workspaces.
    pub max_workspaces: usize,
    /// Maximum active sessions across all workspaces.
    pub max_sessions: usize,
    /// Maximum queued calls for one Work Item actor.
    pub work_item_mailbox_capacity: usize,
    /// Maximum retained Work Item actor slots across all workspaces.
    pub max_work_item_actors: usize,
    /// Maximum retained Work Item actor slots for one workspace.
    pub max_work_item_actors_per_workspace: usize,
    /// Idle actor slot lifetime before bounded eviction.
    pub work_item_actor_idle_ms: u64,
    /// Maximum concurrent admitted connection handlers.
    pub connection_capacity: usize,
    /// Maximum Hook deadline accepted from a caller.
    pub hook_deadline_ms: u64,
    /// Maximum ordinary request deadline.
    pub max_deadline_ms: u64,
    /// Maximum context projection payload.
    pub max_context_projection_bytes: usize,
    /// Maximum framed payload.
    pub max_frame_bytes: usize,
    /// Maximum returned event batch.
    pub max_event_batch: usize,
    /// Maximum durable jobs retained by one daemon boot.
    pub max_jobs: usize,
    /// Session heartbeat validity.
    pub session_ttl_ms: u64,
    /// Maximum age of typed parity evidence accepted for a cutover.
    pub parity_evidence_ttl_ms: u64,
    /// Current daemon policy digest.
    pub policy_digest: String,
    /// Current typed operation registry digest.
    pub operation_schema_digest: String,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_workspaces: 10,
            max_sessions: 100,
            work_item_mailbox_capacity: 64,
            max_work_item_actors: 1_024,
            max_work_item_actors_per_workspace: 128,
            work_item_actor_idle_ms: 300_000,
            connection_capacity: 128,
            hook_deadline_ms: 250,
            max_deadline_ms: 30_000,
            max_context_projection_bytes: 65_536,
            max_frame_bytes: MAX_FRAME_BYTES,
            max_event_batch: 256,
            max_jobs: 128,
            session_ttl_ms: 90_000,
            parity_evidence_ttl_ms: 300_000,
            policy_digest: ae_sdd_policy::policy_digest().to_hex(),
            operation_schema_digest: ae_sdd_operations::operation_schema_digest(),
        }
    }
}

impl RuntimeConfig {
    /// Maximum bounded ChildResult bytes published in the handshake.
    #[must_use]
    pub const fn child_result_max_bytes(&self) -> u64 {
        DEFAULT_CHILD_RESULT_MAX_BYTES as u64
    }

    /// Maximum bounded ChildResult summary bytes published in the handshake.
    #[must_use]
    pub const fn child_summary_max_bytes(&self) -> u64 {
        DEFAULT_CHILD_SUMMARY_MAX_BYTES as u64
    }
}
