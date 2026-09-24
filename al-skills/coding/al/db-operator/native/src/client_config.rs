use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::access::WriteLevel;
use thiserror::Error;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientConfig {
    pub endpoint: String,
    pub capability: String,
    pub connection: String,
    #[serde(default)]
    pub write_level: WriteLevel,
}

#[derive(Debug, Error)]
pub enum ClientConfigError {
    #[error("cannot read client configuration {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("client configuration is invalid: {0}")]
    Invalid(String),
    #[error("client configuration JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
}

impl ClientConfig {
    pub fn load_default() -> Result<Self, ClientConfigError> {
        Self::load(&client_config_path())
    }

    pub fn load(path: &Path) -> Result<Self, ClientConfigError> {
        let content = fs::read_to_string(path).map_err(|source| ClientConfigError::Read {
            path: path.to_owned(),
            source,
        })?;
        let config: Self = serde_json::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ClientConfigError> {
        for (name, value) in [
            ("endpoint", self.endpoint.as_str()),
            ("capability", self.capability.as_str()),
            ("connection", self.connection.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ClientConfigError::Invalid(format!(
                    "{name} must not be empty"
                )));
            }
        }
        Ok(())
    }
}

pub fn client_config_path() -> PathBuf {
    if let Some(path) = env::var_os("DB_OPERATOR_CLIENT_CONFIG") {
        return PathBuf::from(path);
    }
    if cfg!(windows)
        && let Some(base) = env::var_os("APPDATA").or_else(|| env::var_os("LOCALAPPDATA"))
    {
        return PathBuf::from(base).join("db-operator").join("client.json");
    }
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    if cfg!(target_os = "macos") {
        return home
            .join("Library")
            .join("Application Support")
            .join("db-operator")
            .join("client.json");
    }
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
        .join("db-operator")
        .join("client.json")
}
