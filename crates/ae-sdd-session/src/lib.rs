#![forbid(unsafe_code)]

//! Session bootstrap implementation boundary for Part B.
//!
//! Owns the pure bootstrap decision (`SessionBootstrapPort::bootstrap`):
//! given a frozen C0 `SessionBootstrapRequest` and an explicit,
//! caller-resolved [`model::BootstrapSnapshot`], decides the ordered
//! [`model::BootstrapPlan`]. This crate performs no I/O and reads no
//! ambient state — see `constraints/layered-arch.md` for the
//! application/adapter boundary this preserves.

mod error;
mod model;
mod ports;
mod service;

pub use error::SessionBootstrapError;
pub use model::{BootstrapPlan, BootstrapSnapshot, BootstrapStep, ExistingSessionInfo};
pub use ports::{PureSessionBootstrap, SessionBootstrapPort};
pub use service::bootstrap;
