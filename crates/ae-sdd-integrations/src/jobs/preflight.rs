//! Native `preflight.collect` job for Part D.
//!
//! Reads a bounded preflight bundle from `.ae-sdd/preflight/<work_item>.json`
//! and returns it to the caller. The job is read-only: it never mutates the
//! bundle, never shells out, and never accepts absolute or traversal paths.

#![allow(dead_code)]

use ae_sdd_runtime::RuntimeResult;
use serde_json::{Value, json};

use super::common::{JobContext, MAX_FILE_BYTES, read_bounded, schema_error};

/// Pure argument validator for `preflight.collect`.
///
/// Returns `Ok(())` when the `intent` field is bounded non-empty text, or a
/// stable runtime error otherwise. Exposed publicly so integration tests can
/// exercise the schema check without a daemon fixture.
pub fn validate_intent(arguments: &Value) -> RuntimeResult<()> {
    arguments
        .get("intent")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .ok_or_else(|| schema_error("intent must be bounded non-empty text"))?;
    Ok(())
}

/// Executes the `preflight.collect` entrypoint.
pub(super) fn execute(context: &JobContext<'_>, arguments: &Value) -> RuntimeResult<Value> {
    let work_item = context.required_work_item()?;
    let intent = arguments
        .get("intent")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .ok_or_else(|| schema_error("intent must be bounded non-empty text"))?;
    let relative = format!(".ae-sdd/preflight/{work_item}.json");
    let path = context.existing_file(&relative)?;
    let bytes = read_bounded(&path, MAX_FILE_BYTES)?;
    let bundle: Value = serde_json::from_slice(&bytes)
        .map_err(|_| schema_error("preflight bundle must be valid JSON"))?;
    Ok(json!({
        "outcome": "PASS",
        "workItemId": work_item,
        "intent": intent,
        "bundlePath": relative,
        "bundle": bundle,
    }))
}

#[cfg(test)]
mod tests {
    // Pure schema-validation tests live in the integrations test crate; this
    // module only provides the bounded I/O path.
}
