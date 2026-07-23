use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ae_sdd_client::{
    DaemonClient, HookClient, HookInvocation, HookOutcome, LocalIpcTransport,
    default_endpoint_manifest, default_state_dir,
};
use ae_sdd_protocol::{
    ClientKind, ConfirmationRef, HookDecision, PROTOCOL_VERSION_V1, RequestParams, RpcMethod,
};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::process::Command;

pub mod legacy;

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
        /// Internal HookRequest or Claude host event JSON, literal or `-` for stdin.
        #[arg(long)]
        request_json: String,
        /// Endpoint manifest override.
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// End-to-end local IPC timeout.
        #[arg(long, default_value_t = 250)]
        timeout_ms: u64,
    },
    /// Resolve one frozen legacy leaf command without a fallback branch.
    #[command(external_subcommand)]
    Legacy(Vec<String>),
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

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct HostHookEvent {
    #[serde(default)]
    hook_event_name: Option<String>,
    #[serde(default)]
    tool_name: Option<String>,
    #[serde(default)]
    tool_input: Option<Value>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    last_assistant_message: Option<String>,
    #[serde(default)]
    event_id: Option<String>,
}

struct ParsedHookRequest {
    request: Option<HookRequest>,
    host_input: bool,
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
            let raw: Value = read_json_argument(&request_json)?;
            let parsed = parse_hook_request(&method, raw)?;
            if parsed.host_input && !host_binding_available() {
                return print_json(&host_fail_closed(
                    method,
                    "trusted hook session binding is unavailable",
                ));
            }
            let request = parsed.request.ok_or_else(|| {
                "host Hook event could not be converted to a typed request".to_owned()
            })?;
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
            if parsed.host_input {
                print_host_outcome(method, &outcome)
            } else {
                print_json(&outcome)
            }
        }
        TopLevel::Legacy(arguments) => run_legacy(arguments).await,
    }
}

fn parse_hook_request(method: &RpcMethod, raw: Value) -> Result<ParsedHookRequest, String> {
    if raw.get("params").is_some() {
        return serde_json::from_value(raw)
            .map(|request| ParsedHookRequest {
                request: Some(request),
                host_input: false,
            })
            .map_err(|error| error.to_string());
    }
    let event: HostHookEvent =
        serde_json::from_value(raw.clone()).map_err(|error| error.to_string())?;
    if event.hook_event_name.is_none()
        && event.tool_name.is_none()
        && event.tool_input.is_none()
        && event.prompt.is_none()
        && event.last_assistant_message.is_none()
    {
        return Err("Hook JSON is neither an internal request nor a host event".to_owned());
    }
    let now = std::env::var("AE_SDD_NOW_UNIX_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(now_unix_ms);
    let event_id = event
        .event_id
        .clone()
        .or_else(|| std::env::var("AE_SDD_HOOK_EVENT_ID").ok())
        .unwrap_or_else(|| format!("{}-{now}", method.as_str()));
    let mut params = empty_params(
        json!({
            "hookEventName": event.hook_event_name,
            "toolName": event.tool_name,
            "toolInput": event.tool_input,
            "prompt": event.prompt,
            "lastAssistantMessage": event.last_assistant_message,
            "hostEvent": raw,
        }),
        250,
    );
    params.workspace_id = std::env::var("AE_SDD_WORKSPACE_ID").ok();
    params.agent_id = std::env::var("AE_SDD_AGENT_ID").ok();
    params.session_id = std::env::var("AE_SDD_SESSION_ID").ok();
    params.capability_token = std::env::var("AE_SDD_CAPABILITY_TOKEN").ok();
    params.turn_id = std::env::var("AE_SDD_TURN_ID").ok();
    params.work_item_id = std::env::var("AE_SDD_WORK_ITEM_ID").ok();
    params.idempotency_key = Some(event_id.clone());
    params.payload = json!({
        "hookEventId": event_id,
        "turnSeq": std::env::var("AE_SDD_TURN_SEQ")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1),
        "hostPayload": params.payload,
    });
    let request = HookRequest {
        params,
        engaged: std::env::var("AE_SDD_HOOK_ENGAGED")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(true),
        offline_capability: std::env::var("AE_SDD_CAPABILITY_TOKEN").ok(),
        now_unix_ms: now,
    };
    Ok(ParsedHookRequest {
        request: Some(request),
        host_input: true,
    })
}

fn host_binding_available() -> bool {
    [
        "AE_SDD_WORKSPACE_ID",
        "AE_SDD_AGENT_ID",
        "AE_SDD_SESSION_ID",
        "AE_SDD_CAPABILITY_TOKEN",
        "AE_SDD_TURN_ID",
        "AE_SDD_WORK_ITEM_ID",
    ]
    .into_iter()
    .all(|name| std::env::var(name).is_ok_and(|value| !value.trim().is_empty()))
}

