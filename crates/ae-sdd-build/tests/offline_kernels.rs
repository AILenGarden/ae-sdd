use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use ae_sdd_build::{
    B_OFFLINE_ENTRYPOINTS, CompileInput, DistributorEntry, ExecutionMode, GeneratedConfig,
    JobInput, NativeJobRequest, OfflineCommand, OfflineError, OfflineRequest, PermissionClass,
    execute_native_job, execute_offline,
};
use sha2::{Digest, Sha256};

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

fn methodology_source(root: &std::path::Path) -> PathBuf {
    let repository_source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source");
    let source = root.join("methodology-source");
    fs::create_dir_all(source.join("standards/runtime")).expect("methodology source directories");
    fs::copy(repository_source.join("SKILL.md"), source.join("SKILL.md"))
        .expect("copy source entry");
    let catalog_relative = "standards/runtime/methodology-catalog.v1.json";
    let catalog = fs::read(repository_source.join(catalog_relative)).expect("production catalog");
    fs::write(source.join(catalog_relative), &catalog).expect("copy production catalog");
    let catalog: serde_json::Value = serde_json::from_slice(&catalog).expect("catalog JSON");
    for entry in catalog["entries"].as_array().expect("catalog entries") {
        for field in ["compactRef", "fallbackRef"] {
            let Some(relative) = entry[field].as_str() else {
                continue;
            };
            let destination = source.join(relative);
            fs::create_dir_all(destination.parent().expect("asset parent"))
                .expect("asset directory");
            fs::copy(repository_source.join(relative), destination).expect("copy catalog asset");
        }
    }
    source
}

fn native_package(name: &str) -> PathBuf {
    let root = fixture(name);
    fs::create_dir(root.join("runtime")).expect("runtime directory");
    let skill = b"---\nname: fixture\n---\n";
    fs::write(root.join("SKILL.md"), skill).expect("skill entry");
    fs::write(
        root.join("runtime/build-manifest.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": "ae-sdd-compiled-runtime/v1",
            "manifestKind": "content-addressed-package",
            "sourceFiles": [{
                "path": "SKILL.md",
                "digest": hex::encode(Sha256::digest(skill)),
                "permission": "PrivateFile"
            }]
        }))
        .expect("native manifest"),
    )
    .expect("write native manifest");
    root
}

fn write_native_manifest(root: &std::path::Path, manifest: &serde_json::Value) {
    fs::write(
        root.join("runtime/build-manifest.json"),
        serde_json::to_vec_pretty(manifest).expect("encode native manifest"),
    )
    .expect("write native manifest");
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
    assert_eq!(valid.payload["version"], "4.0.0");
    assert_eq!(valid.payload["runtime"], "rust");

    let mut invalid = request(OfflineCommand::Version, "version-2", ExecutionMode::DryRun);
    invalid.schema_version = "unknown".to_owned();
    assert!(matches!(
        execute_offline(&invalid),
        Err(OfflineError::Schema(_))
    ));
}

