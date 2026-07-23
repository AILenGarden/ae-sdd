use std::fs;
use std::path::{Path, PathBuf};

use ae_sdd_domain::{ArtifactDigest, ProjectRelativePath};
use ae_sdd_protocol::{StableErrorCode, WorkspaceMode};
use ae_sdd_runtime::{BusinessWorkspace, RuntimeError, RuntimeResult};
use serde_json::Value;

pub(super) const MAX_FILE_BYTES: u64 = 1_048_576;
pub(super) const MAX_ASSET_BYTES: u64 = 8 * MAX_FILE_BYTES;

pub(super) struct JobContext<'a> {
    pub(super) workspace: &'a BusinessWorkspace,
    pub(super) root: PathBuf,
    pub(super) runtime_database: &'a Path,
}

impl<'a> JobContext<'a> {
    pub(super) fn new(
        workspace: &'a BusinessWorkspace,
        runtime_database: &'a Path,
    ) -> RuntimeResult<Self> {
        let root = fs::canonicalize(&workspace.canonical_root).map_err(io_error)?;
        if !root.is_dir() {
            return Err(external_error("registered workspace root is not a directory"));
        }
        Ok(Self {
            workspace,
            root,
            runtime_database,
        })
    }

    pub(super) fn existing_file(&self, value: &str) -> RuntimeResult<PathBuf> {
        let raw = Path::new(value);
        let candidate = if raw.is_absolute() {
            raw.to_path_buf()
        } else {
            let relative = ProjectRelativePath::new(value.replace('\\', "/"))
                .map_err(|_| schema_error("job path must be project-relative and traversal-free"))?;
            self.root.join(relative.as_str())
        };
        let canonical = fs::canonicalize(candidate).map_err(io_error)?;
        if !canonical.starts_with(&self.root) || !canonical.is_file() {
            return Err(RuntimeError::new(
                StableErrorCode::WorkspaceOutsideAllowedRoot,
                "job path is not a regular file inside the registered workspace",
            ));
        }
        Ok(canonical)
    }

    pub(super) fn project_file(&self, relative: &str) -> RuntimeResult<PathBuf> {
        self.existing_file(relative)
    }
}

pub(super) fn read_bounded(path: &Path, limit: u64) -> RuntimeResult<Vec<u8>> {
    let metadata = fs::metadata(path).map_err(io_error)?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(schema_error("job input file exceeds its bounded read contract"));
    }
    let bytes = fs::read(path).map_err(io_error)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Err(schema_error("job input changed beyond its bounded read contract"));
    }
    Ok(bytes)
}

pub(super) fn read_json(path: &Path, limit: u64) -> RuntimeResult<Value> {
    serde_json::from_slice(&read_bounded(path, limit)?)
        .map_err(|_| schema_error("job JSON input is invalid"))
}

pub(super) fn required_string<'a>(arguments: &'a Value, name: &str) -> RuntimeResult<&'a str> {
    let value = arguments
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| schema_error(&format!("{name} is required")))?;
    if value.len() > 4_096 {
        return Err(schema_error(&format!("{name} exceeds its length bound")));
    }
    Ok(value)
}

pub(super) fn bounded_u64(arguments: &Value, name: &str, default: u64, max: u64) -> RuntimeResult<u64> {
    let value = arguments.get(name).and_then(Value::as_u64).unwrap_or(default);
    if value == 0 || value > max {
        return Err(schema_error(&format!("{name} must be between 1 and {max}")));
    }
    Ok(value)
}

pub(super) fn safe_segment(value: &str, name: &str) -> RuntimeResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 128
        || !trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
        || trimmed == "."
        || trimmed == ".."
    {
        return Err(schema_error(&format!("{name} is not a safe identifier")));
    }
    Ok(trimmed.to_owned())
}

pub(super) fn digest(bytes: &[u8]) -> String {
    ArtifactDigest::digest(bytes).to_string()
}

pub(super) fn mutation_rejected(context: &JobContext<'_>, entrypoint: &str) -> RuntimeResult<Value> {
    let (code, message) = match context.workspace.mode {
        WorkspaceMode::Legacy | WorkspaceMode::Shadow => (
            StableErrorCode::RoleOperationForbidden,
            "workspace mode does not permit Rust mutation jobs",
        ),
        WorkspaceMode::RustCanary | WorkspaceMode::RustSoleWriter => (
            StableErrorCode::LeaseRequired,
            "mutation jobs require the typed operation lease, revision, and idempotency envelope",
        ),
    };
    Err(RuntimeError::new(
        code,
        format!("{entrypoint}: {message}"),
    ))
}

pub(super) fn unsupported(entrypoint: &str) -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::OperationNotRegistered,
        format!("legacy job entrypoint is not registered: {entrypoint}"),
    )
}

pub(super) fn schema_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::OperationSchemaInvalid, message)
}

pub(super) fn external_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::ExternalStateConflict, message)
}

pub(super) fn io_error(_error: std::io::Error) -> RuntimeError {
    external_error("bounded project file I/O failed")
}
