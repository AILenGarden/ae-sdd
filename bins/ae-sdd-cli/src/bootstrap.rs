//! Race-safe bootstrap for the per-user ae-sdd daemon.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
#[cfg(windows)]
use std::ptr;

use ae_sdd_client::{ClientError, DaemonClient, LocalIpcTransport, default_state_dir};
use ae_sdd_contracts::session::{SessionBootstrapRequest, SessionContractError};
use ae_sdd_contracts::{AdapterId, ContextBundleId, ExternalSessionKey, SchemaVersion};
use ae_sdd_domain::{AgentRole, CapabilityId, DelegationId, WorkspaceId};
use ae_sdd_protocol::{ClientKind, PROTOCOL_VERSION_V1, RequestParams, RpcMethod};
use fs4::{FileExt, TryLockError};
use serde::Serialize;
use serde_json::{Value, json};
#[cfg(unix)]
use std::process::Stdio;
use thiserror::Error;
#[cfg(unix)]
use tokio::process::{Child, Command};

#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, FALSE, GetLastError};
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{
    CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW,
    CREATE_UNICODE_ENVIRONMENT, CreateProcessW, GetExitCodeProcess, PROCESS_INFORMATION,
    STARTUPINFOW, TerminateProcess, WaitForSingleObject,
};

const ENDPOINT_MANIFEST_NAME: &str = "endpoint.v1.json";
const BOOTSTRAP_LOCK_NAME: &str = "bootstrap.lock";
const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(20);
const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_PROBE_TIMEOUT: Duration = Duration::from_millis(500);

#[cfg(windows)]
struct DaemonChild {
    process: OwnedHandle,
}

#[cfg(unix)]
type DaemonChild = Child;

/// Inputs used to find, probe, or start the per-user daemon.
///
/// Allowed roots are the canonicalized union of `allowed_roots`,
/// `AE_SDD_ALLOWED_ROOTS`, `AE_SDD_WORKSPACE_ROOT`, and `project_root`. When no
/// root is configured at all, the current directory is used as the single
/// default root; an explicit root list never silently gains the caller's CWD.
#[derive(Clone, Debug)]
pub struct BootstrapOptions {
    /// Daemon executable override; the sibling `ae-sddd` is used when absent.
    pub daemon: Option<PathBuf>,
    /// Per-user daemon state directory override.
    pub state_dir: Option<PathBuf>,
    /// Protected endpoint manifest override.
    pub manifest: Option<PathBuf>,
    /// Explicit parent roots from which workspaces may be registered.
    pub allowed_roots: Vec<PathBuf>,
    /// Current project root, falling back to the process current directory.
    pub project_root: Option<PathBuf>,
    /// Optional 64-character lowercase policy digest forwarded to the daemon.
    pub policy_digest: Option<String>,
    /// Total bound for lock acquisition and daemon readiness.
    pub timeout: Duration,
    /// Bound for each authenticated `runtime.status` probe.
    pub probe_timeout: Duration,
    /// Handshake profile used by the caller that requested bootstrap.
    pub client_kind: ClientKind,
}

impl Default for BootstrapOptions {
    fn default() -> Self {
        Self {
            daemon: None,
            state_dir: None,
            manifest: None,
            allowed_roots: Vec::new(),
            project_root: None,
            policy_digest: None,
            timeout: DEFAULT_STARTUP_TIMEOUT,
            probe_timeout: DEFAULT_PROBE_TIMEOUT,
            client_kind: ClientKind::Cli,
        }
    }
}

/// Whether this call reused a ready daemon or spawned the daemon it returned.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapDisposition {
    /// The initial or lock-internal status probe found a ready daemon.
    Reused,
    /// This call spawned `ae-sddd serve` and observed it become ready.
    Started,
}

/// Authenticated daemon status returned by [`ensure_daemon`].
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapResult {
    /// Raw `runtime.status` JSON, preserved for direct CLI output.
    pub status: Value,
    /// Whether the daemon was started or reused.
    pub disposition: BootstrapDisposition,
}

impl BootstrapResult {
    /// Returns true when this call spawned the ready daemon.
    #[must_use]
    pub const fn started(&self) -> bool {
        matches!(self.disposition, BootstrapDisposition::Started)
    }
}

