use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use ae_sdd_build::{
    B_OFFLINE_ENTRYPOINTS, CompileInput, DistributorEntry, ExecutionMode, InstructionLanguage,
    JobInput, ManagedInstructionPlan, ManagedInstructionRenderRequest, NativeJobRequest,
    OfflineCommand, OfflineRequest, execute_native_job, execute_offline,
    render_managed_instruction,
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
    let rust_asset =
        fs::read_to_string(rust.path().join(".ae-sdd/assets/oracle-project.assets.md"))
            .expect("Rust canonical asset");
    let rust_assets = json!({
        "projectKey": "oracle-project",
        "changed": !rust_result.changed_paths.is_empty(),
        "buildFileDiscovered": rust_asset.contains("Cargo.toml")
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
            .join(".ae-sdd/assets/oracle-project.assets.md")
            .is_file()
    );
}

#[test]
fn migration_0005_installs_resource_context_tables_and_monotonic_version() {
    let repo = repository_root();
    let home = FixtureRoot::new("resource-context-migration-home");
    let result = python_script(
        r#"import json,sqlite3,sys
from pathlib import Path
root=Path(sys.argv[1])
db=sqlite3.connect(":memory:")
db.execute("PRAGMA foreign_keys=ON")
for migration in sorted((root / "migrations").glob("*.sql")):
    db.executescript(migration.read_text(encoding="utf-8"))
tables={row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
indexes={row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='index'")}
invalid_digest_rejected=False
try:
    db.execute("INSERT INTO resource_resolution(workspace_id,work_item_id,resource_id,resource_kind,intent,winner_path,winner_digest,byte_length,inventory_generation,source_layer,resolution_digest,resolved_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)", ("w","i","r","story","read","x","bad",1,0,"canonical","0"*64,"2026-07-24T00:00:00Z"))
except sqlite3.IntegrityError:
    invalid_digest_rejected=True
print(json.dumps({
    "userVersion": db.execute("PRAGMA user_version").fetchone()[0],
    "tables": sorted(tables),
    "indexes": sorted(indexes),
    "invalidDigestRejected": invalid_digest_rejected,
}))"#,
        std::slice::from_ref(&repo),
        home.path(),
    );

    // Part D added migrations 0007/0008, so the final user_version is now >= 5
    // rather than exactly 5. The test's intent is to verify that 0005 installs
    // its tables and that user_version is at least 5 (monotonic).
    assert!(
        result["userVersion"].as_u64().unwrap() >= 5,
        "user_version should be >= 5 after applying all migrations"
    );
    for table in [
        "resource_resolution",
        "loaded_context_proof",
        "document_transaction_plan",
        "document_transaction_operation",
        "compact_checkpoint",
    ] {
        assert!(
            result["tables"]
                .as_array()
                .expect("table list")
                .iter()
                .any(|value| value == table),
            "missing migration table {table}"
        );
    }
    for index in [
        "resource_resolution_lookup",
        "loaded_context_proof_freshness",
        "document_transaction_plan_status",
        "compact_checkpoint_status_deadline",
    ] {
        assert!(
            result["indexes"]
                .as_array()
                .expect("index list")
                .iter()
                .any(|value| value == index),
            "missing migration index {index}"
        );
    }
    assert_eq!(result["invalidDigestRejected"], true);
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
    let valid = home.path().join("valid-runtime");
    let tampered = home.path().join("tampered-runtime");
    let missing = home.path().join("missing-runtime");
    legacy_runtime_fixture(&valid, LEGACY_RUNTIME_FINGERPRINT);
    legacy_runtime_fixture(&tampered, LEGACY_RUNTIME_FINGERPRINT);
    write(
        tampered.join("runtime/manifest.json"),
        &legacy_runtime_manifest(LEGACY_TAMPERED_FINGERPRINT),
    );
    let legacy = python_script(
        r#"import json,sys
from pathlib import Path
repo,valid,tampered,missing=(Path(argument) for argument in sys.argv[1:5])
sys.path.insert(0, str(repo / "tools"))
from lib import runtime_verify
print(json.dumps({"validAccepted":runtime_verify.verify_runtime_package(valid).ok,"tamperRejected":not runtime_verify.verify_runtime_package(tampered).ok,"missingRejected":not runtime_verify.verify_runtime_package(missing).ok}))"#,
        &[repo, valid.clone(), tampered.clone(), missing.clone()],
        home.path(),
    );
    let rust_valid = execute_offline(&request(
        OfflineCommand::RuntimeVerify {
            package_directory: valid,
        },
        "runtime-verify-valid",
        ExecutionMode::DryRun,
    ));
    let rust_tampered = execute_offline(&request(
        OfflineCommand::RuntimeVerify {
            package_directory: tampered,
        },
        "runtime-verify-tampered",
        ExecutionMode::DryRun,
    ));
    let rust_missing = execute_offline(&request(
        OfflineCommand::RuntimeVerify {
            package_directory: missing,
        },
        "runtime-verify-missing",
        ExecutionMode::DryRun,
    ));
    let expected = json!({
        "validAccepted": true,
        "tamperRejected": true,
        "missingRejected": true
    });
    assert_eq!(
        legacy, expected,
        "Python oracle must accept the legacy fixture and reject both defects"
    );
    assert_eq!(
        json!({
            "validAccepted": rust_valid.is_ok(),
            "tamperRejected": rust_tampered.is_err(),
            "missingRejected": rust_missing.is_err()
        }),
        expected,
        "Rust must reach the same verdicts as the Python oracle"
    );
    assert_eq!(
        rust_valid.expect("Rust legacy verify").payload["format"],
        "legacy-oracle",
        "the fixture must exercise the legacy branch, not the native one"
    );
}

/// Fingerprint published by both `SKILL.md` and `runtime/manifest.json` in a
/// coherent legacy package.
const LEGACY_RUNTIME_FINGERPRINT: &str =
    "1f0d3a5c7e9b2d4f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7";
/// Fingerprint written into the manifest alone so `SKILL.md` no longer carries
/// it, which is the tamper both verifiers must reject.
const LEGACY_TAMPERED_FINGERPRINT: &str =
    "d7c6b5a4938271605f4e3d2c1b0a9f8e7d6c5b4a3928170695f4d2e7c5a3d0f1";

/// Builds the minimal legacy runtime package that satisfies both the Python
/// `runtime_verify` contract and the Rust `legacy_runtime` branch. `runtime/`
/// deliberately omits `build-manifest.json` so the Rust verifier does not
/// dispatch to its native format.
fn legacy_runtime_fixture(package: &Path, fingerprint: &str) {
    write(
        package.join("SKILL.md"),
        &format!(
            "---\nname: oracle\nversion: 1.2.3\ncompiled: true\nruntime: runtime/manifest.json\nruntime_fingerprint: {fingerprint}\n---\n\n# ae-sdd Compiled Runtime Entry\n"
        ),
    );
    write(
        package.join("runtime/manifest.json"),
        &legacy_runtime_manifest(fingerprint),
    );
    write(
        package.join("runtime/core.compact.md"),
        "# Core\n\nG-08 and G-14 stay reachable from the compiled fast path.\n",
    );
    write(
        package.join("runtime/fallback/SKILL.full.md"),
        "# ae-sdd Method Source\n\nThis fallback carries the uncompiled method text so the\nverifier can prove the package preserved its source of truth rather than\nshipping only the generated bootloader. G-08, G-14, and TR-1 appear here.\n",
    );
}

/// Renders the legacy `runtime/manifest.json` body for `fingerprint`. Kept
/// separate so a test can rewrite the manifest alone and leave `SKILL.md`
/// pointing at the original fingerprint.
fn legacy_runtime_manifest(fingerprint: &str) -> String {
    serde_json::to_string_pretty(&json!({
        "schema": "ae-sdd-runtime/v1",
        "version": "1.2.3",
        "compiled": true,
        "deterministic": true,
        "compiler": {"name": "ae-sdd-oracle", "version": "1.2.3"},
        "runtime_fingerprint": fingerprint,
        "entry": "SKILL.md",
        "load_order": ["runtime/core.compact.md"],
        "generated_files": ["runtime/core.compact.md"],
        "source": {
            "skill_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "fallback_sha256": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
            "checksums": {"source/SKILL.md": "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0"}
        },
        "extracts": {"gate_count": 36, "flow_scales": ["大", "中", "小", "微"]}
    }))
    .expect("legacy manifest serializes")
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

const MANAGED_L2_SSOT: &str = concat!(
    "<!-- ae-sdd L2 conversation discipline SSOT -->\n",
    "\n",
    "<!-- SECTION:zh -->\n",
    "## 强制工作流\n",
    "\n",
    "- 中文条目\n",
    "<!-- /SECTION:zh -->\n",
    "\n",
    "<!-- SECTION:en -->\n",
    "## Mandatory Workflow\n",
    "\n",
    "- English item\n",
    "<!-- /SECTION:en -->\n",
);

/// Drives the legacy Python injector against a temporary SSOT and target file.
///
/// `scripts/l2_inject.py` reads the SSOT from a fixed repository path, so the
/// oracle patches `_read_ssot` instead of copying the released source. Only the
/// normal anchored-injection path is exercised; bootstrap and rollback remain out
/// of scope for the Rust port.
const PYTHON_L2_ORACLE: &str = r#"
import json, sys
from pathlib import Path
sys.path.insert(0, "scripts")
import l2_inject

ssot_path = Path(sys.argv[1])
target_path = Path(sys.argv[2])
language = sys.argv[3]
l2_inject._read_ssot = lambda: ssot_path.read_text(encoding="utf-8")
l2_inject._git_commit = lambda: "0123456"
result = l2_inject._inject_anchored(target_path, language, True)
print(json.dumps({
    "status": result.status,
    "contents": target_path.read_text(encoding="utf-8"),
}, ensure_ascii=False))
"#;

/// Normalizes the audit header so the Rust and Python blocks are comparable.
///
/// Rust intentionally removes the wall-clock field to keep rendering
/// deterministic and replay-stable, and records a Rust adapter label instead.
fn normalize_audit_header(contents: &str) -> String {
    contents
        .lines()
        .map(|line| {
            if line.contains("BEGIN ae-sdd-l2-ssot") {
                let hash = line
                    .split("hash=")
                    .nth(1)
                    .and_then(|rest| rest.split_whitespace().next())
                    .unwrap_or_default()
                    .to_owned();
                format!("<!-- BEGIN ae-sdd-l2-ssot hash={hash} -->")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn python_l2_injection(ssot: &Path, target: &Path, language: &str, home: &Path) -> Value {
    let mut command_args = vec![OsString::from("-c"), OsString::from(PYTHON_L2_ORACLE)];
    command_args.push(ssot.as_os_str().to_owned());
    command_args.push(target.as_os_str().to_owned());
    command_args.push(OsString::from(language));
    let output = Command::new(std::env::var_os("PYTHON").unwrap_or_else(|| "python".into()))
        .args(command_args)
        .current_dir(repository_root())
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .output()
        .expect("Python L2 injection oracle must be installed for migration tests");
    assert!(
        output.status.success(),
        "Python L2 oracle failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("Python L2 oracle JSON")
}

fn rust_l2_injection(ssot: &str, target: &str, language: InstructionLanguage) -> (String, String) {
    let plan = render_managed_instruction(&ManagedInstructionRenderRequest {
        source: ssot,
        target,
        language,
        revision: "0123456",
    })
    .expect("Rust renderer must accept the oracle fixture");
    match plan {
        ManagedInstructionPlan::Updated { contents, .. } => ("ok".to_owned(), contents),
        ManagedInstructionPlan::Unchanged { .. } => ("skip".to_owned(), target.to_owned()),
        ManagedInstructionPlan::MissingAnchor => ("skip_no_anchor".to_owned(), target.to_owned()),
    }
}

fn managed_l2_fixture(root: &Path, target_body: &str) -> (PathBuf, PathBuf) {
    let ssot = root.join("L2-DISCIPLINE.md");
    let target = root.join("AGENTS.md");
    write(&ssot, MANAGED_L2_SSOT);
    write(&target, target_body);
    (ssot, target)
}

fn anchored_oracle_target(prefix: &str, stale: &str, suffix: &str) -> String {
    format!(
        "{prefix}\n\n<!-- BEGIN ae-sdd-l2-ssot @ deadbee @ 20260101T000000Z (hash=000000000000 v=1.0.0) -->\n{stale}\n<!-- END ae-sdd-l2-ssot -->\n\n{suffix}\n"
    )
}

#[test]
fn migration_oracle_managed_instruction_anchored_update_matches_python() {
    let root = FixtureRoot::new("managed-l2-update");
    for (label, language, rust_language, expected_body) in [
        (
            "english",
            "en",
            InstructionLanguage::En,
            "## Mandatory Workflow",
        ),
        ("chinese", "zh", InstructionLanguage::Zh, "## 强制工作流"),
    ] {
        let case = root.path().join(label);
        fs::create_dir_all(&case).expect("oracle case root");
        let body = anchored_oracle_target(
            "# Global Instructions\n\n## Personal preface",
            "## Stale discipline",
            "## Skill Source\n\n- keep me",
        );
        let (ssot, target) = managed_l2_fixture(&case, &body);

        let (rust_status, rust_contents) = rust_l2_injection(MANAGED_L2_SSOT, &body, rust_language);
        let legacy = python_l2_injection(&ssot, &target, language, root.path());

        assert_eq!(legacy["status"], rust_status, "case {label}");
        let legacy_contents = legacy["contents"].as_str().expect("legacy contents");
        assert_eq!(
            normalize_audit_header(legacy_contents),
            normalize_audit_header(&rust_contents),
            "case {label}: Rust and Python must preserve identical injected semantics"
        );
        assert!(rust_contents.contains(expected_body), "case {label}");
    }
}

#[test]
fn migration_oracle_managed_instruction_skips_unanchored_target_like_python() {
    let root = FixtureRoot::new("managed-l2-skip");
    let body = "# Global Instructions\n\n## Hand written only\n";
    let (ssot, target) = managed_l2_fixture(root.path(), body);

    let (rust_status, rust_contents) =
        rust_l2_injection(MANAGED_L2_SSOT, body, InstructionLanguage::En);
    let legacy = python_l2_injection(&ssot, &target, "en", root.path());

    assert_eq!(rust_status, "skip_no_anchor");
    assert_eq!(legacy["status"], "skip_no_anchor");
    assert_eq!(legacy["contents"].as_str(), Some(body));
    assert_eq!(rust_contents, body);
}

#[test]
fn migration_oracle_managed_instruction_preserves_bytes_outside_the_anchor_like_python() {
    let root = FixtureRoot::new("managed-l2-outside");
    let prefix = "# Global Instructions\n\n## Personal preface\n\n- keep me";
    let suffix = "## Skill Source\n\n- path: C:/Users/example/.codex/skills/ae-sdd/SKILL.md\n\n## Sync Discipline\n\n- keep this too";
    let body = anchored_oracle_target(prefix, "## Stale", suffix);
    let (ssot, target) = managed_l2_fixture(root.path(), &body);

    let (_, rust_contents) = rust_l2_injection(MANAGED_L2_SSOT, &body, InstructionLanguage::En);
    let legacy = python_l2_injection(&ssot, &target, "en", root.path());
    let legacy_contents = legacy["contents"].as_str().expect("legacy contents");

    let outside = |text: &str| {
        let before = text
            .split("<!-- BEGIN ae-sdd-l2-ssot")
            .next()
            .expect("prefix")
            .to_owned();
        let after = text
            .split("<!-- END ae-sdd-l2-ssot -->")
            .nth(1)
            .expect("suffix")
            .to_owned();
        (before, after)
    };
    let original = outside(&body);
    assert_eq!(outside(&rust_contents), original);
    assert_eq!(outside(legacy_contents), original);
}

#[test]
fn migration_oracle_managed_instruction_keeps_python_out_of_the_released_chain() {
    let root = repository_root();
    let hook = fs::read_to_string(root.join(".githooks/post-commit")).expect("post-commit hook");
    assert!(!hook.to_ascii_lowercase().contains("python"));
    assert!(!hook.contains("l2_inject"));
    assert!(hook.contains("--codex-instructions"));

    let package = root.join("dist/ae-sdd");
    if package.is_dir() {
        assert!(
            package.join("L2-DISCIPLINE.md").is_file(),
            "the compiled package must carry the L2 SSOT the Rust stage reads"
        );
    }
    assert!(
        root.join("scripts/l2_inject.py").is_file(),
        "the Python injector stays available as a migration oracle and manual fallback"
    );
}

#[test]
fn migration_oracle_paths_are_normalized_for_cross_platform_comparison() {
    let root = FixtureRoot::new("path-normalization");
    assert!(!normalized_path(root.path()).contains('\\'));
}
