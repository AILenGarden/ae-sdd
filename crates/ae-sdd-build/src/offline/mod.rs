use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::{
    AdminChange, ExecutionMode, InitInput, JobInput, JobReceipt, NativeJobRequest,
    execute_native_job,
};

mod assets;
mod bootstrap;
mod distributor;
mod verify;

const OFFLINE_SCHEMA: &str = "ae-sdd-offline-build/v1";

pub const B_OFFLINE_ENTRYPOINTS: [&str; 13] = [
    "assets.generate",
    "bump",
    "distributor.disable",
    "distributor.enable",
    "distributor.list",
    "distributor.register",
    "distributor.scan",
    "distributor.unregister",
    "init",
    "init-hooks",
    "plugin.init",
    "runtime.verify",
    "version",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineRequest {
    pub schema_version: String,
    pub mode: ExecutionMode,
    pub actor: String,
    pub reason: String,
    pub idempotency_key: String,
    #[serde(flatten)]
    pub command: OfflineCommand,
}

impl<'de> Deserialize<'de> for OfflineRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let mut object = value
            .as_object()
            .cloned()
            .ok_or_else(|| serde::de::Error::custom("offline request must be an object"))?;
        let schema_version =
            take_wire_field(&mut object, "schemaVersion").map_err(serde::de::Error::custom)?;
        let mode = take_wire_field(&mut object, "mode").map_err(serde::de::Error::custom)?;
        let actor = take_wire_field(&mut object, "actor").map_err(serde::de::Error::custom)?;
        let reason = take_wire_field(&mut object, "reason").map_err(serde::de::Error::custom)?;
        let idempotency_key =
            take_wire_field(&mut object, "idempotencyKey").map_err(serde::de::Error::custom)?;
        let command =
            serde_json::from_value(Value::Object(object)).map_err(serde::de::Error::custom)?;
        Ok(Self {
            schema_version,
            mode,
            actor,
            reason,
            idempotency_key,
            command,
        })
    }
}