#[test]
fn offline_request_wire_round_trips_and_rejects_unknown_fields() {
    let expected = request(
        OfflineCommand::AssetsGenerate {
            project_root: PathBuf::from("C:/workspace/example"),
            project_key: "example-project".to_owned(),
        },
        "assets-wire-roundtrip",
        ExecutionMode::DryRun,
    );
    let encoded = serde_json::to_vec(&expected).expect("serialize offline request");
    let decoded: OfflineRequest =
        serde_json::from_slice(&encoded).expect("deserialize offline request");
    assert_eq!(decoded, expected);

    let mut unknown = serde_json::to_value(&expected).expect("serialize request value");
    unknown
        .as_object_mut()
        .expect("request object")
        .insert("unknownField".to_owned(), serde_json::json!(true));
    assert!(serde_json::from_value::<OfflineRequest>(unknown).is_err());
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
    assert!(hooks.contains("SessionStart"));
    assert!(hooks.contains("runtime ensure"));
    assert!(hooks.contains("hook --method hook.pre_tool --request-json -"));
    assert!(hooks.contains("hook --method hook.user_prompt --request-json -"));
    assert!(hooks.contains("hook --method hook.post_tool --request-json -"));
    assert!(hooks.contains("hook --method hook.stop --request-json -"));
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
    assert_eq!(assets.changed_paths.len(), 1);
    let asset = fs::read_to_string(root.join(".ae-sdd/assets/fixture-project.assets.md"))
        .expect("asset file");
    assert!(asset.contains("README.md"));
    assert!(asset.contains("schemaVersion: ae-sdd-project-assets/v1"));
    assert!(asset.contains("projectKey: fixture-project"));
    assert!(asset.contains("inventoryDigest:"));
    for section in ["§A", "§B", "§C", "§D", "§E", "§F", "§G"] {
        assert!(
            asset.contains(section),
            "missing canonical section {section}"
        );
    }
    assert_eq!(
        assets.payload["assetFile"],
        ".ae-sdd/assets/fixture-project.assets.md"
    );
    assert_eq!(assets.payload["schemaVersion"], "ae-sdd-project-assets/v1");
    assert_eq!(
        assets.payload["assetDigest"].as_str().map(str::len),
        Some(64)
    );
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
fn assets_generation_is_byte_deterministic_and_root_independent() {
    let left = fixture("assets-deterministic-left");
    let right = fixture("assets-deterministic-right");
    for root in [&left, &right] {
        fs::create_dir_all(root.join("src")).expect("source directory");
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"fixture\"\n").expect("manifest");
        fs::write(root.join("src/lib.rs"), "pub fn fixture() {}\n").expect("source");
        execute_offline(&request(
            OfflineCommand::AssetsGenerate {
                project_root: root.clone(),
                project_key: "deterministic-project".to_owned(),
            },
            "assets-deterministic",
            ExecutionMode::Apply,
        ))
        .expect("canonical assets");
    }

    let relative = ".ae-sdd/assets/deterministic-project.assets.md";
    let left_bytes = fs::read(left.join(relative)).expect("left asset");
    let right_bytes = fs::read(right.join(relative)).expect("right asset");
    assert_eq!(left_bytes, right_bytes);

    fs::remove_dir_all(left).expect("left cleanup");
    fs::remove_dir_all(right).expect("right cleanup");
}

