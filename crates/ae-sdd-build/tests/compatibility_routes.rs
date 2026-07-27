use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use ae_sdd_build::{
    AdminChange, BenchmarkError, CapabilitySurface, CompatibilityManifest,
    CompatibilityRoutingManifest, ExecutionMode, ExpectedCounts, HookBenchmarkConfig,
    ImplementationStatus, InitInput, JobInput, ManifestError, NativeJobRequest, PermissionClass,
    RouteIdentity, RouteTarget, audit_compatibility, benchmark_hook,
};
use ae_sdd_protocol::{CapabilityTokenWire, RequestParams, RpcMethod};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RpcMethodsFixture {
    schema_version: String,
    methods: Vec<String>,
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn fixture_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ae-sdd-build-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&root).expect("fixture root");
    root
}

fn write_json(path: &std::path::Path, value: &impl serde::Serialize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("JSON parent");
    }
    fs::write(path, serde_json::to_vec_pretty(value).expect("encode JSON")).expect("write JSON");
}

fn staged_compatibility_fixture(
    label: &str,
) -> (
    PathBuf,
    PathBuf,
    CompatibilityManifest,
    CompatibilityRoutingManifest,
) {
    let root = fixture_root(label);
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("workspace marker");
    let source = repository_root().join("tests/fixtures/compatibility");
    let manifest: CompatibilityManifest = serde_json::from_slice(
        &fs::read(source.join("legacy-surface.v1.json")).expect("source inventory"),
    )
    .expect("inventory JSON");
    let routing: CompatibilityRoutingManifest = serde_json::from_slice(
        &fs::read(source.join("cli-routing.v1.json")).expect("source routing"),
    )
    .expect("routing JSON");

    for relative in routing
        .commands
        .iter()
        .flat_map(|route| [route.fixture.as_str(), route.evidence.as_str()])
        .chain(
            routing
                .capabilities
                .iter()
                .flat_map(|entry| [entry.fixture.as_str(), entry.evidence.as_str()]),
        )
    {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("evidence parent")).expect("evidence directory");
        if !path.exists() {
            fs::write(path, b"fixture\n").expect("evidence file");
        }
    }

    let manifest_path = root.join("tests/fixtures/compatibility/legacy-surface.v1.json");
    write_json(&manifest_path, &manifest);
    write_json(
        &manifest_path
            .parent()
            .expect("manifest parent")
            .join("cli-routing.v1.json"),
        &routing,
    );
    (root, manifest_path, manifest, routing)
}

fn write_compatibility_fixture(
    manifest_path: &std::path::Path,
    manifest: &CompatibilityManifest,
    routing: &CompatibilityRoutingManifest,
) {
    write_json(manifest_path, manifest);
    write_json(
        &manifest_path
            .parent()
            .expect("manifest parent")
            .join("cli-routing.v1.json"),
        routing,
    );
}

#[test]
fn compatibility_audit_accepts_only_fully_evidenced_legacy_routes() {
    let root = repository_root();
    let inventory = root.join("tests/fixtures/compatibility/legacy-surface.v1.json");
    let summary = audit_compatibility(
        &inventory,
        ExpectedCounts::legacy(),
        &[PathBuf::from("apps/ae-sdd-monitor/**")],
    )
    .expect("every preserved or breaking-fix route must carry executable evidence");
    let expected = ExpectedCounts::legacy();
    assert_eq!(summary.command_count, expected.commands);
    // Commands are evidenced by the routing manifest; the capability evidence
    // set covers the three registry-backed surfaces.
    assert_eq!(
        summary.capability_evidence_count,
        expected.operations + expected.gates + expected.scanners
    );
    assert_eq!(summary.stub_count, 0);
    assert_eq!(summary.logical_fallback_count, 0);
}

