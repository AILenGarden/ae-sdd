use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ae_sdd_client::{
    ClientError, DaemonClient, HookClient, HookInvocation, HookOutcome, LocalIpcTransport,
    default_endpoint_manifest, default_state_dir,
};
use ae_sdd_domain::ArtifactDigest;
use ae_sdd_operations::{OperationName, validate_operation_payload};
use ae_sdd_protocol::{
    ClientKind, ConfirmationRef, HookDecision, PROTOCOL_VERSION_V1, RequestParams, RpcMethod,
    WorkspaceMode,
};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;

mod diagnostics;
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
    ///
    /// For `host.register`, omit `capabilityToken` from the params: the client
    /// binds the boot-scoped credential from the endpoint manifest in memory and
    /// discards any supplied value, so the secret never belongs in argv, stdin
    /// JSON, or shell history.
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
        /// Handshake identity for exact HostAdapter RPCs; defaults to ordinary CLI.
        #[arg(long, value_enum, default_value_t = RpcClientKind::Cli)]
        client_kind: RpcClientKind,
        /// Full host.register RequestParams JSON, required to bind the same
        /// connection before a HostAdapter method other than host.register.
        /// Omit `capabilityToken`: the client binds the boot-scoped credential
        /// from the endpoint manifest in memory and discards any supplied value.
        #[arg(long)]
        host_register_json: Option<String>,
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
        #[arg(long)]
        timeout_ms: Option<u64>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum RpcClientKind {
    Cli,
    HostAdapter,
}

impl From<RpcClientKind> for ClientKind {
    fn from(value: RpcClientKind) -> Self {
        match value {
            RpcClientKind::Cli => Self::Cli,
            RpcClientKind::HostAdapter => Self::HostAdapter,
        }
    }
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
    /// Query the daemon diagnostic tracks.
    Trace {
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Which track to read: `trace`, `ops` or `all`.
        #[arg(long, default_value = "all")]
        track: diagnostics::TrackSelector,
        /// Only records newer than this window, such as `30m`, `2h` or `7d`.
        #[arg(long)]
        since: Option<String>,
        /// Only records belonging to this turn.
        #[arg(long)]
        turn: Option<String>,
        /// Only records belonging to this Hook event.
        #[arg(long)]
        hook: Option<String>,
        /// Only records whose method, operation or site contains this text.
        #[arg(long)]
        name: Option<String>,
        /// Only records that failed.
        #[arg(long)]
        failed: bool,
        /// Maximum records to print.
        #[arg(long, default_value_t = 50)]
        limit: usize,
        /// Output shape: `lines`, `count` or `gaps`.
        #[arg(long, default_value = "lines")]
        format: diagnostics::OutputFormat,
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
    /// Host session identity, preferred when opening the daemon session.
    #[serde(default, alias = "sessionId")]
    session_id: Option<String>,
    /// Host thread identity, used when the event has no session identity.
    #[serde(default, alias = "threadId")]
    thread_id: Option<String>,
    /// Host conversation identity, used when neither session nor thread exists.
    #[serde(default, alias = "conversationId")]
    conversation_id: Option<String>,
    /// Host working directory, used as the workspace project root.
    #[serde(default)]
    cwd: Option<String>,
    /// `SubagentStart`/`SubagentStop` child identity, minted in-process by the
    /// host (`crypto.randomBytes`), never caller-supplied. ROUTE-702d576a
    /// Task 1 Q4/Q5: this is the only field that makes the event correlation
    /// non-forgeable; a missing value means the event cannot be trusted at all.
    #[serde(default, alias = "agentId")]
    agent_id: Option<String>,
    /// `SubagentStart`/`SubagentStop` agent type name (must name a registered
    /// agent definition on the host side, never a caller-chosen identifier).
    /// Not part of the correlation key (`(session_id, agent_id)`, Task 1 Q4);
    /// kept here so the Admission slice can read it without widening this
    /// struct again, but it is not consulted by the Host payload RED slice.
    #[serde(default, alias = "agentType")]
    #[allow(dead_code)]
    agent_type: Option<String>,
}

/// Host-supplied identity a Hook can bind a typed session from.
struct HostIdentity {
    external_key: String,
    project_root: PathBuf,
}

struct ParsedHookRequest {
    request: Option<HookRequest>,
    host_input: bool,
    /// Present when the host event carried enough identity to self-bind.
    host_identity: Option<HostIdentity>,
}

/// Minimal `workspace.register` projection the Hook binding needs.
///
/// `mode` is the typed protocol enum rather than a string so the engaged rule
/// cannot drift from the wire names it is derived from.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceBinding {
    workspace_id: String,
    mode: WorkspaceMode,
}

/// Minimal `session.open` projection the Hook binding needs.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionBinding {
    session_id: String,
    capability_token: String,
}

