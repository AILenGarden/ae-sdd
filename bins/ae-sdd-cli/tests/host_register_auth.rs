//! Process-level regression coverage for the host-adapter credential binding:
//! `ae-sdd rpc --client-kind host-adapter --method host.register` (and the
//! `--host-register-json` rebind path used by every other HostAdapter method)
//! must succeed without the caller ever supplying the boot-scoped endpoint
//! credential. The client binds it in memory from the endpoint manifest, so
//! the secret must never appear in argv, stdin JSON, stdout, or stderr.

#![cfg(windows)]

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const ENDPOINT_MANIFEST: &str = "endpoint.v1.json";
const STARTUP_TIMEOUT_MS: &str = "15000";
const PROCESS_TIMEOUT: Duration = Duration::from_secs(30);
static CAPTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn register_without_capability_token_succeeds() {
    let Some(daemon) = daemon_executable() else {
        eprintln!(
            "skipping host.register auth regression: no debug or release ae-sddd.exe is available"
        );
        return;
    };
    let state = IsolatedState::new("register-no-token");
    let manifest = state.manifest();
    let cleanup = DaemonCleanup::new(manifest.clone());
    let ensured = run_command(ensure_command(&daemon, state.path()), None);
    parse_success_json(&ensured, "runtime ensure");

    let register = host_register_params("hra-register-no-token", "hra-register-no-token-1");
    let output = run_command(
        rpc_command(&manifest, "host.register", "host-adapter", &register, None),
        None,
    );
    let result = parse_success_json(&output, "host.register without capabilityToken");
    assert_eq!(result["adapterId"], "hra-register-no-token");
    assert_eq!(result["capabilities"], json!(["create", "attest"]));

    stop_daemon(&manifest);
    wait_for_manifest_removal(&manifest);
    drop(cleanup);
}

#[test]
fn register_discards_forged_capability_token() {
    let Some(daemon) = daemon_executable() else {
        eprintln!(
            "skipping forged-credential regression: no debug or release ae-sddd.exe is available"
        );
        return;
    };
    let state = IsolatedState::new("register-forged-token");
    let manifest = state.manifest();
    let cleanup = DaemonCleanup::new(manifest.clone());
    let ensured = run_command(ensure_command(&daemon, state.path()), None);
    parse_success_json(&ensured, "runtime ensure");

    let forged = format!("forged-endpoint-token-{}", std::process::id());
    let params = serde_json::to_vec(&json!({
        "protocolVersion": "1.0",
        "deadlineMs": 10_000,
        "idempotencyKey": "hra-forged-register-1",
        "capabilityToken": forged,
        "payload": {"adapterId": "hra-forged", "capabilities": ["create", "attest"]},
    }))
    .expect("forged register params serialize");
    // The forged value travels over stdin here, but the client must overwrite
    // it in memory before the request ever reaches the daemon.
    let output = run_command(
        rpc_command(&manifest, "host.register", "host-adapter", "-", None),
        Some(&params),
    );
    let result = parse_success_json(&output, "host.register with a forged capabilityToken");
    assert_eq!(result["adapterId"], "hra-forged");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stdout.contains(&forged) && !stderr.contains(&forged),
        "the discarded forged credential must not be echoed:\n{}",
        command_diagnostics(&output)
    );

    stop_daemon(&manifest);
    wait_for_manifest_removal(&manifest);
    drop(cleanup);
}

#[test]
fn action_next_after_register_rebinds_without_token() {
    let Some(daemon) = daemon_executable() else {
        eprintln!(
            "skipping post-register rebind regression: no debug or release ae-sddd.exe is available"
        );
        return;
    };
    let state = IsolatedState::new("action-next-rebind");
    let manifest = state.manifest();
    let cleanup = DaemonCleanup::new(manifest.clone());
    let ensured = run_command(ensure_command(&daemon, state.path()), None);
    parse_success_json(&ensured, "runtime ensure");

    // Neither the register params nor the target-method params carry a token:
    // `call_after` replays host.register on the shared connection and the
    // client injects the boot credential into that replay.
    let register = host_register_params("hra-action-next", "hra-action-next-register-1");
    let action_next = params_json(json!({"adapterId": "hra-action-next"}), None);
    let output = run_command(
        rpc_command(
            &manifest,
            "host.action_next",
            "host-adapter",
            &action_next,
            Some(&register),
        ),
        None,
    );
    let result = parse_success_json(&output, "host.action_next after a token-less rebind");
    assert!(
        result.is_null(),
        "no action was enqueued, so the queue read must be empty:\n{}",
        command_diagnostics(&output)
    );

    stop_daemon(&manifest);
    wait_for_manifest_removal(&manifest);
    drop(cleanup);
}