/// Failure to find or bootstrap a trustworthy local daemon.
#[derive(Debug, Error)]
pub enum BootstrapError {
    /// An authenticated client failure that is not evidence of absence.
    #[error(transparent)]
    Client(#[from] ClientError),
    /// A filesystem operation failed.
    #[error("failed to {action} {path}: {source}", path = .path.display())]
    Io {
        /// Stable operation description.
        action: &'static str,
        /// Affected path.
        path: PathBuf,
        /// Original I/O failure.
        #[source]
        source: io::Error,
    },
    /// The process executable has no parent directory.
    #[error("CLI executable has no parent directory")]
    ExecutableParent,
    /// The manifest override cannot be served by the selected state directory.
    #[error(
        "daemon startup requires manifest {expected}, but the configured manifest is {manifest}",
        expected = .expected.display(),
        manifest = .manifest.display()
    )]
    ManifestStateMismatch {
        /// Configured manifest path.
        manifest: PathBuf,
        /// Fixed manifest path published by `ae-sddd serve`.
        expected: PathBuf,
    },
    /// A ready daemon does not match an explicitly selected policy identity.
    #[error("ready daemon policy digest mismatch: expected {expected}, observed {observed}")]
    PolicyDigestMismatch {
        /// Explicit digest requested by the caller.
        expected: String,
        /// Digest projected by `runtime.status`, or a missing/invalid marker.
        observed: String,
    },
    /// An allowed workspace root exists but is not a directory.
    #[error("allowed workspace root is not a directory: {path}", path = .0.display())]
    AllowedRootNotDirectory(PathBuf),
    /// The total startup bound elapsed.
    #[error("daemon did not become ready before the {timeout_ms} ms startup deadline")]
    StartupTimeout {
        /// Configured total startup bound in milliseconds.
        timeout_ms: u128,
    },
    /// The spawned daemon exited before authenticated readiness.
    #[error("spawned daemon exited before readiness with status {status}")]
    DaemonExited {
        /// Child exit status.
        status: ExitStatus,
    },
    /// The configured duration cannot be represented by an `Instant` deadline.
    #[error("daemon startup timeout is too large")]
    InvalidTimeout,
}

/// Returns authenticated `runtime.status`, starting the local daemon exactly
/// once when it is absent.
///
/// Only [`ClientError::EndpointManifest`] and
/// [`ClientError::DaemonUnavailable`] are absence signals. Protocol failures
/// and daemon errors are returned unchanged and never hidden by a restart.
pub async fn ensure_daemon(options: BootstrapOptions) -> Result<BootstrapResult, BootstrapError> {
    let configured_timeout = options.timeout.max(Duration::from_millis(1));
    let started_at = Instant::now();
    let deadline = started_at
        .checked_add(configured_timeout)
        .ok_or(BootstrapError::InvalidTimeout)?;
    let paths = ResolvedPaths::new(&options)?;

    match probe_status(&paths.manifest, &options, deadline).await? {
        Probe::Ready(status) => {
            return ready_result(status, BootstrapDisposition::Reused, &options);
        }
        Probe::Unavailable => {}
    }

    let lock = open_bootstrap_lock(&paths.bootstrap_lock)?;
    if let Some(status) = acquire_bootstrap_lock(&lock, &paths.manifest, &options, deadline).await?
    {
        return ready_result(status, BootstrapDisposition::Reused, &options);
    }

    // This probe must happen while the bootstrap lock is held. Another Agent
    // may have completed startup between the optimistic probe and lock entry.
    match probe_status(&paths.manifest, &options, deadline).await? {
        Probe::Ready(status) => {
            return ready_result(status, BootstrapDisposition::Reused, &options);
        }
        Probe::Unavailable => {}
    }

    paths.validate_start_manifest()?;
    ensure_before_deadline(deadline, configured_timeout)?;
    let allowed_roots = canonical_allowed_roots(&options)?;
    ensure_before_deadline(deadline, configured_timeout)?;
    let mut child = spawn_daemon(&options, &paths.state_dir, &allowed_roots)?;
    let mut child_exit = None;

    let result = loop {
        let probe = match probe_status(&paths.manifest, &options, deadline).await {
            Ok(probe) => probe,
            Err(BootstrapError::StartupTimeout { .. }) if child_exit.is_some() => {
                break Err(BootstrapError::DaemonExited {
                    status: child_exit.expect("checked child exit"),
                });
            }
            Err(error) => break Err(error),
        };
        match probe {
            Probe::Ready(status) => {
                let disposition = if child_exit.is_some() {
                    BootstrapDisposition::Reused
                } else {
                    BootstrapDisposition::Started
                };
                break ready_result(status, disposition, &options);
            }
            Probe::Unavailable => {}
        }

        if child_exit.is_none() {
            child_exit = match child_try_wait(&mut child) {
                Ok(status) => status,
                Err(source) => {
                    break Err(BootstrapError::Io {
                        action: "inspect daemon process",
                        path: options.daemon.clone().unwrap_or_default(),
                        source,
                    });
                }
            };
        }

        if let Err(error) = sleep_for_poll(deadline, configured_timeout).await {
            break match child_exit {
                Some(status) => Err(BootstrapError::DaemonExited { status }),
                None => Err(error),
            };
        }
    };
    if result.is_err()
        && child_exit.is_none()
        && let Err(source) = terminate_child(&mut child).await
    {
        return Err(BootstrapError::Io {
            action: "terminate failed daemon process",
            path: options.daemon.clone().unwrap_or_default(),
            source,
        });
    }
    result
}

