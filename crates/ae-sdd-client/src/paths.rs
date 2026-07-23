use std::path::PathBuf;

use crate::{ClientError, ClientResult};

/// Returns the OS-specific per-user daemon state directory.
pub fn default_state_dir() -> ClientResult<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or(ClientError::EndpointManifest)?;
    #[cfg(unix)]
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or(ClientError::EndpointManifest)?;
    Ok(base.join("ae-sdd").join("runtime"))
}

/// Returns the protected endpoint manifest used by default clients.
pub fn default_endpoint_manifest() -> ClientResult<PathBuf> {
    Ok(default_state_dir()?.join("endpoint.v1.json"))
}
