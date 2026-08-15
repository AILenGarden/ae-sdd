use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

static TEMP_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let sequence = TEMP_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::AcqRel);
        let path =
            std::env::temp_dir().join(format!("ae-sdd-{label}-{}-{sequence}", std::process::id()));
        fs::create_dir(&path).expect("create isolated process-test TempDir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match fs::remove_dir_all(&self.path) {
                Ok(()) => return,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) if std::thread::panicking() => {
                    eprintln!(
                        "failed to clean process-test TempDir {}: {error}",
                        self.path.display()
                    );
                    return;
                }
                Err(error) => panic!(
                    "failed to clean process-test TempDir {}: {error}",
                    self.path.display()
                ),
            }
        }
    }
}

struct ProcessFixture {
    root: TempDir,
    cli: OnceLock<PathBuf>,
    noop_build: OnceLock<PathBuf>,
}

impl ProcessFixture {
    fn new() -> Self {
        Self {
            root: TempDir::new("cli-process-fixture"),
            cli: OnceLock::new(),
            noop_build: OnceLock::new(),
        }
    }

    fn root(&self) -> &Path {
        self.root.path()
    }

    fn isolated_cli(&self) -> &Path {
        self.cli
            .get_or_init(|| {
                let source = PathBuf::from(env!("CARGO_BIN_EXE_ae-sdd"));
                let destination = self
                    .root()
                    .join(source.file_name().expect("CLI executable has a file name"));
                fs::copy(&source, &destination).expect("copy instrumented CLI");
                destination
            })
            .as_path()
    }

    fn noop_build(&self) -> &Path {
        self.noop_build
            .get_or_init(|| {
                let source = self.root().join("noop-build.rs");
                let executable = self
                    .root()
                    .join(format!("ae-sdd-build{}", std::env::consts::EXE_SUFFIX));
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
}

thread_local! {
    static PROCESS_FIXTURE: ProcessFixture = ProcessFixture::new();
}

fn fixture_root() -> PathBuf {
    PROCESS_FIXTURE.with(|fixture| fixture.root().to_path_buf())
}

fn isolated_cli() -> PathBuf {
    PROCESS_FIXTURE.with(|fixture| fixture.isolated_cli().to_path_buf())
}

fn noop_build() -> PathBuf {
    PROCESS_FIXTURE.with(|fixture| fixture.noop_build().to_path_buf())
}

fn missing_manifest() -> String {
    fixture_root()
        .join("missing-endpoint.json")
        .display()
        .to_string()
}

struct DaemonBuildTarget {
    target_dir: PathBuf,
    profile: Box<str>,
    executable: PathBuf,
}

fn daemon_build_target() -> DaemonBuildTarget {
    let cargo_cli = PathBuf::from(env!("CARGO_BIN_EXE_ae-sdd"));
    let profile_dir = cargo_cli
        .parent()
        .expect("Cargo CLI executable has a profile directory");
    let profile = profile_dir
        .file_name()
        .and_then(|name| name.to_str())
        .expect("Cargo CLI profile directory is UTF-8")
        .to_owned();
    assert!(
        matches!(profile.as_str(), "debug" | "release"),
        "unsupported Cargo profile directory for CLI process tests: {}",
        profile_dir.display()
    );
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                repository_root().join(path)
            }
        })
        .unwrap_or_else(|| {
            profile_dir
                .parent()
                .expect("Cargo CLI profile has a target directory")
                .to_path_buf()
        });
    let executable = target_dir
        .join(&profile)
        .join(format!("ae-sddd{}", std::env::consts::EXE_SUFFIX));
    DaemonBuildTarget {
        target_dir,
        profile: profile.into_boxed_str(),
        executable,
    }
}

