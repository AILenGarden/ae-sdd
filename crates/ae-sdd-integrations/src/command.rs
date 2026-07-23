use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ae_sdd_host::{
    HostAction, HostAdapterError, HostAdapterId, HostCapability, HostCapabilitySet,
    HostRuntimeAdapter,
};

use crate::{IntegrationError, IntegrationResult};

/// Bounded output from one typed external process invocation.
#[derive(Clone, Debug)]
pub struct BoundedCommandOutput {
    /// Process exit code; `None` when terminated by the OS.
    pub exit_code: Option<i32>,
    /// Captured standard output.
    pub stdout: Vec<u8>,
    /// Captured standard error.
    pub stderr: Vec<u8>,
}

/// Shell-free command runner with deadline and output bounds.
#[derive(Clone, Debug)]
pub struct BoundedCommandRunner {
    maximum_output_bytes: usize,
}

impl BoundedCommandRunner {
    /// Creates a runner with one combined per-stream bound.
    #[must_use]
    pub fn new(maximum_output_bytes: usize) -> Self {
        Self {
            maximum_output_bytes: maximum_output_bytes.max(1),
        }
    }

    /// Runs an executable directly; arguments never pass through a shell.
    pub fn run(
        &self,
        program: &Path,
        arguments: &[String],
        current_dir: Option<&Path>,
        deadline: Duration,
    ) -> IntegrationResult<BoundedCommandOutput> {
        let mut command = Command::new(program);
        command
            .args(arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(directory) = current_dir {
            command.current_dir(directory);
        }
        let mut child = command.spawn()?;
        let stdout = child.stdout.take().ok_or_else(|| {
            IntegrationError::Io(std::io::Error::other("stdout pipe is unavailable"))
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            IntegrationError::Io(std::io::Error::other("stderr pipe is unavailable"))
        })?;
        let (sender, receiver) = mpsc::channel();
        let maximum = self.maximum_output_bytes;
        let sender_stdout = sender.clone();
        std::thread::spawn(move || {
            let _ = sender_stdout.send((true, read_bounded(stdout, maximum)));
        });
        std::thread::spawn(move || {
            let _ = sender.send((false, read_bounded(stderr, maximum)));
        });

        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if started.elapsed() >= deadline {
                child.kill()?;
                let _ = child.wait();
                return Err(IntegrationError::CommandTimeout);
            }
            std::thread::sleep(Duration::from_millis(1));
        };
        let mut stdout = None;
        let mut stderr = None;
        for _ in 0..2 {
            let (is_stdout, bytes) = receiver.recv().map_err(|_| {
                IntegrationError::Io(std::io::Error::other("output reader stopped"))
            })?;
            let bytes = bytes?;
            if is_stdout {
                stdout = Some(bytes);
            } else {
                stderr = Some(bytes);
            }
        }
        Ok(BoundedCommandOutput {
            exit_code: status.code(),
            stdout: stdout.unwrap_or_default(),
            stderr: stderr.unwrap_or_default(),
        })
    }
}

/// Git executable adapter with shell-free arguments.
#[derive(Clone, Debug)]
pub struct GitAdapter {
    runner: BoundedCommandRunner,
    executable: PathBuf,
}

impl GitAdapter {
    /// Creates a Git adapter.
    #[must_use]
    pub fn new(runner: BoundedCommandRunner, executable: impl Into<PathBuf>) -> Self {
        Self {
            runner,
            executable: executable.into(),
        }
    }

    /// Runs an exact Git argument vector in a repository.
    pub fn run(
        &self,
        repository: &Path,
        arguments: &[String],
        deadline: Duration,
    ) -> IntegrationResult<BoundedCommandOutput> {
        self.runner
            .run(&self.executable, arguments, Some(repository), deadline)
    }
}

/// Generic toolchain/test-runner adapter.
#[derive(Clone, Debug)]
pub struct ToolchainAdapter {
    runner: BoundedCommandRunner,
}

impl ToolchainAdapter {
    /// Creates a toolchain adapter.
    #[must_use]
    pub const fn new(runner: BoundedCommandRunner) -> Self {
        Self { runner }
    }

    /// Runs a configured executable directly.
    pub fn run(
        &self,
        executable: &Path,
        arguments: &[String],
        directory: &Path,
        deadline: Duration,
    ) -> IntegrationResult<BoundedCommandOutput> {
        self.runner
            .run(executable, arguments, Some(directory), deadline)
    }
}

/// Typed user-service lifecycle adapter.
#[derive(Clone, Debug)]
pub struct ServiceAdapter {
    runner: BoundedCommandRunner,
}

impl ServiceAdapter {
    /// Creates a service adapter.
    #[must_use]
    pub const fn new(runner: BoundedCommandRunner) -> Self {
        Self { runner }
    }

