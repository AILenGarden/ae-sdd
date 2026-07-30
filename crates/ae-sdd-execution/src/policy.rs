//! Worker-isolation content policy for verification execution plans, plus the
//! decision vocabulary returned by the pure execution supervisor.

use std::collections::BTreeSet;

use ae_sdd_contracts::execution::VerificationExecutionPlan;
use ae_sdd_domain::{ArtifactDigest, ArtifactRef};

use crate::error::{ExecutionPolicyError, ExecutionPolicyFault, ExecutionSupervisorError};

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

/// Machine-recognised progress event kinds (implementation plan §4.5).
///
/// These are the only events that may reset the consecutive no-progress
/// batch counter; repeated reads, repeated failing runs, cache hits and
/// state reprints never produce one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionProgressKindV1 {
    /// A patch produced a content digest not seen before.
    NewPatchDigest,
    /// The focused verification ran for the first time in this slice.
    FirstFocusedRun,
    /// The focused verification turned from non-green to green.
    FocusedTurnedGreen,
    /// A blocker reported a new code + evidence locator pair.
    NewBlocker,
    /// A new evidence ledger event was appended.
    NewEvidenceEvent,
    /// The slice advanced its lifecycle: a next legal status, or a refactor
    /// loop opening/closing at the focused GREEN.
    SliceAdvanced,
}

/// Bounded retained-output directive attached to an allowed tool event.
///
/// The supervisor never retains more than `max_tool_output_bytes` of one
/// tool call; when truncation engages, the full output is bound to its
/// digest and (when already persisted) artifact locator so evidence stays
/// verifiable without carrying the output body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionOutputDirectiveV1 {
    pub(crate) retained_bytes: u32,
    pub(crate) truncated: bool,
    pub(crate) output_digest: Option<ArtifactDigest>,
    pub(crate) output_locator: Option<ArtifactRef>,
}

impl ExecutionOutputDirectiveV1 {
    /// Returns how many output bytes may be retained for this call.
    pub const fn retained_bytes(&self) -> u32 {
        self.retained_bytes
    }

    /// Returns whether the raw output exceeded the retained budget.
    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    /// Returns the digest of the full output when truncation engaged.
    pub const fn output_digest(&self) -> Option<ArtifactDigest> {
        self.output_digest
    }

    /// Returns the locator of the full output artifact when truncation engaged.
    pub const fn output_locator(&self) -> Option<&ArtifactRef> {
        self.output_locator.as_ref()
    }
}

/// Allowance for one tool event, recording whether it made progress.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionAllowanceV1 {
    pub(crate) progress: Option<ExecutionProgressKindV1>,
    pub(crate) output: Option<ExecutionOutputDirectiveV1>,
}

impl ExecutionAllowanceV1 {
    /// Returns the progress kind this event produced, if any.
    pub const fn progress(&self) -> Option<ExecutionProgressKindV1> {
        self.progress
    }

    /// Returns the retained-output directive for tool events carrying output.
    pub const fn output(&self) -> Option<&ExecutionOutputDirectiveV1> {
        self.output.as_ref()
    }
}

/// Deferral for one tool event, with a bounded retry hint.
///
/// The pure slice-progress reducer never defers; the variant exists so the
/// runtime resource arbitration (daemon-wide Cargo lock) can reuse the same
/// decision vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionDeferralV1 {
    pub(crate) retry_after_ms: u64,
}

impl ExecutionDeferralV1 {
    /// Returns how long the caller should wait before retrying, in milliseconds.
    pub const fn retry_after_ms(&self) -> u64 {
        self.retry_after_ms
    }
}

/// Machine decision returned by [`crate::ExecutionSupervisor::decide`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionDecisionV1 {
    /// The event is admissible; the allowance records progress and the
    /// bounded retained-output directive.
    Allow(ExecutionAllowanceV1),
    /// The event is rejected because an execution budget is exhausted or the
    /// slice can no longer change.
    Deny(ExecutionSupervisorError),
    /// The event must wait for a resource; never emitted by the pure reducer.
    Defer(ExecutionDeferralV1),
    /// The event is rejected until machine-verified progress is made.
    RequireProgress(ExecutionSupervisorError),
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
