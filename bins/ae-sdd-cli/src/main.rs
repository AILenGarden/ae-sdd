use std::io::{self, Read};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ae_sdd_client::{
    ClientError, DaemonClient, HookClient, HookInvocation, HookOutcome, LocalIpcTransport,
    default_endpoint_manifest, default_state_dir,
};
use ae_sdd_operations::{OperationName, validate_operation_payload};
use ae_sdd_protocol::{
    ClientKind, ConfirmationRef, HookDecision, PROTOCOL_VERSION_V1, RequestParams, RpcMethod,
};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::process::Command;

pub mod bootstrap;
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
    /// Resume an approved execution plan; the daemon owns every business rule.
    ResumeApprovedPlan {
        /// Resume request JSON literal or `-` to read stdin.
        #[arg(long)]
        request: String,
        /// Endpoint manifest override.
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// End-to-end local IPC timeout.
        #[arg(long, default_value_t = 30_000)]
        timeout_ms: u64,
    },
    /// Resolve one frozen legacy leaf command without a fallback branch.
    #[command(external_subcommand)]
    Legacy(Vec<String>),
}

#[derive(Subcommand)]
enum RuntimeCommand {
    /// Ensure `ae-sddd serve` is ready and print its authenticated status.
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
    /// Reuse or race-safely bootstrap the per-user daemon.
    Ensure {
        /// Daemon executable override; defaults to a sibling `ae-sddd` binary.
        #[arg(long)]
        daemon: Option<PathBuf>,
        /// Per-user daemon state directory.
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Endpoint manifest override, primarily for isolated tests.
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// Canonical parent roots from which workspaces may be registered.
        #[arg(long)]
        allowed_root: Vec<PathBuf>,
        /// Current project root; defaults to the current working directory.
        #[arg(long)]
        project_root: Option<PathBuf>,
        /// Current policy digest.
        #[arg(long)]
        policy_digest: Option<String>,
        /// Maximum bootstrap wait.
        #[arg(long, default_value_t = 5_000)]
        timeout_ms: u64,
        /// Suppress status output for host lifecycle prewarming.
        #[arg(long)]
        quiet: bool,
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

/// Request document accepted by `ae-sdd resume-approved-plan --request <json>`.
///
/// The CLI only normalizes these identity fields and the optional resume
/// cursor; every business rule (approved-plan validation, capsule build or
/// reuse, projection kind) stays inside the daemon. Unknown fields fail
/// closed so a drifting client cannot smuggle authority inputs past the
/// registry contract.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResumeApprovedPlanRequest {
    workspace_id: String,
    agent_id: String,
    session_id: String,
    work_item_id: String,
    #[serde(default)]
    turn_id: Option<String>,
    #[serde(default)]
    capability_token: Option<String>,
    #[serde(default)]
    known_capsule_digest: Option<String>,
    #[serde(default)]
    known_context_revision: Option<u64>,
    #[serde(default)]
    deadline_ms: Option<u64>,
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
            let recover_runtime = manifest.is_none()
                && !matches!(method, RpcMethod::RuntimeStatus | RpcMethod::RuntimeDrain);
            let client = client(manifest, ClientKind::Cli, timeout_ms)?;
            let result: Value = call_with_runtime_recovery(
                &client,
                method,
                params,
                recover_runtime,
                ClientKind::Cli,
                timeout_ms,
            )
            .await?;
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
            let recover_runtime = manifest.is_none();
            if parsed.host_input && !host_binding_available() {
                if recover_runtime {
                    let _ = ensure_default_runtime(ClientKind::Hook, timeout_ms).await;
                }
                return print_json(&host_fail_closed(
                    method,
                    "trusted hook session binding is unavailable",
                ));
            }
            let request = parsed.request.ok_or_else(|| {
                "host Hook event could not be converted to a typed request".to_owned()
            })?;
            let client = client(manifest, ClientKind::Hook, timeout_ms)?;
            let invocation = HookInvocation {
                method,
                params: request.params,
                engaged: request.engaged,
                offline_capability: request.offline_capability,
                now_unix_ms: request.now_unix_ms,
            };
            let hook_client = HookClient::new(&client);
            let outcome = if recover_runtime {
                hook_client
                    .invoke_with_recovery(invocation, || async move {
                        ensure_default_runtime(ClientKind::Hook, timeout_ms)
                            .await
                            .map(|_| ())
                            .map_err(|_| ClientError::DaemonUnavailable)
                    })
                    .await
            } else {
                hook_client.invoke(invocation).await
            }
            .map_err(|error| error.to_string())?;
            if parsed.host_input {
                print_host_outcome(method, &outcome)
            } else {
                print_json(&outcome)
            }
        }
        TopLevel::ResumeApprovedPlan {
            request,
            manifest,
            timeout_ms,
        } => {
            let request: ResumeApprovedPlanRequest = read_json_argument(&request)?;
            let params = assemble_resume_request(&request)?;
            let recover_runtime = manifest.is_none();
            let client = client(manifest, ClientKind::Cli, timeout_ms)?;
            let result: Value = call_with_runtime_recovery(
                &client,
                RpcMethod::OperationExecute,
                params,
                recover_runtime,
                ClientKind::Cli,
                timeout_ms,
            )
            .await?;
            print_json(&render_resume_projection(&result)?)
        }
        TopLevel::Legacy(arguments) => run_legacy(arguments).await,
    }
}

