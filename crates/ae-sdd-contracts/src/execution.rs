//! Frozen verification execution-plan and receipt contracts.
//!
//! The wire surface deliberately represents an executable as a content-addressed
//! program reference plus an argument vector.  It has no raw shell-command or
//! environment-value field, so a worker cannot receive secrets through this DTO.

use std::{collections::BTreeSet, fmt};

use ae_sdd_domain::{
    ArtifactRef, EvidenceDigest, InputFingerprint, ProjectRelativePath, WorkItemId,
};
use ae_sdd_protocol::JobStatus;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{BoundedText, ExecutionId, ExecutionStepId, SchemaVersion, WorkerId, serde_domain};

/// Maximum arguments supplied to one executable.
pub const MAX_EXECUTION_ARGS: usize = 64;
/// Maximum environment references supplied to one executable.
pub const MAX_ENVIRONMENT_REFS: usize = 64;
/// Maximum execution steps in one verification plan.
pub const MAX_EXECUTION_STEPS: usize = 64;
/// Default per-step timeout in milliseconds.
pub const DEFAULT_EXECUTION_TIMEOUT_MS: u64 = 15 * 60 * 1000;
/// Maximum allowed per-step timeout in milliseconds.
pub const MAX_EXECUTION_TIMEOUT_MS: u64 = 2 * 60 * 60 * 1000;
/// Default retained output budget for each output stream.
pub const DEFAULT_OUTPUT_BYTES: u32 = 64 * 1024;
/// Maximum retained output budget for each output stream.
pub const MAX_OUTPUT_BYTES: u32 = 1024 * 1024;

/// Validation errors for execution plans and receipts.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ExecutionStepError {
    /// A collection exceeded its frozen v1 limit.
    #[error("execution collection exceeds its frozen v1 limit")]
    CollectionLimitExceeded,
    /// An environment reference appeared more than once.
    #[error("execution step contains a duplicate environment reference")]
    DuplicateEnvironmentRef,
    /// An execution step identity appeared more than once.
    #[error("verification plan contains a duplicate step identity")]
    DuplicateStep,
    /// A verification plan did not contain any work.
    #[error("verification execution plan must contain at least one step")]
    EmptyPlan,
    /// Environment reference syntax was not portable and value-free.
    #[error("environment reference must be an uppercase ASCII name without a value")]
    InvalidEnvironmentRef,
    /// Timeout or output limits were zero or outside their v1 bounds.
    #[error("execution limits are outside their frozen v1 bounds")]
    InvalidLimits,
    /// A receipt used a non-terminal job status.
    #[error("verification receipt requires a terminal job status")]
    NonTerminalStatus,
    /// Receipt timestamps were reversed.
    #[error("verification receipt finished before it started")]
    InvalidTimeRange,
    /// PASS did not describe a successful, non-cancelled process result.
    #[error("verification PASS requires exit code 0 without timeout or cancellation")]
    InvalidPassResult,
    /// Timeout/cancellation flags did not match the terminal status.
    #[error("verification receipt flags do not match its terminal status")]
    InconsistentTerminalStatus,
}

/// Name-only reference to an environment value held by a trusted worker.
///
/// The contract carries no corresponding value.  Resolution and secret access
/// remain outside the daemon contract and must be authorized by the worker.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct EnvironmentRef(Box<str>);

impl EnvironmentRef {
    /// Maximum environment-reference name length.
    pub const MAX_BYTES: usize = 128;

    /// Validates a value-free, portable environment reference.
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, ExecutionStepError> {
        let value = value.into();
        let valid_start = value
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_uppercase() || *byte == b'_');
        let valid_tail = value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_');
        if value.len() > Self::MAX_BYTES || !valid_start || !valid_tail || value.contains('=') {
            return Err(ExecutionStepError::InvalidEnvironmentRef);
        }
        Ok(Self(value))
    }

    /// Returns the environment name, never a value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EnvironmentRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for EnvironmentRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Worker-enforced time and output limits for one execution step.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "ExecutionLimitsWire", into = "ExecutionLimitsWire")]
pub struct ExecutionLimits {
    timeout_ms: u64,
    max_stdout_bytes: u32,
    max_stderr_bytes: u32,
}

