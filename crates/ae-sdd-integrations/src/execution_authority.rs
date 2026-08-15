use std::fs;
use std::path::Path;
use std::str::FromStr;

use ae_sdd_contracts::execution_runtime::{
    ExecutionBudgetsV1, ExecutionCapsuleError, ExecutionCapsuleV1, MAX_SLICE_PATH_SCOPE,
    SourceReadSpecV1,
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, ExecutionSliceId, InventoryGeneration, PolicyDigest,
    ProjectRelativePath, StateRevision, StoryId, VerificationId, WorkItemId,
};
use ae_sdd_execution::{
    CapsuleBuildInputV1, CapsuleBuildOutcome, ExecutionCapsuleBuildError, ExecutionPolicy,
    ExecutionSliceSpecV1, VerificationExecutionPlan, VerificationReceipt, build_execution_capsule,
    validate_against_plan,
};
use ae_sdd_protocol::{JobStatus, StableErrorCode};
use ae_sdd_runtime::{RuntimeError, RuntimeJobRecord, RuntimeJobStatus, RuntimeResult};
use serde::Deserialize;
use serde_json::{Map, Value, json};

const MAX_PROJECT_RECEIPT_BYTES: usize = 262_144;
const TOOLSET_RECEIPT_KIND: &str = "toolsetReceiptAuthority";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VerificationPlanOperationInput {
    toolset_job_id: String,
    plan: VerificationExecutionPlan,
    receipt_id: String,
    receipt_digest: String,
    source_revision: u64,
    plan_digest: String,
    methodology_digest: String,
    policy_digest: String,
    input_fingerprint: String,
    changed_paths: Vec<String>,
    since_fingerprint: Option<String>,
    persist: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectToolsetReceipt {
    schema_version: u64,
    kind: String,
    toolset_job_id: String,
    workspace_id: String,
    work_item_id: String,
    outcome: String,
    status: String,
    validated: bool,
    receipt_id: String,
    receipt_digest: String,
    plan: VerificationExecutionPlan,
    receipt: VerificationReceipt,
    plan_digest: String,
    methodology_digest: String,
    policy_digest: String,
    input_fingerprint: String,
    source_revision: u64,
    committed_revision: u64,
    inventory_generation: u64,
    identity_digest: String,
    mutation_id: String,
    recorder: ProjectReceiptRecorder,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectReceiptRecorder {
    session_id: String,
    root_session_id: String,
    delegation_id: Option<String>,
    context_generation: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ToolsetReceiptRef {
    schema_version: u64,
    toolset_job_id: String,
    receipt_id: String,
    receipt_digest: String,
    artifact_ref: String,
    project_receipt_digest: String,
    manifest_ref: String,
    manifest_digest: String,
    mutation_id: String,
    source_revision: u64,
    committed_revision: u64,
}

/// Fully verified durable authority used to construct `verificationPlan`.
#[derive(Clone, Debug)]
pub(crate) struct VerifiedExecutionAuthority {
    plan: Value,
    toolset_job_id: String,
    source_revision: u64,
    plan_digest: String,
    input_fingerprint: String,
    changed_paths: Vec<Value>,
    since_fingerprint: String,
}

impl VerifiedExecutionAuthority {
    pub(crate) fn plan(&self) -> &Value {
        &self.plan
    }

    pub(crate) fn toolset_job_id(&self) -> &str {
        &self.toolset_job_id
    }

    pub(crate) const fn source_revision(&self) -> u64 {
        self.source_revision
    }

    pub(crate) fn plan_digest(&self) -> &str {
        &self.plan_digest
    }

    pub(crate) fn input_fingerprint(&self) -> &str {
        &self.input_fingerprint
    }

    pub(crate) fn changed_paths(&self) -> &[Value] {
        &self.changed_paths
    }

    pub(crate) fn since_fingerprint(&self) -> &str {
        &self.since_fingerprint
    }
}

/// Loads and validates the runtime-job -> project-receipt -> active-manifest chain.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_execution_plan_from_authority(
    workspace_root: &Path,
    state: &Value,
    payload: &Value,
    job: &RuntimeJobRecord,
    workspace_id: &str,
    work_item_id: &str,
    expected_revision: StateRevision,
    current_policy_digest: &str,
    current_inventory_generation: u64,
) -> RuntimeResult<VerifiedExecutionAuthority> {
    let input: VerificationPlanOperationInput = serde_json::from_value(payload.clone())
        .map_err(|_| schema_error("verification.plan payload is malformed"))?;
    if !input.persist || input.changed_paths.is_empty() {
        return Err(schema_error(
            "verification.plan requires persist=true and non-empty changedPaths",
        ));
    }
    validate_bounded_identity(&input.toolset_job_id, "toolsetJobId")?;
    validate_bounded_identity(&input.receipt_id, "receiptId")?;
    validate_plain_digest(&input.receipt_digest, "receiptDigest")?;
    validate_plain_digest(&input.plan_digest, "planDigest")?;
    validate_plain_digest(&input.methodology_digest, "methodologyDigest")?;
    validate_plain_digest(&input.policy_digest, "policyDigest")?;
    normalize_input_fingerprint(&input.input_fingerprint)?;

    ExecutionPolicy::validate_plan(&input.plan)
        .map_err(|_| schema_error("verification plan violates execution policy"))?;
    let canonical_plan = serde_json::to_value(&input.plan)
        .map_err(|_| schema_error("verification plan could not be canonicalized"))?;
    let actual_plan_digest = canonical_execution_plan_digest(&canonical_plan)?;
    if input.plan_digest != actual_plan_digest {
        return Err(schema_error(
            "verification.plan planDigest does not bind the complete plan",
        ));
    }
    let plan_input = canonical_plan
        .get("inputFingerprint")
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error("verification plan inputFingerprint is missing"))?;
    if normalize_input_fingerprint(plan_input)?
        != normalize_input_fingerprint(&input.input_fingerprint)?
    {
        return Err(schema_error(
            "verification.plan inputFingerprint does not match its plan",
        ));
    }

    let state_revision = state
        .get("revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| schema_error("authoritative state revision is missing"))?;
    if state_revision != expected_revision.get()
        || input.source_revision != state_revision
        || input.policy_digest != current_policy_digest
    {
        return Err(RuntimeError::new(
            StableErrorCode::StaleGateResult,
            "verification authority is stale for the locked project revision or policy",
        ));
    }

    validate_job(
        job,
        &input,
        workspace_id,
        work_item_id,
        current_inventory_generation,
    )?;
    let locator = job
        .receipt_locator
        .as_deref()
        .ok_or_else(uncommitted_job_error)?;
    let project_digest = job
        .project_receipt_digest
        .as_deref()
        .ok_or_else(uncommitted_job_error)?;
    let mutation_id = job
        .mutation_id
        .as_deref()
        .ok_or_else(uncommitted_job_error)?;
    validate_plain_digest(project_digest, "projectReceiptDigest")?;

    let state_ref: ToolsetReceiptRef = serde_json::from_value(
        state
            .get("toolsetReceiptRef")
            .cloned()
            .ok_or_else(|| schema_error("active toolsetReceiptRef is missing"))?,
    )
    .map_err(|_| schema_error("toolsetReceiptRef is malformed"))?;
    validate_state_ref(
        &state_ref,
        &input,
        locator,
        project_digest,
        mutation_id,
        state_revision,
        job.source_revision.ok_or_else(uncommitted_job_error)?,
    )?;

    let snapshot_bytes = read_project_file(workspace_root, locator, MAX_PROJECT_RECEIPT_BYTES)?;
    if ArtifactDigest::digest(&snapshot_bytes).to_string() != project_digest {
        return Err(external_conflict(
            "project toolset receipt digest does not match its committed locator",
        ));
    }
    let project: ProjectToolsetReceipt = serde_json::from_slice(&snapshot_bytes)
        .map_err(|_| external_conflict("project toolset receipt is malformed"))?;
    validate_project_receipt(
        &project,
        &input,
        job,
        workspace_id,
        work_item_id,
        mutation_id,
        current_inventory_generation,
    )?;
    validate_active_manifest(
        workspace_root,
        &state_ref,
        &project,
        locator,
        project_digest,
        work_item_id,
    )?;

    Ok(VerifiedExecutionAuthority {
        plan: canonical_plan,
        toolset_job_id: input.toolset_job_id,
        source_revision: input.source_revision,
        plan_digest: input.plan_digest,
        input_fingerprint: input.input_fingerprint,
        changed_paths: input.changed_paths.into_iter().map(Value::String).collect(),
        since_fingerprint: input.since_fingerprint.unwrap_or_default(),
    })
}

pub(crate) fn canonical_execution_plan_digest(candidate_plan: &Value) -> RuntimeResult<String> {
    let plan: VerificationExecutionPlan = serde_json::from_value(candidate_plan.clone())
        .map_err(|_| schema_error("verification plan is malformed"))?;
    let bytes = serde_json::to_vec(&plan)
        .map_err(|_| schema_error("verification plan could not be canonicalized"))?;
    Ok(ArtifactDigest::digest(bytes).to_string())
}

pub(crate) fn validate_verification_input_binding(
    execution_plan: &Value,
    verification_plan: &Value,
) -> RuntimeResult<()> {
    let execution_fingerprint = normalize_input_fingerprint(
        execution_plan
            .get("inputFingerprint")
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("verification plan inputFingerprint is missing"))?,
    )?;
    let verification_fingerprint = normalize_input_fingerprint(
        verification_plan
            .get("inputFingerprint")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                schema_error("generated verificationPlan inputFingerprint is missing")
            })?,
    )?;
    if execution_fingerprint != verification_fingerprint {
        return Err(schema_error(
            "verificationPlan inputFingerprint does not match the frozen execution plan",
        ));
    }
    Ok(())
}

