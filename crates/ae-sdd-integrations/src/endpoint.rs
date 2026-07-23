use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

use ae_sdd_protocol::EndpointManifest;
use atomicwrites::{AtomicFile, OverwriteBehavior};
use sha2::{Digest, Sha256};

use crate::{IntegrationError, IntegrationResult};

/// Stable paths owned by one per-user daemon installation.
#[derive(Clone, Debug)]
pub struct RuntimePaths {
    /// Protected runtime directory.
    pub state_dir: PathBuf,
    /// Protected atomic endpoint manifest.
    pub endpoint_manifest: PathBuf,
    /// Cross-process singleton lock file.
    pub lock_file: PathBuf,
    /// Durable runtime metadata database.
    pub database: PathBuf,
    /// Bounded append-only daemon diagnostic log.
    pub log_file: PathBuf,
    /// Unix UDS path or Windows Named Pipe namespaced identity.
    pub endpoint: String,
}

impl RuntimePaths {
    /// Derives paths from an explicit protected state directory.
    #[must_use]
    pub fn from_state_dir(state_dir: impl Into<PathBuf>) -> Self {
        let state_dir = state_dir.into();
        let endpoint = platform_endpoint(&state_dir);
        Self {
            endpoint_manifest: state_dir.join("endpoint.v1.json"),
            lock_file: state_dir.join("daemon.lock"),
            database: state_dir.join("runtime.sqlite3"),
            log_file: state_dir.join("daemon.log"),
            state_dir,
            endpoint,
        }
    }

    /// Chooses the OS-specific per-user runtime directory.
    pub fn per_user_default() -> IntegrationResult<Self> {
        #[cfg(windows)]
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or(IntegrationError::EndpointProtection)?;
        #[cfg(unix)]
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .ok_or(IntegrationError::EndpointProtection)?;
        Ok(Self::from_state_dir(base.join("ae-sdd").join("runtime")))
    }

    /// Creates and restricts the parent directory before any secret is written.
    pub fn prepare(&self) -> IntegrationResult<()> {
        fs::create_dir_all(&self.state_dir)?;
        protect_runtime_dir(&self.state_dir)
    }

    /// Removes a crash-left Unix socket only after the caller owns the daemon lock.
    pub fn remove_stale_local_endpoint(&self) -> IntegrationResult<()> {
        remove_stale_endpoint(self)
    }
}

/// Held cross-process singleton lock; dropping it releases ownership.
#[derive(Debug)]
pub struct DaemonLock {
    _file: File,
}

impl DaemonLock {
    /// Acquires the singleton lock without waiting.
    pub fn acquire(paths: &RuntimePaths) -> IntegrationResult<Self> {
        paths.prepare()?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&paths.lock_file)?;
        file.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => IntegrationError::AlreadyRunning,
            std::fs::TryLockError::Error(error) => IntegrationError::Io(error),
        })?;
        Ok(Self { _file: file })
    }
}

/// Publishes a protected endpoint manifest via same-directory atomic replace.
pub fn publish_endpoint_manifest(
    paths: &RuntimePaths,
    manifest: &EndpointManifest,
) -> IntegrationResult<()> {
    paths.prepare()?;
    let file = AtomicFile::new(&paths.endpoint_manifest, OverwriteBehavior::AllowOverwrite);
    file.write(|handle| -> Result<(), std::io::Error> {
        serde_json::to_writer(handle, manifest).map_err(std::io::Error::other)
    })
    .map_err(|_| IntegrationError::EndpointProtection)?;
    protect_manifest(&paths.endpoint_manifest)
}

/// Reads one complete manifest snapshot.
pub fn read_endpoint_manifest(path: impl AsRef<Path>) -> IntegrationResult<EndpointManifest> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(unix)]
fn protect_runtime_dir(path: &Path) -> IntegrationResult<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(unix)]
fn protect_manifest(path: &Path) -> IntegrationResult<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(windows)]
fn protect_runtime_dir(path: &Path) -> IntegrationResult<()> {
    let user = std::env::var("USERNAME").map_err(|_| IntegrationError::EndpointProtection)?;
    let grant = format!("{user}:(OI)(CI)F");
    let status = std::process::Command::new("icacls.exe")
        .arg(path)
        .args(["/inheritance:r", "/grant:r", &grant])
        .status()?;
    status
        .success()
        .then_some(())
        .ok_or(IntegrationError::EndpointProtection)
}

#[cfg(windows)]
fn protect_manifest(path: &Path) -> IntegrationResult<()> {
    let user = std::env::var("USERNAME").map_err(|_| IntegrationError::EndpointProtection)?;
    let grant = format!("{user}:F");
    let status = std::process::Command::new("icacls.exe")
        .arg(path)
        .args(["/inheritance:r", "/grant:r", &grant])
        .status()?;
    status
        .success()
        .then_some(())
        .ok_or(IntegrationError::EndpointProtection)
}

#[cfg(windows)]
fn platform_endpoint(state_dir: &Path) -> String {
    let digest = hex::encode(Sha256::digest(state_dir.to_string_lossy().as_bytes()));
    format!("ae-sdd-{}", &digest[..24])
}

#[cfg(windows)]
fn remove_stale_endpoint(_paths: &RuntimePaths) -> IntegrationResult<()> {
    Ok(())
}

#[cfg(unix)]
fn remove_stale_endpoint(paths: &RuntimePaths) -> IntegrationResult<()> {
    use std::os::unix::fs::FileTypeExt;

    let endpoint = PathBuf::from(&paths.endpoint);
    if endpoint.parent() != Some(paths.state_dir.as_path()) {
        return Err(IntegrationError::EndpointProtection);
    }
    match fs::symlink_metadata(&endpoint) {
        Ok(metadata) if metadata.file_type().is_socket() => {
            fs::remove_file(endpoint)?;
            Ok(())
        }
        Ok(_) => Err(IntegrationError::EndpointProtection),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(unix)]
fn platform_endpoint(state_dir: &Path) -> String {
    state_dir.join("ae-sdd.sock").to_string_lossy().into_owned()
}