#[test]
fn every_route_declares_one_typed_target_without_overclaiming_implementation() {
    const IMPLEMENTED_RPC: [&str; 6] = [
        "gate coding-required",
        "gate ra-required",
        "health",
        "ops describe",
        "ops execute",
        "ops next",
    ];
    const BREAKING_RPC: [&str; 7] = [
        "flow-violation-scan",
        "ra-authenticity-scan",
        "ra-depth-scan",
        "ra-implementation-scan",
        "review abort",
        "review collect",
        "review-loop collect",
    ];
    const IMPLEMENTED_TYPED: [&str; 12] = [
        "doc resolve",
        "doc save",
        "evidence finalize",
        "evidence record",
        "gates check",
        "lease acquire",
        "lease release",
        "lease renew",
        "lease status",
        "state next-step",
        "state read",
        "verify plan",
    ];
    const IMPLEMENTED_JOBS: [&str; 21] = [
        "assets check",
        "assets outline",
        "assets query",
        "assets read",
        "assets section",
        "assets stats",
        "automation status",
        "baseline diff",
        "baseline inspect",
        "classify",
        "evidence lookup",
        "git blame",
        "git diff",
        "git impact",
        "git log",
        "git status",
        "gate doc-storage",
        "iteration-check",
        "perf doctor",
        "perf report",
        "update-check",
    ];
    const BREAKING_JOBS: [&str; 15] = [
        "db audit",
        "db explain",
        "db profiles",
        "db query",
        "memory clean",
        "memory clean-all",
        "memory common",
        "memory create",
        "memory read",
        "memory search",
        "memory summarize",
        "memory update",
        "plugin list",
        "plugin trace",
        "plugin validate",
    ];
    const BREAKING_TYPED: [&str; 1] = ["lease break"];
    let path = repository_root().join("tests/fixtures/compatibility/cli-routing.v1.json");
    let routing: CompatibilityRoutingManifest =
        serde_json::from_slice(&std::fs::read(path).expect("routing fixture"))
            .expect("strict routing schema");
    let inventory = ae_sdd_build::CompatibilityManifest::from_path(
        &repository_root().join("tests/fixtures/compatibility/legacy-surface.v1.json"),
    )
    .expect("compatibility inventory");

    let mut command_ids = BTreeSet::new();
    let mut dispatches = Vec::new();
    for route in routing.commands {
        assert!(command_ids.insert(route.id.clone()), "duplicate command");
        assert!(route.fail_closed, "{} must fail closed", route.id);
        let dotted = route.id.replace(' ', ".");
        let expected_status = if ae_sdd_build::B_OFFLINE_ENTRYPOINTS.contains(&dotted.as_str())
            || IMPLEMENTED_RPC.contains(&route.id.as_str())
            || IMPLEMENTED_TYPED.contains(&route.id.as_str())
            || IMPLEMENTED_JOBS.contains(&route.id.as_str())
        {
            ImplementationStatus::Implemented
        } else if BREAKING_RPC.contains(&route.id.as_str())
            || BREAKING_JOBS.contains(&route.id.as_str())
            || BREAKING_TYPED.contains(&route.id.as_str())
            || matches!(&route.route, RouteTarget::Rejected { .. })
        {
            ImplementationStatus::BreakingFixVerified
        } else {
            ImplementationStatus::Pending
        };
        assert_eq!(route.status, expected_status, "{} status", route.id);
        let disposition = inventory
            .commands
            .iter()
            .find(|entry| entry.id == route.id)
            .expect("inventory route")
            .disposition;
        if route.status == ImplementationStatus::BreakingFixVerified {
            assert_eq!(disposition, ae_sdd_build::Disposition::BreakingFix);
        } else {
            assert_ne!(disposition, ae_sdd_build::Disposition::BreakingFix);
        }
        let dispatch = match route.route {
            RouteTarget::Rpc { method } => format!("rpc:{method}"),
            RouteTarget::TypedOperation { operation } => format!("operation:{operation}"),
            RouteTarget::NativeBuildJob { job, entrypoint } => {
                format!("native:{}:{entrypoint}", job.as_str())
            }
            RouteTarget::Rejected {
                stable_code,
                remediation,
            } => format!("rejected:{stable_code}:{remediation}"),
        };
        assert!(!dispatch.ends_with(':'), "{} has an empty target", route.id);
        dispatches.push((route.id, dispatch));
    }

    assert_eq!(command_ids.len(), 113);
    assert_eq!(dispatches.len(), 113);
}

#[test]
fn build_tool_contains_no_subprocess_fallback_executor() {
    let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut pending = vec![source_root];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).expect("source directory") {
            let entry = entry.expect("source entry");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().and_then(|value| value.to_str()) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("Rust source");
            let normalized = path.to_string_lossy().replace('\\', "/");
            for line in source.lines().filter(|line| line.contains("Command::new(")) {
                let daemon_benchmark = normalized.ends_with("/src/benchmark.rs")
                    && line.contains("Command::new(&daemon_binary)");
                let windows_acl = normalized.ends_with("/src/service/materialize.rs")
                    && line.contains("Command::new(\"icacls.exe\")");
                let native_service_manager = normalized.ends_with("/src/service/executor.rs")
                    && line.contains("Command::new(command.program)");
                assert!(
                    daemon_benchmark || windows_acl || native_service_manager,
                    "unapproved subprocess executor found in {}: {}",
                    path.display(),
                    line.trim()
                );
            }
            assert!(
                !source.contains("Command::new(\"python")
                    && !source.contains("Command::new(\"python3")
                    && !source.contains("legacy_fallback")
                    && !source.contains("legacyFallback"),
                "Python or legacy logical fallback found in {}",
                path.display()
            );
        }
    }
}