/// Trusted identity a Hook request is sent under.
struct HookBinding {
    workspace_id: String,
    agent_id: String,
    session_id: String,
    capability_token: String,
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
            client_kind,
            host_register_json,
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
            let client_kind = ClientKind::from(client_kind);
            let client = client(manifest, client_kind, timeout_ms)?;
            let result: Value = if client_kind == ClientKind::HostAdapter
                && method != RpcMethod::HostRegister
            {
                let register_json = host_register_json.ok_or_else(|| {
                    "HostAdapter RPCs after host.register require --host-register-json so registration and the target method share one connection".to_owned()
                })?;
                let register: RequestParams<Value> = read_json_argument(&register_json)?;
                if recover_runtime {
                    client
                        .call_after_with_ensure(
                            RpcMethod::HostRegister,
                            register,
                            method,
                            params,
                            || async {
                                ensure_default_runtime(client_kind, timeout_ms)
                                    .await
                                    .map(|_| ())
                                    .map_err(|_| ClientError::DaemonUnavailable)
                            },
                        )
                        .await
                        .map_err(|error| error.to_string())?
                } else {
                    client
                        .call_after(RpcMethod::HostRegister, register, method, params)
                        .await
                        .map_err(|error| error.to_string())?
                }
            } else {
                if host_register_json.is_some() {
                    return Err(
                        "--host-register-json is valid only for HostAdapter RPCs after host.register"
                            .to_owned(),
                    );
                }
                call_with_runtime_recovery(
                    &client,
                    method,
                    params,
                    recover_runtime,
                    client_kind,
                    timeout_ms,
                )
                .await?
            };
            print_json(&result)
        }
        TopLevel::Hook {
            method,
            request_json,
            manifest,
            timeout_ms,
        } => {
            let timeouts = hook_timeouts(timeout_ms);
            // `hook.subagent_start` is a CLI-local translation, not a protocol
            // `RpcMethod` (ROUTE-702d576a Task 2: Plan §Task 2 requires proving
            // the existing `delegation.accept`/`session.open` contracts cannot
            // safely express this before any new method is frozen). It is
            // intercepted here, before `RpcMethod::from_str`, so an unverified
            // field is never read as identity and a missing one fails closed
            // in the same host JSON shape as the four registered Hooks.
            if method == "hook.subagent_start" {
                let raw: Value = read_json_argument(&request_json)?;
                return print_json(&subagent_start_fail_closed(raw));
            }
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
            let mut request = parsed.request.ok_or_else(|| {
                "host Hook event could not be converted to a typed request".to_owned()
            })?;
            let bootstrap_activation = is_bootstrap_activation(method, &request);
            let binding_client = client(manifest.clone(), ClientKind::Hook, timeouts.binding_ms)?;
            if parsed.host_input && (parsed.host_identity.is_some() || !host_binding_available()) {
                // The daemon must exist before a binding can be resolved from it.
                if recover_runtime {
                    let _ = ensure_default_runtime(ClientKind::Hook, timeouts.binding_ms).await;
                }
                let Some(identity) = parsed.host_identity.as_ref() else {
                    let reason = format!(
                        "{}; the host event carried no usable session, thread, or conversation id to bind from",
                        missing_binding_reason()
                    );
                    report_hook_fail_closed(method, &reason);
                    return print_json(&host_fail_closed(method, &reason));
                };
                let hook_event_id =
                    request.params.idempotency_key.as_deref().ok_or_else(|| {
                        "host Hook event lacks its idempotency identity".to_owned()
                    })?;
                match bind_host_session(
                    &binding_client,
                    identity,
                    hook_event_id,
                    bootstrap_activation,
                    timeouts.binding_ms,
                )
                .await
                {
                    Ok(binding) => apply_hook_binding(&mut request, &binding),
                    Err(error) => {
                        let reason =
                            format!("trusted hook session binding could not be created: {error}");
                        report_hook_fail_closed(method, &reason);
                        return print_json(&host_fail_closed(method, &reason));
                    }
                }
            }
            let invocation = HookInvocation {
                method,
                params: request.params,
                engaged: request.engaged,
                offline_capability: request.offline_capability,
                now_unix_ms: request.now_unix_ms,
            };
            let invocation_timeout_ms = hook_invocation_timeout(timeouts, bootstrap_activation);
            let client = client(manifest, ClientKind::Hook, invocation_timeout_ms)?;
            let hook_client = HookClient::new(&client);
            let outcome = if recover_runtime {
                hook_client
                    .invoke_with_recovery(invocation, || async move {
                        ensure_default_runtime(ClientKind::Hook, invocation_timeout_ms)
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
    let response = data
        .as_object()
        .ok_or_else(|| "execution.resume response must be a JSON object".to_owned())?;
    let object = response
        .get("data")
        .unwrap_or(data)
        .as_object()
        .ok_or_else(|| "execution.resume response data must be a JSON object".to_owned())?;
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
                host_identity: None,
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
    // `turnSeq` travels only with an explicit host `turnId`. A Hook subprocess
    // holds no durable state, so a hardcoded sequence would collide with the
    // session's real turn on every event after the first; an omitted pair asks
    // the daemon to allocate the turn it alone can order.
    let mut payload = serde_json::Map::new();
    payload.insert("hookEventId".to_owned(), Value::String(event_id.clone()));
    if params.turn_id.is_some() {
        payload.insert(
            "turnSeq".to_owned(),
            json!(
                std::env::var("AE_SDD_TURN_SEQ")
                    .ok()
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or(1)
            ),
        );
    }
    payload.insert("hostPayload".to_owned(), params.payload);
    params.payload = Value::Object(payload);
    let request = HookRequest {
        params,
        engaged: std::env::var("AE_SDD_HOOK_ENGAGED")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(true),
        offline_capability: std::env::var("AE_SDD_CAPABILITY_TOKEN").ok(),
        now_unix_ms: now,
    };
    // A host that names its conversation and working directory carries enough
    // identity for the Hook to bind a typed session itself. The project root
    // falls back to the process directory, which for a Hook subprocess is the
    // host's own workspace.
    let host_identity = host_external_key(&event).and_then(|external_key| {
        let project_root = event
            .cwd
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())?;
        Some(HostIdentity {
            external_key: external_key.to_owned(),
            project_root,
        })
    });
    Ok(ParsedHookRequest {
        request: Some(request),
        host_input: true,
        host_identity,
    })
}

/// Validates a `SubagentStart` event's identity fields and fails closed on
/// any missing one, without ever reading prompt or transcript content as a
/// substitute identity (ROUTE-702d576a Task 2 Host payload RED).
///
/// This is intentionally CLI-local shape validation only, and stays that way
/// permanently -- this is not a placeholder for later Admission code. The
/// daemon design (`source/docs/ae-sdd-daemon-design.md` §9.3) has `Host` --
/// not `Child` -- as the party that receives the daemon-issued `claimId`
/// (delivered only through `host.action_next`); the sequence diagram's
/// `Child->>Daemon: one-time claim + attestation` is the step *after* Host
/// hands the claim to the child, not something the child can originate on
/// its own. In the A2 model the "Host" role is root's own connection, so
/// `delegation.accept` and the child's `session.open` are root-side
/// orchestration (root already holds the claim from its own
/// `host.action_next`/`host.action_ack` sequence). A `SubagentStart` hook
/// subprocess never receives a `claimId` in its payload and must not
/// attempt to originate accept or session.open itself.
fn subagent_start_fail_closed(raw: Value) -> Value {
    let deny = |reason: String| json!({"decision":"deny","reason":reason});
    let event: HostHookEvent = match serde_json::from_value(raw) {
        Ok(event) => event,
        Err(error) => {
            return deny(format!(
                "SubagentStart payload could not be parsed: {error}"
            ));
        }
    };
    if event.hook_event_name.as_deref() != Some("SubagentStart") {
        return deny("hook.subagent_start requires a SubagentStart hook_event_name".to_owned());
    }
    let Some(agent_id) = event
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return deny(
            "SubagentStart event carries no agent_id; the host-minted child identity is absent"
                .to_owned(),
        );
    };
    let Some(session_id) = event
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return deny(
            "SubagentStart event carries no session_id; the parent correlation is absent"
                .to_owned(),
        );
    };
    // Both fields are validated above and intentionally unused past this
    // point: this event carries no claimId, so accept/session.open cannot be
    // originated here (see the function doc). The event's own identity is
    // well-formed; whether it correlates to a real pending delegation is
    // root's own `host.action_next`/`accept` sequence to determine, not this
    // hook's.
    let _ = (agent_id, session_id);
    json!({
        "decision":"deny",
        "reason":"hook.subagent_start validates event shape only; delegation.accept and session.open are root-side orchestration and are not performed by this hook"
    })
}

