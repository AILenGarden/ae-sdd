use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use ae_sdd_build::{
    B_OFFLINE_ENTRYPOINTS, CompileInput, DistributorEntry, ExecutionMode, JobInput,
    NativeJobRequest, OfflineCommand, OfflineRequest, execute_native_job, execute_offline,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OracleManifest {
    schema_version: String,
    commands: Vec<OracleCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OracleCase {
    id: String,
    canonical_result: Vec<String>,
    side_effects: Vec<String>,
}

struct FixtureRoot(PathBuf);

impl FixtureRoot {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ae-sdd-migration-oracle-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("fixture root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for FixtureRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn request(command: OfflineCommand, key: &str, mode: ExecutionMode) -> OfflineRequest {
    OfflineRequest {
        schema_version: "ae-sdd-offline-build/v1".to_owned(),
        mode,
        actor: "migration-oracle".to_owned(),
        reason: "compare the read-only Python oracle with the Rust kernel".to_owned(),
        idempotency_key: key.to_owned(),
        command,
    }
}

fn write(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    fs::write(path, contents).expect("fixture file");
}

fn python_output(
    program: impl AsRef<Path>,
    args: &[OsString],
    home: &Path,
    current_dir: &Path,
) -> Output {
    let output = Command::new(std::env::var_os("PYTHON").unwrap_or_else(|| "python".into()))
        .arg(program.as_ref())
        .args(args)
        .current_dir(current_dir)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .output()
        .expect("Python migration oracle must be installed for migration tests");
    assert!(
        output.status.success(),
        "Python oracle failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn python_script(script: &str, args: &[PathBuf], home: &Path) -> Value {
    let mut command_args = vec![OsString::from("-c"), OsString::from(script)];
    command_args.extend(args.iter().map(|path| path.as_os_str().to_owned()));
    let output = Command::new(std::env::var_os("PYTHON").unwrap_or_else(|| "python".into()))
        .args(command_args)
        .current_dir(repository_root())
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .output()
        .expect("Python migration oracle");
    assert!(
        output.status.success(),
        "Python oracle failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("Python oracle JSON")
}

fn run_python_file(script: &Path, args: &[OsString], home: &Path, current_dir: &Path) {
    let _ = python_output(script, args, home, current_dir);
}

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn migration_oracle_fixture_covers_the_exact_b13_surface() {
    let fixture = repository_root().join("tests/fixtures/compatibility/migration-oracle.v1.json");
    let manifest: OracleManifest =
        serde_json::from_slice(&fs::read(fixture).expect("oracle fixture"))
            .expect("strict oracle fixture");
    assert_eq!(manifest.schema_version, "ae-sdd-migration-oracle/v1");
    let ids = manifest
        .commands
        .iter()
        .map(|case| case.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, B_OFFLINE_ENTRYPOINTS);
    assert!(
        manifest
            .commands
            .iter()
            .all(|case| !case.canonical_result.is_empty())
    );
    assert_eq!(
        manifest
            .commands
            .iter()
            .filter(|case| case.side_effects.is_empty())
            .count(),
        4
    );
}

#[test]
fn migration_oracle_version_and_assets_match_canonical_results_and_side_effects() {
    let home = FixtureRoot::new("version-assets-home");
    let repo = repository_root();
    let legacy_version = python_script(
        r#"import json,sys
from pathlib import Path
sys.path.insert(0, str(Path(sys.argv[1]) / "tools"))
from lib import paths
print(json.dumps({"name":"ae-sdd","version":paths.MASTER_VERSION}))"#,
        std::slice::from_ref(&repo),
        home.path(),
    );
    let rust_version = execute_offline(&request(
        OfflineCommand::Version,
        "version",
        ExecutionMode::DryRun,
    ))
    .expect("Rust version");
    assert_eq!(
        legacy_version,
        json!({
            "name": rust_version.payload["name"],
            "version": rust_version.payload["version"]
        })
    );

    let legacy = FixtureRoot::new("assets-legacy");
    let rust = FixtureRoot::new("assets-rust");
    for root in [legacy.path(), rust.path()] {
        write(root.join("Cargo.toml"), "[package]\nname = \"oracle\"\n");
        write(root.join("src/lib.rs"), "pub fn oracle() {}\n");
        write(
            root.join(".ae-sdd/config.yaml"),
            "projectKey: oracle-project\n",
        );
    }
    let legacy_assets = python_script(
        r#"import json,sys
from pathlib import Path
repo,root=Path(sys.argv[1]),Path(sys.argv[2])
sys.path.insert(0, str(repo / "tools"))
from lib import project_assets
r=project_assets.generate_project_assets(root / ".ae-sdd", "oracle-project", project_root=root)
text=r.asset_file.read_text(encoding="utf-8")
print(json.dumps({"projectKey":r.project_key,"changed":r.changed,"buildFileDiscovered":"Cargo.toml" in text}))"#,
        &[repo, legacy.path().to_path_buf()],
        home.path(),
    );
    let rust_result = execute_offline(&request(
        OfflineCommand::AssetsGenerate {
            project_root: rust.path().to_path_buf(),
            project_key: "oracle-project".to_owned(),
        },
        "assets",
        ExecutionMode::Apply,
    ))
    .expect("Rust assets");
    let rust_asset: Value = serde_json::from_slice(
        &fs::read(
            rust.path()
                .join(".ae-sdd/assets/oracle-project.assets.json"),
        )
        .expect("Rust asset"),
    )
    .expect("Rust asset JSON");
    let rust_assets = json!({
        "projectKey": rust_asset["projectKey"],
        "changed": !rust_result.changed_paths.is_empty(),
        "buildFileDiscovered": rust_asset["files"]
            .as_array()
            .expect("asset files")
            .iter()
            .any(|item| item["path"] == "Cargo.toml")
    });
    assert_eq!(legacy_assets, rust_assets);
    assert!(
        legacy
            .path()
            .join(".ae-sdd/assets/oracle-project.assets.md")
            .is_file()
    );
    assert!(
        rust.path()
            .join(".ae-sdd/assets/oracle-project.assets.json")
            .is_file()
    );
}

#[test]
fn migration_oracle_hooks_and_plugin_match_canonical_results_and_side_effects() {
    let repo = repository_root();
    let home = FixtureRoot::new("hooks-plugin-home");
    let legacy = FixtureRoot::new("hooks-plugin-legacy");
    let rust = FixtureRoot::new("hooks-plugin-rust");
    for root in [legacy.path(), rust.path()] {
        write(
            root.join(".ae-sdd/config.yaml"),
            "projectKey: oracle-project\n",
        );
    }
    run_python_file(
        &repo.join("tools/bin/ae-sdd"),
        &[
            OsString::from("init-hooks"),
            legacy.path().as_os_str().to_owned(),
            OsString::from("--force"),
        ],
        home.path(),
        legacy.path(),
    );
    execute_offline(&request(
        OfflineCommand::InitHooks {
            project_root: rust.path().to_path_buf(),
            executable: "ae-sdd".to_owned(),
            hosts: vec!["claude".to_owned()],
        },
        "hooks",
        ExecutionMode::Apply,
    ))
    .expect("Rust hooks");
    let legacy_hooks =
        fs::read_to_string(legacy.path().join(".claude/settings.json")).expect("legacy hooks");
    let rust_hooks =
        fs::read_to_string(rust.path().join(".claude/settings.json")).expect("Rust hooks");
    assert_eq!(
        json!({
            "preTool": legacy_hooks.contains("gate-intercept"),
            "userPrompt": legacy_hooks.contains("prompt-inject"),
            "stop": legacy_hooks.contains("stop-check")
        }),
        json!({
            "preTool": rust_hooks.contains("hook.pre_tool"),
            "userPrompt": rust_hooks.contains("hook.user_prompt"),
            "stop": rust_hooks.contains("hook.stop")
        })
    );

    run_python_file(
        &repo.join("tools/bin/ae-sdd"),
        &[
            OsString::from("plugin"),
            OsString::from("init"),
            OsString::from("--layer"),
            OsString::from("project"),
            OsString::from("--force"),
        ],
        home.path(),
        legacy.path(),
    );
    let rust_plugins = rust.path().join(".ae-sdd/plugins");
    fs::create_dir_all(&rust_plugins).expect("Rust plugins root");
    execute_offline(&request(
        OfflineCommand::PluginInit {
            plugins_root: rust_plugins.clone(),
            name: "oracle-plugin".to_owned(),
            description: "Migration oracle plugin".to_owned(),
        },
        "plugin",
        ExecutionMode::Apply,
    ))
    .expect("Rust plugin init");
    let legacy_registry = legacy.path().join(".ae-sdd/plugins/registry.yaml");
    let rust_registry = rust_plugins.join("registry.yaml");
    assert_eq!(
        json!({
            "registryCreated": legacy_registry.is_file(),
            "pluginsKey": fs::read_to_string(legacy_registry).expect("legacy registry").contains("plugins:")
        }),
        json!({
            "registryCreated": rust_registry.is_file(),
            "pluginsKey": fs::read_to_string(rust_registry).expect("Rust registry").contains("\"plugins\"")
        })
    );
}

#[test]
fn migration_oracle_distributor_commands_match_canonical_results_and_side_effects() {
    let repo = repository_root();
    let legacy_home = FixtureRoot::new("distributor-legacy-home");
    let rust_home = FixtureRoot::new("distributor-rust-home");
    let legacy_target = legacy_home.path().join("oracle-target");
    let rust_target = rust_home.path().join("oracle-target");
    fs::create_dir_all(&legacy_target).expect("legacy target");
    fs::create_dir_all(&rust_target).expect("Rust target");
    fs::create_dir_all(legacy_home.path().join(".codex/skills/ae-sdd"))
        .expect("legacy scan target");
    let legacy = normalize_fixture_paths(
        python_script(
            r#"import json,sys
from pathlib import Path
repo,target=Path(sys.argv[1]),Path(sys.argv[2])
sys.path.insert(0, str(repo / "tools"))
from lib import distributor_registry as dr
def canon(entry):
    return {"name":entry.name,"kind":entry.protocol.replace("_","-"),"targetPath":str(Path(entry.target_path)).replace("\\","/"),"enabled":entry.enabled}
ok,_,entries=dr.register_one("oracle","copytree",str(target),force=False)
registered=canon(next(e for e in entries if e.name=="oracle"))
listed=canon(next(e for e in dr.load_registry() if e.name=="oracle"))
dr.set_enabled("oracle",False)
disabled=next(e.enabled for e in dr.load_registry() if e.name=="oracle")
dr.set_enabled("oracle",True)
enabled=next(e.enabled for e in dr.load_registry() if e.name=="oracle")
scan=any(item["name"]=="codex" and item["found"] for item in dr.scan_for_agents())
dr.unregister_one("oracle")
present=any(e.name=="oracle" for e in dr.load_registry())
print(json.dumps({"registered":registered,"listed":listed,"disabled":disabled,"enabled":enabled,"scanTargetExists":scan,"presentAfterUnregister":present}))"#,
            &[repo, legacy_target],
            legacy_home.path(),
        ),
        legacy_home.path(),
    );

    let registry = rust_home.path().join("distributors.json");
    let entry = DistributorEntry {
        name: "oracle".to_owned(),
        kind: "copytree".to_owned(),
        target_path: rust_target.clone(),
        enabled: true,
    };
    execute_offline(&request(
        OfflineCommand::DistributorRegister {
            registry_file: registry.clone(),
            entry,
        },
        "distributor-register",
        ExecutionMode::Apply,
    ))
    .expect("Rust register");
    let registered = rust_distributor(&registry);
    let listed = rust_distributor(&registry);
    execute_offline(&request(
        OfflineCommand::DistributorDisable {
            registry_file: registry.clone(),
            name: "oracle".to_owned(),
        },
        "distributor-disable",
        ExecutionMode::Apply,
    ))
    .expect("Rust disable");
    let disabled = rust_distributor(&registry)["enabled"]
        .as_bool()
        .expect("disabled flag");
    execute_offline(&request(
        OfflineCommand::DistributorEnable {
            registry_file: registry.clone(),
            name: "oracle".to_owned(),
        },
        "distributor-enable",
        ExecutionMode::Apply,
    ))
    .expect("Rust enable");
    let enabled = rust_distributor(&registry)["enabled"]
        .as_bool()
        .expect("enabled flag");
    let scan = execute_offline(&request(
        OfflineCommand::DistributorScan {
            registry_file: registry.clone(),
        },
        "distributor-scan",
        ExecutionMode::DryRun,
    ))
    .expect("Rust scan");
    execute_offline(&request(
        OfflineCommand::DistributorUnregister {
            registry_file: registry.clone(),
            name: "oracle".to_owned(),
        },
        "distributor-unregister",
        ExecutionMode::Apply,
    ))
    .expect("Rust unregister");
    let after = execute_offline(&request(
        OfflineCommand::DistributorList {
            registry_file: registry.clone(),
        },
        "distributor-list-after",
        ExecutionMode::DryRun,
    ))
    .expect("Rust list");
    let rust = normalize_fixture_paths(
        json!({
            "registered": registered,
            "listed": listed,
            "disabled": disabled,
            "enabled": enabled,
            "scanTargetExists": scan.payload["entries"][0]["targetExists"],
            "presentAfterUnregister": !after.payload["entries"].as_array().expect("entries").is_empty()
        }),
        rust_home.path(),
    );
    assert_eq!(legacy, rust);
    assert!(
        legacy_home
            .path()
            .join(".ae-sdd/distributors.json")
            .is_file()
    );
    assert!(registry.is_file());
}

fn rust_distributor(registry: &Path) -> Value {
    let listed = execute_offline(&request(
        OfflineCommand::DistributorList {
            registry_file: registry.to_path_buf(),
        },
        "distributor-list",
        ExecutionMode::DryRun,
    ))
    .expect("Rust list");
    let entry = &listed.payload["entries"][0];
    json!({
        "name": entry["name"],
        "kind": entry["kind"],
        "targetPath": entry["targetPath"]
            .as_str()
            .map(|value| value.replace('\\', "/"))
            .expect("target path"),
        "enabled": entry["enabled"]
    })
}

fn normalize_fixture_paths(value: Value, root: &Path) -> Value {
    let root = normalized_path(root);
    match value {
        Value::String(value) => Value::String(value.replace(&root, "$FIXTURE")),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| normalize_fixture_paths(value, Path::new(&root)))
                .collect(),
        ),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, normalize_fixture_paths(value, Path::new(&root))))
                .collect(),
        ),
        other => other,
    }
}

