//! Reads the distributor registry that declares which agent hosts receive the
//! compiled package.
//!
//! Distribution targets used to be hardcoded in the post-commit hook while the
//! registry at `~/.ae-sdd/distributors.json` stayed data-driven, so the two
//! drifted: a host registered there never reached the hook, and a host listed
//! only in the hook kept receiving a package with nothing installed to consume
//! it. Deriving both the package targets and the managed-instruction targets
//! from the registry leaves one place to edit.
//!
//! `detect` is honoured, so a host that is registered but not present is
//! skipped rather than having its directory created as a side effect.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::managed_instructions::InstructionLanguage;

/// How the package reaches a host.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributorProtocol {
    /// A self-contained skill directory tree.
    Copytree,
    /// An agent-home mount with a different layout; no native implementation.
    HarnessMount,
}

/// When a host counts as present.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributorDetect {
    /// Always present.
    Always,
    /// The resolved target path exists.
    PathExists,
    /// `detect_cli` resolves on `PATH`.
    CliExists,
}

/// The registry schema this reader understands.
const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// The registry file's envelope.
///
/// A missing or renamed `distributors` key is a parse failure rather than an
/// empty host list. Reading it as empty is how a registry comes to look healthy
/// while distributing to a different set of hosts than it declares, so this
/// fails closed instead.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
struct RegistryEnvelope {
    schema_version: u32,
    distributors: Vec<RegistryHostEntry>,
}

/// One registered host.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RegistryHostEntry {
    pub name: String,
    pub protocol: DistributorProtocol,
    pub target_path: String,
    pub detect: DistributorDetect,
    #[serde(default)]
    pub detect_cli: Option<String>,
    pub enabled: bool,
    #[serde(default)]
    pub l2_global_file: Option<String>,
    #[serde(default)]
    pub l2_language: Option<String>,
    /// Fields this reader does not interpret are carried through rather than
    /// rejected, so an entry written by a newer schema still resolves.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// A host that passed `enabled` and `detect`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedHost {
    pub name: String,
    pub package_target: PathBuf,
    pub instruction_target: Option<(PathBuf, InstructionLanguage)>,
}

/// Why a registered host was not resolved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkippedHost {
    pub name: String,
    pub reason: SkipReason,
}

/// The specific check that excluded a host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkipReason {
    /// `enabled` is false.
    Disabled,
    /// `detect` did not pass.
    NotDetected,
}

impl SkipReason {
    /// Returns the stable label used in operator output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::NotDetected => "not-detected",
        }
    }
}

/// The outcome of reading one registry.
#[derive(Clone, Debug)]
pub struct RegistryResolution {
    pub hosts: Vec<ResolvedHost>,
    pub skipped: Vec<SkippedHost>,
}

/// Why a registry could not be read.
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("distributor registry {path} could not be read: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("distributor registry {path} is not valid JSON: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "distributor registry {path} declares schema version {found}, but only {supported} is \
         understood; refusing to guess which hosts it means"
    )]
    UnsupportedSchema {
        path: String,
        found: u32,
        supported: u32,
    },
    #[error("distributor registry declares host {name} twice")]
    DuplicateHost { name: String },
    #[error(
        "host {name} uses protocol harness_mount, which has no native implementation; \
         disable the entry or add native support before distributing"
    )]
    UnsupportedProtocol { name: String },
    #[error("host {name} declares l2GlobalFile without l2Language")]
    MissingLanguage { name: String },
    #[error("host {name} declares an unknown l2Language {language}")]
    UnknownLanguage { name: String, language: String },
    #[error("host {name} has an empty target path")]
    EmptyTarget { name: String },
}

