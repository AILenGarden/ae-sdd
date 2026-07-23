//! User-level daemon application runtime.
//!
//! The crate owns admission, identity, session, Hook, event, delegation,
//! context, and supervisor coordination. Filesystems, clocks, SQLite, local
//! IPC, Git, toolchains, and host processes enter only through inward ports.

#![warn(missing_docs)]

mod actor;
mod config;
mod error;
mod model;
mod ports;
mod service;
mod supervisor;

pub use actor::WorkItemActors;
pub use config::RuntimeConfig;
pub use error::{RuntimeError, RuntimeResult};
pub use model::{
    CompactAckPayload, CompactRequestPayload, CompactResult, ContextProjectPayload,
    ContextProjectResult, ContextProjectionInput, DaemonLifecycle, DelegationAcceptPayload,
    DelegationCreatePayload, DelegationReportPayload, DelegationResult, DurableEvent, EventBatch,
    EventSubscriptionPayload, HookPayload, HookResult, HostAckPayload, HostActionPayload,
    HostPressurePayload, HostRegisterPayload, IdempotencyReceipt, RuntimeStatus,
    SessionOpenPayload, SessionResult, WireAgentRole, WorkspaceRegisterPayload, WorkspaceResult,
};
pub use ports::{
    BusinessOperationPort, BusinessWorkspace, ClockPort, MemoryPersistence, PersistencePort,
    RejectingBusinessPort, ResolvedWorkspace, WorkspaceResolverPort,
};
pub use service::{ConnectionState, RuntimeService};
pub use supervisor::{ContextCache, DelegationSupervisor, FlowSupervisor, HostCoordinator};

/// Runtime build identity.
pub const RUNTIME_BUILD: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
