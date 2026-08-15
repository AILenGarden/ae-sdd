#![cfg(windows)]

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

const ENDPOINT_MANIFEST: &str = "endpoint.v1.json";
const STARTUP_TIMEOUT_MS: &str = "15000";
const PROCESS_TIMEOUT: Duration = Duration::from_secs(30);
static CAPTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn explicit_missing_daemon_fails_without_python_fallback() {
    let state = IsolatedState::new("missing-daemon");
    let missing_daemon = state.path().join("missing-ae-sddd.exe");
    let output = run_command(ensure_command(&missing_daemon, state.path()), None);

    assert_command_failed(&output, "runtime ensure with an absent daemon");
    let diagnostics = command_diagnostics(&output).to_ascii_lowercase();
    assert!(
        diagnostics.contains("missing-ae-sddd.exe"),
        "failure must identify the explicitly selected daemon:\n{diagnostics}"
    );
    assert!(
        !diagnostics.contains("python") && !diagnostics.contains(".py"),
        "native bootstrap must not fall back to Python:\n{diagnostics}"
    );
    assert_absent_runtime_files(state.path());
}

#[test]
fn explicit_missing_manifest_status_fails_without_bootstrap() {
    let state = IsolatedState::new("missing-manifest-status");
    let manifest = state.manifest();
    let mut command = cli_command();
    command
        .args(["runtime", "status", "--manifest"])
        .arg(&manifest);
    let output = run_command(command, None);

    assert_command_failed(&output, "runtime status with an absent manifest");
    assert!(
        !state.path().join("bootstrap.lock").exists(),
        "runtime status must not enter the bootstrap path"
    );
    assert_absent_runtime_files(state.path());
}

#[test]
fn default_raw_runtime_status_keeps_non_starting_management_semantics() {
    let local_app_data = IsolatedState::new("raw-status-no-bootstrap");
    let runtime_state = local_app_data.path().join("ae-sdd").join("runtime");
    let params = serde_json::to_vec(&serde_json::json!({
        "protocolVersion": "1.0",
        "deadlineMs": 2000,
        "payload": {}
    }))
    .expect("runtime.status params serialize");
    let mut command = cli_command();
    command.env("LOCALAPPDATA", local_app_data.path()).args([
        "rpc",
        "--method",
        "runtime.status",
        "--params-json",
        "-",
        "--timeout-ms",
        "2000",
    ]);

    let output = run_command(command, Some(&params));

    assert_command_failed(&output, "raw runtime.status with no daemon");
    assert!(!runtime_state.join("bootstrap.lock").exists());
    assert_absent_runtime_files(&runtime_state);
}

#[test]
fn cold_ensure_reuses_the_same_ready_daemon_until_stop() {
    let Some(daemon) = daemon_executable() else {
        eprintln!("skipping real daemon bootstrap: no debug or release ae-sddd.exe is available");
        return;
    };
    let state = IsolatedState::new("cold-reuse");
    let manifest = state.manifest();
    let cleanup = DaemonCleanup::new(manifest.clone());

    let first = run_command(ensure_command(&daemon, state.path()), None);
    let first = parse_success_json(&first, "first runtime ensure");
    assert_eq!(first["disposition"], "started");
    assert_eq!(first["status"]["lifecycle"], "running");
    let first_identity = read_manifest_identity(&manifest);
    assert_eq!(first["status"]["bootId"], first_identity.boot_id);

    let second = run_command(ensure_command(&daemon, state.path()), None);
    let second = parse_success_json(&second, "second runtime ensure");
    assert_eq!(second["disposition"], "reused");
    assert_eq!(second["status"]["lifecycle"], "running");
    let second_identity = read_manifest_identity(&manifest);

    assert_eq!(second["status"]["bootId"], first_identity.boot_id);
    assert_eq!(second_identity, first_identity);

    let status = runtime_status(&manifest);
    assert_eq!(status["lifecycle"], "running");
    assert_eq!(status["bootId"], first_identity.boot_id);

    stop_daemon(&manifest);
    wait_for_manifest_removal(&manifest);
    drop(cleanup);
}

#[test]
fn explicit_daemon_mismatch_fails_closed() {
    let daemon = daemon_executable().expect(
        "critical daemon identity regression requires the sibling ae-sddd.exe; run `cargo build -p ae-sdd-daemon` first",
    );
    let state = IsolatedState::new("explicit-daemon-mismatch");
    let manifest = state.manifest();
    let cleanup = DaemonCleanup::new(manifest.clone());
    let alternate = state.path().join("alternate-ae-sddd.exe");
    fs::copy(&daemon, &alternate).expect("alternate daemon binary is copied");

    let first = parse_success_json(
        &run_command(ensure_command(&daemon, state.path()), None),
        "start explicitly selected daemon",
    );
    assert_eq!(first["disposition"], "started");

    let mismatch = run_command(ensure_command(&alternate, state.path()), None);
    assert_command_failed(&mismatch, "reuse with a different explicit daemon");
    let diagnostics = command_diagnostics(&mismatch);
    assert!(
        diagnostics.contains("daemon executable mismatch"),
        "explicit binary drift must fail closed:\n{diagnostics}"
    );
    assert_eq!(
        read_manifest_identity(&manifest).boot_id,
        first["status"]["bootId"],
        "the mismatch check must not replace the running daemon"
    );

    stop_daemon(&manifest);
    wait_for_manifest_removal(&manifest);
    drop(cleanup);
}

