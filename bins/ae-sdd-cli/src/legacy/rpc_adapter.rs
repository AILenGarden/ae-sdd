use std::fs;
use std::path::Path;

use ae_sdd_protocol::{RequestParams, RpcMethod};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::LegacyArgumentError;

const RA_REQUIRED_GATES: [&str; 7] = [
    "G-RA-1",
    "G-RA-2",
    "G-RA-3",
    "G-RA-4",
    "G-RA-FLOW-VIOLATION",
    "G-RA-5",
    "G-RA-6",
];

/// Converts a frozen passthrough route into the strict payload accepted by its
/// daemon method. A route without semantic parity is rejected before IPC.
pub fn adapt_passthrough_request(
    command_id: &str,
    method: RpcMethod,
    params: &mut RequestParams<Value>,
) -> Result<(), LegacyArgumentError> {
    match command_id {
        "health" => {
            require_method(method, RpcMethod::RuntimeStatus, command_id)?;
            require_empty_payload(command_id, &params.payload)
        }
        "ops describe" => {
            require_method(method, RpcMethod::OperationDescribe, command_id)?;
            params.payload = describe_payload(command_id, take_object(params)?)?;
            Ok(())
        }
        "ops execute" => {
            require_method(method, RpcMethod::OperationExecute, command_id)?;
            adapt_operation_request(params)
        }
        "ops next" => {
            require_method(method, RpcMethod::FlowNext, command_id)?;
            params.payload = flow_next_payload(command_id, take_object(params)?)?;
            Ok(())
        }
        "review abort" => {
            require_method(method, RpcMethod::DelegationCancel, command_id)?;
            params.payload = delegation_cancel_payload(command_id, take_object(params)?)?;
            Ok(())
        }
        "review collect" | "review-loop collect" => {
            require_method(method, RpcMethod::DelegationCollect, command_id)?;
            params.payload = delegation_collect_payload(command_id, take_object(params)?)?;
            Ok(())
        }
        "flow-violation-scan" => gate_payload(params, method, command_id, "G-RA-FLOW-VIOLATION"),
        "gate coding-required" => gate_payload(params, method, command_id, "G-CODE-1"),
        "gate ra-required" => gate_batch_payload(params, method, command_id, &RA_REQUIRED_GATES),
        "ra-authenticity-scan" => gate_payload(params, method, command_id, "G-RA-4"),
        "ra-depth-scan" => gate_payload(params, method, command_id, "G-RA-5"),
        "ra-implementation-scan" => gate_payload(params, method, command_id, "G-RA-6"),
        "memory clean" | "memory clean-all" | "memory common" | "memory create" | "memory read"
        | "memory search" | "memory summarize" | "memory update" => unsupported(
            command_id,
            "daemon memory is lifecycle-owned; a legacy manual memory command cannot be reported as context projection success",
        ),
        "review retry"
        | "review retry-role"
        | "review start"
        | "review status"
        | "review verify-exit"
        | "review-loop start"
        | "review-loop status"
        | "review-loop verify-exit" => unsupported(
            command_id,
            "the legacy in-file review state machine is not equivalent to FlowRuntime projection",
        ),
        _ => unsupported(command_id, "no verified passthrough adapter is registered"),
    }
}

/// Converts a valid JSON-RPC Gate response into process-level success or
/// failure. Non-PASS is evidence, but never a successful legacy command exit.
pub fn validate_passthrough_result(
    command_id: &str,
    method: RpcMethod,
    result: &Value,
) -> Result<(), LegacyArgumentError> {
    if method != RpcMethod::GateEvaluate {
        return Ok(());
    }
    let all_pass = result
        .get("allPass")
        .and_then(Value::as_bool)
        .or_else(|| {
            result
                .pointer("/outcome/kind")
                .and_then(Value::as_str)
                .map(|kind| kind == "PASS")
        })
        .unwrap_or(false);
    if all_pass {
        Ok(())
    } else {
        Err(LegacyArgumentError::new(format!(
            "LEGACY_GATE_NON_PASS: {command_id} returned a non-PASS authoritative Gate outcome"
        )))
    }
}

fn gate_payload(
    params: &mut RequestParams<Value>,
    method: RpcMethod,
    command_id: &str,
    gate_id: &str,
) -> Result<(), LegacyArgumentError> {
    require_method(method, RpcMethod::GateEvaluate, command_id)?;
    let mut object = take_object(params)?;
    let expected_root = take_gate_locator(command_id, &mut object)?;
    reject_remaining(command_id, &object)?;
    params.payload = json!({"gateId":gate_id,"expectedProjectRoot":expected_root});
    remove_null_field(&mut params.payload, "expectedProjectRoot");
    Ok(())
}

fn gate_batch_payload(
    params: &mut RequestParams<Value>,
    method: RpcMethod,
    command_id: &str,
    gate_ids: &[&str],
) -> Result<(), LegacyArgumentError> {
    require_method(method, RpcMethod::GateEvaluate, command_id)?;
    let mut object = take_object(params)?;
    let expected_root = take_gate_locator(command_id, &mut object)?;
    reject_remaining(command_id, &object)?;
    params.payload = json!({"gateIds":gate_ids,"expectedProjectRoot":expected_root});
    remove_null_field(&mut params.payload, "expectedProjectRoot");
    Ok(())
}