fn ready_result(
    status: Value,
    disposition: BootstrapDisposition,
    options: &BootstrapOptions,
) -> Result<BootstrapResult, BootstrapError> {
    if let Some(expected) = &options.policy_digest {
        let observed = status.get("policyDigest").and_then(Value::as_str);
        if observed != Some(expected.as_str()) {
            return Err(BootstrapError::PolicyDigestMismatch {
                expected: expected.clone(),
                observed: observed.unwrap_or("<missing-or-invalid>").to_owned(),
            });
        }
    }
    Ok(BootstrapResult {
        status,
        disposition,
    })
}

#[derive(Debug)]
struct ResolvedPaths {
    state_dir: PathBuf,
    manifest: PathBuf,
    bootstrap_lock: PathBuf,
}

impl ResolvedPaths {
    fn new(options: &BootstrapOptions) -> Result<Self, BootstrapError> {
        let current_dir = std::env::current_dir().map_err(|source| BootstrapError::Io {
            action: "read the current directory",
            path: PathBuf::from("."),
            source,
        })?;
        let raw_state_dir = match (&options.state_dir, &options.manifest) {
            (Some(state_dir), _) => state_dir.clone(),
            (None, Some(manifest)) => manifest.parent().map(Path::to_path_buf).unwrap_or_default(),
            (None, None) => default_state_dir()?,
        };
        let raw_manifest = options
            .manifest
            .clone()
            .unwrap_or_else(|| raw_state_dir.join(ENDPOINT_MANIFEST_NAME));
        let state_dir = absolute(raw_state_dir, &current_dir);
        std::fs::create_dir_all(&state_dir).map_err(|source| BootstrapError::Io {
            action: "create daemon state directory",
            path: state_dir.clone(),
            source,
        })?;
        let state_dir = std::fs::canonicalize(&state_dir).map_err(|source| BootstrapError::Io {
            action: "canonicalize daemon state directory",
            path: state_dir,
            source,
        })?;
        let manifest = if options.manifest.is_some() {
            canonicalize_parent(absolute(raw_manifest, &current_dir))?
        } else {
            state_dir.join(ENDPOINT_MANIFEST_NAME)
        };
        let bootstrap_lock = state_dir.join(BOOTSTRAP_LOCK_NAME);
        Ok(Self {
            state_dir,
            manifest,
            bootstrap_lock,
        })
    }

    fn validate_start_manifest(&self) -> Result<(), BootstrapError> {
        let expected = self.state_dir.join(ENDPOINT_MANIFEST_NAME);
        if self.manifest == expected {
            Ok(())
        } else {
            Err(BootstrapError::ManifestStateMismatch {
                manifest: self.manifest.clone(),
                expected,
            })
        }
    }
}

enum Probe {
    Ready(Value),
    Unavailable,
}

async fn probe_status(
    manifest: &Path,
    options: &BootstrapOptions,
    deadline: Instant,
) -> Result<Probe, BootstrapError> {
    let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
        return Err(timeout_error(options.timeout));
    };
    if remaining.is_zero() {
        return Err(timeout_error(options.timeout));
    }
    let probe_timeout = options
        .probe_timeout
        .max(Duration::from_millis(1))
        .min(remaining);
    let deadline_ms = duration_millis_u64(probe_timeout);
    let client = DaemonClient::new(
        manifest,
        options.client_kind,
        Arc::new(LocalIpcTransport),
        probe_timeout,
    );
    let params = RequestParams {
        protocol_version: PROTOCOL_VERSION_V1.to_owned(),
        workspace_id: None,
        agent_id: None,
        session_id: None,
        capability_token: None,
        turn_id: None,
        work_item_id: None,
        lease_id: None,
        fencing_token: None,
        expected_revision: None,
        idempotency_key: None,
        confirmation: None,
        deadline_ms,
        payload: json!({}),
    };
    let call =
        call_with_timeout(probe_timeout, client.call(RpcMethod::RuntimeStatus, params)).await;
    match call {
        Ok(status) => Ok(Probe::Ready(status)),
        Err(error) if is_unavailable(&error) => Ok(Probe::Unavailable),
        Err(error) => Err(BootstrapError::Client(error)),
    }
}

