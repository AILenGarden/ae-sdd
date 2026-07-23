use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ae_sdd_domain::ProjectRelativePath;
use ae_sdd_protocol::StableErrorCode;
use ae_sdd_runtime::{RuntimeError, RuntimeResult};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use super::{MAX_COMMON_BYTES, MAX_SLICE_BYTES, MemoryContext, schema_error};

const MAX_SOURCE_FILES: usize = 16;
const MAX_SOURCE_BYTES: u64 = 256 * 1024;
const MAX_SOURCE_TOTAL_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug)]
pub(super) struct EntityScope {
    pub(super) entity_type: String,
    pub(super) entity_id: String,
}

pub(super) struct SourceContexts {
    pub(super) hashes: BTreeMap<String, String>,
    pub(super) contents: BTreeMap<String, String>,
}

pub(super) fn is_mutation(entrypoint: &str, arguments: &Value) -> RuntimeResult<bool> {
    Ok(match entrypoint {
        "memory.create" | "memory.update" | "memory.clean" | "memory.clean-all" => true,
        "memory.common" => common_action(argument_object(arguments)?)? != "read",
        "memory.read" | "memory.search" | "memory.summarize" => false,
        _ => false,
    })
}

pub(super) fn argument_object(arguments: &Value) -> RuntimeResult<&Map<String, Value>> {
    arguments
        .as_object()
        .ok_or_else(|| schema_error("memory job arguments must be an object"))
}

pub(super) fn reject_unknown(object: &Map<String, Value>, allowed: &[&str]) -> RuntimeResult<()> {
    if let Some(field) = object
        .keys()
        .find(|field| !allowed.contains(&field.as_str()))
    {
        return Err(schema_error(&format!("unknown memory argument: {field}")));
    }
    Ok(())
}

pub(super) fn resolve_scope(object: &Map<String, Value>) -> RuntimeResult<EntityScope> {
    let explicit_type = optional_text(object, "entityType", 32)?;
    let phase = optional_text(object, "phase", 64)?;
    let story = optional_text(object, "story", 128)?;
    let task = optional_text(object, "task", 128)?;
    let explicit_id = optional_text(object, "entityId", 128)?;
    let entity_type = explicit_type
        .map(str::to_owned)
        .or_else(|| phase.and_then(phase_entity_type).map(str::to_owned))
        .or_else(|| task.map(|_| "coding".to_owned()))
        .unwrap_or_else(|| "common".to_owned());
    if !matches!(
        entity_type.as_str(),
        "prd" | "dr" | "story" | "testcase" | "coding" | "common"
    ) {
        return Err(schema_error(
            "entityType is not in the supported memory vocabulary",
        ));
    }
    let entity_id = explicit_id.or(story).or(task).unwrap_or("default");
    Ok(EntityScope {
        entity_type,
        entity_id: safe_segment(entity_id, "entityId")?,
    })
}

pub(super) fn common_scope() -> EntityScope {
    EntityScope {
        entity_type: "common".to_owned(),
        entity_id: "default".to_owned(),
    }
}

pub(super) fn common_action(object: &Map<String, Value>) -> RuntimeResult<&str> {
    if object.contains_key("action") && object.contains_key("commonAction") {
        return Err(schema_error("action and commonAction cannot be combined"));
    }
    let action = object
        .get("action")
        .or_else(|| object.get("commonAction"))
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error("memory common requires read, update, or clean action"))?;
    if matches!(action, "read" | "update" | "clean") {
        Ok(action)
    } else {
        Err(schema_error(
            "memory common action must be read, update, or clean",
        ))
    }
}

pub(super) fn required_text<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    max_bytes: usize,
) -> RuntimeResult<&'a str> {
    optional_text(object, field, max_bytes)?
        .ok_or_else(|| schema_error(&format!("{field} is required")))
}

pub(super) fn optional_text<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    max_bytes: usize,
) -> RuntimeResult<Option<&'a str>> {
    let Some(value) = object.get(field) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= max_bytes)
        .ok_or_else(|| schema_error(&format!("{field} must be bounded non-empty text")))?;
    Ok(Some(value))
}

pub(super) fn assert_project(
    context: &MemoryContext<'_>,
    value: Option<&Value>,
) -> RuntimeResult<()> {
    let Some(project) = value else {
        return Ok(());
    };
    let project = project
        .as_str()
        .ok_or_else(|| schema_error("project must be text"))?;
    if project.trim().is_empty() {
        return Ok(());
    }
    let raw = Path::new(project);
    let candidate = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        context.root.join(raw)
    };
    let canonical = std::fs::canonicalize(candidate)
        .map_err(|_| schema_error("project path cannot be canonicalized"))?;
    if canonical == context.root {
        Ok(())
    } else {
        Err(RuntimeError::new(
            StableErrorCode::ProjectMismatch,
            "memory project path differs from the registered workspace",
        ))
    }
}

pub(super) fn structured_context(object: &Map<String, Value>) -> RuntimeResult<Value> {
    if object.contains_key("context") && object.contains_key("contextJson") {
        return Err(schema_error("context and contextJson cannot be combined"));
    }
    let Some(value) = object.get("context").or_else(|| object.get("contextJson")) else {
        return Ok(json!({}));
    };
    let parsed = match value {
        Value::Object(_) => value.clone(),
        Value::String(text) => {
            serde_json::from_str(text).map_err(|_| schema_error("contextJson is not valid JSON"))?
        }
        _ => {
            return Err(schema_error(
                "contextJson must be an object or JSON object string",
            ));
        }
    };
    if !parsed.is_object() {
        return Err(schema_error("contextJson must decode to an object"));
    }
    if serde_json::to_vec(&parsed)
        .map_err(|_| schema_error("contextJson could not be serialized"))?
        .len()
        > 64 * 1024
    {
        return Err(schema_error("contextJson exceeds 64 KiB"));
    }
    Ok(parsed)
}

