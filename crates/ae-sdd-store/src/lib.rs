mod authority;
mod error;
mod filesystem;
mod journal;
mod lease;
mod model;
mod repository;
mod service;
mod sqlite;

pub use authority::{AuthoritySnapshot, StateAuthority};
pub use error::StoreError;
pub use filesystem::{
    CrossProcessLockPort, DurableFileSystem, ExclusiveLockGuard, InMemoryFileSystem,
    StdCrossProcessLock, StdDurableFileSystem,
};
pub use journal::{
    JournalEvent, JournalReceipt, JournalStatus, MutationJournalEntry, MutationTarget,
    RecoveryDisposition, RecoveryReport, TargetDescriptor,
};
pub use lease::{LeaseLedger, LeaseOwner, LeaseProof, LeaseRecord, LeaseTombstone};
pub use model::{
    ChildResultRecord, CompactCycleRecord, ContextPressureSampleRecord, ContextProjectionRecord,
    DelegationRecord, DelegationRequestReceipt, HookEventReceipt, HostAckReceipt, HostActionRecord,
    HostAdapterRecord, IdempotencyKey, MemoryCleanupReceipt, OperationReceipt, RuntimeEventDraft,
    RuntimeEventPayload, RuntimeEventRecord, SupervisorCheckpointRecord, UtcTimestamp,
};
pub use repository::{InMemoryRuntimeRepository, RuntimeRepository};
pub use service::{
    CommitFaultPort, CommitPoint, CommittedLeaseControl, CommittedMutation, LeaseControlAction,
    LeaseControlPreview, LeaseControlRequest, MutationRequest, NoCommitFault, ProjectMutationStore,
    ProjectStorePaths,
};
pub use sqlite::{
    RuntimeMigration, SQLITE_RUNTIME_BASE_MIGRATION, SQLITE_RUNTIME_MIGRATIONS,
    SqliteRuntimeRepository,
};
