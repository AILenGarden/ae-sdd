//! Command-policy integration tests covering shell-injection rejection,
//! environment isolation and receipt identity validation.

use ae_sdd_contracts::execution::{
    EnvironmentRef, ExecutionLimits, ExecutionStep, VerificationExecutionPlan,
};
use ae_sdd_contracts::{BoundedText, ExecutionId, ExecutionStepId, SchemaVersion};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, InputFingerprint, ProjectRelativePath, WorkItemId,
};
use ae_sdd_execution::{ExecutionPolicy, ExecutionPolicyError, ExecutionPolicyFault};

fn artifact(kind: &str, path: &str) -> ArtifactRef {
    let bytes = path.as_bytes();
    ArtifactRef::new(
        ArtifactKind::new(kind).unwrap(),
        ProjectRelativePath::new(path).unwrap(),
        ArtifactDigest::digest(bytes),
        bytes.len() as u64,
    )
}

fn plan_with_program(program_path: &str) -> VerificationExecutionPlan {
    let step = ExecutionStep::with_limits(
        SchemaVersion::V1,
        ExecutionStepId::new("step-1").unwrap(),
        artifact("program", program_path),
        vec![],
        None,
        vec![],
        ExecutionLimits::new(60_000, 1024, 1024).unwrap(),
    )
    .unwrap();
    VerificationExecutionPlan::new(
        SchemaVersion::V1,
        ExecutionId::new("exec-1").unwrap(),
        WorkItemId::new("STORY-1").unwrap(),
        InputFingerprint::digest(b"plan-input"),
        vec![step],
    )
    .unwrap()
}

#[test]
fn clean_cargo_program_passes_policy() {
    let plan = plan_with_program("tools/cargo.exe");
    ExecutionPolicy::validate_plan(&plan).expect("clean program passes");
}

#[test]
fn cmd_exe_program_is_rejected() {
    // `ProjectRelativePath` rejects Windows absolute paths; exercise the pure
    // path validator directly, which is the authoritative shell-injection guard.
    let err =
        ae_sdd_execution::reject_shell_program_path("C:/Windows/System32/cmd.exe").unwrap_err();
    assert_eq!(
        err,
        ExecutionPolicyError::PlanRejected(ExecutionPolicyFault::ShellExecutableProgram)
    );
}

#[test]
fn bash_program_is_rejected() {
    let err = ae_sdd_execution::reject_shell_program_path("/usr/bin/bash").unwrap_err();
    assert_eq!(
        err,
        ExecutionPolicyError::PlanRejected(ExecutionPolicyFault::ShellExecutableProgram)
    );
}

#[test]
fn pipe_metacharacter_in_program_path_is_rejected() {
    let err = ae_sdd_execution::reject_shell_program_path("tools/cargo|run").unwrap_err();
    assert_eq!(
        err,
        ExecutionPolicyError::PlanRejected(ExecutionPolicyFault::ShellInjectionInProgramRef)
    );
}

#[test]
fn semicolon_in_program_path_is_rejected() {
    assert!(ae_sdd_execution::reject_shell_program_path("tools/cargo;rm -rf /").is_err());
}

#[test]
fn cmd_c_fragment_in_program_path_is_rejected() {
    assert!(ae_sdd_execution::reject_shell_program_path("cmd /c cargo test").is_err());
}

#[test]
fn environment_reference_rejects_inline_secret_value() {
    // EnvironmentRef itself is the contract-level guard; confirm that a value
    // like "API_TOKEN=secret" is rejected at construction time.
    assert!(EnvironmentRef::new("API_TOKEN=secret").is_err());
    assert!(EnvironmentRef::new("CARGO_HOME").is_ok());
}

#[test]
fn too_many_args_rejected_at_step_construction() {
    let too_many = vec![
        BoundedText::<256>::new("x").unwrap();
        ae_sdd_contracts::execution::ExecutionStep::MAX_ARGS + 1
    ];
    let result = ExecutionStep::new(
        SchemaVersion::V1,
        ExecutionStepId::new("step-many").unwrap(),
        artifact("program", "tools/cargo.exe"),
        too_many,
        None,
        vec![],
    );
    assert!(result.is_err());
}
