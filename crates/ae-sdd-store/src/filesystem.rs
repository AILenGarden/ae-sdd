use std::{
    collections::{BTreeMap, BTreeSet},
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use atomicwrites::{AllowOverwrite, AtomicFile};

use crate::StoreError;

pub trait DurableFileSystem: Send + Sync {
    fn read(&self, path: &Path) -> Result<Option<Vec<u8>>, StoreError>;
    fn write_atomic_durable(&self, path: &Path, bytes: &[u8]) -> Result<(), StoreError>;
    fn create_dir_all(&self, path: &Path) -> Result<(), StoreError>;
    fn sync_directory(&self, path: &Path) -> Result<(), StoreError>;
    fn list_files(&self, path: &Path) -> Result<Vec<PathBuf>, StoreError>;
}

pub trait ExclusiveLockGuard: Send {}

impl<T: Send> ExclusiveLockGuard for T {}

pub trait CrossProcessLockPort: Send + Sync {
    fn lock_exclusive(&self, path: &Path) -> Result<Box<dyn ExclusiveLockGuard>, StoreError>;
    /// Attempts the exclusive lock without blocking; returns `Ok(None)` when
    /// the lock is already held by another process.
    fn try_lock_exclusive(
        &self,
        path: &Path,
    ) -> Result<Option<Box<dyn ExclusiveLockGuard>>, StoreError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StdDurableFileSystem;

impl DurableFileSystem for StdDurableFileSystem {
    fn read(&self, path: &Path) -> Result<Option<Vec<u8>>, StoreError> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(StoreError::io(path, error)),
        }
    }

    fn write_atomic_durable(&self, path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
        let parent = path.parent().ok_or_else(|| StoreError::InvalidJournal {
            reason: "atomic target must have a parent directory".into(),
        })?;
        std::fs::create_dir_all(parent).map_err(|error| StoreError::io(parent, error))?;
        AtomicFile::new(path, AllowOverwrite)
            .write(|file| {
                file.write_all(bytes)?;
                file.sync_all()
            })
            .map_err(|error| StoreError::io(path, std::io::Error::from(error)))?;
        self.sync_directory(parent)
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), StoreError> {
        std::fs::create_dir_all(path).map_err(|error| StoreError::io(path, error))?;
        if let Some(parent) = path.parent() {
            self.sync_directory(parent)?;
        }
        Ok(())
    }

    fn sync_directory(&self, path: &Path) -> Result<(), StoreError> {
        sync_directory(path)
    }

    fn list_files(&self, path: &Path) -> Result<Vec<PathBuf>, StoreError> {
        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(StoreError::io(path, error)),
        };
        let mut files = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| StoreError::io(path, error))?;
            let file_type = entry
                .file_type()
                .map_err(|error| StoreError::io(entry.path(), error))?;
            if file_type.is_file() {
                files.push(entry.path());
            }
        }
        files.sort();
        Ok(files)
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), StoreError> {
    std::fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| StoreError::io(path, error))
}

#[cfg(windows)]
fn sync_directory(path: &Path) -> Result<(), StoreError> {
    // Atomicwrites uses MoveFileExW with WRITE_THROUGH and REPLACE_EXISTING.
    // A directory handle cannot be opened through safe std APIs on Windows;
    // forcing a metadata read still makes a missing/replaced directory fail closed.
    std::fs::metadata(path)
        .map(|_| ())
        .map_err(|error| StoreError::io(path, error))
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StdCrossProcessLock;

impl CrossProcessLockPort for StdCrossProcessLock {
    fn lock_exclusive(&self, path: &Path) -> Result<Box<dyn ExclusiveLockGuard>, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| StoreError::io(parent, error))?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)
            .map_err(|error| StoreError::io(path, error))?;
        fs4::FileExt::lock(&file).map_err(|error| StoreError::io(path, error))?;
        Ok(Box::new(file))
    }

    fn try_lock_exclusive(
        &self,
        path: &Path,
    ) -> Result<Option<Box<dyn ExclusiveLockGuard>>, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| StoreError::io(parent, error))?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)
            .map_err(|error| StoreError::io(path, error))?;
        match fs4::FileExt::try_lock(&file) {
            Ok(()) => Ok(Some(Box::new(file))),
            Err(fs4::TryLockError::WouldBlock) => Ok(None),
            Err(fs4::TryLockError::Error(error)) => Err(StoreError::io(path, error)),
        }
    }
}

#[derive(Debug, Default)]
pub struct InMemoryFileSystem {
    files: RwLock<BTreeMap<PathBuf, Vec<u8>>>,
    directories: RwLock<BTreeSet<PathBuf>>,
    lock_held: Arc<AtomicBool>,
}

impl InMemoryFileSystem {
    pub fn insert(&self, path: impl Into<PathBuf>, bytes: Vec<u8>) {
        self.files
            .write()
            .expect("in-memory filesystem lock is not poisoned")
            .insert(path.into(), bytes);
    }

    pub fn snapshot(&self, path: &Path) -> Option<Vec<u8>> {
        self.files
            .read()
            .expect("in-memory filesystem lock is not poisoned")
            .get(path)
            .cloned()
    }
}

impl DurableFileSystem for InMemoryFileSystem {
    fn read(&self, path: &Path) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self
            .files
            .read()
            .expect("in-memory filesystem lock is not poisoned")
            .get(path)
            .cloned())
    }

    fn write_atomic_durable(&self, path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
        self.files
            .write()
            .expect("in-memory filesystem lock is not poisoned")
            .insert(path.to_path_buf(), bytes.to_vec());
        Ok(())
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), StoreError> {
        self.directories
            .write()
            .expect("in-memory filesystem lock is not poisoned")
            .insert(path.to_path_buf());
        Ok(())
    }

    fn sync_directory(&self, _path: &Path) -> Result<(), StoreError> {
        Ok(())
    }

    fn list_files(&self, path: &Path) -> Result<Vec<PathBuf>, StoreError> {
        let mut files: Vec<_> = self
            .files
            .read()
            .expect("in-memory filesystem lock is not poisoned")
            .keys()
            .filter(|candidate| candidate.parent() == Some(path))
            .cloned()
            .collect();
        files.sort();
        Ok(files)
    }
}

struct InMemoryGuard(Arc<AtomicBool>);

impl Drop for InMemoryGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl CrossProcessLockPort for InMemoryFileSystem {
    fn lock_exclusive(&self, _path: &Path) -> Result<Box<dyn ExclusiveLockGuard>, StoreError> {
        if self
            .lock_held
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return Err(StoreError::LeaseConflict);
        }
        Ok(Box::new(InMemoryGuard(Arc::clone(&self.lock_held))))
    }

    fn try_lock_exclusive(
        &self,
        _path: &Path,
    ) -> Result<Option<Box<dyn ExclusiveLockGuard>>, StoreError> {
        if self
            .lock_held
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return Ok(None);
        }
        Ok(Some(Box::new(InMemoryGuard(Arc::clone(&self.lock_held)))))
    }
}