#[test]
fn shared_capability_fixture_uses_protocol_canonical_claims_and_negative_cases() {
    let path = repository_root().join("tests/fixtures/protocol/capability-token.v1.json");
    let fixture: Value = serde_json::from_slice(&std::fs::read(path).expect("capability fixture"))
        .expect("fixture JSON");
    let token: CapabilityTokenWire =
        serde_json::from_value(fixture["token"].clone()).expect("strict token wire");
    let canonical = token.canonical_claims_bytes().expect("canonical claims");
    assert_eq!(
        canonical,
        fixture["canonicalClaimsUtf8"]
            .as_str()
            .expect("canonical UTF-8")
            .as_bytes()
    );

    let public_key_bytes: [u8; 32] = hex::decode(
        fixture["publicKey"]["ed25519PublicKeyHex"]
            .as_str()
            .expect("public key"),
    )
    .expect("public key hex")
    .try_into()
    .expect("32-byte public key");
    let public_key = VerifyingKey::from_bytes(&public_key_bytes).expect("Ed25519 public key");
    let signature_bytes: [u8; 64] = hex::decode(token.signature())
        .expect("signature hex")
        .try_into()
        .expect("64-byte signature");
    public_key
        .verify(&canonical, &Signature::from_bytes(&signature_bytes))
        .expect("valid fixture signature");

    let cases = fixture["requestParamsCases"]
        .as_array()
        .expect("request cases");
    assert_eq!(cases.len(), 3);
    for case in cases {
        let params: RequestParams<Value> =
            serde_json::from_value(case["params"].clone()).expect("strict RequestParams");
        let encoded = params.capability_token.expect("capabilityToken");
        let candidate = CapabilityTokenWire::decode_json(&encoded).expect("token wire");
        let expected = case["expected"].as_str().expect("expected outcome");
        let key_matches =
            candidate.key_id() == fixture["publicKey"]["keyId"].as_str().expect("key id");
        let signature: [u8; 64] = hex::decode(candidate.signature())
            .expect("signature hex")
            .try_into()
            .expect("signature length");
        let signature_valid = public_key
            .verify(
                &candidate.canonical_claims_bytes().expect("claims"),
                &Signature::from_bytes(&signature),
            )
            .is_ok();
        assert_eq!(expected == "accepted", key_matches && signature_valid);
    }
}

#[test]
fn protocol_method_fixture_matches_the_exact_37_method_registry() {
    let path = repository_root().join("tests/fixtures/protocol/rpc-methods.v1.json");
    let fixture: RpcMethodsFixture =
        serde_json::from_slice(&std::fs::read(path).expect("method fixture"))
            .expect("strict method fixture");
    assert_eq!(fixture.schema_version, "ae-sdd-rpc-methods/v1");
    assert_eq!(fixture.methods.len(), ae_sdd_protocol::METHOD_COUNT);
    assert_eq!(
        fixture.methods,
        ae_sdd_protocol::RpcMethod::ALL
            .into_iter()
            .map(|method| method.as_str().to_owned())
            .collect::<Vec<_>>()
    );
}

#[test]
fn compatibility_classification_partitions_the_113_commands_exactly() {
    let path = repository_root().join("tests/fixtures/compatibility/cli-routing.v1.json");
    let routing: CompatibilityRoutingManifest =
        serde_json::from_slice(&std::fs::read(path).expect("routing fixture"))
            .expect("strict routing fixture");
    let b = routing
        .commands
        .iter()
        .filter(|route| {
            let dotted = route.id.replace(' ', ".");
            ae_sdd_build::B_OFFLINE_ENTRYPOINTS.contains(&dotted.as_str())
        })
        .count();
    let c = routing
        .commands
        .iter()
        .filter(|route| ae_sdd_build::C_ADMIN_JOB_COMMANDS.contains(&route.id.as_str()))
        .count();
    let d = routing
        .commands
        .iter()
        .filter(|route| ae_sdd_build::D_REJECTED_COMMANDS.contains(&route.id.as_str()))
        .count();
    assert_eq!(
        (b, c, d, routing.commands.len() - b - c - d),
        (13, 24, 38, 38)
    );
}

