use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::*;
use crate::{AdminChange, PermissionClass};

const ASSET_SCHEMA: &str = "ae-sdd-project-assets/v1";
const MAX_FILES: usize = 50_000;
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ASSET_BYTES: usize = 8 * 1024 * 1024;

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

    let inventory_digest = inventory_digest(&records);
    let asset_relative = PathBuf::from(format!(".ae-sdd/assets/{project_key}.assets.md"));
    let asset_contents = render(project_key, &inventory_digest, &records);
    if asset_contents.len() > MAX_ASSET_BYTES {
        return Err(OfflineError::InvalidInput("asset document budget"));
    }
    let asset_digest = hex::encode(Sha256::digest(asset_contents.as_bytes()));
    let mut result = apply_changes(
        request,
        &root,
        vec![AdminChange {
            relative_path: asset_relative.clone(),
            contents: asset_contents,
            permission: PermissionClass::PrivateFile,
        }],
    )?;
    result.payload["schemaVersion"] = serde_json::Value::String(ASSET_SCHEMA.to_owned());
    result.payload["projectKey"] = serde_json::Value::String(project_key.to_owned());
    result.payload["assetFile"] = serde_json::Value::String(display(&asset_relative));
    result.payload["assetDigest"] = serde_json::Value::String(asset_digest);
    result.payload["inventoryDigest"] = serde_json::Value::String(inventory_digest);
    result.payload["fileCount"] = serde_json::json!(records.len());
    Ok(result)
}

fn render(project_key: &str, inventory_digest: &str, records: &[AssetRecord]) -> String {
    let mut extensions = BTreeMap::<String, usize>::new();
    for record in records {
        let extension = Path::new(&record.path)
            .extension()
            .and_then(|value| value.to_str())
            .map_or_else(|| "(none)".to_owned(), |value| value.to_ascii_lowercase());
        *extensions.entry(extension).or_default() += 1;
    }

    let mut output = String::new();
    writeln!(output, "---").expect("String writes cannot fail");
    writeln!(output, "schemaVersion: {ASSET_SCHEMA}").expect("String writes cannot fail");
    writeln!(output, "projectKey: {project_key}").expect("String writes cannot fail");
    writeln!(output, "inventoryDigest: {inventory_digest}").expect("String writes cannot fail");
    writeln!(output, "fileCount: {}", records.len()).expect("String writes cannot fail");
    writeln!(output, "---").expect("String writes cannot fail");
    writeln!(output, "# {project_key} Project Assets\n").expect("String writes cannot fail");
    writeln!(output, "## §A Asset Outline\n").expect("String writes cannot fail");
    writeln!(output, "| field | value |").expect("String writes cannot fail");
    writeln!(output, "| --- | --- |").expect("String writes cannot fail");
    writeln!(output, "| schemaVersion | `{ASSET_SCHEMA}` |").expect("String writes cannot fail");
    writeln!(output, "| projectKey | `{project_key}` |").expect("String writes cannot fail");
    writeln!(output, "| inventoryDigest | `{inventory_digest}` |")
        .expect("String writes cannot fail");
    writeln!(output, "| fileCount | `{}` |\n", records.len()).expect("String writes cannot fail");

    writeln!(output, "## §B File Inventory\n").expect("String writes cannot fail");
    writeln!(output, "| path | bytes | sha256 |").expect("String writes cannot fail");
    writeln!(output, "| --- | ---: | --- |").expect("String writes cannot fail");
    for record in records {
        writeln!(
            output,
            "| `{}` | {} | `{}` |",
            markdown_cell(&record.path),
            record.byte_length,
            record.sha256
        )
        .expect("String writes cannot fail");
    }

    writeln!(output, "\n## §C Build And Configuration Index\n").expect("String writes cannot fail");
    let mut selected = records.iter().filter(|record| {
        let name = Path::new(&record.path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        matches!(
            name,
            "Cargo.toml"
                | "pom.xml"
                | "build.gradle"
                | "build.gradle.kts"
                | "package.json"
                | "pyproject.toml"
                | "go.mod"
                | "Dockerfile"
                | "docker-compose.yml"
                | "docker-compose.yaml"
        ) || name.starts_with("application.")
    });
    if let Some(first) = selected.next() {
        writeln!(output, "- `{}`", markdown_cell(&first.path)).expect("String writes cannot fail");
        for record in selected {
            writeln!(output, "- `{}`", markdown_cell(&record.path))
                .expect("String writes cannot fail");
        }
    } else {
        writeln!(
            output,
            "- No build or common configuration file discovered."
        )
        .expect("String writes cannot fail");
    }

    writeln!(output, "\n## §D Source Component Index\n").expect("String writes cannot fail");
    let source_paths = records
        .iter()
        .filter(|record| {
            record.path.starts_with("src/")
                || record.path.contains("/src/")
                || record.path.starts_with("app/")
                || record.path.starts_with("lib/")
        })
        .take(256)
        .collect::<Vec<_>>();
    if source_paths.is_empty() {
        writeln!(output, "- No conventional source path discovered.")
            .expect("String writes cannot fail");
    } else {
        for record in source_paths {
            writeln!(output, "- `{}`", markdown_cell(&record.path))
                .expect("String writes cannot fail");
        }
    }

    writeln!(output, "\n## §E API And Integration Index\n").expect("String writes cannot fail");
    writeln!(
        output,
        "- Deterministic baseline only; enrich typed API and integration contracts through the governed assets update flow."
    )
    .expect("String writes cannot fail");

    writeln!(output, "\n## §F Reverse Keyword Index\n").expect("String writes cannot fail");
    writeln!(output, "| extension | files |").expect("String writes cannot fail");
    writeln!(output, "| --- | ---: |").expect("String writes cannot fail");
    for (extension, count) in extensions {
        writeln!(output, "| `{}` | {} |", markdown_cell(&extension), count)
            .expect("String writes cannot fail");
    }

    writeln!(output, "\n## §G Read API\n").expect("String writes cannot fail");
    writeln!(output, "| operation | bounded result |").expect("String writes cannot fail");
    writeln!(output, "| --- | --- |").expect("String writes cannot fail");
    writeln!(
        output,
        "| `assets.check` | metadata, required sections and digest |"
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "| `assets.query` | ordered matches with explicit truncation metadata |"
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "| `assets.read` / `assets.section` | bounded content with explicit truncation metadata |"
    )
    .expect("String writes cannot fail");
    output
}

fn inventory_digest(records: &[AssetRecord]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"ae-sdd-project-assets-inventory/v1\0");
    for record in records {
        digest.update((record.path.len() as u64).to_be_bytes());
        digest.update(record.path.as_bytes());
        digest.update(record.byte_length.to_be_bytes());
        digest.update(record.sha256.as_bytes());
    }
    hex::encode(digest.finalize())
}

fn markdown_cell(value: &str) -> String {
    value
        .replace('\\', "/")
        .replace('|', "\\|")
        .replace('`', "\\`")
        .replace(['\r', '\n'], " ")
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