fn is_bootstrap_activation(method: RpcMethod, request: &HookRequest) -> bool {
    method == RpcMethod::HookUserPrompt
        && request
            .params
            .payload
            .pointer("/hostPayload/prompt")
            .and_then(Value::as_str)
            .is_some_and(|prompt| prompt.trim() == "/ae-sdd")
}

/// Returns a stable host identity without treating an empty higher-priority ID
/// as a reason to discard a usable lower-priority one.
fn host_external_key(event: &HostHookEvent) -> Option<&str> {
    [
        event.session_id.as_deref(),
        event.thread_id.as_deref(),
        event.conversation_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .find(|value| !value.is_empty())
}

/// Environment variables that together form a trusted host Hook binding.
const HOST_BINDING_VARIABLES: [&str; 6] = [
    "AE_SDD_WORKSPACE_ID",
    "AE_SDD_AGENT_ID",
    "AE_SDD_SESSION_ID",
    "AE_SDD_CAPABILITY_TOKEN",
    "AE_SDD_TURN_ID",
    "AE_SDD_WORK_ITEM_ID",
];

/// Default Agent identity for a host that does not name one.
const DEFAULT_HOOK_AGENT_ID: &str = "host-hook";
const DEFAULT_HOOK_RPC_TIMEOUT_MS: u64 = 250;
const DEFAULT_HOOK_BINDING_TIMEOUT_MS: u64 = 2_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HookTimeouts {
    rpc_ms: u64,
    binding_ms: u64,
}

fn hook_timeouts(explicit_timeout_ms: Option<u64>) -> HookTimeouts {
    explicit_timeout_ms.map_or(
        HookTimeouts {
            rpc_ms: DEFAULT_HOOK_RPC_TIMEOUT_MS,
            binding_ms: DEFAULT_HOOK_BINDING_TIMEOUT_MS,
        },
        |timeout_ms| HookTimeouts {
            rpc_ms: timeout_ms,
            binding_ms: timeout_ms,
        },
    )
}

const fn hook_invocation_timeout(timeouts: HookTimeouts, bootstrap_activation: bool) -> u64 {
    if bootstrap_activation {
        timeouts.binding_ms
    } else {
        timeouts.rpc_ms
    }
}

/// Binds a trusted typed session for one host Hook event.
///
/// `SessionStart` cannot hand a binding to later Hooks: each Hook is a separate
/// host subprocess and can never export variables back to its parent. So the
/// binding is re-resolved per event from the daemon, which is cheap because both
/// calls are idempotent — `workspace.register` by canonical root and
/// `session.open` by external key both return the existing record.
///
/// `turnId` and `workItemId` are deliberately not synthesized here: the turn is
/// a durable monotonic sequence only the daemon can allocate, and the Work Item
/// is a routing decision. Inventing either would fabricate business state.
///
/// Work Item binding is daemon-owned end to end: the daemon binds the session
/// server-side (e.g. after a bootstrap `workitem.create`), and re-opening the
/// same external key without a `workItemId` preserves that binding
/// (`service_sessions.rs` only overwrites `current_work_item` when the request
/// carries one). This function must therefore never send a `workItemId` it
/// invented on `session.open`.
async fn bind_host_session(
    client: &DaemonClient,
    identity: &HostIdentity,
    hook_event_id: &str,
    bootstrap_activation: bool,
    timeout_ms: u64,
) -> Result<HookBinding, String> {
    let project_key = project_key_for(&identity.project_root);
    let mut register = empty_params(
        json!({
            "projectRoot": identity.project_root.display().to_string(),
            "projectKey": project_key,
        }),
        timeout_ms,
    );
    // The event-scoped key keeps same-event retries idempotent while allowing a
    // later Hook to resolve the workspace's current mode and generation. A
    // root-only key would replay the original Shadow registration after
    // bootstrap activation and feed a stale projection into session.open.
    register.idempotency_key = Some(workspace_registration_idempotency_key(
        &identity.project_root,
        hook_event_id,
    ));
    let workspace: WorkspaceBinding = client
        .call(RpcMethod::WorkspaceRegister, register)
        .await
        .map_err(|error| format!("workspace.register failed: {error}"))?;

    let workspace = if requires_bootstrap_activation(bootstrap_activation, workspace.mode) {
        let now = now_unix_ms();
        let mut activate = empty_params(json!({"bootstrapActivation":true}), timeout_ms);
        activate.workspace_id = Some(workspace.workspace_id.clone());
        activate.idempotency_key = Some(hook_identity_idempotency_key(
            "workspace-activation",
            &identity.project_root.display().to_string(),
        ));
        activate.confirmation = Some(ConfirmationRef {
            confirmation_id: hook_identity_idempotency_key(
                "command-confirmation",
                &format!("{}:/ae-sdd", identity.external_key),
            ),
            approved_by: "user:/ae-sdd".to_owned(),
            approved_at: now.to_string(),
        });
        client
            .call(RpcMethod::WorkspaceModeTransition, activate)
            .await
            .map_err(|error| format!("workspace bootstrap activation failed: {error}"))?
    } else {
        workspace
    };

    // The daemon rejects an `engaged` claim that disagrees with its own
    // workspace policy, so it is derived from the registered mode, never chosen.
    let engaged = matches!(
        workspace.mode,
        WorkspaceMode::RustCanary | WorkspaceMode::RustSoleWriter
    );
    let agent_id = std::env::var("AE_SDD_AGENT_ID")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_HOOK_AGENT_ID.to_owned());
    let mut open = empty_params(
        json!({
            "externalKey": identity.external_key,
            "role": "root",
            "engaged": engaged,
        }),
        timeout_ms,
    );
    open.workspace_id = Some(workspace.workspace_id.clone());
    open.agent_id = Some(agent_id.clone());
    open.idempotency_key = Some(session_open_idempotency_key(
        &identity.external_key,
        hook_event_id,
    ));
    let session: SessionBinding = client
        .call(RpcMethod::SessionOpen, open)
        .await
        .map_err(|error| format!("session.open failed: {error}"))?;

    Ok(HookBinding {
        workspace_id: workspace.workspace_id,
        agent_id,
        session_id: session.session_id,
        capability_token: session.capability_token,
    })
}

