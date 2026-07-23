use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ae_sdd_build::{
    B_OFFLINE_ENTRYPOINTS, CompileInput, DistributorEntry, ExecutionMode, JobInput,
    NativeJobRequest, OfflineCommand, OfflineError, OfflineRequest, execute_native_job,
    execute_offline,
};

fn fixture(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis();
    let root = std::env::temp_dir().join(format!(
        "ae-sdd-offline-{name}-{}-{millis}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("fixture root");
    root
}

fn request(command: OfflineCommand, key: &str, mode: ExecutionMode) -> OfflineRequest {
    OfflineRequest {
        schema_version: "ae-sdd-offline-build/v1".to_owned(),
        mode,
        actor: "offline-test".to_owned(),
        reason: "exercise exact Rust kernel".to_owned(),
        idempotency_key: key.to_owned(),
        command,
    }
}

#[test]
fn registry_is_exactly_the_frozen_b13_surface() {
    assert_eq!(B_OFFLINE_ENTRYPOINTS.len(), 13);
    assert_eq!(
        B_OFFLINE_ENTRYPOINTS,
        [
            "assets.generate",
            "bump",
            "distributor.disable",
            "distributor.enable",
            "distributor.list",
            "distributor.register",
            "distributor.scan",
            "distributor.unregister",
            "init",
            "init-hooks",
            "plugin.init",
            "runtime.verify",
            "version",
        ]
    );
}

#[test]
fn version_and_strict_schema_have_success_and_negative_paths() {
    let valid = execute_offline(&request(
        OfflineCommand::Version,
        "version-1",
        ExecutionMode::DryRun,
    ))
    .expect("version");
    assert_eq!(valid.payload["name"], "ae-sdd");
    assert_eq!(valid.payload["version"], "3.14.0");
    assert_eq!(valid.payload["runtime"], "rust");

    let mut invalid = request(OfflineCommand::Version, "version-2", ExecutionMode::DryRun);
    invalid.schema_version = "unknown".to_owned();
    assert!(matches!(
        execute_offline(&invalid),
        Err(OfflineError::Schema(_))
    ));
}

#[test]
fn init_and_hook_kernels_write_only_frozen_files() {
    let root = fixture("init-hooks");
    let dry = execute_offline(&request(
        OfflineCommand::Init {
            project_root: root.clone(),
            project_key: "sample-project".to_owned(),
            force: false,
        },
        "init-dry",
        ExecutionMode::DryRun,
    ))
    .expect("init dry run");
    assert_eq!(dry.changed_paths.len(), 4);
    assert!(!root.join(".ae-sdd").exists());

    execute_offline(&request(
        OfflineCommand::Init {
            project_root: root.clone(),
            project_key: "sample-project".to_owned(),
            force: false,
        },
        "init-apply",
        ExecutionMode::Apply,
    ))
    .expect("init apply");
    assert!(root.join(".ae-sdd/config.yaml").is_file());
    assert!(matches!(
        execute_offline(&request(
            OfflineCommand::Init {
                project_root: root.clone(),
                project_key: "sample-project".to_owned(),
                force: false,
            },
            "init-again",
            ExecutionMode::Apply,
        )),
        Err(OfflineError::AlreadyExists(_))
    ));

    execute_offline(&request(
        OfflineCommand::InitHooks {
            project_root: root.clone(),
            executable: "C:\\Program Files\\ae-sdd\\ae-sdd.exe".to_owned(),
            hosts: vec!["codex".to_owned(), "claude".to_owned()],
        },
        "hooks-apply",
        ExecutionMode::Apply,
    ))
    .expect("hooks apply");
    let hooks = fs::read_to_string(root.join(".codex/hooks.json")).expect("Codex hooks");
    assert!(hooks.contains("hook --method hook.pre_tool --request-json -"));
    assert!(!hooks.to_ascii_lowercase().contains("python"));
    assert!(matches!(
        execute_offline(&request(
            OfflineCommand::InitHooks {
                project_root: root.clone(),
                executable: "ae-sdd".to_owned(),
                hosts: vec!["unknown".to_owned()],
            },
            "hooks-invalid",
            ExecutionMode::DryRun,
        )),
        Err(OfflineError::InvalidInput("host"))
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn assets_and_plugin_kernels_validate_names_and_content() {
    let root = fixture("assets-plugin");
    fs::write(root.join("README.md"), "fixture\n").expect("source");
    let assets = execute_offline(&request(
        OfflineCommand::AssetsGenerate {
            project_root: root.clone(),
            project_key: "fixture-project".to_owned(),
        },
        "assets-1",
        ExecutionMode::Apply,
    ))
    .expect("assets");
    assert_eq!(assets.changed_paths.len(), 2);
    let asset = fs::read_to_string(root.join(".ae-sdd/assets/fixture-project.assets.json"))
        .expect("asset file");
    assert!(asset.contains("README.md"));
    assert!(matches!(
        execute_offline(&request(
            OfflineCommand::AssetsGenerate {
                project_root: root.clone(),
                project_key: "INVALID NAME".to_owned(),
            },
            "assets-invalid",
            ExecutionMode::DryRun,
        )),
        Err(OfflineError::InvalidInput("name"))
    ));

    let plugins = root.join("plugins");
    fs::create_dir(&plugins).expect("plugins root");
    execute_offline(&request(
        OfflineCommand::PluginInit {
            plugins_root: plugins.clone(),
            name: "example-plugin".to_owned(),
            description: "Example plugin".to_owned(),
        },
        "plugin-1",
        ExecutionMode::Apply,
    ))
    .expect("plugin init");
    assert!(plugins.join("registry.yaml").is_file());
    assert!(plugins.join("example-plugin/plugin.json").is_file());
    assert!(matches!(
        execute_offline(&request(
            OfflineCommand::PluginInit {
                plugins_root: plugins,
                name: "example-plugin".to_owned(),
                description: "Duplicate".to_owned(),
            },
            "plugin-2",
            ExecutionMode::Apply,
        )),
        Err(OfflineError::AlreadyExists(_))
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn bump_requires_all_three_authoritative_version_occurrences() {
    let root = fixture("bump");
    fs::create_dir(root.join("source")).expect("source");
    fs::write(root.join("Cargo.toml"), "version = \"1.2.3\"\n").expect("Cargo");
    fs::write(root.join("source/SKILL.md"), "version: 1.2.3\n").expect("skill");
    fs::write(root.join("README.md"), "ae-sdd 1.2.3\n").expect("readme");
    execute_offline(&request(
        OfflineCommand::Bump {
            repository_root: root.clone(),
            expected_version: "1.2.3".to_owned(),
            new_version: "1.2.4".to_owned(),
        },
        "bump-1",
        ExecutionMode::Apply,
    ))
    .expect("bump");
    assert!(
        fs::read_to_string(root.join("Cargo.toml"))
            .expect("Cargo")
            .contains("1.2.4")
    );
    assert!(matches!(
        execute_offline(&request(
            OfflineCommand::Bump {
                repository_root: root.clone(),
                expected_version: "9.9.9".to_owned(),
                new_version: "10.0.0".to_owned(),
            },
            "bump-invalid",
            ExecutionMode::DryRun,
        )),
        Err(OfflineError::InvalidArtifact(_))
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn distributor_registry_supports_all_six_exact_commands() {
    let root = fixture("distributor");
    let target = root.join("installed");
    fs::create_dir(&target).expect("target");
    let registry = root.join("distributors.json");

    let empty = execute_offline(&request(
        OfflineCommand::DistributorList {
            registry_file: registry.clone(),
        },
        "dist-list-empty",
        ExecutionMode::DryRun,
    ))
    .expect("empty list");
    assert_eq!(
        empty.payload["entries"].as_array().expect("entries").len(),
        0
    );

    let entry = DistributorEntry {
        name: "codex".to_owned(),
        kind: "copytree".to_owned(),
        target_path: target,
        enabled: true,
    };
    execute_offline(&request(
        OfflineCommand::DistributorRegister {
            registry_file: registry.clone(),
            entry: entry.clone(),
        },
        "dist-register",
        ExecutionMode::Apply,
    ))
    .expect("register");
    assert!(matches!(
        execute_offline(&request(
            OfflineCommand::DistributorRegister {
                registry_file: registry.clone(),
                entry,
            },
            "dist-register-duplicate",
            ExecutionMode::Apply,
        )),
        Err(OfflineError::DistributorExists(_))
    ));
    execute_offline(&request(
        OfflineCommand::DistributorDisable {
            registry_file: registry.clone(),
            name: "codex".to_owned(),
        },
        "dist-disable",
        ExecutionMode::Apply,
    ))
    .expect("disable");
    execute_offline(&request(
        OfflineCommand::DistributorEnable {
            registry_file: registry.clone(),
            name: "codex".to_owned(),
        },
        "dist-enable",
        ExecutionMode::Apply,
    ))
    .expect("enable");
    let scan = execute_offline(&request(
        OfflineCommand::DistributorScan {
            registry_file: registry.clone(),
        },
        "dist-scan",
        ExecutionMode::DryRun,
    ))
    .expect("scan");
    assert_eq!(scan.payload["entries"][0]["targetExists"], true);
    execute_offline(&request(
        OfflineCommand::DistributorUnregister {
            registry_file: registry.clone(),
            name: "codex".to_owned(),
        },
        "dist-unregister",
        ExecutionMode::Apply,
    ))
    .expect("unregister");
    assert!(matches!(
        execute_offline(&request(
            OfflineCommand::DistributorUnregister {
                registry_file: registry,
                name: "codex".to_owned(),
            },
            "dist-unregister-missing",
            ExecutionMode::Apply,
        )),
        Err(OfflineError::DistributorMissing(_))
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn runtime_verify_checks_compiler_manifest_and_detects_tampering() {
    let root = fixture("runtime-verify");
    let source = root.join("source-package");
    let output = root.join("compiled-package");
    fs::create_dir(&source).expect("source package");
    fs::write(source.join("SKILL.md"), "---\nname: fixture\n---\n").expect("skill");
    execute_native_job(&NativeJobRequest {
        schema_version: "ae-sdd-native-job/v1".to_owned(),
        entrypoint: "compile".to_owned(),
        actor: "offline-test".to_owned(),
        reason: "compile verification fixture".to_owned(),
        idempotency_key: "compile-runtime-fixture".to_owned(),
        mode: ExecutionMode::Apply,
        allowed_roots: vec![root.clone()],
        job: JobInput::Compile(CompileInput {
            source_directory: source,
            output_directory: output.clone(),
            generated_configs: Vec::new(),
        }),
    })
    .expect("compile package");
    let verified = execute_offline(&request(
        OfflineCommand::RuntimeVerify {
            package_directory: output.clone(),
        },
        "runtime-verify-ok",
        ExecutionMode::DryRun,
    ))
    .expect("runtime verify");
    assert_eq!(verified.payload["pythonRuntimeFiles"], 0);

    fs::write(output.join("unlisted-extra.md"), "not in manifest\n").expect("extra file");
    assert!(matches!(
        execute_offline(&request(
            OfflineCommand::RuntimeVerify {
                package_directory: output.clone(),
            },
            "runtime-verify-extra",
            ExecutionMode::DryRun,
        )),
        Err(OfflineError::InvalidArtifact(_))
    ));
    fs::remove_file(output.join("unlisted-extra.md")).expect("remove extra file");

    fs::write(output.join("SKILL.md"), "tampered\n").expect("tamper");
    assert!(matches!(
        execute_offline(&request(
            OfflineCommand::RuntimeVerify {
                package_directory: output,
            },
            "runtime-verify-bad",
            ExecutionMode::DryRun,
        )),
        Err(OfflineError::InvalidArtifact(_))
    ));
    fs::remove_dir_all(root).expect("cleanup");
}
