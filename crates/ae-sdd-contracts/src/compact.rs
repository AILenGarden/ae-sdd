//! Frozen compact request, acknowledgement, and rehydrate contracts.

use ae_sdd_domain::{
    ArtifactRef, CompactId, ContextDigest, ContextGeneration, HostAckId, HostActionId, SessionId,
};
use ae_sdd_protocol::HostAckOutcome;
use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

use crate::{
    AdapterId, IdempotencyKey, SchemaVersion,
    host::{AttestedAck, HostAction, HostActionBody, HostContractError},
    serde_domain,
};

/// Durable request to compact one exact physical session generation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactRequest {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::compact_id")]
    compact_id: CompactId,
    #[serde(with = "serde_domain::session_id")]
    session_id: SessionId,
    adapter_id: AdapterId,
    #[serde(with = "serde_domain::artifact_ref")]
    snapshot_ref: ArtifactRef,
    #[serde(with = "serde_domain::context_generation")]
    previous_generation: ContextGeneration,
    #[serde(with = "serde_domain::context_generation")]
    next_generation: ContextGeneration,
    deadline_unix_ms: u64,
    idempotency_key: IdempotencyKey,
}

impl CompactRequest {
    /// Validates and constructs a generation-CAS compact request.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        compact_id: CompactId,
        session_id: SessionId,
        adapter_id: AdapterId,
        snapshot_ref: ArtifactRef,
        previous_generation: ContextGeneration,
        next_generation: ContextGeneration,
        deadline_unix_ms: u64,
        idempotency_key: IdempotencyKey,
    ) -> Result<Self, CompactContractError> {
        validate_generation_step(previous_generation, next_generation)?;
        if deadline_unix_ms == 0 {
            return Err(CompactContractError::ZeroDeadline);
        }
        Ok(Self {
            schema_version,
            compact_id,
            session_id,
            adapter_id,
            snapshot_ref,
            previous_generation,
            next_generation,
            deadline_unix_ms,
            idempotency_key,
        })
    }

    /// Returns the contract schema version.
    #[must_use]
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the durable compact cycle identity.
    #[must_use]
    pub const fn compact_id(&self) -> CompactId {
        self.compact_id
    }

    /// Returns the exact physical session being compacted.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Returns the authenticated target Host Adapter.
    #[must_use]
    pub const fn adapter_id(&self) -> &AdapterId {
        &self.adapter_id
    }

    /// Returns the durable pre-compact snapshot reference.
    #[must_use]
    pub const fn snapshot_ref(&self) -> &ArtifactRef {
        &self.snapshot_ref
    }

    /// Returns the current generation expected by the compact CAS.
    #[must_use]
    pub const fn previous_generation(&self) -> ContextGeneration {
        self.previous_generation
    }

    /// Returns the single allowed successor generation.
    #[must_use]
    pub const fn next_generation(&self) -> ContextGeneration {
        self.next_generation
    }

    /// Returns the absolute Host Adapter ACK deadline.
    #[must_use]
    pub const fn deadline_unix_ms(&self) -> u64 {
        self.deadline_unix_ms
    }

    /// Returns the retry-safe compact operation key.
    #[must_use]
    pub const fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompactRequestWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::compact_id")]
    compact_id: CompactId,
    #[serde(with = "serde_domain::session_id")]
    session_id: SessionId,
    adapter_id: AdapterId,
    #[serde(with = "serde_domain::artifact_ref")]
    snapshot_ref: ArtifactRef,
    #[serde(with = "serde_domain::context_generation")]
    previous_generation: ContextGeneration,
    #[serde(with = "serde_domain::context_generation")]
    next_generation: ContextGeneration,
    deadline_unix_ms: u64,
    idempotency_key: IdempotencyKey,
}

impl<'de> Deserialize<'de> for CompactRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = CompactRequestWire::deserialize(deserializer)?;
        Self::new(
            wire.schema_version,
            wire.compact_id,
            wire.session_id,
            wire.adapter_id,
            wire.snapshot_ref,
            wire.previous_generation,
            wire.next_generation,
            wire.deadline_unix_ms,
            wire.idempotency_key,
        )
        .map_err(de::Error::custom)
    }
}