/// Derives a stable project key from a workspace root.
///
/// A project key admits only `[A-Za-z0-9._:-]`, must start alphanumeric, and is
/// bounded to 64 bytes. Every other byte is folded to `-`; long leaf names keep
/// a readable prefix and a canonical-root digest suffix so truncation cannot
/// collapse distinct workspaces.
fn project_key_for(root: &Path) -> String {
    let name = root
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut key: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || ".:-_".contains(character) {
                character
            } else {
                '-'
            }
        })
        .collect();
    if !key
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
    {
        key.insert_str(0, "ws");
    }
    if key.len() <= 64 {
        return key;
    }

    let digest = ArtifactDigest::digest(root.display().to_string().as_bytes()).to_string();
    key.truncate(31);
    format!("{key}-{}", &digest[..32])
}

/// Produces a stable identity-transition key within the runtime's 128-byte
/// contract, even when the host identity itself occupies its full 128 bytes.
fn hook_identity_idempotency_key(namespace: &str, identity: &str) -> String {
    format!(
        "hook-{namespace}-{}",
        ArtifactDigest::digest(identity.as_bytes())
    )
}

fn session_open_idempotency_key(external_key: &str, hook_event_id: &str) -> String {
    hook_identity_idempotency_key("session", &format!("{external_key}\0{hook_event_id}"))
}

fn requires_bootstrap_activation(requested: bool, mode: WorkspaceMode) -> bool {
    requested && mode == WorkspaceMode::Shadow
}

fn workspace_registration_idempotency_key(root: &Path, hook_event_id: &str) -> String {
    hook_identity_idempotency_key("workspace", &format!("{}\0{hook_event_id}", root.display()))
}

/// Applies a freshly created binding to the Hook request.
///
/// The capability doubles as the offline proof so a daemon that dies between the
/// binding and the Hook call still reaches the signed fail-closed path instead
/// of an unbound one. `turnId` and `workItemId` stay absent: the daemon
/// allocates the turn and resolves the Work Item from the session binding.
fn apply_hook_binding(request: &mut HookRequest, binding: &HookBinding) {
    request.params.workspace_id = Some(binding.workspace_id.clone());
    request.params.agent_id = Some(binding.agent_id.clone());
    request.params.session_id = Some(binding.session_id.clone());
    request.params.capability_token = Some(binding.capability_token.clone());
    request.offline_capability = Some(binding.capability_token.clone());
}