#[test]
fn post_commit_and_harness_docs_use_rust_typed_argv_only() {
    let root = repository_root();
    let hook =
        std::fs::read_to_string(root.join(".githooks/post-commit")).expect("post-commit hook");
    assert!(hook.contains("run_build_tool harness"));
    assert!(hook.contains("run_build_tool post-commit"));
    assert!(hook.contains("--repository-root"));
    assert!(hook.contains("--allowed-root"));
    assert!(!hook.contains("native-job --request"));
    assert!(!hook.contains("cat >"));
    assert!(!hook.to_ascii_lowercase().contains("python"));
    assert!(!hook.contains("l2_inject"));

    // Hosts and their instruction files are declared once, in the distributor
    // registry. The hook previously carried its own hardcoded list alongside it
    // and the two drifted in both directions: a registered host silently went
    // stale, and a listed-but-unregistered host had its directory invented.
    assert!(
        hook.contains("--distributor-registry"),
        "the released hook must resolve hosts from the distributor registry"
    );
    assert!(
        hook.contains("--registry-home"),
        "the hook must pass the home used to expand registry paths, never let it be guessed"
    );
    for flag in [
        "--codex-instructions",
        "--claude-instructions",
        "--zcode-instructions",
        "--harness-instructions",
        "--hermes-instructions",
    ] {
        assert!(
            !hook.contains(flag),
            "{flag} reintroduces a second host list beside the registry"
        );
    }
    assert!(
        !hook.contains("$USER_HOME/.codex/skills")
            && !hook.contains("$USER_HOME/.claude/skills")
            && !hook.contains("$USER_HOME/.zcode/skills"),
        "package targets must come from the registry, not from paths spelled out here"
    );
    assert!(
        hook.contains(".ae-sdd/distributors.json"),
        "the registry path must be explicit so an absent registry is a visible failure"
    );

    let readme = std::fs::read_to_string(root.join(".harness/README.md")).expect("harness README");
    assert!(readme.contains("ae-sdd-build --release -- harness"));
    assert!(!readme.contains("build_harness.py"));
    assert!(!readme.to_ascii_lowercase().contains("python"));
}

#[test]
fn l2_discipline_ssot_carries_bilingual_execution_efficiency() {
    let source = std::fs::read_to_string(repository_root().join("source/L2-DISCIPLINE.md"))
        .expect("L2 discipline SSOT");
    assert!(
        source.contains("ae-sdd-build post-commit"),
        "the SSOT header must name the released Rust injection authority"
    );
    assert!(
        source.contains("`scripts/l2_inject.py` is migration/manual legacy tooling"),
        "the SSOT header must demote the Python injector to legacy/oracle tooling"
    );

    let english = language_section(&source, "en");
    let chinese = language_section(&source, "zh");
    for (language, body, heading, subsections) in [
        (
            "en",
            english,
            "## Execution Efficiency and Scope Discipline",
            [
                "### Fast resume",
                "### Shortest verified slice",
                "### Bounded investigation and output",
                "### Agent coordination",
                "### Progress control",
            ],
        ),
        (
            "zh",
            chinese,
            "## 执行效率与范围纪律",
            [
                "### 快速续接",
                "### 最短可验证切片",
                "### 有界调查与输出",
                "### Agent 协同",
                "### 进度控制",
            ],
        ),
    ] {
        assert!(
            body.contains(heading),
            "SECTION:{language} must carry the execution efficiency discipline"
        );
        for subsection in subsections {
            assert!(
                body.contains(subsection),
                "SECTION:{language} must carry the detailed subsection {subsection}"
            );
        }
        assert_eq!(
            source.matches(heading).count(),
            1,
            "the {language} efficiency heading must exist exactly once in the SSOT"
        );
    }
}

fn language_section<'a>(source: &'a str, language: &str) -> &'a str {
    let open = format!("<!-- SECTION:{language} -->");
    let close = format!("<!-- /SECTION:{language} -->");
    let start = source
        .find(&open)
        .map(|index| index + open.len())
        .expect("language section start");
    let end = source[start..]
        .find(&close)
        .map(|index| start + index)
        .expect("language section end");
    &source[start..end]
}