async fn call_with_timeout<T, F>(timeout: Duration, call: F) -> Result<T, ClientError>
where
    F: Future<Output = Result<T, ClientError>>,
{
    tokio::time::timeout(timeout, call)
        .await
        .unwrap_or(Err(ClientError::DaemonUnavailable))
}

fn is_unavailable(error: &ClientError) -> bool {
    matches!(
        error,
        ClientError::EndpointManifest | ClientError::DaemonUnavailable
    )
}

fn open_bootstrap_lock(path: &Path) -> Result<File, BootstrapError> {
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|source| BootstrapError::Io {
            action: "open daemon bootstrap lock",
            path: path.to_path_buf(),
            source,
        })
}

async fn acquire_bootstrap_lock(
    lock: &File,
    manifest: &Path,
    options: &BootstrapOptions,
    deadline: Instant,
) -> Result<Option<Value>, BootstrapError> {
    loop {
        match FileExt::try_lock(lock) {
            Ok(()) => return Ok(None),
            Err(TryLockError::WouldBlock) => {
                // Avoid waiting behind a slow starter once its authenticated
                // endpoint is already ready.
                if let Probe::Ready(status) = probe_status(manifest, options, deadline).await? {
                    return Ok(Some(status));
                }
                sleep_for_poll(deadline, options.timeout).await?;
            }
            Err(TryLockError::Error(source)) => {
                return Err(BootstrapError::Io {
                    action: "acquire daemon bootstrap lock",
                    path: manifest
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(BOOTSTRAP_LOCK_NAME),
                    source,
                });
            }
        }
    }
}

fn canonical_allowed_roots(options: &BootstrapOptions) -> Result<Vec<PathBuf>, BootstrapError> {
    canonical_allowed_roots_with(
        options,
        std::env::var_os("AE_SDD_ALLOWED_ROOTS"),
        std::env::var_os("AE_SDD_WORKSPACE_ROOT"),
        std::env::current_dir(),
    )
}

fn canonical_allowed_roots_with(
    options: &BootstrapOptions,
    allowed_roots_env: Option<OsString>,
    workspace_root_env: Option<OsString>,
    current_dir: io::Result<PathBuf>,
) -> Result<Vec<PathBuf>, BootstrapError> {
    let mut candidates = options.allowed_roots.clone();
    if let Some(value) = allowed_roots_env {
        candidates
            .extend(std::env::split_paths(&value).filter(|path| !path.as_os_str().is_empty()));
    }
    if let Some(value) = workspace_root_env
        && !value.is_empty()
    {
        candidates.push(PathBuf::from(value));
    }
    if let Some(path) = &options.project_root {
        candidates.push(path.clone());
    } else if candidates.is_empty() {
        candidates.push(current_dir.map_err(|source| BootstrapError::Io {
            action: "read the current project directory",
            path: PathBuf::from("."),
            source,
        })?);
    }

    let mut seen = HashSet::new();
    let mut canonical = Vec::new();
    for root in candidates {
        let resolved = std::fs::canonicalize(&root).map_err(|source| BootstrapError::Io {
            action: "canonicalize allowed workspace root",
            path: root.clone(),
            source,
        })?;
        let metadata = std::fs::metadata(&resolved).map_err(|source| BootstrapError::Io {
            action: "inspect allowed workspace root",
            path: resolved.clone(),
            source,
        })?;
        if !metadata.is_dir() {
            return Err(BootstrapError::AllowedRootNotDirectory(resolved));
        }
        if seen.insert(resolved.clone()) {
            canonical.push(resolved);
        }
    }
    Ok(canonical)
}

