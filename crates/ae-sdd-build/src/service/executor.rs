use std::io::{self, Read};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::materialize::{
    atomic_write, create_private_directory, ensure_destination, protect_private_file,
    read_descriptor, remove_service_descriptor,
};
use super::{
    SERVICE_EXECUTION_SCHEMA, ServiceCommandReceipt, ServiceDescriptorAction,
    ServiceDescriptorState, ServiceError, ServiceExecutionLimits, ServiceExecutionReceipt,
    ServiceLifecyclePlan, ServiceManagerCommand, ServiceManagerOutput, ServiceOperation,
    ServicePlatform, inspect_service_descriptor, materialize_service_descriptor,
};

const RECEIPT_RECORD_SCHEMA: &str = "ae-sdd-service-execution-record/v1";
const RECEIPT_FILE: &str = "service-lifecycle.receipt.json";
const MAX_LIMIT_BYTES: usize = 1024 * 1024;
const MAX_TIMEOUT: Duration = Duration::from_secs(300);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Injectable boundary for executing one native service-manager command.
pub trait ServiceManagerRunner {
    /// Runs the exact program and argv under the supplied deadline and output budgets.
    fn run(
        &self,
        command: &ServiceManagerCommand,
        limits: ServiceExecutionLimits,
    ) -> Result<ServiceManagerOutput, ServiceError>;
}

/// Production runner that starts an allowlisted manager directly without a shell.
#[derive(Clone, Copy, Debug, Default)]
struct NativeServiceManagerRunner;

impl ServiceManagerRunner for NativeServiceManagerRunner {
    fn run(
        &self,
        command: &ServiceManagerCommand,
        limits: ServiceExecutionLimits,
    ) -> Result<ServiceManagerOutput, ServiceError> {
        validate_limits(limits)?;
        let planned = manager_platform(command.program)
            .ok_or_else(|| ServiceError::ManagerProgramDenied(command.program.to_owned()))?;
        if planned != ServicePlatform::current() {
            return Err(ServiceError::PlatformMismatch {
                planned,
                current: ServicePlatform::current(),
            });
        }
        validate_arguments(planned, &command.arguments)?;
        let started = Instant::now();
        let mut child = Command::new(command.program)
            .args(&command.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| ServiceError::ManagerIo {
                program: command.program.to_owned(),
                source,
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ServiceError::ManagerPipe {
                program: command.program.to_owned(),
                stream: "stdout",
            })?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ServiceError::ManagerPipe {
                program: command.program.to_owned(),
                stream: "stderr",
            })?;
        let max_bytes = limits.max_output_bytes;
        let stdout_reader = thread::spawn(move || drain_bounded(stdout, max_bytes));
        let stderr_reader = thread::spawn(move || drain_bounded(stderr, max_bytes));
        let deadline = started
            .checked_add(limits.command_timeout)
            .ok_or(ServiceError::InvalidExecutionLimits)?;
        let (status, timed_out) = loop {
            if let Some(status) = child.try_wait().map_err(|source| ServiceError::ManagerIo {
                program: command.program.to_owned(),
                source,
            })? {
                break (status, false);
            }
            let now = Instant::now();
            if now >= deadline {
                child.kill().map_err(|source| ServiceError::ManagerIo {
                    program: command.program.to_owned(),
                    source,
                })?;
                let status = child.wait().map_err(|source| ServiceError::ManagerIo {
                    program: command.program.to_owned(),
                    source,
                })?;
                break (status, true);
            }
            thread::sleep(POLL_INTERVAL.min(deadline.saturating_duration_since(now)));
        };
        let stdout = join_reader(stdout_reader, command.program, "stdout")?;
        let stderr = join_reader(stderr_reader, command.program, "stderr")?;
        Ok(ServiceManagerOutput {
            exit_code: status.code(),
            stdout,
            stderr,
            timed_out,
            elapsed_millis: millis(started.elapsed()),
        })
    }
}

