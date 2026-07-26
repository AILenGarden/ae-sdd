//! Worker-isolation content policy for verification execution plans.

use std::collections::BTreeSet;

use ae_sdd_contracts::execution::VerificationExecutionPlan;

use crate::error::{ExecutionPolicyError, ExecutionPolicyFault};

/// Names of executables that always indicate a shell dispatcher.
const SHELL_EXECUTABLES: &[&str] = &[
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "sh",
    "bash",
    "zsh",
    "fish",
    "csh",
    "tcsh",
    "dash",
];

/// Characters that, when present in a program reference path or argument,
/// indicate an attempt to invoke a shell string rather than a bounded program.
const SHELL_METACHARACTERS: &[char] = &[';', '|', '&', '\n', '\r', '\t', '`'];

/// Pure worker-isolation policy.
///
/// The policy only inspects the frozen [`VerificationExecutionPlan`] content.
/// It never reads the filesystem, clock or process table; actual program
/// resolution is performed by the C1-supervised `ae-sdd-worker` binary.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExecutionPolicy;

impl ExecutionPolicy {
    /// Validates that every step in the plan uses a bounded program reference
    /// without shell-injection content.
    ///
    /// The plan is read via serde JSON because C0 does not yet expose a
    /// `steps()` accessor; Part D must not extend the contract.
    pub fn validate_plan(plan: &VerificationExecutionPlan) -> Result<(), ExecutionPolicyError> {
        let plan_value = serde_json::to_value(plan).map_err(|_| {
            ExecutionPolicyError::PlanRejected(ExecutionPolicyFault::ShellInjectionInProgramRef)
        })?;
        let steps = plan_value
            .get("steps")
            .and_then(|value| value.as_array())
            .ok_or(ExecutionPolicyError::PlanRejected(
                ExecutionPolicyFault::ShellInjectionInProgramRef,
            ))?;
        for step in steps {
            let program_ref_path = step
                .get("programRef")
                .and_then(|value| value.get("path"))
                .and_then(|value| value.as_str())
                .ok_or(ExecutionPolicyError::PlanRejected(
                    ExecutionPolicyFault::ShellInjectionInProgramRef,
                ))?;
            reject_shell_program_path(program_ref_path)?;
        }
        Ok(())
    }
}

/// Validates one program reference path against the shell-injection policy.
///
/// Exposed publicly so integration tests and the `ae-sdd-worker` binary can
/// exercise the validator directly without going through the strict
/// `ProjectRelativePath` constructor (which rejects Windows absolute paths
/// before this policy runs).
pub fn reject_shell_program_path(path: &str) -> Result<(), ExecutionPolicyError> {
    let lowered = path.to_ascii_lowercase();
    let tail = lowered.rsplit(['/', '\\']).next().unwrap_or(&lowered);
    if SHELL_EXECUTABLES.contains(&tail) {
        return Err(ExecutionPolicyError::PlanRejected(
            ExecutionPolicyFault::ShellExecutableProgram,
        ));
    }
    if path.chars().any(|ch| SHELL_METACHARACTERS.contains(&ch)) {
        return Err(ExecutionPolicyError::PlanRejected(
            ExecutionPolicyFault::ShellInjectionInProgramRef,
        ));
    }
    if lowered.contains("/c ") || lowered.contains(" /c") || lowered.ends_with(" /c") {
        return Err(ExecutionPolicyError::PlanRejected(
            ExecutionPolicyFault::ShellInjectionInProgramRef,
        ));
    }
    Ok(())
}

/// Returns the unique set of shell executables recognised by the policy.
#[must_use]
pub fn shell_executable_blocklist() -> BTreeSet<&'static str> {
    SHELL_EXECUTABLES.iter().copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocklist_recognises_common_shells() {
        let block = shell_executable_blocklist();
        assert!(block.contains("cmd.exe"));
        assert!(block.contains("bash"));
        assert!(block.contains("powershell.exe"));
    }

    #[test]
    fn path_rejects_shell_executable_tail() {
        let err = reject_shell_program_path("C:/Windows/System32/cmd.exe").unwrap_err();
        assert_eq!(
            err,
            ExecutionPolicyError::PlanRejected(ExecutionPolicyFault::ShellExecutableProgram)
        );
    }

    #[test]
    fn path_rejects_pipe_metacharacter() {
        let err = reject_shell_program_path("tools/cargo|run").unwrap_err();
        assert_eq!(
            err,
            ExecutionPolicyError::PlanRejected(ExecutionPolicyFault::ShellInjectionInProgramRef)
        );
    }

    #[test]
    fn path_rejects_cmd_c_fragment() {
        let err = reject_shell_program_path("cmd /c cargo test").unwrap_err();
        assert!(matches!(
            err,
            ExecutionPolicyError::PlanRejected(ExecutionPolicyFault::ShellInjectionInProgramRef)
                | ExecutionPolicyError::PlanRejected(ExecutionPolicyFault::ShellExecutableProgram)
        ));
    }

    #[test]
    fn clean_program_path_passes() {
        reject_shell_program_path("tools/cargo.exe").unwrap();
        reject_shell_program_path("/usr/local/bin/cargo").unwrap();
    }
}