/// Assembles the `operation.execute` params for `execution.resume`.
///
/// The cursor is validated against the frozen operation registry before any
/// IPC so malformed input fails closed on the client side; the daemon
/// remains authoritative and re-validates the complete request. The CLI
/// never reads Story, constraints, state or source files itself.
pub fn assemble_resume_request(
    request: &ResumeApprovedPlanRequest,
) -> Result<RequestParams<Value>, String> {
    let mut cursor = serde_json::Map::new();
    if let Some(digest) = request.known_capsule_digest.as_deref() {
        cursor.insert(
            "knownCapsuleDigest".to_owned(),
            Value::String(digest.to_owned()),
        );
    }
    if let Some(revision) = request.known_context_revision {
        cursor.insert("knownContextRevision".to_owned(), json!(revision));
    }
    let cursor = Value::Object(cursor);
    validate_operation_payload(OperationName::ExecutionResume, &cursor)
        .map_err(|error| format!("execution.resume cursor is invalid: {error}"))?;
    Ok(RequestParams {
        protocol_version: PROTOCOL_VERSION_V1.to_owned(),
        workspace_id: Some(request.workspace_id.clone()),
        agent_id: Some(request.agent_id.clone()),
        session_id: Some(request.session_id.clone()),
        capability_token: request.capability_token.clone(),
        turn_id: request.turn_id.clone(),
        work_item_id: Some(request.work_item_id.clone()),
        lease_id: None,
        fencing_token: None,
        expected_revision: None,
        idempotency_key: None,
        confirmation: None,
        deadline_ms: request.deadline_ms.unwrap_or(30_000).max(1),
        payload: json!({
            "operation": OperationName::ExecutionResume.as_str(),
            "dryRun": false,
            "payload": cursor,
        }),
    })
}

/// Frozen fields the CLI renders from an `execution.resume` daemon response.
const RESUME_PROJECTION_KEYS: [&str; 6] = [
    "projectionKind",
    "contextRevision",
    "capsuleDigest",
    "capsule",
    "nextAction",
    "authorityRefreshCount",
];

