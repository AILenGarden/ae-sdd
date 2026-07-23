use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::*;
use crate::{AdminChange, PermissionClass};

const MAX_FILES: usize = 50_000;
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AssetRecord {
    path: String,
    byte_length: u64,
    sha256: String,
}

pub(super) fn generate(
    request: &OfflineRequest,
    project_root: &Path,
    project_key: &str,
) -> Result<OfflineResult, OfflineError> {
    validate_name(project_key)?;
    let root = project_root
        .canonicalize()
        .map_err(|source| io(project_root, source))?;
    if !root.is_dir() {
        return Err(OfflineError::InvalidArtifact(display(&root)));
    }
    let mut records = Vec::new();
    collect(&root, &root, &mut records)?;
    records.sort_by(|left, right| left.path.cmp(&right.path));
    let asset_relative = PathBuf::from(format!(".ae-sdd/assets/{project_key}.assets.json"));
    let asset_contents = serde_json::to_string_pretty(&serde_json::json!({
        "schemaVersion": "ae-sdd-project-assets/v1",
        "projectKey": project_key,
        "projectRoot": display(&root),
        "fileCount": records.len(),
        "files": records
    }))? + "\n";
    let index_contents = serde_json::to_string_pretty(&serde_json::json!({
        "schemaVersion": "ae-sdd-assets-index/v1",
        "projectKey": project_key,
        "asset": display(&asset_relative),
        "generator": "ae-sdd-build/assets.generate"
    }))? + "\n";
    apply_changes(
        request,
        &root,
        vec![
            AdminChange {
                relative_path: asset_relative,
                contents: asset_contents,
                permission: PermissionClass::PrivateFile,
            },
            AdminChange {
                relative_path: PathBuf::from(".ae-sdd/assets/index.json"),
                contents: index_contents,
                permission: PermissionClass::PrivateFile,
            },
        ],
    )
}

fn collect(
    root: &Path,
    directory: &Path,
    records: &mut Vec<AssetRecord>,
) -> Result<(), OfflineError> {
    for entry in std::fs::read_dir(directory).map_err(|source| io(directory, source))? {
        let entry = entry.map_err(|source| io(directory, source))?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|_| OfflineError::InvalidArtifact(display(&path)))?;
        let first = relative
            .components()
            .next()
            .and_then(|part| part.as_os_str().to_str());
        if matches!(first, Some(".git" | ".ae-sdd" | "target" | "dist")) {
            continue;
        }
        let file_type = entry.file_type().map_err(|source| io(&path, source))?;
        if file_type.is_symlink() {
            return Err(OfflineError::InvalidArtifact(display(&path)));
        }
        if file_type.is_dir() {
            collect(root, &path, records)?;
            continue;
        }
        if !file_type.is_file() {
            return Err(OfflineError::InvalidArtifact(display(&path)));
        }
        let metadata = entry.metadata().map_err(|source| io(&path, source))?;
        if metadata.len() > MAX_FILE_BYTES || records.len() >= MAX_FILES {
            return Err(OfflineError::InvalidInput("asset inventory budget"));
        }
        let bytes = std::fs::read(&path).map_err(|source| io(&path, source))?;
        records.push(AssetRecord {
            path: display(relative),
            byte_length: metadata.len(),
            sha256: hex::encode(Sha256::digest(bytes)),
        });
    }
    Ok(())
}
