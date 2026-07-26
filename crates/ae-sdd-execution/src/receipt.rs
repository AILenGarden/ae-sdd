//! Verification-receipt validation against its originating plan.

use ae_sdd_contracts::execution::{VerificationExecutionPlan, VerificationReceipt};
use ae_sdd_protocol::JobStatus;

use crate::error::{ExecutionPolicyError, ExecutionPolicyFault};

/// Validates that a receipt corresponds to the supplied plan identity.
///
/// The frozen [`VerificationReceipt`] contract already enforces status/time
/// consistency at construction. This pure layer adds plan↔receipt identity
/// matching by reading the canonical camelCase wire fields via serde JSON
/// (C0 does not yet expose field accessors, and Part D must not extend the
/// contract).
///
/// Identity fields compared: `executionId`, `workItemId`, `inputFingerprint`.
/// Terminal consistency (`Pass` ⟺ `exit_code=0 && !timed_out && !cancelled`;
/// `Timeout` ⟺ `timed_out`; `Cancelled` ⟺ `cancelled`) is re-checked so a
/// future contract loosening cannot silently admit a stale receipt.
pub fn validate_against_plan(
    plan: &VerificationExecutionPlan,
    receipt: &VerificationReceipt,
) -> Result<(), ExecutionPolicyError> {
    let plan_value = serde_json::to_value(plan).map_err(|_| {
        ExecutionPolicyError::ReceiptRejected(ExecutionPolicyFault::IdentityMismatch)
    })?;
    let receipt_value = serde_json::to_value(receipt).map_err(|_| {
        ExecutionPolicyError::ReceiptRejected(ExecutionPolicyFault::IdentityMismatch)
    })?;

    for field in &["executionId", "workItemId", "inputFingerprint"] {
        if plan_value.get(field) != receipt_value.get(field) {
            return Err(ExecutionPolicyError::ReceiptRejected(
                ExecutionPolicyFault::IdentityMismatch,
            ));
        }
    }

    let status = receipt.status();
    let exit_code = receipt_value
        .get("exitCode")
        .and_then(|value| value.as_i64());
    let timed_out = receipt_value
        .get("timedOut")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let cancelled = receipt_value
        .get("cancelled")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    if status == JobStatus::Pass && (exit_code != Some(0) || timed_out || cancelled) {
        return Err(ExecutionPolicyError::ReceiptRejected(
            ExecutionPolicyFault::FakePassResult,
        ));
    }
    if (status == JobStatus::Timeout) != timed_out || (status == JobStatus::Cancelled) != cancelled
    {
        return Err(ExecutionPolicyError::ReceiptRejected(
            ExecutionPolicyFault::StaleArtifactDigest,
        ));
    }
    Ok(())
}
