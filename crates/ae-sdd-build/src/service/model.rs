use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const SERVICE_REQUEST_SCHEMA: &str = "ae-sdd-service-request/v1";
pub const SERVICE_PLAN_SCHEMA: &str = "ae-sdd-service-plan/v1";
pub const SERVICE_EXECUTION_SCHEMA: &str = "ae-sdd-service-execution/v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ServicePlatform {
    Windows,
    Macos,
    Linux,
}

impl ServicePlatform {
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Linux
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Macos => "macos",
            Self::Linux => "linux",
        }
    }

    #[must_use]
    pub const fn manager(self) -> &'static str {
        match self {
            Self::Windows => "task-scheduler",
            Self::Macos => "launchd-agent",
            Self::Linux => "systemd-user",
        }
    }

    #[must_use]
    pub const fn service_name(self) -> &'static str {
        match self {
            Self::Windows => "ae-sdd-daemon",
            Self::Macos => "com.ae-sdd.daemon",
            Self::Linux => "ae-sdd.service",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceOperation {
    Install,
    Uninstall,
    Status,
}

impl ServiceOperation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Uninstall => "uninstall",
            Self::Status => "status",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceLifecycleRequest {
    pub schema_version: String,
    pub platform: ServicePlatform,
    pub operation: ServiceOperation,
    pub executable: PathBuf,
    pub state_dir: PathBuf,
    pub working_directory: PathBuf,
    pub allowed_roots: Vec<PathBuf>,
    pub user_home: PathBuf,
    pub user_identity: String,
    #[serde(default)]
    pub extra_arguments: Vec<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    pub restart_delay_seconds: u32,
}

impl ServiceLifecycleRequest {
    #[must_use]
    pub fn new(
        platform: ServicePlatform,
        operation: ServiceOperation,
        executable: PathBuf,
        state_dir: PathBuf,
        working_directory: PathBuf,
        allowed_roots: Vec<PathBuf>,
        user_home: PathBuf,
        user_identity: String,
    ) -> Self {
        Self {
            schema_version: SERVICE_REQUEST_SCHEMA.to_owned(),
            platform,
            operation,
            executable,
            state_dir,
            working_directory,
            allowed_roots,
            user_home,
            user_identity,
            extra_arguments: Vec::new(),
            environment: BTreeMap::new(),
            restart_delay_seconds: 2,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceLifecyclePlan {
    pub schema_version: &'static str,
    pub platform: ServicePlatform,
    pub operation: ServiceOperation,
    pub manager: &'static str,
    pub service_name: &'static str,
    pub user_home: PathBuf,
    pub state_dir: PathBuf,
    pub descriptor_path: PathBuf,
    pub descriptor_digest: String,
    pub descriptor_contents: String,
    pub manager_commands: Vec<ServiceManagerCommand>,
    pub permission_policy: ServicePermissionPolicy,
    pub lifecycle_contract: ServiceLifecycleContract,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceManagerCommand {
    pub purpose: &'static str,
    pub program: &'static str,
    pub arguments: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServicePermissionPolicy {
    pub user_scope_only: bool,
    pub elevation_required: bool,
    pub runtime_directory_mode: Option<&'static str>,
    pub descriptor_mode: Option<&'static str>,
    pub endpoint_manifest_mode: Option<&'static str>,
    pub windows_dacl_principal: Option<String>,
    pub windows_inheritance_removed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceLifecycleContract {
    pub daemon_argv: Vec<String>,
    pub secrets_embedded: bool,
    pub shell_wrapper: bool,
    pub current_user_identity: String,
    pub state_retained_on_uninstall: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceMaterialization {
    pub schema_version: &'static str,
    pub descriptor_path: PathBuf,
    pub descriptor_digest: String,
    pub created: bool,
    pub permission_assertions: Vec<ServicePermissionAssertion>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServicePermissionAssertion {
    pub target: PathBuf,
    pub expected: String,
    pub observed: String,
    pub passed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceDescriptorState {
    Absent,
    Matches,
    Drifted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDescriptorStatus {
    pub schema_version: &'static str,
    pub descriptor_path: PathBuf,
    pub state: ServiceDescriptorState,
    pub expected_digest: String,
    pub observed_digest: Option<String>,
    pub permission_assertions: Vec<ServicePermissionAssertion>,
}

/// Filesystem action applied to the native service descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceDescriptorAction {
    /// No descriptor mutation was permitted, as for status operations.
    None,
    /// A missing or drifted descriptor was written before manager registration.
    Materialized,
    /// The matching descriptor already existed and was revalidated.
    Revalidated,
    /// The matching descriptor was removed after manager unregistration.
    Removed,
    /// No descriptor existed after manager unregistration.
    AlreadyAbsent,
    /// An earlier committed operation already established the requested state.
    Replayed,
}

/// Bounded raw result returned by an injectable service-manager runner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceManagerOutput {
    /// Native process exit code, or `None` when the platform did not expose one.
    pub exit_code: Option<i32>,
    /// Raw standard output. The executor applies its configured byte bound again.
    pub stdout: Vec<u8>,
    /// Raw standard error. The executor applies its configured byte bound again.
    pub stderr: Vec<u8>,
    /// Whether the manager exceeded the supplied deadline and was terminated.
    pub timed_out: bool,
    /// Observed wall-clock execution duration in milliseconds.
    pub elapsed_millis: u64,
}

/// Successful bounded execution evidence for one manager command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCommandReceipt {
    /// Stable semantic purpose from the generated lifecycle plan.
    pub purpose: String,
    /// Exact allowlisted executable invoked without a shell.
    pub program: String,
    /// Exact argument vector passed to the executable.
    pub arguments: Vec<String>,
    /// Successful native exit code.
    pub exit_code: Option<i32>,
    /// UTF-8-lossy, bounded standard output.
    pub stdout: String,
    /// UTF-8-lossy, bounded standard error.
    pub stderr: String,
    /// Whether stdout exceeded the configured receipt budget.
    pub stdout_truncated: bool,
    /// Whether stderr exceeded the configured receipt budget.
    pub stderr_truncated: bool,
    /// Observed wall-clock execution duration in milliseconds.
    pub elapsed_millis: u64,
}

/// Typed receipt for an explicitly executed service lifecycle plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceExecutionReceipt {
    /// Stable wire schema for lifecycle execution evidence.
    pub schema_version: &'static str,
    /// Digest binding the descriptor and exact manager argv sequence.
    pub plan_digest: String,
    /// Host platform selected by the executed plan.
    pub platform: ServicePlatform,
    /// Lifecycle operation that completed.
    pub operation: ServiceOperation,
    /// True when a prior committed receipt made this call side-effect free.
    pub replayed: bool,
    /// Descriptor state observed before any permitted mutation.
    pub descriptor_before: ServiceDescriptorState,
    /// Descriptor mutation or replay decision made by the executor.
    pub descriptor_action: ServiceDescriptorAction,
    /// Bounded evidence for commands executed in plan order.
    pub commands: Vec<ServiceCommandReceipt>,
}

/// Deadline and output budgets enforced for every manager command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceExecutionLimits {
    /// Maximum duration allowed for one native manager process.
    pub command_timeout: Duration,
    /// Maximum stdout and stderr bytes retained independently in a receipt.
    pub max_output_bytes: usize,
}

impl Default for ServiceExecutionLimits {
    fn default() -> Self {
        Self {
            command_timeout: Duration::from_secs(30),
            max_output_bytes: 64 * 1024,
        }
    }
}