    /// Installs the daemon in the native per-user service manager.
    pub fn install(&self, executable: &Path) -> IntegrationResult<BoundedCommandOutput> {
        platform_service(&self.runner, "install", executable)
    }

    /// Removes the daemon from the native per-user service manager.
    pub fn uninstall(&self, executable: &Path) -> IntegrationResult<BoundedCommandOutput> {
        platform_service(&self.runner, "uninstall", executable)
    }
}

/// Host process adapter with a fixed executable and capability matrix.
pub struct HostProcessAdapter {
    adapter_id: HostAdapterId,
    capabilities: HostCapabilitySet,
    executable: PathBuf,
    runner: BoundedCommandRunner,
}

impl HostProcessAdapter {
    /// Creates a host process adapter.
    #[must_use]
    pub fn new(
        adapter_id: HostAdapterId,
        capabilities: HostCapabilitySet,
        executable: impl Into<PathBuf>,
        runner: BoundedCommandRunner,
    ) -> Self {
        Self {
            adapter_id,
            capabilities,
            executable: executable.into(),
            runner,
        }
    }
}

impl HostRuntimeAdapter for HostProcessAdapter {
    fn adapter_id(&self) -> &HostAdapterId {
        &self.adapter_id
    }

    fn capabilities(&self) -> &HostCapabilitySet {
        &self.capabilities
    }

    fn dispatch(&self, action: &HostAction) -> Result<(), HostAdapterError> {
        if !self
            .capabilities
            .supports(action.kind().required_capability())
        {
            return Err(HostAdapterError::Unsupported(
                action.kind().required_capability(),
            ));
        }
        let arguments = vec![
            capability_name(action.kind().required_capability()).to_owned(),
            "--action-id".to_owned(),
            action.action_id().to_string(),
            "--command-seq".to_owned(),
            action.command_seq().to_string(),
        ];
        let output = self
            .runner
            .run(&self.executable, &arguments, None, Duration::from_secs(30))
            .map_err(|error| match error {
                IntegrationError::CommandTimeout => HostAdapterError::Timeout,
                _ => HostAdapterError::Unavailable,
            })?;
        if output.exit_code == Some(0) {
            Ok(())
        } else {
            Err(HostAdapterError::Rejected(
                "host process rejected action".into(),
            ))
        }
    }
}

const fn capability_name(value: HostCapability) -> &'static str {
    match value {
        HostCapability::Create => "create",
        HostCapability::Send => "send",
        HostCapability::Wait => "wait",
        HostCapability::Cancel => "cancel",
        HostCapability::Attest => "attest",
        HostCapability::Compact => "compact",
        HostCapability::PressureTelemetry => "pressure",
    }
}

fn read_bounded(mut reader: impl Read, maximum: usize) -> IntegrationResult<Vec<u8>> {
    let mut bytes = Vec::with_capacity(maximum.min(8_192));
    reader
        .by_ref()
        .take(u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(IntegrationError::CommandOutputTooLarge);
    }
    Ok(bytes)
}

#[cfg(windows)]
fn platform_service(
    runner: &BoundedCommandRunner,
    operation: &str,
    executable: &Path,
) -> IntegrationResult<BoundedCommandOutput> {
    let task = "ae-sdd-daemon".to_owned();
    let arguments = if operation == "install" {
        vec![
            "/Create".to_owned(),
            "/F".to_owned(),
            "/SC".to_owned(),
            "ONLOGON".to_owned(),
            "/TN".to_owned(),
            task,
            "/TR".to_owned(),
            format!("\"{}\" serve", executable.display()),
        ]
    } else {
        vec![
            "/Delete".to_owned(),
            "/F".to_owned(),
            "/TN".to_owned(),
            task,
        ]
    };
    runner.run(
        Path::new("schtasks.exe"),
        &arguments,
        None,
        Duration::from_secs(30),
    )
}

#[cfg(target_os = "linux")]
fn platform_service(
    runner: &BoundedCommandRunner,
    operation: &str,
    _executable: &Path,
) -> IntegrationResult<BoundedCommandOutput> {
    let arguments = vec![
        "--user".to_owned(),
        if operation == "install" {
            "enable"
        } else {
            "disable"
        }
        .to_owned(),
        "--now".to_owned(),
        "ae-sdd.service".to_owned(),
    ];
    runner.run(
        Path::new("systemctl"),
        &arguments,
        None,
        Duration::from_secs(30),
    )
}

#[cfg(target_os = "macos")]
fn platform_service(
    runner: &BoundedCommandRunner,
    operation: &str,
    executable: &Path,
) -> IntegrationResult<BoundedCommandOutput> {
    let arguments = vec![
        if operation == "install" {
            "bootstrap"
        } else {
            "bootout"
        }
        .to_owned(),
        format!("gui/{}", std::process::id()),
        executable.to_string_lossy().into_owned(),
    ];
    runner.run(
        Path::new("launchctl"),
        &arguments,
        None,
        Duration::from_secs(30),
    )
}
