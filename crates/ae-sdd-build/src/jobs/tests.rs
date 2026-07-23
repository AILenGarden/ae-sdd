use super::*;

fn fixture(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ae-sdd-build-job-{name}-{}-{}",
        std::process::id(),
        now_unix_ms()
    ));
    fs::create_dir_all(&path).expect("fixture root");
    path
}

fn request(root: &Path, mode: ExecutionMode) -> NativeJobRequest {
    NativeJobRequest {
        schema_version: JOB_SCHEMA.to_owned(),
        entrypoint: "init".to_owned(),
        actor: "test-agent".to_owned(),
        reason: "test atomic initialization".to_owned(),
        idempotency_key: "init-001".to_owned(),
        mode,
        allowed_roots: vec![root.to_path_buf()],
        job: JobInput::Init(InitInput {
            project_root: root.to_path_buf(),
            changes: vec![AdminChange {
                relative_path: PathBuf::from(".ae-sdd/config.json"),
                contents: "{\"version\":1}\n".to_owned(),
                permission: PermissionClass::PrivateFile,
            }],
        }),
    }
}

#[test]
fn dry_run_has_no_filesystem_side_effect() {
    let root = fixture("dry-run");
    let execution =
        execute_native_job(&request(&root, ExecutionMode::DryRun)).expect("dry run succeeds");
    assert!(execution.receipt.is_none());
    assert!(!root.join(".ae-sdd").exists());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn apply_writes_receipt_and_replay_is_side_effect_free() {
    let root = fixture("apply");
    let first = execute_native_job(&request(&root, ExecutionMode::Apply)).expect("apply succeeds");
    assert!(!first.replayed);
    assert_eq!(
        fs::read_to_string(root.join(".ae-sdd/config.json")).expect("generated config"),
        "{\"version\":1}\n"
    );
    let second =
        execute_native_job(&request(&root, ExecutionMode::Apply)).expect("replay succeeds");
    assert!(second.replayed);
    assert_eq!(first.plan_digest, second.plan_digest);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn idempotency_key_reuse_with_new_payload_fails() {
    let root = fixture("conflict");
    execute_native_job(&request(&root, ExecutionMode::Apply)).expect("first apply");
    let mut changed = request(&root, ExecutionMode::Apply);
    let JobInput::Init(input) = &mut changed.job else {
        panic!("init fixture")
    };
    input.changes[0].contents = "{\"version\":2}\n".to_owned();
    assert!(matches!(
        execute_native_job(&changed),
        Err(JobError::IdempotencyConflict)
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn parent_path_and_external_target_are_rejected() {
    let root = fixture("containment");
    let mut invalid = request(&root, ExecutionMode::DryRun);
    let JobInput::Init(input) = &mut invalid.job else {
        panic!("init fixture")
    };
    input.changes[0].relative_path = PathBuf::from("../escape");
    assert!(matches!(
        execute_native_job(&invalid),
        Err(JobError::InvalidRelativePath(_))
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn exact_entrypoint_registry_rejects_unknown_and_kind_mismatch() {
    let root = fixture("entrypoint");
    let mut unknown = request(&root, ExecutionMode::DryRun);
    unknown.entrypoint = "git.status".to_owned();
    assert!(matches!(
        execute_native_job(&unknown),
        Err(JobError::EntrypointNotRegistered(_))
    ));

    let mut mismatch = request(&root, ExecutionMode::DryRun);
    mismatch.entrypoint = "compile".to_owned();
    assert!(matches!(
        execute_native_job(&mismatch),
        Err(JobError::EntrypointKindMismatch { .. })
    ));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn harness_is_bounded_and_contains_source_digests() {
    let root = fixture("harness");
    fs::write(root.join("one.md"), "one\n").expect("source");
    let request = NativeJobRequest {
        schema_version: JOB_SCHEMA.to_owned(),
        entrypoint: "harness".to_owned(),
        actor: "test-agent".to_owned(),
        reason: "generate harness".to_owned(),
        idempotency_key: "harness-001".to_owned(),
        mode: ExecutionMode::Apply,
        allowed_roots: vec![root.clone()],
        job: JobInput::Harness(HarnessInput {
            source_files: vec![root.join("one.md")],
            target_file: root.join("output/agent.md"),
            title: "Agent Harness".to_owned(),
        }),
    };
    execute_native_job(&request).expect("harness apply");
    let output = fs::read_to_string(root.join("output/agent.md")).expect("harness");
    assert!(output.contains("ae-sdd:harness-source"));
    assert!(output.contains("sha256="));
    fs::remove_dir_all(root).expect("cleanup");
}
