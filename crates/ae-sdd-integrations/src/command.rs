use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ae_sdd_domain::HostAckId;
use ae_sdd_host::{
    HostAck, HostAckOutcome, HostAction, HostActionKind, HostAdapterError, HostAdapterId,
    HostRuntimeAdapter,
};
use uuid::Uuid;

use crate::{IntegrationError, IntegrationResult};

/// Host environment variables ever forwarded to a spawned child. Everything
/// else (credentials, tokens, unrelated user configuration) is stripped
/// before spawn, matching `constraints/security.md` §四's allowlist
/// requirement. Mirrors the equivalent list already verified for the daemon
/// launcher in `bins/ae-sdd-cli/src/bootstrap.rs`.
const SAFE_ENVIRONMENT_KEYS: &[&str] = &[
    "APPDATA",
    "CARGO_HOME",
    "COMSPEC",
    "HOME",
    "HOMEDRIVE",
    "HOMEPATH",
    "LANG",
    "LOCALAPPDATA",
    "NUMBER_OF_PROCESSORS",
    "OS",
    "PATH",
    "PATHEXT",
    "PROCESSOR_ARCHITECTURE",
    "PROCESSOR_IDENTIFIER",
    "PROGRAMDATA",
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "PROGRAMW6432",
    "RUST_BACKTRACE",
    "RUST_LOG",
    "RUSTUP_HOME",
    "SYSTEMDRIVE",
    "SYSTEMROOT",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "USERNAME",
    "USERDOMAIN",
    "WINDIR",
];

fn is_safe_environment_key(key: &std::ffi::OsStr) -> bool {
    let key = key.to_string_lossy();
    SAFE_ENVIRONMENT_KEYS
        .iter()
        .any(|allowed| key.eq_ignore_ascii_case(allowed))
}

/// Clears the inherited host environment and re-admits only allowlisted
/// keys, so unrelated secrets/tokens in the caller's process never reach a
/// spawned child.
fn apply_environment_allowlist(command: &mut Command) {
    command.env_clear();
    for (key, value) in std::env::vars_os().filter(|(key, _)| is_safe_environment_key(key)) {
        command.env(key, value);
    }
}

/// Hides the console window and isolates the child into its own process
/// group/job so a deadline timeout can clean up the whole subtree, not just
/// the immediate child. `CREATE_NO_WINDOW` mirrors the constant already
/// verified for the daemon launcher; `CREATE_NEW_PROCESS_GROUP` gives the
/// timeout path a stable process-group root to target with `taskkill /T`.
#[cfg(windows)]
fn apply_platform_hardening(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
}

/// Isolates the child into its own POSIX process group so a deadline
/// timeout can signal the whole subtree via the group, not just the
/// immediate child.
#[cfg(unix)]
fn apply_platform_hardening(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

/// Best-effort process-tree cleanup after a deadline overrun. `child.kill()`
/// only terminates the immediate process; grandchildren spawned by it (a
/// shell, a build tool wrapper) would otherwise keep running past the
/// caller's deadline. On Windows, `taskkill /T` walks the process tree from
/// the child's PID. On Unix, signalling the negated PID reaches the whole
/// process group `process_group(0)` created for the child at spawn time.
/// Both cleanup commands are themselves program+args invocations (never a
/// shell), consistent with `constraints/security.md` §四.
fn kill_process_tree(child: &mut Child) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill.exe")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .env_clear()
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-KILL", &format!("-{}", child.id())])
            .env_clear()
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

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
        apply_environment_allowlist(&mut command);
        apply_platform_hardening(&mut command);
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
                kill_process_tree(&mut child);
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
    executable: PathBuf,
    runner: BoundedCommandRunner,
}

impl HostProcessAdapter {
    /// Creates a host process adapter.
    #[must_use]
    pub fn new(
        adapter_id: HostAdapterId,
        executable: impl Into<PathBuf>,
        runner: BoundedCommandRunner,
    ) -> Self {
        Self {
            adapter_id,
            executable: executable.into(),
            runner,
        }
    }
}

impl HostRuntimeAdapter for HostProcessAdapter {
    fn adapter_id(&self) -> &HostAdapterId {
        &self.adapter_id
    }

    fn dispatch(&self, action: &HostAction) -> Result<HostAck, HostAdapterError> {
        let arguments = vec![
            action_verb(action.kind()).to_owned(),
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
        let outcome = if output.exit_code == Some(0) {
            HostAckOutcome::Accepted
        } else {
            HostAckOutcome::Rejected {
                error_code: "host process rejected action".into(),
            }
        };
        // `HostAck::new`'s only failure mode is a zero command_seq, which
        // `action.command_seq()` cannot produce: `HostAction::new` already
        // rejects zero at construction time.
        Ok(HostAck::new(
            HostAckId::from_uuid(Uuid::new_v4()),
            action.action_id(),
            action.adapter_id().clone(),
            action.command_seq(),
            outcome,
            None,
            action.session_id(),
        )
        .expect("HostAction guarantees a non-zero command_seq"))
    }
}

/// Subcommand the host executable is invoked with for an errand.
///
/// This used to be derived from the capability enum, which quietly made that
/// enum serve two unrelated purposes: gating dispatch and naming the CLI verb.
/// Only the verb was ever load-bearing.
const fn action_verb(value: HostActionKind) -> &'static str {
    match value {
        HostActionKind::Create => "create",
        HostActionKind::Send => "send",
        HostActionKind::Wait => "wait",
        HostActionKind::Cancel => "cancel",
        HostActionKind::Attest => "attest",
        HostActionKind::Compact => "compact",
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

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::{SAFE_ENVIRONMENT_KEYS, is_safe_environment_key};

    #[test]
    fn safe_environment_keys_match_case_insensitively() {
        assert!(is_safe_environment_key(OsStr::new("PATH")));
        assert!(is_safe_environment_key(OsStr::new("path")));
        assert!(is_safe_environment_key(OsStr::new("Path")));
    }

    #[test]
    fn secrets_and_unrelated_variables_are_not_allowlisted() {
        for leaked in [
            "AWS_SECRET_ACCESS_KEY",
            "GITHUB_TOKEN",
            "OPENAI_API_KEY",
            "SSH_AUTH_SOCK",
            "AE_SDD_ENDPOINT_TOKEN",
            "npm_config__auth",
        ] {
            assert!(
                !is_safe_environment_key(OsStr::new(leaked)),
                "{leaked} must not be forwarded to a spawned child"
            );
        }
    }

    #[test]
    fn allowlist_has_no_duplicate_keys() {
        let mut seen = std::collections::HashSet::new();
        for key in SAFE_ENVIRONMENT_KEYS {
            assert!(
                seen.insert(key.to_ascii_uppercase()),
                "{key} is listed more than once"
            );
        }
    }
}