fn validate_job(
    job: &RuntimeJobRecord,
    input: &VerificationPlanOperationInput,
    workspace_id: &str,
    work_item_id: &str,
    inventory_generation: u64,
) -> RuntimeResult<()> {
    match job.status {
        RuntimeJobStatus::Pass => {}
        RuntimeJobStatus::Stale => {
            return Err(RuntimeError::new(
                StableErrorCode::StaleGateResult,
                "toolset receipt job is stale",
            ));
        }
        RuntimeJobStatus::Error => {
            return Err(RuntimeError::new(
                StableErrorCode::GateError,
                "toolset receipt job ended with an error",
            ));
        }
        RuntimeJobStatus::Timeout => {
            return Err(RuntimeError::new(
                StableErrorCode::GateTimeout,
                "toolset receipt job timed out",
            ));
        }
        RuntimeJobStatus::Queued
        | RuntimeJobStatus::Running
        | RuntimeJobStatus::Fail
        | RuntimeJobStatus::Cancelled => return Err(uncommitted_job_error()),
    }
    if job.job_id != input.toolset_job_id
        || job.workspace_id != workspace_id
        || job.work_item_id.as_deref() != Some(work_item_id)
        || job.entrypoint != "toolset.receipt.record"
        || job.inventory_generation != inventory_generation
        || job.input_fingerprint.as_deref().is_none_or(|fingerprint| {
            normalize_input_fingerprint(fingerprint).ok()
                != normalize_input_fingerprint(&input.input_fingerprint).ok()
        })
    {
        return Err(RuntimeError::new(
            StableErrorCode::StaleGateResult,
            "runtime job scope or freshness does not match verification.plan",
        ));
    }
    let result = job
        .result
        .as_ref()
        .and_then(Value::as_object)
        .ok_or_else(uncommitted_job_error)?;
    for (field, expected) in [
        ("outcome", "PASS"),
        ("toolsetJobId", input.toolset_job_id.as_str()),
        ("receiptId", input.receipt_id.as_str()),
        ("receiptDigest", input.receipt_digest.as_str()),
        ("planDigest", input.plan_digest.as_str()),
        ("methodologyDigest", input.methodology_digest.as_str()),
        ("policyDigest", input.policy_digest.as_str()),
        ("workItemId", work_item_id),
    ] {
        if result.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(external_conflict(
                "runtime job result does not match verification.plan bindings",
            ));
        }
    }
    if result.get("validated").and_then(Value::as_bool) != Some(true)
        || result.get("sourceRevision").and_then(Value::as_u64) != job.source_revision
        || result.get("committedRevision").and_then(Value::as_u64) != Some(input.source_revision)
        || result.get("revisionAfter").and_then(Value::as_u64) != Some(input.source_revision)
        || result.get("inventoryGeneration").and_then(Value::as_u64) != Some(inventory_generation)
        || result
            .get("inputFingerprint")
            .and_then(Value::as_str)
            .is_none_or(|fingerprint| {
                normalize_input_fingerprint(fingerprint).ok()
                    != normalize_input_fingerprint(&input.input_fingerprint).ok()
            })
    {
        return Err(external_conflict(
            "runtime job result freshness does not match verification.plan",
        ));
    }
    Ok(())
}