#[test]
fn register_follows_boot_token_rotation() {
    let Some(daemon) = daemon_executable() else {
        eprintln!(
            "skipping boot-rotation regression: no debug or release ae-sddd.exe is available"
        );
        return;
    };
    let state = IsolatedState::new("boot-rotation");
    let manifest = state.manifest();
    let cleanup = DaemonCleanup::new(manifest.clone());
    let ensured = run_command(ensure_command(&daemon, state.path()), None);
    parse_success_json(&ensured, "first runtime ensure");
    let first = read_manifest_boot_and_token(&manifest);

    let register = host_register_params("hra-rotation", "hra-rotation-register-1");
    let output = run_command(
        rpc_command(&manifest, "host.register", "host-adapter", &register, None),
        None,
    );
    parse_success_json(&output, "host.register before the restart");

    stop_daemon(&manifest);
    wait_for_manifest_removal(&manifest);

    let ensured = run_command(ensure_command(&daemon, state.path()), None);
    parse_success_json(&ensured, "second runtime ensure");
    let second = read_manifest_boot_and_token(&manifest);
    assert_ne!(first.0, second.0, "a restart must produce a fresh boot");
    assert_ne!(
        first.1, second.1,
        "a fresh boot must rotate the endpoint credential"
    );

    // The exact same token-less params must still succeed: the client re-reads
    // the manifest and binds the new boot's credential, which the daemon
    // verifies before the idempotent replay is even considered.
    let output = run_command(
        rpc_command(&manifest, "host.register", "host-adapter", &register, None),
        None,
    );
    let result = parse_success_json(&output, "host.register after boot-token rotation");
    assert_eq!(result["adapterId"], "hra-rotation");

    stop_daemon(&manifest);
    wait_for_manifest_removal(&manifest);
    drop(cleanup);
}

#[test]
fn endpoint_token_stays_out_of_cli_output_and_input() {
    let Some(daemon) = daemon_executable() else {
        eprintln!(
            "skipping secret-hygiene regression: no debug or release ae-sddd.exe is available"
        );
        return;
    };
    let state = IsolatedState::new("secret-hygiene");
    let manifest = state.manifest();
    let cleanup = DaemonCleanup::new(manifest.clone());
    let ensured = run_command(ensure_command(&daemon, state.path()), None);
    parse_success_json(&ensured, "runtime ensure");
    // The test may read the credential; no CLI invocation ever receives it in
    // argv or stdin (the params below are exactly what each command sends).
    let (_, endpoint_token) = read_manifest_boot_and_token(&manifest);
    assert!(
        !endpoint_token.is_empty(),
        "endpoint manifest carries a boot credential"
    );

    let register = host_register_params("hra-hygiene", "hra-hygiene-register-1");
    let register_output = run_command(
        rpc_command(&manifest, "host.register", "host-adapter", &register, None),
        None,
    );
    parse_success_json(&register_output, "host.register for secret hygiene");

    let action_next = params_json(json!({"adapterId": "hra-hygiene"}), None);
    let action_output = run_command(
        rpc_command(
            &manifest,
            "host.action_next",
            "host-adapter",
            "-",
            Some(&register),
        ),
        Some(action_next.as_bytes()),
    );
    parse_success_json(&action_output, "host.action_next for secret hygiene");

    for (operation, output) in [
        ("runtime ensure", &ensured),
        ("host.register", &register_output),
        ("host.action_next", &action_output),
    ] {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stdout.contains(&endpoint_token) && !stderr.contains(&endpoint_token),
            "{operation} must never leak the endpoint credential:\n{}",
            command_diagnostics(output)
        );
    }

    stop_daemon(&manifest);
    wait_for_manifest_removal(&manifest);
    drop(cleanup);
}

