#![forbid(unsafe_code)]

//! Rust-native Gate registry, evaluation ports, and concurrent scheduler.

mod dag;
mod evaluator;
mod registry;
mod scheduler;

pub use dag::{GateDag, GateDagError};
pub use evaluator::{
    GateEvidenceSet, GateExecutor, GateInputError, GateInputSource, NativeGateExecutor,
    PredicateEvidence,
};
pub use registry::{
    GATE_COUNT, GateDependencySpec, GateInputSelector, GateRegistry, GateSeverity, GateSpec,
    NativeGateRule, PredicateKey,
};
pub use scheduler::{
    CancellationToken, EchoFreshness, GateFreshnessSource, GateRunRequest, GateScheduler,
    GateSchedulerError, GateSchedulerStats, canonical_gate_key_digest,
};