fn validate_state_ref(
    state_ref: &ToolsetReceiptRef,
    input: &VerificationPlanOperationInput,
    locator: &str,
    project_digest: &str,
    mutation_id: &str,
    state_revision: u64,
    source_revision: u64,
) -> RuntimeResult<()> {
    validate_plain_digest(&state_ref.receipt_digest, "state receiptDigest")?;
    validate_plain_digest(
        &state_ref.project_receipt_digest,
        "state projectReceiptDigest",
    )?;
    validate_plain_digest(&state_ref.manifest_digest, "state manifestDigest")?;
    if state_ref.schema_version != 1
        || state_ref.toolset_job_id != input.toolset_job_id
        || state_ref.receipt_id != input.receipt_id
        || state_ref.receipt_digest != input.receipt_digest
        || state_ref.artifact_ref != locator
        || state_ref.project_receipt_digest != project_digest
        || state_ref.mutation_id != mutation_id
        || state_ref.source_revision != source_revision
        || state_ref.committed_revision != state_revision
    {
        return Err(external_conflict(
            "toolsetReceiptRef does not match the committed runtime job",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_project_receipt(
    project: &ProjectToolsetReceipt,
    input: &VerificationPlanOperationInput,
    job: &RuntimeJobRecord,
    workspace_id: &str,
    work_item_id: &str,
    mutation_id: &str,
    inventory_generation: u64,
) -> RuntimeResult<()> {
    validate_plain_digest(&project.receipt_digest, "project receiptDigest")?;
    validate_plain_digest(&project.plan_digest, "project planDigest")?;
    validate_plain_digest(&project.methodology_digest, "project methodologyDigest")?;
    validate_plain_digest(&project.policy_digest, "project policyDigest")?;
    validate_plain_digest(&project.identity_digest, "project identityDigest")?;
    if project.schema_version != 1
        || project.kind != TOOLSET_RECEIPT_KIND
        || project.toolset_job_id != input.toolset_job_id
        || project.workspace_id != workspace_id
        || project.work_item_id != work_item_id
        || project.outcome != "PASS"
        || project.status != "pass"
        || !project.validated
        || project.receipt_id != input.receipt_id
        || project.receipt_digest != input.receipt_digest
        || project.plan_digest != input.plan_digest
        || project.methodology_digest != input.methodology_digest
        || project.policy_digest != input.policy_digest
        || project.source_revision != job.source_revision.unwrap_or_default()
        || project.committed_revision != input.source_revision
        || project.inventory_generation != inventory_generation
        || project.mutation_id != mutation_id
        || job.identity_digest.as_deref() != Some(project.identity_digest.as_str())
        || job.session_id.as_deref() != Some(project.recorder.session_id.as_str())
        || job.root_session_id.as_deref() != Some(project.recorder.root_session_id.as_str())
        || job.delegation_id.as_deref() != project.recorder.delegation_id.as_deref()
        || job.context_generation != Some(project.recorder.context_generation)
    {
        return Err(external_conflict(
            "project toolset receipt does not match runtime job authority",
        ));
    }
    let project_plan = serde_json::to_value(&project.plan)
        .map_err(|_| external_conflict("project plan could not be canonicalized"))?;
    let expected_plan = serde_json::to_value(&input.plan)
        .map_err(|_| schema_error("verification plan could not be canonicalized"))?;
    if project_plan != expected_plan
        || canonical_execution_plan_digest(&project_plan)? != input.plan_digest
        || normalize_input_fingerprint(&project.input_fingerprint)?
            != normalize_input_fingerprint(&input.input_fingerprint)?
    {
        return Err(external_conflict(
            "project receipt plan does not match verification.plan",
        ));
    }
    validate_against_plan(&project.plan, &project.receipt)
        .map_err(|_| external_conflict("project verification receipt does not match its plan"))?;
    if project.receipt.status() != JobStatus::Pass {
        return Err(RuntimeError::new(
            StableErrorCode::GateBlocked,
            "project verification receipt is not PASS",
        ));
    }
    let receipt_value = serde_json::to_value(&project.receipt)
        .map_err(|_| external_conflict("project receipt could not be canonicalized"))?;
    let actual_receipt_digest = ArtifactDigest::digest(
        serde_json::to_vec(&receipt_value)
            .map_err(|_| external_conflict("project receipt could not be canonicalized"))?,
    )
    .to_string();
    if actual_receipt_digest != input.receipt_digest {
        return Err(external_conflict(
            "project receiptDigest does not bind the verification receipt",
        ));
    }
    Ok(())
}

fn validate_active_manifest(
    workspace_root: &Path,
    state_ref: &ToolsetReceiptRef,
    project: &ProjectToolsetReceipt,
    locator: &str,
    project_digest: &str,
    work_item_id: &str,
) -> RuntimeResult<()> {
    let manifest_bytes = read_project_file(workspace_root, &state_ref.manifest_ref, 262_144)?;
    if ArtifactDigest::digest(&manifest_bytes).to_string() != state_ref.manifest_digest {
        return Err(external_conflict(
            "active evidence manifest digest does not match toolsetReceiptRef",
        ));
    }
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| external_conflict("active evidence manifest is malformed"))?;
    if manifest.get("schemaVersion").and_then(Value::as_u64) != Some(1)
        || manifest.get("storyId").and_then(Value::as_str) != Some(work_item_id)
    {
        return Err(external_conflict(
            "active evidence manifest scope does not match the Work Item",
        ));
    }
    let entries = manifest
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| external_conflict("active evidence manifest entries are missing"))?;
    let matching = entries
        .iter()
        .filter(|entry| {
            entry.get("evidenceId").and_then(Value::as_str) == Some(project.receipt_id.as_str())
        })
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(external_conflict(
            "active evidence manifest has missing or duplicate toolset receipt entries",
        ));
    }
    let entry = matching[0];
    let artifact = entry
        .get("artifacts")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .ok_or_else(|| external_conflict("toolset manifest artifact is missing"))?;
    let expected_sha = format!("sha256:{project_digest}");
    if entry.get("status").and_then(Value::as_str) != Some("active")
        || entry.get("kind").and_then(Value::as_str) != Some("toolset-receipt")
        || entry.get("toolsetJobId").and_then(Value::as_str)
            != Some(project.toolset_job_id.as_str())
        || entry.get("workItemId").and_then(Value::as_str) != Some(work_item_id)
        || entry.get("receiptDigest").and_then(Value::as_str)
            != Some(project.receipt_digest.as_str())
        || entry.get("planDigest").and_then(Value::as_str) != Some(project.plan_digest.as_str())
        || entry.get("sourceRevision").and_then(Value::as_u64) != Some(project.source_revision)
        || entry.get("inventoryGeneration").and_then(Value::as_u64)
            != Some(project.inventory_generation)
        || entry.get("recorderSessionId").and_then(Value::as_str)
            != Some(project.recorder.session_id.as_str())
        || artifact.get("snapshotPath").and_then(Value::as_str) != Some(locator)
        || artifact.get("sha256").and_then(Value::as_str) != Some(expected_sha.as_str())
    {
        return Err(external_conflict(
            "toolset manifest entry does not match the project receipt",
        ));
    }
    Ok(())
}

fn read_project_file(root: &Path, relative: &str, limit: usize) -> RuntimeResult<Vec<u8>> {
    let relative = ProjectRelativePath::new(relative.to_owned())
        .map_err(|_| external_conflict("project authority locator is unsafe"))?;
    let canonical_root = fs::canonicalize(root)
        .map_err(|_| external_conflict("workspace root could not be canonicalized"))?;
    let absolute = fs::canonicalize(canonical_root.join(relative.as_str()))
        .map_err(|_| external_conflict("project authority locator does not exist"))?;
    if !absolute.is_file() || absolute.strip_prefix(&canonical_root).is_err() {
        return Err(external_conflict(
            "project authority locator escaped the workspace",
        ));
    }
    let bytes =
        fs::read(absolute).map_err(|_| external_conflict("project authority could not be read"))?;
    if bytes.is_empty() || bytes.len() > limit {
        return Err(external_conflict(
            "project authority exceeds its durable byte bound",
        ));
    }
    Ok(bytes)
}

fn normalize_input_fingerprint(value: &str) -> RuntimeResult<&str> {
    let digest = value.strip_prefix("sha256:").unwrap_or(value);
    validate_plain_digest(digest, "inputFingerprint")?;
    Ok(digest)
}

fn validate_plain_digest(value: &str, field: &str) -> RuntimeResult<()> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(schema_error(&format!(
            "{field} must be canonical lowercase sha256 hex"
        )));
    }
    Ok(())
}

