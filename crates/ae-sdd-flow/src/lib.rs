#![forbid(unsafe_code)]

//! Pure deterministic flow runtime and event supervisor reducer.
//!
//! Callers persist the returned decision as a checkpoint and perform any I/O
//! represented by [`NextAction`]. This crate never reads a clock, filesystem,
//! database, prompt, or host API.

mod canonical;
mod control;
mod error;
mod model;
mod route;
mod runtime;
mod series;

pub use control::{
    ControlAction, ControlDecision, ControlPlaneError, ControlPlaneRuntime, ControlProvenance,
};
pub use error::FlowError;
pub use model::{
    EventCursor, EventProvenance, ExecutionCursor, FlowDecision, FlowEnvironment, FlowEvent,
    FlowEventKind, FlowInput, FlowSnapshot, NextAction, RouteSelection, SupervisorDegradation,
    SupervisorFault, SupervisorHealth,
};
pub use route::{DEFAULT_ROUTE_CONFIDENCE_THRESHOLD_BPS, RouteEngine, RouteEngineError};
pub use runtime::FlowRuntime;
pub use series::{SeriesPlanner, SeriesPlannerError};