/// Correlated host ACK for a compact action.
///
/// This type intentionally contains no restored projection digest. Host ACK
/// and projection rehydration are separate durability facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactAck {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::compact_id")]
    compact_id: CompactId,
    #[serde(with = "serde_domain::session_id")]
    session_id: SessionId,
    adapter_id: AdapterId,
    #[serde(with = "serde_domain::context_generation")]
    previous_generation: ContextGeneration,
    #[serde(with = "serde_domain::context_generation")]
    next_generation: ContextGeneration,
    ack: AttestedAck,
}

impl CompactAck {
    /// Validates the compact request, typed host action, and authenticated ACK.
    pub fn new(
        schema_version: SchemaVersion,
        request: &CompactRequest,
        action: &HostAction,
        ack: AttestedAck,
    ) -> Result<Self, CompactContractError> {
        if schema_version != request.schema_version
            || schema_version != action.schema_version()
            || schema_version != ack.schema_version()
        {
            return Err(CompactContractError::SchemaVersionMismatch);
        }
        let HostActionBody::Compact(compact) = action.body() else {
            return Err(CompactContractError::CompactActionRequired);
        };
        if compact.request() != request {
            return Err(CompactContractError::RequestActionMismatch);
        }
        ack.validate_for(action)?;
        if ack.outcome() != HostAckOutcome::Accepted {
            return Err(CompactContractError::AckNotAccepted);
        }
        if ack.session_id() != Some(request.session_id)
            || ack.observed_generation() != Some(request.previous_generation)
        {
            return Err(CompactContractError::AckGenerationMismatch);
        }
        Ok(Self {
            schema_version,
            compact_id: request.compact_id,
            session_id: request.session_id,
            adapter_id: request.adapter_id.clone(),
            previous_generation: request.previous_generation,
            next_generation: request.next_generation,
            ack,
        })
    }

    /// Validates the ACK against a compact request after restart or replay.
    pub fn validate_for(&self, request: &CompactRequest) -> Result<(), CompactContractError> {
        if self.schema_version != request.schema_version
            || self.compact_id != request.compact_id
            || self.session_id != request.session_id
            || self.adapter_id != request.adapter_id
            || self.previous_generation != request.previous_generation
            || self.next_generation != request.next_generation
            || self.ack.adapter_id() != &request.adapter_id
            || self.ack.session_id() != Some(request.session_id)
            || self.ack.observed_generation() != Some(request.previous_generation)
            || self.ack.outcome() != HostAckOutcome::Accepted
        {
            return Err(CompactContractError::AckGenerationMismatch);
        }
        Ok(())
    }

    /// Returns the contract schema version.
    #[must_use]
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the compact cycle identity.
    #[must_use]
    pub const fn compact_id(&self) -> CompactId {
        self.compact_id
    }

    /// Returns the compacted physical session.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Returns the authenticated Host Adapter identity.
    #[must_use]
    pub const fn adapter_id(&self) -> &AdapterId {
        &self.adapter_id
    }

    /// Returns the generation observed before host compact.
    #[must_use]
    pub const fn previous_generation(&self) -> ContextGeneration {
        self.previous_generation
    }

    /// Returns the generation reserved for successful rehydration.
    #[must_use]
    pub const fn next_generation(&self) -> ContextGeneration {
        self.next_generation
    }