#[test]
fn pipe_captured_cold_ensure_returns_without_waiting_for_daemon_exit() {
    let Some(daemon) = daemon_executable() else {
        eprintln!("skipping pipe-capture bootstrap: no ae-sddd.exe is available");
        return;
    };
    let state = IsolatedState::new("pipe-capture");
    let manifest = state.manifest();
    let cleanup = DaemonCleanup::new(manifest.clone());
    let mut command = ensure_command(&daemon, state.path());
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().expect("pipe-captured CLI process starts");
    let mut stdout = child.stdout.take().expect("CLI stdout pipe exists");
    let mut stderr = child.stderr.take().expect("CLI stderr pipe exists");
    let (sender, receiver) = mpsc::channel();
    let stdout_sender = sender.clone();
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.read_to_end(&mut bytes).map(|_| bytes);
        let _ = stdout_sender.send((true, result));
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stderr.read_to_end(&mut bytes).map(|_| bytes);
        let _ = sender.send((false, result));
    });
    let process_deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().expect("CLI process status is readable") {
            break status;
        }
        if Instant::now() >= process_deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("cold ensure CLI process did not exit within 10s");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let pipe_deadline = Instant::now() + Duration::from_secs(10);
    let mut stdout_bytes = None;
    let mut stderr_bytes = None;
    while stdout_bytes.is_none() || stderr_bytes.is_none() {
        let remaining = pipe_deadline.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(remaining) {
            Ok((true, bytes)) => stdout_bytes = Some(bytes.expect("CLI stdout is readable")),
            Ok((false, bytes)) => stderr_bytes = Some(bytes.expect("CLI stderr is readable")),
            Err(_) => {
                let manifest_before_stop = manifest.exists();
                if manifest_before_stop {
                    stop_daemon(&manifest);
                }
                panic!(
                    "cold ensure CLI exited but captured pipes stayed open for 10s (manifest_before_stop={manifest_before_stop})"
                );
            }
        }
    }
    stdout_reader.join().expect("stdout reader joins");
    stderr_reader.join().expect("stderr reader joins");
    let output = Output {
        status,
        stdout: stdout_bytes.expect("stdout was collected"),
        stderr: stderr_bytes.expect("stderr was collected"),
    };
    let result = parse_success_json(&output, "pipe-captured runtime ensure");
    assert_eq!(result["status"]["lifecycle"], "running");
    stop_daemon(&manifest);
    wait_for_manifest_removal(&manifest);
    drop(cleanup);
}

#[test]
fn concurrent_cold_ensure_callers_converge_to_one_boot_and_pid() {
    const CALLERS: usize = 32;

    let Some(daemon) = daemon_executable() else {
        eprintln!(
            "skipping concurrent daemon bootstrap: no debug or release ae-sddd.exe is available"
        );
        return;
    };
    let state = IsolatedState::new("concurrent-ensure");
    let manifest = state.manifest();
    let cleanup = DaemonCleanup::new(manifest.clone());
    let barrier = Arc::new(Barrier::new(CALLERS));
    let mut callers = Vec::with_capacity(CALLERS);

    for _ in 0..CALLERS {
        let barrier = Arc::clone(&barrier);
        let daemon = daemon.clone();
        let state_dir = state.path().to_path_buf();
        let manifest = manifest.clone();
        callers.push(thread::spawn(move || {
            barrier.wait();
            let output = run_command(ensure_command(&daemon, &state_dir), None);
            let result = parse_success_json(&output, "concurrent runtime ensure");
            let identity = read_manifest_identity(&manifest);
            (result, identity)
        }));
    }

    let results: Vec<_> = callers
        .into_iter()
        .map(|caller| caller.join().expect("concurrent ensure caller completes"))
        .collect();
    let boot_ids: HashSet<_> = results
        .iter()
        .map(|(_, identity)| identity.boot_id.clone())
        .collect();
    let pids: HashSet<_> = results.iter().map(|(_, identity)| identity.pid).collect();
    let started_count = results
        .iter()
        .filter(|(result, _)| result["disposition"] == "started")
        .count();

    assert_eq!(boot_ids.len(), 1, "all callers must observe one boot");
    assert_eq!(pids.len(), 1, "all callers must observe one daemon PID");
    assert_eq!(
        started_count, 1,
        "exactly one caller must win the bootstrap race"
    );
    for (result, identity) in &results {
        assert_eq!(result["status"]["lifecycle"], "running");
        assert_eq!(result["status"]["bootId"], identity.boot_id);
    }

    stop_daemon(&manifest);
    wait_for_manifest_removal(&manifest);
    drop(cleanup);
}