fn take_gate_locator(
    command_id: &str,
    object: &mut Map<String, Value>,
) -> Result<Option<String>, LegacyArgumentError> {
    let project = take_optional_string(command_id, object, "project")?;
    let root = take_optional_string(command_id, object, "root")?;
    if project.is_some() && root.is_some() {
        return Err(LegacyArgumentError::new(format!(
            "{command_id} cannot combine project and root locators"
        )));
    }
    if let Some(strict) = object.remove("strict")
        && !strict.is_boolean()
    {
        return Err(LegacyArgumentError::new(format!(
            "{command_id} strict must be boolean"
        )));
    }
    Ok(project.or(root))
}

fn describe_payload(
    command_id: &str,
    mut object: Map<String, Value>,
) -> Result<Value, LegacyArgumentError> {
    let operation = take_optional_string(command_id, &mut object, "operation")?;
    let _project = take_optional_string(command_id, &mut object, "project")?;
    reject_remaining(command_id, &object)?;
    Ok(operation.map_or_else(|| json!({}), |operation| json!({"operation":operation})))
}

fn flow_next_payload(
    command_id: &str,
    mut object: Map<String, Value>,
) -> Result<Value, LegacyArgumentError> {
    let story = take_optional_string(command_id, &mut object, "story")?;
    let expected_root = take_optional_string(command_id, &mut object, "project")?;
    reject_remaining(command_id, &object)?;
    let mut payload = json!({"story":story,"expectedProjectRoot":expected_root});
    remove_null_field(&mut payload, "story");
    remove_null_field(&mut payload, "expectedProjectRoot");
    Ok(payload)
}

fn delegation_collect_payload(
    command_id: &str,
    mut object: Map<String, Value>,
) -> Result<Value, LegacyArgumentError> {
    let delegation_id = take_required_string(command_id, &mut object, "delegationId")?;
    reject_remaining(command_id, &object)?;
    Ok(json!({"delegationId":delegation_id}))
}

fn delegation_cancel_payload(
    command_id: &str,
    mut object: Map<String, Value>,
) -> Result<Value, LegacyArgumentError> {
    let delegation_id = take_required_string(command_id, &mut object, "delegationId")?;
    let reason = take_optional_string(command_id, &mut object, "reason")?
        .unwrap_or_else(|| "user-abort".to_owned());
    reject_remaining(command_id, &object)?;
    Ok(json!({"delegationId":delegation_id,"reason":reason}))
}

fn adapt_operation_request(params: &mut RequestParams<Value>) -> Result<(), LegacyArgumentError> {
    let mut object = take_object(params)?;
    let request_file = take_required_string("ops execute", &mut object, "requestFile")?;
    reject_remaining("ops execute", &object)?;
    let bytes = fs::read(Path::new(&request_file)).map_err(|error| {
        LegacyArgumentError::new(format!(
            "OPERATION_REQUEST_INVALID_JSON: request-file cannot be read: {error}"
        ))
    })?;
    let request: LegacyOperationRequest = serde_json::from_slice(&bytes).map_err(|error| {
        LegacyArgumentError::new(format!(
            "OPERATION_REQUEST_INVALID_JSON: request-file violates the legacy envelope: {error}"
        ))
    })?;
    if request.schema_version != "1" {
        return Err(LegacyArgumentError::new(
            "OPERATION_SCHEMA_VERSION_UNSUPPORTED: ops execute accepts schemaVersion 1",
        ));
    }
    if request.project.trim().is_empty() {
        return Err(LegacyArgumentError::new(
            "OPERATION_SCHEMA_INVALID: project is required",
        ));
    }
    if let Some(project_key) = request.project_key.as_deref()
        && project_key.trim().is_empty()
    {
        return Err(LegacyArgumentError::new(
            "OPERATION_SCHEMA_INVALID: projectKey cannot be empty",
        ));
    }
    if let Some(story) = request.story.as_deref()
        && story.trim().is_empty()
    {
        return Err(LegacyArgumentError::new(
            "OPERATION_SCHEMA_INVALID: story cannot be empty",
        ));
    }
    merge_text(&mut params.work_item_id, request.work_item, "workItem")?;
    if let Some(lease) = request.lease {
        merge_text(&mut params.lease_id, lease.lease_id, "leaseId")?;
        merge_u64(
            &mut params.fencing_token,
            lease.fencing_token,
            "fencingToken",
        )?;
    }
    if let Some(revision) = request.expected_revision {
        merge_u64(&mut params.expected_revision, revision, "expectedRevision")?;
    }
    if let Some(key) = request.idempotency_key {
        merge_text(&mut params.idempotency_key, key, "idempotencyKey")?;
    }
    if !request.parameters.is_object() {
        return Err(LegacyArgumentError::new(
            "OPERATION_SCHEMA_INVALID: parameters must be an object",
        ));
    }
    params.payload = json!({
        "operation":request.operation,
        "dryRun":request.dry_run,
        "payload":request.parameters,
        "expectedProjectRoot":request.project,
        "expectedProjectKey":request.project_key,
        "story":request.story,
    });
    remove_null_field(&mut params.payload, "expectedProjectKey");
    remove_null_field(&mut params.payload, "story");
    Ok(())
}

