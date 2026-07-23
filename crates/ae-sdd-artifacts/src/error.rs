use std::{io, path::PathBuf};

use ae_sdd_domain::{ArtifactDigest, ProjectRelativePathError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArtifactStoreError {
    #[error("workspace root is not an existing directory: {path}")]
    InvalidWorkspaceRoot { path: PathBuf },
    #[error("artifact path escapes the canonical workspace root")]
    OutsideWorkspace,
    #[error("artifact path is outside its delegated scope")]
    OutsideGrant,
    #[error("artifact does not exist")]
    NotFound,
    #[error("artifact changed while it was being verified")]
    ChangedDuringRead,
    #[error("artifact length differs: expected {expected}, observed {observed}")]
    LengthMismatch { expected: u64, observed: u64 },
    #[error("artifact digest differs: expected {expected}, observed {observed}")]
    DigestMismatch {
        expected: ArtifactDigest,
        observed: ArtifactDigest,
    },
    #[error("artifact path is invalid: {0}")]
    InvalidPath(#[from] ProjectRelativePathError),
    #[error("artifact filesystem operation failed: {0}")]
    Io(#[from] io::Error),
}

impl PartialEq for ArtifactStoreError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::InvalidWorkspaceRoot { path: left },
                Self::InvalidWorkspaceRoot { path: right },
            ) => left == right,
            (Self::OutsideWorkspace, Self::OutsideWorkspace)
            | (Self::OutsideGrant, Self::OutsideGrant)
            | (Self::NotFound, Self::NotFound)
            | (Self::ChangedDuringRead, Self::ChangedDuringRead) => true,
            (
                Self::LengthMismatch {
                    expected: left_expected,
                    observed: left_observed,
                },
                Self::LengthMismatch {
                    expected: right_expected,
                    observed: right_observed,
                },
            ) => left_expected == right_expected && left_observed == right_observed,
            (
                Self::DigestMismatch {
                    expected: left_expected,
                    observed: left_observed,
                },
                Self::DigestMismatch {
                    expected: right_expected,
                    observed: right_observed,
                },
            ) => left_expected == right_expected && left_observed == right_observed,
            (Self::InvalidPath(left), Self::InvalidPath(right)) => left == right,
            (Self::Io(left), Self::Io(right)) => {
                left.kind() == right.kind() && left.to_string() == right.to_string()
            }
            _ => false,
        }
    }
}

impl Eq for ArtifactStoreError {}