#[test]
fn migration_oracle_init_and_bump_use_temp_repositories_and_match_side_effects() {
    let repo = repository_root();
    let home = FixtureRoot::new("init-bump-home");
    let legacy_init = FixtureRoot::new("init-legacy");
    let rust_init = FixtureRoot::new("init-rust");
    run_python_file(
        &repo.join("scripts/init.py"),
        &[
            legacy_init.path().as_os_str().to_owned(),
            OsString::from("oracle-project"),
            OsString::from("--no-asset"),
            OsString::from("--no-hooks"),
        ],
        home.path(),
        legacy_init.path(),
    );
    execute_offline(&request(
        OfflineCommand::Init {
            project_root: rust_init.path().to_path_buf(),
            project_key: "oracle-project".to_owned(),
            force: false,
        },
        "init",
        ExecutionMode::Apply,
    ))
    .expect("Rust init");
    assert_eq!(
        init_projection(legacy_init.path()),
        init_projection(rust_init.path())
    );

    let legacy_bump = FixtureRoot::new("bump-legacy");
    let rust_bump = FixtureRoot::new("bump-rust");
    write(
        legacy_bump.path().join("source/SKILL.md"),
        "---\nname: ae-sdd\nversion: 1.2.3\n---\n",
    );
    write(
        legacy_bump.path().join("tools/lib/paths.py"),
        "MASTER_VERSION = \"1.2.3\"\n",
    );
    write(
        legacy_bump.path().join("README.md"),
        "# ae-sdd\n\n**\u{7248}\u{672c}\u{ff1a}** v1.2.3\n",
    );
    write(rust_bump.path().join("source/SKILL.md"), "version: 1.2.3\n");
    write(
        rust_bump.path().join("tools/lib/paths.py"),
        "MASTER_VERSION = \"1.2.3\"\n",
    );
    write(rust_bump.path().join("README.md"), "> **版本：** v1.2.3\n");
    let legacy = python_script(
        r#"import json,sys
from pathlib import Path
repo,root=Path(sys.argv[1]),Path(sys.argv[2])
sys.path.insert(0, str(repo / "tools"))
from lib import update_graph
r=update_graph.bump_version(root,"1.2.4")
texts=[(root/"source/SKILL.md").read_text(encoding="utf-8"),(root/"tools/lib/paths.py").read_text(encoding="utf-8"),(root/"README.md").read_text(encoding="utf-8")]
print(json.dumps({"oldVersion":r["old"],"newVersion":r["new"],"changedAuthorityCount":len(r["written"]),"allUpdated":all("1.2.4" in text for text in texts)}))"#,
        &[repo, legacy_bump.path().to_path_buf()],
        home.path(),
    );
    let rust_result = execute_offline(&request(
        OfflineCommand::Bump {
            repository_root: rust_bump.path().to_path_buf(),
            expected_version: "1.2.3".to_owned(),
            new_version: "1.2.4".to_owned(),
        },
        "bump",
        ExecutionMode::Apply,
    ))
    .expect("Rust bump");
    let rust_texts = [
        fs::read_to_string(rust_bump.path().join("source/SKILL.md")).expect("skill"),
        fs::read_to_string(rust_bump.path().join("tools/lib/paths.py")).expect("paths"),
        fs::read_to_string(rust_bump.path().join("README.md")).expect("README"),
    ];
    assert_eq!(
        legacy,
        json!({
            "oldVersion": "1.2.3",
            "newVersion": "1.2.4",
            "changedAuthorityCount": rust_result.changed_paths.len(),
            "allUpdated": rust_texts.iter().all(|text| text.contains("1.2.4"))
        })
    );
}