impl ExecutionLimits {
    /// Constructs validated worker limits.
    pub fn new(
        timeout_ms: u64,
        max_stdout_bytes: u32,
        max_stderr_bytes: u32,
    ) -> Result<Self, ExecutionStepError> {
        if timeout_ms == 0
            || timeout_ms > MAX_EXECUTION_TIMEOUT_MS
            || max_stdout_bytes == 0
            || max_stdout_bytes > MAX_OUTPUT_BYTES
            || max_stderr_bytes == 0
            || max_stderr_bytes > MAX_OUTPUT_BYTES
        {
            return Err(ExecutionStepError::InvalidLimits);
        }
        Ok(Self {
            timeout_ms,
            max_stdout_bytes,
            max_stderr_bytes,
        })
    }
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            timeout_ms: DEFAULT_EXECUTION_TIMEOUT_MS,
            max_stdout_bytes: DEFAULT_OUTPUT_BYTES,
            max_stderr_bytes: DEFAULT_OUTPUT_BYTES,
        }
    }
}

impl<'de> Deserialize<'de> for ExecutionLimits {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ExecutionLimitsWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExecutionLimitsWire {
    timeout_ms: u64,
    max_stdout_bytes: u32,
    max_stderr_bytes: u32,
}

impl TryFrom<ExecutionLimitsWire> for ExecutionLimits {
    type Error = ExecutionStepError;

    fn try_from(value: ExecutionLimitsWire) -> Result<Self, Self::Error> {
        Self::new(
            value.timeout_ms,
            value.max_stdout_bytes,
            value.max_stderr_bytes,
        )
    }
}

impl From<ExecutionLimits> for ExecutionLimitsWire {
    fn from(value: ExecutionLimits) -> Self {
        Self {
            timeout_ms: value.timeout_ms,
            max_stdout_bytes: value.max_stdout_bytes,
            max_stderr_bytes: value.max_stderr_bytes,
        }
    }
}

/// One allowlisted program invocation in a verification plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "ExecutionStepWire", into = "ExecutionStepWire")]
pub struct ExecutionStep {
    schema_version: SchemaVersion,
    step_id: ExecutionStepId,
    program_ref: ArtifactRef,
    args: Vec<BoundedText<256>>,
    cwd: Option<ProjectRelativePath>,
    env_refs: Vec<EnvironmentRef>,
    limits: ExecutionLimits,
}

impl ExecutionStep {
    /// Maximum argument count accepted by this contract.
    pub const MAX_ARGS: usize = MAX_EXECUTION_ARGS;
    /// Maximum environment-reference count accepted by this contract.
    pub const MAX_ENV_REFS: usize = MAX_ENVIRONMENT_REFS;

    /// Constructs a step using the frozen v1 execution limits.
    pub fn new(
        schema_version: SchemaVersion,
        step_id: ExecutionStepId,
        program_ref: ArtifactRef,
        args: Vec<BoundedText<256>>,
        cwd: Option<ProjectRelativePath>,
        env_refs: Vec<EnvironmentRef>,
    ) -> Result<Self, ExecutionStepError> {
        Self::with_limits(
            schema_version,
            step_id,
            program_ref,
            args,
            cwd,
            env_refs,
            ExecutionLimits::default(),
        )
    }

    /// Constructs a step with explicit bounded execution limits.
    #[allow(clippy::too_many_arguments)]
    pub fn with_limits(
        schema_version: SchemaVersion,
        step_id: ExecutionStepId,
        program_ref: ArtifactRef,
        args: Vec<BoundedText<256>>,
        cwd: Option<ProjectRelativePath>,
        env_refs: Vec<EnvironmentRef>,
        limits: ExecutionLimits,
    ) -> Result<Self, ExecutionStepError> {
        if args.len() > MAX_EXECUTION_ARGS || env_refs.len() > MAX_ENVIRONMENT_REFS {
            return Err(ExecutionStepError::CollectionLimitExceeded);
        }
        let unique: BTreeSet<&EnvironmentRef> = env_refs.iter().collect();
        if unique.len() != env_refs.len() {
            return Err(ExecutionStepError::DuplicateEnvironmentRef);
        }
        Ok(Self {
            schema_version,
            step_id,
            program_ref,
            args,
            cwd,
            env_refs,
            limits,
        })
    }

    /// Returns the step identity.
    pub const fn step_id(&self) -> &ExecutionStepId {
        &self.step_id
    }

