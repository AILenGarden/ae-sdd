//! Thin local client for the ae-sdd daemon.

#![warn(missing_docs)]

mod capability;
mod client;
mod error;
mod hook;
mod paths;
mod transport;

pub use capability::{OfflineCapabilityClaims, OfflineCapabilityVerifier};
pub use client::DaemonClient;
pub use error::{ClientError, ClientResult};
pub use hook::{HookClient, HookInvocation, HookOutcome};
pub use paths::{default_endpoint_manifest, default_state_dir};
pub use transport::{ClientTransport, LocalIpcTransport};

/// Client build identity.
pub const CLIENT_BUILD: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
