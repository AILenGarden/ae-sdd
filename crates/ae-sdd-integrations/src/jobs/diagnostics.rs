mod document_storage;
mod iteration;
mod update;

use std::fs;

use ae_sdd_protocol::StableErrorCode;
use ae_sdd_runtime::{RuntimeError, RuntimeResult};
use serde_json::Value;

use super::common::{JobContext, MAX_FILE_BYTES, read_bounded, schema_error};

pub(super) fn execute(
    context: &JobContext<'_>,
    entrypoint: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let work_item_id = context.required_work_item()?;
    match entrypoint {
        "gate.doc-storage" => {
            validate_argument_keys(arguments, &["intent", "path", "project"])?;
            document_storage::execute(context, work_item_id, arguments)
        }
        "iteration-check" => {
            validate_argument_keys(arguments, &["project"])?;
            iteration::execute(context, work_item_id, arguments)
        }
        "update-check" => {
            validate_argument_keys(arguments, &["affected", "only"])?;
            update::execute(context, work_item_id, arguments)
        }
        _ => unreachable!("diagnostic entrypoint was classified by caller"),
    }
}

fn assert_registered_project(context: &JobContext<'_>, arguments: &Value) -> RuntimeResult<bool> {
    let Some(project) = optional_string(arguments, "project")? else {
        return Ok(false);
    };
    let canonical =
        fs::canonicalize(project).map_err(|_| schema_error("project cannot be canonicalized"))?;
    if canonical != context.root {
        return Err(RuntimeError::new(
            StableErrorCode::ProjectMismatch,
            "project argument differs from the registered workspace",
        ));
    }
    Ok(true)
}

fn optional_string(arguments: &Value, name: &str) -> RuntimeResult<Option<String>> {
    arguments
        .get(name)
        .map(|value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty() && value.len() <= 4_096)
                .map(str::to_owned)
                .ok_or_else(|| schema_error(&format!("{name} must be bounded non-empty text")))
        })
        .transpose()
}

fn validate_argument_keys(arguments: &Value, allowed: &[&str]) -> RuntimeResult<()> {
    let object = arguments
        .as_object()
        .ok_or_else(|| schema_error("diagnostic arguments must be an object"))?;
    if let Some(unknown) = object
        .keys()
        .find(|field| !allowed.contains(&field.as_str()))
    {
        return Err(schema_error(&format!(
            "diagnostic argument is not registered: {unknown}"
        )));
    }
    Ok(())
}

fn required_known_text(context: &JobContext<'_>, relative: &str) -> RuntimeResult<String> {
    let path = context.project_file(relative)?;
    String::from_utf8(read_bounded(&path, MAX_FILE_BYTES)?)
        .map_err(|_| schema_error("diagnostic input must be UTF-8"))
}

fn optional_known_text(context: &JobContext<'_>, relative: &str) -> RuntimeResult<Option<String>> {
    if !context.root.join(relative).is_file() {
        return Ok(None);
    }
    required_known_text(context, relative).map(Some)
}
