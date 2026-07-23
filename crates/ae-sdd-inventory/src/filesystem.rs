use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use ae_sdd_domain::{ArtifactDigest, ProjectRelativePath};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::FileRecord;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilesystemLimits {
    pub max_files: usize,
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
    pub max_depth: usize,
}

impl Default for FilesystemLimits {
    fn default() -> Self {
        Self {
            max_files: 100_000,
            max_file_bytes: 64 * 1024 * 1024,
            max_total_bytes: 2 * 1024 * 1024 * 1024,
            max_depth: 128,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilesystemScan {
    pub records: Vec<FileRecord>,
    pub skipped_symlinks: Vec<ProjectRelativePath>,
}

/// Bounded, non-symlink-following filesystem inventory adapter.
pub struct FilesystemInventory;

impl FilesystemInventory {
    pub fn scan(root: &Path, limits: FilesystemLimits) -> Result<FilesystemScan, FilesystemError> {
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err(FilesystemError::RootNotDirectory(root));
        }
        let mut state = ScanState {
            root: &root,
            limits,
            total_bytes: 0,
            records: Vec::new(),
            skipped_symlinks: Vec::new(),
        };
        state.visit(&root, 0)?;
        state
            .records
            .sort_by(|left, right| left.path().cmp(right.path()));
        state.skipped_symlinks.sort();
        Ok(FilesystemScan {
            records: state.records,
            skipped_symlinks: state.skipped_symlinks,
        })
    }
}

struct ScanState<'a> {
    root: &'a Path,
    limits: FilesystemLimits,
    total_bytes: u64,
    records: Vec<FileRecord>,
    skipped_symlinks: Vec<ProjectRelativePath>,
}

impl ScanState<'_> {
    fn visit(&mut self, directory: &Path, depth: usize) -> Result<(), FilesystemError> {
        if depth > self.limits.max_depth {
            return Err(FilesystemError::DepthLimit {
                maximum: self.limits.max_depth,
            });
        }
        let mut entries: Vec<_> = fs::read_dir(directory)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            let relative = relative_path(self.root, &path)?;
            if metadata.file_type().is_symlink() {
                self.skipped_symlinks.push(relative);
                continue;
            }
            if metadata.is_dir() {
                self.visit(&path, depth + 1)?;
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            if self.records.len() >= self.limits.max_files {
                return Err(FilesystemError::FileCountLimit {
                    maximum: self.limits.max_files,
                });
            }
            if metadata.len() > self.limits.max_file_bytes {
                return Err(FilesystemError::FileByteLimit {
                    path: relative,
                    actual: metadata.len(),
                    maximum: self.limits.max_file_bytes,
                });
            }
            self.total_bytes = self
                .total_bytes
                .checked_add(metadata.len())
                .ok_or(FilesystemError::TotalByteOverflow)?;
            if self.total_bytes > self.limits.max_total_bytes {
                return Err(FilesystemError::TotalByteLimit {
                    actual: self.total_bytes,
                    maximum: self.limits.max_total_bytes,
                });
            }
            self.records.push(FileRecord::new(
                relative,
                digest_file(&path)?,
                metadata.len(),
            ));
        }
        Ok(())
    }
}

fn relative_path(root: &Path, path: &Path) -> Result<ProjectRelativePath, FilesystemError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| FilesystemError::PathOutsideRoot(path.to_path_buf()))?;
    let mut segments = Vec::new();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(FilesystemError::NonPortablePath(path.to_path_buf()));
        };
        segments.push(
            segment
                .to_str()
                .ok_or_else(|| FilesystemError::NonPortablePath(path.to_path_buf()))?,
        );
    }
    ProjectRelativePath::new(segments.join("/"))
        .map_err(|_| FilesystemError::NonPortablePath(path.to_path_buf()))
}

fn digest_file(path: &Path) -> Result<ArtifactDigest, io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(ArtifactDigest::from_array(hasher.finalize().into()))
}

#[derive(Debug, Error)]
pub enum FilesystemError {
    #[error("filesystem inventory I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("inventory root is not a directory: {0}")]
    RootNotDirectory(PathBuf),
    #[error("path escaped inventory root: {0}")]
    PathOutsideRoot(PathBuf),
    #[error("path is not portable UTF-8 project-relative syntax: {0}")]
    NonPortablePath(PathBuf),
    #[error("inventory exceeds {maximum} files")]
    FileCountLimit { maximum: usize },
    #[error("inventory directory nesting exceeds {maximum}")]
    DepthLimit { maximum: usize },
    #[error("file {path} has {actual} bytes, exceeding {maximum}")]
    FileByteLimit {
        path: ProjectRelativePath,
        actual: u64,
        maximum: u64,
    },
    #[error("inventory byte counter overflowed")]
    TotalByteOverflow,
    #[error("inventory has {actual} bytes, exceeding {maximum}")]
    TotalByteLimit { actual: u64, maximum: u64 },
}
