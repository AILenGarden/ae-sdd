use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ae_sdd_build::{
    SERVICE_EXECUTION_SCHEMA, SERVICE_PLAN_SCHEMA, ServiceDescriptorAction, ServiceDescriptorState,
    ServiceError, ServiceExecutionLimits, ServiceLifecycleRequest, ServiceManagerCommand,
    ServiceManagerOutput, ServiceManagerRunner, ServiceOperation, ServicePlatform,
    execute_service_lifecycle, execute_service_lifecycle_with_runner,
    generate_service_lifecycle_plan, inspect_service_descriptor, materialize_service_descriptor,
};

fn fixture_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ae-sdd-service-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&root).expect("fixture root");
    root
}

fn request(
    root: &Path,
    platform: ServicePlatform,
    operation: ServiceOperation,
) -> ServiceLifecycleRequest {
    let executable = root.join(if platform == ServicePlatform::Windows {
        "bin/ae-sddd.exe"
    } else {
        "bin/ae-sddd"
    });
    let workspace = root.join("workspace with space");
    fs::create_dir_all(executable.parent().expect("binary parent")).expect("binary parent");
    fs::create_dir_all(&workspace).expect("workspace");
    fs::write(&executable, b"native rust daemon").expect("binary");
    ServiceLifecycleRequest::new(
        platform,
        operation,
        executable,
        root.join("runtime"),
        workspace.clone(),
        vec![workspace],
        root.to_path_buf(),
        match platform {
            ServicePlatform::Windows => {
                let user = std::env::var("USERNAME").unwrap_or_else(|_| "runner".to_owned());
                std::env::var("USERDOMAIN")
                    .map_or(user.clone(), |domain| format!("{domain}\\{user}"))
            }
            ServicePlatform::Macos => "501".to_owned(),
            ServicePlatform::Linux => "1000".to_owned(),
        },
    )
}

#[derive(Default)]
struct FakeRunner {
    calls: Mutex<Vec<ServiceManagerCommand>>,
    outputs: Mutex<VecDeque<ServiceManagerOutput>>,
    descriptor_required: Option<PathBuf>,
}

impl FakeRunner {
    fn with_outputs(outputs: Vec<ServiceManagerOutput>) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            outputs: Mutex::new(outputs.into()),
            descriptor_required: None,
        }
    }

    fn requiring_descriptor(path: PathBuf) -> Self {
        Self {
            descriptor_required: Some(path),
            ..Self::default()
        }
    }

    fn calls(&self) -> Vec<ServiceManagerCommand> {
        self.calls.lock().expect("fake calls lock").clone()
    }
}

impl ServiceManagerRunner for FakeRunner {
    fn run(
        &self,
        command: &ServiceManagerCommand,
        _limits: ServiceExecutionLimits,
    ) -> Result<ServiceManagerOutput, ServiceError> {
        if let Some(path) = &self.descriptor_required {
            assert!(path.is_file(), "descriptor must exist before manager call");
        }
        self.calls
            .lock()
            .expect("fake calls lock")
            .push(command.clone());
        Ok(self
            .outputs
            .lock()
            .expect("fake outputs lock")
            .pop_front()
            .unwrap_or_else(success_output))
    }
}

fn success_output() -> ServiceManagerOutput {
    ServiceManagerOutput {
        exit_code: Some(0),
        stdout: b"manager-ok".to_vec(),
        stderr: Vec::new(),
        timed_out: false,
        elapsed_millis: 1,
    }
}