    /// Returns the authenticated underlying Host Adapter ACK.
    #[must_use]
    pub const fn ack(&self) -> &AttestedAck {
        &self.ack
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompactAckWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::compact_id")]
    compact_id: CompactId,
    #[serde(with = "serde_domain::session_id")]
    session_id: SessionId,
    adapter_id: AdapterId,
    #[serde(with = "serde_domain::context_generation")]
    previous_generation: ContextGeneration,
    #[serde(with = "serde_domain::context_generation")]
    next_generation: ContextGeneration,
    ack: AttestedAck,
}

impl<'de> Deserialize<'de> for CompactAck {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = CompactAckWire::deserialize(deserializer)?;
        validate_generation_step(wire.previous_generation, wire.next_generation)
            .map_err(de::Error::custom)?;
        if wire.schema_version != wire.ack.schema_version()
            || wire.adapter_id != *wire.ack.adapter_id()
            || wire.session_id
                != wire.ack.session_id().ok_or_else(|| {
                    de::Error::custom("compact ACK requires an exact session binding")
                })?
            || wire.previous_generation
                != wire.ack.observed_generation().ok_or_else(|| {
                    de::Error::custom("compact ACK requires an observed generation")
                })?
            || wire.ack.outcome() != HostAckOutcome::Accepted
        {
            return Err(de::Error::custom("invalid compact ACK correlation"));
        }
        Ok(Self {
            schema_version: wire.schema_version,
            compact_id: wire.compact_id,
            session_id: wire.session_id,
            adapter_id: wire.adapter_id,
            previous_generation: wire.previous_generation,
            next_generation: wire.next_generation,
            ack: wire.ack,
        })
    }
}

/// Separate proof that projection rehydration succeeded and generation advanced.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RehydrateReceipt {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::compact_id")]
    compact_id: CompactId,
    #[serde(with = "serde_domain::host_action_id")]
    action_id: HostActionId,
    #[serde(with = "serde_domain::host_ack_id")]
    ack_id: HostAckId,
    #[serde(with = "serde_domain::session_id")]
    session_id: SessionId,
    #[serde(with = "serde_domain::context_generation")]
    previous_generation: ContextGeneration,
    #[serde(with = "serde_domain::context_generation")]
    restored_generation: ContextGeneration,
    #[serde(with = "serde_domain::context_digest")]
    restored_projection_digest: ContextDigest,
    rehydrated_at_unix_ms: u64,
}

impl RehydrateReceipt {
    /// Validates and constructs a rehydration receipt after a matching host ACK.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        request: &CompactRequest,
        ack: &CompactAck,
        restored_generation: ContextGeneration,
        restored_projection_digest: ContextDigest,
        rehydrated_at_unix_ms: u64,
    ) -> Result<Self, CompactContractError> {
        ack.validate_for(request)?;
        if schema_version != request.schema_version || schema_version != ack.schema_version {
            return Err(CompactContractError::SchemaVersionMismatch);
        }
        if restored_generation != request.next_generation {
            return Err(CompactContractError::RehydrateGenerationMismatch {
                expected: request.next_generation.get(),
                actual: restored_generation.get(),
            });
        }
        if rehydrated_at_unix_ms == 0 || rehydrated_at_unix_ms < ack.ack.observed_at_unix_ms() {
            return Err(CompactContractError::InvalidRehydrateTime);
        }
        Ok(Self {
            schema_version,
            compact_id: request.compact_id,
            action_id: ack.ack.action_id(),
            ack_id: ack.ack.ack_id(),
            session_id: request.session_id,
            previous_generation: request.previous_generation,
            restored_generation,
            restored_projection_digest,
            rehydrated_at_unix_ms,
        })
    }

    /// Returns the compact cycle identity.
    #[must_use]
    pub const fn compact_id(&self) -> CompactId {
        self.compact_id
    }

    /// Returns the correlated host action identity.
    #[must_use]
    pub const fn action_id(&self) -> HostActionId {
        self.action_id
    }

    /// Returns the correlated host ACK identity.
    #[must_use]
    pub const fn ack_id(&self) -> HostAckId {
        self.ack_id
    }

    /// Returns the rehydrated physical session.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Returns the generation that was compacted.
    #[must_use]
    pub const fn previous_generation(&self) -> ContextGeneration {
        self.previous_generation
    }

    /// Returns the generation made active by rehydration.
    #[must_use]
    pub const fn restored_generation(&self) -> ContextGeneration {
        self.restored_generation
    }

    /// Returns the digest of the restored bounded projection.
    #[must_use]
    pub const fn restored_projection_digest(&self) -> ContextDigest {
        self.restored_projection_digest
    }

    /// Returns the rehydration completion timestamp.
    #[must_use]
    pub const fn rehydrated_at_unix_ms(&self) -> u64 {
        self.rehydrated_at_unix_ms
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RehydrateReceiptWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::compact_id")]
    compact_id: CompactId,
    #[serde(with = "serde_domain::host_action_id")]
    action_id: HostActionId,
    #[serde(with = "serde_domain::host_ack_id")]
    ack_id: HostAckId,
    #[serde(with = "serde_domain::session_id")]
    session_id: SessionId,
    #[serde(with = "serde_domain::context_generation")]
    previous_generation: ContextGeneration,
    #[serde(with = "serde_domain::context_generation")]
    restored_generation: ContextGeneration,
    #[serde(with = "serde_domain::context_digest")]
    restored_projection_digest: ContextDigest,
    rehydrated_at_unix_ms: u64,
}