#[test]
fn bump_requires_all_three_authoritative_version_occurrences() {
    let root = fixture("bump");
    fs::create_dir_all(root.join("tools/lib")).expect("tools/lib");
    fs::create_dir(root.join("source")).expect("source");
    fs::write(root.join("source/SKILL.md"), "version: 1.2.3\n").expect("skill");
    fs::write(
        root.join("tools/lib/paths.py"),
        "MASTER_VERSION = \"1.2.3\"\n",
    )
    .expect("paths");
    fs::write(root.join("README.md"), "> **版本：** v1.2.3\n").expect("readme");
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
        fs::read_to_string(root.join("source/SKILL.md"))
            .expect("skill")
            .contains("version: 1.2.4")
    );
    assert!(
        fs::read_to_string(root.join("tools/lib/paths.py"))
            .expect("paths")
            .contains("MASTER_VERSION = \"1.2.4\"")
    );
    assert!(
        fs::read_to_string(root.join("README.md"))
            .expect("readme")
            .contains("> **版本：** v1.2.4")
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
            source_directory: source.clone(),
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

#[test]
fn runtime_verify_rejects_unbounded_manifest_entry_count_before_file_reads() {
    let root = fixture("runtime-verify-manifest-budget");
    let source = root.join("source-package");
    let output = root.join("compiled-package");
    fs::create_dir(&source).expect("source package");
    fs::write(source.join("SKILL.md"), "---\nname: fixture\n---\n").expect("skill");
    execute_native_job(&NativeJobRequest {
        schema_version: "ae-sdd-native-job/v1".to_owned(),
        entrypoint: "compile".to_owned(),
        actor: "offline-test".to_owned(),
        reason: "compile manifest budget fixture".to_owned(),
        idempotency_key: "compile-runtime-manifest-budget".to_owned(),
        mode: ExecutionMode::Apply,
        allowed_roots: vec![root.clone()],
        job: JobInput::Compile(CompileInput {
            source_directory: source,
            output_directory: output.clone(),
            generated_configs: Vec::new(),
        }),
    })
    .expect("compile package");
    let manifest_path = output.join("runtime/build-manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest"))
            .expect("manifest JSON");
    manifest["sourceFiles"] = serde_json::Value::Array(
        (0..50_001)
            .map(|index| {
                serde_json::json!({
                    "path":format!("fake/{index:05}.json"),
                    "digest":"0".repeat(64),
                    "permission":"PrivateFile"
                })
            })
            .collect(),
    );
    fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest).expect("encode oversized manifest"),
    )
    .expect("write oversized manifest");

    let result = execute_offline(&request(
        OfflineCommand::RuntimeVerify {
            package_directory: output,
        },
        "runtime-verify-manifest-budget",
        ExecutionMode::DryRun,
    ));
    assert!(matches!(
        result,
        Err(OfflineError::InvalidArtifact(message)) if message.contains("budget")
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn runtime_verify_includes_generated_configs_and_rejects_reserved_manifest_collision() {
    let root = fixture("runtime-verify-generated");
    let source = root.join("source-package");
    let output = root.join("compiled-package");
    fs::create_dir(&source).expect("source package");
    fs::write(source.join("SKILL.md"), "---\nname: fixture\n---\n").expect("skill");
    execute_native_job(&NativeJobRequest {
        schema_version: "ae-sdd-native-job/v1".to_owned(),
        entrypoint: "compile".to_owned(),
        actor: "offline-test".to_owned(),
        reason: "compile package with generated config".to_owned(),
        idempotency_key: "compile-runtime-generated".to_owned(),
        mode: ExecutionMode::Apply,
        allowed_roots: vec![root.clone()],
        job: JobInput::Compile(CompileInput {
            source_directory: source.clone(),
            output_directory: output.clone(),
            generated_configs: vec![GeneratedConfig {
                schema_version: "fixture/v1".to_owned(),
                relative_path: "runtime/generated.json".to_owned(),
                contents: "{\"generated\":true}\n".to_owned(),
                permission: PermissionClass::PrivateFile,
            }],
        }),
    })
    .expect("compile generated package");
    execute_offline(&request(
        OfflineCommand::RuntimeVerify {
            package_directory: output,
        },
        "runtime-verify-generated-ok",
        ExecutionMode::DryRun,
    ))
    .expect("generated config is listed in manifest");

    let collision = root.join("collision-package");
    let result = execute_native_job(&NativeJobRequest {
        schema_version: "ae-sdd-native-job/v1".to_owned(),
        entrypoint: "compile".to_owned(),
        actor: "offline-test".to_owned(),
        reason: "reject reserved manifest collision".to_owned(),
        idempotency_key: "compile-runtime-manifest-collision".to_owned(),
        mode: ExecutionMode::Apply,
        allowed_roots: vec![root.clone()],
        job: JobInput::Compile(CompileInput {
            source_directory: source.clone(),
            output_directory: collision,
            generated_configs: vec![GeneratedConfig {
                schema_version: "fixture/v1".to_owned(),
                relative_path: "runtime/build-manifest.json".to_owned(),
                contents: "{}\n".to_owned(),
                permission: PermissionClass::PrivateFile,
            }],
        }),
    });
    assert!(result.is_err());

    for (index, relative_path) in [
        "runtime/generated:stream.json",
        "runtime/CON.json",
        "runtime/trailing.",
    ]
    .into_iter()
    .enumerate()
    {
        let result = execute_native_job(&NativeJobRequest {
            schema_version: "ae-sdd-native-job/v1".to_owned(),
            entrypoint: "compile".to_owned(),
            actor: "offline-test".to_owned(),
            reason: "reject non-portable generated path".to_owned(),
            idempotency_key: format!("compile-runtime-invalid-path-{index}"),
            mode: ExecutionMode::DryRun,
            allowed_roots: vec![root.clone()],
            job: JobInput::Compile(CompileInput {
                source_directory: source.clone(),
                output_directory: root.join(format!("invalid-path-{index}")),
                generated_configs: vec![GeneratedConfig {
                    schema_version: "fixture/v1".to_owned(),
                    relative_path: relative_path.to_owned(),
                    contents: "{}\n".to_owned(),
                    permission: PermissionClass::PrivateFile,
                }],
            }),
        });
        assert!(result.is_err(), "accepted non-portable {relative_path}");
    }
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn compiler_emits_byte_stable_methodology_bundle_and_offline_verifier_accepts_it() {
    let root = fixture("methodology-bundle");
    let source = methodology_source(&root);
    let first = root.join("compiled-first");
    let second = root.join("compiled-second");

    for (output, idempotency_key) in [
        (&first, "compile-methodology-first"),
        (&second, "compile-methodology-second"),
    ] {
        execute_native_job(&NativeJobRequest {
            schema_version: "ae-sdd-native-job/v1".to_owned(),
            entrypoint: "compile".to_owned(),
            actor: "offline-test".to_owned(),
            reason: "compile deterministic Methodology bundle".to_owned(),
            idempotency_key: idempotency_key.to_owned(),
            mode: ExecutionMode::Apply,
            allowed_roots: vec![root.clone()],
            job: JobInput::Compile(CompileInput {
                source_directory: source.clone(),
                output_directory: output.clone(),
                generated_configs: Vec::new(),
            }),
        })
        .expect("compile Methodology source");
    }

    let relative = "runtime/methodology/catalog.v1.json";
    let first_bundle = fs::read(first.join(relative)).expect("first Methodology bundle");
    let second_bundle = fs::read(second.join(relative)).expect("second Methodology bundle");
    assert_eq!(first_bundle, second_bundle);
    let bundle: serde_json::Value = serde_json::from_slice(&first_bundle).expect("bundle JSON");
    assert_eq!(bundle["schemaVersion"], "ae-sdd-methodology-bundle/v1");
    assert_eq!(bundle["entries"].as_array().map(Vec::len), Some(31));
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(first.join("runtime/build-manifest.json")).expect("build manifest"),
    )
    .expect("build manifest JSON");
    assert_eq!(
        manifest["methodology"]["schemaVersion"],
        "ae-sdd-methodology-manifest/v1"
    );
    assert_eq!(manifest["methodology"]["entryCount"], 31);
    assert_eq!(
        manifest["methodology"]["entries"].as_array().map(Vec::len),
        Some(31)
    );
    assert_eq!(
        manifest["methodology"]["catalogDigest"],
        bundle["catalogDigest"]
    );

    let verified = execute_offline(&request(
        OfflineCommand::RuntimeVerify {
            package_directory: first,
        },
        "verify-methodology-bundle",
        ExecutionMode::DryRun,
    ))
    .expect("verify Methodology package");
    assert_eq!(verified.payload["methodologyEntries"], 31);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn offline_verifier_accepts_legacy_native_manifest_without_methodology_extension() {
    let root = fixture("legacy-native-methodology");
    let source = methodology_source(&root);
    let output = root.join("compiled");
    execute_native_job(&NativeJobRequest {
        schema_version: "ae-sdd-native-job/v1".to_owned(),
        entrypoint: "compile".to_owned(),
        actor: "offline-test".to_owned(),
        reason: "create legacy native compatibility fixture".to_owned(),
        idempotency_key: "compile-legacy-native-methodology".to_owned(),
        mode: ExecutionMode::Apply,
        allowed_roots: vec![root.clone()],
        job: JobInput::Compile(CompileInput {
            source_directory: source,
            output_directory: output.clone(),
            generated_configs: Vec::new(),
        }),
    })
    .expect("compile compatibility fixture");

    let bundle_relative = "runtime/methodology/catalog.v1.json";
    fs::remove_file(output.join(bundle_relative)).expect("remove future bundle");
    let manifest_path = output.join("runtime/build-manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest"))
            .expect("manifest JSON");
    manifest
        .as_object_mut()
        .expect("manifest object")
        .remove("methodology");
    manifest["sourceFiles"]
        .as_array_mut()
        .expect("manifest entries")
        .retain(|entry| entry["path"] != bundle_relative);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("encode legacy manifest"),
    )
    .expect("write legacy manifest");

    let verified = execute_offline(&request(
        OfflineCommand::RuntimeVerify {
            package_directory: output,
        },
        "verify-legacy-native-methodology",
        ExecutionMode::DryRun,
    ))
    .expect("legacy native manifest remains accepted");
    assert_eq!(verified.payload["methodologyEntries"], 0);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn offline_verifier_rejects_methodology_tampering_even_when_package_digest_is_rewritten() {
    let root = fixture("methodology-tamper");
    let source = methodology_source(&root);
    let output = root.join("compiled");
    execute_native_job(&NativeJobRequest {
        schema_version: "ae-sdd-native-job/v1".to_owned(),
        entrypoint: "compile".to_owned(),
        actor: "offline-test".to_owned(),
        reason: "compile Methodology tamper fixture".to_owned(),
        idempotency_key: "compile-methodology-tamper".to_owned(),
        mode: ExecutionMode::Apply,
        allowed_roots: vec![root.clone()],
        job: JobInput::Compile(CompileInput {
            source_directory: source,
            output_directory: output.clone(),
            generated_configs: Vec::new(),
        }),
    })
    .expect("compile Methodology source");

    let bundle: serde_json::Value = serde_json::from_slice(
        &fs::read(output.join("runtime/methodology/catalog.v1.json")).expect("bundle"),
    )
    .expect("bundle JSON");
    let artifact = bundle["entries"][0]["compactRef"]["path"]
        .as_str()
        .expect("compact path");
    let artifact_path = output.join(artifact);
    let mut tampered = fs::read(&artifact_path).expect("compact artifact");
    tampered.extend_from_slice(b"\ntampered\n");
    fs::write(&artifact_path, &tampered).expect("tamper compact artifact");

    let manifest_path = output.join("runtime/build-manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest"))
            .expect("manifest JSON");
    let entry = manifest["sourceFiles"]
        .as_array_mut()
        .expect("manifest entries")
        .iter_mut()
        .find(|entry| entry["path"] == artifact)
        .expect("artifact manifest entry");
    entry["digest"] = serde_json::json!(hex::encode(Sha256::digest(&tampered)));
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("encode manifest"),
    )
    .expect("rewrite outer digest");

    let result = execute_offline(&request(
        OfflineCommand::RuntimeVerify {
            package_directory: output,
        },
        "verify-methodology-tamper",
        ExecutionMode::DryRun,
    ));
    assert!(matches!(result, Err(OfflineError::InvalidArtifact(_))));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn offline_request_identity_and_required_wire_fields_fail_closed() {
    for invalid in ["", "line\nbreak", "nul\0byte"] {
        let mut candidate = request(
            OfflineCommand::Version,
            "identity-negative",
            ExecutionMode::DryRun,
        );
        candidate.actor = invalid.to_owned();
        assert!(matches!(
            execute_offline(&candidate),
            Err(OfflineError::InvalidInput("request identity"))
        ));
    }

    let mut oversized = request(
        OfflineCommand::Version,
        "identity-oversized",
        ExecutionMode::DryRun,
    );
    oversized.reason = "r".repeat(1_025);
    assert!(matches!(
        execute_offline(&oversized),
        Err(OfflineError::InvalidInput("request identity"))
    ));

    let encoded = serde_json::to_value(request(
        OfflineCommand::Version,
        "wire-required",
        ExecutionMode::DryRun,
    ))
    .expect("request value");
    for field in ["schemaVersion", "mode", "actor", "reason", "idempotencyKey"] {
        let mut missing = encoded.clone();
        missing
            .as_object_mut()
            .expect("request object")
            .remove(field);
        assert!(
            serde_json::from_value::<OfflineRequest>(missing).is_err(),
            "accepted missing {field}"
        );
    }
    let mut invalid_mode = encoded;
    invalid_mode["mode"] = serde_json::json!(42);
    assert!(serde_json::from_value::<OfflineRequest>(invalid_mode).is_err());
}

#[test]
fn offline_input_boundaries_cover_bootstrap_and_distributor_failures() {
    let root = fixture("offline-boundaries");
    let plugin_root = root.join("plugins");
    fs::create_dir(&plugin_root).expect("plugin root");

    for (executable, hosts, field) in [
        ("", vec!["codex".to_owned()], "executable"),
        ("ae-sdd", Vec::new(), "hosts"),
        ("ae-sdd", vec!["codex".to_owned(); 6], "hosts"),
    ] {
        let result = execute_offline(&request(
            OfflineCommand::InitHooks {
                project_root: root.clone(),
                executable: executable.to_owned(),
                hosts,
            },
            &format!("hooks-{field}"),
            ExecutionMode::DryRun,
        ));
        assert!(matches!(result, Err(OfflineError::InvalidInput(actual)) if actual == field));
    }

    for description in ["", "line\nbreak"] {
        assert!(matches!(
            execute_offline(&request(
                OfflineCommand::PluginInit {
                    plugins_root: plugin_root.clone(),
                    name: "bounded-plugin".to_owned(),
                    description: description.to_owned(),
                },
                "plugin-description-negative",
                ExecutionMode::DryRun,
            )),
            Err(OfflineError::InvalidInput("description"))
        ));
    }

    for (expected, new) in [("1.0", "2.0.0"), ("1.0.0", "1.0.0")] {
        assert!(matches!(
            execute_offline(&request(
                OfflineCommand::Bump {
                    repository_root: root.clone(),
                    expected_version: expected.to_owned(),
                    new_version: new.to_owned(),
                },
                "bump-boundary",
                ExecutionMode::DryRun,
            )),
            Err(OfflineError::InvalidInput(_))
        ));
    }

    let registry = root.join("registry.json");
    for entry in [
        DistributorEntry {
            name: "invalid-kind".to_owned(),
            kind: "shell".to_owned(),
            target_path: root.clone(),
            enabled: true,
        },
        DistributorEntry {
            name: "empty-target".to_owned(),
            kind: "native".to_owned(),
            target_path: PathBuf::new(),
            enabled: true,
        },
    ] {
        assert!(matches!(
            execute_offline(&request(
                OfflineCommand::DistributorRegister {
                    registry_file: registry.clone(),
                    entry,
                },
                "distributor-boundary",
                ExecutionMode::DryRun,
            )),
            Err(OfflineError::InvalidInput(_))
        ));
    }
    assert!(matches!(
        execute_offline(&request(
            OfflineCommand::DistributorEnable {
                registry_file: registry.clone(),
                name: "missing".to_owned(),
            },
            "distributor-missing-enable",
            ExecutionMode::DryRun,
        )),
        Err(OfflineError::DistributorMissing(_))
    ));

    fs::write(&registry, r#"{"schemaVersion":"wrong","entries":[]}"#).expect("invalid registry");
    assert!(matches!(
        execute_offline(&request(
            OfflineCommand::DistributorList {
                registry_file: registry,
            },
            "distributor-schema",
            ExecutionMode::DryRun,
        )),
        Err(OfflineError::InvalidArtifact(_))
    ));

    let file_root = root.join("not-a-directory");
    fs::write(&file_root, b"file").expect("file root");
    assert!(matches!(
        execute_offline(&request(
            OfflineCommand::AssetsGenerate {
                project_root: file_root,
                project_key: "file-root".to_owned(),
            },
            "asset-file-root",
            ExecutionMode::DryRun,
        )),
        Err(OfflineError::InvalidArtifact(_))
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn runtime_verifier_rejects_manifest_and_inventory_boundary_violations() {
    let missing_skill = fixture("verify-missing-skill");
    assert!(matches!(
        execute_offline(&request(
            OfflineCommand::RuntimeVerify {
                package_directory: missing_skill.clone(),
            },
            "verify-missing-skill",
            ExecutionMode::DryRun,
        )),
        Err(OfflineError::InvalidArtifact(_))
    ));
    fs::remove_dir_all(missing_skill).expect("missing skill cleanup");

    for (name, mutation) in [
        (
            "schema",
            serde_json::json!({
                "schemaVersion": "wrong",
                "manifestKind": "content-addressed-package",
                "sourceFiles": [{
                    "path": "SKILL.md",
                    "digest": "0".repeat(64),
                    "permission": "PrivateFile"
                }]
            }),
        ),
        (
            "empty",
            serde_json::json!({
                "schemaVersion": "ae-sdd-compiled-runtime/v1",
                "manifestKind": "content-addressed-package",
                "sourceFiles": []
            }),
        ),
        (
            "unsafe-path",
            serde_json::json!({
                "schemaVersion": "ae-sdd-compiled-runtime/v1",
                "manifestKind": "content-addressed-package",
                "sourceFiles": [{
                    "path": "../escape",
                    "digest": "0".repeat(64),
                    "permission": "PrivateFile"
                }]
            }),
        ),
        (
            "python",
            serde_json::json!({
                "schemaVersion": "ae-sdd-compiled-runtime/v1",
                "manifestKind": "content-addressed-package",
                "sourceFiles": [{
                    "path": "payload.py",
                    "digest": "0".repeat(64),
                    "permission": "PrivateFile"
                }]
            }),
        ),
    ] {
        let root = native_package(&format!("verify-{name}"));
        write_native_manifest(&root, &mutation);
        assert!(matches!(
            execute_offline(&request(
                OfflineCommand::RuntimeVerify {
                    package_directory: root.clone(),
                },
                &format!("verify-{name}"),
                ExecutionMode::DryRun,
            )),
            Err(OfflineError::InvalidArtifact(_))
        ));
        fs::remove_dir_all(root).expect("manifest mutation cleanup");
    }

    let directory_entry = native_package("verify-directory-entry");
    fs::create_dir(directory_entry.join("payload")).expect("payload directory");
    let skill_digest = hex::encode(Sha256::digest(
        fs::read(directory_entry.join("SKILL.md")).expect("skill"),
    ));
    write_native_manifest(
        &directory_entry,
        &serde_json::json!({
            "schemaVersion": "ae-sdd-compiled-runtime/v1",
            "manifestKind": "content-addressed-package",
            "sourceFiles": [
                {"path":"SKILL.md","digest":skill_digest,"permission":"PrivateFile"},
                {"path":"payload","digest":"0".repeat(64),"permission":"PrivateFile"}
            ]
        }),
    );
    assert!(matches!(
        execute_offline(&request(
            OfflineCommand::RuntimeVerify {
                package_directory: directory_entry.clone(),
            },
            "verify-directory-entry",
            ExecutionMode::DryRun,
        )),
        Err(OfflineError::InvalidArtifact(_))
    ));
    fs::remove_dir_all(directory_entry).expect("directory entry cleanup");

    for (name, relative) in [
        ("python-inventory", "rogue.py"),
        ("extra-inventory", "extra.txt"),
    ] {
        let root = native_package(name);
        fs::write(root.join(relative), b"extra").expect("extra inventory file");
        assert!(matches!(
            execute_offline(&request(
                OfflineCommand::RuntimeVerify {
                    package_directory: root.clone(),
                },
                name,
                ExecutionMode::DryRun,
            )),
            Err(OfflineError::InvalidArtifact(_))
        ));
        fs::remove_dir_all(root).expect("inventory cleanup");
    }
}

#[test]
fn build_cli_offline_modes_are_typed_and_fail_closed() {
    let root = fixture("offline-cli");
    let request_path = root.join("version.json");
    fs::write(
        &request_path,
        serde_json::to_vec_pretty(&request(
            OfflineCommand::Version,
            "offline-cli",
            ExecutionMode::DryRun,
        ))
        .expect("request JSON"),
    )
    .expect("write request");

    let text = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"))
        .args(["offline", "--request"])
        .arg(&request_path)
        .output()
        .expect("offline text CLI");
    assert!(text.status.success());
    assert!(String::from_utf8_lossy(&text.stdout).contains("offline version planned: changed=0"));

    let json = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"))
        .args(["offline", "--request"])
        .arg(&request_path)
        .arg("--json")
        .output()
        .expect("offline JSON CLI");
    assert!(json.status.success());
    let payload: serde_json::Value =
        serde_json::from_slice(&json.stdout).expect("offline result JSON");
    assert_eq!(payload["command"], "version");

    let mut invalid = request(
        OfflineCommand::Version,
        "offline-cli-invalid",
        ExecutionMode::DryRun,
    );
    invalid.schema_version = "wrong".to_owned();
    fs::write(
        &request_path,
        serde_json::to_vec(&invalid).expect("invalid request JSON"),
    )
    .expect("rewrite request");
    let rejected = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"))
        .args(["offline", "--request"])
        .arg(&request_path)
        .output()
        .expect("offline rejected CLI");
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("unsupported offline request schema")
    );
    fs::remove_dir_all(root).expect("cleanup");
}
