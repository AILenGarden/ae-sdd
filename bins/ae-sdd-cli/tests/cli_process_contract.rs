use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::json;

const ROUTING_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/compatibility/cli-routing.v1.json"
));

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoutingManifest {
    commands: Vec<ManifestCommand>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestCommand {
    id: String,
    route: ManifestRoute,
    identity: ManifestIdentity,
    status: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestIdentity {
    workspace: bool,
    work_item: bool,
    session: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestRoute {
    kind: String,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    operation: Option<String>,
}

fn manifest() -> RoutingManifest {
    serde_json::from_str(ROUTING_JSON).expect("frozen routing manifest")
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

fn sandbox() -> &'static Path {
    static SANDBOX: OnceLock<PathBuf> = OnceLock::new();
    SANDBOX
        .get_or_init(|| {
            let path = repository_root()
                .join("target")
                .join(format!("cli-process-contract-{}", std::process::id()));
            fs::create_dir_all(&path).expect("process-test sandbox");
            path
        })
        .as_path()
}

fn isolated_cli() -> &'static Path {
    static CLI: OnceLock<PathBuf> = OnceLock::new();
    CLI.get_or_init(|| {
        let source = PathBuf::from(env!("CARGO_BIN_EXE_ae-sdd"));
        let destination =
            sandbox().join(source.file_name().expect("CLI executable has a file name"));
        fs::copy(&source, &destination).expect("copy instrumented CLI");
        destination
    })
    .as_path()
}

fn noop_build() -> &'static Path {
    static BUILD: OnceLock<PathBuf> = OnceLock::new();
    BUILD
        .get_or_init(|| {
            let source = sandbox().join("noop-build.rs");
            let executable =
                sandbox().join(format!("ae-sdd-build{}", std::env::consts::EXE_SUFFIX));
            fs::write(&source, "fn main() {}\n").expect("write no-op build source");
            let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
            let status = Command::new(rustc)
                .arg(&source)
                .arg("-o")
                .arg(&executable)
                .status()
                .expect("compile no-op build boundary");
            assert!(status.success(), "compile no-op build boundary");
            executable
        })
        .as_path()
}

fn missing_manifest() -> String {
    sandbox()
        .join("missing-endpoint.json")
        .display()
        .to_string()
}

fn daemon_executable() -> Option<PathBuf> {
    let cargo_cli = PathBuf::from(env!("CARGO_BIN_EXE_ae-sdd"));
    let executable = format!("ae-sddd{}", std::env::consts::EXE_SUFFIX);
    [
        cargo_cli.parent().map(|parent| parent.join(&executable)),
        Some(repository_root().join("target/debug").join(&executable)),
        Some(repository_root().join("target/release").join(executable)),
    ]
    .into_iter()
    .flatten()
    .find(|candidate| candidate.is_file())
}

struct DaemonGuard {
    manifest: PathBuf,
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        if self.manifest.is_file() {
            let _ = Command::new(isolated_cli())
                .args(["runtime", "stop", "--manifest"])
                .arg(&self.manifest)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

fn run_cli(args: &[String], stdin: Option<&str>) -> Output {
    let mut command = Command::new(isolated_cli());
    command
        .args(args)
        .current_dir(repository_root())
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for name in [
        "AE_SDD_AGENT_ID",
        "AE_SDD_CAPABILITY_TOKEN",
        "AE_SDD_CONFIRMATION_APPROVED_AT",
        "AE_SDD_CONFIRMATION_APPROVED_BY",
        "AE_SDD_CONFIRMATION_ID",
        "AE_SDD_DEADLINE_MS",
        "AE_SDD_EXPECTED_REVISION",
        "AE_SDD_FENCING_TOKEN",
        "AE_SDD_HOOK_ENGAGED",
        "AE_SDD_IDEMPOTENCY_KEY",
        "AE_SDD_LEASE_ID",
        "AE_SDD_MANIFEST",
        "AE_SDD_SESSION_ID",
        "AE_SDD_TURN_ID",
        "AE_SDD_WORKSPACE_ID",
        "AE_SDD_WORK_ITEM_ID",
    ] {
        command.env_remove(name);
    }
    let mut child = command.spawn().expect("spawn isolated CLI");
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(input.as_bytes())
            .expect("write CLI stdin");
    }
    child.wait_with_output().expect("collect CLI output")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn base_args(command: &ManifestCommand, ordinal: usize) -> Vec<String> {
    let mut args = command
        .id
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if command.identity.workspace {
        args.extend(["--workspace-id".to_owned(), "workspace-1".to_owned()]);
    }
    if command.identity.work_item {
        args.extend(["--work-item-id".to_owned(), "WORK-1".to_owned()]);
    }
    if command.identity.session {
        args.extend([
            "--session-id".to_owned(),
            "session-1".to_owned(),
            "--agent-id".to_owned(),
            "agent-1".to_owned(),
            "--capability-token".to_owned(),
            "capability-1".to_owned(),
        ]);
    }
    args.extend([
        "--idempotency-key".to_owned(),
        format!("cli-process-{ordinal}"),
        "--manifest".to_owned(),
        missing_manifest(),
    ]);
    args
}

fn typed_payload(operation: &str) -> serde_json::Value {
    match operation {
        "document.resolve" => json!({"intent":"STORY"}),
        "document.save" => json!({"intent":"STORY","contentFile":"story.md"}),
        "evidence.finalize" | "lease.status" | "state.next_actions" | "workitem.complete"
        | "workitem.get" => json!({}),
        "evidence.record" => {
            json!({"artifactPath":"artifact.json","inputFingerprint":"sha256:input"})
        }
        "execution.plan.approve" => json!({"approvedBy":"user"}),
        "execution.plan.set" => json!({
            "goal":"exercise production CLI adapter",
            "changedPaths":["Cargo.toml"],
            "verification":[{"id":"V-1","acId":"AC-1","command":"cargo test"}]
        }),
        "gate.check" => json!({"gateIds":["G-08"]}),
        "lease.acquire" => json!({"owner":{"agentId":"agent-1"},"ttlSeconds":60}),
        "lease.break" => json!({"actor":{"agentId":"agent-1"},"reason":"test"}),
        "lease.release" => json!({"owner":{"agentId":"agent-1"}}),
        "lease.renew" => json!({"owner":{"agentId":"agent-1"},"ttlSeconds":60}),
        "review.record" => json!({"status":"passed","findings":[]}),
        "state.transition" => json!({"targetPhase":"coding"}),
        "verification.plan" => json!({"changedPaths":["Cargo.toml"]}),
        other => panic!("missing typed payload for {other}"),
    }
}

fn add_typed_arguments(args: &mut Vec<String>, operation: &str) {
    args.extend([
        "--lease-id".to_owned(),
        "lease-1".to_owned(),
        "--fencing-token".to_owned(),
        "1".to_owned(),
        "--expected-revision".to_owned(),
        "1".to_owned(),
        "--confirmation-id".to_owned(),
        "confirmation-1".to_owned(),
        "--approved-by".to_owned(),
        "user".to_owned(),
        "--approved-at".to_owned(),
        "2026-07-26T00:00:00Z".to_owned(),
        "--payload-json".to_owned(),
        typed_payload(operation).to_string(),
    ]);
}

fn add_job_arguments(args: &mut Vec<String>, command_id: &str) {
    let values: &[&str] = match command_id {
        "assets query" => &["needle"],
        "assets read" => &["coding"],
        "assets section" => &["section-a"],
        "gate doc-storage" => &["--path", "Cargo.toml"],
        "memory update" => &["--slice", "decisions"],
        "memory common" => &["read"],
        "memory search" => &["--query", "needle"],
        "baseline diff" => &["--report", "{}"],
        "classify" => &["--text", "sample"],
        "db query" | "db explain" => &["--profile", "test", "--sql", "SELECT 1"],
        "evidence lookup" => &[
            "--command",
            "cargo test",
            "--input-fingerprint",
            "sha256:input",
            "--story",
            "STORY-1",
            "--toolchain-fingerprint",
            "sha256:toolchain",
        ],
        "git blame" => &["--file", "Cargo.toml"],
        "plugin trace" => &["sample-plugin"],
        _ => &[],
    };
    args.extend(values.iter().map(|value| (*value).to_owned()));
}

fn add_passthrough_arguments(args: &mut Vec<String>, command_id: &str) {
    let values: &[&str] = match command_id {
        "ops describe" => &["--operation", "workitem.get"],
        "ops next" => &["--story", "STORY-1", "--project", "project-root"],
        "review abort" => &["--delegation-id", "delegation-1", "--reason", "test"],
        "review collect" | "review-loop collect" => &["--delegation-id", "delegation-1"],
        "flow-violation-scan"
        | "gate coding-required"
        | "gate ra-required"
        | "ra-authenticity-scan"
        | "ra-depth-scan"
        | "ra-implementation-scan" => &["--project", "project-root", "--strict"],
        _ => &[],
    };
    args.extend(values.iter().map(|value| (*value).to_owned()));
}

fn native_arguments(command_id: &str) -> Vec<String> {
    let root = repository_root().display().to_string();
    let registry = sandbox().join("distributors.json").display().to_string();
    let project = sandbox().join("new-project").display().to_string();
    let plugins = sandbox().join("plugins").display().to_string();
    match command_id {
        "assets generate" => vec![
            "--project-root".into(),
            root,
            "--project-key".into(),
            "sample".into(),
        ],
        "bump" => vec![
            "3.15.0".into(),
            "--repository-root".into(),
            repository_root().display().to_string(),
            "--expected-version".into(),
            "3.14.0".into(),
        ],
        "distributor disable" | "distributor enable" | "distributor unregister" => {
            vec!["codex".into(), "--registry-file".into(), registry]
        }
        "distributor list" | "distributor scan" => {
            vec!["--registry-file".into(), registry]
        }
        "distributor register" => vec![
            "codex".into(),
            "--protocol".into(),
            "copytree".into(),
            "--target-path".into(),
            sandbox().join("codex").display().to_string(),
            "--registry-file".into(),
            registry,
        ],
        "init" => vec![project, "sample".into(), "--force".into()],
        "init-hooks" => vec![
            root,
            "--executable".into(),
            isolated_cli().display().to_string(),
            "--hosts".into(),
            "claude,codex".into(),
        ],
        "plugin init" => vec![
            "--plugins-root".into(),
            plugins,
            "--name".into(),
            "sample-plugin".into(),
            "--description".into(),
            "sample".into(),
        ],
        "runtime verify" => vec!["--path".into(), repository_root().display().to_string()],
        "version" => vec!["--json".into()],
        other => panic!("missing native arguments for {other}"),
    }
}

#[test]
fn every_frozen_rejected_route_fails_closed_in_the_production_binary() {
    let mut count = 0;
    for command in manifest()
        .commands
        .into_iter()
        .filter(|command| command.route.kind == "rejected" || command.status == "pending")
    {
        let args = command
            .id
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let output = run_cli(&args, None);
        assert!(
            !output.status.success(),
            "{} unexpectedly passed",
            command.id
        );
        let stderr = text(&output.stderr);
        assert!(
            stderr.contains("removed")
                || stderr.contains("pending verified parity")
                || stderr.contains("LEGACY")
                || (command.id == "runtime compact"
                    && stderr.contains("unrecognized subcommand 'compact'")),
            "{} did not expose a stable fail-closed reason: {stderr}",
            command.id
        );
        count += 1;
    }
    assert_eq!(count, 38);
}

#[test]
fn every_native_route_builds_a_typed_request_and_stops_at_the_noop_boundary() {
    let build = noop_build();
    assert_eq!(build.parent(), isolated_cli().parent());
    let commands = manifest().commands;
    let native = commands
        .iter()
        .filter(|command| command.route.kind == "native-build-job")
        .collect::<Vec<_>>();
    assert_eq!(native.len(), 13);
    for command in native {
        let mut args = command
            .id
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        args.extend(native_arguments(&command.id));
        let output = run_cli(&args, None);
        if command.id == "runtime verify" {
            assert!(!output.status.success());
            assert!(
                text(&output.stderr).contains("unrecognized subcommand 'verify'"),
                "{} did not fail closed in Clap: {}",
                command.id,
                text(&output.stderr)
            );
        } else {
            assert!(
                output.status.success(),
                "{} did not reach the no-op build boundary: {}",
                command.id,
                text(&output.stderr)
            );
        }
    }

    let request = sandbox().join("version-request.json");
    fs::write(
        &request,
        serde_json::to_vec(&json!({
            "schemaVersion":"ae-sdd-offline-build/v1",
            "command":"version"
        }))
        .expect("version request JSON"),
    )
    .expect("write version request");
    let output = run_cli(
        &[
            "version".into(),
            "--request".into(),
            request.display().to_string(),
            "--json".into(),
        ],
        None,
    );
    assert!(
        output.status.success(),
        "explicit request did not reach no-op build boundary: {}",
        text(&output.stderr)
    );
}

#[test]
fn every_typed_operation_reaches_the_production_adapter_before_ipc() {
    let commands = manifest().commands;
    let typed = commands
        .iter()
        .filter(|command| command.route.kind == "typed-operation")
        .collect::<Vec<_>>();
    assert_eq!(typed.len(), 13);
    for (index, command) in typed.into_iter().enumerate() {
        let operation = command.route.operation.as_deref().expect("typed operation");
        let mut args = base_args(command, index);
        add_typed_arguments(&mut args, operation);
        let output = run_cli(&args, None);
        assert!(
            !output.status.success(),
            "{} unexpectedly passed",
            command.id
        );
        let stderr = text(&output.stderr);
        assert!(
            stderr.contains("endpoint")
                || stderr.contains("manifest")
                || stderr.contains("typed operation payload is invalid"),
            "{} did not reach the typed adapter or IPC boundary: {stderr}",
            command.id
        );
    }
}

#[test]
fn every_rpc_route_reaches_its_command_specific_adapter_before_ipc() {
    let operation_request = sandbox().join("operation-request.json");
    fs::write(
        &operation_request,
        serde_json::to_vec(&json!({
            "schemaVersion":"1",
            "operation":"workitem.get",
            "project":repository_root(),
            "workItem":"WORK-1",
            "parameters":{}
        }))
        .expect("request JSON"),
    )
    .expect("write operation request");

    let rpc = manifest()
        .commands
        .into_iter()
        .filter(|command| command.route.kind == "rpc")
        .collect::<Vec<_>>();
    assert_eq!(rpc.len(), 49);
    for (index, command) in rpc.into_iter().enumerate() {
        let mut args = base_args(&command, 100 + index);
        if command.route.method.as_deref() == Some("job.submit") {
            add_job_arguments(&mut args, &command.id);
        } else if command.id == "ops execute" {
            args.extend([
                "--request-file".to_owned(),
                operation_request.display().to_string(),
            ]);
        } else {
            add_passthrough_arguments(&mut args, &command.id);
        }
        let output = run_cli(&args, None);
        assert!(
            !output.status.success(),
            "{} unexpectedly passed",
            command.id
        );
        let stderr = text(&output.stderr);
        assert!(
            !stderr.contains("unknown or removed deprecated legacy command") && !stderr.is_empty(),
            "{} bypassed its frozen adapter: {stderr}",
            command.id
        );
    }
}

#[test]
fn top_level_rpc_hook_runtime_and_stdin_paths_are_process_verified() {
    let invalid_method = run_cli(
        &[
            "rpc".into(),
            "--method".into(),
            "not.registered".into(),
            "--params-json".into(),
            "{}".into(),
        ],
        None,
    );
    assert!(!invalid_method.status.success());
    assert!(text(&invalid_method.stderr).contains("RPC method is not registered"));

    let handshake = run_cli(
        &[
            "rpc".into(),
            "--method".into(),
            "runtime.handshake".into(),
            "--params-json".into(),
            "{}".into(),
        ],
        None,
    );
    assert!(!handshake.status.success());
    assert!(text(&handshake.stderr).contains("managed by the client"));

    let malformed_stdin = run_cli(
        &[
            "rpc".into(),
            "--method".into(),
            "runtime.status".into(),
            "--params-json".into(),
            "-".into(),
        ],
        Some("not-json"),
    );
    assert!(!malformed_stdin.status.success());
    assert!(text(&malformed_stdin.stderr).contains("expected"));

    for (method, expected) in [
        ("hook.pre_tool", "\"decision\":\"deny\""),
        ("hook.stop", "\"decision\":\"block\""),
        ("hook.user_prompt", "\"additionalContext\":\"\""),
        ("hook.post_tool", "\"decision\":\"allow\""),
    ] {
        let output = run_cli(
            &[
                "hook".into(),
                "--method".into(),
                method.into(),
                "--request-json".into(),
                r#"{"hook_event_name":"test","tool_name":"Bash"}"#.into(),
                "--manifest".into(),
                missing_manifest(),
            ],
            None,
        );
        assert!(
            output.status.success(),
            "{method}: {}",
            text(&output.stderr)
        );
        assert!(text(&output.stdout).contains(expected));
    }

    let state_dir = sandbox().join("runtime-state");
    fs::create_dir_all(&state_dir).expect("runtime state dir");
    fs::write(state_dir.join("daemon.log"), "one\ntwo\nthree\n").expect("daemon log");
    let logs = run_cli(
        &[
            "runtime".into(),
            "logs".into(),
            "--state-dir".into(),
            state_dir.display().to_string(),
            "--tail".into(),
            "2".into(),
        ],
        None,
    );
    assert!(logs.status.success(), "{}", text(&logs.stderr));
    assert_eq!(text(&logs.stdout), "two\nthree\n");

    for action in ["status", "drain", "stop"] {
        let output = run_cli(
            &[
                "runtime".into(),
                action.into(),
                "--manifest".into(),
                missing_manifest(),
            ],
            None,
        );
        assert!(!output.status.success());
        assert!(!text(&output.stderr).is_empty());
    }
}

#[test]
fn successful_legacy_rpc_and_job_routes_flush_production_coverage() {
    let Some(daemon) = daemon_executable() else {
        eprintln!("skipping live legacy RPC coverage: no ae-sddd executable is available");
        return;
    };
    let state_dir = sandbox().join("live-legacy-rpc-state");
    fs::create_dir_all(&state_dir).expect("live daemon state dir");
    let endpoint_manifest = state_dir.join("endpoint.v1.json");
    let root = repository_root().display().to_string();
    let start = run_cli(
        &[
            "runtime".into(),
            "ensure".into(),
            "--daemon".into(),
            daemon.display().to_string(),
            "--state-dir".into(),
            state_dir.display().to_string(),
            "--allowed-root".into(),
            root.clone(),
            "--project-root".into(),
            root.clone(),
            "--timeout-ms".into(),
            "15000".into(),
        ],
        None,
    );
    assert!(
        start.status.success(),
        "start isolated daemon: {}",
        text(&start.stderr)
    );
    let _guard = DaemonGuard {
        manifest: endpoint_manifest.clone(),
    };

    let register_params = json!({
        "protocolVersion":"1.0",
        "idempotencyKey":"cli-process-workspace-register",
        "deadlineMs":5000,
        "payload":{"projectRoot":root,"projectKey":"cli-process-contract"}
    });
    let register = run_cli(
        &[
            "rpc".into(),
            "--method".into(),
            "workspace.register".into(),
            "--params-json".into(),
            register_params.to_string(),
            "--manifest".into(),
            endpoint_manifest.display().to_string(),
            "--timeout-ms".into(),
            "5000".into(),
        ],
        None,
    );
    assert!(
        register.status.success(),
        "register isolated workspace: {}",
        text(&register.stderr)
    );
    let registered: serde_json::Value =
        serde_json::from_slice(&register.stdout).expect("workspace.register response");
    let workspace_id = registered["workspaceId"]
        .as_str()
        .expect("workspace.register returns workspaceId")
        .to_owned();
    let work_item_id = "PRD-AE-SDD-RUST-DAEMON-001";
    let open_session_params = json!({
        "protocolVersion":"1.0",
        "workspaceId":workspace_id,
        "workItemId":work_item_id,
        "agentId":"cli-process-root",
        "idempotencyKey":"cli-process-session-open",
        "deadlineMs":5000,
        "payload":{
            "externalKey":"cli-process-contract",
            "role":"root",
            "engaged":false
        }
    });
    let open_session = run_cli(
        &[
            "rpc".into(),
            "--method".into(),
            "session.open".into(),
            "--params-json".into(),
            open_session_params.to_string(),
            "--manifest".into(),
            endpoint_manifest.display().to_string(),
            "--timeout-ms".into(),
            "5000".into(),
        ],
        None,
    );
    assert!(
        open_session.status.success(),
        "open isolated root session: {}",
        text(&open_session.stderr)
    );
    let session: serde_json::Value =
        serde_json::from_slice(&open_session.stdout).expect("session.open response");
    let session_id = session["sessionId"]
        .as_str()
        .expect("session.open returns sessionId")
        .to_owned();
    let capability_token = session["capabilityToken"]
        .as_str()
        .expect("session.open returns capabilityToken")
        .to_owned();

    for (command_id, trailing) in [
        ("health", Vec::<String>::new()),
        (
            "ops describe",
            vec!["--operation".into(), "workitem.get".into()],
        ),
    ] {
        let mut args = command_id
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        args.extend(trailing);
        args.extend([
            "--idempotency-key".into(),
            format!("cli-process-{}", command_id.replace(' ', "-")),
            "--manifest".into(),
            endpoint_manifest.display().to_string(),
            "--deadline-ms".into(),
            "5000".into(),
        ]);
        let output = run_cli(&args, None);
        assert!(
            output.status.success(),
            "{command_id}: {}",
            text(&output.stderr)
        );
    }

    let routing = manifest();
    for (index, command_id) in [
        "assets check",
        "assets outline",
        "assets query",
        "assets read",
        "assets stats",
        "automation status",
        "classify",
        "db audit",
        "db profiles",
        "evidence lookup",
        "git blame",
        "git impact",
        "git log",
        "git status",
        "perf doctor",
        "perf report",
        "plugin list",
        "plugin validate",
    ]
    .into_iter()
    .enumerate()
    {
        let route = routing
            .commands
            .iter()
            .find(|command| command.id == command_id)
            .expect("live job route is frozen");
        let mut args = command_id
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        args.extend([
            "--workspace-id".into(),
            workspace_id.clone(),
            "--idempotency-key".into(),
            format!("cli-process-live-job-{index}"),
            "--manifest".into(),
            endpoint_manifest.display().to_string(),
            "--deadline-ms".into(),
            "10000".into(),
        ]);
        add_job_arguments(&mut args, &route.id);
        let output = run_cli(&args, None);
        assert!(
            output.status.success(),
            "{command_id} legacy job: {}",
            text(&output.stderr)
        );
    }

    for (index, command_id) in [
        "iteration-check",
        "memory common",
        "memory read",
        "memory search",
    ]
    .into_iter()
    .enumerate()
    {
        let route = routing
            .commands
            .iter()
            .find(|command| command.id == command_id)
            .expect("authenticated live job route is frozen");
        let mut args = command_id
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        args.extend([
            "--workspace-id".into(),
            workspace_id.clone(),
            "--work-item-id".into(),
            work_item_id.into(),
            "--session-id".into(),
            session_id.clone(),
            "--agent-id".into(),
            "cli-process-root".into(),
            "--capability-token".into(),
            capability_token.clone(),
            "--idempotency-key".into(),
            format!("cli-process-authenticated-job-{index}"),
            "--manifest".into(),
            endpoint_manifest.display().to_string(),
            "--deadline-ms".into(),
            "10000".into(),
        ]);
        add_job_arguments(&mut args, &route.id);
        let output = run_cli(&args, None);
        assert!(
            output.status.success(),
            "{command_id} authenticated legacy job: {}",
            text(&output.stderr)
        );
    }
}