/// The physical-delegation chain the `/ae-sdd delegate-series` flow drives,
/// executed entirely through the real CLI binary: workspace/session setup,
/// `host.register`, `delegation.create`, `host.action_next`,
/// `host.action_ack`, `delegation.accept`, then the series `session.open`.
/// The caller never holds the endpoint credential — only daemon-minted
/// session capabilities travel in params — and the boot credential must not
/// leak into any CLI output along the way.
///
/// The chain opens its sessions without a `workItemId`: an explicit Work Item
/// makes `session.open` resolve the project authority state directory
/// (`.auto-engineering/*/state.json`), and fabricating that state is the
/// daemon process tests' job, not this credential regression's. The two
/// delegation calls carry the Work Item their admission contract requires;
/// neither resolves it against project files.
#[test]
fn physical_delegation_chain_needs_no_endpoint_credential() {
    let Some(daemon) = daemon_executable() else {
        eprintln!(
            "skipping delegation-chain regression: no debug or release ae-sddd.exe is available"
        );
        return;
    };
    let state = IsolatedState::new("delegation-chain");
    let manifest = state.manifest();
    let cleanup = DaemonCleanup::new(manifest.clone());
    let ensured = run_command(ensure_command(&daemon, state.path()), None);
    parse_success_json(&ensured, "runtime ensure");
    let (_, endpoint_token) = read_manifest_boot_and_token(&manifest);

    let suffix = format!("{:08x}", (nonce() as u32) ^ std::process::id());
    let key = format!("hra-chain-{suffix}");
    let project_key = format!("hra-chain-{suffix}");
    // Only an opaque routing identity here: `delegation.create`/`accept`
    // require the envelope field, and resolving it against project authority
    // state is the daemon process tests' job, not this credential regression's.
    let work_item_id = format!("STORY-HRA-{suffix}");
    let adapter_id = format!("{key}-host");
    let root_agent = format!("{key}-root");
    let child_agent = format!("{key}-series-agent");
    let child_session_id = test_uuid(1);
    let claim_id = test_uuid(2);
    let ack_id = test_uuid(3);
    let now = now_unix_ms();
    let mut transcripts: Vec<(&'static str, Output)> = vec![("runtime ensure", ensured)];

    // workspace.register (ordinary CLI client).
    let output = run_command(
        rpc_command(
            &manifest,
            "workspace.register",
            "cli",
            &params_json(
                json!({
                    "projectRoot": repository_root(),
                    "projectKey": project_key,
                }),
                Some(&format!("{key}-workspace")),
            ),
            None,
        ),
        None,
    );
    let workspace = parse_success_json(&output, "workspace.register");
    let workspace_id = workspace["workspaceId"]
        .as_str()
        .expect("workspace id")
        .to_owned();
    transcripts.push(("workspace.register", output));

    // Root session. A shadow workspace means the daemon policy is disengaged,
    // so `engaged` must be false.
    let output = run_command(
        rpc_command(
            &manifest,
            "session.open",
            "cli",
            &serde_json::to_string(&json!({
                "protocolVersion": "1.0",
                "workspaceId": workspace_id,
                "agentId": root_agent,
                "idempotencyKey": format!("{key}-open-root"),
                "deadlineMs": 10_000,
                "payload": {
                    "externalKey": format!("{key}-root-external"),
                    "role": "root",
                    "engaged": false,
                },
            }))
            .expect("root session.open params serialize"),
            None,
        ),
        None,
    );
    let root_session = parse_success_json(&output, "root session.open");
    let root_session_id = root_session["sessionId"]
        .as_str()
        .expect("root session id")
        .to_owned();
    let root_capability = root_session["capabilityToken"]
        .as_str()
        .expect("root session capability")
        .to_owned();
    transcripts.push(("root session.open", output));

    // host.register with no caller-supplied credential.
    let register = host_register_params(&adapter_id, &format!("{key}-host-register"));
    let output = run_command(
        rpc_command(&manifest, "host.register", "host-adapter", &register, None),
        None,
    );
    let registered = parse_success_json(&output, "chain host.register");
    assert_eq!(registered["adapterId"], adapter_id);
    transcripts.push(("host.register", output));

    // delegation.create under the root session's daemon-minted capability.
    let output = run_command(
        rpc_command(
            &manifest,
            "delegation.create",
            "cli",
            &serde_json::to_string(&json!({
                "protocolVersion": "1.0",
                "workspaceId": workspace_id,
                "agentId": root_agent,
                "sessionId": root_session_id,
                "capabilityToken": root_capability,
                "workItemId": work_item_id,
                "idempotencyKey": format!("{key}-create"),
                "deadlineMs": 10_000,
                "payload": {
                    "childRole": "series",
                    "parentDelegationId": null,
                    "inputRevision": 1,
                    "inputFingerprint": "ab".repeat(32),
                    "deadlineUnixMs": now.saturating_add(600_000),
                    "adapterId": adapter_id,
                    "grant": {
                        "operations": [
                            "document.save",
                            "evidence.finalize",
                            "evidence.record",
                            "lease.acquire",
                            "lease.release",
                            "review.record",
                            "verification.plan",
                        ],
                        "capabilities": ["review.specialty.general"],
                        "paths": [{"kind": "project_root"}],
                    },
                },
            }))
            .expect("delegation.create params serialize"),
            None,
        ),
        None,
    );
    let created = parse_success_json(&output, "delegation.create");
    let delegation_id = created["delegationId"]
        .as_str()
        .expect("delegation id")
        .to_owned();
    transcripts.push(("delegation.create", output));

    // host.action_next through the token-less `--host-register-json` rebind.
    let output = run_command(
        rpc_command(
            &manifest,
            "host.action_next",
            "host-adapter",
            &params_json(json!({"adapterId": adapter_id}), None),
            Some(&register),
        ),
        None,
    );
    let action = parse_success_json(&output, "chain host.action_next");
    assert_eq!(action["kind"], "create");
    assert_eq!(action["delegationId"], delegation_id);
    let action_id = action["actionId"].as_str().expect("host action id");
    let command_seq = action["commandSeq"].as_u64().expect("command seq");
    transcripts.push(("host.action_next", output));

    // host.action_ack, still with no caller-side credential.
    let output = run_command(
        rpc_command(
            &manifest,
            "host.action_ack",
            "host-adapter",
            &params_json(
                json!({
                    "adapterId": adapter_id,
                    "ack": {
                        "ackId": ack_id,
                        "actionId": action_id,
                        "commandSeq": command_seq,
                        "outcome": "accepted",
                        "hostTaskId": format!("{key}-host-task"),
                        "sessionId": child_session_id,
                    },
                }),
                Some(&format!("{key}-ack")),
            ),
            Some(&register),
        ),
        None,
    );
    let acknowledged = parse_success_json(&output, "chain host.action_ack");
    assert_eq!(acknowledged["actionId"], action_id);
    transcripts.push(("host.action_ack", output));

    // delegation.accept: the physical claim correlated to the ACK.
    let output = run_command(
        rpc_command(
            &manifest,
            "delegation.accept",
            "cli",
            &serde_json::to_string(&json!({
                "protocolVersion": "1.0",
                "workspaceId": workspace_id,
                "workItemId": work_item_id,
                "idempotencyKey": format!("{key}-accept"),
                "deadlineMs": 10_000,
                "payload": {
                    "delegationId": delegation_id,
                    "claimId": claim_id,
                    "actionId": action_id,
                    "childSessionId": child_session_id,
                    "expiresAtUnixMs": now.saturating_add(500_000),
                },
            }))
            .expect("delegation.accept params serialize"),
            None,
        ),
        None,
    );
    let accepted = parse_success_json(&output, "delegation.accept");
    assert_eq!(accepted["delegationId"], delegation_id);
    assert_eq!(accepted["status"], "running");
    assert_eq!(accepted["childSessionId"], child_session_id);
    transcripts.push(("delegation.accept", output));

    // The delegated series session opens against the accepted delegation.
    let output = run_command(
        rpc_command(
            &manifest,
            "session.open",
            "cli",
            &serde_json::to_string(&json!({
                "protocolVersion": "1.0",
                "workspaceId": workspace_id,
                "agentId": child_agent,
                "sessionId": child_session_id,
                "idempotencyKey": format!("{key}-open-series"),
                "deadlineMs": 10_000,
                "payload": {
                    "externalKey": format!("{key}-series-external"),
                    "role": "series",
                    "engaged": false,
                    "delegationId": delegation_id,
                },
            }))
            .expect("series session.open params serialize"),
            None,
        ),
        None,
    );
    let series = parse_success_json(&output, "series session.open");
    assert_eq!(series["sessionId"], child_session_id);
    assert_eq!(series["role"], "series");
    assert!(
        series["capabilityToken"]
            .as_str()
            .is_some_and(|token| !token.is_empty()),
        "the series session must receive its own daemon-minted capability"
    );
    transcripts.push(("series session.open", output));

    for (operation, output) in &transcripts {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stdout.contains(&endpoint_token) && !stderr.contains(&endpoint_token),
            "{operation} must never leak the endpoint credential:\n{}",
            command_diagnostics(output)
        );
    }

    stop_daemon(&manifest);
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

/// Builds `ae-sdd rpc` against an ensured daemon's explicit endpoint manifest.
/// `params_json` may be `-` to read the params from stdin.
fn rpc_command(
    manifest: &Path,
    method: &str,
    client_kind: &str,
    params_json: &str,
    host_register_json: Option<&str>,
) -> Command {
    let mut command = cli_command();
    command
        .args(["rpc", "--method", method, "--params-json", params_json])
        .arg("--manifest")
        .arg(manifest)
        .args(["--client-kind", client_kind, "--timeout-ms", "10000"]);
    if let Some(register) = host_register_json {
        command.args(["--host-register-json", register]);
    }
    command
}

fn params_json(payload: Value, idempotency_key: Option<&str>) -> String {
    let mut params = json!({
        "protocolVersion": "1.0",
        "deadlineMs": 10_000,
        "payload": payload,
    });
    if let Some(key) = idempotency_key {
        params["idempotencyKey"] = json!(key);
    }
    serde_json::to_string(&params).expect("request params serialize")
}

/// Token-less `host.register` params: the client binds the credential.
fn host_register_params(adapter_id: &str, idempotency_key: &str) -> String {
    params_json(
        json!({"adapterId": adapter_id, "capabilities": ["create", "attest"]}),
        Some(idempotency_key),
    )
}

/// Mints an RFC 4122-shaped identity from local entropy so the test needs no
/// `uuid` dev-dependency; the daemon only requires the wire format.
fn test_uuid(seed: u64) -> String {
    let mixed = nonce() ^ (u128::from(std::process::id()) << 64) ^ (u128::from(seed) << 96);
    let hex = format!("{mixed:032x}");
    let variant = ["8", "9", "a", "b"][(mixed >> 61) as usize & 3];
    format!(
        "{}-{}-4{}-{}{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[13..16],
        variant,
        &hex[17..20],
        &hex[20..32]
    )
}

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after Unix epoch")
        .as_nanos()
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after Unix epoch")
        .as_millis()
        .try_into()
        .expect("current timestamp fits u64")
}