#[test]
fn member_manifests_inherit_every_dependency_from_the_workspace() {
    let root = repository_root();
    for parent in ["bins", "crates"] {
        for entry in std::fs::read_dir(root.join(parent)).expect("workspace member directory") {
            let path = entry.expect("workspace member").path().join("Cargo.toml");
            if !path.is_file() {
                continue;
            }
            let manifest = std::fs::read_to_string(&path).expect("member manifest");
            let mut dependency_section = false;
            for line in manifest.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('[') {
                    dependency_section = trimmed.ends_with("dependencies]");
                    continue;
                }
                if dependency_section && !trimmed.is_empty() && !trimmed.starts_with('#') {
                    assert!(
                        trimmed.contains(".workspace = true")
                            || trimmed.contains("workspace = true"),
                        "member dependency must inherit [workspace.dependencies] in {}: {}",
                        path.display(),
                        trimmed
                    );
                }
            }
        }
    }
}

#[test]
fn build_cli_native_job_and_harness_modes_are_content_idempotent() {
    let root = fixture_root("native-cli");
    let project = root.join("project");
    fs::create_dir(&project).expect("project");
    let request_path = root.join("native-job.json");
    let native = NativeJobRequest {
        schema_version: "ae-sdd-native-job/v1".to_owned(),
        entrypoint: "init".to_owned(),
        actor: "compatibility-test".to_owned(),
        reason: "exercise typed CLI output modes".to_owned(),
        idempotency_key: "native-cli-apply".to_owned(),
        mode: ExecutionMode::Apply,
        allowed_roots: vec![root.clone()],
        job: JobInput::Init(InitInput {
            project_root: project.clone(),
            changes: vec![AdminChange {
                relative_path: PathBuf::from("generated.txt"),
                contents: "generated\n".to_owned(),
                permission: PermissionClass::PrivateFile,
            }],
        }),
    };
    write_json(&request_path, &native);

    let applied = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"))
        .args(["native-job", "--request"])
        .arg(&request_path)
        .output()
        .expect("native apply CLI");
    assert!(applied.status.success());
    assert!(String::from_utf8_lossy(&applied.stdout).contains("applied"));
    assert!(project.join("generated.txt").is_file());

    let replayed = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"))
        .args(["native-job", "--request"])
        .arg(&request_path)
        .output()
        .expect("native replay CLI");
    assert!(replayed.status.success());
    assert!(String::from_utf8_lossy(&replayed.stdout).contains("replayed"));

    let json = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"))
        .args(["native-job", "--request"])
        .arg(&request_path)
        .arg("--json")
        .output()
        .expect("native JSON CLI");
    assert!(json.status.success());
    let execution: serde_json::Value =
        serde_json::from_slice(&json.stdout).expect("native execution JSON");
    assert_eq!(execution["replayed"], true);

    let mut dry_run = native;
    dry_run.mode = ExecutionMode::DryRun;
    dry_run.idempotency_key = "native-cli-dry-run".to_owned();
    dry_run.job = JobInput::Init(InitInput {
        project_root: project,
        changes: vec![AdminChange {
            relative_path: PathBuf::from("planned.txt"),
            contents: "planned\n".to_owned(),
            permission: PermissionClass::PrivateFile,
        }],
    });
    write_json(&request_path, &dry_run);
    let planned = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"))
        .args(["native-job", "--request"])
        .arg(&request_path)
        .output()
        .expect("native dry-run CLI");
    assert!(planned.status.success());
    assert!(String::from_utf8_lossy(&planned.stdout).contains("planned"));

    let source = root.join("source.md");
    let target = root.join("harness/agent.md");
    fs::write(&source, "# Source\n").expect("harness source");
    let harness = |dry_run: bool, json: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"));
        command
            .arg("harness")
            .arg("--source")
            .arg(&source)
            .arg("--target")
            .arg(&target)
            .args(["--title", "Native Harness"])
            .arg("--allowed-root")
            .arg(&root);
        if dry_run {
            command.arg("--dry-run");
        }
        if json {
            command.arg("--json");
        }
        command.output().expect("harness CLI")
    };
    let planned = harness(true, false);
    assert!(planned.status.success());
    assert!(String::from_utf8_lossy(&planned.stdout).contains("planned"));
    let applied = harness(false, false);
    assert!(applied.status.success());
    assert!(String::from_utf8_lossy(&applied.stdout).contains("applied"));
    let replayed = harness(false, false);
    assert!(replayed.status.success());
    assert!(String::from_utf8_lossy(&replayed.stdout).contains("replayed"));
    let json = harness(false, true);
    assert!(json.status.success());
    let execution: serde_json::Value = serde_json::from_slice(&json.stdout).expect("harness JSON");
    assert_eq!(execution["replayed"], true);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn build_cli_post_commit_compatibility_release_and_benchmark_paths_are_safe()
