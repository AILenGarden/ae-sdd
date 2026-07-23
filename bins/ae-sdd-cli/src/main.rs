use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ae_sdd_client::{
    DaemonClient, HookClient, HookInvocation, LocalIpcTransport, default_endpoint_manifest,
    default_state_dir,
};
use ae_sdd_protocol::{ClientKind, ConfirmationRef, PROTOCOL_VERSION_V1, RequestParams, RpcMethod};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::process::Command;

#[derive(Parser)]
#[command(
    name = "ae-sdd",
    version,
    about = "Thin CLI for the ae-sdd Rust daemon"
)]
struct Arguments {
    #[command(subcommand)]
    command: TopLevel,
}

#[derive(Subcommand)]
enum TopLevel {
    /// Manage the per-user daemon lifecycle.
    Runtime {
        #[command(subcommand)]
        command: RuntimeCommand,
    },
    /// Invoke an exact registered JSON-RPC method with a full RequestParams object.
    Rpc {
        /// Exact protocol method, for example `workspace.snapshot`.
        #[arg(long)]
        method: String,
        /// JSON literal or `-` to read stdin.
        #[arg(long)]
        params_json: String,
        /// Endpoint manifest override.
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// End-to-end local IPC timeout.
        #[arg(long, default_value_t = 30_000)]
        timeout_ms: u64,
    },
    /// Invoke one of the four Hook methods with fail-closed host JSON output.
    Hook {
        /// Exact Hook method.
        #[arg(long)]
        method: String,
        /// HookRequest JSON literal or `-` to read stdin.
        #[arg(long)]
        request_json: String,
        /// Endpoint manifest override.
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// End-to-end local IPC timeout.
        #[arg(long, default_value_t = 250)]
        timeout_ms: u64,
    },
}