pub(super) fn content_argument(
    context: &MemoryContext<'_>,
    object: &Map<String, Value>,
    common: bool,
) -> RuntimeResult<String> {
    let limit = if common {
        MAX_COMMON_BYTES
    } else {
        MAX_SLICE_BYTES
    };
    let content = if let Some(path) = object.get("contentFile") {
        let path = path
            .as_str()
            .ok_or_else(|| schema_error("contentFile must be text"))?;
        let bytes = read_bounded(
            &existing_granted_file(context, path)?,
            u64::try_from(limit).unwrap_or(u64::MAX),
        )?;
        String::from_utf8(bytes)
            .map_err(|_| schema_error("memory content file must be UTF-8 text"))?
    } else {
        object
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    if content.len() > limit {
        return Err(schema_error("memory slice content exceeds its byte bound"));
    }
    Ok(content)
}

pub(super) fn source_contexts(
    context: &MemoryContext<'_>,
    value: Option<&Value>,
) -> RuntimeResult<SourceContexts> {
    let entries = match value {
        None => Vec::new(),
        Some(Value::String(value)) => vec![value.clone()],
        Some(Value::Array(values)) if values.len() <= MAX_SOURCE_FILES => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| schema_error("sources must contain only strings"))
            })
            .collect::<RuntimeResult<Vec<_>>>()?,
        _ => {
            return Err(schema_error(
                "sources must be a string or an array of at most 16 strings",
            ));
        }
    };
    if entries.len() > MAX_SOURCE_FILES {
        return Err(schema_error("sources exceeds the 16-file bound"));
    }
    let mut hashes = BTreeMap::new();
    let mut contents = BTreeMap::new();
    let mut total = 0_usize;
    for entry in entries {
        let (name, relative) = entry.split_once('=').unwrap_or((entry.as_str(), ""));
        let name = safe_segment(name, "source name")?;
        if relative.is_empty() {
            continue;
        }
        let path = existing_granted_file(context, relative)?;
        let bytes = read_bounded(&path, MAX_SOURCE_BYTES)?;
        total = total.saturating_add(bytes.len());
        if total > MAX_SOURCE_TOTAL_BYTES {
            return Err(schema_error("source contexts exceed the 1 MiB total bound"));
        }
        let text = String::from_utf8(bytes)
            .map_err(|_| schema_error("memory source context must be UTF-8 text"))?;
        hashes.insert(name.clone(), hex::encode(Sha256::digest(text.as_bytes())));
        contents.insert(name, text);
    }
    Ok(SourceContexts { hashes, contents })
}

fn existing_granted_file(context: &MemoryContext<'_>, value: &str) -> RuntimeResult<PathBuf> {
    let relative = ProjectRelativePath::new(value.replace('\\', "/")).map_err(|_| {
        schema_error("memory source path must be project-relative and traversal-free")
    })?;
    if !context.grant.permits_path(&relative) {
        return Err(RuntimeError::new(
            StableErrorCode::RoleOperationForbidden,
            "trusted scoped grant does not permit the memory source path",
        ));
    }
    let canonical = std::fs::canonicalize(context.root.join(relative.as_str())).map_err(|_| {
        RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "bounded memory source file I/O failed",
        )
    })?;
    if !canonical.starts_with(&context.root) || !canonical.is_file() {
        return Err(RuntimeError::new(
            StableErrorCode::WorkspaceOutsideAllowedRoot,
            "memory source is not a regular file inside the registered workspace",
        ));
    }
    Ok(canonical)
}

fn read_bounded(path: &Path, limit: u64) -> RuntimeResult<Vec<u8>> {
    let metadata = std::fs::metadata(path).map_err(|_| {
        RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "bounded memory source file I/O failed",
        )
    })?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(schema_error(
            "memory source exceeds its bounded read contract",
        ));
    }
    let bytes = std::fs::read(path).map_err(|_| {
        RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "bounded memory source file I/O failed",
        )
    })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Err(schema_error(
            "memory source changed beyond its bounded read contract",
        ));
    }
    Ok(bytes)
}

fn safe_segment(value: &str, name: &str) -> RuntimeResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 128
        || !trimmed.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
        || matches!(trimmed, "." | "..")
    {
        return Err(schema_error(&format!("{name} is not a safe identifier")));
    }
    Ok(trimmed.to_owned())
}

fn phase_entity_type(value: &str) -> Option<&'static str> {
    match value {
        "ra" | "ra-generated" => Some("prd"),
        "dr-generated" => Some("dr"),
        "design" | "story-generated" | "story-reviewed" => Some("story"),
        "testcase-generated" | "testcase-reviewed" => Some("testcase"),
        "coding-plan" | "coding-process" | "coding" | "test-running" | "code-reviewed" => {
            Some("coding")
        }
        _ => None,
    }
}

#[cfg(windows)]
fn path_key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

#[cfg(not(windows))]
fn path_key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[allow(dead_code)]
fn unique_paths(paths: impl IntoIterator<Item = PathBuf>) -> BTreeSet<String> {
    paths.into_iter().map(|path| path_key(&path)).collect()
}
