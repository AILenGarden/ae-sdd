use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use ae_sdd_domain::{ArtifactDigest, ArtifactRef, ProjectRelativePath};

use crate::ArtifactStoreError;

pub const DEFAULT_MAX_ARTIFACT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceRoot(PathBuf);

impl WorkspaceRoot {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ArtifactStoreError> {
        let requested = path.as_ref();
        let canonical =
            requested
                .canonicalize()
                .map_err(|_| ArtifactStoreError::InvalidWorkspaceRoot {
                    path: requested.to_path_buf(),
                })?;
        if !canonical.is_dir() {
            return Err(ArtifactStoreError::InvalidWorkspaceRoot {
                path: requested.to_path_buf(),
            });
        }
        Ok(Self(canonical))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    fn resolve_existing(
        &self,
        relative: &ProjectRelativePath,
    ) -> Result<PathBuf, ArtifactStoreError> {
        let candidate = self.0.join(relative.as_str());
        let canonical = candidate.canonicalize().map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ArtifactStoreError::NotFound
            } else {
                ArtifactStoreError::Io(error)
            }
        })?;
        if !canonical.starts_with(&self.0) || canonical == self.0 {
            return Err(ArtifactStoreError::OutsideWorkspace);
        }
        Ok(canonical)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactValidation {
    reference: ArtifactRef,
    bytes: Vec<u8>,
}

impl ArtifactValidation {
    pub const fn reference(&self) -> &ArtifactRef {
        &self.reference
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

pub trait ArtifactReadPort: Send + Sync {
    fn read(
        &self,
        path: &ProjectRelativePath,
        max_bytes: u64,
    ) -> Result<Vec<u8>, ArtifactStoreError>;
}

#[derive(Clone, Debug)]
pub struct FsArtifactStore {
    root: WorkspaceRoot,
}

impl FsArtifactStore {
    pub const fn new(root: WorkspaceRoot) -> Self {
        Self { root }
    }

    pub const fn root(&self) -> &WorkspaceRoot {
        &self.root
    }
}

impl ArtifactReadPort for FsArtifactStore {
    fn read(
        &self,
        path: &ProjectRelativePath,
        max_bytes: u64,
    ) -> Result<Vec<u8>, ArtifactStoreError> {
        let canonical_before = self.root.resolve_existing(path)?;
        let file = File::open(&canonical_before)?;
        let metadata_before = file.metadata()?;
        if !metadata_before.is_file() {
            return Err(ArtifactStoreError::NotFound);
        }
        if metadata_before.len() > max_bytes {
            return Err(ArtifactStoreError::LengthMismatch {
                expected: max_bytes,
                observed: metadata_before.len(),
            });
        }

        let capacity = usize::try_from(metadata_before.len())
            .unwrap_or(usize::MAX)
            .min(1024 * 1024);
        let mut bytes = Vec::with_capacity(capacity);
        file.take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
            return Err(ArtifactStoreError::LengthMismatch {
                expected: max_bytes,
                observed: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            });
        }

        let canonical_after = self.root.resolve_existing(path)?;
        let metadata_after = std::fs::metadata(&canonical_after)?;
        if canonical_before != canonical_after
            || metadata_before.len() != metadata_after.len()
            || metadata_before.modified().ok() != metadata_after.modified().ok()
        {
            return Err(ArtifactStoreError::ChangedDuringRead);
        }
        Ok(bytes)
    }
}

#[derive(Clone, Debug)]
pub struct ArtifactValidator<R> {
    reader: R,
    max_bytes: u64,
}

impl<R> ArtifactValidator<R>
where
    R: ArtifactReadPort,
{
    pub const fn new(reader: R, max_bytes: u64) -> Self {
        Self { reader, max_bytes }
    }

    pub fn validate(
        &self,
        reference: &ArtifactRef,
        allowed_scopes: &[ProjectRelativePath],
    ) -> Result<ArtifactValidation, ArtifactStoreError> {
        if !allowed_scopes
            .iter()
            .any(|scope| scope.contains(reference.path()))
        {
            return Err(ArtifactStoreError::OutsideGrant);
        }
        if reference.byte_length() > self.max_bytes {
            return Err(ArtifactStoreError::LengthMismatch {
                expected: self.max_bytes,
                observed: reference.byte_length(),
            });
        }

        let bytes = self.reader.read(reference.path(), self.max_bytes)?;
        let observed_length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if observed_length != reference.byte_length() {
            return Err(ArtifactStoreError::LengthMismatch {
                expected: reference.byte_length(),
                observed: observed_length,
            });
        }
        let observed_digest = ArtifactDigest::digest(&bytes);
        if observed_digest != reference.digest() {
            return Err(ArtifactStoreError::DigestMismatch {
                expected: reference.digest(),
                observed: observed_digest,
            });
        }
        Ok(ArtifactValidation {
            reference: reference.clone(),
            bytes,
        })
    }
}