fn init_projection(root: &Path) -> Value {
    let config = fs::read_to_string(root.join(".ae-sdd/config.yaml")).expect("project config");
    json!({
        "projectKey": config.lines().any(|line| line.trim() == "projectKey: oracle-project"),
        "runtimeRoot": root.join(".ae-sdd").is_dir(),
        "overrideRoot": root.join(".ae-sdd/overrides").is_dir(),
        "reportRoot": root.join(".ae-sdd/reports").is_dir()
    })
}

#[test]
fn migration_oracle_runtime_verify_accepts_and_rejects_the_same_legacy_fixtures() {
    let repo = repository_root();
    let home = FixtureRoot::new("runtime-verify-home");
    let valid = repo.join("dist/ae-sdd");
    let missing = home.path().join("missing-runtime");
    let legacy = python_script(
        r#"import json,sys
from pathlib import Path
repo,valid,missing=Path(sys.argv[1]),Path(sys.argv[2]),Path(sys.argv[3])
sys.path.insert(0, str(repo / "tools"))
from lib import runtime_verify
print(json.dumps({"validAccepted":runtime_verify.verify_runtime_package(valid).ok,"tamperRejected":not runtime_verify.verify_runtime_package(missing).ok}))"#,
        &[repo, valid.clone(), missing.clone()],
        home.path(),
    );
    let rust_valid = execute_offline(&request(
        OfflineCommand::RuntimeVerify {
            package_directory: valid,
        },
        "runtime-verify-valid",
        ExecutionMode::DryRun,
    ));
    let rust_invalid = execute_offline(&request(
        OfflineCommand::RuntimeVerify {
            package_directory: missing,
        },
        "runtime-verify-invalid",
        ExecutionMode::DryRun,
    ));
    assert_eq!(
        legacy,
        json!({
            "validAccepted": rust_valid.is_ok(),
            "tamperRejected": rust_invalid.is_err()
        })
    );
}