/// Executes a validated lifecycle plan with the native service manager.
pub fn execute_service_lifecycle(
    plan: &ServiceLifecyclePlan,
) -> Result<ServiceExecutionReceipt, ServiceError> {
    execute_service_lifecycle_with_runner(
        plan,
        &NativeServiceManagerRunner,
        ServiceExecutionLimits::default(),
    )
}

/// Executes a lifecycle plan through an injectable runner for contract testing.
pub fn execute_service_lifecycle_with_runner(
    plan: &ServiceLifecyclePlan,
    runner: &dyn ServiceManagerRunner,
    limits: ServiceExecutionLimits,
) -> Result<ServiceExecutionReceipt, ServiceError> {
    validate_limits(limits)?;
    validate_plan(plan)?;
    validate_storage(plan)?;
    let descriptor_before = inspect_service_descriptor(plan)?.state;
    if plan.operation == ServiceOperation::Uninstall
        && descriptor_before == ServiceDescriptorState::Drifted
    {
        return Err(ServiceError::DescriptorDrift);
    }
    let plan_digest = execution_digest(plan);
    if plan.operation != ServiceOperation::Status
        && committed_replay(plan, &plan_digest, descriptor_before)?
    {
        return Ok(ServiceExecutionReceipt {
            schema_version: SERVICE_EXECUTION_SCHEMA,
            plan_digest,
            platform: plan.platform,
            operation: plan.operation,
            replayed: true,
            descriptor_before,
            descriptor_action: ServiceDescriptorAction::Replayed,
            commands: Vec::new(),
        });
    }

    let descriptor_action = if plan.operation == ServiceOperation::Install {
        let materialization = materialize_service_descriptor(plan)?;
        if materialization.created {
            ServiceDescriptorAction::Materialized
        } else {
            ServiceDescriptorAction::Revalidated
        }
    } else {
        ServiceDescriptorAction::None
    };
    let commands = execute_commands(plan, runner, limits)?;
    let descriptor_action = if plan.operation == ServiceOperation::Uninstall {
        if remove_service_descriptor(plan)? {
            ServiceDescriptorAction::Removed
        } else {
            ServiceDescriptorAction::AlreadyAbsent
        }
    } else {
        descriptor_action
    };
    let receipt = ServiceExecutionReceipt {
        schema_version: SERVICE_EXECUTION_SCHEMA,
        plan_digest: plan_digest.clone(),
        platform: plan.platform,
        operation: plan.operation,
        replayed: false,
        descriptor_before,
        descriptor_action,
        commands,
    };
    if plan.operation != ServiceOperation::Status {
        persist_commit(plan, &plan_digest)?;
    }
    Ok(receipt)
}

fn execute_commands(
    plan: &ServiceLifecyclePlan,
    runner: &dyn ServiceManagerRunner,
    limits: ServiceExecutionLimits,
) -> Result<Vec<ServiceCommandReceipt>, ServiceError> {
    let mut receipts = Vec::with_capacity(plan.manager_commands.len());
    for command in &plan.manager_commands {
        let output = runner.run(command, limits)?;
        let (stdout, stdout_truncated) = bounded_text(&output.stdout, limits.max_output_bytes);
        let (stderr, stderr_truncated) = bounded_text(&output.stderr, limits.max_output_bytes);
        if output.timed_out {
            return Err(ServiceError::ManagerTimedOut {
                purpose: command.purpose.to_owned(),
                timeout_millis: millis(limits.command_timeout),
                stderr,
            });
        }
        if output.exit_code != Some(0) {
            return Err(ServiceError::ManagerFailed {
                purpose: command.purpose.to_owned(),
                exit_code: output.exit_code,
                stderr,
            });
        }
        receipts.push(ServiceCommandReceipt {
            purpose: command.purpose.to_owned(),
            program: command.program.to_owned(),
            arguments: command.arguments.clone(),
            exit_code: output.exit_code,
            stdout,
            stderr,
            stdout_truncated,
            stderr_truncated,
            elapsed_millis: output.elapsed_millis,
        });
    }
    Ok(receipts)
}

