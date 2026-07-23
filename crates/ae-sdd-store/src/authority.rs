use ae_sdd_domain::{ArtifactDigest, FencingToken, StateRevision};

use crate::StoreError;

pub const MAX_AUTHORITATIVE_STATE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthoritySnapshot {
    revision: StateRevision,
    last_fencing_token: FencingToken,
    digest: ArtifactDigest,
}

impl AuthoritySnapshot {
    pub const fn revision(self) -> StateRevision {
        self.revision
    }

    pub const fn last_fencing_token(self) -> FencingToken {
        self.last_fencing_token
    }

    pub const fn digest(self) -> ArtifactDigest {
        self.digest
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StateAuthority;

impl StateAuthority {
    pub fn inspect(bytes: &[u8]) -> Result<AuthoritySnapshot, StoreError> {
        if bytes.len() > MAX_AUTHORITATIVE_STATE_BYTES {
            return Err(StoreError::PayloadTooLarge {
                maximum: MAX_AUTHORITATIVE_STATE_BYTES,
                actual: bytes.len(),
            });
        }
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|error| StoreError::InvalidState {
                reason: error.to_string().into_boxed_str(),
            })?;
        let object = value.as_object().ok_or_else(|| StoreError::InvalidState {
            reason: "state root must be a JSON object".into(),
        })?;
        let revision = object
            .get("revision")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| StoreError::InvalidState {
                reason: "state.revision must be a non-negative integer".into(),
            })?;
        let last_fencing_token = object
            .get("lastFencingToken")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| StoreError::InvalidState {
                reason: "state.lastFencingToken must be a non-negative integer".into(),
            })?;
        Ok(AuthoritySnapshot {
            revision: StateRevision::new(revision),
            last_fencing_token: FencingToken::new(last_fencing_token),
            digest: ArtifactDigest::digest(bytes),
        })
    }

    pub fn verify_unchanged(
        expected: AuthoritySnapshot,
        observed: AuthoritySnapshot,
    ) -> Result<(), StoreError> {
        if observed.revision == expected.revision && observed.digest != expected.digest {
            return Err(StoreError::ExternalStateConflict {
                revision: expected.revision,
                expected_digest: expected.digest,
                observed_digest: observed.digest,
            });
        }
        if observed.revision != expected.revision {
            return Err(StoreError::RevisionConflict {
                expected: expected.revision,
                observed: observed.revision,
            });
        }
        if observed.last_fencing_token < expected.last_fencing_token {
            return Err(StoreError::StaleFencingToken {
                minimum: expected.last_fencing_token,
                observed: observed.last_fencing_token,
            });
        }
        Ok(())
    }

    pub fn verify_successor(
        before: AuthoritySnapshot,
        after: AuthoritySnapshot,
        fencing_token: FencingToken,
    ) -> Result<(), StoreError> {
        let expected_revision =
            before
                .revision
                .checked_next()
                .map_err(|error| StoreError::InvalidState {
                    reason: error.to_string().into_boxed_str(),
                })?;
        if after.revision != expected_revision {
            return Err(StoreError::RevisionConflict {
                expected: expected_revision,
                observed: after.revision,
            });
        }
        if after.last_fencing_token != fencing_token || fencing_token < before.last_fencing_token {
            return Err(StoreError::StaleFencingToken {
                minimum: before.last_fencing_token,
                observed: after.last_fencing_token,
            });
        }
        Ok(())
    }
}
