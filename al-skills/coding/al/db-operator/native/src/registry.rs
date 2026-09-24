use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::config::{
    DEFAULT_CONNECT_TIMEOUT_SECONDS, DEFAULT_MAX_ROWS, DEFAULT_STATEMENT_TIMEOUT_SECONDS,
};

pub const SCHEMA_VERSION: u32 = 3;
pub const SERVICE_NAME: &str = "db-operator";

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("cannot read registry {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("registry JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Mysql,
    Postgresql,
}

pub use crate::access::WriteLevel;

impl Engine {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mysql => "mysql",
            Self::Postgresql => "postgresql",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Endpoint {
    pub mode: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SslProfile {
    #[serde(default = "default_ssl_mode")]
    pub mode: String,
    pub ca_file: Option<String>,
    pub client_cert: Option<String>,
    pub client_key: Option<String>,
}

fn default_ssl_mode() -> String {
    "prefer".to_owned()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Limits {
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_seconds: u64,
    #[serde(default = "default_statement_timeout")]
    pub statement_timeout_seconds: u64,
    #[serde(default = "default_max_rows")]
    pub max_rows: usize,
}

fn default_connect_timeout() -> u64 {
    DEFAULT_CONNECT_TIMEOUT_SECONDS
}

fn default_statement_timeout() -> u64 {
    DEFAULT_STATEMENT_TIMEOUT_SECONDS
}

fn default_max_rows() -> usize {
    DEFAULT_MAX_ROWS
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConnectionProfile {
    pub engine: Engine,
    pub endpoint: Endpoint,
    pub database: Option<String>,
    pub username: String,
    pub secret_ref: String,
    #[serde(default)]
    pub ssl: SslProfile,
    #[serde(default = "default_environment")]
    pub environment: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub limits: Limits,
    #[serde(default)]
    pub write_level: WriteLevel,
}

fn default_environment() -> String {
    "unknown".to_owned()
}

impl Default for SslProfile {
    fn default() -> Self {
        Self {
            mode: default_ssl_mode(),
            ca_file: None,
            client_cert: None,
            client_key: None,
        }
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            connect_timeout_seconds: default_connect_timeout(),
            statement_timeout_seconds: default_statement_timeout(),
            max_rows: default_max_rows(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Registry {
    pub version: u32,
    pub default: Option<String>,
    pub connections: BTreeMap<String, ConnectionProfile>,
}

impl Registry {
    pub fn empty() -> Self {
        Self {
            version: SCHEMA_VERSION,
            default: None,
            connections: BTreeMap::new(),
        }
    }

    pub fn from_json_str(input: &str) -> Result<Self, RegistryError> {
        let value: Value = serde_json::from_str(input)?;
        reject_secret_fields(&value, "registry")?;
        let registry: Self = serde_json::from_value(value)?;
        registry.validate()?;
        Ok(registry)
    }

    pub fn load(path: &Path) -> Result<Self, RegistryError> {
        let content = fs::read_to_string(path).map_err(|source| RegistryError::Read {
            path: path.to_owned(),
            source,
        })?;
        Self::from_json_str(&content)
    }

    pub fn load_default() -> Result<Self, RegistryError> {
        Self::load(&registry_path())
    }

    pub fn load_default_or_empty() -> Result<Self, RegistryError> {
        let path = registry_path();
        if path.exists() {
            Self::load(&path)
        } else {
            Ok(Self::empty())
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), RegistryError> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| RegistryError::Read {
                path: parent.to_owned(),
                source,
            })?;
        }
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp_path =
            path.with_file_name(format!(".connections-{}-{suffix}.tmp", std::process::id()));
        let serialized = serde_json::to_vec_pretty(self)?;
        let write_result = (|| -> Result<(), std::io::Error> {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)?;
            file.write_all(&serialized)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            set_private_permissions(&temp_path)?;
            replace_file(&temp_path, path)?;
            set_private_permissions(path)?;
            Ok(())
        })();
        if let Err(source) = write_result {
            let _ = fs::remove_file(&temp_path);
            return Err(RegistryError::Read {
                path: path.to_owned(),
                source,
            });
        }
        Ok(())
    }

    pub fn save_default(&self) -> Result<(), RegistryError> {
        self.save(&registry_path())
    }

    pub fn resolve(
        &self,
        requested: Option<&str>,
    ) -> Result<(&str, &ConnectionProfile), RegistryError> {
        if let Some(alias) = requested {
            let (stored_alias, connection) =
                self.connections.get_key_value(alias).ok_or_else(|| {
                    RegistryError::Invalid(format!("connection {alias:?} is not registered"))
                })?;
            return Ok((stored_alias.as_str(), connection));
        }
        if let Some(alias) = self.default.as_deref() {
            return Ok((alias, &self.connections[alias]));
        }
        match self.connections.len() {
            0 => Err(RegistryError::Invalid(
                "no database connections are registered; run db-operator-admin register".to_owned(),
            )),
            1 => Ok(self
                .connections
                .first_key_value()
                .map(|(key, value)| (key.as_str(), value))
                .unwrap()),
            _ => Err(RegistryError::Invalid(
                "multiple connections are registered; select one with db-operator-admin use <name>"
                    .to_owned(),
            )),
        }
    }

    fn validate(&self) -> Result<(), RegistryError> {
        if self.version != SCHEMA_VERSION {
            return Err(RegistryError::Invalid(format!(
                "unsupported registry version {}; expected {SCHEMA_VERSION}",
                self.version
            )));
        }
        if let Some(default) = self.default.as_deref()
            && !self.connections.contains_key(default)
        {
            return Err(RegistryError::Invalid(format!(
                "default connection {default:?} is not registered"
            )));
        }
        for (alias, connection) in &self.connections {
            validate_alias(alias)?;
            connection.validate(alias)?;
        }
        Ok(())
    }
}

impl ConnectionProfile {
    fn validate(&self, alias: &str) -> Result<(), RegistryError> {
        if self.endpoint.mode != "tcp" {
            return invalid(alias, "endpoint.mode must be tcp");
        }
        if self.endpoint.host.trim().is_empty() {
            return invalid(alias, "host is required");
        }
        if self.username.trim().is_empty() {
            return invalid(alias, "username is required");
        }
        if self.engine == Engine::Postgresql
            && self
                .database
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return invalid(alias, "database is required for PostgreSQL");
        }
        if self.secret_ref != format!("vault://{SERVICE_NAME}/{alias}") {
            return invalid(alias, "secret_ref is invalid");
        }
        if !matches!(
            self.ssl.mode.as_str(),
            "disable" | "prefer" | "require" | "verify-ca" | "verify-full"
        ) {
            return invalid(alias, "ssl.mode is unsupported");
        }
        if !(1..=300).contains(&self.limits.connect_timeout_seconds) {
            return invalid(alias, "connect timeout must be between 1 and 300 seconds");
        }
        if !(1..=3600).contains(&self.limits.statement_timeout_seconds) {
            return invalid(
                alias,
                "statement timeout must be between 1 and 3600 seconds",
            );
        }
        if !(1..=10_000).contains(&self.limits.max_rows) {
            return invalid(alias, "max rows must be between 1 and 10000");
        }
        Ok(())
    }
}

fn invalid<T>(alias: &str, message: &str) -> Result<T, RegistryError> {
    Err(RegistryError::Invalid(format!(
        "connection {alias:?} {message}"
    )))
}

pub fn validate_alias(alias: &str) -> Result<(), RegistryError> {
    let valid = !alias.is_empty()
        && alias.len() <= 64
        && alias.as_bytes()[0].is_ascii_alphanumeric()
        && alias
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(RegistryError::Invalid(format!(
            "connection name {alias:?} is invalid"
        )))
    }
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source_wide: Vec<u16> = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let result = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn reject_secret_fields(value: &Value, path: &str) -> Result<(), RegistryError> {
    const FORBIDDEN: &[&str] = &[
        "access_token",
        "credential",
        "credentials",
        "dsn",
        "passwd",
        "password",
        "secret",
        "token",
        "uri",
        "url",
    ];
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let child_path = format!("{path}.{key}");
                if FORBIDDEN.contains(&key.to_ascii_lowercase().as_str()) {
                    return Err(RegistryError::Invalid(format!(
                        "registry contains forbidden secret-bearing field {child_path}"
                    )));
                }
                reject_secret_fields(child, &child_path)?;
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                reject_secret_fields(child, &format!("{path}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn config_dir() -> PathBuf {
    if let Some(override_path) = env::var_os("DB_OPERATOR_HOME") {
        return PathBuf::from(override_path);
    }
    if cfg!(windows)
        && let Some(base) = env::var_os("APPDATA").or_else(|| env::var_os("LOCALAPPDATA"))
    {
        return PathBuf::from(base).join(SERVICE_NAME);
    }
    if cfg!(target_os = "macos") {
        return home_dir()
            .join("Library")
            .join("Application Support")
            .join(SERVICE_NAME);
    }
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config"))
        .join(SERVICE_NAME)
}

pub fn registry_path() -> PathBuf {
    config_dir().join("connections.json")
}

fn home_dir() -> PathBuf {
    env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
