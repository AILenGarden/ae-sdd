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

#[cfg(test)]
mod tests {
    use super::*;

    /// These paths decide where every client looks for the daemon, so the
    /// layout is a compatibility surface: a silent change strands clients.
    /// Asserted without mutating the environment (unsafe in edition 2024) —
    /// CI always provides the base variable this reads on each platform.
    #[test]
    fn state_dir_is_anchored_under_the_per_user_base_directory() {
        let state_dir = default_state_dir().expect("a base directory is set in any test env");

        assert!(
            state_dir.ends_with("ae-sdd/runtime") || state_dir.ends_with("ae-sdd\\runtime"),
            "unexpected state dir layout: {state_dir:?}"
        );
        assert!(
            state_dir.is_absolute(),
            "state dir must be absolute so clients do not resolve it per-cwd: {state_dir:?}"
        );
    }

    #[test]
    fn endpoint_manifest_is_the_v1_file_inside_the_state_dir() {
        let state_dir = default_state_dir().expect("a base directory is set in any test env");
        let manifest = default_endpoint_manifest().expect("manifest path derives from state dir");

        assert_eq!(
            manifest.file_name().and_then(|n| n.to_str()),
            Some("endpoint.v1.json")
        );
        assert_eq!(manifest.parent(), Some(state_dir.as_path()));
    }
}