#[cfg(unix)]
fn spawn_daemon(
    options: &BootstrapOptions,
    state_dir: &Path,
    allowed_roots: &[PathBuf],
) -> Result<DaemonChild, BootstrapError> {
    let executable = match &options.daemon {
        Some(path) => path.clone(),
        None => sibling_daemon()?,
    };
    let mut command = Command::new(&executable);
    command
        .arg("serve")
        .arg("--state-dir")
        .arg(state_dir)
        .current_dir(state_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    configure_daemon_environment(&mut command);
    for root in allowed_roots {
        command.arg("--allowed-root").arg(root);
    }
    if let Some(policy_digest) = &options.policy_digest {
        command.arg("--policy-digest").arg(policy_digest);
    }
    configure_background(&mut command);
    command.spawn().map_err(|source| BootstrapError::Io {
        action: "spawn daemon executable",
        path: executable,
        source,
    })
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn spawn_daemon(
    options: &BootstrapOptions,
    state_dir: &Path,
    allowed_roots: &[PathBuf],
) -> Result<DaemonChild, BootstrapError> {
    let executable = match &options.daemon {
        Some(path) => path.clone(),
        None => sibling_daemon()?,
    };
    let mut arguments = vec![
        OsString::from("serve"),
        OsString::from("--state-dir"),
        state_dir.as_os_str().to_owned(),
    ];
    for root in allowed_roots {
        arguments.push(OsString::from("--allowed-root"));
        arguments.push(root.as_os_str().to_owned());
    }
    if let Some(policy_digest) = &options.policy_digest {
        arguments.push(OsString::from("--policy-digest"));
        arguments.push(OsString::from(policy_digest));
    }
    let command_line = windows_command_line(&executable, &arguments);
    let environment = windows_environment_block();
    let mut startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..STARTUPINFOW::default()
    };
    let mut process_info = PROCESS_INFORMATION::default();
    let flags = CREATE_NO_WINDOW
        | CREATE_NEW_PROCESS_GROUP
        | CREATE_UNICODE_ENVIRONMENT
        | CREATE_BREAKAWAY_FROM_JOB;
    create_process_windows(
        &executable,
        state_dir,
        command_line,
        environment,
        &mut startup,
        &mut process_info,
        flags,
    )
    .or_else(|error| {
        // Some hosts disallow job breakaway. Handle inheritance is still
        // disabled in the fallback, so captured Hook/CLI pipes cannot leak.
        if error.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32) {
            let command_line = windows_command_line(&executable, &arguments);
            let environment = windows_environment_block();
            create_process_windows(
                &executable,
                state_dir,
                command_line,
                environment,
                &mut startup,
                &mut process_info,
                CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP | CREATE_UNICODE_ENVIRONMENT,
            )
        } else {
            Err(error)
        }
    })
    .map_err(|source| BootstrapError::Io {
        action: "spawn daemon executable",
        path: executable,
        source,
    })?;
    // SAFETY: successful CreateProcessW returned two valid owned handles.
    // The thread handle is closed exactly once; the process handle is moved
    // into OwnedHandle and closed exactly once on drop.
    unsafe {
        let _ = CloseHandle(process_info.hThread);
        Ok(DaemonChild {
            process: OwnedHandle::from_raw_handle(process_info.hProcess as _),
        })
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn create_process_windows(
    executable: &Path,
    current_directory: &Path,
    mut command_line: Vec<u16>,
    mut environment: Vec<u16>,
    startup: &mut STARTUPINFOW,
    process_info: &mut PROCESS_INFORMATION,
    flags: u32,
) -> io::Result<()> {
    let application = wide_null(executable.as_os_str());
    let current_directory = wide_null(current_directory.as_os_str());
    // SAFETY: all UTF-16 buffers are live and NUL-terminated for this call;
    // mutable pointers refer to writable Vec storage; security pointers are
    // null; current-dir and STARTUPINFO/PROCESS_INFORMATION are valid.
    // FALSE forbids inheriting every caller handle, including captured pipes.
    let created = unsafe {
        CreateProcessW(
            application.as_ptr(),
            command_line.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            FALSE,
            flags,
            environment.as_mut_ptr().cast(),
            current_directory.as_ptr(),
            startup,
            process_info,
        )
    };
    if created == FALSE {
        Err(io::Error::from_raw_os_error(
            unsafe { GetLastError() } as i32
        ))
    } else {
        Ok(())
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn child_try_wait(child: &mut DaemonChild) -> io::Result<Option<ExitStatus>> {
    let mut code = 0u32;
    // SAFETY: OwnedHandle keeps a valid process handle alive for the call and
    // `code` is writable for one u32.
    let ok = unsafe { GetExitCodeProcess(child.process.as_raw_handle() as _, &mut code) };
    if ok == FALSE {
        return Err(io::Error::last_os_error());
    }
    if code == windows_sys::Win32::Foundation::STILL_ACTIVE as u32 {
        Ok(None)
    } else {
        use std::os::windows::process::ExitStatusExt;
        Ok(Some(ExitStatus::from_raw(code)))
    }
}

#[cfg(unix)]
fn child_try_wait(child: &mut DaemonChild) -> io::Result<Option<ExitStatus>> {
    child.try_wait()
}

#[cfg(windows)]
#[allow(unsafe_code)]
async fn terminate_child(child: &mut DaemonChild) -> io::Result<()> {
    // SAFETY: both calls receive the live process handle owned by `child`.
    // TerminateProcess is used only after readiness has failed; the bounded
    // wait reaps termination without blocking bootstrap indefinitely.
    let terminated = unsafe { TerminateProcess(child.process.as_raw_handle() as _, 1) };
    if terminated == FALSE {
        let error = io::Error::last_os_error();
        if child_try_wait(child)?.is_none() {
            return Err(error);
        }
        return Ok(());
    }
    let waited = unsafe { WaitForSingleObject(child.process.as_raw_handle() as _, 5_000) };
    if waited != windows_sys::Win32::Foundation::WAIT_OBJECT_0 {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "failed daemon process did not terminate within 5 seconds",
        ));
    }
    Ok(())
}

#[cfg(unix)]
async fn terminate_child(child: &mut DaemonChild) -> io::Result<()> {
    match child.kill().await {
        Ok(()) => {}
        Err(_) if child.try_wait()?.is_some() => return Ok(()),
        Err(error) => return Err(error),
    }
    child.wait().await.map(|_| ())
}

#[cfg(windows)]
fn windows_command_line(executable: &Path, arguments: &[OsString]) -> Vec<u16> {
    let mut values = Vec::with_capacity(arguments.len() + 1);
    values.push(executable.as_os_str().to_owned());
    values.extend(arguments.iter().cloned());
    let mut command_line = Vec::new();
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            command_line.push(' ' as u16);
        }
        append_windows_quoted_arg(&mut command_line, value);
    }
    command_line.push(0);
    command_line
}