    /// Returns the content-addressed executable reference.
    pub const fn program_ref(&self) -> &ArtifactRef {
        &self.program_ref
    }
}

impl<'de> Deserialize<'de> for ExecutionStep {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ExecutionStepWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExecutionStepWire {
    schema_version: SchemaVersion,
    step_id: ExecutionStepId,
    #[serde(with = "serde_domain::artifact_ref")]
    program_ref: ArtifactRef,
    args: Vec<BoundedText<256>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_project_relative_path"
    )]
    cwd: Option<ProjectRelativePath>,
    env_refs: Vec<EnvironmentRef>,
    limits: ExecutionLimits,
}

impl TryFrom<ExecutionStepWire> for ExecutionStep {
    type Error = ExecutionStepError;

    fn try_from(value: ExecutionStepWire) -> Result<Self, Self::Error> {
        Self::with_limits(
            value.schema_version,
            value.step_id,
            value.program_ref,
            value.args,
            value.cwd,
            value.env_refs,
            value.limits,
        )
    }
}

impl From<ExecutionStep> for ExecutionStepWire {
    fn from(value: ExecutionStep) -> Self {
        Self {
            schema_version: value.schema_version,
            step_id: value.step_id,
            program_ref: value.program_ref,
            args: value.args,
            cwd: value.cwd,
            env_refs: value.env_refs,
            limits: value.limits,
        }
    }
}

/// Ordered, deterministic verification work assigned to an isolated worker.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(
    try_from = "VerificationExecutionPlanWire",
    into = "VerificationExecutionPlanWire"
)]
pub struct VerificationExecutionPlan {
    schema_version: SchemaVersion,
    execution_id: ExecutionId,
    work_item_id: WorkItemId,
    input_fingerprint: InputFingerprint,
    steps: Vec<ExecutionStep>,
}

impl VerificationExecutionPlan {
    /// Constructs a bounded execution plan.
    pub fn new(
        schema_version: SchemaVersion,
        execution_id: ExecutionId,
        work_item_id: WorkItemId,
        input_fingerprint: InputFingerprint,
        steps: Vec<ExecutionStep>,
    ) -> Result<Self, ExecutionStepError> {
        if steps.is_empty() {
            return Err(ExecutionStepError::EmptyPlan);
        }
        if steps.len() > MAX_EXECUTION_STEPS {
            return Err(ExecutionStepError::CollectionLimitExceeded);
        }
        let unique: BTreeSet<&ExecutionStepId> = steps.iter().map(ExecutionStep::step_id).collect();
        if unique.len() != steps.len() {
            return Err(ExecutionStepError::DuplicateStep);
        }
        Ok(Self {
            schema_version,
            execution_id,
            work_item_id,
            input_fingerprint,
            steps,
        })
    }

    /// Constructs and validates a terminal receipt for this exact plan input.
    #[allow(clippy::too_many_arguments)]
    pub fn receipt(
        &self,
        worker_id: WorkerId,
        status: JobStatus,
        exit_code: Option<i32>,
        stdout_digest: EvidenceDigest,
        stderr_digest: EvidenceDigest,
        started_at_unix_ms: u64,
        finished_at_unix_ms: u64,
        timed_out: bool,
        cancelled: bool,
    ) -> Result<VerificationReceipt, ExecutionStepError> {
        VerificationReceipt::new(
            self.schema_version,
            self.execution_id.clone(),
            self.work_item_id.clone(),
            self.input_fingerprint,
            worker_id,
            status,
            exit_code,
            stdout_digest,
            stderr_digest,
            started_at_unix_ms,
            finished_at_unix_ms,
            timed_out,
            cancelled,
        )
    }
}

impl<'de> Deserialize<'de> for VerificationExecutionPlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        VerificationExecutionPlanWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VerificationExecutionPlanWire {
    schema_version: SchemaVersion,
    execution_id: ExecutionId,
    #[serde(with = "serde_domain::work_item_id")]
    work_item_id: WorkItemId,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    steps: Vec<ExecutionStep>,
}

impl TryFrom<VerificationExecutionPlanWire> for VerificationExecutionPlan {
    type Error = ExecutionStepError;

    fn try_from(value: VerificationExecutionPlanWire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.execution_id,
            value.work_item_id,
            value.input_fingerprint,
            value.steps,
        )
    }
}

impl From<VerificationExecutionPlan> for VerificationExecutionPlanWire {
    fn from(value: VerificationExecutionPlan) -> Self {
        Self {
            schema_version: value.schema_version,
            execution_id: value.execution_id,
            work_item_id: value.work_item_id,
            input_fingerprint: value.input_fingerprint,
            steps: value.steps,
        }
    }
}

