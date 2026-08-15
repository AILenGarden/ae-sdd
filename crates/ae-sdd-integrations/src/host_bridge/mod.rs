//! Host bridge adapters that connect the daemon to a native host session.
//!
//! The bridge is the execution bridge between daemon-owned delegation state and
//! a host's native session lifecycle. It never declares capabilities; each
//! action reports its real outcome.

mod bootstrap;

pub use bootstrap::{
    ChildBootstrapChallenge, ChildBootstrapEnvelope, ChildBootstrapError, OneShotBootstrapChannel,
};
