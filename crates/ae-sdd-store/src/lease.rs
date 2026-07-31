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

    /// Releases the active lease owned by `owner`, if any.
    ///
    /// Differs from `release` in that the caller need not hold a `LeaseProof`;
    /// it is the correct entry point when a session closure is tearing down
    /// all leases that session owned. Replaces the previous behaviour in
    /// which an orphaned lease could permanently block `lease.acquire` on its
    /// work item.
    ///
    /// Takes `now_ms` rather than a `UtcTimestamp` so the runtime can drive
    /// the call from its injected `ClockPort` (which exposes Unix milliseconds)
    /// without taking a hard dependency on `jiff`.
    ///
    /// Returns the tombstone if a release occurred, `None` if no active lease
    /// matched.
    pub fn release_by_owner(
        &mut self,
        owner: &LeaseOwner,
        now_ms: u64,
    ) -> Result<Option<LeaseTombstone>, StoreError> {
        let now = UtcTimestamp::from_unix_ms(now_ms);
        self.expire_if_needed(&now);
        let Some(active) = self.active.take() else {
            return Ok(None);
        };
        if active.owner != *owner {
            // Wrong owner cannot release someone else's lease. Put the
            // record back so the active lease stays intact and the call
            // surfaces as `None` to the caller — the owner filter is the
            // observable signal of the no-match case.
            self.active = Some(active);
            return Ok(None);
        }
        let tombstone = LeaseTombstone {
            lease_id: active.lease_id,
            owner: active.owner,
            fencing_token: active.fencing_token,
            ended_at: now,
            reason: "session-closed".into(),
        };
        self.tombstones.push(tombstone.clone());
        Ok(Some(tombstone))
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
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|error| StoreError::InvalidState {
                reason: format!("lease ledger JSON is invalid: {error}").into_boxed_str(),
            })?;
        // `lastFencingToken` is the native discriminator. Ledgers written by the
        // retired Python CLI carry the generation in `fencingToken`/`history`
        // instead; they stay read-compatible here and gain the native shape on
        // the next write, exactly as a legacy evidence manifest gains a ledger.
        if value.get("lastFencingToken").is_some() {
            let wire: LeaseLedgerWire =
                serde_json::from_value(value).map_err(|error| StoreError::InvalidState {
                    reason: format!("lease ledger JSON is invalid: {error}").into_boxed_str(),
                })?;
            return wire.try_into();
        }
        let wire: LegacyLeaseLedgerWire =
            serde_json::from_value(value).map_err(|error| StoreError::InvalidState {
                reason: format!("legacy lease ledger JSON is invalid: {error}").into_boxed_str(),
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

/// Ledger shape written by the retired Python CLI. Read-only compatibility: it
/// is never emitted, only accepted so that pre-Rust Work Items stay operable.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyLeaseLedgerWire {
    status: Option<Box<str>>,
    lease_id: Option<Box<str>>,
    owner: Option<LegacyLeaseOwnerWire>,
    fencing_token: Option<u64>,
    acquired_at: Option<Box<str>>,
    expires_at: Option<Box<str>>,
    released_at: Option<Box<str>>,
    #[serde(default)]
    history: Vec<LegacyLeaseEventWire>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyLeaseEventWire {
    event: Box<str>,
    lease_id: Box<str>,
    owner: LegacyLeaseOwnerWire,
    fencing_token: u64,
    at: Box<str>,
}

/// Python recorded the owner as four fields; Rust owns a single bounded string.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyLeaseOwnerWire {
    agent_id: Box<str>,
    session_id: Box<str>,
}

impl LegacyLeaseOwnerWire {
    fn flatten(&self) -> Result<LeaseOwner, StoreError> {
        let joined = format!("{}/{}", self.agent_id, self.session_id);
        let bounded = joined
            .char_indices()
            .take_while(|(index, character)| index + character.len_utf8() <= LeaseOwner::MAX_BYTES)
            .map(|(_, character)| character)
            .collect::<String>();
        LeaseOwner::new(bounded)
    }
}

impl TryFrom<LegacyLeaseLedgerWire> for LeaseLedger {
    type Error = StoreError;

    fn try_from(wire: LegacyLeaseLedgerWire) -> Result<Self, Self::Error> {
        let mut tombstones = Vec::with_capacity(wire.history.len());
        for event in &wire.history {
            // Only terminal events end a generation. `acquired` opens one and
            // `renewed` extends one in place; recording either as a tombstone
            // would invent a lease ending that never happened.
            if !matches!(event.event.as_ref(), "released" | "expired" | "broken") {
                continue;
            }
            tombstones.push(LeaseTombstone {
                lease_id: LeaseId::from_str(&event.lease_id)?,
                owner: event.owner.flatten()?,
                fencing_token: FencingToken::new(event.fencing_token),
                ended_at: UtcTimestamp::from_str(&event.at)?,
                reason: event.event.clone(),
            });
        }
        // A released or expired Python lease holds nothing; only a still-held
        // one becomes an active record. `releasedAt` is authoritative because
        // the legacy writer set it in the same write as the terminal status.
        let held = wire.released_at.is_none()
            && !matches!(
                wire.status.as_deref(),
                Some("released" | "expired" | "broken")
            );
        let active = match (
            held,
            &wire.lease_id,
            &wire.owner,
            &wire.acquired_at,
            &wire.expires_at,
        ) {
            (true, Some(lease_id), Some(owner), Some(acquired_at), Some(expires_at)) => {
                let acquired_at = UtcTimestamp::from_str(acquired_at)?;
                let expires_at = UtcTimestamp::from_str(expires_at)?;
                if expires_at <= acquired_at {
                    return Err(StoreError::InvalidState {
                        reason: "legacy lease expires before acquisition".into(),
                    });
                }
                Some(LeaseRecord {
                    lease_id: LeaseId::from_str(lease_id)?,
                    owner: owner.flatten()?,
                    fencing_token: FencingToken::new(wire.fencing_token.unwrap_or_default()),
                    acquired_at,
                    expires_at,
                })
            }
            _ => None,
        };
        // The generation must not regress, or a stale proof from the Python era
        // could validate again. Take the highest token the file mentions.
        let last_fencing_token = FencingToken::new(
            wire.fencing_token
                .unwrap_or_default()
                .max(
                    wire.history
                        .iter()
                        .map(|event| event.fencing_token)
                        .max()
                        .unwrap_or_default(),
                )
                .max(
                    active
                        .as_ref()
                        .map_or(0, |record| record.fencing_token.get()),
                ),
        );
        Ok(Self {
            last_fencing_token,
            active,
            tombstones,
        })
    }
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

    /// Verbatim shape the retired Python CLI left in
    /// `.auto-engineering/*/state.lease.json`: `schemaVersion` is a string,
    /// there is no `lastFencingToken`, `owner` is an object, and lifecycle is
    /// carried by `status`/`releasedAt` plus a `history` array.
    const PYTHON_RELEASED_LEDGER: &str = r#"{
      "schemaVersion": "1",
      "status": "released",
      "leaseId": "c2a0061b-790d-43a4-9d60-7e1e2fcf6827",
      "owner": {"agentId":"ae-sdd-legacy-cli","sessionId":"state-write-37740",
                "host":"CHINAMI-LAEJPRK","pid":37740},
      "fencingToken": 9,
      "acquiredAt": "2026-07-27T01:13:57Z",
      "expiresAt": "2026-07-27T01:14:27Z",
      "ttlSeconds": 30,
      "acquireIdempotencyKey": "legacy-state-write-37740-lease",
      "history": [
        {"event":"acquired","leaseId":"6bea5578-e378-473d-b1d0-7e240356a77e",
         "owner":{"agentId":"claude-code","sessionId":"cutover","host":"H","pid":1},
         "fencingToken":8,"at":"2026-07-27T00:59:11Z"},
        {"event":"released","leaseId":"6bea5578-e378-473d-b1d0-7e240356a77e",
         "owner":{"agentId":"claude-code","sessionId":"cutover","host":"H","pid":1},
         "fencingToken":8,"at":"2026-07-27T01:03:56Z"}
      ],
      "releasedAt": "2026-07-27T01:13:57Z"
    }"#;

    #[test]
    fn release_by_owner_releases_only_when_owner_matches() {
        let mut ledger = LeaseLedger::empty(FencingToken::new(7));
        let owner = LeaseOwner::new("session-a").expect("owner is valid");
        let other = LeaseOwner::new("session-b").expect("other is valid");
        ledger
            .acquire(
                lease(1),
                owner.clone(),
                UtcTimestamp::from_unix_ms(0),
                UtcTimestamp::from_unix_ms(120_000),
            )
            .expect("acquire");

        let tombstone = ledger
            .release_by_owner(&other, 60_000)
            .expect("no ownership mismatch is an error");
        assert!(tombstone.is_none(), "wrong owner must not release anything");
        assert!(
            ledger.active().is_some(),
            "the active lease must remain when the owner does not match"
        );

        let tombstone = ledger
            .release_by_owner(&owner, 60_000)
            .expect("releasing the matching owner succeeds")
            .expect("a tombstone is produced");
        assert_eq!(tombstone.reason.as_ref(), "session-closed");
        assert_eq!(tombstone.lease_id, lease(1));
        assert!(ledger.active().is_none());
        assert_eq!(ledger.tombstones().len(), 1);
    }

    #[test]
    fn release_by_owner_after_expiry_is_a_noop() {
        let mut ledger = LeaseLedger::empty(FencingToken::new(7));
        let owner = LeaseOwner::new("session-a").expect("owner is valid");
        // Acquire and release both use the same Unix-millisecond epoch so
        // the sweep-vs-release boundary is unambiguous regardless of the
        // fixture time.
        ledger
            .acquire(
                lease(1),
                owner.clone(),
                UtcTimestamp::from_unix_ms(0),
                UtcTimestamp::from_unix_ms(10),
            )
            .expect("acquire");
        let initial_tombstones = ledger.tombstones().len();

        let tombstone = ledger
            .release_by_owner(&owner, 20)
            .expect("release after expiry is well-defined");
        assert!(
            tombstone.is_none(),
            "the sweep already happened; release_by_owner must report no-op"
        );
        assert!(ledger.active().is_none());
        assert_eq!(
            ledger.tombstones().len(),
            initial_tombstones + 1,
            "the sweep itself produced exactly one new tombstone"
        );
    }

    #[test]
    fn python_written_ledger_stays_readable_and_keeps_its_fencing_generation() {
        let ledger = LeaseLedger::from_json(PYTHON_RELEASED_LEDGER.as_bytes())
            .expect("a Python-era ledger must stay readable");
        assert_eq!(ledger.last_fencing_token(), FencingToken::new(9));
        assert!(
            ledger.active().is_none(),
            "a released Python ledger holds no active lease"
        );
    }

    #[test]
    fn a_python_ledger_grants_the_next_generation_without_reusing_a_token() {
        let mut ledger = LeaseLedger::from_json(PYTHON_RELEASED_LEDGER.as_bytes())
            .expect("a Python-era ledger must stay readable");
        let granted = ledger
            .acquire(
                lease(11),
                LeaseOwner::new("root-session").expect("owner is valid"),
                at("2026-07-28T00:00:00Z"),
                at("2026-07-28T00:05:00Z"),
            )
            .expect("acquire succeeds on a converted ledger");
        assert_eq!(granted.fencing_token(), FencingToken::new(10));
    }

    #[test]
    fn a_read_python_ledger_is_written_back_in_the_native_shape() {
        let ledger = LeaseLedger::from_json(PYTHON_RELEASED_LEDGER.as_bytes())
            .expect("a Python-era ledger must stay readable");
        let bytes = ledger.to_canonical_json().expect("canonical encode");
        let text = String::from_utf8(bytes).expect("canonical JSON is UTF-8");
        assert!(
            text.contains("\"lastFencingToken\":9") && text.contains("\"schemaVersion\":1"),
            "write-time normalization must emit the native shape: {text}"
        );
        LeaseLedger::from_json(text.as_bytes()).expect("normalized output round-trips");
    }
}