fn host_binding_available() -> bool {
    missing_binding_variables(|name| std::env::var(name).ok()).is_empty()
}

/// Returns the binding variables the host did not inject, in registry order.
///
/// The resolver is injected so the rule is testable without mutating
/// process-global environment state from concurrent tests.
fn missing_binding_variables<F>(resolve: F) -> Vec<&'static str>
where
    F: Fn(&str) -> Option<String>,
{
    HOST_BINDING_VARIABLES
        .into_iter()
        .filter(|name| !resolve(name).is_some_and(|value| !value.trim().is_empty()))
        .collect()
}

/// Names the absent binding inputs so an unbound host event is diagnosable.
///
/// The generic phrasing alone cannot distinguish "the host injects nothing" from
/// "one variable is missing", which is the difference between a broken host
/// adapter and a single missing field.
fn missing_binding_reason() -> String {
    binding_reason(&missing_binding_variables(|name| std::env::var(name).ok()))
}

fn binding_reason(missing: &[&str]) -> String {
    format!(
        "trusted hook session binding is unavailable: missing {}",
        missing.join(", ")
    )
}

/// Writes one fail-closed diagnostic to stderr.
///
/// `additionalContext` is injected verbatim into the Agent's context, so the
/// cause cannot travel on stdout without polluting the conversation. stderr is
/// the host's debug channel and is never treated as Hook output.
fn report_hook_fail_closed(method: RpcMethod, reason: &str) {
    eprintln!("ae-sdd: {} fail-closed: {reason}", method.as_str());
}

/// Formats the stderr note for a Work Item the daemon bound during the Hook.
///
/// stdout carries the fixed per-method host JSON contract, so a daemon-minted
/// binding cannot be added there; the note keeps it observable on the debug
/// channel in the same style as `report_hook_fail_closed`.
fn hook_work_item_note(method: RpcMethod, outcome: &HookOutcome) -> Option<String> {
    outcome.work_item_id.as_deref().map(|work_item_id| {
        format!(
            "ae-sdd: {} bound workItemId: {work_item_id}",
            method.as_str()
        )
    })
}

fn host_fail_closed(method: RpcMethod, reason: &str) -> Value {
    match method {
        RpcMethod::HookPreTool => json!({"decision":"deny","reason":reason}),
        RpcMethod::HookStop => json!({"decision":"block","reason":reason}),
        // `additionalContext` stays empty so a failure never becomes Agent
        // context; the sibling `reason` carries the cause for hosts that log
        // the raw Hook response.
        RpcMethod::HookUserPrompt => json!({"additionalContext":"","reason":reason}),
        RpcMethod::HookPostTool => json!({"decision":"allow","reason":reason}),
        _ => json!({"decision":"deny","reason":reason}),
    }
}

fn print_host_outcome(method: RpcMethod, outcome: &HookOutcome) -> Result<(), String> {
    let reason = outcome
        .error_code
        .map_or_else(|| "".to_owned(), |code| code.as_str().to_owned());
    // An offline outcome carrying a stable code is a fail-closed decision the
    // daemon never confirmed. Its cause belongs on stderr for every method,
    // including the two whose host contract has no visible reason field.
    if outcome.offline && !reason.is_empty() {
        report_hook_fail_closed(method, &reason);
    }
    // A daemon-minted Work Item binding (e.g. after a bootstrap
    // `workitem.create`) is observable on stderr only; the host JSON contract
    // below keeps its exact shape either way.
    if let Some(note) = hook_work_item_note(method, outcome) {
        eprintln!("{note}");
    }
    let value = match method {
        RpcMethod::HookPreTool => json!({
            "decision": if outcome.decision == HookDecision::Deny { "deny" } else { "allow" },
            "reason": reason,
        }),
        RpcMethod::HookStop => json!({
            "decision": if outcome.decision == HookDecision::Block { "block" } else { "allow" },
            "reason": reason,
        }),
        RpcMethod::HookUserPrompt => {
            let context = outcome.context.as_ref().map_or_else(
                || "".to_owned(),
                |value| serde_json::to_string(value).unwrap_or_default(),
            );
            json!({
                "additionalContext": context.chars().take(65_536).collect::<String>(),
                "reason": reason,
            })
        }
        RpcMethod::HookPostTool => json!({"decision":"allow","reason":reason}),
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
        RuntimeCommand::Trace {
            state_dir,
            track,
            since,
            turn,
            hook,
            name,
            failed,
            limit,
            format,
        } => {
            let directory = state_dir
                .map(Ok)
                .unwrap_or_else(default_state_dir)
                .map_err(|error| error.to_string())?;
            let since_ms = match since {
                Some(window) => {
                    let span = diagnostics::parse_window(&window)?;
                    Some(current_unix_ms()?.saturating_sub(span))
                }
                None => None,
            };
            let query = diagnostics::Query {
                since_ms,
                turn,
                hook,
                name,
                failed,
                limit,
            };
            diagnostics::run(&directory, track, format, &query)
        }
    }
}