-> Result<(), BenchmarkError> {
    let root = fixture_root("cli-matrix");
    let repository = root.join("repo");
    let source = repository.join("source");
    let home = root.join("home");
    fs::create_dir_all(&source).expect("source");
    fs::create_dir(&home).expect("home");
    fs::write(source.join("SKILL.md"), "---\nname: fixture\n---\n").expect("skill");
    let package = repository.join("dist/ae-sdd");
    let target = home.join(".codex/skills/ae-sdd");
    let run_post_commit = |json: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"));
        command
            .arg("post-commit")
            .arg("--repository-root")
            .arg(&repository)
            .arg("--source")
            .arg(&source)
            .arg("--package")
            .arg(&package)
            .arg("--target")
            .arg(&target)
            .arg("--allowed-root")
            .arg(&repository)
            .arg("--allowed-root")
            .arg(&home)
            .args(["--commit", "0123456789abcdef0123456789abcdef01234567"]);
        if json {
            command.arg("--json");
        }
        command.output().expect("post-commit CLI")
    };
    let post_commit = run_post_commit(false);
    assert!(post_commit.status.success());
    assert!(String::from_utf8_lossy(&post_commit.stdout).contains("post-commit complete"));
    let post_commit = run_post_commit(true);
    assert!(post_commit.status.success());
    let post_commit: serde_json::Value =
        serde_json::from_slice(&post_commit.stdout).expect("post-commit JSON");
    assert_eq!(post_commit["compile"]["replayed"], true);

    let inventory = repository_root().join("tests/fixtures/compatibility/legacy-surface.v1.json");
    for json in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"));
        command
            .arg("compatibility-audit")
            .arg("--manifest")
            .arg(&inventory)
            .arg("--exclude")
            .arg("apps/ae-sdd-monitor/**");
        if json {
            command.arg("--json");
        }
        let output = command.output().expect("compatibility CLI");
        assert!(output.status.success());
        if json {
            let summary: serde_json::Value =
                serde_json::from_slice(&output.stdout).expect("audit JSON");
            assert_eq!(summary["commandCount"], 113);
        } else {
            assert!(String::from_utf8_lossy(&output.stdout).contains("commands=113"));
        }
    }

    let artifacts = root.join("release");
    fs::create_dir(&artifacts).expect("release artifacts");
    for binary in ["ae-sdd", "ae-sddd", "ae-sdd-build"] {
        fs::write(artifacts.join(binary), b"native-rust-binary").expect("release binary");
    }
    for json in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"));
        command
            .arg("verify-release")
            .arg("--artifact-dir")
            .arg(&artifacts);
        if json {
            command.arg("--json");
        }
        let output = command.output().expect("release verification CLI");
        assert!(output.status.success());
        if json {
            let verification: serde_json::Value =
                serde_json::from_slice(&output.stdout).expect("release JSON");
            assert_eq!(verification["artifacts"].as_array().map(Vec::len), Some(3));
        } else {
            assert!(String::from_utf8_lossy(&output.stdout).contains("binaries=3"));
        }
    }

    for args in [
        vec!["benchmark-hook", "--samples", "0"],
        vec!["benchmark-hook", "--samples", "1", "--histogram", "linear"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"))
            .args(args)
            .output()
            .expect("rejected benchmark CLI");
        assert!(!output.status.success());
    }
    let missing_manifest = root.join("missing-endpoint.json");
    let missing_daemon = root.join("missing-daemon.exe");
    let debug_only = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"))
        .args(["benchmark-hook", "--warmup", "0", "--samples", "1"])
        .arg("--manifest")
        .arg(&missing_manifest)
        .arg("--workspace-root")
        .arg(&root)
        .arg("--daemon-binary")
        .arg(&missing_daemon)
        .arg("--json")
        .output()
        .expect("debug benchmark CLI");
    assert!(!debug_only.status.success());
    assert!(String::from_utf8_lossy(&debug_only.stderr).contains("release build"));

    let config = HookBenchmarkConfig::new(0, 1, "hdr")?
        .with_manifest(missing_manifest)
        .with_workspace_root(root.clone())
        .with_daemon_binary(missing_daemon);
    let benchmark = benchmark_hook(config);
    if cfg!(debug_assertions) {
        assert!(matches!(
            benchmark,
            Err(BenchmarkError::ReleaseProfileRequired)
        ));
    } else {
        assert!(benchmark.is_err());
    }
    fs::remove_dir_all(root).expect("cleanup");
    Ok::<(), BenchmarkError>(())
}

