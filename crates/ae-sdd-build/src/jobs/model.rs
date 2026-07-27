use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::GeneratedConfig;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionMode {
    DryRun,
    Apply,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionClass {
    Directory,
    PrivateFile,
    Executable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeJobKind {
    Offline,
    Compile,
    Init,
    Install,
    Distribute,
    Harness,
    Migrate,
    Admin,
}

impl NativeJobKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Compile => "compile",
            Self::Init => "init",
            Self::Install => "install",
            Self::Distribute => "distribute",
            Self::Harness => "harness",
            Self::Migrate => "migrate",
            Self::Admin => "admin",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeEntrypointSpec {
    pub entrypoint: &'static str,
    pub kind: NativeJobKind,
}

pub const NATIVE_ENTRYPOINTS: &[NativeEntrypointSpec] = &[
    NativeEntrypointSpec {
        entrypoint: "admin",
        kind: NativeJobKind::Admin,
    },
    NativeEntrypointSpec {
        entrypoint: "compile",
        kind: NativeJobKind::Compile,
    },
    NativeEntrypointSpec {
        entrypoint: "distribute",
        kind: NativeJobKind::Distribute,
    },
    NativeEntrypointSpec {
        entrypoint: "harness",
        kind: NativeJobKind::Harness,
    },
    NativeEntrypointSpec {
        entrypoint: "init",
        kind: NativeJobKind::Init,
    },
    NativeEntrypointSpec {
        entrypoint: "init-hooks",
        kind: NativeJobKind::Init,
    },
    NativeEntrypointSpec {
        entrypoint: "install",
        kind: NativeJobKind::Install,
    },
    NativeEntrypointSpec {
        entrypoint: "migrate",
        kind: NativeJobKind::Migrate,
    },
    NativeEntrypointSpec {
        entrypoint: "post-commit.compile",
        kind: NativeJobKind::Compile,
    },
    NativeEntrypointSpec {
        entrypoint: "post-commit.distribute",
        kind: NativeJobKind::Distribute,
    },
    NativeEntrypointSpec {
        entrypoint: "post-commit.managed-instructions",
        kind: NativeJobKind::Admin,
    },
];

#[must_use]
pub fn native_entrypoint(entrypoint: &str) -> Option<&'static NativeEntrypointSpec> {
    NATIVE_ENTRYPOINTS
        .iter()
        .find(|spec| spec.entrypoint == entrypoint)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompileInput {
    pub source_directory: PathBuf,
    pub output_directory: PathBuf,
    #[serde(default)]
    pub generated_configs: Vec<GeneratedConfig>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InitInput {
    pub project_root: PathBuf,
    pub changes: Vec<AdminChange>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallInput {
    pub package_directory: PathBuf,
    pub target_directory: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DistributeInput {
    pub package_directory: PathBuf,
    pub target_directories: Vec<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HarnessInput {
    pub source_files: Vec<PathBuf>,
    pub target_file: PathBuf,
    pub title: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MigrateInput {
    pub source_directory: PathBuf,
    pub target_directory: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdminChange {
    pub relative_path: PathBuf,
    pub contents: String,
    pub permission: PermissionClass,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "input", rename_all = "kebab-case")]
pub enum JobInput {
    Compile(CompileInput),
    Init(InitInput),
    Install(InstallInput),
    Distribute(DistributeInput),
    Harness(HarnessInput),
    Migrate(MigrateInput),
    Admin(InitInput),
}

impl JobInput {
    pub(super) const fn kind(&self) -> NativeJobKind {
        match self {
            Self::Compile(_) => NativeJobKind::Compile,
            Self::Init(_) => NativeJobKind::Init,
            Self::Install(_) => NativeJobKind::Install,
            Self::Distribute(_) => NativeJobKind::Distribute,
            Self::Harness(_) => NativeJobKind::Harness,
            Self::Migrate(_) => NativeJobKind::Migrate,
            Self::Admin(_) => NativeJobKind::Admin,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeJobRequest {
    pub schema_version: String,
    pub entrypoint: String,
    pub actor: String,
    pub reason: String,
    pub idempotency_key: String,
    pub mode: ExecutionMode,
    pub allowed_roots: Vec<PathBuf>,
    pub job: JobInput,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedChange {
    pub destination: String,
    pub source: Option<String>,
    pub digest: String,
    pub byte_length: u64,
    pub permission: PermissionClass,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub entrypoint: String,
    pub job_kind: NativeJobKind,
    pub actor: String,
    pub reason: String,
    pub idempotency_key_digest: String,
    pub request_digest: String,
    pub plan_digest: String,
    pub applied_at_unix_ms: u64,
    pub promoted_roots: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobExecution {
    pub schema_version: &'static str,
    pub mode: ExecutionMode,
    pub entrypoint: String,
    pub job_kind: NativeJobKind,
    pub request_digest: String,
    pub plan_digest: String,
    pub changes: Vec<PlannedChange>,
    pub receipt: Option<JobReceipt>,
    pub replayed: bool,
}

#[derive(Debug, Error)]
pub enum JobError {
    #[error("unsupported native job schema: {0}")]
    Schema(String),
    #[error("job field {0} is empty, malformed, or unbounded")]
    InvalidField(&'static str),
    #[error("native job entrypoint is not registered: {0}")]
    EntrypointNotRegistered(String),
    #[error("native job entrypoint {entrypoint} requires {expected:?}, received {actual:?}")]
    EntrypointKindMismatch {
        entrypoint: String,
        expected: NativeJobKind,
        actual: NativeJobKind,
    },
    #[error("allowed root does not exist or is not a directory: {0}")]
    InvalidAllowedRoot(String),
    #[error("path escapes all explicitly allowed roots: {0}")]
    Containment(String),
    #[error("relative path is absolute, empty, or contains '.'/'..': {0}")]
    InvalidRelativePath(String),
    #[error("symbolic links/reparse aliases are not accepted in native job inputs: {0}")]
    SymbolicLink(String),
    #[error("source path does not exist or has an unsupported type: {0}")]
    InvalidSource(String),
    #[error("source and target directories overlap: {0} -> {1}")]
    OverlappingTrees(String, String),
    #[error("native job plan exceeds the bounded file/byte budget")]
    PlanBudgetExceeded,
    #[error("idempotency key was reused with a different request payload")]
    IdempotencyConflict,
    #[error("native job I/O failed at {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("native job serialization failed: {0}")]
    Encode(#[from] serde_json::Error),
}
