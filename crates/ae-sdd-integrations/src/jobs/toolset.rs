//! Native `toolset.required` and `toolset.receipt.record` jobs.
//!
//! `toolset.required` derives a bounded toolset requirement from a Methodology
//! dependency reference and an input fingerprint.
//! `toolset.receipt.record` validates a `VerificationReceipt`'s identity fields
//! against an originating `VerificationExecutionPlan` using the pure
//! `ae-sdd-execution` policy layer.

use std::str::FromStr;

use ae_sdd_contracts::{MethodologyRef, SchemaVersion, VerificationContractId};
use ae_sdd_domain::{ArtifactDigest, InputFingerprint, WorkItemId};
use ae_sdd_execution::{
    ExecutionPolicy, ToolsetPort, ToolsetQuery, ToolsetRequirement, VerificationExecutionPlan,
    VerificationReceipt,
};
use ae_sdd_runtime::{BoundJobIdentity, RuntimeResult};
use serde::Deserialize;
use serde_json::{Value, json};

use super::common::{JobContext, schema_error};

const MAX_CANONICAL_BODY_BYTES: usize = 65_536;

#[derive(Clone, Copy, Debug, Default)]
struct NativeToolset;

impl ToolsetPort for NativeToolset {
    fn require(
        &self,
        query: &ToolsetQuery,
    ) -> Result<ToolsetRequirement, ae_sdd_contracts::ControlPlaneError> {
        Ok(ToolsetRequirement::new(
            query.schema_version,
            query.verification_contract_id.clone(),
            query.input_fingerprint,
            query.methodology_ref.entry_digest(),
            query.mandatory,
        ))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequirementArguments {
    schema_version: SchemaVersion,
    methodology_ref: MethodologyRef,
    methodology_digest: String,
    verification_contract_id: VerificationContractId,
    work_item_id: String,
    input_fingerprint: String,
    mandatory: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReceiptArguments {
    plan: VerificationExecutionPlan,
    receipt: VerificationReceipt,
    source_revision: u64,
    policy_digest: String,
    methodology_digest: String,
    inventory_generation: u64,
    lease_id: String,
    fencing_token: u64,
}

/// Pure validation of a `toolset.required` argument set.
pub fn validate_required(arguments: &Value) -> RuntimeResult<()> {
    let wire: RequirementArguments = serde_json::from_value(arguments.clone())
        .map_err(|_| schema_error("toolset.required arguments are malformed"))?;
    let methodology_digest = ArtifactDigest::from_str(&wire.methodology_digest)
        .map_err(|_| schema_error("methodologyDigest must be canonical sha256 hex"))?;
    if methodology_digest != wire.methodology_ref.entry_digest() {
        return Err(schema_error(
            "methodologyDigest does not match methodologyRef entryDigest",
        ));
    }
    InputFingerprint::from_str(&wire.input_fingerprint)
        .map_err(|_| schema_error("inputFingerprint must be canonical sha256 hex"))?;
    WorkItemId::new(wire.work_item_id.as_str())
        .map_err(|_| schema_error("workItemId is not a valid Work Item identity"))?;
    Ok(())
}

/// Pure identity + PASS-consistency validation shared by `toolset.receipt.record`.
pub fn validate_receipt_identity(plan: &Value, receipt: &Value) -> RuntimeResult<()> {
    let plan: VerificationExecutionPlan = serde_json::from_value(plan.clone())
        .map_err(|_| schema_error("verification plan is malformed"))?;
    let receipt: VerificationReceipt = serde_json::from_value(receipt.clone())
        .map_err(|_| schema_error("verification receipt is malformed"))?;
    ExecutionPolicy::validate_plan(&plan)
        .map_err(|_| schema_error("verification plan violates execution policy"))?;
    NativeToolset
        .record_receipt(&plan, &receipt)
        .map_err(|_| schema_error("verification receipt does not match its plan"))
}

/// Executes `toolset.required` or `toolset.receipt.record`.
pub(super) fn execute(
    context: &JobContext<'_>,
    identity: &BoundJobIdentity,
    policy_digest: &str,
    entrypoint: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    match entrypoint {
        "toolset.required" => required(context, identity, arguments),
        "toolset.receipt.record" => record_receipt(context, identity, policy_digest, arguments),
        _ => unreachable!("toolset entrypoint was classified by caller"),
    }
}

fn required(
    context: &JobContext<'_>,
    identity: &BoundJobIdentity,
    arguments: &Value,
) -> RuntimeResult<Value> {
    validate_required(arguments)?;
    let wire: RequirementArguments = serde_json::from_value(arguments.clone())
        .map_err(|_| schema_error("toolset.required arguments are malformed"))?;
    let trusted_work_item = context.required_work_item()?;
    if wire.work_item_id != trusted_work_item {
        return Err(schema_error(
            "toolset.required workItemId does not match trusted job scope",
        ));
    }
    let query = ToolsetQuery {
        schema_version: wire.schema_version,
        methodology_ref: wire.methodology_ref,
        verification_contract_id: wire.verification_contract_id,
        work_item_id: WorkItemId::new(wire.work_item_id.as_str())
            .map_err(|_| schema_error("workItemId is invalid"))?,
        input_fingerprint: InputFingerprint::from_str(&wire.input_fingerprint)
            .map_err(|_| schema_error("inputFingerprint is invalid"))?,
        mandatory: wire.mandatory,
    };
    let requirement = NativeToolset
        .require(&query)
        .map_err(|_| schema_error("toolset requirement could not be derived"))?;
    let requirement_value = serde_json::to_value(&requirement)
        .map_err(|_| schema_error("toolset requirement could not be serialized"))?;
    Ok(json!({
        "outcome": "PASS",
        "workItemId": trusted_work_item,
        "recorderSessionId": identity.session_id,
        "toolsetRequirement": requirement_value,
    }))
}

fn record_receipt(
    context: &JobContext<'_>,
    identity: &BoundJobIdentity,
    current_policy_digest: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let wire: ReceiptArguments = serde_json::from_value(arguments.clone())
        .map_err(|_| schema_error("toolset.receipt.record arguments are malformed"))?;
    if wire.lease_id.trim().is_empty() || wire.lease_id.len() > 128 || wire.fencing_token == 0 {
        return Err(schema_error(
            "toolset.receipt.record requires a bounded leaseId and positive fencingToken",
        ));
    }
    if wire.policy_digest != current_policy_digest {
        return Err(schema_error("toolset receipt policyDigest is not current"));
    }
    if wire.inventory_generation != context.workspace.inventory_generation {
        return Err(schema_error("toolset receipt inventoryGeneration is stale"));
    }
    ArtifactDigest::from_str(&wire.methodology_digest)
        .map_err(|_| schema_error("methodologyDigest must be canonical sha256 hex"))?;
    let plan_value = serde_json::to_value(&wire.plan)
        .map_err(|_| schema_error("verification plan could not be canonicalized"))?;
    let receipt_value = serde_json::to_value(&wire.receipt)
        .map_err(|_| schema_error("verification receipt could not be canonicalized"))?;
    validate_receipt_identity(&plan_value, &receipt_value)?;
    let trusted_work_item = context.required_work_item()?;
    if plan_value.get("workItemId").and_then(Value::as_str) != Some(trusted_work_item) {
        return Err(schema_error(
            "verification plan does not match trusted job Work Item",
        ));
    }
    let plan_bytes = serde_json::to_vec(&wire.plan)
        .map_err(|_| schema_error("verification plan could not be canonicalized"))?;
    let receipt_bytes = serde_json::to_vec(&receipt_value)
        .map_err(|_| schema_error("verification receipt could not be canonicalized"))?;
    if plan_bytes.len() > MAX_CANONICAL_BODY_BYTES || receipt_bytes.len() > MAX_CANONICAL_BODY_BYTES
    {
        return Err(schema_error(
            "toolset plan or receipt exceeds the 64 KiB durable bound",
        ));
    }
    let plan_digest = ArtifactDigest::digest(&plan_bytes).to_string();
    let receipt_digest = ArtifactDigest::digest(&receipt_bytes).to_string();
    let receipt_identity = ArtifactDigest::digest(
        format!("toolset-receipt\0{}\0{receipt_digest}", identity.job_id).as_bytes(),
    )
    .to_string();
    let receipt_id = format!("toolset-{}", &receipt_identity[..24]);
    let status = receipt_value
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error("verification receipt status is missing"))?;
    let outcome = match status {
        "pass" => "PASS",
        "fail" => "FAIL",
        "error" => "ERROR",
        "timeout" => "TIMEOUT",
        "cancelled" => "CANCELLED",
        _ => return Err(schema_error("verification receipt status is unsupported")),
    };
    Ok(json!({
        "outcome": outcome,
        "validated": true,
        "toolsetJobId": identity.job_id,
        "receiptId": receipt_id,
        "receiptDigest": receipt_digest,
        "planDigest": plan_digest,
        "methodologyDigest": wire.methodology_digest,
        "policyDigest": wire.policy_digest,
        "inputFingerprint": plan_value.get("inputFingerprint"),
        "sourceRevision": wire.source_revision,
        "inventoryGeneration": wire.inventory_generation,
        "workItemId": trusted_work_item,
        "executionId": plan_value.get("executionId"),
        "identityDigest": identity.identity_digest,
        "recorder": {
            "sessionId": identity.session_id,
            "rootSessionId": identity.root_session_id,
            "delegationId": identity.delegation_id,
            "contextGeneration": identity.context_generation,
        },
    }))
}