#[cfg(windows)]
fn append_windows_quoted_arg(output: &mut Vec<u16>, value: &OsStr) {
    let wide: Vec<u16> = value.encode_wide().collect();
    let needs_quotes = wide.is_empty()
        || wide.iter().any(|character| {
            *character == b' ' as u16 || *character == b'\t' as u16 || *character == b'"' as u16
        });
    if !needs_quotes {
        output.extend(wide);
        return;
    }
    output.push('"' as u16);
    let mut backslashes = 0usize;
    for character in wide {
        if character == b'\\' as u16 {
            backslashes += 1;
        } else if character == b'"' as u16 {
            output.extend(std::iter::repeat_n(b'\\' as u16, backslashes * 2 + 1));
            output.push(character);
            backslashes = 0;
        } else {
            output.extend(std::iter::repeat_n(b'\\' as u16, backslashes));
            output.push(character);
            backslashes = 0;
        }
    }
    output.extend(std::iter::repeat_n(b'\\' as u16, backslashes * 2));
    output.push('"' as u16);
}

#[cfg(windows)]
fn wide_null(value: &OsStr) -> Vec<u16> {
    let mut wide: Vec<u16> = value.encode_wide().collect();
    wide.push(0);
    wide
}

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

fn is_safe_environment_key(key: &OsStr) -> bool {
    let key = key.to_string_lossy();
    SAFE_ENVIRONMENT_KEYS
        .iter()
        .any(|allowed| key.eq_ignore_ascii_case(allowed))
}

#[cfg(unix)]
fn configure_daemon_environment(command: &mut Command) {
    command.env_clear();
    for (key, value) in std::env::vars_os().filter(|(key, _)| is_safe_environment_key(key)) {
        command.env(key, value);
    }
}

#[cfg(windows)]
fn windows_environment_block() -> Vec<u16> {
    windows_environment_block_from(
        std::env::vars_os().filter(|(key, _)| is_safe_environment_key(key)),
    )
}

#[cfg(windows)]
fn windows_environment_block_from<I>(variables: I) -> Vec<u16>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let mut variables: Vec<(OsString, OsString)> = variables.into_iter().collect();
    variables.sort_by(|left, right| {
        left.0
            .to_string_lossy()
            .to_ascii_uppercase()
            .cmp(&right.0.to_string_lossy().to_ascii_uppercase())
    });
    let mut block = Vec::new();
    for (key, value) in variables {
        block.extend(key.encode_wide());
        block.push('=' as u16);
        block.extend(value.encode_wide());
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    block
}