fn validate_limits(limits: ServiceExecutionLimits) -> Result<(), ServiceError> {
    if limits.command_timeout.is_zero()
        || limits.command_timeout > MAX_TIMEOUT
        || limits.max_output_bytes == 0
        || limits.max_output_bytes > MAX_LIMIT_BYTES
    {
        return Err(ServiceError::InvalidExecutionLimits);
    }
    Ok(())
}

fn validate_plan(plan: &ServiceLifecyclePlan) -> Result<(), ServiceError> {
    if plan.platform != ServicePlatform::current() {
        return Err(ServiceError::PlatformMismatch {
            planned: plan.platform,
            current: ServicePlatform::current(),
        });
    }
    if !plan.permission_policy.user_scope_only
        || plan.permission_policy.elevation_required
        || plan.lifecycle_contract.shell_wrapper
        || plan.manager_commands.is_empty()
    {
        return Err(ServiceError::PrivilegeEscalation);
    }
    for command in &plan.manager_commands {
        if manager_platform(command.program) != Some(plan.platform) {
            return Err(ServiceError::ManagerProgramDenied(
                command.program.to_owned(),
            ));
        }
        validate_arguments(plan.platform, &command.arguments)?;
    }
    Ok(())
}

fn manager_platform(program: &str) -> Option<ServicePlatform> {
    match program {
        "schtasks.exe" => Some(ServicePlatform::Windows),
        "launchctl" => Some(ServicePlatform::Macos),
        "systemctl" => Some(ServicePlatform::Linux),
        _ => None,
    }
}

