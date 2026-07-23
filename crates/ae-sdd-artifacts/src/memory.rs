use std::{collections::BTreeMap, sync::RwLock};

use ae_sdd_domain::ProjectRelativePath;

use crate::{ArtifactReadPort, ArtifactStoreError};

#[derive(Debug, Default)]
pub struct InMemoryArtifactStore {
    files: RwLock<BTreeMap<ProjectRelativePath, Vec<u8>>>,
}

impl InMemoryArtifactStore {
    pub fn insert(&self, path: ProjectRelativePath, bytes: Vec<u8>) {
        self.files
            .write()
            .expect("artifact test adapter lock is not poisoned")
            .insert(path, bytes);
    }
}

impl ArtifactReadPort for InMemoryArtifactStore {
    fn read(
        &self,
        path: &ProjectRelativePath,
        max_bytes: u64,
    ) -> Result<Vec<u8>, ArtifactStoreError> {
        let guard = self
            .files
            .read()
            .expect("artifact test adapter lock is not poisoned");
        let bytes = guard.get(path).ok_or(ArtifactStoreError::NotFound)?;
        let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if observed > max_bytes {
            return Err(ArtifactStoreError::LengthMismatch {
                expected: max_bytes,
                observed,
            });
        }
        Ok(bytes.clone())
    }
}