fn validate_bounded_identity(value: &str, field: &str) -> RuntimeResult<()> {
    if value.trim().is_empty() || value.len() > 128 {
        return Err(schema_error(&format!("{field} is missing or oversized")));
    }
    Ok(())
}

fn uncommitted_job_error() -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::GateBlocked,
        "toolset receipt job is not a committed PASS authority",
    )
}

fn external_conflict(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::ExternalStateConflict, message)
}

fn schema_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::OperationSchemaInvalid, message)
}

// ---------------------------------------------------------------------------
// Authoritative `execution.resume`
//
// One resume call resolves the approved plan and the four required contexts
// (story, constraints, thinking engine, verification) from a single project
// authority snapshot plus a single required-context bundle load.  Every later
// check inside the call reuses those in-call values, so the authority refresh
// count stays at one.  An unapproved plan or any digest drift fails closed
// with `EXECUTION_CAPSULE_STALE` and never writes an artifact.
// ---------------------------------------------------------------------------

/// Byte bound for reading one committed capsule artifact.
const MAX_CAPSULE_ARTIFACT_BYTES: usize = 32 * 1024;
/// Byte bound for reading one committed queue artifact.
const MAX_EXECUTION_QUEUE_BYTES: usize = 262_144;
/// Byte bound for reading the execution ledger.
const MAX_EXECUTION_LEDGER_BYTES: usize = 262_144;
/// Byte bound for one required-context file.
const MAX_CONTEXT_FILE_BYTES: usize = 262_144;
/// Maximum number of files the constraints bundle may contain.
const MAX_CONSTRAINTS_FILES: usize = 64;
/// Project-relative constraints bundle locator.
const CONSTRAINTS_BUNDLE_PATH: &str = "constraints";
/// Project-relative v1 coding thinking-engine locator.
const THINKING_ENGINE_PATH: &str = "source/standards/thinking/be-coding-thinking-engine.md";

/// Decoded `execution.resume` cursor: the caller's last known capsule digest
/// and context revision.  Anything else in the payload is rejected.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExecutionResumeWire {
    #[serde(default)]
    known_capsule_digest: Option<String>,
    #[serde(default)]
    known_context_revision: Option<u64>,
}

/// Typed `execution.resume` request cursor.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ExecutionResumeRequest {
    known_capsule_digest: Option<ArtifactDigest>,
    known_context_revision: Option<u64>,
}

impl ExecutionResumeRequest {
    /// Returns the caller's last known capsule digest, when supplied.
    pub(crate) const fn known_capsule_digest(&self) -> Option<ArtifactDigest> {
        self.known_capsule_digest
    }

    /// Returns the caller's last known context revision, when supplied.
    pub(crate) const fn known_context_revision(&self) -> Option<u64> {
        self.known_context_revision
    }
}

/// Decodes and validates the `execution.resume` payload.
pub(crate) fn decode_execution_resume_payload(
    payload: &Value,
) -> RuntimeResult<ExecutionResumeRequest> {
    let wire: ExecutionResumeWire = serde_json::from_value(payload.clone())
        .map_err(|_| schema_error("execution.resume payload is malformed"))?;
    let known_capsule_digest = wire
        .known_capsule_digest
        .as_deref()
        .map(|value| parse_prefixed_digest(value, "knownCapsuleDigest"))
        .transpose()?;
    Ok(ExecutionResumeRequest {
        known_capsule_digest,
        known_context_revision: wire.known_context_revision,
    })
}