#[test]
fn default_business_rpc_cold_start_survives_the_starting_cli_process() {
    let Some(daemon) = default_sibling_daemon() else {
        eprintln!("skipping default-path bootstrap: ae-sddd.exe is not beside the Cargo-built CLI");
        return;
    };
    assert!(
        daemon.is_file(),
        "default bootstrap daemon must be executable"
    );

    let local_app_data = IsolatedState::new("default-localappdata");
    let manifest = local_app_data
        .path()
        .join("ae-sdd")
        .join("runtime")
        .join(ENDPOINT_MANIFEST);
    let cleanup =
        DaemonCleanup::with_local_app_data(manifest.clone(), local_app_data.path().to_path_buf());
    let root = repository_root();
    let params = serde_json::to_vec(&serde_json::json!({
        "protocolVersion": "1.0",
        "idempotencyKey": "runtime-autostart-workspace-register",
        "deadlineMs": 5000,
        "payload": {
            "projectRoot": root,
            "projectKey": "runtime-autostart-test"
        }
    }))
    .expect("workspace.register params serialize");

    let mut first = cli_command();
    first
        .current_dir(repository_root())
        .env("LOCALAPPDATA", local_app_data.path())
        .args([
            "rpc",
            "--method",
            "workspace.register",
            "--params-json",
            "-",
            "--timeout-ms",
            "5000",
        ]);
    let first = parse_success_json(
        &run_command(first, Some(&params)),
        "default-path rpc workspace.register",
    );
    assert_eq!(first["projectKey"], "runtime-autostart-test");
    let first_identity = read_manifest_identity(&manifest);

    // This is a new CLI process after the bootstrap caller has exited. Seeing
    // the same boot proves the daemon is not tied to the first CLI lifetime.
    let mut second = cli_command();
    second
        .env("LOCALAPPDATA", local_app_data.path())
        .args(["runtime", "status"]);
    let second = parse_success_json(
        &run_command(second, None),
        "second-process default runtime status",
    );
    let second_identity = read_manifest_identity(&manifest);
    assert_eq!(second["lifecycle"], "running");
    assert_eq!(second["bootId"], first_identity.boot_id);
    assert_eq!(second_identity, first_identity);

    stop_daemon_with_local_app_data(&manifest, Some(local_app_data.path()));
    wait_for_manifest_removal(&manifest);
    drop(cleanup);
}

fn ensure_command(daemon: &Path, state_dir: &Path) -> Command {
    let root = repository_root();
    let mut command = cli_command();
    command
        .current_dir(&root)
        .args(["runtime", "ensure", "--daemon"])
        .arg(daemon)
        .arg("--state-dir")
        .arg(state_dir)
        .arg("--allowed-root")
        .arg(&root)
        .arg("--project-root")
        .arg(&root)
        .arg("--timeout-ms")
        .arg(STARTUP_TIMEOUT_MS);
    command
}

fn runtime_status(manifest: &Path) -> Value {
    let mut command = cli_command();
    command
        .args(["runtime", "status", "--manifest"])
        .arg(manifest);
    let output = run_command(command, None);
    parse_success_json(&output, "runtime status")
}

fn stop_daemon(manifest: &Path) {
    stop_daemon_with_local_app_data(manifest, None);
}

fn stop_daemon_with_local_app_data(manifest: &Path, local_app_data: Option<&Path>) {
    let mut command = cli_command();
    command
        .args(["runtime", "stop", "--manifest"])
        .arg(manifest);
    if let Some(local_app_data) = local_app_data {
        command.env("LOCALAPPDATA", local_app_data);
    }
    let output = run_command(command, None);
    let status = parse_success_json(&output, "runtime stop");
    assert_eq!(status["lifecycle"], "stopping");
}

fn cli_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ae-sdd"));
    command
        .env_remove("AE_SDD_ALLOWED_ROOTS")
        .env_remove("AE_SDD_WORKSPACE_ROOT")
        .env_remove("AE_SDD_MANIFEST");
    command
}

