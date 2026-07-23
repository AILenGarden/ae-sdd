mod benchmark;
mod compatibility;
mod config;
mod jobs;
mod release;

pub use benchmark::{BenchmarkError, HookBenchmarkConfig, HookBenchmarkSummary, benchmark_hook};
pub use compatibility::{
    AuditSummary, CapabilityEvidence, CapabilitySurface, CommandRoute, CompatibilityManifest,
    CompatibilityRoutingManifest, Disposition, ExpectedCounts, ImplementationStatus,
    InventorySources, ManifestError, RouteIdentity, RouteTarget, SurfaceEntry, audit_compatibility,
};
pub use config::{
    GeneratedConfig, HookConfigInput, HookEvent, HookHost, ServiceConfigInput, ServiceTarget,
    generate_hook_config, generate_service_config,
};
pub use jobs::{
    AdminChange, CompileInput, DistributeInput, ExecutionMode, HarnessInput, InitInput,
    InstallInput, JobError, JobExecution, JobInput, JobReceipt, MigrateInput, NATIVE_ENTRYPOINTS,
    NativeEntrypointSpec, NativeJobKind, NativeJobRequest, PermissionClass, PlannedChange,
    execute_native_job, native_entrypoint,
};
pub use release::{ReleaseArtifact, ReleaseFinding, ReleaseVerification, verify_release};
