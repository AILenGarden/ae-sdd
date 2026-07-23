use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuildManifest {
    schema_version: String,
    source_files: Vec<ManifestEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestEntry {
    path: String,
    digest: String,
    permission: String,
}

#[derive(Deserialize)]
struct LegacyManifest {
    schema: String,
    compiled: bool,
    deterministic: bool,
    runtime_fingerprint: String,
    entry: String,
    load_order: Vec<String>,
    generated_files: Vec<String>,
}

pub(super) fn runtime(
    request: &OfflineRequest,
    package_directory: &Path,
) -> Result<OfflineResult, OfflineError> {
    let root = package_directory
        .canonicalize()
        .map_err(|source| io(package_directory, source))?;
    if !root.join("SKILL.md").is_file() {
        return Err(OfflineError::InvalidArtifact(display(
            &root.join("SKILL.md"),
        )));
    }
    let manifest_path = root.join("runtime/build-manifest.json");
    if !manifest_path.is_file() {
        return legacy_runtime(request, &root);
    }
    let manifest: BuildManifest = serde_json::from_slice(
        &std::fs::read(&manifest_path).map_err(|source| io(&manifest_path, source))?,
    )?;
    if manifest.schema_version != "ae-sdd-compiled-runtime/v1" || manifest.source_files.is_empty() {
        return Err(OfflineError::InvalidArtifact(display(&manifest_path)));
    }
    let mut paths = BTreeSet::new();
    let mut bytes = 0_u64;
    for entry in &manifest.source_files {
        let relative = Path::new(&entry.path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            || !paths.insert(entry.path.clone())
            || entry.digest.len() != 64
            || !matches!(entry.permission.as_str(), "PrivateFile" | "Executable")
        {
            return Err(OfflineError::InvalidArtifact(entry.path.clone()));
        }
        if relative.extension().and_then(|value| value.to_str()) == Some("py")
            || matches!(
                relative
                    .components()
                    .next()
                    .and_then(|value| value.as_os_str().to_str()),
                Some("scripts" | "tools")
            )
        {
            return Err(OfflineError::InvalidArtifact(entry.path.clone()));
        }
        let path = root.join(relative);
        let metadata = std::fs::symlink_metadata(&path).map_err(|source| io(&path, source))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(OfflineError::InvalidArtifact(display(&path)));
        }
        let content = std::fs::read(&path).map_err(|source| io(&path, source))?;
        if hex::encode(Sha256::digest(&content)) != entry.digest {
            return Err(OfflineError::InvalidArtifact(format!(
                "digest mismatch: {}",
                display(&path)
            )));
        }
        bytes = bytes.saturating_add(metadata.len());
    }
    let inventory = package_inventory(&root)?;
    if inventory.python_files != 0 || inventory.forbidden_roots != 0 {
        return Err(OfflineError::InvalidArtifact(format!(
            "native package contains {} Python files and {} forbidden tool roots",
            inventory.python_files, inventory.forbidden_roots
        )));
    }
    let mut packaged_paths = inventory.paths.clone();
    if !packaged_paths.remove("runtime/build-manifest.json") || packaged_paths != paths {
        return Err(OfflineError::InvalidArtifact(
            "native package inventory differs from the signed build manifest".to_owned(),
        ));
    }
    Ok(result(
        request,
        Vec::new(),
        serde_json::json!({
            "packageDirectory": display(&root),
            "format": "native",
            "valid": true,
            "verifiedFiles": paths.len(),
            "verifiedBytes": bytes,
            "pythonRuntimeFiles": 0
        }),
        None,
    ))
}

fn legacy_runtime(request: &OfflineRequest, root: &Path) -> Result<OfflineResult, OfflineError> {
    let manifest_path = root.join("runtime/manifest.json");
    let manifest: LegacyManifest = serde_json::from_slice(
        &std::fs::read(&manifest_path).map_err(|source| io(&manifest_path, source))?,
    )?;
    if manifest.schema != "ae-sdd-runtime/v1"
        || !manifest.compiled
        || !manifest.deterministic
        || !lower_hex_digest(&manifest.runtime_fingerprint)
        || manifest.entry != "SKILL.md"
        || manifest.load_order.is_empty()
        || manifest.generated_files.is_empty()
    {
        return Err(OfflineError::InvalidArtifact(display(&manifest_path)));
    }
    for relative in manifest
        .load_order
        .iter()
        .chain(manifest.generated_files.iter())
    {
        let path = safe_package_path(root, relative)?;
        if !path.is_file() {
            return Err(OfflineError::InvalidArtifact(display(&path)));
        }
    }
    let skill = std::fs::read_to_string(root.join("SKILL.md"))
        .map_err(|source| io(&root.join("SKILL.md"), source))?;
    if !skill.contains("compiled: true") || !skill.contains(&manifest.runtime_fingerprint) {
        return Err(OfflineError::InvalidArtifact(display(
            &root.join("SKILL.md"),
        )));
    }
    let inventory = package_inventory(root)?;
    Ok(result(
        request,
        Vec::new(),
        serde_json::json!({
            "packageDirectory": display(root),
            "format": "legacy-oracle",
            "valid": true,
            "verifiedFiles": inventory.files,
            "verifiedBytes": inventory.bytes,
            "pythonRuntimeFiles": inventory.python_files
        }),
        None,
    ))
}

fn safe_package_path(root: &Path, relative: &str) -> Result<PathBuf, OfflineError> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(OfflineError::InvalidArtifact(relative.to_owned()));
    }
    Ok(root.join(path))
}

fn lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Default)]
struct PackageInventory {
    files: usize,
    bytes: u64,
    python_files: usize,
    forbidden_roots: usize,
    paths: BTreeSet<String>,
}

fn package_inventory(root: &Path) -> Result<PackageInventory, OfflineError> {
    let mut inventory = PackageInventory::default();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).map_err(|source| io(&directory, source))? {
            let entry = entry.map_err(|source| io(&directory, source))?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|source| io(&path, source))?;
            if kind.is_symlink() {
                return Err(OfflineError::InvalidArtifact(display(&path)));
            }
            if kind.is_dir() {
                pending.push(path);
                continue;
            }
            if !kind.is_file() {
                return Err(OfflineError::InvalidArtifact(display(&path)));
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|_| OfflineError::InvalidArtifact(display(&path)))?;
            let first = relative
                .components()
                .next()
                .and_then(|component| component.as_os_str().to_str());
            if matches!(first, Some("scripts" | "tools")) {
                inventory.forbidden_roots = inventory.forbidden_roots.saturating_add(1);
            }
            if matches!(
                relative
                    .extension()
                    .and_then(|extension| extension.to_str()),
                Some("py" | "pyc" | "pyo")
            ) {
                inventory.python_files = inventory.python_files.saturating_add(1);
            }
            inventory.paths.insert(display(relative));
            inventory.files = inventory.files.saturating_add(1);
            inventory.bytes = inventory
                .bytes
                .saturating_add(entry.metadata().map_err(|source| io(&path, source))?.len());
        }
    }
    Ok(inventory)
}
