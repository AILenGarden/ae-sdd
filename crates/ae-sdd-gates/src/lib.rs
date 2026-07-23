#![forbid(unsafe_code)]

//! Rust-native Gate registry, evaluation ports, and concurrent scheduler.

mod evaluator;
mod registry;
mod scheduler;

pub use evaluator::{
    GateEvidenceSet, GateExecutor, GateInputError, GateInputSource, NativeGateExecutor,
    PredicateEvidence,
};
pub use registry::{
    GATE_COUNT, GateRegistry, GateSeverity, GateSpec, NativeGateRule, PredicateKey,
};
pub use scheduler::{
    CancellationToken, EchoFreshness, GateFreshnessSource, GateRunRequest, GateScheduler,
    GateSchedulerError, canonical_gate_key_digest,
};