fn take_object(
    params: &mut RequestParams<Value>,
) -> Result<Map<String, Value>, LegacyArgumentError> {
    std::mem::take(&mut params.payload)
        .as_object()
        .cloned()
        .ok_or_else(|| LegacyArgumentError::new("legacy RPC business payload must be an object"))
}

fn require_empty_payload(command_id: &str, payload: &Value) -> Result<(), LegacyArgumentError> {
    let object = payload.as_object().ok_or_else(|| {
        LegacyArgumentError::new(format!("{command_id} payload must be an object"))
    })?;
    reject_remaining(command_id, object)
}

fn take_required_string(
    command_id: &str,
    object: &mut Map<String, Value>,
    field: &str,
) -> Result<String, LegacyArgumentError> {
    take_optional_string(command_id, object, field)?.ok_or_else(|| {
        LegacyArgumentError::new(format!("{command_id} requires --{}", camel_to_kebab(field)))
    })
}

fn take_optional_string(
    command_id: &str,
    object: &mut Map<String, Value>,
    field: &str,
) -> Result<Option<String>, LegacyArgumentError> {
    object
        .remove(field)
        .map(|value| match value {
            Value::String(value) if !value.trim().is_empty() => Ok(value),
            _ => Err(LegacyArgumentError::new(format!(
                "{command_id} field {field} must be a non-empty string"
            ))),
        })
        .transpose()
}

fn reject_remaining(
    command_id: &str,
    object: &Map<String, Value>,
) -> Result<(), LegacyArgumentError> {
    if object.is_empty() {
        Ok(())
    } else {
        Err(LegacyArgumentError::new(format!(
            "{command_id} has unsupported legacy business fields: {}",
            object.keys().cloned().collect::<Vec<_>>().join(", ")
        )))
    }
}

fn require_method(
    actual: RpcMethod,
    expected: RpcMethod,
    command_id: &str,
) -> Result<(), LegacyArgumentError> {
    if actual == expected {
        Ok(())
    } else {
        Err(LegacyArgumentError::new(format!(
            "legacy route {command_id} resolved to unexpected method {actual}"
        )))
    }
}

fn merge_text(
    target: &mut Option<String>,
    incoming: String,
    field: &str,
) -> Result<(), LegacyArgumentError> {
    if incoming.trim().is_empty() {
        return Err(LegacyArgumentError::new(format!(
            "OPERATION_SCHEMA_INVALID: {field} cannot be empty"
        )));
    }
    match target {
        Some(current) if current != &incoming => Err(LegacyArgumentError::new(format!(
            "OPERATION_SCOPE_CONFLICT: CLI {field} conflicts with request-file"
        ))),
        Some(_) => Ok(()),
        None => {
            *target = Some(incoming);
            Ok(())
        }
    }
}

fn merge_u64(
    target: &mut Option<u64>,
    incoming: u64,
    field: &str,
) -> Result<(), LegacyArgumentError> {
    match target {
        Some(current) if *current != incoming => Err(LegacyArgumentError::new(format!(
            "OPERATION_SCOPE_CONFLICT: CLI {field} conflicts with request-file"
        ))),
        Some(_) => Ok(()),
        None => {
            *target = Some(incoming);
            Ok(())
        }
    }
}

fn remove_null_field(value: &mut Value, field: &str) {
    if value.get(field).is_some_and(Value::is_null) {
        value
            .as_object_mut()
            .expect("constructed payload is an object")
            .remove(field);
    }
}

fn camel_to_kebab(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        if character.is_ascii_uppercase() {
            output.push('-');
            output.push(character.to_ascii_lowercase());
        } else {
            output.push(character);
        }
    }
    output
}

fn unsupported<T>(command_id: &str, reason: &str) -> Result<T, LegacyArgumentError> {
    Err(LegacyArgumentError::new(format!(
        "LEGACY_ROUTE_SEMANTICS_UNAVAILABLE: {command_id} was denied because {reason}"
    )))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyOperationRequest {
    schema_version: String,
    operation: String,
    project: String,
    #[serde(default)]
    project_key: Option<String>,
    work_item: String,
    #[serde(default)]
    story: Option<String>,
    #[serde(default)]
    lease: Option<LegacyLease>,
    #[serde(default)]
    expected_revision: Option<u64>,
    #[serde(default)]
    idempotency_key: Option<String>,
    #[serde(default)]
    dry_run: bool,
    #[serde(default = "empty_object")]
    parameters: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyLease {
    lease_id: String,
    fencing_token: u64,
}

fn empty_object() -> Value {
    json!({})
}
