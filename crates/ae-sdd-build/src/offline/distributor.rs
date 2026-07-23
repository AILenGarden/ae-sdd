use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::*;
use crate::{AdminChange, PermissionClass};

const REGISTRY_SCHEMA: &str = "ae-sdd-distributor-registry/v1";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Registry {
    schema_version: String,
    entries: Vec<DistributorEntry>,
}

pub(super) fn list(
    request: &OfflineRequest,
    registry_file: &Path,
) -> Result<OfflineResult, OfflineError> {
    let registry = load(registry_file)?;
    Ok(result(
        request,
        Vec::new(),
        serde_json::json!({"entries": registry.entries}),
        None,
    ))
}

pub(super) fn scan(
    request: &OfflineRequest,
    registry_file: &Path,
) -> Result<OfflineResult, OfflineError> {
    let registry = load(registry_file)?;
    let entries: Vec<_> = registry
        .entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "name": entry.name,
                "kind": entry.kind,
                "targetPath": display(&entry.target_path),
                "enabled": entry.enabled,
                "targetExists": entry.target_path.exists()
            })
        })
        .collect();
    Ok(result(
        request,
        Vec::new(),
        serde_json::json!({"entries": entries}),
        None,
    ))
}

pub(super) fn register(
    request: &OfflineRequest,
    registry_file: &Path,
    entry: &DistributorEntry,
) -> Result<OfflineResult, OfflineError> {
    validate_entry(entry)?;
    let mut registry = load(registry_file)?;
    if registry
        .entries
        .iter()
        .any(|existing| existing.name == entry.name)
    {
        return Err(OfflineError::DistributorExists(entry.name.clone()));
    }
    registry.entries.push(entry.clone());
    registry
        .entries
        .sort_by(|left, right| left.name.cmp(&right.name));
    persist(request, registry_file, registry)
}

pub(super) fn unregister(
    request: &OfflineRequest,
    registry_file: &Path,
    name: &str,
) -> Result<OfflineResult, OfflineError> {
    validate_name(name)?;
    let mut registry = load(registry_file)?;
    let before = registry.entries.len();
    registry.entries.retain(|entry| entry.name != name);
    if registry.entries.len() == before {
        return Err(OfflineError::DistributorMissing(name.to_owned()));
    }
    persist(request, registry_file, registry)
}

pub(super) fn set_enabled(
    request: &OfflineRequest,
    registry_file: &Path,
    name: &str,
    enabled: bool,
) -> Result<OfflineResult, OfflineError> {
    validate_name(name)?;
    let mut registry = load(registry_file)?;
    let entry = registry
        .entries
        .iter_mut()
        .find(|entry| entry.name == name)
        .ok_or_else(|| OfflineError::DistributorMissing(name.to_owned()))?;
    entry.enabled = enabled;
    persist(request, registry_file, registry)
}

fn load(path: &Path) -> Result<Registry, OfflineError> {
    if !path.exists() {
        return Ok(Registry {
            schema_version: REGISTRY_SCHEMA.to_owned(),
            entries: Vec::new(),
        });
    }
    let registry: Registry =
        serde_json::from_slice(&std::fs::read(path).map_err(|source| io(path, source))?)?;
    if registry.schema_version != REGISTRY_SCHEMA {
        return Err(OfflineError::InvalidArtifact(display(path)));
    }
    for entry in &registry.entries {
        validate_entry(entry)?;
    }
    let mut names: Vec<_> = registry.entries.iter().map(|entry| &entry.name).collect();
    names.sort();
    names.dedup();
    if names.len() != registry.entries.len() {
        return Err(OfflineError::InvalidArtifact(display(path)));
    }
    Ok(registry)
}

fn persist(
    request: &OfflineRequest,
    registry_file: &Path,
    registry: Registry,
) -> Result<OfflineResult, OfflineError> {
    let parent = registry_file
        .parent()
        .ok_or(OfflineError::InvalidInput("registryFile"))?
        .canonicalize()
        .map_err(|source| io(registry_file, source))?;
    let file_name = registry_file
        .file_name()
        .ok_or(OfflineError::InvalidInput("registryFile"))?;
    let contents = serde_json::to_string_pretty(&registry)? + "\n";
    apply_changes(
        request,
        &parent,
        vec![AdminChange {
            relative_path: PathBuf::from(file_name),
            contents,
            permission: PermissionClass::PrivateFile,
        }],
    )
}

fn validate_entry(entry: &DistributorEntry) -> Result<(), OfflineError> {
    validate_name(&entry.name)?;
    if !matches!(entry.kind.as_str(), "copytree" | "harness-mount" | "native") {
        return Err(OfflineError::InvalidInput("distributor kind"));
    }
    if entry.target_path.as_os_str().is_empty() {
        return Err(OfflineError::InvalidInput("targetPath"));
    }
    Ok(())
}