fn daemon_executable() -> PathBuf {
    if let Some(explicit) = std::env::var_os("AE_SDDD_BIN") {
        let explicit = PathBuf::from(explicit);
        assert!(
            explicit.is_file(),
            "AE_SDDD_BIN does not identify an existing file: {}",
            explicit.display()
        );
        return explicit;
    }

    static BUILD: OnceLock<PathBuf> = OnceLock::new();
    BUILD
        .get_or_init(|| {
            let target = daemon_build_target();
            let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
            let mut arguments = vec![
                "build".to_owned(),
                "--locked".to_owned(),
                "-p".to_owned(),
                "ae-sdd-daemon".to_owned(),
                "--bin".to_owned(),
                "ae-sddd".to_owned(),
                "--target-dir".to_owned(),
                target.target_dir.display().to_string(),
            ];
            if target.profile.as_ref() == "release" {
                arguments.push("--release".to_owned());
            }
            let command_text = format!(
                "{} {}",
                PathBuf::from(&cargo).display(),
                arguments.join(" ")
            );
            let output = Command::new(&cargo)
                .args(&arguments)
                .current_dir(repository_root())
                .output()
                .unwrap_or_else(|error| {
                    panic!("failed to execute daemon build command `{command_text}`: {error}")
                });
            assert!(
                output.status.success(),
                "daemon build command `{command_text}` failed with exit code {:?}\nstderr:\n{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                target.executable.is_file(),
                "daemon build succeeded but executable is missing: {}",
                target.executable.display()
            );
            target.executable
        })
        .clone()
}

