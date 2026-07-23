#![forbid(unsafe_code)]

//! Pure deterministic flow runtime and event supervisor reducer.
//!
//! Callers persist the returned decision as a checkpoint and perform any I/O
//! represented by [`NextAction`]. This crate never reads a clock, filesystem,
//! database, prompt, or host API.

mod error;
mod model;
mod runtime;

pub use error::FlowError;
pub use model::{
    EventCursor, EventProvenance, FlowDecision, FlowEnvironment, FlowEvent, FlowEventKind,
    FlowInput, FlowSnapshot, NextAction, RouteSelection, SupervisorDegradation, SupervisorFault,
    SupervisorHealth,
};
pub use runtime::FlowRuntime;
