#![allow(unused_imports)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use ae_sdd_context::{
    PressureDecision, PressurePolicy, PressureSample, PressureSource, PressureTracker,
};
use ae_sdd_domain::{
    AgentRole, ClaimId, CompactId, ContextGeneration, DelegationId, EventStoreId, HostAckId,
    HostActionId, InputFingerprint, SampleSequence, SessionId,
};
use ae_sdd_flow::{FlowDecision, FlowEvent, FlowInput, FlowRuntime};
use ae_sdd_host::{
    ChildClaim, HostAck, HostAckOutcome, HostAction, HostActionKind, HostAdapterId, HostTaskId,
    PhysicalSessionProof,
};
use ae_sdd_protocol::StableErrorCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    ContextProjectResult, ContextProjectionInput, DelegationCreatePayload, DelegationReportPayload,
    DelegationResult, HostAckPayload, HostActionPayload, HostPressurePayload, PersistencePort,
    RuntimeError, RuntimeResult, WireAgentRole,
};

/// Precomputed role-aware context cache and pressure tracker registry.
pub struct ContextCache {
    maximum_bytes: usize,
    projections: Mutex<BTreeMap<String, CachedProjection>>,
    pressure: Mutex<BTreeMap<String, PressureTracker>>,
}

#[derive(Clone, Debug)]
struct CachedProjection {
    source_revision: u64,
    context_revision: u64,
    digest: String,
    value: Value,
    bytes: usize,
}

impl ContextCache {
    /// Creates a context cache with a hard serialized byte bound.
    #[must_use]
    pub fn new(maximum_bytes: usize) -> Self {
        Self {
            maximum_bytes,
            projections: Mutex::new(BTreeMap::new()),
            pressure: Mutex::new(BTreeMap::new()),
        }
    }

    /// Replaces one precomputed projection after validating its exact serialized size.
    pub fn put(&self, input: ContextProjectionInput) -> RuntimeResult<ContextProjectResult> {
        let canonical = serde_json::to_vec(&input.projection).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ContextBudgetExceeded,
                "context projection could not be canonicalized",
            )
        })?;
        if canonical.len() > self.maximum_bytes {
            return Err(RuntimeError::new(
                StableErrorCode::ContextBudgetExceeded,
                "context projection exceeds the configured byte budget",
            ));
        }
        let digest = hex::encode(Sha256::digest(&canonical));
        let mut projections = self.projections.lock().map_err(lock_error)?;
        let revision = projections
            .get(&input.session_id)
            .map_or(1, |current| current.context_revision.saturating_add(1));
        if revision == 0 {
            return Err(RuntimeError::new(
                StableErrorCode::ContextRevisionStale,
                "context revision overflow",
            ));
        }
        let cached = CachedProjection {
            source_revision: input.source_revision,
            context_revision: revision,
            digest: digest.clone(),
            value: input.projection.clone(),
            bytes: canonical.len(),
        };
        projections.insert(input.session_id, cached);
        Ok(ContextProjectResult {
            kind: "full".to_owned(),
            context_revision: revision,
            digest,
            source_revision: input.source_revision,
            projection: Some(input.projection),
            byte_length: canonical.len(),
        })
    }

    /// Returns a full or no-change projection without filesystem or Gate work.
    pub fn project(
        &self,
        session_id: &str,
        known_revision: u64,
        known_digest: &str,
    ) -> RuntimeResult<ContextProjectResult> {
        let projections = self.projections.lock().map_err(lock_error)?;
        let cached = projections.get(session_id).ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::ContextRevisionStale,
                "no precomputed context exists for this trusted session",
            )
        })?;
        if known_revision > cached.context_revision {
            return Err(RuntimeError::new(
                StableErrorCode::ContextRevisionStale,
                "caller context revision is ahead of the daemon",
            ));
        }
        let unchanged = known_revision == cached.context_revision && known_digest == cached.digest;
        Ok(ContextProjectResult {
            kind: if unchanged { "no_change" } else { "full" }.to_owned(),
            context_revision: cached.context_revision,
            digest: cached.digest.clone(),
            source_revision: cached.source_revision,
            projection: (!unchanged).then(|| cached.value.clone()),
            byte_length: cached.bytes,
        })
    }

    /// Returns only the precomputed projection body for the Hook fast path.
    pub fn hook_projection(&self, session_id: &str) -> RuntimeResult<Option<Value>> {
        Ok(self
            .projections
            .lock()
            .map_err(lock_error)?
            .get(session_id)
            .map(|cached| cached.value.clone()))
    }

    /// Applies an authenticated host pressure sample and returns whether compact should start.
    pub fn observe_pressure(
        &self,
        session_id: SessionId,
        payload: &HostPressurePayload,
    ) -> RuntimeResult<PressureDecision> {
        let adapter_id = HostAdapterId::new(payload.adapter_id.clone().into_boxed_str())
            .map_err(|_| invalid_pressure("invalid host adapter identity"))?;
        let sample = PressureSample::new(
            adapter_id.clone(),
            session_id,
            ContextGeneration::new(payload.context_generation),
            SampleSequence::new(payload.sample_seq),
            payload.used_tokens,
            payload.context_window_tokens,
            PressureSource::HostTokenCounter,
            payload.observed_at_unix_ms,
        )
        .map_err(|_| invalid_pressure("invalid or replayed pressure sample"))?;
        let key = format!("{}\0{}", payload.adapter_id, session_id);
        let mut trackers = self.pressure.lock().map_err(lock_error)?;
        let tracker = trackers.entry(key).or_insert_with(|| {
            PressureTracker::new(
                adapter_id,
                session_id,
                ContextGeneration::new(payload.context_generation),
                PressurePolicy::default(),
            )
        });
        tracker
            .observe(&sample)
            .map_err(|_| invalid_pressure("pressure identity, generation, or sequence mismatch"))
    }
}

fn lock_error<T>(_error: std::sync::PoisonError<T>) -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::ExternalStateConflict,
        "runtime supervisor lock is poisoned",
    )
}

fn invalid_pressure(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::CompactAckInvalid, message)
}