#[derive(Subcommand)]
enum RuntimeCommand {
    /// Start `ae-sddd serve` in the background and wait for a successful handshake.
    Start {
        /// Daemon executable override; defaults to a sibling `ae-sddd` binary.
        #[arg(long)]
        daemon: Option<PathBuf>,
        /// Per-user daemon state directory.
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Canonical parent roots from which workspaces may be registered.
        #[arg(long, required = true)]
        allowed_root: Vec<PathBuf>,
        /// Current policy digest.
        #[arg(long)]
        policy_digest: Option<String>,
        /// Maximum startup wait.
        #[arg(long, default_value_t = 5_000)]
        timeout_ms: u64,
    },
    /// Read daemon status over authenticated local RPC.
    Status {
        #[arg(long)]
        manifest: Option<PathBuf>,
    },
    /// Enter drain mode; new sessions and workspaces are rejected.
    Drain {
        #[arg(long)]
        manifest: Option<PathBuf>,
    },
    /// Drain and stop the daemon.
    Stop {
        #[arg(long)]
        manifest: Option<PathBuf>,
    },
    /// Print the bounded daemon lifecycle log.
    Logs {
        #[arg(long)]
        state_dir: Option<PathBuf>,
        #[arg(long, default_value_t = 200)]
        tail: usize,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HookRequest {
    params: RequestParams<Value>,
    engaged: bool,
    #[serde(default)]
    offline_capability: Option<String>,
    now_unix_ms: u64,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run(Arguments::parse()).await {
        eprintln!("ae-sdd: {error}");
        std::process::exit(1);
    }
}

async fn run(arguments: Arguments) -> Result<(), String> {
    match arguments.command {
        TopLevel::Runtime { command } => runtime(command).await,
        TopLevel::Rpc {
            method,
            params_json,
            manifest,
            timeout_ms,
        } => {
            let method = RpcMethod::from_str(&method).map_err(|error| error.to_string())?;
            if method == RpcMethod::RuntimeHandshake {
                return Err(
                    "runtime.handshake is managed by the client and cannot be invoked directly"
                        .to_owned(),
                );
            }
            let params: RequestParams<Value> = read_json_argument(&params_json)?;
            let client = client(manifest, ClientKind::Cli, timeout_ms)?;
            let result: Value = client
                .call(method, params)
                .await
                .map_err(|error| error.to_string())?;
            print_json(&result)
        }
        TopLevel::Hook {
            method,
            request_json,
            manifest,
            timeout_ms,
        } => {
            let method = RpcMethod::from_str(&method).map_err(|error| error.to_string())?;
            if !matches!(
                method,
                RpcMethod::HookUserPrompt
                    | RpcMethod::HookPreTool
                    | RpcMethod::HookPostTool
                    | RpcMethod::HookStop
            ) {
                return Err(
                    "hook command accepts only the four registered hook.* methods".to_owned(),
                );
            }
            let request: HookRequest = read_json_argument(&request_json)?;
            let client = client(manifest, ClientKind::Hook, timeout_ms)?;
            let outcome = HookClient::new(&client)
                .invoke(HookInvocation {
                    method,
                    params: request.params,
                    engaged: request.engaged,
                    offline_capability: request.offline_capability,
                    now_unix_ms: request.now_unix_ms,
                })
                .await
                .map_err(|error| error.to_string())?;
            print_json(&outcome)
        }
    }
}

async fn runtime(command: RuntimeCommand) -> Result<(), String> {
    match command {
        RuntimeCommand::Start {
            daemon,
            state_dir,
            allowed_root,
            policy_digest,
            timeout_ms,
        } => start_daemon(daemon, state_dir, allowed_root, policy_digest, timeout_ms).await,
        RuntimeCommand::Status { manifest } => {
            let client = client(manifest, ClientKind::Cli, 2_000)?;
            let status: Value = client
                .call(RpcMethod::RuntimeStatus, empty_params(json!({}), 2_000))
                .await
                .map_err(|error| error.to_string())?;
            print_json(&status)
        }
        RuntimeCommand::Drain { manifest } => lifecycle_request(manifest, false).await,
        RuntimeCommand::Stop { manifest } => lifecycle_request(manifest, true).await,
        RuntimeCommand::Logs { state_dir, tail } => {
            let directory = state_dir
                .map(Ok)
                .unwrap_or_else(default_state_dir)
                .map_err(|error| error.to_string())?;
            let contents = tokio::fs::read_to_string(directory.join("daemon.log"))
                .await
                .map_err(|error| error.to_string())?;
            let lines: Vec<_> = contents.lines().collect();
            let start = lines.len().saturating_sub(tail);
            for line in &lines[start..] {
                println!("{line}");
            }
            Ok(())
        }
    }
}

async fn start_daemon(
    daemon: Option<PathBuf>,
    state_dir: Option<PathBuf>,
    allowed_roots: Vec<PathBuf>,
    policy_digest: Option<String>,
    timeout_ms: u64,
) -> Result<(), String> {
    let state_dir = state_dir
        .map(Ok)
        .unwrap_or_else(default_state_dir)
        .map_err(|error| error.to_string())?;
    let daemon = daemon.unwrap_or(sibling_daemon()?);
    let mut command = Command::new(daemon);
    command
        .arg("serve")
        .arg("--state-dir")
        .arg(&state_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for root in allowed_roots {
        command.arg("--allowed-root").arg(root);
    }
    if let Some(digest) = policy_digest {
        command.arg("--policy-digest").arg(digest);
    }
    configure_background(&mut command);
    command.spawn().map_err(|error| error.to_string())?;

    let manifest = state_dir.join("endpoint.v1.json");
    let started = Instant::now();
    let timeout = Duration::from_millis(timeout_ms.max(1));
    loop {
        if tokio::fs::try_exists(&manifest).await.unwrap_or(false) {
            let client = client(Some(manifest.clone()), ClientKind::Cli, 500)?;
            if let Ok(status) =
                client
                    .call::<Value>(RpcMethod::RuntimeStatus, empty_params(json!({}), 500))
                    .await
            {
                return print_json(&status);
            }
        }
        if started.elapsed() >= timeout {
            return Err("daemon did not become ready before the startup deadline".to_owned());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn lifecycle_request(manifest: Option<PathBuf>, stop: bool) -> Result<(), String> {
    let client = client(manifest, ClientKind::Admin, 2_000)?;
    let now = now_unix_ms();
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
        idempotency_key: Some(format!("runtime-lifecycle-{now}-{stop}")),
        confirmation: Some(ConfirmationRef {
            confirmation_id: format!("cli-{now}"),
            approved_by: "user:cli".to_owned(),
            approved_at: now.to_string(),
        }),
        deadline_ms: 2_000,
        payload: json!({"stop":stop}),
    };
    let status: Value = client
        .call(RpcMethod::RuntimeDrain, params)
        .await
        .map_err(|error| error.to_string())?;
    print_json(&status)
}

fn client(
    manifest: Option<PathBuf>,
    kind: ClientKind,
    timeout_ms: u64,
) -> Result<DaemonClient, String> {
    let manifest = manifest
        .map(Ok)
        .unwrap_or_else(default_endpoint_manifest)
        .map_err(|error| error.to_string())?;
    Ok(DaemonClient::new(
        manifest,
        kind,
        Arc::new(LocalIpcTransport),
        Duration::from_millis(timeout_ms.max(1)),
    ))
}

fn empty_params(payload: Value, deadline_ms: u64) -> RequestParams<Value> {
    RequestParams {
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
        payload,
    }
}

fn read_json_argument<T: serde::de::DeserializeOwned>(argument: &str) -> Result<T, String> {
    let source = if argument == "-" {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|error| error.to_string())?;
        source
    } else {
        argument.to_owned()
    };
    serde_json::from_str(&source).map_err(|error| error.to_string())
}

fn print_json(value: &impl serde::Serialize) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string(value).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn sibling_daemon() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let parent = executable
        .parent()
        .ok_or_else(|| "CLI executable has no parent directory".to_owned())?;
    let name = if cfg!(windows) {
        "ae-sddd.exe"
    } else {
        "ae-sddd"
    };
    Ok(parent.join(name))
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().try_into().unwrap_or(u64::MAX)
        })
}

#[cfg(windows)]
fn configure_background(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
}

#[cfg(unix)]
fn configure_background(_command: &mut Command) {}

#[allow(dead_code)]
fn _path_anchor(_path: &Path) {}