/// Returns the current wall clock in epoch milliseconds.
fn current_unix_ms() -> Result<i64, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before the Unix epoch".to_owned())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| "system clock is out of range".to_owned())
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
    const MAX_JSON_INPUT_BYTES: usize = 1_048_576;

    let source = if argument == "-" {
        let mut bytes = Vec::new();
        io::stdin()
            .take((MAX_JSON_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("failed to read JSON stdin: {error}"))?;
        bounded_utf8_json(bytes, MAX_JSON_INPUT_BYTES)?
    } else if let Some(path) = argument.strip_prefix('@') {
        if path.is_empty() {
            return Err("JSON @file path is empty".to_owned());
        }
        let metadata = std::fs::metadata(path)
            .map_err(|error| format!("failed to inspect JSON input file: {error}"))?;
        if metadata.len() > MAX_JSON_INPUT_BYTES as u64 {
            return Err("JSON input exceeds the 1048576-byte limit".to_owned());
        }
        let bytes = std::fs::read(path)
            .map_err(|error| format!("failed to read JSON input file: {error}"))?;
        bounded_utf8_json(bytes, MAX_JSON_INPUT_BYTES)?
    } else {
        if argument.len() > MAX_JSON_INPUT_BYTES {
            return Err("JSON input exceeds the 1048576-byte limit".to_owned());
        }
        argument.to_owned()
    };
    serde_json::from_str(&source).map_err(|error| error.to_string())
}

