//! User-level daemon application runtime.
//!
//! The crate owns admission, identity, session, Hook, event, delegation,
//! context, and supervisor coordination. Filesystems, clocks, SQLite, local
//! IPC, Git, toolchains, and host processes enter only through inward ports.

#![warn(missing_docs)]

mod actor;
mod config;
mod error;
mod grant;
mod model;
mod ports;
mod service;
mod supervisor;

pub use actor::WorkItemActors;
pub use config::RuntimeConfig;
pub use error::{RuntimeError, RuntimeResult};
pub use grant::{GrantPathWire, ScopedGrantWire};
pub use model::{
    CompactAckPayload, CompactRequestPayload, CompactResult, ContextProjectPayload,
    ContextProjectResult, ContextProjectionInput, DaemonLifecycle, DelegationAcceptPayload,
    DelegationCreatePayload, DelegationReportPayload, DelegationResult, DurableEvent, EventBatch,
    EventSubscriptionPayload, ExecutionHookDirective, ExecutionHookDirectiveDecision,
    ExecutionHookEvent, HookPayload, HookResult, HostAckPayload, HostActionPayload,
    HostPressurePayload, HostRegisterPayload, IdempotencyReceipt,
    RuntimeDelegationAttestationRecord, RuntimeDelegationHostActionRecord, RuntimeDelegationRecord,
    RuntimeIdentityKind, RuntimeIdentitySnapshot, RuntimeIdentityTransition, RuntimeJobRecord,
    RuntimeJobStatus, RuntimeJobTransition, RuntimeSessionRecord, RuntimeStatus,
    RuntimeWorkspaceRecord, SessionOpenPayload, SessionResult, WireAgentRole,
    WorkspaceModeTransitionPayload, WorkspaceParityEvidence, WorkspaceRegisterPayload,
    WorkspaceResult,
};
pub use ports::{
    BoundJobIdentity, BusinessOperationPort, BusinessWorkspace, ClockPort, MemoryPersistence,
    PersistencePort, RejectingBusinessPort, ResolvedWorkspace, WorkspaceResolverPort,
};
pub use service::{ConnectionState, ExecutionSessionBinding, RuntimeService};
pub use supervisor::{ContextCache, DelegationSupervisor, FlowSupervisor, HostCoordinator};

/// Runtime build identity.
pub const RUNTIME_BUILD: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