fn host_fail_closed(method: RpcMethod, reason: &str) -> Value {
    match method {
        RpcMethod::HookPreTool => json!({"decision":"deny","reason":reason}),
        RpcMethod::HookStop => json!({"decision":"block","reason":reason}),
        RpcMethod::HookUserPrompt => json!({"additionalContext":""}),
        RpcMethod::HookPostTool => json!({"decision":"allow"}),
        _ => json!({"decision":"deny","reason":reason}),
    }
}

fn print_host_outcome(method: RpcMethod, outcome: &HookOutcome) -> Result<(), String> {
    let value = match method {
        RpcMethod::HookPreTool => json!({
            "decision": if outcome.decision == HookDecision::Deny { "deny" } else { "allow" },
            "reason": outcome.error_code.map_or_else(|| "".to_owned(), |code| code.as_str().to_owned()),
        }),
        RpcMethod::HookStop => json!({
            "decision": if outcome.decision == HookDecision::Block { "block" } else { "allow" },
            "reason": outcome.error_code.map_or_else(|| "".to_owned(), |code| code.as_str().to_owned()),
        }),
        RpcMethod::HookUserPrompt => {
            let context = outcome.context.as_ref().map_or_else(
                || "".to_owned(),
                |value| serde_json::to_string(value).unwrap_or_default(),
            );
            json!({"additionalContext": context.chars().take(65_536).collect::<String>()})
        }
        RpcMethod::HookPostTool => json!({"decision":"allow"}),
        _ => host_fail_closed(method, "unsupported host Hook method"),
    };
    print_json(&value)
}

async fn run_legacy(arguments: Vec<String>) -> Result<(), String> {
    use legacy::{ImplementationStatus, LegacyRpcAdapter, LegacyTarget};

    let resolved = legacy::resolve_legacy_argv(&arguments).map_err(|error| error.to_string())?;
    if resolved.route.contract.status == ImplementationStatus::Pending {
        return Err(format!(
            "legacy route is still pending verified parity and was denied: {} (evidence: {})",
            resolved.route.command_id, resolved.route.contract.evidence
        ));
    }
    match resolved.route.target {
        LegacyTarget::Rejected { .. } => {
            Err("removed legacy route cannot be dispatched".to_owned())
        }
        LegacyTarget::NativeBuildJob { entrypoint, .. } => {
            let request = required_flag(&resolved.trailing_arguments, "--request")?;
            let request_bytes = tokio::fs::read(&request)
                .await
                .map_err(|error| error.to_string())?;
            let request_json: Value =
                serde_json::from_slice(&request_bytes).map_err(|error| error.to_string())?;
            if request_json.get("entrypoint").and_then(Value::as_str) != Some(entrypoint.as_str()) {
                return Err(
                    "native job request entrypoint differs from the frozen route".to_owned(),
                );
            }
            let status = Command::new(sibling_build()?)
                .arg("native-job")
                .arg("--request")
                .arg(request)
                .arg("--json")
                .status()
                .await
                .map_err(|error| error.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err(format!("native Rust build job failed with status {status}"))
            }
        }
        LegacyTarget::Rpc { method, adapter } => {
            let request = required_flag(&resolved.trailing_arguments, "--request-json")?;
            let mut params: RequestParams<Value> = read_json_argument(&request)?;
            match adapter {
                LegacyRpcAdapter::Passthrough => {}
                LegacyRpcAdapter::TypedOperation { operation } => {
                    params.payload = json!({"operation":operation,"payload":params.payload});
                }
                LegacyRpcAdapter::JobSubmission { entrypoint, .. } => {
                    params.payload = json!({
                        "entrypoint":entrypoint,
                        "arguments":params.payload,
                        "deadlineUnixMs":now_unix_ms().saturating_add(params.deadline_ms),
                    });
                }
            }
            let client = client(None, ClientKind::Cli, params.deadline_ms)?;
            let result: Value = client
                .call(method, params)
                .await
                .map_err(|error| error.to_string())?;
            print_json(&result)
        }
    }
}

fn required_flag(arguments: &[String], name: &str) -> Result<String, String> {
    arguments
        .windows(2)
        .find(|window| window[0] == name)
        .map(|window| window[1].clone())
        .ok_or_else(|| format!("{name} is required for this legacy route"))
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
            if let Ok(status) = client
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

fn sibling_build() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let parent = executable
        .parent()
        .ok_or_else(|| "CLI executable has no parent directory".to_owned())?;
    Ok(parent.join(if cfg!(windows) {
        "ae-sdd-build.exe"
    } else {
        "ae-sdd-build"
    }))
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
