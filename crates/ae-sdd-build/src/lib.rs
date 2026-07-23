mod benchmark;
mod compatibility;
mod config;
mod harness_build;
mod jobs;
mod offline;
mod post_commit;
mod release;
mod service;

pub use benchmark::{BenchmarkError, HookBenchmarkConfig, HookBenchmarkSummary, benchmark_hook};
pub use compatibility::{
    AuditSummary, C_ADMIN_JOB_COMMANDS, CapabilityEvidence, CapabilitySurface, CommandRoute,
    CompatibilityManifest, CompatibilityRoutingManifest, D_REJECTED_COMMANDS, Disposition,
    ExpectedCounts, ImplementationStatus, InventorySources, ManifestError, RouteIdentity,
    RouteTarget, SurfaceEntry, audit_compatibility,
};
pub use config::{
    GeneratedConfig, HookConfigInput, HookEvent, HookHost, ServiceConfigInput, ServiceTarget,
    generate_hook_config, generate_service_config,
};
pub use harness_build::{HarnessBuildRequest, execute_harness_build};
pub use jobs::{
    AdminChange, CompileInput, DistributeInput, ExecutionMode, HarnessInput, InitInput,
    InstallInput, JobError, JobExecution, JobInput, JobReceipt, MigrateInput, NATIVE_ENTRYPOINTS,
    NativeEntrypointSpec, NativeJobKind, NativeJobRequest, PermissionClass, PlannedChange,
    execute_native_job, native_entrypoint,
};
pub use offline::{
    B_OFFLINE_ENTRYPOINTS, DistributorEntry, OfflineCommand, OfflineError, OfflineRequest,
    OfflineResult, execute_offline,
};
pub use post_commit::{
    PostCommitError, PostCommitExecution, PostCommitRequest, execute_post_commit,
};
pub use release::{ReleaseArtifact, ReleaseFinding, ReleaseVerification, verify_release};
pub use service::{
    SERVICE_EXECUTION_SCHEMA, SERVICE_PLAN_SCHEMA, SERVICE_REQUEST_SCHEMA, NativeServiceManagerRunner,
    ServiceCommandReceipt, ServiceDescriptorAction, ServiceDescriptorState, ServiceDescriptorStatus,
    ServiceError, ServiceExecutionLimits, ServiceExecutionReceipt, ServiceLifecycleContract,
    ServiceLifecyclePlan, ServiceLifecycleRequest, ServiceManagerCommand, ServiceManagerOutput,
    ServiceManagerRunner, ServiceMaterialization, ServiceOperation, ServicePermissionAssertion,
    ServicePermissionPolicy, ServicePlatform, execute_service_lifecycle,
    execute_service_lifecycle_with_runner, generate_service_lifecycle_plan,
    inspect_service_descriptor, materialize_service_descriptor,
};