#[test]
fn compatibility_manifest_inventory_failures_are_precise() {
    let path = repository_root().join("tests/fixtures/compatibility/legacy-surface.v1.json");
    let manifest = CompatibilityManifest::from_path(&path).expect("manifest");

    let mut wrong_schema = manifest.clone();
    wrong_schema.schema_version = "wrong".to_owned();
    assert!(matches!(
        wrong_schema.audit(ExpectedCounts::legacy()),
        Err(ManifestError::SchemaVersion(_))
    ));

    let mut unsafe_routing = manifest.clone();
    unsafe_routing.routing_manifest = "../routing.json".to_owned();
    assert!(matches!(
        unsafe_routing.audit(ExpectedCounts::legacy()),
        Err(ManifestError::EvidencePath(_))
    ));

    let mut wrong_count = manifest.clone();
    wrong_count.commands.pop();
    assert!(matches!(
        wrong_count.audit(ExpectedCounts::legacy()),
        Err(ManifestError::Count {
            surface: "commands",
            ..
        })
    ));

    for field in ["id", "source", "owner"] {
        let mut empty = manifest.clone();
        match field {
            "id" => empty.commands[0].id.clear(),
            "source" => empty.commands[0].source.clear(),
            _ => empty.commands[0].owner.clear(),
        }
        assert!(matches!(
            empty.audit(ExpectedCounts::legacy()),
            Err(ManifestError::EmptyField { field: actual, .. }) if actual == field
        ));
    }

    let mut duplicate = manifest;
    duplicate.commands[1].id = duplicate.commands[0].id.clone();
    assert!(matches!(
        duplicate.audit(ExpectedCounts::legacy()),
        Err(ManifestError::DuplicateId {
            surface: "commands",
            ..
        })
    ));
}