/// Approved execution-plan authority: the canonical plan value plus the
/// digest that binds the complete approved plan.
#[derive(Clone, Debug)]
pub(crate) struct ApprovedPlanAuthority {
    plan: Value,
    plan_digest: ArtifactDigest,
}

impl ApprovedPlanAuthority {
    /// Returns the canonical approved plan value.
    pub(crate) fn plan(&self) -> &Value {
        &self.plan
    }

    /// Returns the digest binding the complete approved plan.
    pub(crate) const fn plan_digest(&self) -> ArtifactDigest {
        self.plan_digest
    }
}

/// Verifies that the snapshot's `executionPlan` is user-approved and binds it
/// by digest.  An absent or unapproved plan fails closed with
/// `EXECUTION_CAPSULE_STALE`.
pub(crate) fn approved_plan_authority(state: &Value) -> RuntimeResult<ApprovedPlanAuthority> {
    let plan = state
        .get("executionPlan")
        .cloned()
        .ok_or_else(|| capsule_stale("approved executionPlan is missing"))?;
    let object = plan
        .as_object()
        .ok_or_else(|| capsule_stale("approved executionPlan is malformed"))?;
    if object.get("approved").and_then(Value::as_bool) != Some(true)
        || object
            .get("approvedBy")
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        || object
            .get("approvedAt")
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
    {
        return Err(capsule_stale("executionPlan is not approved by the user"));
    }
    if object
        .get("goal")
        .and_then(Value::as_str)
        .is_none_or(|value| value.trim().is_empty())
        || object
            .get("changedPaths")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
        || object
            .get("verification")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    {
        return Err(capsule_stale(
            "approved executionPlan lacks goal, changedPaths or verification",
        ));
    }
    let bytes = serde_json::to_vec(&plan)
        .map_err(|_| schema_error("approved executionPlan could not be canonicalized"))?;
    Ok(ApprovedPlanAuthority {
        plan,
        plan_digest: ArtifactDigest::digest(bytes),
    })
}

/// The four required execution contexts with content-addressed references,
/// loaded exactly once per resume call.
#[derive(Clone, Debug)]
pub(crate) struct RequiredContextBundle {
    story_ref: ArtifactRef,
    constraints_ref: ArtifactRef,
    thinking_engine_ref: ArtifactRef,
    verification_ref: ArtifactRef,
}

impl RequiredContextBundle {
    /// Returns the content-addressed story reference.
    pub(crate) const fn story_ref(&self) -> &ArtifactRef {
        &self.story_ref
    }

    /// Returns the content-addressed constraints reference.
    pub(crate) const fn constraints_ref(&self) -> &ArtifactRef {
        &self.constraints_ref
    }

    /// Returns the content-addressed thinking-engine reference.
    pub(crate) const fn thinking_engine_ref(&self) -> &ArtifactRef {
        &self.thinking_engine_ref
    }

    /// Returns the content-addressed verification-contract reference.
    pub(crate) const fn verification_ref(&self) -> &ArtifactRef {
        &self.verification_ref
    }
}

/// Loads the four required contexts in one bounded pass.  Each artifact is
/// read once; the resulting refs are the only values later checks may use.
pub(crate) fn load_required_context_bundle(
    workspace_root: &Path,
    state: &Value,
    work_item_id: &str,
    plan: &Value,
) -> RuntimeResult<RequiredContextBundle> {
    let story_locator = story_context_locator(state, work_item_id)?;
    let story_locator = relativize_story_locator(workspace_root, &story_locator)?;
    let story_bytes =
        read_execution_artifact(workspace_root, &story_locator, MAX_CONTEXT_FILE_BYTES)?;
    let story_path = ProjectRelativePath::new(story_locator)
        .map_err(|_| capsule_stale("story context locator is not project-relative"))?;
    let story_ref = ArtifactRef::new(
        artifact_kind("execution-context-story")?,
        story_path,
        ArtifactDigest::digest(&story_bytes),
        story_bytes.len() as u64,
    );

    let constraints_ref = constraints_bundle_ref(workspace_root)?;

    let thinking_bytes =
        read_execution_artifact(workspace_root, THINKING_ENGINE_PATH, MAX_CONTEXT_FILE_BYTES)?;
    let thinking_engine_ref = ArtifactRef::new(
        artifact_kind("execution-context-thinking-engine")?,
        ProjectRelativePath::new(THINKING_ENGINE_PATH.to_owned())
            .map_err(|_| capsule_stale("thinking engine locator is not project-relative"))?,
        ArtifactDigest::digest(&thinking_bytes),
        thinking_bytes.len() as u64,
    );

    // The verification contract is the machine-readable `verification` array
    // of the approved plan; the story document stays its human locator.
    let verification_bytes = serde_json::to_vec(
        plan.get("verification")
            .ok_or_else(|| capsule_stale("approved executionPlan verification is missing"))?,
    )
    .map_err(|_| schema_error("verification contract could not be canonicalized"))?;
    let verification_ref = ArtifactRef::new(
        artifact_kind("execution-context-verification")?,
        story_ref.path().clone(),
        ArtifactDigest::digest(&verification_bytes),
        verification_bytes.len() as u64,
    );

    Ok(RequiredContextBundle {
        story_ref,
        constraints_ref,
        thinking_engine_ref,
        verification_ref,
    })
}

/// Project-relative locators of the execution authority artifacts.
#[derive(Clone, Debug)]
pub(crate) struct ExecutionArtifactLocators {
    queue: ProjectRelativePath,
    capsule: ProjectRelativePath,
    ledger: ProjectRelativePath,
}

impl ExecutionArtifactLocators {
    /// Returns the queue artifact locator.
    pub(crate) fn queue(&self) -> &ProjectRelativePath {
        &self.queue
    }

    /// Returns the capsule artifact locator.
    pub(crate) fn capsule(&self) -> &ProjectRelativePath {
        &self.capsule
    }

    /// Returns the ledger artifact locator.
    pub(crate) fn ledger(&self) -> &ProjectRelativePath {
        &self.ledger
    }
}

/// Resolves the conventional execution artifact locators for one Work Item.
pub(crate) fn execution_artifact_locators(
    work_item_id: &str,
) -> RuntimeResult<ExecutionArtifactLocators> {
    let locator = |name: &str| {
        ProjectRelativePath::new(format!(".auto-engineering/{work_item_id}/execution/{name}"))
            .map_err(|_| capsule_stale("execution artifact locator is not project-relative"))
    };
    Ok(ExecutionArtifactLocators {
        queue: locator("queue.json")?,
        capsule: locator("capsule.json")?,
        ledger: locator("ledger.jsonl")?,
    })
}

