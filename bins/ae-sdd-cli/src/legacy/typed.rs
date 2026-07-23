use std::str::FromStr;

use ae_sdd_operations::{OperationName, validate_operation_payload};
use ae_sdd_protocol::RequestParams;
use serde_json::{Value, json};

use super::LegacyArgumentError;

/// Applies the frozen operation selector and validates registry-derived
/// concurrency controls before the CLI opens IPC.
pub fn adapt_typed_operation_request(
    operation: &str,
    params: &mut RequestParams<Value>,
) -> Result<(), LegacyArgumentError> {
    let operation = OperationName::from_str(operation).map_err(|_| {
        LegacyArgumentError::new("typed legacy route references an unregistered operation")
    })?;
    let mut payload = std::mem::take(&mut params.payload)
        .as_object()
        .cloned()
        .ok_or_else(|| LegacyArgumentError::new("typed operation payload must be an object"))?;
    let dry_run = match payload.remove("dryRun") {
        None => false,
        Some(Value::Bool(value)) => value,
        Some(_) => {
            return Err(LegacyArgumentError::new(
                "typed operation dry-run flag must be boolean",
            ));
        }
    };
    let payload = Value::Object(payload);
    validate_operation_payload(operation, &payload).map_err(|error| {
        LegacyArgumentError::new(format!("typed operation payload is invalid: {error}"))
    })?;
    let spec = operation.spec();
    if spec.requires_lease && (params.lease_id.is_none() || params.fencing_token.is_none()) {
        return Err(LegacyArgumentError::new(
            "typed operation requires lease-id and fencing-token",
        ));
    }
    if spec.requires_revision && params.expected_revision.is_none() {
        return Err(LegacyArgumentError::new(
            "typed operation requires expected-revision",
        ));
    }
    if spec.requires_idempotency && params.idempotency_key.is_none() {
        return Err(LegacyArgumentError::new(
            "typed operation requires an idempotency key",
        ));
    }
    if spec.requires_confirmation && params.confirmation.is_none() {
        return Err(LegacyArgumentError::new(
            "typed operation requires explicit confirmation",
        ));
    }
    params.payload = json!({
        "operation":operation.as_str(),
        "dryRun":dry_run,
        "payload":payload,
    });
    Ok(())
}