/// Canonical terminal receipt emitted by an isolated verification worker.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "VerificationReceiptWire", into = "VerificationReceiptWire")]
pub struct VerificationReceipt {
    schema_version: SchemaVersion,
    execution_id: ExecutionId,
    work_item_id: WorkItemId,
    input_fingerprint: InputFingerprint,
    worker_id: WorkerId,
    status: JobStatus,
    exit_code: Option<i32>,
    stdout_digest: EvidenceDigest,
    stderr_digest: EvidenceDigest,
    started_at_unix_ms: u64,
    finished_at_unix_ms: u64,
    timed_out: bool,
    cancelled: bool,
}

impl VerificationReceipt {
    /// Constructs a terminal verification receipt and checks status consistency.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        execution_id: ExecutionId,
        work_item_id: WorkItemId,
        input_fingerprint: InputFingerprint,
        worker_id: WorkerId,
        status: JobStatus,
        exit_code: Option<i32>,
        stdout_digest: EvidenceDigest,
        stderr_digest: EvidenceDigest,
        started_at_unix_ms: u64,
        finished_at_unix_ms: u64,
        timed_out: bool,
        cancelled: bool,
    ) -> Result<Self, ExecutionStepError> {
        if matches!(status, JobStatus::Queued | JobStatus::Running) {
            return Err(ExecutionStepError::NonTerminalStatus);
        }
        if finished_at_unix_ms < started_at_unix_ms {
            return Err(ExecutionStepError::InvalidTimeRange);
        }
        if status == JobStatus::Pass && (exit_code != Some(0) || timed_out || cancelled) {
            return Err(ExecutionStepError::InvalidPassResult);
        }
        if (status == JobStatus::Timeout) != timed_out
            || (status == JobStatus::Cancelled) != cancelled
        {
            return Err(ExecutionStepError::InconsistentTerminalStatus);
        }
        Ok(Self {
            schema_version,
            execution_id,
            work_item_id,
            input_fingerprint,
            worker_id,
            status,
            exit_code,
            stdout_digest,
            stderr_digest,
            started_at_unix_ms,
            finished_at_unix_ms,
            timed_out,
            cancelled,
        })
    }

    /// Returns the terminal worker status.
    pub const fn status(&self) -> JobStatus {
        self.status
    }
}

impl<'de> Deserialize<'de> for VerificationReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        VerificationReceiptWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VerificationReceiptWire {
    schema_version: SchemaVersion,
    execution_id: ExecutionId,
    #[serde(with = "serde_domain::work_item_id")]
    work_item_id: WorkItemId,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    worker_id: WorkerId,
    status: JobStatus,
    exit_code: Option<i32>,
    #[serde(with = "serde_domain::evidence_digest")]
    stdout_digest: EvidenceDigest,
    #[serde(with = "serde_domain::evidence_digest")]
    stderr_digest: EvidenceDigest,
    started_at_unix_ms: u64,
    finished_at_unix_ms: u64,
    timed_out: bool,
    cancelled: bool,
}

impl TryFrom<VerificationReceiptWire> for VerificationReceipt {
    type Error = ExecutionStepError;

    fn try_from(value: VerificationReceiptWire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.execution_id,
            value.work_item_id,
            value.input_fingerprint,
            value.worker_id,
            value.status,
            value.exit_code,
            value.stdout_digest,
            value.stderr_digest,
            value.started_at_unix_ms,
            value.finished_at_unix_ms,
            value.timed_out,
            value.cancelled,
        )
    }
}

impl From<VerificationReceipt> for VerificationReceiptWire {
    fn from(value: VerificationReceipt) -> Self {
        Self {
            schema_version: value.schema_version,
            execution_id: value.execution_id,
            work_item_id: value.work_item_id,
            input_fingerprint: value.input_fingerprint,
            worker_id: value.worker_id,
            status: value.status,
            exit_code: value.exit_code,
            stdout_digest: value.stdout_digest,
            stderr_digest: value.stderr_digest,
            started_at_unix_ms: value.started_at_unix_ms,
            finished_at_unix_ms: value.finished_at_unix_ms,
            timed_out: value.timed_out,
            cancelled: value.cancelled,
        }
    }
}

mod optional_project_relative_path {
    use ae_sdd_domain::ProjectRelativePath;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(
        value: &Option<ProjectRelativePath>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .as_ref()
            .map(ToString::to_string)
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Option<ProjectRelativePath>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| ProjectRelativePath::new(value).map_err(de::Error::custom))
            .transpose()
    }
}