/// Builds the deterministic queue artifact and active-slice capsule from the
/// approved plan and the already loaded context bundle.
pub(crate) fn build_capsule_from_authority(
    state: &Value,
    work_item_id: &str,
    source_revision: StateRevision,
    active_ordinal: u32,
    plan: &ApprovedPlanAuthority,
    bundle: &RequiredContextBundle,
    policy_digest: &str,
    inventory_generation: u64,
) -> RuntimeResult<CapsuleBuildOutcome> {
    let locators = execution_artifact_locators(work_item_id)?;
    let input = CapsuleBuildInputV1 {
        work_item_id: WorkItemId::new(work_item_id.to_owned())
            .map_err(|_| schema_error("workItemId cannot bind an execution capsule"))?,
        story_id: StoryId::new(story_identity(state, work_item_id))
            .map_err(|_| capsule_stale("active story identity cannot bind an execution capsule"))?,
        source_revision,
        approved_plan_digest: plan.plan_digest(),
        policy_digest: PolicyDigest::from_str(policy_digest)
            .map_err(|_| schema_error("daemon policy digest is not canonical"))?,
        inventory_generation: InventoryGeneration::new(inventory_generation),
        story_ref: bundle.story_ref().clone(),
        constraints_ref: bundle.constraints_ref().clone(),
        thinking_engine_ref: bundle.thinking_engine_ref().clone(),
        verification_ref: bundle.verification_ref().clone(),
        queue_artifact_kind: artifact_kind("execution-queue")?,
        queue_artifact_path: locators.queue().clone(),
        slices: derive_slice_specs(plan.plan(), work_item_id)?,
        active_ordinal,
        budgets: ExecutionBudgetsV1::default(),
    };
    build_execution_capsule(&input).map_err(|error| match error {
        ExecutionCapsuleBuildError::Contract(ExecutionCapsuleError::CapsuleBudgetExceeded {
            max_bytes,
            actual_bytes,
        }) => RuntimeError::new(
            StableErrorCode::ExecutionBudgetExceeded,
            format!("encoded execution capsule exceeds the {max_bytes}-byte budget (actual: {actual_bytes})"),
        ),
        other => capsule_stale(format!(
            "approved executionPlan cannot produce a deterministic slice queue: {other}"
        )),
    })
}