#[test]
fn all_platform_plans_are_user_scoped_shell_free_and_secret_free() {
    let root = fixture_root("platforms");
    for platform in [
        ServicePlatform::Windows,
        ServicePlatform::Macos,
        ServicePlatform::Linux,
    ] {
        let plan =
            generate_service_lifecycle_plan(&request(&root, platform, ServiceOperation::Install))
                .expect("service plan");
        assert_eq!(plan.schema_version, SERVICE_PLAN_SCHEMA);
        assert!(plan.permission_policy.user_scope_only);
        assert!(!plan.permission_policy.elevation_required);
        assert!(!plan.lifecycle_contract.secrets_embedded);
        assert!(!plan.lifecycle_contract.shell_wrapper);
        assert!(plan.lifecycle_contract.state_retained_on_uninstall);
        assert_eq!(plan.lifecycle_contract.daemon_argv[1], "serve");
        assert!(
            plan.lifecycle_contract
                .daemon_argv
                .iter()
                .any(|argument| argument == "--state-dir")
        );
        assert!(
            plan.lifecycle_contract
                .daemon_argv
                .iter()
                .any(|argument| argument == "--allowed-root")
        );
        for command in &plan.manager_commands {
            assert!(!matches!(
                command.program,
                "sh" | "bash" | "cmd.exe" | "powershell.exe"
            ));
        }
        let normalized = plan.descriptor_contents.to_ascii_lowercase();
        assert!(!normalized.contains("python"));
        assert!(!normalized.contains("endpoint_token"));
        assert!(!normalized.contains("private_key"));
    }
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn platform_descriptors_encode_native_user_service_security() {
    let root = fixture_root("descriptors");

    let windows = generate_service_lifecycle_plan(&request(
        &root,
        ServicePlatform::Windows,
        ServiceOperation::Install,
    ))
    .expect("Windows plan");
    assert!(windows.descriptor_path.ends_with("ae-sdd-daemon.xml"));
    assert!(
        windows
            .descriptor_contents
            .contains("<LogonType>InteractiveToken</LogonType>")
    );
    assert!(
        windows
            .descriptor_contents
            .contains("<RunLevel>LeastPrivilege</RunLevel>")
    );
    assert!(
        windows
            .descriptor_contents
            .contains("<MultipleInstancesPolicy>IgnoreNew")
    );
    assert!(windows.descriptor_contents.contains("&quot;"));
    assert_eq!(windows.manager_commands[0].program, "schtasks.exe");
    assert!(
        !windows.manager_commands[0]
            .arguments
            .iter()
            .any(|value| value == "/RU")
    );

    let macos = generate_service_lifecycle_plan(&request(
        &root,
        ServicePlatform::Macos,
        ServiceOperation::Install,
    ))
    .expect("macOS plan");
    assert!(macos.descriptor_path.ends_with("com.ae-sdd.daemon.plist"));
    assert!(
        macos
            .descriptor_contents
            .contains("<key>ProgramArguments</key>")
    );
    assert!(macos.descriptor_contents.contains("<key>Umask</key>"));
    assert!(macos.descriptor_contents.contains("<integer>63</integer>"));
    assert_eq!(macos.manager_commands[0].arguments[1], "gui/501");

    let linux = generate_service_lifecycle_plan(&request(
        &root,
        ServicePlatform::Linux,
        ServiceOperation::Install,
    ))
    .expect("Linux plan");
    assert!(linux.descriptor_path.ends_with("ae-sdd.service"));
    assert!(linux.descriptor_contents.contains("UMask=0077"));
    assert!(linux.descriptor_contents.contains("NoNewPrivileges=true"));
    assert!(linux.descriptor_contents.contains("Restart=on-failure"));
    assert_eq!(
        linux.manager_commands[0].arguments,
        ["--user", "daemon-reload"]
    );

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn operation_plans_cover_install_uninstall_and_status_without_privilege_escalation() {
    let root = fixture_root("operations");
    for platform in [
        ServicePlatform::Windows,
        ServicePlatform::Macos,
        ServicePlatform::Linux,
    ] {
        for operation in [
            ServiceOperation::Install,
            ServiceOperation::Uninstall,
            ServiceOperation::Status,
        ] {
            let plan = generate_service_lifecycle_plan(&request(&root, platform, operation))
                .expect("operation plan");
            assert!(!plan.manager_commands.is_empty());
            assert!(
                plan.manager_commands
                    .iter()
                    .all(|command| !command.arguments.iter().any(|value| {
                        matches!(
                            value.as_str(),
                            "root" | "SYSTEM" | "/RL" | "HIGHEST" | "sudo"
                        )
                    }))
            );
        }
    }
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn materialization_is_private_idempotent_and_detects_drift() {
    let root = fixture_root("materialize");
    let plan = generate_service_lifecycle_plan(&request(
        &root,
        ServicePlatform::current(),
        ServiceOperation::Install,
    ))
    .expect("current platform plan");

    let first = materialize_service_descriptor(&plan).expect("first materialization");
    assert!(first.created);
    assert!(first.permission_assertions.iter().all(|value| value.passed));
    let replay = materialize_service_descriptor(&plan).expect("idempotent replay");
    assert!(!replay.created);

    let status = inspect_service_descriptor(&plan).expect("matching status");
    assert_eq!(status.state, ServiceDescriptorState::Matches);
    fs::write(&plan.descriptor_path, b"drift").expect("inject drift");
    let status = inspect_service_descriptor(&plan).expect("drift status");
    assert_eq!(status.state, ServiceDescriptorState::Drifted);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn descriptor_rejects_secret_bearing_environment_and_non_absolute_paths() {
    let root = fixture_root("negative");
    let mut secret = request(&root, ServicePlatform::Linux, ServiceOperation::Install);
    secret
        .environment
        .insert("ENDPOINT_TOKEN".to_owned(), "must-not-ship".to_owned());
    assert!(matches!(
        generate_service_lifecycle_plan(&secret),
        Err(ServiceError::SecretInDescriptor)
    ));

    let mut relative = request(&root, ServicePlatform::Linux, ServiceOperation::Install);
    relative.executable = PathBuf::from("target/release/ae-sddd");
    assert!(matches!(
        generate_service_lifecycle_plan(&relative),
        Err(ServiceError::InvalidPath("executable"))
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn fake_runner_executes_install_before_replaying_without_side_effects() {
    let root = fixture_root("execute-install");
    let plan = generate_service_lifecycle_plan(&request(
        &root,
        ServicePlatform::current(),
        ServiceOperation::Install,
    ))
    .expect("install plan");
    let runner = FakeRunner::requiring_descriptor(plan.descriptor_path.clone());
    let limits = ServiceExecutionLimits::default();

    let first = execute_service_lifecycle_with_runner(&plan, &runner, limits)
        .expect("first install execution");
    assert_eq!(first.schema_version, SERVICE_EXECUTION_SCHEMA);
    assert_eq!(
        first.descriptor_action,
        ServiceDescriptorAction::Materialized
    );
    assert_eq!(runner.calls(), plan.manager_commands);

    let replay =
        execute_service_lifecycle_with_runner(&plan, &runner, limits).expect("install replay");
    assert!(replay.replayed);
    assert_eq!(replay.descriptor_action, ServiceDescriptorAction::Replayed);
    assert_eq!(runner.calls().len(), plan.manager_commands.len());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn fake_runner_uninstalls_before_safe_descriptor_removal_and_replays() {
    let root = fixture_root("execute-uninstall");
    let install = generate_service_lifecycle_plan(&request(
        &root,
        ServicePlatform::current(),
        ServiceOperation::Install,
    ))
    .expect("install plan");
    execute_service_lifecycle_with_runner(
        &install,
        &FakeRunner::requiring_descriptor(install.descriptor_path.clone()),
        ServiceExecutionLimits::default(),
    )
    .expect("install fixture service");
    let uninstall = generate_service_lifecycle_plan(&request(
        &root,
        ServicePlatform::current(),
        ServiceOperation::Uninstall,
    ))
    .expect("uninstall plan");
    let runner = FakeRunner::requiring_descriptor(uninstall.descriptor_path.clone());

    let first = execute_service_lifecycle_with_runner(
        &uninstall,
        &runner,
        ServiceExecutionLimits::default(),
    )
    .expect("uninstall execution");
    assert_eq!(first.descriptor_action, ServiceDescriptorAction::Removed);
    assert!(!uninstall.descriptor_path.exists());
    let call_count = runner.calls().len();
    let replay = execute_service_lifecycle_with_runner(
        &uninstall,
        &runner,
        ServiceExecutionLimits::default(),
    )
    .expect("uninstall replay");
    assert!(replay.replayed);
    assert_eq!(runner.calls().len(), call_count);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn status_execution_is_read_only_and_is_never_cached() {
    let root = fixture_root("execute-status");
    let plan = generate_service_lifecycle_plan(&request(
        &root,
        ServicePlatform::current(),
        ServiceOperation::Status,
    ))
    .expect("status plan");
    let runner = FakeRunner::default();

    for _ in 0..2 {
        let receipt = execute_service_lifecycle_with_runner(
            &plan,
            &runner,
            ServiceExecutionLimits::default(),
        )
        .expect("status execution");
        assert!(!receipt.replayed);
        assert_eq!(receipt.descriptor_action, ServiceDescriptorAction::None);
    }
    assert_eq!(runner.calls().len(), plan.manager_commands.len() * 2);
    assert!(!plan.state_dir.exists());
    assert!(!plan.descriptor_path.exists());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn manager_failures_and_timeouts_are_bounded_and_do_not_commit() {
    let root = fixture_root("execute-errors");
    let plan = generate_service_lifecycle_plan(&request(
        &root,
        ServicePlatform::current(),
        ServiceOperation::Install,
    ))
    .expect("install plan");
    let failed = ServiceManagerOutput {
        exit_code: Some(42),
        stderr: vec![b'e'; 64],
        ..success_output()
    };
    let limits = ServiceExecutionLimits {
        command_timeout: Duration::from_millis(50),
        max_output_bytes: 8,
    };
    let error = execute_service_lifecycle_with_runner(
        &plan,
        &FakeRunner::with_outputs(vec![failed]),
        limits,
    )
    .expect_err("non-zero manager exit must fail");
    assert!(matches!(error, ServiceError::ManagerFailed { ref stderr, .. } if stderr.len() == 8));

    let timed_out = ServiceManagerOutput {
        timed_out: true,
        stderr: vec![b't'; 64],
        ..success_output()
    };
    let error = execute_service_lifecycle_with_runner(
        &plan,
        &FakeRunner::with_outputs(vec![timed_out]),
        limits,
    )
    .expect_err("manager timeout must fail");
    assert!(matches!(error, ServiceError::ManagerTimedOut { ref stderr, .. } if stderr.len() == 8));

    execute_service_lifecycle_with_runner(&plan, &FakeRunner::default(), limits)
        .expect("failed execution remains retryable");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn executor_rejects_non_allowlisted_or_elevated_manager_plans() {
    let root = fixture_root("execute-deny");
    let mut plan = generate_service_lifecycle_plan(&request(
        &root,
        ServicePlatform::current(),
        ServiceOperation::Install,
    ))
    .expect("install plan");
    plan.manager_commands[0].program = "powershell.exe";
    assert!(matches!(
        execute_service_lifecycle_with_runner(
            &plan,
            &FakeRunner::default(),
            ServiceExecutionLimits::default()
        ),
        Err(ServiceError::ManagerProgramDenied(_))
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn build_cli_emits_the_typed_service_plan() {
    let root = fixture_root("cli");
    let request = request(&root, ServicePlatform::current(), ServiceOperation::Status);
    let request_path = root.join("service-request.json");
    fs::write(
        &request_path,
        serde_json::to_vec_pretty(&request).expect("request JSON"),
    )
    .expect("write request");
    let output = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"))
        .args(["service", "--request"])
        .arg(&request_path)
        .args(["--json"])
        .output()
        .expect("run build CLI");
    assert!(output.status.success());
    let plan: serde_json::Value = serde_json::from_slice(&output.stdout).expect("plan JSON");
    assert_eq!(plan["schemaVersion"], SERVICE_PLAN_SCHEMA);
    assert_eq!(plan["operation"], "status");
    assert_eq!(plan["platform"], ServicePlatform::current().as_str());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn request_validation_rejects_bounded_fields_and_runtime_fallbacks() {
    let root = fixture_root("request-validation");
    let baseline = request(&root, ServicePlatform::Linux, ServiceOperation::Install);

    let mut invalid = baseline.clone();
    invalid.schema_version = "wrong".to_owned();
    assert!(matches!(
        generate_service_lifecycle_plan(&invalid),
        Err(ServiceError::Schema(_))
    ));

    for roots in [Vec::new(), vec![root.clone(); 65]] {
        let mut invalid = baseline.clone();
        invalid.allowed_roots = roots;
        assert!(matches!(
            generate_service_lifecycle_plan(&invalid),
            Err(ServiceError::InvalidField("allowedRoots"))
        ));
    }

    for restart_delay_seconds in [0, 301] {
        let mut invalid = baseline.clone();
        invalid.restart_delay_seconds = restart_delay_seconds;
        assert!(matches!(
            generate_service_lifecycle_plan(&invalid),
            Err(ServiceError::InvalidRestartDelay)
        ));
    }

    let mut invalid = baseline.clone();
    invalid.user_identity.clear();
    assert!(matches!(
        generate_service_lifecycle_plan(&invalid),
        Err(ServiceError::InvalidField("userIdentity"))
    ));

    for argument in ["line\nbreak", "--endpoint-token=secret"] {
        let mut invalid = baseline.clone();
        invalid.extra_arguments = vec![argument.to_owned()];
        assert!(matches!(
            generate_service_lifecycle_plan(&invalid),
            Err(ServiceError::InvalidField("extraArguments"))
                | Err(ServiceError::SecretInDescriptor)
        ));
    }

    for (key, value, expected_key_error) in [
        ("9INVALID", "value", true),
        ("SAFE_NAME", "token=secret", false),
        ("PRIVATE_KEY", "value", false),
    ] {
        let mut invalid = baseline.clone();
        invalid.environment.insert(key.to_owned(), value.to_owned());
        let result = generate_service_lifecycle_plan(&invalid);
        if expected_key_error {
            assert!(matches!(
                result,
                Err(ServiceError::InvalidEnvironmentKey(_))
            ));
        } else {
            assert!(matches!(result, Err(ServiceError::SecretInDescriptor)));
        }
    }

    let mut invalid = baseline.clone();
    invalid.executable = root.join("bin/../ae-sddd");
    assert!(matches!(
        generate_service_lifecycle_plan(&invalid),
        Err(ServiceError::InvalidPath("executable"))
    ));

    let mut forbidden = baseline;
    forbidden.executable = root.join("bin/python.exe");
    assert!(matches!(
        generate_service_lifecycle_plan(&forbidden),
        Err(ServiceError::SecretInDescriptor)
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn executor_rejects_invalid_limits_plans_and_storage_before_running() {
    let root = fixture_root("executor-validation");
    let plan = generate_service_lifecycle_plan(&request(
        &root,
        ServicePlatform::current(),
        ServiceOperation::Status,
    ))
    .expect("status plan");
    let runner = FakeRunner::default();

    for limits in [
        ServiceExecutionLimits {
            command_timeout: Duration::ZERO,
            max_output_bytes: 1,
        },
        ServiceExecutionLimits {
            command_timeout: Duration::from_secs(301),
            max_output_bytes: 1,
        },
        ServiceExecutionLimits {
            command_timeout: Duration::from_secs(1),
            max_output_bytes: 0,
        },
        ServiceExecutionLimits {
            command_timeout: Duration::from_secs(1),
            max_output_bytes: 1024 * 1024 + 1,
        },
    ] {
        assert!(matches!(
            execute_service_lifecycle_with_runner(&plan, &runner, limits),
            Err(ServiceError::InvalidExecutionLimits)
        ));
    }

    let mut wrong_platform = plan.clone();
    wrong_platform.platform = match ServicePlatform::current() {
        ServicePlatform::Windows => ServicePlatform::Linux,
        ServicePlatform::Macos | ServicePlatform::Linux => ServicePlatform::Windows,
    };
    assert!(matches!(
        execute_service_lifecycle_with_runner(
            &wrong_platform,
            &runner,
            ServiceExecutionLimits::default()
        ),
        Err(ServiceError::PlatformMismatch { .. })
    ));

    for mutation in 0..4 {
        let mut denied = plan.clone();
        match mutation {
            0 => denied.permission_policy.user_scope_only = false,
            1 => denied.permission_policy.elevation_required = true,
            2 => denied.lifecycle_contract.shell_wrapper = true,
            _ => denied.manager_commands.clear(),
        }
        assert!(matches!(
            execute_service_lifecycle_with_runner(
                &denied,
                &runner,
                ServiceExecutionLimits::default()
            ),
            Err(ServiceError::PrivilegeEscalation)
        ));
    }

    let mut invalid_argument = plan.clone();
    invalid_argument.manager_commands[0]
        .arguments
        .push("line\nbreak".to_owned());
    assert!(matches!(
        execute_service_lifecycle_with_runner(
            &invalid_argument,
            &runner,
            ServiceExecutionLimits::default()
        ),
        Err(ServiceError::InvalidManagerArguments)
    ));

    let mut elevated = plan.clone();
    elevated.manager_commands[0]
        .arguments
        .push("sudo".to_owned());
    assert!(matches!(
        execute_service_lifecycle_with_runner(
            &elevated,
            &runner,
            ServiceExecutionLimits::default()
        ),
        Err(ServiceError::PrivilegeEscalation)
    ));

    let mut missing_home = plan.clone();
    missing_home.user_home = root.join("missing-home");
    assert!(matches!(
        execute_service_lifecycle_with_runner(
            &missing_home,
            &runner,
            ServiceExecutionLimits::default()
        ),
        Err(ServiceError::Io { .. })
    ));

    let outside = root.parent().expect("fixture parent").to_path_buf();
    let mut escaped = plan.clone();
    escaped.state_dir = outside;
    assert!(matches!(
        execute_service_lifecycle_with_runner(&escaped, &runner, ServiceExecutionLimits::default()),
        Err(ServiceError::DestinationOutsideUserHome)
    ));

    let mut native_denied = plan;
    native_denied.manager_commands[0].program = "powershell.exe";
    assert!(matches!(
        execute_service_lifecycle(&native_denied),
        Err(ServiceError::ManagerProgramDenied(_))
    ));
    assert!(runner.calls().is_empty());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn lifecycle_execution_covers_descriptor_states_and_bounded_receipts() {
    let revalidate_root = fixture_root("revalidate");
    let revalidate = generate_service_lifecycle_plan(&request(
        &revalidate_root,
        ServicePlatform::current(),
        ServiceOperation::Install,
    ))
    .expect("revalidate plan");
    materialize_service_descriptor(&revalidate).expect("preexisting matching descriptor");
    let receipt = execute_service_lifecycle_with_runner(
        &revalidate,
        &FakeRunner::default(),
        ServiceExecutionLimits::default(),
    )
    .expect("revalidated install");
    assert_eq!(
        receipt.descriptor_action,
        ServiceDescriptorAction::Revalidated
    );
    fs::remove_dir_all(revalidate_root).expect("revalidate cleanup");

    let absent_root = fixture_root("already-absent");
    let absent = generate_service_lifecycle_plan(&request(
        &absent_root,
        ServicePlatform::current(),
        ServiceOperation::Uninstall,
    ))
    .expect("absent uninstall plan");
    let receipt = execute_service_lifecycle_with_runner(
        &absent,
        &FakeRunner::default(),
        ServiceExecutionLimits::default(),
    )
    .expect("absent uninstall");
    assert_eq!(
        receipt.descriptor_action,
        ServiceDescriptorAction::AlreadyAbsent
    );
    fs::remove_dir_all(absent_root).expect("absent cleanup");

    let drift_root = fixture_root("uninstall-drift");
    let drift = generate_service_lifecycle_plan(&request(
        &drift_root,
        ServicePlatform::current(),
        ServiceOperation::Uninstall,
    ))
    .expect("drift uninstall plan");
    fs::create_dir_all(drift.descriptor_path.parent().expect("descriptor parent"))
        .expect("descriptor parent");
    fs::write(&drift.descriptor_path, b"drift").expect("drift descriptor");
    assert!(matches!(
        execute_service_lifecycle_with_runner(
            &drift,
            &FakeRunner::default(),
            ServiceExecutionLimits::default()
        ),
        Err(ServiceError::DescriptorDrift)
    ));
    fs::remove_dir_all(drift_root).expect("drift cleanup");

    let bounded_root = fixture_root("bounded-output");
    let bounded = generate_service_lifecycle_plan(&request(
        &bounded_root,
        ServicePlatform::current(),
        ServiceOperation::Status,
    ))
    .expect("bounded status plan");
    let output = ServiceManagerOutput {
        stdout: vec![b'o'; 64],
        stderr: vec![b'e'; 64],
        ..success_output()
    };
    let receipt = execute_service_lifecycle_with_runner(
        &bounded,
        &FakeRunner::with_outputs(vec![output]),
        ServiceExecutionLimits {
            command_timeout: Duration::from_secs(1),
            max_output_bytes: 8,
        },
    )
    .expect("bounded status");
    assert!(receipt.commands[0].stdout_truncated);
    assert!(receipt.commands[0].stderr_truncated);
    assert_eq!(receipt.commands[0].stdout.len(), 8);
    assert_eq!(receipt.commands[0].stderr.len(), 8);
    fs::remove_dir_all(bounded_root).expect("bounded cleanup");
}

#[test]
fn lifecycle_receipts_and_descriptor_staging_fail_closed() {
    for (label, receipt_bytes, expected_schema) in [
        ("invalid-json", b"not-json".to_vec(), None),
        (
            "invalid-schema",
            serde_json::to_vec(&serde_json::json!({
                "schemaVersion": "wrong",
                "planDigest": "digest",
                "platform": ServicePlatform::current().as_str(),
                "operation": "install",
                "descriptorDigest": "digest"
            }))
            .expect("schema receipt"),
            Some("wrong"),
        ),
    ] {
        let root = fixture_root(label);
        let plan = generate_service_lifecycle_plan(&request(
            &root,
            ServicePlatform::current(),
            ServiceOperation::Install,
        ))
        .expect("receipt plan");
        materialize_service_descriptor(&plan).expect("matching descriptor");
        let receipt_path = plan.state_dir.join("service-lifecycle.receipt.json");
        fs::write(&receipt_path, receipt_bytes).expect("corrupt receipt");
        let error = execute_service_lifecycle_with_runner(
            &plan,
            &FakeRunner::default(),
            ServiceExecutionLimits::default(),
        )
        .expect_err("corrupt receipt must fail");
        if let Some(schema) = expected_schema {
            assert!(
                matches!(error, ServiceError::ExecutionReceiptSchema(actual) if actual == schema)
            );
        } else {
            assert!(matches!(error, ServiceError::InvalidExecutionReceipt(_)));
        }
        fs::remove_dir_all(root).expect("receipt cleanup");
    }

    let conflict_root = fixture_root("descriptor-stage-conflict");
    let conflict = generate_service_lifecycle_plan(&request(
        &conflict_root,
        ServicePlatform::current(),
        ServiceOperation::Install,
    ))
    .expect("conflict plan");
    let parent = conflict
        .descriptor_path
        .parent()
        .expect("descriptor parent");
    fs::create_dir_all(parent).expect("descriptor parent");
    let name = conflict
        .descriptor_path
        .file_name()
        .and_then(|value| value.to_str())
        .expect("descriptor name");
    fs::write(
        parent.join(format!(".{name}.ae-sdd-stage-{}", std::process::id())),
        b"conflict",
    )
    .expect("staging conflict");
    assert!(matches!(
        materialize_service_descriptor(&conflict),
        Err(ServiceError::StagingConflict)
    ));
    fs::remove_dir_all(conflict_root).expect("conflict cleanup");

    let oversized_root = fixture_root("oversized-descriptor");
    let mut oversized = generate_service_lifecycle_plan(&request(
        &oversized_root,
        ServicePlatform::current(),
        ServiceOperation::Install,
    ))
    .expect("oversized plan");
    oversized.descriptor_contents = "x".repeat(1024 * 1024 + 1);
    assert!(matches!(
        materialize_service_descriptor(&oversized),
        Err(ServiceError::DescriptorTooLarge)
    ));
    fs::remove_dir_all(oversized_root).expect("oversized cleanup");

    let directory_root = fixture_root("descriptor-directory");
    let directory = generate_service_lifecycle_plan(&request(
        &directory_root,
        ServicePlatform::current(),
        ServiceOperation::Status,
    ))
    .expect("directory plan");
    fs::create_dir_all(&directory.descriptor_path).expect("descriptor directory");
    assert!(matches!(
        inspect_service_descriptor(&directory),
        Err(ServiceError::InvalidPath("descriptorPath"))
    ));
    fs::remove_dir_all(directory_root).expect("directory cleanup");
}

#[test]
fn build_cli_covers_plan_materialize_and_inspect_output_modes() {
    let root = fixture_root("cli-modes");
    let install = request(&root, ServicePlatform::current(), ServiceOperation::Install);
    let request_path = root.join("install-request.json");
    fs::write(
        &request_path,
        serde_json::to_vec_pretty(&install).expect("request JSON"),
    )
    .expect("write request");

    let plan = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"))
        .args(["service", "--request"])
        .arg(&request_path)
        .output()
        .expect("text plan");
    assert!(plan.status.success());
    assert!(String::from_utf8_lossy(&plan.stdout).contains("service install plan"));

    let materialized = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"))
        .args(["service", "--request"])
        .arg(&request_path)
        .arg("--materialize")
        .output()
        .expect("text materialize");
    assert!(materialized.status.success());
    assert!(String::from_utf8_lossy(&materialized.stdout).contains("materialized"));

    let replay = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"))
        .args(["service", "--request"])
        .arg(&request_path)
        .args(["--materialize", "--json"])
        .output()
        .expect("JSON materialize replay");
    assert!(replay.status.success());
    let replay: serde_json::Value =
        serde_json::from_slice(&replay.stdout).expect("materialize JSON");
    assert_eq!(replay["created"], false);

    for json in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"));
        command
            .args(["service", "--request"])
            .arg(&request_path)
            .arg("--inspect");
        if json {
            command.arg("--json");
        }
        let output = command.output().expect("inspect CLI");
        assert!(output.status.success());
        if json {
            let status: serde_json::Value =
                serde_json::from_slice(&output.stdout).expect("inspect JSON");
            assert_eq!(status["state"], "matches");
        } else {
            assert!(String::from_utf8_lossy(&output.stdout).contains("status: Matches"));
        }
    }

    let status_request = request(&root, ServicePlatform::current(), ServiceOperation::Status);
    fs::write(
        &request_path,
        serde_json::to_vec(&status_request).expect("status request"),
    )
    .expect("rewrite request");
    let rejected = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"))
        .args(["service", "--request"])
        .arg(&request_path)
        .arg("--materialize")
        .output()
        .expect("rejected materialize");
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("valid only for an install plan"));
    fs::remove_dir_all(root).expect("cleanup");
}