#[test]
fn migration_oracle_native_runtime_fixture_remains_python_free() {
    let root = FixtureRoot::new("native-runtime");
    let source = root.path().join("source");
    let output = root.path().join("output");
    fs::create_dir_all(&source).expect("native source");
    write(source.join("SKILL.md"), "---\nname: oracle\n---\n");
    execute_native_job(&NativeJobRequest {
        schema_version: "ae-sdd-native-job/v1".to_owned(),
        entrypoint: "compile".to_owned(),
        actor: "migration-oracle".to_owned(),
        reason: "build native runtime verifier fixture".to_owned(),
        idempotency_key: "native-runtime".to_owned(),
        mode: ExecutionMode::Apply,
        allowed_roots: vec![root.path().to_path_buf()],
        job: JobInput::Compile(CompileInput {
            source_directory: source,
            output_directory: output.clone(),
            generated_configs: Vec::new(),
        }),
    })
    .expect("native compile");
    let result = execute_offline(&request(
        OfflineCommand::RuntimeVerify {
            package_directory: output,
        },
        "native-runtime-verify",
        ExecutionMode::DryRun,
    ))
    .expect("native runtime verify");
    assert_eq!(result.payload["format"], "native");
    assert_eq!(result.payload["pythonRuntimeFiles"], 0);
}

#[test]
fn migration_oracle_paths_are_normalized_for_cross_platform_comparison() {
    let root = FixtureRoot::new("path-normalization");
    assert!(!normalized_path(root.path()).contains('\\'));
}