fn take_wire_field<T: DeserializeOwned>(
    object: &mut Map<String, Value>,
    field: &'static str,
) -> Result<T, String> {
    let value = object
        .remove(field)
        .ok_or_else(|| format!("missing field `{field}`"))?;
    serde_json::from_value(value).map_err(|error| format!("invalid field `{field}`: {error}"))
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "command",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum OfflineCommand {
    #[serde(rename = "assets.generate")]
    AssetsGenerate {
        project_root: PathBuf,
        project_key: String,
    },
    Bump {
        repository_root: PathBuf,
        expected_version: String,
        new_version: String,
    },
    #[serde(rename = "distributor.disable")]
    DistributorDisable {
        registry_file: PathBuf,
        name: String,
    },
    #[serde(rename = "distributor.enable")]
    DistributorEnable {
        registry_file: PathBuf,
        name: String,
    },
    #[serde(rename = "distributor.list")]
    DistributorList {
        registry_file: PathBuf,
    },
    #[serde(rename = "distributor.register")]
    DistributorRegister {
        registry_file: PathBuf,
        entry: DistributorEntry,
    },
    #[serde(rename = "distributor.scan")]
    DistributorScan {
        registry_file: PathBuf,
    },
    #[serde(rename = "distributor.unregister")]
    DistributorUnregister {
        registry_file: PathBuf,
        name: String,
    },
    Init {
        project_root: PathBuf,
        project_key: String,
        force: bool,
    },
    #[serde(rename = "init-hooks")]
    InitHooks {
        project_root: PathBuf,
        executable: String,
        hosts: Vec<String>,
    },
    #[serde(rename = "plugin.init")]
    PluginInit {
        plugins_root: PathBuf,
        name: String,
        description: String,
    },
    #[serde(rename = "runtime.verify")]
    RuntimeVerify {
        package_directory: PathBuf,
    },
    Version,
}

impl OfflineCommand {
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::AssetsGenerate { .. } => "assets.generate",
            Self::Bump { .. } => "bump",
            Self::DistributorDisable { .. } => "distributor.disable",
            Self::DistributorEnable { .. } => "distributor.enable",
            Self::DistributorList { .. } => "distributor.list",
            Self::DistributorRegister { .. } => "distributor.register",
            Self::DistributorScan { .. } => "distributor.scan",
            Self::DistributorUnregister { .. } => "distributor.unregister",
            Self::Init { .. } => "init",
            Self::InitHooks { .. } => "init-hooks",
            Self::PluginInit { .. } => "plugin.init",
            Self::RuntimeVerify { .. } => "runtime.verify",
            Self::Version => "version",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DistributorEntry {
    pub name: String,
    pub kind: String,
    pub target_path: PathBuf,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineResult {
    pub schema_version: &'static str,
    pub command: &'static str,
    pub mode: ExecutionMode,
    pub changed_paths: Vec<String>,
    pub payload: Value,
    pub receipt: Option<JobReceipt>,
}

pub fn execute_offline(request: &OfflineRequest) -> Result<OfflineResult, OfflineError> {
    validate_request(request)?;
    match &request.command {
        OfflineCommand::AssetsGenerate {
            project_root,
            project_key,
        } => assets::generate(request, project_root, project_key),
        OfflineCommand::Bump {
            repository_root,
            expected_version,
            new_version,
        } => bootstrap::bump(request, repository_root, expected_version, new_version),
        OfflineCommand::DistributorDisable {
            registry_file,
            name,
        } => distributor::set_enabled(request, registry_file, name, false),
        OfflineCommand::DistributorEnable {
            registry_file,
            name,
        } => distributor::set_enabled(request, registry_file, name, true),
        OfflineCommand::DistributorList { registry_file } => {
            distributor::list(request, registry_file)
        }
        OfflineCommand::DistributorRegister {
            registry_file,
            entry,
        } => distributor::register(request, registry_file, entry),
        OfflineCommand::DistributorScan { registry_file } => {
            distributor::scan(request, registry_file)
        }
        OfflineCommand::DistributorUnregister {
            registry_file,
            name,
        } => distributor::unregister(request, registry_file, name),
        OfflineCommand::Init {
            project_root,
            project_key,
            force,
        } => bootstrap::init(request, project_root, project_key, *force),
        OfflineCommand::InitHooks {
            project_root,
            executable,
            hosts,
        } => bootstrap::init_hooks(request, project_root, executable, hosts),
        OfflineCommand::PluginInit {
            plugins_root,
            name,
            description,
        } => bootstrap::plugin_init(request, plugins_root, name, description),
        OfflineCommand::RuntimeVerify { package_directory } => {
            verify::runtime(request, package_directory)
        }
        OfflineCommand::Version => Ok(result(
            request,
            Vec::new(),
            serde_json::json!({
                "name": "ae-sdd",
                "version": env!("AE_SDD_PRODUCT_VERSION"),
                "runtime": "rust"
            }),
            None,
        )),
    }
}

fn validate_request(request: &OfflineRequest) -> Result<(), OfflineError> {
    if request.schema_version != OFFLINE_SCHEMA {
        return Err(OfflineError::Schema(request.schema_version.clone()));
    }
    for value in [
        request.actor.as_str(),
        request.reason.as_str(),
        request.idempotency_key.as_str(),
    ] {
        if value.trim().is_empty() || value.len() > 1_024 || value.contains(['\0', '\r', '\n']) {
            return Err(OfflineError::InvalidInput("request identity"));
        }
    }
    if !B_OFFLINE_ENTRYPOINTS.contains(&request.command.id()) {
        return Err(OfflineError::Unsupported(request.command.id().to_owned()));
    }
    Ok(())
}

fn result(
    request: &OfflineRequest,
    changed_paths: Vec<String>,
    payload: Value,
    receipt: Option<JobReceipt>,
) -> OfflineResult {
    OfflineResult {
        schema_version: "ae-sdd-offline-build-result/v1",
        command: request.command.id(),
        mode: request.mode,
        changed_paths,
        payload,
        receipt,
    }
}

fn apply_changes(
    request: &OfflineRequest,
    root: &Path,
    changes: Vec<AdminChange>,
) -> Result<OfflineResult, OfflineError> {
    let execution = execute_native_job(&NativeJobRequest {
        schema_version: "ae-sdd-native-job/v1".to_owned(),
        entrypoint: "admin".to_owned(),
        actor: request.actor.clone(),
        reason: format!("{}: {}", request.command.id(), request.reason),
        idempotency_key: format!("{}:{}", request.command.id(), request.idempotency_key),
        mode: request.mode,
        allowed_roots: vec![root.to_path_buf()],
        job: JobInput::Admin(InitInput {
            project_root: root.to_path_buf(),
            changes,
        }),
    })?;
    Ok(result(
        request,
        execution
            .changes
            .iter()
            .map(|change| change.destination.clone())
            .collect(),
        serde_json::json!({
            "planDigest": execution.plan_digest,
            "replayed": execution.replayed
        }),
        execution.receipt,
    ))
}

fn display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn validate_name(value: &str) -> Result<(), OfflineError> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
    {
        return Err(OfflineError::InvalidInput("name"));
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum OfflineError {
    #[error("unsupported offline request schema {0}")]
    Schema(String),
    #[error("unsupported offline command {0}")]
    Unsupported(String),
    #[error("invalid offline input: {0}")]
    InvalidInput(&'static str),
    #[error("offline target already exists: {0}")]
    AlreadyExists(String),
    #[error("offline artifact is missing or invalid: {0}")]
    InvalidArtifact(String),
    #[error("distributor does not exist: {0}")]
    DistributorMissing(String),
    #[error("distributor already exists: {0}")]
    DistributorExists(String),
    #[error("offline build I/O failed at {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("offline build JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("offline atomic job failed: {0}")]
    Job(#[from] crate::JobError),
}

fn io(path: &Path, source: std::io::Error) -> OfflineError {
    OfflineError::Io {
        path: display(path),
        source,
    }
}