fn bounded_utf8_json(bytes: Vec<u8>, maximum: usize) -> Result<String, String> {
    if bytes.len() > maximum {
        return Err("JSON input exceeds the 1048576-byte limit".to_owned());
    }
    let text = String::from_utf8(bytes).map_err(|_| "JSON input must be valid UTF-8".to_owned())?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn json_input_path(suffix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ae-sdd-json-input-{}-{}-{suffix}",
            std::process::id(),
            now_unix_ms()
        ))
    }

    #[test]
    fn json_argument_reads_a_bounded_utf8_file() {
        let path = json_input_path("valid.json");
        std::fs::write(&path, br#"{"value":7}"#).expect("fixture writes");
        let value: Value =
            read_json_argument(&format!("@{}", path.display())).expect("@file JSON input parses");
        let _ = std::fs::remove_file(&path);
        assert_eq!(value, json!({"value":7}));
    }

    #[test]
    fn json_argument_accepts_a_windows_utf8_bom_file() {
        let path = json_input_path("utf8-bom.json");
        let mut bytes = vec![0xef, 0xbb, 0xbf];
        bytes.extend_from_slice(br#"{"value":7}"#);
        std::fs::write(&path, bytes).expect("fixture writes");
        let value: Value = read_json_argument(&format!("@{}", path.display()))
            .expect("PowerShell UTF-8 BOM JSON input parses");
        let _ = std::fs::remove_file(&path);
        assert_eq!(value, json!({"value":7}));
    }

    #[test]
    fn json_argument_rejects_an_oversized_file() {
        let path = json_input_path("oversized.json");
        std::fs::write(&path, vec![b' '; 1_048_577]).expect("fixture writes");
        let error = read_json_argument::<Value>(&format!("@{}", path.display()))
            .expect_err("oversized JSON input is rejected");
        let _ = std::fs::remove_file(&path);
        assert!(error.contains("1048576"), "bounded diagnostic: {error}");
    }

    #[test]
    fn json_argument_rejects_non_utf8_without_echoing_bytes() {
        let path = json_input_path("invalid-utf8.json");
        std::fs::write(&path, [0xff, 0xfe, 0xfd]).expect("fixture writes");
        let error = read_json_argument::<Value>(&format!("@{}", path.display()))
            .expect_err("non-UTF-8 JSON input is rejected");
        let _ = std::fs::remove_file(&path);
        assert!(error.contains("UTF-8"), "typed diagnostic: {error}");
        assert!(!error.contains("255"), "input bytes must not leak: {error}");
    }

    #[test]
    fn default_hook_binding_budget_is_separate_from_the_fast_path() {
        let defaults = hook_timeouts(None);
        assert_eq!(defaults.rpc_ms, 250);
        assert_eq!(defaults.binding_ms, 2_000);

        let explicit = hook_timeouts(Some(700));
        assert_eq!(explicit.rpc_ms, 700);
        assert_eq!(explicit.binding_ms, 700);
    }

    #[test]
    fn bootstrap_hook_invocation_uses_the_binding_budget() {
        let defaults = hook_timeouts(None);
        assert_eq!(hook_invocation_timeout(defaults, false), 250);
        assert_eq!(hook_invocation_timeout(defaults, true), 2_000);
    }

    #[test]
    fn rpc_can_negotiate_the_host_adapter_client_kind_explicitly() {
        let arguments = Arguments::try_parse_from([
            "ae-sdd",
            "rpc",
            "--method",
            "host.register",
            "--params-json",
            "{}",
            "--client-kind",
            "host-adapter",
            "--host-register-json",
            "{}",
        ])
        .expect("host adapter client kind parses");
        let TopLevel::Rpc {
            client_kind,
            host_register_json,
            ..
        } = arguments.command
        else {
            panic!("rpc command expected");
        };
        assert_eq!(client_kind, RpcClientKind::HostAdapter);
        assert_eq!(host_register_json.as_deref(), Some("{}"));
    }

    #[test]
    fn a_binding_is_available_only_when_every_variable_carries_a_value() {
        assert!(
            missing_binding_variables(|name| Some(format!("{name}-value"))).is_empty(),
            "a fully injected binding has nothing missing"
        );
        assert_eq!(
            missing_binding_variables(|_| None),
            HOST_BINDING_VARIABLES.to_vec(),
            "an uninjected host reports every variable, not a generic failure"
        );
        // Whitespace is not a binding: a host that exports an empty variable
        // must not be treated as trusted.
        assert_eq!(
            missing_binding_variables(|name| Some(if name == "AE_SDD_TURN_ID" {
                "   ".to_owned()
            } else {
                "value".to_owned()
            })),
            vec!["AE_SDD_TURN_ID"]
        );
    }

    #[test]
    fn the_fail_closed_reason_names_the_absent_variables() {
        let reason = binding_reason(&missing_binding_variables(|_| None));
        for name in HOST_BINDING_VARIABLES {
            assert!(
                reason.contains(name),
                "{name} must appear in the diagnostic reason: {reason}"
            );
        }
    }

    #[test]
    fn a_project_key_stays_within_the_identifier_alphabet() {
        assert_eq!(project_key_for(Path::new(r"D:\Item\ae-sdd")), "ae-sdd");
        // Spaces and other bytes outside the alphabet fold to `-`.
        assert_eq!(
            project_key_for(Path::new(r"C:\work\my project (v2)")),
            "my-project--v2-"
        );
        // A key must start alphanumeric, so an unusable leading byte is
        // prefixed rather than dropped, which would collide across roots.
        assert_eq!(project_key_for(Path::new("/srv/.hidden")), "ws.hidden");
        for key in [
            project_key_for(Path::new("/srv/.hidden")),
            project_key_for(Path::new(r"C:\work\my project (v2)")),
        ] {
            assert!(
                key.chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric()),
                "{key} must start alphanumeric"
            );
            assert!(
                key.chars()
                    .all(|c| c.is_ascii_alphanumeric() || ".:-_".contains(c)),
                "{key} must stay inside the identifier alphabet"
            );
            assert!(key.len() <= 64);
        }
    }

    #[test]
    fn long_project_names_stay_in_domain_bounds_without_colliding() {
        let shared = "a".repeat(96);
        let first = project_key_for(Path::new(&format!("{shared}-first")));
        let second = project_key_for(Path::new(&format!("{shared}-second")));

        assert!(
            first.len() <= 64,
            "ProjectKey is capped at 64 bytes: {first}"
        );
        assert!(
            second.len() <= 64,
            "ProjectKey is capped at 64 bytes: {second}"
        );
        assert_ne!(
            first, second,
            "the canonical root digest disambiguates long names"
        );
    }

    #[test]
    fn maximum_external_session_keys_produce_bounded_idempotency_keys() {
        let maximum_external_key = "s".repeat(128);
        let first = hook_identity_idempotency_key("session", &maximum_external_key);
        let second = hook_identity_idempotency_key("session", &format!("{}t", "s".repeat(127)));

        assert!(
            first.len() <= 128,
            "identity key exceeds runtime bound: {first}"
        );
        assert_ne!(
            first, second,
            "different external identities must not collide"
        );
    }

    #[test]
    fn each_hook_event_gets_a_refreshable_session_open_receipt() {
        let first = session_open_idempotency_key("host-session", "event-1");
        let retry = session_open_idempotency_key("host-session", "event-1");
        let later = session_open_idempotency_key("host-session", "event-2");

        assert_eq!(first, retry, "one Hook request must retry idempotently");
        assert_ne!(
            first, later,
            "a later Hook event must commit a refreshed durable session after-image"
        );
        assert!(later.len() <= 128);
    }

    #[test]
    fn repeated_ae_sdd_skips_activation_after_workspace_enrollment() {
        assert!(requires_bootstrap_activation(true, WorkspaceMode::Shadow));
        assert!(!requires_bootstrap_activation(
            true,
            WorkspaceMode::RustCanary
        ));
        assert!(!requires_bootstrap_activation(
            true,
            WorkspaceMode::RustSoleWriter
        ));
        assert!(!requires_bootstrap_activation(false, WorkspaceMode::Shadow));
    }

    #[test]
    fn each_hook_event_refreshes_the_current_workspace_registration_projection() {
        let root = Path::new(r"D:\Item\ae-sdd");
        let first = workspace_registration_idempotency_key(root, "event-1");
        let retry = workspace_registration_idempotency_key(root, "event-1");
        let later = workspace_registration_idempotency_key(root, "event-2");

        assert_eq!(first, retry, "one Hook request must retry idempotently");
        assert_ne!(
            first, later,
            "a later Hook event must resolve the current workspace mode and generation"
        );
        assert!(later.len() <= 128);
    }

    #[test]
    fn host_identity_accepts_codex_session_thread_and_conversation_ids() {
        for (field, value) in [
            ("session_id", "session-snake"),
            ("sessionId", "session-camel"),
            ("thread_id", "thread-snake"),
            ("threadId", "thread-camel"),
            ("conversation_id", "conversation-snake"),
            ("conversationId", "conversation-camel"),
        ] {
            let mut event = serde_json::Map::from_iter([
                ("hook_event_name".to_owned(), json!("user_prompt_submit")),
                ("prompt".to_owned(), json!("/ae-sdd")),
                ("cwd".to_owned(), json!(r"D:\\Item\\ae-sdd")),
            ]);
            event.insert(field.to_owned(), json!(value));
            let parsed = parse_hook_request(&RpcMethod::HookUserPrompt, Value::Object(event))
                .unwrap_or_else(|error| panic!("{field} must parse: {error}"));
            assert_eq!(
                parsed
                    .host_identity
                    .as_ref()
                    .map(|identity| identity.external_key.as_str()),
                Some(value),
                "{field} must bootstrap the same trusted session identity"
            );
        }

        let parsed = parse_hook_request(
            &RpcMethod::HookUserPrompt,
            json!({
                "hook_event_name":"user_prompt_submit",
                "prompt":"/ae-sdd",
                "cwd":r"D:\\Item\\ae-sdd",
                "session_id":"session",
                "thread_id":"thread",
                "conversation_id":"conversation",
            }),
        )
        .expect("a multi-identity host event parses");
        assert_eq!(
            parsed
                .host_identity
                .as_ref()
                .map(|identity| identity.external_key.as_str()),
            Some("session"),
            "session identity takes precedence over thread and conversation"
        );
    }

    #[test]
    fn only_the_exact_ae_sdd_prompt_requests_bootstrap_enrollment() {
        let exact = parse_hook_request(
            &RpcMethod::HookUserPrompt,
            json!({
                "hook_event_name":"user_prompt_submit",
                "prompt":"/ae-sdd",
                "cwd":r"D:\Item\ae-sdd",
                "session_id":"session",
            }),
        )
        .expect("exact command parses");
        let ordinary = parse_hook_request(
            &RpcMethod::HookUserPrompt,
            json!({
                "hook_event_name":"user_prompt_submit",
                "prompt":"implement the story",
                "cwd":r"D:\Item\ae-sdd",
                "session_id":"session",
            }),
        )
        .expect("ordinary prompt parses");
        let prefixed = parse_hook_request(
            &RpcMethod::HookUserPrompt,
            json!({
                "hook_event_name":"user_prompt_submit",
                "prompt":"/ae-sdd extra",
                "cwd":r"D:\Item\ae-sdd",
                "session_id":"session",
            }),
        )
        .expect("prefixed command parses");

        assert!(is_bootstrap_activation(
            RpcMethod::HookUserPrompt,
            exact.request.as_ref().expect("request")
        ));
        assert!(!is_bootstrap_activation(
            RpcMethod::HookUserPrompt,
            ordinary.request.as_ref().expect("request")
        ));
        assert!(!is_bootstrap_activation(
            RpcMethod::HookUserPrompt,
            prefixed.request.as_ref().expect("request")
        ));
    }

    /// A binding must never invent a turn or a Work Item: the turn is a durable
    /// monotonic sequence only the daemon can order, and the Work Item is a
    /// routing decision.
    #[test]
    fn applying_a_binding_sets_identity_but_never_turn_or_work_item() {
        let mut request = HookRequest {
            params: empty_params(json!({}), 250),
            engaged: true,
            offline_capability: None,
            now_unix_ms: 0,
        };
        let binding = HookBinding {
            workspace_id: "workspace-1".to_owned(),
            agent_id: "agent-1".to_owned(),
            session_id: "session-1".to_owned(),
            capability_token: "capability-1".to_owned(),
        };

        apply_hook_binding(&mut request, &binding);

        assert_eq!(request.params.workspace_id.as_deref(), Some("workspace-1"));
        assert_eq!(request.params.agent_id.as_deref(), Some("agent-1"));
        assert_eq!(request.params.session_id.as_deref(), Some("session-1"));
        assert_eq!(
            request.params.capability_token.as_deref(),
            Some("capability-1")
        );
        // The capability doubles as the offline proof so a daemon that dies
        // after binding still reaches the signed fail-closed path.
        assert_eq!(request.offline_capability.as_deref(), Some("capability-1"));
        assert!(request.params.turn_id.is_none());
        assert!(request.params.work_item_id.is_none());
    }

    /// A daemon-bound Work Item must be observable without polluting stdout:
    /// the host JSON contract shape is fixed per method, so the binding is
    /// reported as one stderr diagnostic line, mirroring the fail-closed
    /// diagnostics style.
    #[test]
    fn a_bound_work_item_is_a_stderr_note_never_host_output() {
        let mut outcome = HookOutcome {
            engaged: true,
            decision: HookDecision::Allow,
            context: None,
            event_seq: 41,
            offline: false,
            error_code: None,
            turn_id: Some("turn-7".to_owned()),
            turn_seq: Some(7),
            work_item_id: Some("WI-20260728-bootstrap".to_owned()),
        };

        let note = hook_work_item_note(RpcMethod::HookPostTool, &outcome)
            .expect("a daemon-bound Work Item must be reported");
        assert!(
            note.contains("WI-20260728-bootstrap"),
            "the note must name the bound Work Item: {note}"
        );

        outcome.work_item_id = None;
        assert!(
            hook_work_item_note(RpcMethod::HookPostTool, &outcome).is_none(),
            "an outcome without a binding stays silent"
        );
    }

    /// The host contract per method is a safety boundary. `user_prompt` must
    /// keep an empty `additionalContext` so a failure never becomes Agent
    /// context, while still exposing its cause on a sibling field.
    #[test]
    fn host_fail_closed_keeps_each_host_contract_and_exposes_the_reason() {
        let reason = "binding unavailable";
        let cases = [
            (RpcMethod::HookPreTool, Some("deny")),
            (RpcMethod::HookStop, Some("block")),
            (RpcMethod::HookPostTool, Some("allow")),
            (RpcMethod::HookUserPrompt, None),
        ];

        for (method, decision) in cases {
            let value = host_fail_closed(method, reason);
            assert_eq!(
                value["reason"], reason,
                "{method:?} must expose its fail-closed cause"
            );
            match decision {
                Some(expected) => assert_eq!(value["decision"], expected, "{method:?}"),
                None => {
                    assert_eq!(
                        value["additionalContext"], "",
                        "a fail-closed prompt must inject no context"
                    );
                    assert!(
                        value.get("decision").is_none(),
                        "user_prompt has no host decision field"
                    );
                }
            }
        }
    }
}