fn sibling_daemon() -> Result<PathBuf, BootstrapError> {
    let executable = std::env::current_exe().map_err(|source| BootstrapError::Io {
        action: "resolve CLI executable",
        path: PathBuf::new(),
        source,
    })?;
    let parent = executable
        .parent()
        .ok_or(BootstrapError::ExecutableParent)?;
    Ok(parent.join(if cfg!(windows) {
        "ae-sddd.exe"
    } else {
        "ae-sddd"
    }))
}

fn canonicalize_parent(path: PathBuf) -> Result<PathBuf, BootstrapError> {
    let parent = path.parent().ok_or_else(|| BootstrapError::Io {
        action: "resolve endpoint manifest parent",
        path: path.clone(),
        source: io::Error::new(
            io::ErrorKind::InvalidInput,
            "manifest has no parent directory",
        ),
    })?;
    std::fs::create_dir_all(parent).map_err(|source| BootstrapError::Io {
        action: "create endpoint manifest parent",
        path: parent.to_path_buf(),
        source,
    })?;
    let parent = std::fs::canonicalize(parent).map_err(|source| BootstrapError::Io {
        action: "canonicalize endpoint manifest parent",
        path: parent.to_path_buf(),
        source,
    })?;
    let file_name = path.file_name().ok_or_else(|| BootstrapError::Io {
        action: "resolve endpoint manifest name",
        path: path.clone(),
        source: io::Error::new(io::ErrorKind::InvalidInput, "manifest has no file name"),
    })?;
    Ok(parent.join(file_name))
}

fn absolute(path: PathBuf, current_dir: &Path) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        current_dir.join(path)
    }
}

/// Assembles a type-safe `SessionBootstrapRequest` from its constituent
/// identities, deferring all frozen-contract validation (root/delegation
/// invariant, capability bound/uniqueness) to `SessionBootstrapRequest::new`.
/// Pure construction, no I/O; `schema_version` is fixed to `SchemaVersion::V1`
/// per the existing constructor convention (e.g. `HostActionBody::create`)
/// rather than taken as a caller argument. Not yet wired to any call site;
/// C1 will call this once workspace registration and delegation context
/// resolution land.
pub fn build_session_bootstrap_request(
    workspace_id: WorkspaceId,
    external_session_key: ExternalSessionKey,
    adapter_id: AdapterId,
    role: AgentRole,
    engaged: bool,
    delegation_id: Option<DelegationId>,
    capabilities: Vec<CapabilityId>,
    context_bundle_id: Option<ContextBundleId>,
) -> Result<SessionBootstrapRequest, SessionContractError> {
    SessionBootstrapRequest::new(
        SchemaVersion::V1,
        workspace_id,
        external_session_key,
        adapter_id,
        role,
        engaged,
        delegation_id,
        capabilities,
        context_bundle_id,
    )
}

fn ensure_before_deadline(deadline: Instant, timeout: Duration) -> Result<(), BootstrapError> {
    if Instant::now() < deadline {
        Ok(())
    } else {
        Err(timeout_error(timeout))
    }
}

async fn sleep_for_poll(deadline: Instant, timeout: Duration) -> Result<(), BootstrapError> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(|| timeout_error(timeout))?;
    if remaining.is_zero() {
        return Err(timeout_error(timeout));
    }
    tokio::time::sleep(LOCK_POLL_INTERVAL.min(remaining)).await;
    ensure_before_deadline(deadline, timeout)
}

fn timeout_error(timeout: Duration) -> BootstrapError {
    BootstrapError::StartupTimeout {
        timeout_ms: timeout.as_millis().max(1),
    }
}