/// The canonical first line of the append-only execution ledger, binding the
/// seeded queue and capsule digests.
pub(crate) fn ledger_seed_bytes(
    outcome: &CapsuleBuildOutcome,
    plan_digest: ArtifactDigest,
) -> RuntimeResult<Vec<u8>> {
    let seed = json!({
        "schemaVersion": 1,
        "kind": "execution-ledger-seed",
        "workItemId": outcome.capsule().work_item_id().as_str(),
        "storyId": outcome.capsule().story_id().as_str(),
        "approvedPlanDigest": plan_digest.to_string(),
        "queueDigest": outcome.queue_digest().to_string(),
        "capsuleDigest": outcome.capsule_digest().to_string(),
        "activeSliceOrdinal": outcome.capsule().queue().active_ordinal(),
    });
    let mut bytes = serde_json::to_vec(&seed)
        .map_err(|_| schema_error("execution ledger seed could not be canonicalized"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// The `executionRuntime` state section committed on first generation: only
/// locators, digests and the execution cursor, never artifact bodies.
pub(crate) fn execution_runtime_state_section(
    outcome: &CapsuleBuildOutcome,
    locators: &ExecutionArtifactLocators,
    ledger_digest: ArtifactDigest,
) -> Value {
    json!({
        "schemaVersion": 1,
        "capsuleRef": locators.capsule().as_str(),
        "capsuleDigest": format!("sha256:{}", outcome.capsule_digest()),
        "queueRef": locators.queue().as_str(),
        "queueDigest": format!("sha256:{}", outcome.queue_digest()),
        "ledgerRef": locators.ledger().as_str(),
        "ledgerDigest": format!("sha256:{ledger_digest}"),
        "activeSliceOrdinal": outcome.capsule().queue().active_ordinal(),
        "activeSliceStatus": "pending",
        "refactorCycle": "idle",
        "completionMilestone": "none",
    })
}

/// A committed capsule verified fresh against the current authority snapshot
/// and the in-call context bundle.
#[derive(Clone, Debug)]
pub(crate) struct CommittedExecutionCapsule {
    capsule: ExecutionCapsuleV1,
    capsule_digest: ArtifactDigest,
}

impl CommittedExecutionCapsule {
    /// Returns the verified committed capsule.
    pub(crate) const fn capsule(&self) -> &ExecutionCapsuleV1 {
        &self.capsule
    }

    /// Returns the verified digest of the committed capsule artifact.
    pub(crate) const fn capsule_digest(&self) -> ArtifactDigest {
        self.capsule_digest
    }
}

/// Verifies the committed capsule chain: state locator/digest section,
/// capsule artifact, queue artifact, ledger and all four required context
/// refs.  Any drift fails closed with `EXECUTION_CAPSULE_STALE`; nothing is
/// written on this path.
pub(crate) fn verify_committed_capsule(
    workspace_root: &Path,
    state: &Value,
    plan: &ApprovedPlanAuthority,
    bundle: &RequiredContextBundle,
    current_policy_digest: &str,
) -> RuntimeResult<CommittedExecutionCapsule> {
    let runtime = state
        .get("executionRuntime")
        .ok_or_else(|| capsule_stale("committed execution runtime is missing"))?;
    let object = runtime
        .as_object()
        .ok_or_else(|| capsule_stale("executionRuntime section is malformed"))?;
    if object.get("schemaVersion").and_then(Value::as_u64) != Some(1) {
        return Err(capsule_stale(
            "executionRuntime schemaVersion is unsupported",
        ));
    }
    if !matches!(
        object.get("completionMilestone").and_then(Value::as_str),
        Some("none" | "implementation-verified" | "review-ready" | "governance-closed")
    ) {
        return Err(capsule_stale(
            "executionRuntime completion milestone is unsupported",
        ));
    }
    let capsule_locator = required_string(object, "capsuleRef")?;
    let capsule_digest = parse_prefixed_digest(
        required_string(object, "capsuleDigest")?,
        "executionRuntime.capsuleDigest",
    )?;
    let queue_locator = required_string(object, "queueRef")?;
    let queue_digest = parse_prefixed_digest(
        required_string(object, "queueDigest")?,
        "executionRuntime.queueDigest",
    )?;
    let ledger_locator = required_string(object, "ledgerRef")?;
    let ledger_digest = parse_prefixed_digest(
        required_string(object, "ledgerDigest")?,
        "executionRuntime.ledgerDigest",
    )?;
    let active_ordinal = object
        .get("activeSliceOrdinal")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| capsule_stale("executionRuntime activeSliceOrdinal is malformed"))?;

    let capsule_bytes =
        read_execution_artifact(workspace_root, capsule_locator, MAX_CAPSULE_ARTIFACT_BYTES)?;
    if ArtifactDigest::digest(&capsule_bytes) != capsule_digest {
        return Err(capsule_stale("execution capsule artifact digest drifted"));
    }
    let capsule: ExecutionCapsuleV1 = serde_json::from_slice(&capsule_bytes)
        .map_err(|_| capsule_stale("committed execution capsule is malformed"))?;
    if capsule.approved_plan_digest() != plan.plan_digest() {
        return Err(capsule_stale("approved plan digest drifted"));
    }
    if capsule.policy_digest().to_string() != current_policy_digest {
        return Err(capsule_stale("policy digest drifted"));
    }
    if capsule.story_ref() != bundle.story_ref() {
        return Err(capsule_stale("story context digest drifted"));
    }
    if capsule.constraints_ref() != bundle.constraints_ref() {
        return Err(capsule_stale("constraints context digest drifted"));
    }
    if capsule.thinking_engine_ref() != bundle.thinking_engine_ref() {
        return Err(capsule_stale("thinking engine context digest drifted"));
    }
    if capsule.verification_ref() != bundle.verification_ref() {
        return Err(capsule_stale("verification context digest drifted"));
    }

    let queue_bytes =
        read_execution_artifact(workspace_root, queue_locator, MAX_EXECUTION_QUEUE_BYTES)?;
    if ArtifactDigest::digest(&queue_bytes) != queue_digest
        || capsule.queue().queue_digest() != queue_digest
        || capsule.queue().artifact().digest() != queue_digest
        || capsule.queue().artifact().path().as_str() != queue_locator
    {
        return Err(capsule_stale("execution queue artifact digest drifted"));
    }
    if capsule.queue().active_ordinal() != active_ordinal {
        return Err(capsule_stale("execution cursor drifted"));
    }
    let ledger_bytes =
        read_execution_artifact(workspace_root, ledger_locator, MAX_EXECUTION_LEDGER_BYTES)?;
    if ArtifactDigest::digest(&ledger_bytes) != ledger_digest {
        return Err(capsule_stale("execution ledger digest drifted"));
    }
    Ok(CommittedExecutionCapsule {
        capsule,
        capsule_digest,
    })
}

/// Resolves the story context locator from the snapshot: the active story's
/// authoritative `docPath`, falling back to the `documentPaths.STORY`
/// intent.  The locator may be absolute; the caller relativizes it against
/// the workspace root.
fn story_context_locator(state: &Value, work_item_id: &str) -> RuntimeResult<String> {
    let story_id = story_identity(state, work_item_id);
    state
        .get("storyStates")
        .and_then(Value::as_object)
        .and_then(|stories| stories.get(&story_id))
        .and_then(|story| story.get("docPath"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            state
                .get("documentPaths")
                .and_then(Value::as_object)
                .and_then(|paths| paths.get("STORY"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .ok_or_else(|| capsule_stale("story context locator is missing"))
}

/// Relativizes an absolute story `docPath` against the canonical workspace
/// root; relative locators pass through after normalization.
pub(crate) fn relativize_story_locator(
    workspace_root: &Path,
    locator: &str,
) -> RuntimeResult<String> {
    let path = Path::new(locator);
    if !path.is_absolute() {
        return Ok(locator.replace('\\', "/"));
    }
    let root = fs::canonicalize(workspace_root)
        .map_err(|_| external_conflict("workspace root could not be canonicalized"))?;
    let absolute = fs::canonicalize(path)
        .map_err(|_| capsule_stale("story context locator does not exist"))?;
    let relative = absolute
        .strip_prefix(&root)
        .map_err(|_| capsule_stale("story context locator escaped the workspace"))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

/// Returns the active story identity for capsule binding.
fn story_identity(state: &Value, work_item_id: &str) -> String {
    state
        .get("activeStory")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(work_item_id)
        .to_owned()
}

/// Computes the deterministic constraints bundle reference: every regular
/// file directly inside `constraints/`, canonically ordered, folded into one
/// digest.  The bundle is loaded once per resume call.
fn constraints_bundle_ref(workspace_root: &Path) -> RuntimeResult<ArtifactRef> {
    let root = fs::canonicalize(workspace_root)
        .map_err(|_| external_conflict("workspace root could not be canonicalized"))?;
    let directory = root.join(CONSTRAINTS_BUNDLE_PATH);
    let mut entries = Vec::new();
    for entry in
        fs::read_dir(&directory).map_err(|_| capsule_stale("constraints directory is missing"))?
    {
        let entry = entry.map_err(|_| capsule_stale("constraints directory is unreadable"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let bytes =
            fs::read(&path).map_err(|_| capsule_stale("constraints bundle file is unreadable"))?;
        if bytes.is_empty() || bytes.len() > MAX_CONTEXT_FILE_BYTES {
            return Err(capsule_stale(
                "constraints bundle file exceeds its durable byte bound",
            ));
        }
        entries.push((name, ArtifactDigest::digest(&bytes), bytes.len() as u64));
        if entries.len() > MAX_CONSTRAINTS_FILES {
            return Err(capsule_stale("constraints bundle exceeds its file budget"));
        }
    }
    if entries.is_empty() {
        return Err(capsule_stale("constraints bundle is empty"));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut bundle_input = Vec::new();
    let mut total_bytes = 0_u64;
    for (name, digest, len) in &entries {
        bundle_input.extend_from_slice(name.as_bytes());
        bundle_input.push(0);
        bundle_input.extend_from_slice(digest.as_bytes());
        bundle_input.push(0);
        total_bytes = total_bytes.saturating_add(*len);
    }
    Ok(ArtifactRef::new(
        artifact_kind("execution-context-constraints")?,
        ProjectRelativePath::new(CONSTRAINTS_BUNDLE_PATH.to_owned())
            .map_err(|_| capsule_stale("constraints bundle locator is not project-relative"))?,
        ArtifactDigest::digest(&bundle_input),
        total_bytes,
    ))
}

/// Derives one slice spec per approved-plan verification entry, in plan
/// order, chained by dependency. Approved paths are distributed without
/// widening authority, and source reads outside each slice scope are dropped.
fn derive_slice_specs(
    plan: &Value,
    work_item_id: &str,
) -> RuntimeResult<Vec<ExecutionSliceSpecV1>> {
    let verification = plan
        .get("verification")
        .and_then(Value::as_array)
        .filter(|entries| !entries.is_empty())
        .ok_or_else(|| capsule_stale("approved executionPlan verification is missing"))?;
    let path_scopes = distribute_path_scopes(
        plan.get("changedPaths")
            .and_then(Value::as_array)
            .ok_or_else(|| capsule_stale("approved executionPlan changedPaths is missing"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|path| !path.trim().is_empty())
                    .ok_or_else(|| {
                        capsule_stale("approved executionPlan changedPaths must be strings")
                    })
                    .and_then(|path| {
                        ProjectRelativePath::new(path.to_owned()).map_err(|_| {
                            capsule_stale("approved executionPlan changedPath is unsafe")
                        })
                    })
            })
            .collect::<RuntimeResult<Vec<_>>>()?,
        verification.len(),
    )?;
    let source_read_paths = plan
        .get("sourceReads")
        .and_then(Value::as_array)
        .map(|reads| {
            reads
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|read| ProjectRelativePath::new(read.to_owned()).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut verification_ids = Vec::with_capacity(verification.len());
    for entry in verification {
        let id = entry
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| capsule_stale("verification entry is missing its id"))?;
        verification_ids.push(
            VerificationId::new(id.to_owned())
                .map_err(|_| capsule_stale("verification entry id is not a valid identity"))?,
        );
    }

    let mut specs = Vec::with_capacity(verification.len());
    for (index, entry) in verification.iter().enumerate() {
        let path_scope = path_scopes[index].clone();
        let source_reads = source_read_paths
            .iter()
            .filter(|read| path_scope.iter().any(|scope| scope.contains(read)))
            .cloned()
            .map(|read| SourceReadSpecV1::new(read, None, None))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| capsule_stale("approved executionPlan sourceReads are invalid"))?;
        let verification_id = verification_ids[index].clone();
        let objective = entry
            .get("expected")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                entry
                    .get("expect")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
            })
            .or_else(|| {
                entry
                    .get("command")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
            })
            .ok_or_else(|| capsule_stale("verification entry has no objective"))?;
        let slice_id = ExecutionSliceId::new(format!("slice-{}", verification_id.as_str()))
            .map_err(|_| capsule_stale("verification entry cannot bind a slice identity"))?;
        let depends_on = if index == 0 {
            Vec::new()
        } else {
            vec![
                ExecutionSliceId::new(format!("slice-{}", verification_ids[index - 1].as_str()))
                    .map_err(|_| {
                        capsule_stale("verification entry cannot bind a slice identity")
                    })?,
            ]
        };
        let ordinal = u32::try_from(index + 1)
            .map_err(|_| capsule_stale("verification entries exceed the queue ordinal range"))?;
        specs.push(ExecutionSliceSpecV1 {
            slice_id,
            ordinal,
            objective: objective.to_owned().into_boxed_str(),
            depends_on,
            path_scope,
            source_reads,
            focused_verification_id: verification_id.clone(),
            broad_verification_ids: verification_ids
                .iter()
                .filter(|candidate| **candidate != verification_id)
                .cloned()
                .collect(),
            evidence_logical_key: format!("execution/{work_item_id}/{}", verification_id.as_str())
                .into_boxed_str(),
        });
    }
    Ok(specs)
}

fn distribute_path_scopes(
    mut scopes: Vec<ProjectRelativePath>,
    slice_count: usize,
) -> RuntimeResult<Vec<Vec<ProjectRelativePath>>> {
    scopes.sort_unstable();
    scopes.dedup();
    if scopes.is_empty() || slice_count == 0 {
        return Err(capsule_stale(
            "approved executionPlan requires changedPaths and verification entries",
        ));
    }
    if scopes.len() > slice_count.saturating_mul(MAX_SLICE_PATH_SCOPE) {
        return Err(capsule_stale(
            "approved executionPlan changedPaths cannot fit the frozen v1 path-scope limit",
        ));
    }

    let mut assignments = vec![Vec::new(); slice_count];
    for (index, scope) in scopes.iter().cloned().enumerate() {
        assignments[index % slice_count].push(scope);
    }
    for index in scopes.len()..slice_count {
        assignments[index].push(scopes[index % scopes.len()].clone());
    }
    Ok(assignments)
}

/// Reads one committed execution artifact with containment and byte bounds,
/// mapping every failure to the fail-closed stale code.
fn read_execution_artifact(
    workspace_root: &Path,
    locator: &str,
    limit: usize,
) -> RuntimeResult<Vec<u8>> {
    read_project_file(workspace_root, locator, limit).map_err(|error| {
        capsule_stale(format!(
            "committed execution artifact cannot be verified: {}",
            error.message()
        ))
    })
}

fn required_string<'a>(object: &'a Map<String, Value>, field: &str) -> RuntimeResult<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| capsule_stale(format!("executionRuntime {field} is missing")))
}

fn parse_prefixed_digest(value: &str, field: &str) -> RuntimeResult<ArtifactDigest> {
    ArtifactDigest::from_str(value.strip_prefix("sha256:").unwrap_or(value))
        .map_err(|_| capsule_stale(format!("{field} is not a canonical digest")))
}

fn artifact_kind(value: &str) -> RuntimeResult<ArtifactKind> {
    ArtifactKind::new(value).map_err(|_| schema_error("execution artifact kind is invalid"))
}

fn capsule_stale(message: impl Into<String>) -> RuntimeError {
    RuntimeError::new(StableErrorCode::ExecutionCapsuleStale, message)
}