/// Renders the daemon-owned `execution.resume` projection.
///
/// Only the frozen projection fields are surfaced; a response that is not an
/// object or misses a required key fails closed instead of fabricating a
/// projection the daemon never produced.
pub fn render_resume_projection(data: &Value) -> Result<Value, String> {
    let object = data
        .as_object()
        .ok_or_else(|| "execution.resume response must be a JSON object".to_owned())?;
    for key in RESUME_PROJECTION_KEYS {
        if !object.contains_key(key) {
            return Err(format!("execution.resume response is missing `{key}`"));
        }
    }
    let mut projection = serde_json::Map::new();
    for key in RESUME_PROJECTION_KEYS {
        projection.insert(key.to_owned(), object[key].clone());
    }
    Ok(Value::Object(projection))
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
    use legacy::{
        ImplementationStatus, LegacyNativeRequestSource, LegacyRequestSource, LegacyRpcAdapter,
        LegacyTarget,
    };

    let resolved = legacy::resolve_legacy_argv(&arguments).map_err(|error| error.to_string())?;
    if resolved.route.contract.status == ImplementationStatus::Pending {
        return Err(format!(
            "legacy route is still pending verified parity and was denied: {} (evidence: {})",
            resolved.route.command_id, resolved.route.contract.evidence
        ));
    }
    match &resolved.route.target {
        LegacyTarget::Rejected { .. } => {
            Err("removed legacy route cannot be dispatched".to_owned())
        }
        LegacyTarget::NativeBuildJob { entrypoint, .. } => {
            let invocation = legacy::parse_native_invocation(
                &resolved.route,
                entrypoint,
                &resolved.trailing_arguments,
                |name| std::env::var(name).ok(),
            )
            .map_err(|error| error.to_string())?;
            let mut temporary = None;
            let request = match invocation.request {
                LegacyNativeRequestSource::ExplicitFile(path) => {
                    let request_bytes = tokio::fs::read(&path)
                        .await
                        .map_err(|error| error.to_string())?;
                    let request_json: Value = serde_json::from_slice(&request_bytes)
                        .map_err(|error| error.to_string())?;
                    legacy::verify_offline_request(entrypoint, &request_json)
                        .map_err(|error| error.to_string())?;
                    path
                }
                LegacyNativeRequestSource::Generated(request) => {
                    let file = legacy::TemporaryJsonRequest::create(&request)
                        .map_err(|error| error.to_string())?;
                    let path = file.path().to_path_buf();
                    temporary = Some(file);
                    path
                }
            };
            let mut command = Command::new(sibling_build()?);
            command.arg("offline").arg("--request").arg(request);
            if invocation.output_json {
                command.arg("--json");
            }
            let status = command.status().await.map_err(|error| error.to_string())?;
            drop(temporary);
            if status.success() {
                Ok(())
            } else {
                Err(format!(
                    "offline Rust build job failed with status {status}"
                ))
            }
        }
        LegacyTarget::Rpc { method, adapter } => {
            let invocation = legacy::parse_rpc_invocation(
                &resolved.route,
                *method,
                &resolved.trailing_arguments,
                |name| std::env::var(name).ok(),
            )
            .map_err(|error| error.to_string())?;
            let (mut params, synthesized): (RequestParams<Value>, bool) = match invocation.request {
                LegacyRequestSource::ExplicitJson(request) => {
                    (read_json_argument(&request)?, false)
                }
                LegacyRequestSource::Synthesized(params) => (*params, true),
            };
            legacy::validate_request_params(&resolved.route, *method, &params)
                .map_err(|error| error.to_string())?;
            match adapter {
                LegacyRpcAdapter::Passthrough => {
                    if synthesized {
                        legacy::adapt_passthrough_request(
                            &resolved.route.command_id,
                            *method,
                            &mut params,
                        )
                        .map_err(|error| error.to_string())?;
                    }
                }
                LegacyRpcAdapter::TypedOperation { operation } => {
                    legacy::adapt_typed_operation_request(operation, &mut params)
                        .map_err(|error| error.to_string())?;
                }
                LegacyRpcAdapter::JobSubmission { entrypoint, .. } => {
                    legacy::adapt_job_submission(
                        &resolved.route,
                        entrypoint,
                        &mut params,
                        now_unix_ms(),
                    )
                    .map_err(|error| error.to_string())?;
                }
            }
            legacy::validate_request_params(&resolved.route, *method, &params)
                .map_err(|error| error.to_string())?;
            let job_poll = matches!(adapter, LegacyRpcAdapter::JobSubmission { .. })
                .then(|| legacy::LegacyJobPollContext::from_submission(&params))
                .transpose()
                .map_err(|error| error.to_string())?;
            let client_kind = if matches!(
                adapter,
                LegacyRpcAdapter::TypedOperation { operation } if operation == "lease.break"
            ) {
                ClientKind::Admin
            } else {
                ClientKind::Cli
            };
            let recover_runtime = invocation.manifest.is_none();
            let deadline_ms = params.deadline_ms;
            let client = client(invocation.manifest, client_kind, deadline_ms)?;
            let result: Value = call_with_runtime_recovery(
                &client,
                *method,
                params,
                recover_runtime,
                client_kind,
                deadline_ms,
            )
            .await?;
            if let Some(job_poll) = job_poll {
                return poll_legacy_job(&client, &resolved.route.command_id, &job_poll, &result)
                    .await;
            }
            print_json(&result)?;
            legacy::validate_passthrough_result(&resolved.route.command_id, *method, &result)
                .map_err(|error| error.to_string())
        }
    }
}

