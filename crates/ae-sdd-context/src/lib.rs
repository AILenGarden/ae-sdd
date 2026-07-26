mod compact;
mod pressure;
mod projection;
mod service;

pub use compact::{
    CompactCoordinator, CompactCoordinatorError, CompactCoordinatorStatus, CompactCycle,
    CompactCycleError, CompactStatus, ContextCapsule, MAX_COMPACT_CAPSULE_BYTES,
};
pub use pressure::{
    DEFAULT_CONSECUTIVE_SAMPLES, DEFAULT_COOLDOWN_MS, DEFAULT_HIGH_WATERMARK_BPS,
    DEFAULT_HIGH_WATERMARK_PERMILLE, DEFAULT_LOW_WATERMARK_BPS, DEFAULT_LOW_WATERMARK_PERMILLE,
    PressureDecision, PressureError, PressurePolicy, PressureSample, PressureSource,
    PressureTracker,
};
pub use projection::{
    ContextDelta, ContextProjection, ContextProjectionError, ContextView,
    ExecutionCapsuleProjection, MemoryVisibility, ProjectionBudget, ProjectionKind, RoleMemoryRef,
};
pub use service::{
    BundledContext, CompactStateDelta, ContextBundleInput, ContextCacheKey, ContextFreshness,
    ContextFreshnessDimension, ContextPort, ContextRefresh, ContextSelector, ContextService,
    ContextServiceError, ExecutionCapsuleKey, MAX_COMPACT_STATE_DELTA_BYTES,
    MAX_COMPACT_STATE_DELTA_ENTRIES,
};
