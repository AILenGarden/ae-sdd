use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use ae_sdd_domain::ProjectRelativePath;
use ae_sdd_methodology::{
    MethodologyAssetSource, compile_catalog, decode_bundle, encode_bundle, verify_builtin_coverage,
    verify_bundle,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::*;

const METHODOLOGY_CATALOG_PATH: &str = "standards/runtime/methodology-catalog.v1.json";
const METHODOLOGY_BUNDLE_PATH: &str = "runtime/methodology/catalog.v1.json";
const MAX_RUNTIME_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_RUNTIME_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_RUNTIME_PACKAGE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_RUNTIME_PACKAGE_FILES: usize = 50_000;
const MAX_RUNTIME_DIRECTORY_DEPTH: usize = 64;

struct PackageAssets(BTreeMap<ProjectRelativePath, Vec<u8>>);

impl MethodologyAssetSource for PackageAssets {
    fn read(&self, path: &ProjectRelativePath) -> Option<&[u8]> {
        self.0.get(path).map(Vec::as_slice)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuildManifest {
    schema_version: String,
    #[serde(default)]
    manifest_kind: Option<String>,
    source_files: Vec<ManifestEntry>,
    #[serde(default)]
    methodology: Option<MethodologyManifest>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestEntry {
    path: String,
    digest: String,
    permission: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MethodologyManifest {
    schema_version: String,
    bundle_path: String,
    bundle_digest: String,
    catalog_digest: String,
    entry_count: usize,
    entries: Vec<MethodologyManifestEntry>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MethodologyManifestEntry {
    skill_id: String,
    series_kind: String,
    variant: String,
    entry_digest: String,
    compact_path: String,
    compact_digest: String,
    fallback_path: Option<String>,
    fallback_digest: Option<String>,
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
    let manifest: BuildManifest = serde_json::from_slice(&read_package_file(
        &manifest_path,
        MAX_RUNTIME_MANIFEST_BYTES,
    )?)?;
    if manifest.schema_version != "ae-sdd-compiled-runtime/v1"
        || manifest
            .manifest_kind
            .as_deref()
            .is_some_and(|kind| kind != "content-addressed-package")
        || manifest.source_files.is_empty()
    {
        return Err(OfflineError::InvalidArtifact(display(&manifest_path)));
    }
    if manifest.source_files.len() > MAX_RUNTIME_PACKAGE_FILES {
        return Err(OfflineError::InvalidArtifact(
            "native package manifest exceeds its entry budget".to_owned(),
        ));
    }
    let mut paths = BTreeSet::new();
    let mut bytes = 0_u64;
    for entry in &manifest.source_files {
        let relative = Path::new(&entry.path);
        if ProjectRelativePath::new(entry.path.clone()).is_err()
            || relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            || relative.components().count() > MAX_RUNTIME_DIRECTORY_DEPTH
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
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() > MAX_RUNTIME_FILE_BYTES
            || bytes.saturating_add(metadata.len()) > MAX_RUNTIME_PACKAGE_BYTES
        {
            return Err(OfflineError::InvalidArtifact(display(&path)));
        }
        let content = read_package_file(&path, MAX_RUNTIME_FILE_BYTES)?;
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
            "native package inventory differs from the digested build manifest".to_owned(),
        ));
    }
    let methodology_entries = verify_methodology(&root, &paths, manifest.methodology.as_ref())?;
    Ok(result(
        request,
        Vec::new(),
        serde_json::json!({
            "packageDirectory": display(&root),
            "format": "native",
            "valid": true,
            "verifiedFiles": paths.len(),
            "verifiedBytes": bytes,
            "pythonRuntimeFiles": 0,
            "methodologyEntries": methodology_entries.unwrap_or(0)
        }),
        None,
    ))
}

fn verify_methodology(
    root: &Path,
    packaged_paths: &BTreeSet<String>,
    manifest: Option<&MethodologyManifest>,
) -> Result<Option<usize>, OfflineError> {
    let has_source = packaged_paths.contains(METHODOLOGY_CATALOG_PATH);
    let has_bundle = packaged_paths.contains(METHODOLOGY_BUNDLE_PATH);
    let Some(manifest) = manifest else {
        if has_bundle {
            return Err(OfflineError::InvalidArtifact(
                "Methodology bundle requires a versioned manifest extension".to_owned(),
            ));
        }
        return Ok(None);
    };
    if !has_source || !has_bundle {
        return Err(OfflineError::InvalidArtifact(
            "Methodology source Catalog and compiled bundle must be packaged together".to_owned(),
        ));
    }

    let bundle_path = safe_package_path(root, METHODOLOGY_BUNDLE_PATH)?;
    let bundle_bytes = read_package_file(&bundle_path, MAX_RUNTIME_FILE_BYTES)?;
    let bundle = decode_bundle(&bundle_bytes).map_err(methodology_artifact_error)?;
    verify_builtin_coverage(&bundle).map_err(methodology_artifact_error)?;
    let actual_entries = bundle
        .entries()
        .iter()
        .map(|entry| MethodologyManifestEntry {
            skill_id: entry.skill_id().to_string(),
            series_kind: entry.series_kind().to_string(),
            variant: entry.variant().to_string(),
            entry_digest: entry.entry_digest().to_string(),
            compact_path: entry.compact_ref().path().to_string(),
            compact_digest: entry.compact_ref().digest().to_string(),
            fallback_path: entry
                .fallback_ref()
                .map(|reference| reference.path().to_string()),
            fallback_digest: entry
                .fallback_ref()
                .map(|reference| reference.digest().to_string()),
        })
        .collect::<Vec<_>>();
    let metadata_matches = manifest.schema_version == "ae-sdd-methodology-manifest/v1"
        && manifest.bundle_path == METHODOLOGY_BUNDLE_PATH
        && manifest.bundle_digest == hex::encode(Sha256::digest(&bundle_bytes))
        && manifest.catalog_digest == bundle.catalog_digest().to_string()
        && manifest.entry_count == bundle.entry_count()
        && manifest.entry_count == manifest.entries.len()
        && manifest.entries == actual_entries;
    if !metadata_matches {
        return Err(OfflineError::InvalidArtifact(
            "Methodology manifest metadata differs from the compiled bundle".to_owned(),
        ));
    }

    let mut assets = BTreeMap::new();
    for reference in bundle
        .entries()
        .iter()
        .flat_map(|entry| std::iter::once(entry.compact_ref()).chain(entry.fallback_ref()))
    {
        if assets.contains_key(reference.path()) {
            continue;
        }
        let artifact_path = safe_package_path(root, reference.path().as_str())?;
        let content = read_package_file(&artifact_path, MAX_RUNTIME_FILE_BYTES)?;
        assets.insert(reference.path().clone(), content);
    }
    let assets = PackageAssets(assets);
    verify_bundle(&bundle, &assets).map_err(methodology_artifact_error)?;

    let source_path = safe_package_path(root, METHODOLOGY_CATALOG_PATH)?;
    let source = read_package_file(&source_path, MAX_RUNTIME_FILE_BYTES)?;
    let rebuilt = compile_catalog(&source, &assets).map_err(methodology_artifact_error)?;
    verify_builtin_coverage(&rebuilt).map_err(methodology_artifact_error)?;
    let rebuilt_bytes = encode_bundle(&rebuilt).map_err(methodology_artifact_error)?;
    if rebuilt_bytes != bundle_bytes {
        return Err(OfflineError::InvalidArtifact(
            "compiled Methodology bundle differs from packaged source Catalog".to_owned(),
        ));
    }
    Ok(Some(bundle.entry_count()))
}

fn methodology_artifact_error(error: impl std::fmt::Display) -> OfflineError {
    OfflineError::InvalidArtifact(format!("invalid Methodology artifact: {error}"))
}

fn legacy_runtime(request: &OfflineRequest, root: &Path) -> Result<OfflineResult, OfflineError> {
    let manifest_path = root.join("runtime/manifest.json");
    let manifest: LegacyManifest = serde_json::from_slice(&read_package_file(
        &manifest_path,
        MAX_RUNTIME_MANIFEST_BYTES,
    )?)?;
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
                let depth = path
                    .strip_prefix(root)
                    .map_err(|_| OfflineError::InvalidArtifact(display(&path)))?
                    .components()
                    .count();
                if depth > MAX_RUNTIME_DIRECTORY_DEPTH {
                    return Err(OfflineError::InvalidArtifact(
                        "native package directory depth exceeds budget".to_owned(),
                    ));
                }
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
            let metadata = entry.metadata().map_err(|source| io(&path, source))?;
            if inventory.files >= MAX_RUNTIME_PACKAGE_FILES
                || metadata.len() > MAX_RUNTIME_FILE_BYTES
                || inventory.bytes.saturating_add(metadata.len()) > MAX_RUNTIME_PACKAGE_BYTES
            {
                return Err(OfflineError::InvalidArtifact(
                    "native package inventory exceeds file or byte budget".to_owned(),
                ));
            }
            inventory.paths.insert(display(relative));
            inventory.files = inventory.files.saturating_add(1);
            inventory.bytes = inventory.bytes.saturating_add(metadata.len());
        }
    }
    Ok(inventory)
}

fn read_package_file(path: &Path, limit: u64) -> Result<Vec<u8>, OfflineError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|source| io(path, source))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > limit {
        return Err(OfflineError::InvalidArtifact(format!(
            "native package file violates its byte budget: {}",
            display(path)
        )));
    }
    std::fs::read(path).map_err(|source| io(path, source))
}
