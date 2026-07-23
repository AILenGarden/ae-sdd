use std::{io, path::PathBuf};

use ae_sdd_domain::{
    ArtifactDigest, FencingToken, InputFingerprint, LeaseId, ProjectRelativePathError,
    StateRevision, StringIdError, UuidIdError,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("STATE_INVALID: authoritative state is not valid bounded JSON: {reason}")]
    InvalidState { reason: Box<str> },
    #[error("REVISION_CONFLICT: expected {expected:?}, observed {observed:?}")]
    RevisionConflict {
        expected: StateRevision,
        observed: StateRevision,
    },
    #[error("EXTERNAL_STATE_CONFLICT: revision {revision:?} retained a different digest")]
    ExternalStateConflict {
        revision: StateRevision,
        expected_digest: ArtifactDigest,
        observed_digest: ArtifactDigest,
    },
    #[error("LEASE_CONFLICT: another non-expired lease owns the Work Item")]
    LeaseConflict,
    #[error("LEASE_REQUIRED: no matching active lease was supplied")]
    LeaseRequired,
    #[error("LEASE_EXPIRED: lease is no longer active")]
    LeaseExpired,
    #[error("STALE_FENCING_TOKEN: expected at least {minimum:?}, observed {observed:?}")]
    StaleFencingToken {
        minimum: FencingToken,
        observed: FencingToken,
    },
    #[error("LEASE_MISMATCH: lease {lease_id} does not match owner or fencing generation")]
    LeaseMismatch { lease_id: LeaseId },
    #[error("IDEMPOTENCY_KEY_REUSED: key is already bound to another payload")]
    IdempotencyKeyReused {
        expected: InputFingerprint,
        observed: InputFingerprint,
    },
    #[error("IDEMPOTENCY_KEY_INVALID: {reason}")]
    InvalidIdempotencyKey { reason: &'static str },
    #[error("domain path is invalid: {0}")]
    InvalidPath(#[from] ProjectRelativePathError),
    #[error("domain identifier is invalid: {0}")]
    InvalidStringId(#[from] StringIdError),
    #[error("domain UUID identifier is invalid: {0}")]
    InvalidUuidId(#[from] UuidIdError),
    #[error("JOURNAL_INVALID: {reason}")]
    InvalidJournal { reason: Box<str> },
    #[error("JOURNAL_CONFLICT: recovery cannot prove target state for {path}")]
    JournalConflict { path: PathBuf },
    #[error("PAYLOAD_TOO_LARGE: {actual} bytes exceeds {maximum}")]
    PayloadTooLarge { maximum: usize, actual: usize },
    #[error("PERSISTENCE_CONFLICT: {entity} already exists with different content")]
    PersistenceConflict { entity: &'static str },
    #[error("DATABASE_INCOMPATIBLE: {reason}")]
    DatabaseIncompatible { reason: Box<str> },
    #[error("database operation failed: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("filesystem operation failed at {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("commit interrupted at fault point {point}")]
    InjectedFault { point: &'static str },
}

impl StoreError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