/// Reads `(bootId, endpointToken)` from a ready daemon's endpoint manifest.
/// Tests may hold the credential to assert it never leaks; CLI callers never
/// see it.
fn read_manifest_boot_and_token(manifest: &Path) -> (String, String) {
    let bytes = fs::read(manifest).expect("ready daemon publishes its endpoint manifest");
    let value: Value = serde_json::from_slice(&bytes).expect("endpoint manifest is valid JSON");
    let boot_id = value["bootId"]
        .as_str()
        .expect("endpoint manifest contains bootId")
        .to_owned();
    let token = value["endpointToken"]
        .as_str()
        .expect("endpoint manifest contains endpointToken")
        .to_owned();
    (boot_id, token)
}

fn stop_daemon(manifest: &Path) {
    let mut command = cli_command();
    command
        .args(["runtime", "stop", "--manifest"])
        .arg(manifest);
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

fn command_diagnostics(output: &Output) -> String {
    format!(
        "status={}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("CLI crate is nested under the repository root")
        .to_path_buf()
}

fn daemon_executable() -> Option<PathBuf> {
    let cli = PathBuf::from(env!("CARGO_BIN_EXE_ae-sdd"));
    let root = repository_root();
    [
        cli.parent().map(|parent| parent.join("ae-sddd.exe")),
        Some(root.join("target").join("debug").join("ae-sddd.exe")),
        Some(root.join("target").join("release").join("ae-sddd.exe")),
    ]
    .into_iter()
    .flatten()
    .find(|candidate| candidate.is_file())
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
}

impl DaemonCleanup {
    fn new(manifest: PathBuf) -> Self {
        Self { manifest }
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
            "ae-sdd-host-register-auth-{label}-{}-{nonce}",
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
