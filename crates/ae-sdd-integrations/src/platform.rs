use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ae_sdd_protocol::StableErrorCode;
use ae_sdd_runtime::{
    ClockPort, ResolvedWorkspace, RuntimeError, RuntimeResult, WorkspaceResolverPort,
};

/// Production wall clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl ClockPort for SystemClock {
    fn now_unix_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| {
                duration.as_millis().try_into().unwrap_or(u64::MAX)
            })
    }
}

/// Canonical filesystem resolver constrained to explicit allowed roots.
#[derive(Clone, Debug)]
pub struct FileWorkspaceResolver {
    allowed_roots: Vec<PathBuf>,
}

impl FileWorkspaceResolver {
    /// Canonicalizes the configured allowed roots immediately.
    pub fn new(allowed_roots: impl IntoIterator<Item = PathBuf>) -> RuntimeResult<Self> {
        let allowed_roots = allowed_roots
            .into_iter()
            .map(|path| canonical(&path))
            .collect::<RuntimeResult<Vec<_>>>()?;
        if allowed_roots.is_empty() {
            return Err(RuntimeError::new(
                StableErrorCode::WorkspaceOutsideAllowedRoot,
                "at least one allowed workspace root is required",
            ));
        }
        Ok(Self { allowed_roots })
    }
}

impl WorkspaceResolverPort for FileWorkspaceResolver {
    fn resolve(&self, requested_root: &str) -> RuntimeResult<ResolvedWorkspace> {
        let canonical = canonical(Path::new(requested_root))?;
        let inside_allowed_root = self
            .allowed_roots
            .iter()
            .any(|allowed| canonical.starts_with(allowed));
        Ok(ResolvedWorkspace {
            canonical_root: canonical.to_string_lossy().into_owned(),
            inside_allowed_root,
        })
    }
}

fn canonical(path: &Path) -> RuntimeResult<PathBuf> {
    path.canonicalize().map_err(|_| {
        RuntimeError::new(
            StableErrorCode::WorkspaceOutsideAllowedRoot,
            "workspace path cannot be canonicalized",
        )
    })
}
