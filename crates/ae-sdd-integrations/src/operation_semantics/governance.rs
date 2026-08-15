use std::collections::BTreeSet;

use ae_sdd_domain::ResultDigest;
use ae_sdd_operations::Confirmation;
use serde_json::{Map, Value};

pub(crate) fn execution_plan(payload: &Value) -> Result<Value, &'static str> {
    let object = payload
        .as_object()
        .ok_or("execution plan payload must be an object")?;
    let goal = required_trimmed_string(object.get("goal"), "execution plan goal is required")?;
    let changed_paths = normalized_strings(
        object.get("changedPaths"),
        true,
        true,
        "execution plan changedPaths must contain strings",
    )?;
    let verification = object
        .get("verification")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or("execution plan verification must not be empty")?;
    if verification.iter().any(|item| !item.is_object()) {
        return Err("execution plan verification entries must be objects");
    }
    let risks = normalized_strings(
        object.get("risks"),
        false,
        false,
        "execution plan risks must contain strings",
    )?;
    let source_reads = normalized_strings(
        object.get("sourceReads"),
        false,
        true,
        "execution plan sourceReads must contain strings",
    )?;

    let mut plan = Map::new();
    plan.insert("goal".to_owned(), Value::String(goal));
    plan.insert("changedPaths".to_owned(), Value::Array(changed_paths));
    plan.insert(
        "verification".to_owned(),
        Value::Array(verification.clone()),
    );
    plan.insert("risks".to_owned(), Value::Array(risks));
    plan.insert("sourceReads".to_owned(), Value::Array(source_reads));
    plan.insert("approved".to_owned(), Value::Bool(false));
    plan.insert("approvedAt".to_owned(), Value::Null);
    plan.insert("approvedBy".to_owned(), Value::Null);
    Ok(Value::Object(plan))
}

pub(crate) fn approved_execution_plan(
    state: &Value,
    confirmation: &Confirmation,
) -> Result<Value, &'static str> {
    let mut plan = state
        .get("executionPlan")
        .cloned()
        .ok_or("executionPlan does not exist")?;
    if plan.get("approved").and_then(Value::as_bool) != Some(false) {
        return Err("executionPlan is not awaiting approval");
    }
    let binding = execution_plan_confirmation_binding(&plan)?;
    let object = plan
        .as_object_mut()
        .ok_or("executionPlan must be an object")?;
    required_trimmed_string(object.get("goal"), "executionPlan goal is required")?;
    required_nonempty_array(
        object.get("changedPaths"),
        "executionPlan changedPaths is required",
    )?;
    required_nonempty_array(
        object.get("verification"),
        "executionPlan verification is required",
    )?;
    let expected_confirmation = format!("plan:{binding}");
    if confirmation.confirmation_id() != expected_confirmation {
        return Err("execution plan approval confirmation does not bind the current plan");
    }
    object.insert("approved".to_owned(), Value::Bool(true));
    object.insert(
        "approvedAt".to_owned(),
        Value::String(confirmation.approved_at().to_owned()),
    );
    object.insert(
        "approvedBy".to_owned(),
        Value::String(confirmation.approved_by().to_owned()),
    );
    Ok(plan)
}

fn execution_plan_confirmation_binding(plan: &Value) -> Result<String, &'static str> {
    let bytes = serde_json::to_vec(plan)
        .map_err(|_| "executionPlan could not be canonicalized for approval")?;
    Ok(ResultDigest::digest(bytes).to_string())
}

fn required_trimmed_string(
    value: Option<&Value>,
    error: &'static str,
) -> Result<String, &'static str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or(error)
}

fn required_nonempty_array(value: Option<&Value>, error: &'static str) -> Result<(), &'static str> {
    value
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .map(|_| ())
        .ok_or(error)
}

fn normalized_strings(
    value: Option<&Value>,
    required: bool,
    normalize_slashes: bool,
    entry_error: &'static str,
) -> Result<Vec<Value>, &'static str> {
    let values = match value {
        Some(value) => value.as_array().ok_or(entry_error)?,
        None if required => return Err(entry_error),
        None => return Ok(Vec::new()),
    };
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for value in values {
        let item = value.as_str().ok_or(entry_error)?.trim();
        if item.is_empty() {
            continue;
        }
        let item = if normalize_slashes {
            item.replace('\\', "/")
        } else {
            item.to_owned()
        };
        if seen.insert(item.clone()) {
            normalized.push(Value::String(item));
        }
    }
    if required && normalized.is_empty() {
        return Err(entry_error);
    }
    Ok(normalized)
}
