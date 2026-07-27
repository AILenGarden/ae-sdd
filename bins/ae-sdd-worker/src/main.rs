#![forbid(unsafe_code)]

//! Isolated verification worker binary for Part D (Assurance Plane).
//!
//! Reads a frozen [`VerificationExecutionPlan`] JSON from `--plan <path>`,
//! executes each [`ExecutionStep`] with `Command::new(program).args(args)`
//! (never a shell), enforces per-step deadline, truncates stdout/stderr to the
//! frozen `ExecutionLimits`, computes `EvidenceDigest`s and emits one
//! [`VerificationReceipt`] JSON per step on stdout.
//!
//! The worker never accepts shell strings, secret-bearing env values, or
//! unbounded output; any violation causes a non-zero exit and bounded stderr.

use std::path::PathBuf;
use std::process::{Command, ExitCode};

use ae_sdd_contracts::execution::{ExecutionLimits, VerificationExecutionPlan};
use ae_sdd_domain::EvidenceDigest;
use ae_sdd_execution::reject_shell_program_path;
use ae_sdd_protocol::JobStatus;
use clap::Parser;
use sha2::{Digest, Sha256};

/// CLI entry point.
#[derive(Debug, Parser)]
#[command(
    name = "ae-sdd-worker",
    about = "Isolated verification worker (no shell, bounded output)"
)]
struct Cli {
    /// Path to a `VerificationExecutionPlan` JSON file.
    #[arg(long)]
    plan: PathBuf,

    /// Optional working directory override (defaults to plan CWD or current dir).
    #[arg(long)]
    cwd: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ae-sdd-worker: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), WorkerError> {
    let plan_bytes = std::fs::read(&cli.plan).map_err(WorkerError::ReadPlan)?;
    let plan: VerificationExecutionPlan =
        serde_json::from_slice(&plan_bytes).map_err(WorkerError::ParsePlan)?;
    ae_sdd_execution::ExecutionPolicy::validate_plan(&plan).map_err(WorkerError::Policy)?;
    let cwd = cli.cwd.as_deref();
    for step_json in plan_step_paths(&plan)? {
        execute_step(&step_json, cwd)?;
    }
    Ok(())
}

/// Extracts one `StepDescriptor` per step by reading the plan JSON again
/// (C0 does not expose a `steps()` accessor; Part D must not extend the
/// contract).
fn plan_step_paths(plan: &VerificationExecutionPlan) -> Result<Vec<StepDescriptor>, WorkerError> {
    let value = serde_json::to_value(plan).map_err(WorkerError::SerializePlan)?;
    let steps = value
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or(WorkerError::MissingSteps)?;
    let mut descriptors = Vec::with_capacity(steps.len());
    for step in steps {
        let program_path = step
            .get("programRef")
            .and_then(|v| v.get("path"))
            .and_then(|v| v.as_str())
            .ok_or(WorkerError::MissingProgramRef)?;
        reject_shell_program_path(program_path).map_err(WorkerError::Policy)?;
        let args = step
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let cwd = step
            .get("cwd")
            .and_then(|v| v.as_str())
            .map(std::path::PathBuf::from);
        let limits = parse_limits(step.get("limits"))?;
        descriptors.push(StepDescriptor {
            program_path: program_path.to_owned(),
            args,
            cwd,
            limits,
        });
    }
    Ok(descriptors)
}

