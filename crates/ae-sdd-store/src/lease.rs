use ae_sdd_domain::{FencingToken, LeaseId};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::{StoreError, UtcTimestamp};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LeaseOwner(Box<str>);

impl LeaseOwner {
    pub const MAX_BYTES: usize = 256;

    pub fn new(value: impl Into<Box<str>>) -> Result<Self, StoreError> {
        let value = value.into();
        if value.is_empty() || value.len() > Self::MAX_BYTES || value.chars().any(char::is_control)
        {
            return Err(StoreError::InvalidState {
                reason: "lease owner must be bounded printable text".into(),
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaseRecord {
    lease_id: LeaseId,
    owner: LeaseOwner,
    fencing_token: FencingToken,
    acquired_at: UtcTimestamp,
    expires_at: UtcTimestamp,
}

impl LeaseRecord {
    pub const fn lease_id(&self) -> LeaseId {
        self.lease_id
    }

    pub const fn owner(&self) -> &LeaseOwner {
        &self.owner
    }

    pub const fn fencing_token(&self) -> FencingToken {
        self.fencing_token
    }

    pub const fn acquired_at(&self) -> &UtcTimestamp {
        &self.acquired_at
    }

    pub const fn expires_at(&self) -> &UtcTimestamp {
        &self.expires_at
    }

    pub fn is_active_at(&self, now: &UtcTimestamp) -> bool {
        now < &self.expires_at
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaseProof {
    pub lease_id: LeaseId,
    pub owner: LeaseOwner,
    pub fencing_token: FencingToken,
}

impl From<&LeaseRecord> for LeaseProof {
    fn from(record: &LeaseRecord) -> Self {
        Self {
            lease_id: record.lease_id,
            owner: record.owner.clone(),
            fencing_token: record.fencing_token,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaseTombstone {
    pub lease_id: LeaseId,
    pub owner: LeaseOwner,
    pub fencing_token: FencingToken,
    pub ended_at: UtcTimestamp,
    pub reason: Box<str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaseLedger {
    last_fencing_token: FencingToken,
    active: Option<LeaseRecord>,
    tombstones: Vec<LeaseTombstone>,
}

impl LeaseLedger {
    pub const fn empty(last_fencing_token: FencingToken) -> Self {
        Self {
            last_fencing_token,
            active: None,
            tombstones: Vec::new(),
        }
    }

    pub const fn last_fencing_token(&self) -> FencingToken {
        self.last_fencing_token
    }

    pub const fn active(&self) -> Option<&LeaseRecord> {
        self.active.as_ref()
    }

    pub fn tombstones(&self) -> &[LeaseTombstone] {
        &self.tombstones
    }

    pub fn acquire(
        &mut self,
        lease_id: LeaseId,
        owner: LeaseOwner,
        now: UtcTimestamp,
        expires_at: UtcTimestamp,
    ) -> Result<LeaseRecord, StoreError> {
        if expires_at <= now {
            return Err(StoreError::InvalidState {
                reason: "lease expiry must be later than acquisition time".into(),
            });
        }
        self.expire_if_needed(&now);
        if self.active.is_some() {
            return Err(StoreError::LeaseConflict);
        }
        let fencing_token =
            self.last_fencing_token
                .checked_next()
                .map_err(|error| StoreError::InvalidState {
                    reason: error.to_string().into_boxed_str(),
                })?;
        let record = LeaseRecord {
            lease_id,
            owner,
            fencing_token,
            acquired_at: now,
            expires_at,
        };
        self.last_fencing_token = fencing_token;
        self.active = Some(record.clone());
        Ok(record)
    }

    pub fn renew(
        &mut self,
        proof: &LeaseProof,
        now: &UtcTimestamp,
        expires_at: UtcTimestamp,
    ) -> Result<LeaseRecord, StoreError> {
        self.expire_if_needed(now);
        let minimum = self.last_fencing_token;
        let active = self.active.as_mut().ok_or(StoreError::LeaseExpired)?;
        validate_fencing(minimum, proof)?;
        validate_identity(active, proof)?;
        if expires_at <= *now || expires_at <= active.expires_at {
            return Err(StoreError::InvalidState {
                reason: "renewed lease expiry must increase and remain in the future".into(),
            });
        }
        active.expires_at = expires_at;
        Ok(active.clone())
    }

    pub fn release(
        &mut self,
        proof: &LeaseProof,
        now: UtcTimestamp,
    ) -> Result<LeaseTombstone, StoreError> {
        self.end(proof, now, "released")
    }

    pub fn break_active(
        &mut self,
        actor: LeaseOwner,
        reason: impl Into<Box<str>>,
        now: UtcTimestamp,
    ) -> Result<Option<LeaseTombstone>, StoreError> {
        let reason = reason.into();
        if reason.is_empty() || reason.len() > 1024 {
            return Err(StoreError::InvalidState {
                reason: "lease break reason must be present and bounded".into(),
            });
        }
        self.expire_if_needed(&now);
        let Some(active) = self.active.take() else {
            return Ok(None);
        };
        let tombstone = LeaseTombstone {
            lease_id: active.lease_id,
            owner: active.owner,
            fencing_token: active.fencing_token,
            ended_at: now,
            reason: format!("broken by {}: {reason}", actor.as_str()).into_boxed_str(),
        };
        self.tombstones.push(tombstone.clone());
        Ok(Some(tombstone))
    }

    pub fn validate(&mut self, proof: &LeaseProof, now: &UtcTimestamp) -> Result<(), StoreError> {
        self.expire_if_needed(now);
        let active = self.active.as_ref().ok_or(StoreError::LeaseRequired)?;
        validate_fencing(self.last_fencing_token, proof)?;
        validate_identity(active, proof)?;
        Ok(())
    }

    pub fn to_canonical_json(&self) -> Result<Vec<u8>, StoreError> {
        serde_json::to_vec(&LeaseLedgerWire::from(self)).map_err(|error| StoreError::InvalidState {
            reason: error.to_string().into_boxed_str(),
        })
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, StoreError> {
        let wire: LeaseLedgerWire =
            serde_json::from_slice(bytes).map_err(|error| StoreError::InvalidState {
                reason: format!("lease ledger JSON is invalid: {error}").into_boxed_str(),
            })?;
        wire.try_into()
    }

    fn end(
        &mut self,
        proof: &LeaseProof,
        now: UtcTimestamp,
        reason: &'static str,
    ) -> Result<LeaseTombstone, StoreError> {
        self.expire_if_needed(&now);
        let active = self.active.as_ref().ok_or(StoreError::LeaseRequired)?;
        validate_fencing(self.last_fencing_token, proof)?;
        validate_identity(active, proof)?;
        let active = self.active.take().expect("active lease was checked");
        let tombstone = LeaseTombstone {
            lease_id: active.lease_id,
            owner: active.owner,
            fencing_token: active.fencing_token,
            ended_at: now,
            reason: reason.into(),
        };
        self.tombstones.push(tombstone.clone());
        Ok(tombstone)
    }

    fn expire_if_needed(&mut self, now: &UtcTimestamp) {
        let expired = self
            .active
            .as_ref()
            .is_some_and(|record| !record.is_active_at(now));
        if expired {
            let active = self.active.take().expect("expired active lease exists");
            self.tombstones.push(LeaseTombstone {
                lease_id: active.lease_id,
                owner: active.owner,
                fencing_token: active.fencing_token,
                ended_at: now.clone(),
                reason: "expired".into(),
            });
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LeaseLedgerWire {
    schema_version: u32,
    last_fencing_token: u64,
    active: Option<LeaseRecordWire>,
    tombstones: Vec<LeaseTombstoneWire>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LeaseRecordWire {
    lease_id: Box<str>,
    owner: Box<str>,
    fencing_token: u64,
    acquired_at: Box<str>,
    expires_at: Box<str>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LeaseTombstoneWire {
    lease_id: Box<str>,
    owner: Box<str>,
    fencing_token: u64,
    ended_at: Box<str>,
    reason: Box<str>,
}

impl From<&LeaseLedger> for LeaseLedgerWire {
    fn from(ledger: &LeaseLedger) -> Self {
        Self {
            schema_version: 1,
            last_fencing_token: ledger.last_fencing_token.get(),
            active: ledger.active.as_ref().map(|record| LeaseRecordWire {
                lease_id: record.lease_id.to_string().into_boxed_str(),
                owner: record.owner.as_str().into(),
                fencing_token: record.fencing_token.get(),
                acquired_at: record.acquired_at.to_string().into_boxed_str(),
                expires_at: record.expires_at.to_string().into_boxed_str(),
            }),
            tombstones: ledger
                .tombstones
                .iter()
                .map(|tombstone| LeaseTombstoneWire {
                    lease_id: tombstone.lease_id.to_string().into_boxed_str(),
                    owner: tombstone.owner.as_str().into(),
                    fencing_token: tombstone.fencing_token.get(),
                    ended_at: tombstone.ended_at.to_string().into_boxed_str(),
                    reason: tombstone.reason.clone(),
                })
                .collect(),
        }
    }
}

impl TryFrom<LeaseLedgerWire> for LeaseLedger {
    type Error = StoreError;

    fn try_from(wire: LeaseLedgerWire) -> Result<Self, Self::Error> {
        if wire.schema_version != 1 {
            return Err(StoreError::InvalidState {
                reason: format!("unsupported lease ledger schema {}", wire.schema_version)
                    .into_boxed_str(),
            });
        }
        let active = wire.active.map(LeaseRecord::try_from).transpose()?;
        let tombstones = wire
            .tombstones
            .into_iter()
            .map(LeaseTombstone::try_from)
            .collect::<Result<Vec<_>, StoreError>>()?;
        let last_fencing_token = FencingToken::new(wire.last_fencing_token);
        if active
            .as_ref()
            .is_some_and(|record| record.fencing_token > last_fencing_token)
            || tombstones
                .iter()
                .any(|record| record.fencing_token > last_fencing_token)
        {
            return Err(StoreError::InvalidState {
                reason: "lease ledger contains a token beyond lastFencingToken".into(),
            });
        }
        Ok(Self {
            last_fencing_token,
            active,
            tombstones,
        })
    }
}

impl TryFrom<LeaseRecordWire> for LeaseRecord {
    type Error = StoreError;

    fn try_from(wire: LeaseRecordWire) -> Result<Self, Self::Error> {
        let acquired_at = UtcTimestamp::from_str(&wire.acquired_at)?;
        let expires_at = UtcTimestamp::from_str(&wire.expires_at)?;
        if expires_at <= acquired_at {
            return Err(StoreError::InvalidState {
                reason: "persisted lease expires before acquisition".into(),
            });
        }
        Ok(Self {
            lease_id: LeaseId::from_str(&wire.lease_id)?,
            owner: LeaseOwner::new(wire.owner)?,
            fencing_token: FencingToken::new(wire.fencing_token),
            acquired_at,
            expires_at,
        })
    }
}

impl TryFrom<LeaseTombstoneWire> for LeaseTombstone {
    type Error = StoreError;

    fn try_from(wire: LeaseTombstoneWire) -> Result<Self, Self::Error> {
        if wire.reason.is_empty() || wire.reason.len() > 1024 {
            return Err(StoreError::InvalidState {
                reason: "persisted lease tombstone reason is invalid".into(),
            });
        }
        Ok(Self {
            lease_id: LeaseId::from_str(&wire.lease_id)?,
            owner: LeaseOwner::new(wire.owner)?,
            fencing_token: FencingToken::new(wire.fencing_token),
            ended_at: UtcTimestamp::from_str(&wire.ended_at)?,
            reason: wire.reason,
        })
    }
}

fn validate_identity(record: &LeaseRecord, proof: &LeaseProof) -> Result<(), StoreError> {
    if record.lease_id != proof.lease_id
        || record.owner != proof.owner
        || record.fencing_token != proof.fencing_token
    {
        return Err(StoreError::LeaseMismatch {
            lease_id: proof.lease_id,
        });
    }
    Ok(())
}

fn validate_fencing(minimum: FencingToken, proof: &LeaseProof) -> Result<(), StoreError> {
    if proof.fencing_token < minimum {
        Err(StoreError::StaleFencingToken {
            minimum,
            observed: proof.fencing_token,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use uuid::Uuid;

    use super::*;

    fn at(value: &str) -> UtcTimestamp {
        UtcTimestamp::from_str(value).expect("fixture timestamp is valid")
    }

    fn lease(value: u128) -> LeaseId {
        LeaseId::from_uuid(Uuid::from_u128(value))
    }

    #[test]
    fn acquire_and_break_are_bootstrap_operations_with_monotonic_fencing() {
        let mut ledger = LeaseLedger::empty(FencingToken::new(7));
        let owner = LeaseOwner::new("root-session").expect("owner is valid");
        let first = ledger
            .acquire(
                lease(1),
                owner.clone(),
                at("2026-07-23T00:00:00Z"),
                at("2026-07-23T00:05:00Z"),
            )
            .expect("bootstrap acquire does not require a prior lease");
        assert_eq!(first.fencing_token(), FencingToken::new(8));
        ledger
            .break_active(
                LeaseOwner::new("operator").expect("actor is valid"),
                "explicit recovery",
                at("2026-07-23T00:01:00Z"),
            )
            .expect("break succeeds");
        let second = ledger
            .acquire(
                lease(2),
                owner,
                at("2026-07-23T00:02:00Z"),
                at("2026-07-23T00:06:00Z"),
            )
            .expect("a new generation is acquired");
        assert_eq!(second.fencing_token(), FencingToken::new(9));
        assert_eq!(ledger.tombstones().len(), 1);
    }

    #[test]
    fn stale_proof_cannot_validate_after_generation_changes() {
        let mut ledger = LeaseLedger::empty(FencingToken::ZERO);
        let owner = LeaseOwner::new("session").expect("owner is valid");
        let first = ledger
            .acquire(
                lease(1),
                owner.clone(),
                at("2026-07-23T00:00:00Z"),
                at("2026-07-23T00:01:00Z"),
            )
            .expect("first lease is acquired");
        let stale = LeaseProof::from(&first);
        ledger
            .acquire(
                lease(2),
                owner,
                at("2026-07-23T00:02:00Z"),
                at("2026-07-23T00:03:00Z"),
            )
            .expect("expired lease is replaced");

        assert!(matches!(
            ledger.validate(&stale, &at("2026-07-23T00:02:30Z")),
            Err(StoreError::StaleFencingToken { .. })
        ));
    }
}
