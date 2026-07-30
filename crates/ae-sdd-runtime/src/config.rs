use std::path::PathBuf;
use std::sync::Arc;

use ae_sdd_domain::{DEFAULT_CHILD_RESULT_MAX_BYTES, DEFAULT_CHILD_SUMMARY_MAX_BYTES};
use ae_sdd_protocol::MAX_FRAME_BYTES;

#[path = "execution_cache.rs"]
pub mod execution_cache;
#[path = "execution_resources.rs"]
pub mod execution_resources;

use execution_cache::SourceReadCache;
use execution_resources::CargoResourceArbiter;

/// Boot-scoped execution resources shared by every clone of one
/// [`RuntimeConfig`].
///
/// The source-read cache and the Cargo lease arbiter are daemon-wide by
/// design: the config value is created once per daemon boot and carried into
/// the runtime service, so every session of the boot shares one bounded cache
/// and one fair Cargo queue.  Both components hold only rebuildable in-memory
/// state; source bodies are never persisted.
#[derive(Debug)]
pub struct ExecutionResources {
    source_reads: SourceReadCache,
    cargo: CargoResourceArbiter,
}

impl ExecutionResources {
    fn new() -> Self {
        Self {
            source_reads: SourceReadCache::new(),
            cargo: CargoResourceArbiter::new(),
        }
    }

    /// Returns the bounded source-read cache of this daemon boot.
    pub(crate) fn source_reads(&self) -> &SourceReadCache {
        &self.source_reads
    }

    /// Returns the fair, TTL-bounded Cargo lease arbiter of this daemon boot.
    pub(crate) fn cargo(&self) -> &CargoResourceArbiter {
        &self.cargo
    }
}

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
    /// Maximum retained source-read cache entries for one daemon boot.
    pub source_read_cache_capacity: usize,
    /// Cargo lease time-to-live in milliseconds; a lease at or beyond its TTL
    /// is released for the next waiter.
    pub cargo_lock_ttl_ms: u64,
    /// Bounded retry hint attached to a Cargo deferral, in milliseconds.
    pub cargo_lock_retry_after_ms: u64,
    /// Maximum queued Cargo waiters in the fair FIFO.
    pub cargo_lock_queue_capacity: usize,
    /// Explicit daemon-wide Cargo lock file under the per-user runtime state
    /// dir.
    ///
    /// The path is always fully resolved by the daemon composition root —
    /// never derived from a workspace root or an unresolved environment
    /// variable.  `None` degrades to in-process arbitration only.
    pub cargo_lock_path: Option<PathBuf>,
    /// Current daemon policy digest.
    pub policy_digest: String,
    /// Current typed operation registry digest.
    pub operation_schema_digest: String,
    /// Boot-scoped shared execution resources (source-read cache and Cargo
    /// lease arbiter); one logical instance per daemon boot, shared by every
    /// clone of this config value.
    ///
    /// Internal plumbing: the type is intentionally not nameable outside the
    /// crate (the `config` module is private), so this cannot become a public
    /// dependency surface; the field is `pub` only so external test crates
    /// can keep using struct-update syntax over `RuntimeConfig::default()`.
    #[doc(hidden)]
    pub execution_resources: Arc<ExecutionResources>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_workspaces: 256,
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
            source_read_cache_capacity: 256,
            cargo_lock_ttl_ms: 300_000,
            cargo_lock_retry_after_ms: 1_000,
            cargo_lock_queue_capacity: 64,
            cargo_lock_path: None,
            policy_digest: ae_sdd_policy::policy_digest().to_hex(),
            operation_schema_digest: ae_sdd_operations::operation_schema_digest(),
            execution_resources: Arc::new(ExecutionResources::new()),
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

    /// Returns the boot-scoped shared execution resources.
    pub(crate) fn execution_resources(&self) -> &ExecutionResources {
        &self.execution_resources
    }
}