impl<'de> Deserialize<'de> for RehydrateReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RehydrateReceiptWire::deserialize(deserializer)?;
        validate_generation_step(wire.previous_generation, wire.restored_generation)
            .map_err(de::Error::custom)?;
        if wire.rehydrated_at_unix_ms == 0 {
            return Err(de::Error::custom(
                "rehydration timestamp must be greater than zero",
            ));
        }
        Ok(Self {
            schema_version: wire.schema_version,
            compact_id: wire.compact_id,
            action_id: wire.action_id,
            ack_id: wire.ack_id,
            session_id: wire.session_id,
            previous_generation: wire.previous_generation,
            restored_generation: wire.restored_generation,
            restored_projection_digest: wire.restored_projection_digest,
            rehydrated_at_unix_ms: wire.rehydrated_at_unix_ms,
        })
    }
}

/// Validation error for compact and rehydrate contracts.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CompactContractError {
    /// The requested successor generation was not exactly one greater.
    #[error("compact generation must advance exactly once ({previous} -> {next})")]
    NonConsecutiveGeneration {
        /// Current context generation.
        previous: u64,
        /// Requested successor generation.
        next: u64,
    },
    /// The compact request omitted its absolute ACK deadline.
    #[error("compact deadline must be greater than zero")]
    ZeroDeadline,
    /// Nested contract schema versions did not match.
    #[error("nested compact contract schema versions do not match")]
    SchemaVersionMismatch,
    /// The supplied host action was not a typed compact action.
    #[error("compact ACK requires a typed compact host action")]
    CompactActionRequired,
    /// The host action embedded a different compact request.
    #[error("compact host action does not contain the supplied request")]
    RequestActionMismatch,
    /// The Host Adapter ACK was rejected or failed.
    #[error("compact ACK must have an accepted Host Adapter outcome")]
    AckNotAccepted,
    /// ACK identity, session, adapter, or generation did not match.
    #[error("compact ACK does not match the requested session generation")]
    AckGenerationMismatch,
    /// Rehydration attempted to activate a generation other than the reserved successor.
    #[error("rehydrated generation must be {expected} (actual: {actual})")]
    RehydrateGenerationMismatch {
        /// Required successor generation.
        expected: u64,
        /// Observed restored generation.
        actual: u64,
    },
    /// Rehydration time was zero or preceded its host ACK.
    #[error("rehydration time must be non-zero and no earlier than the host ACK")]
    InvalidRehydrateTime,
    /// The underlying Host Adapter contract was invalid.
    #[error(transparent)]
    Host(#[from] HostContractError),
}

fn validate_generation_step(
    previous: ContextGeneration,
    next: ContextGeneration,
) -> Result<(), CompactContractError> {
    let expected = previous.get().checked_add(1);
    if expected != Some(next.get()) {
        return Err(CompactContractError::NonConsecutiveGeneration {
            previous: previous.get(),
            next: next.get(),
        });
    }
    Ok(())
}
