//! Deterministic per-user service descriptors and lifecycle operation plans.

use std::path::PathBuf;

use thiserror::Error;

mod executor;
mod materialize;
mod model;
mod render;

pub use executor::{
    ServiceManagerRunner, execute_service_lifecycle, execute_service_lifecycle_with_runner,
};
pub use materialize::{inspect_service_descriptor, materialize_service_descriptor};
pub use model::{
    SERVICE_EXECUTION_SCHEMA, SERVICE_PLAN_SCHEMA, SERVICE_REQUEST_SCHEMA, ServiceCommandReceipt,
    ServiceDescriptorAction, ServiceDescriptorState, ServiceDescriptorStatus,
    ServiceExecutionLimits, ServiceExecutionReceipt, ServiceLifecycleContract,
    ServiceLifecyclePlan, ServiceLifecycleRequest, ServiceManagerCommand, ServiceManagerOutput,
    ServiceMaterialization, ServiceOperation, ServicePermissionAssertion, ServicePermissionPolicy,
    ServicePlatform,
};
pub use render::generate_service_lifecycle_plan;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("unsupported service request schema {0}")]
    Schema(String),
    #[error("service field {0} is empty, too large, or contains a line/NUL delimiter")]
    InvalidField(&'static str),
    #[error("service path {0} must be absolute and lexically normalized")]
    InvalidPath(&'static str),
    #[error("service environment key is not a portable identifier: {0}")]
    InvalidEnvironmentKey(String),
    #[error("service restart delay must be between 1 and 300 seconds")]
    InvalidRestartDelay,
    #[error("service descriptor would embed a secret or forbidden runtime fallback")]
    SecretInDescriptor,
    #[error("service descriptor exceeds the one MiB safety budget")]
    DescriptorTooLarge,
    #[error("service destination resolves outside the requested user home")]
    DestinationOutsideUserHome,
    #[error("service descriptor path is a symbolic link: {0}")]
    SymbolicLink(PathBuf),
    #[error("service descriptor staging files already exist")]
    StagingConflict,
    #[error("service permission verification failed")]
    PermissionVerificationFailed,
    #[error("service descriptor drift prevents safe removal")]
    DescriptorDrift,
    #[error("service execution plan targets {planned:?}, but this host is {current:?}")]
    PlatformMismatch {
        planned: ServicePlatform,
        current: ServicePlatform,
    },
    #[error("service manager executable is not allowlisted: {0}")]
    ManagerProgramDenied(String),
    #[error("service manager plan requests elevation or escapes the current-user scope")]
    PrivilegeEscalation,
    #[error("service manager arguments are empty, oversized, or contain delimiters")]
    InvalidManagerArguments,
    #[error("service manager execution limits are outside the supported bounds")]
    InvalidExecutionLimits,
    #[error("service manager {program} did not expose its {stream} pipe")]
    ManagerPipe {
        program: String,
        stream: &'static str,
    },
    #[error("service manager {program} {stream} reader panicked")]
    ManagerReaderPanicked {
        program: String,
        stream: &'static str,
    },
    #[error("service manager I/O failed for {program}: {source}")]
    ManagerIo {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("service manager command {purpose} timed out after {timeout_millis}ms: {stderr}")]
    ManagerTimedOut {
        purpose: String,
        timeout_millis: u64,
        stderr: String,
    },
    #[error("service manager command {purpose} exited with {exit_code:?}: {stderr}")]
    ManagerFailed {
        purpose: String,
        exit_code: Option<i32>,
        stderr: String,
    },
    #[error("service execution receipt JSON is invalid: {0}")]
    InvalidExecutionReceipt(#[source] serde_json::Error),
    #[error("unsupported service execution receipt schema {0}")]
    ExecutionReceiptSchema(String),
    #[error("service I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