fn validate_arguments(platform: ServicePlatform, arguments: &[String]) -> Result<(), ServiceError> {
    if arguments.iter().any(|argument| {
        argument.is_empty() || argument.len() > 16 * 1024 || argument.contains(['\0', '\n', '\r'])
    }) {
        return Err(ServiceError::InvalidManagerArguments);
    }
    let lowered = arguments
        .iter()
        .map(|argument| argument.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if lowered.iter().any(|argument| {
        matches!(argument.as_str(), "sudo" | "doas" | "pkexec" | "system")
            || argument.starts_with("system/")
            || matches!(argument.as_str(), "/ru" | "/rl" | "highest")
    }) {
        return Err(ServiceError::PrivilegeEscalation);
    }
    if platform == ServicePlatform::Linux && !lowered.iter().any(|value| value == "--user") {
        return Err(ServiceError::PrivilegeEscalation);
    }
    Ok(())
}

fn validate_storage(plan: &ServiceLifecyclePlan) -> Result<(), ServiceError> {
    let home = plan
        .user_home
        .canonicalize()
        .map_err(|source| ServiceError::Io {
            path: plan.user_home.clone(),
            source,
        })?;
    ensure_destination(&home, &plan.state_dir)?;
    ensure_destination(&home, &plan.descriptor_path)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CommitRecord {
    schema_version: String,
    plan_digest: String,
    platform: ServicePlatform,
    operation: ServiceOperation,
    descriptor_digest: String,
}

fn committed_replay(
    plan: &ServiceLifecyclePlan,
    plan_digest: &str,
    descriptor_state: ServiceDescriptorState,
) -> Result<bool, ServiceError> {
    let path = receipt_path(plan);
    let Some(bytes) = read_descriptor(&path)? else {
        return Ok(false);
    };
    let record: CommitRecord =
        serde_json::from_slice(&bytes).map_err(ServiceError::InvalidExecutionReceipt)?;
    if record.schema_version != RECEIPT_RECORD_SCHEMA {
        return Err(ServiceError::ExecutionReceiptSchema(record.schema_version));
    }
    let terminal_matches = match plan.operation {
        ServiceOperation::Install => descriptor_state == ServiceDescriptorState::Matches,
        ServiceOperation::Uninstall => descriptor_state == ServiceDescriptorState::Absent,
        ServiceOperation::Status => false,
    };
    Ok(record.plan_digest == plan_digest
        && record.platform == plan.platform
        && record.operation == plan.operation
        && record.descriptor_digest == plan.descriptor_digest
        && terminal_matches)
}

fn persist_commit(plan: &ServiceLifecyclePlan, plan_digest: &str) -> Result<(), ServiceError> {
    let record = CommitRecord {
        schema_version: RECEIPT_RECORD_SCHEMA.to_owned(),
        plan_digest: plan_digest.to_owned(),
        platform: plan.platform,
        operation: plan.operation,
        descriptor_digest: plan.descriptor_digest.clone(),
    };
    let bytes = serde_json::to_vec(&record).map_err(ServiceError::InvalidExecutionReceipt)?;
    let home = plan
        .user_home
        .canonicalize()
        .map_err(|source| ServiceError::Io {
            path: plan.user_home.clone(),
            source,
        })?;
    ensure_destination(&home, &plan.state_dir)?;
    create_private_directory(&plan.state_dir, plan)?;
    let path = receipt_path(plan);
    ensure_destination(&home, &path)?;
    atomic_write(&path, &bytes)?;
    protect_private_file(&path, plan)
}

fn receipt_path(plan: &ServiceLifecyclePlan) -> std::path::PathBuf {
    plan.state_dir.join(RECEIPT_FILE)
}

fn execution_digest(plan: &ServiceLifecyclePlan) -> String {
    let mut hasher = Sha256::new();
    hash_part(&mut hasher, plan.platform.as_str().as_bytes());
    hash_part(&mut hasher, plan.operation.as_str().as_bytes());
    hash_part(&mut hasher, plan.service_name.as_bytes());
    hash_part(&mut hasher, plan.descriptor_digest.as_bytes());
    for command in &plan.manager_commands {
        hash_part(&mut hasher, command.purpose.as_bytes());
        hash_part(&mut hasher, command.program.as_bytes());
        for argument in &command.arguments {
            hash_part(&mut hasher, argument.as_bytes());
        }
    }
    hex::encode(hasher.finalize())
}

fn hash_part(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
}

fn bounded_text(bytes: &[u8], max_bytes: usize) -> (String, bool) {
    let retained = &bytes[..bytes.len().min(max_bytes)];
    let mut text = String::from_utf8_lossy(retained).into_owned();
    let expanded = text.len() > max_bytes;
    if expanded {
        let mut boundary = max_bytes;
        while boundary > 0 && !text.is_char_boundary(boundary) {
            boundary -= 1;
        }
        text.truncate(boundary);
    }
    (text, bytes.len() > max_bytes || expanded)
}

fn drain_bounded(mut reader: impl Read, max_bytes: usize) -> io::Result<Vec<u8>> {
    let retained_limit = max_bytes.saturating_add(1);
    let mut retained = Vec::with_capacity(retained_limit.min(8 * 1024));
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            return Ok(retained);
        }
        let available = retained_limit.saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..count.min(available)]);
    }
}

fn join_reader(
    reader: thread::JoinHandle<io::Result<Vec<u8>>>,
    program: &str,
    stream: &'static str,
) -> Result<Vec<u8>, ServiceError> {
    reader
        .join()
        .map_err(|_| ServiceError::ManagerReaderPanicked {
            program: program.to_owned(),
            stream,
        })?
        .map_err(|source| ServiceError::ManagerIo {
            program: program.to_owned(),
            source,
        })
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{
        NativeServiceManagerRunner, ServiceExecutionLimits, ServiceManagerCommand,
        ServiceManagerRunner, bounded_text,
    };

    #[test]
    fn invalid_utf8_cannot_expand_beyond_the_receipt_budget() {
        let (text, truncated) = bounded_text(&[0xff, 0xff, 0xff], 2);
        assert!(truncated);
        assert!(text.len() <= 2);
    }

    #[test]
    fn native_runner_rejects_non_allowlisted_program_before_spawn() {
        let error = NativeServiceManagerRunner
            .run(
                &ServiceManagerCommand {
                    purpose: "negative",
                    program: "powershell.exe",
                    arguments: vec!["-Command".to_owned()],
                },
                ServiceExecutionLimits::default(),
            )
            .expect_err("non-allowlisted manager must be rejected");
        assert!(matches!(
            error,
            super::ServiceError::ManagerProgramDenied(_)
        ));
    }
}