fn duration_millis_u64(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX).max(1)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use ae_sdd_protocol::StableErrorCode;

    use super::*;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "ae-sdd-bootstrap-{}-{sequence}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("test directory must be created");
            Self(path)
        }

        fn child(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::create_dir_all(&path).expect("test child directory must be created");
            path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn only_manifest_and_transport_failures_mean_unavailable() {
        assert!(is_unavailable(&ClientError::EndpointManifest));
        assert!(is_unavailable(&ClientError::DaemonUnavailable));
        assert!(!is_unavailable(&ClientError::Protocol));
        assert!(!is_unavailable(&ClientError::Remote {
            code: StableErrorCode::EndpointStale,
            message: "stale".to_owned(),
        }));
    }

    #[tokio::test]
    async fn whole_probe_call_has_one_outer_timeout() {
        let started = Instant::now();
        let result = call_with_timeout(
            Duration::from_millis(5),
            std::future::pending::<Result<Value, ClientError>>(),
        )
        .await;

        assert!(matches!(result, Err(ClientError::DaemonUnavailable)));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn explicit_policy_digest_mismatch_fails_closed() {
        let expected = "a".repeat(64);
        let options = BootstrapOptions {
            policy_digest: Some(expected.clone()),
            ..BootstrapOptions::default()
        };
        let error = ready_result(
            json!({
                "policyDigest":
                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            }),
            BootstrapDisposition::Reused,
            &options,
        )
        .expect_err("mismatched ready daemon must be rejected");

        assert!(matches!(
            error,
            BootstrapError::PolicyDigestMismatch {
                expected: value,
                ..
            } if value == expected
        ));
    }

    #[test]
    fn roots_are_canonical_union_and_deduplicated() {
        let temp = TestDirectory::new();
        let explicit = temp.child("explicit");
        let workspace = temp.child("workspace");
        let current = temp.child("current");
        let joined = std::env::join_paths([explicit.clone(), workspace.clone()])
            .expect("test roots must form one path list");
        let options = BootstrapOptions {
            allowed_roots: vec![explicit.clone()],
            project_root: Some(current.clone()),
            ..BootstrapOptions::default()
        };

        let roots = canonical_allowed_roots_with(
            &options,
            Some(joined),
            Some(workspace.into_os_string()),
            Ok(temp.0.clone()),
        )
        .expect("roots must resolve");

        assert_eq!(roots.len(), 3);
        assert_eq!(roots[0], std::fs::canonicalize(explicit).unwrap());
        assert_eq!(roots[2], std::fs::canonicalize(current).unwrap());
    }

    #[test]
    fn explicit_roots_do_not_silently_authorize_the_current_directory() {
        let temp = TestDirectory::new();
        let explicit = temp.child("explicit-only");
        let current = temp.child("untrusted-current");
        let options = BootstrapOptions {
            allowed_roots: vec![explicit.clone()],
            ..BootstrapOptions::default()
        };

        let roots = canonical_allowed_roots_with(&options, None, None, Ok(current))
            .expect("explicit root resolves");

        assert_eq!(roots, vec![std::fs::canonicalize(explicit).unwrap()]);
    }

    #[test]
    fn bootstrap_lock_is_not_the_daemon_singleton_lock() {
        let temp = TestDirectory::new();
        let bootstrap = temp.0.join(BOOTSTRAP_LOCK_NAME);
        let singleton = temp.0.join("daemon.lock");
        assert_ne!(bootstrap, singleton);

        let first = open_bootstrap_lock(&bootstrap).expect("first lock file must open");
        let second = open_bootstrap_lock(&bootstrap).expect("second lock file must open");
        FileExt::try_lock(&first).expect("first lock must be acquired");
        assert!(matches!(
            FileExt::try_lock(&second),
            Err(TryLockError::WouldBlock)
        ));
    }

    #[test]
    fn daemon_environment_keeps_platform_basics_but_drops_agent_secrets() {
        assert!(is_safe_environment_key(OsStr::new("PATH")));
        assert!(is_safe_environment_key(OsStr::new("Username")));
        assert!(is_safe_environment_key(OsStr::new("LOCALAPPDATA")));
        for secret in [
            "AE_SDD_CAPABILITY_TOKEN",
            "AE_SDD_SESSION_ID",
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "GITHUB_TOKEN",
            "AWS_SECRET_ACCESS_KEY",
        ] {
            assert!(
                !is_safe_environment_key(OsStr::new(secret)),
                "{secret} must not enter the shared daemon environment"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_environment_block_is_sorted_and_double_nul_terminated() {
        let block = windows_environment_block_from([
            (OsString::from("z-last"), OsString::from("2")),
            (OsString::from("A-first"), OsString::from("1")),
        ]);
        assert!(block.ends_with(&[0, 0]));
        let body = String::from_utf16(&block[..block.len() - 1]).expect("UTF-16 environment");
        assert_eq!(body, "A-first=1\0z-last=2\0");

        assert_eq!(windows_environment_block_from(Vec::new()), vec![0, 0]);
    }

    #[cfg(windows)]
    #[test]
    fn windows_command_line_quotes_spaces_and_trailing_slashes() {
        let command = windows_command_line(
            Path::new(r"C:\Program Files\ae-sddd.exe"),
            &[
                OsString::from("serve"),
                OsString::from(r"C:\root with space\"),
            ],
        );
        let command = String::from_utf16(&command[..command.len() - 1]).expect("UTF-16 argv");
        assert_eq!(
            command,
            r#""C:\Program Files\ae-sddd.exe" serve "C:\root with space\\""#
        );
    }
}