async fn poll_legacy_job(
    client: &DaemonClient,
    command_id: &str,
    context: &legacy::LegacyJobPollContext,
    submitted: &Value,
) -> Result<(), String> {
    let job_id = submitted
        .get("jobId")
        .and_then(Value::as_str)
        .ok_or_else(|| "job.submit returned no jobId".to_owned())?;
    loop {
        let params = context
            .status_request(job_id, now_unix_ms())
            .map_err(|error| error.to_string())?;
        let status: Value = client
            .call(RpcMethod::JobStatus, params)
            .await
            .map_err(|error| error.to_string())?;
        match legacy::validate_job_terminal_status(command_id, &status) {
            Ok(false) => tokio::time::sleep(Duration::from_millis(20)).await,
            Ok(true) => {
                print_json(&status)?;
                return Ok(());
            }
            Err(error) => {
                print_json(&status)?;
                return Err(error.to_string());
            }
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
        RuntimeCommand::Ensure {
            daemon,
            state_dir,
            manifest,
            allowed_root,
            project_root,
            policy_digest,
            timeout_ms,
            quiet,
        } => {
            let result = bootstrap::ensure_daemon(bootstrap::BootstrapOptions {
                daemon,
                state_dir,
                manifest,
                allowed_roots: allowed_root,
                project_root,
                policy_digest,
                timeout: Duration::from_millis(timeout_ms.max(1)),
                probe_timeout: Duration::from_millis(500),
                client_kind: ClientKind::Cli,
            })
            .await
            .map_err(|error| error.to_string())?;
            if quiet { Ok(()) } else { print_json(&result) }
        }
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
    let result = bootstrap::ensure_daemon(bootstrap::BootstrapOptions {
        daemon,
        state_dir,
        manifest: None,
        allowed_roots,
        project_root: None,
        policy_digest,
        timeout: Duration::from_millis(timeout_ms.max(1)),
        probe_timeout: Duration::from_millis(500),
        client_kind: ClientKind::Cli,
    })
    .await
    .map_err(|error| error.to_string())?;
    print_json(&result.status)
}

async fn call_with_runtime_recovery<T>(
    client: &DaemonClient,
    method: RpcMethod,
    params: RequestParams<Value>,
    recover_runtime: bool,
    client_kind: ClientKind,
    request_timeout_ms: u64,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let first_attempt = duplicate_params(&params);
    match client.call(method, first_attempt).await {
        Ok(result) => Ok(result),
        Err(error) if recover_runtime && is_runtime_unavailable(&error) => {
            ensure_default_runtime(client_kind, request_timeout_ms)
                .await
                .map_err(|bootstrap_error| bootstrap_error.to_string())?;
            client
                .call(method, params)
                .await
                .map_err(|replay_error| replay_error.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}

async fn ensure_default_runtime(
    client_kind: ClientKind,
    request_timeout_ms: u64,
) -> Result<bootstrap::BootstrapResult, bootstrap::BootstrapError> {
    bootstrap::ensure_daemon(bootstrap::BootstrapOptions {
        timeout: Duration::from_secs(5),
        probe_timeout: Duration::from_millis(request_timeout_ms.clamp(1, 500)),
        client_kind,
        ..bootstrap::BootstrapOptions::default()
    })
    .await
}

fn duplicate_params(params: &RequestParams<Value>) -> RequestParams<Value> {
    RequestParams {
        protocol_version: params.protocol_version.clone(),
        workspace_id: params.workspace_id.clone(),
        agent_id: params.agent_id.clone(),
        session_id: params.session_id.clone(),
        capability_token: params.capability_token.clone(),
        turn_id: params.turn_id.clone(),
        work_item_id: params.work_item_id.clone(),
        lease_id: params.lease_id.clone(),
        fencing_token: params.fencing_token,
        expected_revision: params.expected_revision,
        idempotency_key: params.idempotency_key.clone(),
        confirmation: params.confirmation.clone(),
        deadline_ms: params.deadline_ms,
        payload: params.payload.clone(),
    }
}

fn is_runtime_unavailable(error: &ClientError) -> bool {
    matches!(
        error,
        ClientError::EndpointManifest | ClientError::DaemonUnavailable
    )
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
