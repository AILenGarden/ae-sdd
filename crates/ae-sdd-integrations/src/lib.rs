//! Platform and persistence adapters for the ae-sdd runtime.

#![warn(missing_docs)]

mod business;
mod command;
mod endpoint;
mod error;
mod gate_source;
mod ipc;
mod persistence;
mod platform;
mod watcher;

pub use business::NativeBusinessAdapter;
pub use command::{
    BoundedCommandOutput, BoundedCommandRunner, GitAdapter, HostProcessAdapter, ServiceAdapter,
    ToolchainAdapter,
};
pub use endpoint::{DaemonLock, RuntimePaths, publish_endpoint_manifest, read_endpoint_manifest};
pub use error::{IntegrationError, IntegrationResult};
pub use gate_source::{AuthoritativeGateRuntime, gate_result_json};
pub use ipc::LocalIpcServer;
pub use persistence::SqliteRuntimePersistence;
pub use platform::{FileWorkspaceResolver, SystemClock};
pub use watcher::{BoundedWorkspaceWatcher, FullReconcileReason, WatchSignal};

/// Integration adapter build identity.
pub const INTEGRATIONS_BUILD: &str =
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
