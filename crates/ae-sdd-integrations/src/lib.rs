//! Platform and persistence adapters for the ae-sdd runtime.

#![warn(missing_docs)]

mod business;
mod command;
mod endpoint;
mod error;
mod execution_authority;
mod gate_source;
mod host_supervisor;
mod ipc;
mod lifecycle_authority;
mod operation_semantics;
mod persistence;
mod platform;
pub mod resources;
mod review_authority;
mod watcher;

pub use business::NativeBusinessAdapter;
pub use command::{
    BoundedCommandOutput, BoundedCommandRunner, GitAdapter, HostProcessAdapter, ServiceAdapter,
    ToolchainAdapter,
};
pub use endpoint::{DaemonLock, RuntimePaths, publish_endpoint_manifest, read_endpoint_manifest};
pub use error::{IntegrationError, IntegrationResult};
pub use gate_source::{AuthoritativeGateRuntime, ReviewGateAuthority, gate_result_json};
pub use host_supervisor::{HostAckSummary, HostSupervisor, HostSupervisorError, LocalCancelTarget};
pub use ipc::LocalIpcServer;
pub use persistence::SqliteRuntimePersistence;
pub use platform::{FileWorkspaceResolver, SystemClock};
pub use watcher::{BoundedWorkspaceWatcher, FullReconcileReason, WatchSignal};

/// Integration adapter build identity.
pub const INTEGRATIONS_BUILD: &str =
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