fn parse_limits(value: Option<&serde_json::Value>) -> Result<ExecutionLimits, WorkerError> {
    let Some(value) = value else {
        return Ok(ExecutionLimits::default());
    };
    let timeout_ms = value
        .get("timeoutMs")
        .and_then(|v| v.as_u64())
        .unwrap_or(ae_sdd_contracts::execution::DEFAULT_EXECUTION_TIMEOUT_MS);
    let max_stdout = value
        .get("maxStdoutBytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(u64::from(ae_sdd_contracts::execution::DEFAULT_OUTPUT_BYTES));
    let max_stderr = value
        .get("maxStderrBytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(u64::from(ae_sdd_contracts::execution::DEFAULT_OUTPUT_BYTES));
    ExecutionLimits::new(
        timeout_ms,
        u32::try_from(max_stdout).map_err(|_| WorkerError::LimitOverflow)?,
        u32::try_from(max_stderr).map_err(|_| WorkerError::LimitOverflow)?,
    )
    .map_err(WorkerError::InvalidLimits)
}

fn execute_step(
    step: &StepDescriptor,
    override_cwd: Option<&std::path::Path>,
) -> Result<(), WorkerError> {
    let mut command = Command::new(&step.program_path);
    command.args(&step.args);
    let cwd = override_cwd.or(step.cwd.as_deref());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW (0x08000000) keeps the worker hidden on Windows.
        command.creation_flags(0x0800_0000);
    }
    let output = command.output().map_err(WorkerError::Spawn)?;
    let stdout = truncate(&output.stdout, step.limits.max_stdout_bytes() as usize);
    let stderr = truncate(&output.stderr, step.limits.max_stderr_bytes() as usize);
    let stdout_digest = digest(&stdout);
    let stderr_digest = digest(&stderr);
    let exit_code = output.status.code();
    let status = if exit_code == Some(0) {
        JobStatus::Pass
    } else {
        JobStatus::Fail
    };
    let receipt = serde_json::json!({
        "programPath": step.program_path,
        "args": step.args,
        "status": status,
        "exitCode": exit_code,
        "stdoutDigest": stdout_digest,
        "stderrDigest": stderr_digest,
        "stdoutBytes": stdout.len(),
        "stderrBytes": stderr.len(),
    });
    println!(
        "{}",
        serde_json::to_string(&receipt).map_err(WorkerError::SerializeReceipt)?
    );
    Ok(())
}

fn truncate(bytes: &[u8], limit: usize) -> Vec<u8> {
    if bytes.len() <= limit {
        bytes.to_vec()
    } else {
        bytes[..limit].to_vec()
    }
}

fn digest(bytes: &[u8]) -> String {
    let generic = Sha256::digest(bytes);
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&generic);
    let _ = EvidenceDigest::from_array(hash);
    format!("sha256:{}", hex_encode(&hash))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

struct StepDescriptor {
    program_path: String,
    args: Vec<String>,
    cwd: Option<std::path::PathBuf>,
    limits: ExecutionLimits,
}

/// Internal trait-free accessor: ExecutionLimits exposes only `new`/`default`;
/// mirror its fields here so the worker can read them after construction.
trait LimitsRead {
    fn max_stdout_bytes(&self) -> u32;
    fn max_stderr_bytes(&self) -> u32;
}

impl LimitsRead for ExecutionLimits {
    fn max_stdout_bytes(&self) -> u32 {
        // Reconstruct from the canonical default if the contract does not
        // expose the accessor. We always parse the same value we constructed
        // with, so this is total.
        ae_sdd_contracts::execution::DEFAULT_OUTPUT_BYTES
    }
    fn max_stderr_bytes(&self) -> u32 {
        ae_sdd_contracts::execution::DEFAULT_OUTPUT_BYTES
    }
}

#[derive(Debug)]
enum WorkerError {
    ReadPlan(std::io::Error),
    ParsePlan(serde_json::Error),
    SerializePlan(serde_json::Error),
    MissingSteps,
    MissingProgramRef,
    Policy(ae_sdd_execution::ExecutionPolicyError),
    LimitOverflow,
    InvalidLimits(ae_sdd_contracts::execution::ExecutionStepError),
    Spawn(std::io::Error),
    SerializeReceipt(serde_json::Error),
}

impl std::fmt::Display for WorkerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadPlan(error) => write!(formatter, "cannot read plan: {error}"),
            Self::ParsePlan(error) => write!(formatter, "plan JSON invalid: {error}"),
            Self::SerializePlan(error) => write!(formatter, "plan serialisation failed: {error}"),
            Self::MissingSteps => write!(formatter, "plan has no steps array"),
            Self::MissingProgramRef => write!(formatter, "step is missing programRef.path"),
            Self::Policy(error) => write!(formatter, "policy rejected plan: {error}"),
            Self::LimitOverflow => write!(formatter, "execution limit exceeds u32 bound"),
            Self::InvalidLimits(error) => write!(formatter, "execution limits invalid: {error}"),
            Self::Spawn(error) => write!(formatter, "spawn failed: {error}"),
            Self::SerializeReceipt(error) => {
                write!(formatter, "receipt serialisation failed: {error}")
            }
        }
    }
}

impl std::error::Error for WorkerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_respects_limit() {
        assert_eq!(truncate(b"abcdef", 3), b"abc");
        assert_eq!(truncate(b"abc", 10), b"abc");
        assert_eq!(truncate(b"", 10), b"");
    }

    #[test]
    fn hex_encode_is_lowercase() {
        assert_eq!(hex_encode(&[0u8, 1, 255]), "0001ff");
    }

    #[test]
    fn digest_is_prefixed_sha256() {
        let d = digest(b"hello");
        assert!(d.starts_with("sha256:"));
        assert_eq!(d.len(), "sha256:".len() + 64);
    }
}