#[test]
fn compatibility_audit_fail_closed_matrix_reaches_route_and_capability_guards() {
    let (root, manifest_path, manifest, routing) = staged_compatibility_fixture("audit-guards");
    audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[])
        .expect("staged compatibility fixture");

    let mut wrong_registry = manifest.clone();
    wrong_registry.operations[0].id = "unknown.operation".to_owned();
    write_compatibility_fixture(&manifest_path, &wrong_registry, &routing);
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::RegistryMismatch {
            surface: "operations",
            ..
        })
    ));

    let mut wrong_schema = routing.clone();
    wrong_schema.schema_version = "wrong".to_owned();
    write_compatibility_fixture(&manifest_path, &manifest, &wrong_schema);
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::SchemaVersion(_))
    ));

    let mut duplicate_route = routing.clone();
    duplicate_route.commands[1].id = duplicate_route.commands[0].id.clone();
    write_compatibility_fixture(&manifest_path, &manifest, &duplicate_route);
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::DuplicateId {
            surface: "command routes",
            ..
        })
    ));

    let mut open_route = routing.clone();
    open_route.commands[0].fail_closed = false;
    write_compatibility_fixture(&manifest_path, &manifest, &open_route);
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::NotFailClosed(_))
    ));

    let mut unbounded_route = routing.clone();
    unbounded_route.commands[0].deadline_ms = 0;
    write_compatibility_fixture(&manifest_path, &manifest, &unbounded_route);
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::Deadline { .. })
    ));

    let mut uncovered_route = routing.clone();
    uncovered_route.commands.pop();
    write_compatibility_fixture(&manifest_path, &manifest, &uncovered_route);
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::RouteCoverage { .. })
    ));

    let mut pending_route = routing.clone();
    pending_route
        .commands
        .iter_mut()
        .find(|route| route.id == "health")
        .expect("health route")
        .status = ImplementationStatus::Pending;
    write_compatibility_fixture(&manifest_path, &manifest, &pending_route);
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::UnimplementedRoutes(_))
    ));

    for id in ["version", "assets check", "scripts-dir", "health"] {
        let mut invalid = routing.clone();
        let route = invalid
            .commands
            .iter_mut()
            .find(|route| route.id == id)
            .expect("classified route");
        route.route = if id == "health" {
            RouteTarget::Rejected {
                stable_code: "LEGACY_COMMAND_REMOVED".to_owned(),
                remediation: "use typed daemon health".to_owned(),
            }
        } else {
            RouteTarget::Rpc {
                method: RpcMethod::RuntimeStatus,
            }
        };
        write_compatibility_fixture(&manifest_path, &manifest, &invalid);
        assert!(matches!(
            audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
            Err(ManifestError::RouteTarget { .. })
        ));
    }

    let mut wrong_identity = routing.clone();
    wrong_identity
        .commands
        .iter_mut()
        .find(|route| route.id == "gate coding-required")
        .expect("gate route")
        .identity = RouteIdentity {
        workspace: false,
        work_item: false,
        session: false,
    };
    write_compatibility_fixture(&manifest_path, &manifest, &wrong_identity);
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::RouteIdentity { .. })
    ));

    let mut unknown_operation = routing.clone();
    let operation = unknown_operation
        .commands
        .iter_mut()
        .find(|route| route.id == "doc resolve")
        .expect("typed route");
    operation.route = RouteTarget::TypedOperation {
        operation: "unknown.operation".to_owned(),
    };
    write_compatibility_fixture(&manifest_path, &manifest, &unknown_operation);
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::RouteTarget { .. })
    ));

    let mut invalid_rejection = routing.clone();
    let rejected = invalid_rejection
        .commands
        .iter_mut()
        .find(|route| route.id == "scripts-dir")
        .expect("rejected route");
    if let RouteTarget::Rejected {
        stable_code,
        remediation,
    } = &mut rejected.route
    {
        *stable_code = "UNKNOWN".to_owned();
        remediation.clear();
    } else {
        panic!("scripts-dir must be rejected");
    }
    write_compatibility_fixture(&manifest_path, &manifest, &invalid_rejection);
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::RouteTarget { .. })
    ));

    write_compatibility_fixture(&manifest_path, &manifest, &routing);
    assert!(matches!(
        audit_compatibility(
            &manifest_path,
            ExpectedCounts::legacy(),
            &[PathBuf::from(&routing.commands[0].fixture)]
        ),
        Err(ManifestError::EvidencePath(_))
    ));

    let mut duplicate_capability = routing.clone();
    duplicate_capability.capabilities[1].surface = duplicate_capability.capabilities[0].surface;
    duplicate_capability.capabilities[1].id = duplicate_capability.capabilities[0].id.clone();
    write_compatibility_fixture(&manifest_path, &manifest, &duplicate_capability);
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::DuplicateId {
            surface: "capability evidence",
            ..
        })
    ));

    let mut open_capability = routing.clone();
    open_capability.capabilities[0].fail_closed = false;
    write_compatibility_fixture(&manifest_path, &manifest, &open_capability);
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::NotFailClosed(_))
    ));

    let mut uncovered_capability = routing.clone();
    uncovered_capability.capabilities.pop();
    write_compatibility_fixture(&manifest_path, &manifest, &uncovered_capability);
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::EvidenceCoverage { .. })
    ));

    let mut pending_capability = routing.clone();
    pending_capability.capabilities[0].status = ImplementationStatus::Pending;
    write_compatibility_fixture(&manifest_path, &manifest, &pending_capability);
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::UnimplementedCapabilities(_))
    ));

    let mut mismatched_capability = routing.clone();
    let lease_break = mismatched_capability
        .capabilities
        .iter_mut()
        .find(|entry| entry.surface == CapabilitySurface::Operation && entry.id == "lease.break")
        .expect("lease.break capability");
    lease_break.status = ImplementationStatus::Implemented;
    write_compatibility_fixture(&manifest_path, &manifest, &mismatched_capability);
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::CapabilityStatus { .. })
    ));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn compatibility_audit_reports_decode_read_and_repository_root_failures() {
    let missing = fixture_root("missing-manifest").join("missing.json");
    assert!(matches!(
        audit_compatibility(&missing, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::Read { .. })
    ));
    fs::remove_dir_all(missing.parent().expect("missing parent")).expect("missing cleanup");

    let invalid_root = fixture_root("invalid-manifest");
    fs::write(invalid_root.join("Cargo.toml"), "[workspace]\n").expect("workspace marker");
    let invalid = invalid_root.join("invalid.json");
    fs::write(&invalid, b"not-json").expect("invalid JSON");
    assert!(matches!(
        audit_compatibility(&invalid, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::Decode(_))
    ));
    fs::remove_dir_all(invalid_root).expect("invalid cleanup");

    let root = fixture_root("repository-root");
    let source = repository_root().join("tests/fixtures/compatibility");
    let manifest_path = root.join("nested/legacy-surface.v1.json");
    fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("manifest directory");
    fs::copy(source.join("legacy-surface.v1.json"), &manifest_path).expect("copy manifest");
    fs::copy(
        source.join("cli-routing.v1.json"),
        manifest_path
            .parent()
            .expect("manifest parent")
            .join("cli-routing.v1.json"),
    )
    .expect("copy routing");
    assert!(matches!(
        audit_compatibility(&manifest_path, ExpectedCounts::legacy(), &[]),
        Err(ManifestError::RepositoryRoot(_))
    ));
    fs::remove_dir_all(root).expect("repository root cleanup");
}