fn run_command(mut command: Command, stdin: Option<&[u8]>) -> Output {
    let capture = CaptureFiles::new();
    let stdout = File::create(&capture.stdout).expect("stdout capture file is created");
    let stderr = File::create(&capture.stderr).expect("stderr capture file is created");
    command
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));

    if stdin.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }
    let mut child = command.spawn().expect("CLI process starts");
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .expect("piped CLI stdin is available")
            .write_all(input)
            .expect("CLI stdin is written");
    }
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait().expect("CLI process status is readable") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("CLI process did not complete within {PROCESS_TIMEOUT:?}");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let stdout = fs::read(&capture.stdout).expect("stdout capture is readable");
    let stderr = fs::read(&capture.stderr).expect("stderr capture is readable");
    Output {
        status,
        stdout,
        stderr,
    }
}

fn parse_success_json(output: &Output, operation: &str) -> Value {
    assert!(
        output.status.success(),
        "{operation} failed:\n{}",
        command_diagnostics(output)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{operation} did not emit JSON ({error}):\n{}",
            command_diagnostics(output)
        )
    })
}

fn assert_command_failed(output: &Output, operation: &str) {
    assert!(
        !output.status.success(),
        "{operation} unexpectedly succeeded:\n{}",
        command_diagnostics(output)
    );
}

fn command_diagnostics(output: &Output) -> String {
    format!(
        "status={}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_absent_runtime_files(state_dir: &Path) {
    for name in [
        ENDPOINT_MANIFEST,
        "daemon.lock",
        "runtime.sqlite3",
        "daemon.log",
    ] {
        assert!(
            !state_dir.join(name).exists(),
            "failed management/bootstrap command must not create {name}"
        );
    }
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("CLI crate is nested under the repository root")
        .to_path_buf()
}

fn daemon_executable() -> Option<PathBuf> {
    default_sibling_daemon()
}

fn default_sibling_daemon() -> Option<PathBuf> {
    PathBuf::from(env!("CARGO_BIN_EXE_ae-sdd"))
        .parent()
        .map(|parent| parent.join("ae-sddd.exe"))
        .filter(|candidate| candidate.is_file())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DaemonIdentity {
    boot_id: String,
    pid: u32,
}

fn read_manifest_identity(manifest: &Path) -> DaemonIdentity {
    let bytes = fs::read(manifest).expect("ready daemon publishes its endpoint manifest");
    let value: Value = serde_json::from_slice(&bytes).expect("endpoint manifest is valid JSON");
    let boot_id = value["bootId"]
        .as_str()
        .expect("endpoint manifest contains bootId")
        .to_owned();
    let pid = u32::try_from(
        value["pid"]
            .as_u64()
            .expect("endpoint manifest contains a numeric pid"),
    )
    .expect("daemon pid fits u32");
    DaemonIdentity { boot_id, pid }
}

fn wait_for_manifest_removal(manifest: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while manifest.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(25));
    }
    assert!(
        !manifest.exists(),
        "daemon did not remove its endpoint manifest after stop"
    );
    thread::sleep(Duration::from_millis(50));
}

struct DaemonCleanup {
    manifest: PathBuf,
    local_app_data: Option<PathBuf>,
}

impl DaemonCleanup {
    fn new(manifest: PathBuf) -> Self {
        Self {
            manifest,
            local_app_data: None,
        }
    }

    fn with_local_app_data(manifest: PathBuf, local_app_data: PathBuf) -> Self {
        Self {
            manifest,
            local_app_data: Some(local_app_data),
        }
    }
}

impl Drop for DaemonCleanup {
    fn drop(&mut self) {
        if !self.manifest.exists() {
            return;
        }
        let identity = fs::read(&self.manifest)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .and_then(|value| value["pid"].as_u64())
            .and_then(|pid| u32::try_from(pid).ok());
        let mut stop = cli_command();
        stop.args(["runtime", "stop", "--manifest"])
            .arg(&self.manifest);
        if let Some(local_app_data) = &self.local_app_data {
            stop.env("LOCALAPPDATA", local_app_data);
        }
        let _ = run_command(stop, None);
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.manifest.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(25));
        }
        if self.manifest.exists()
            && let Some(pid) = identity
        {
            let _ = Command::new("taskkill.exe")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .output();
        }
    }
}

struct CaptureFiles {
    directory: PathBuf,
    stdout: PathBuf,
    stderr: PathBuf,
}

impl CaptureFiles {
    fn new() -> Self {
        let sequence = CAPTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "ae-sdd-cli-capture-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("CLI capture directory is created");
        Self {
            stdout: directory.join("stdout.txt"),
            stderr: directory.join("stderr.txt"),
            directory,
        }
    }
}

impl Drop for CaptureFiles {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

struct IsolatedState {
    path: PathBuf,
}

impl IsolatedState {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ae-sdd-runtime-autostart-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("isolated runtime state directory is created");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn manifest(&self) -> PathBuf {
        self.path.join(ENDPOINT_MANIFEST)
    }
}

impl Drop for IsolatedState {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