struct DaemonGuard {
    manifest: PathBuf,
    _authority_root: TempDir,
    _project_root: TempDir,
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
            let deadline = Instant::now() + Duration::from_secs(10);
            while self.manifest.exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

struct LiveProcessHarness {
    _guard: DaemonGuard,
    endpoint_manifest: PathBuf,
    project_root: PathBuf,
    workspace_id: String,
    work_item_id: String,
    session_id: String,
    capability_token: String,
}

fn copy_live_fixture(project_root: &Path, relative: &str) {
    let source = repository_root().join(relative);
    let destination = project_root.join(relative);
    fs::create_dir_all(destination.parent().expect("fixture parent"))
        .expect("create live fixture directory");
    fs::copy(&source, &destination)
        .unwrap_or_else(|error| panic!("copy live fixture {relative}: {error}"));
}

fn prepare_live_project(project_root: &Path) {
    for relative in [
        "constraints/README.md",
        "source/SKILL.md",
        "source/skills/phase1-design/requirement-analysis-skill.md",
        "source/skill-fallbacks/skills/phase1-design/requirement-analysis-skill.full.md",
    ] {
        copy_live_fixture(project_root, relative);
    }
    let catalog = project_root.join("source/standards/runtime/methodology-catalog.v1.json");
    fs::create_dir_all(catalog.parent().expect("catalog parent"))
        .expect("create methodology catalog directory");
    fs::write(
        catalog,
        serde_json::to_vec(&json!({
            "schemaVersion":"ae-sdd-methodology-catalog/v1",
            "catalogVersion":"1.0.0",
            "entries":[{
                "skillId":"phase1-design.requirement-analysis",
                "seriesKind":"requirement-analysis",
                "activity":"execute",
                "variant":"cli-process-v1",
                "version":"1.0.0",
                "activation":"workflow",
                "spawnPolicy":"physical_series",
                "compactRef":"skills/phase1-design/requirement-analysis-skill.md",
                "fallbackRef":"skill-fallbacks/skills/phase1-design/requirement-analysis-skill.full.md",
                "routePredicates":[],
                "requiredInputs":["requested-intent"],
                "deliverableKinds":["requirement-analysis"],
                "requiredGates":[],
                "toolDependencies":[]
            }]
        }))
        .expect("serialize live methodology catalog"),
    )
    .expect("write live methodology catalog");

    let project_key = project_root
        .file_name()
        .and_then(|name| name.to_str())
        .expect("isolated project root has an ASCII fixture name");
    let asset = project_root
        .join(".ae-sdd/assets")
        .join(format!("{project_key}.assets.md"));
    fs::create_dir_all(asset.parent().expect("asset parent"))
        .expect("create project asset directory");
    fs::write(
        asset,
        format!(
            "# {project_key} Project Assets\n\
             ## §A Outline\nfixture\n\
             ## §B Modules\nfixture\n\
             ## §C Fields\nfixture\n\
             ## §D Components\nfixture\n\
             ## §E API\nfixture\n\
             ## §F Keywords\nfixture\n\
             ## §G Read API\nfixture\n"
        ),
    )
    .expect("write canonical project asset fixture");

    fs::write(
        project_root.join(".ae-sdd/config.yaml"),
        format!(
            "version: 1\nprojectKey: {project_key}\nautomation:\n  enabled: false\n  reviewerTier: 3\n  preflightInfoCollection: true\n  onConsensusStall: pause\n"
        ),
    )
    .expect("write automation config fixture");
    let database_profiles = project_root.join(".ae-sdd/secrets/db-connections.local.json");
    fs::create_dir_all(
        database_profiles
            .parent()
            .expect("database profiles parent"),
    )
    .expect("create database profiles directory");
    fs::write(database_profiles, br#"{"profiles":[]}"#)
        .expect("write empty database profiles fixture");

    fs::write(
        project_root.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\n",
    )
    .expect("write isolated Git fixture");
    for arguments in [
        &["init", "--quiet"][..],
        &["config", "core.autocrlf", "false"][..],
        &["add", "Cargo.toml"][..],
        &[
            "-c",
            "user.name=ae-sdd-test",
            "-c",
            "user.email=ae-sdd@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ][..],
    ] {
        let status = Command::new("git")
            .args(arguments)
            .current_dir(project_root)
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
            .status()
            .expect("run isolated Git fixture command");
        assert!(status.success(), "isolated Git fixture command failed");
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
    let fixture = fixture_root();
    let registry = fixture.join("distributors.json").display().to_string();
    let project = fixture.join("new-project").display().to_string();
    let plugins = fixture.join("plugins").display().to_string();
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
            fixture.join("codex").display().to_string(),
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

    let request = fixture_root().join("version-request.json");
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
    let operation_request = fixture_root().join("operation-request.json");
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

    let state_dir = fixture_root().join("runtime-state");
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

fn start_live_process_harness(daemon: &Path, nonce: u128) -> LiveProcessHarness {
    let authority_root = TempDir::new("cli-process-authority");
    let project_temp = TempDir::new("cli-process-project");
    prepare_live_project(project_temp.path());
    let project_root = project_temp.path().to_path_buf();
    let state_dir = authority_root.path().join("runtime");
    fs::create_dir_all(&state_dir).expect("live daemon state dir");
    let endpoint_manifest = state_dir.join("endpoint.v1.json");
    let root = project_root.display().to_string();
    let project_key = project_root
        .file_name()
        .and_then(|name| name.to_str())
        .expect("isolated project root has an ASCII fixture name");
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
    let guard = DaemonGuard {
        manifest: endpoint_manifest.clone(),
        _authority_root: authority_root,
        _project_root: project_temp,
    };

    let register_params = json!({
        "protocolVersion":"1.0",
        "idempotencyKey":"cli-process-workspace-register",
        "deadlineMs":5000,
        "payload":{"projectRoot":root,"projectKey":project_key}
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
    let bootstrap_event = json!({
        "hook_event_name":"UserPromptSubmit",
        "prompt":"/ae-sdd",
        "event_id":format!("cli-process-route-bootstrap-{nonce}"),
        "session_id":format!("cli-process-contract-{nonce}"),
        "cwd":root
    });
    let bootstrapped = run_cli(
        &[
            "hook".into(),
            "--method".into(),
            "hook.user_prompt".into(),
            "--request-json".into(),
            bootstrap_event.to_string(),
            "--manifest".into(),
            endpoint_manifest.display().to_string(),
            "--timeout-ms".into(),
            "5000".into(),
        ],
        None,
    );
    assert!(
        bootstrapped.status.success(),
        "bootstrap ROUTE work item: {}",
        text(&bootstrapped.stderr)
    );
    let bootstrap_stderr = text(&bootstrapped.stderr);
    let work_item_id = bootstrap_stderr
        .lines()
        .find_map(|line| line.strip_prefix("ae-sdd: hook.user_prompt bound workItemId: "))
        .unwrap_or_else(|| {
            panic!(
                "/ae-sdd reports its daemon-minted workItemId; stdout={} stderr={bootstrap_stderr}",
                text(&bootstrapped.stdout)
            )
        })
        .to_owned();
    let open_session_params = json!({
        "protocolVersion":"1.0",
        "workspaceId":workspace_id,
        "agentId":"host-hook",
        "idempotencyKey":"cli-process-session-reopen",
        "deadlineMs":5000,
        "payload":{
            "externalKey":format!("cli-process-contract-{nonce}"),
            "role":"root",
            "engaged":true
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
        "reopen isolated root session: {}",
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

    LiveProcessHarness {
        _guard: guard,
        endpoint_manifest,
        project_root,
        workspace_id,
        work_item_id,
        session_id,
        capability_token,
    }
}

fn authority_contains(root: &Path, work_item_id: &str) -> bool {
    let suffix = format!("-{work_item_id}");
    fs::read_dir(root.join(".auto-engineering"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_dir())
                && entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.ends_with(&suffix))
        })
}

#[test]
fn successful_legacy_rpc_and_job_routes_flush_production_coverage() {
    let daemon = daemon_executable();
    if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {
        let profile_dir = PathBuf::from(env!("CARGO_BIN_EXE_ae-sdd"))
            .parent()
            .expect("Cargo CLI executable has a profile directory")
            .file_name()
            .expect("Cargo CLI profile directory has a name")
            .to_owned();
        let target_dir = PathBuf::from(target_dir);
        let target_dir = if target_dir.is_absolute() {
            target_dir
        } else {
            repository_root().join(target_dir)
        };
        let expected = target_dir
            .join(profile_dir)
            .join(format!("ae-sddd{}", std::env::consts::EXE_SUFFIX));
        assert_eq!(daemon, expected);
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after Unix epoch")
        .as_nanos();
    let LiveProcessHarness {
        _guard,
        endpoint_manifest,
        workspace_id,
        work_item_id,
        session_id,
        capability_token,
        ..
    } = start_live_process_harness(&daemon, nonce);

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
            work_item_id.clone(),
            "--session-id".into(),
            session_id.clone(),
            "--agent-id".into(),
            "host-hook".into(),
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

#[test]
fn concurrent_live_processes_keep_project_authority_isolated() {
    let daemon = daemon_executable();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after Unix epoch")
        .as_nanos();
    let (left, right) = std::thread::scope(|scope| {
        let left = scope.spawn(|| start_live_process_harness(&daemon, nonce));
        let right = scope.spawn(|| start_live_process_harness(&daemon, nonce + 1));
        (
            left.join().expect("left live process harness"),
            right.join().expect("right live process harness"),
        )
    });

    assert_ne!(left.endpoint_manifest, right.endpoint_manifest);
    assert_ne!(left.project_root, right.project_root);
    assert_ne!(left.workspace_id, right.workspace_id);
    assert_ne!(left.work_item_id, right.work_item_id);
    assert!(authority_contains(&left.project_root, &left.work_item_id));
    assert!(authority_contains(&right.project_root, &right.work_item_id));
    assert!(!authority_contains(&left.project_root, &right.work_item_id));
    assert!(!authority_contains(&right.project_root, &left.work_item_id));
    assert_ne!(left.project_root, repository_root());
    assert_ne!(right.project_root, repository_root());
}