/// Resolves every enabled and detected host declared in the registry.
///
/// `home` supplies the `~` expansion so the caller controls it and tests need
/// no real home directory. An entry whose `detect` fails is reported as skipped
/// rather than silently dropped, because a host disappearing from distribution
/// is exactly the failure this registry exists to make visible.
///
/// # Errors
///
/// Returns [`RegistryError`] when the file cannot be read or parsed, when a host
/// is declared twice, when a host requests `harness_mount` (no native
/// implementation exists, so proceeding would report success while distributing
/// nothing), or when an instruction declaration is incomplete.
pub fn resolve_registry(path: &Path, home: &Path) -> Result<RegistryResolution, RegistryError> {
    let display = path.display().to_string();
    let contents = std::fs::read_to_string(path).map_err(|source| RegistryError::Io {
        path: display.clone(),
        source,
    })?;
    let envelope: RegistryEnvelope =
        serde_json::from_str(&contents).map_err(|source| RegistryError::Json {
            path: display.clone(),
            source,
        })?;
    if envelope.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(RegistryError::UnsupportedSchema {
            path: display,
            found: envelope.schema_version,
            supported: SUPPORTED_SCHEMA_VERSION,
        });
    }
    let entries = envelope.distributors;

    let mut seen = std::collections::BTreeSet::new();
    let mut hosts = Vec::new();
    let mut skipped = Vec::new();
    for entry in entries {
        if !seen.insert(entry.name.clone()) {
            return Err(RegistryError::DuplicateHost { name: entry.name });
        }
        if !entry.enabled {
            skipped.push(SkippedHost {
                name: entry.name,
                reason: SkipReason::Disabled,
            });
            continue;
        }
        if entry.protocol == DistributorProtocol::HarnessMount {
            return Err(RegistryError::UnsupportedProtocol { name: entry.name });
        }
        let package_target = expand(&entry.target_path, home);
        if package_target.as_os_str().is_empty() {
            return Err(RegistryError::EmptyTarget { name: entry.name });
        }
        if !detected(&entry, &package_target) {
            skipped.push(SkippedHost {
                name: entry.name,
                reason: SkipReason::NotDetected,
            });
            continue;
        }
        let instruction_target = match (&entry.l2_global_file, &entry.l2_language) {
            (None, _) => None,
            (Some(_), None) => {
                return Err(RegistryError::MissingLanguage { name: entry.name });
            }
            (Some(file), Some(language)) => {
                let language = match language.as_str() {
                    "en" => InstructionLanguage::En,
                    "zh" => InstructionLanguage::Zh,
                    other => {
                        return Err(RegistryError::UnknownLanguage {
                            name: entry.name,
                            language: other.to_owned(),
                        });
                    }
                };
                Some((expand(file, home), language))
            }
        };
        hosts.push(ResolvedHost {
            name: entry.name,
            package_target,
            instruction_target,
        });
    }
    Ok(RegistryResolution { hosts, skipped })
}

/// Expands a leading `~` against `home`; other paths are taken verbatim.
fn expand(value: &str, home: &Path) -> PathBuf {
    match value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        Some(relative) => home.join(relative),
        None if value == "~" => home.to_path_buf(),
        None => PathBuf::from(value),
    }
}

/// Applies the registry's `detect` contract.
///
/// `cli_exists` without a `detect_cli` is treated as absent rather than as an
/// error: the registry is user-writable state, and a half-filled entry must not
/// stop distribution to every other host.
fn detected(entry: &RegistryHostEntry, target: &Path) -> bool {
    match entry.detect {
        DistributorDetect::Always => true,
        DistributorDetect::PathExists => target.exists(),
        DistributorDetect::CliExists => entry
            .detect_cli
            .as_deref()
            .filter(|value| !value.is_empty())
            .is_some_and(cli_on_path),
    }
}

/// Returns whether `name` resolves on `PATH`, honouring `PATHEXT` on Windows.
fn cli_on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let extensions: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned())
            .split(';')
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase)
            .collect()
    } else {
        Vec::new()
    };
    std::env::split_paths(&path).any(|directory| {
        if directory.join(name).is_file() {
            return true;
        }
        extensions
            .iter()
            .any(|extension| directory.join(format!("{name}{extension}")).is_file())
    })
}
