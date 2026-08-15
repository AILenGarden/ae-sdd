#![allow(unused_imports)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use ae_sdd_context::{
    ExecutionCapsuleProjection, PressureDecision, PressurePolicy, PressureSample, PressureSource,
    PressureTracker,
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
    previous: Option<Box<HistoricalProjection>>,
    /// Digest last delivered on the Hook fast path; `None` forces the next
    /// Hook to deliver the full body (fresh projection or compact rehydrate).
    hook_delivered_digest: Option<String>,
}

#[derive(Clone, Debug)]
struct HistoricalProjection {
    context_revision: u64,
    digest: String,
    value: Value,
}

/// Hook fast-path projection view: the cached body plus whether this Hook
/// turn must re-deliver it or may answer with a digest-only no-change.
#[derive(Clone, Debug)]
pub struct HookProjection {
    /// Cached context revision.
    pub context_revision: u64,
    /// Digest of the cached projection body.
    pub digest: String,
    /// True when the body must be (re-)delivered to the host.
    pub deliver: bool,
    /// Cached projection body.
    pub value: Value,
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
        let ContextProjectionInput {
            session_id,
            source_revision,
            projection,
        } = input;
        let canonical = serde_json::to_vec(&projection).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ContextBudgetExceeded,
                "context projection could not be canonicalized",
            )
        })?;
        let digest = hex::encode(Sha256::digest(&canonical));
        self.put_canonical(
            &session_id,
            source_revision,
            projection,
            digest,
            canonical.len(),
        )
    }

    /// Stores a typed execution-capsule projection through the same
    /// full/delta/no-change machinery as [`Self::put`], but binds the cache
    /// entry digest to the canonical capsule digest so approved-plan, queue or
    /// capsule drift changes the digest clients resume against.  No second
    /// delta algorithm is introduced: the serialized capsule value is the
    /// projection body the existing `project` delta diffs.
    pub fn put_execution_capsule(
        &self,
        stream_key: &str,
        source_revision: u64,
        projection: &ExecutionCapsuleProjection,
    ) -> RuntimeResult<ContextProjectResult> {
        let canonical = serde_json::to_vec(projection.value()).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ContextBudgetExceeded,
                "execution capsule projection could not be canonicalized",
            )
        })?;
        self.put_canonical(
            stream_key,
            source_revision,
            projection.value().clone(),
            projection.digest().to_hex(),
            canonical.len(),
        )
    }

    fn put_canonical(
        &self,
        stream_key: &str,
        source_revision: u64,
        projection: Value,
        digest: String,
        bytes: usize,
    ) -> RuntimeResult<ContextProjectResult> {
        if bytes > self.maximum_bytes {
            return Err(RuntimeError::new(
                StableErrorCode::ContextBudgetExceeded,
                "context projection exceeds the configured byte budget",
            ));
        }
        let mut projections = self.projections.lock().map_err(lock_error)?;
        if let Some(current) = projections.get(stream_key)
            && current.digest == digest
            && current.source_revision == source_revision
        {
            return Ok(ContextProjectResult {
                kind: "no_change".to_owned(),
                context_revision: current.context_revision,
                digest,
                source_revision: current.source_revision,
                projection: None,
                byte_length: 0,
            });
        }
        let revision = projections
            .get(stream_key)
            .map_or(1, |current| current.context_revision.saturating_add(1));
        if revision == 0 {
            return Err(RuntimeError::new(
                StableErrorCode::ContextRevisionStale,
                "context revision overflow",
            ));
        }
        let previous = projections.get(stream_key).map(|current| {
            Box::new(HistoricalProjection {
                context_revision: current.context_revision,
                digest: current.digest.clone(),
                value: current.value.clone(),
            })
        });
        let cached = CachedProjection {
            source_revision,
            context_revision: revision,
            digest: digest.clone(),
            value: projection.clone(),
            bytes,
            previous,
            hook_delivered_digest: None,
        };
        projections.insert(stream_key.to_owned(), cached);
        Ok(ContextProjectResult {
            kind: "full".to_owned(),
            context_revision: revision,
            digest,
            source_revision,
            projection: Some(projection),
            byte_length: bytes,
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
        if known_revision == cached.context_revision && !known_digest.is_empty() && !unchanged {
            return Err(RuntimeError::new(
                StableErrorCode::ContextRevisionStale,
                "caller context digest does not match the daemon revision",
            ));
        }
        if !unchanged
            && let Some(previous) = cached.previous.as_deref()
            && known_revision == previous.context_revision
            && known_digest == previous.digest
        {
            let delta = projection_delta(&previous.value, &cached.value);
            let delta_bytes = serde_json::to_vec(&delta).map_err(|_| {
                RuntimeError::new(
                    StableErrorCode::ContextBudgetExceeded,
                    "context delta could not be canonicalized",
                )
            })?;
            if delta_bytes.len() <= self.maximum_bytes {
                return Ok(ContextProjectResult {
                    kind: "delta".to_owned(),
                    context_revision: cached.context_revision,
                    digest: cached.digest.clone(),
                    source_revision: cached.source_revision,
                    projection: Some(delta),
                    byte_length: delta_bytes.len(),
                });
            }
        }
        Ok(ContextProjectResult {
            kind: if unchanged { "no_change" } else { "full" }.to_owned(),
            context_revision: cached.context_revision,
            digest: cached.digest.clone(),
            source_revision: cached.source_revision,
            projection: (!unchanged).then(|| cached.value.clone()),
            byte_length: cached.bytes,
        })
    }

    /// Returns the precomputed projection for the Hook fast path together
    /// with its delivery state.  A projection whose digest was already
    /// delivered reports `deliver == false` so the Hook response can omit the
    /// body and answer with a digest-only no-change.
    pub fn hook_projection(&self, session_id: &str) -> RuntimeResult<Option<HookProjection>> {
        Ok(self
            .projections
            .lock()
            .map_err(lock_error)?
            .get(session_id)
            .map(|cached| HookProjection {
                context_revision: cached.context_revision,
                digest: cached.digest.clone(),
                deliver: cached.hook_delivered_digest.as_deref() != Some(cached.digest.as_str()),
                value: cached.value.clone(),
            }))
    }

    /// Records that the Hook fast path delivered the projection carrying
    /// `digest`; a stale mark naming a superseded digest is ignored.
    pub fn mark_hook_delivered(&self, session_id: &str, digest: &str) -> RuntimeResult<()> {
        let mut projections = self.projections.lock().map_err(lock_error)?;
        if let Some(cached) = projections.get_mut(session_id)
            && cached.digest == digest
        {
            cached.hook_delivered_digest = Some(digest.to_owned());
        }
        Ok(())
    }

    /// Forces the next Hook projection for the session to deliver the full
    /// body again; a compact rehydrate replaces the host-side context window,
    /// so the previously delivered body is gone even when the digest matches.
    pub fn mark_hook_redelivery(&self, session_id: &str) -> RuntimeResult<()> {
        let mut projections = self.projections.lock().map_err(lock_error)?;
        if let Some(cached) = projections.get_mut(session_id) {
            cached.hook_delivered_digest = None;
        }
        Ok(())
    }

    /// Removes a cached projection so engaged Hooks fail closed until reprojected.
    pub fn invalidate(&self, session_id: &str) -> RuntimeResult<()> {
        self.projections
            .lock()
            .map_err(lock_error)?
            .remove(session_id);
        Ok(())
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

fn projection_delta(previous: &Value, current: &Value) -> Value {
    match (previous.as_object(), current.as_object()) {
        (Some(previous), Some(current)) => {
            let mut set = serde_json::Map::new();
            let mut remove = Vec::new();
            for (key, value) in current {
                if previous.get(key) != Some(value) {
                    set.insert(key.clone(), value.clone());
                }
            }
            for key in previous.keys() {
                if !current.contains_key(key) {
                    remove.push(Value::String(key.clone()));
                }
            }
            json!({
                "schemaVersion":"context-delta/v1",
                "set":set,
                "remove":remove,
            })
        }
        _ => json!({
            "schemaVersion":"context-delta/v1",
            "replace":current,
        }),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn input(session_id: &str, source_revision: u64, projection: Value) -> ContextProjectionInput {
        ContextProjectionInput {
            session_id: session_id.to_owned(),
            source_revision,
            projection,
        }
    }

    #[test]
    fn hook_delivery_is_reported_once_until_the_projection_moves() {
        let cache = ContextCache::new(65_536);
        cache
            .put(input("session", 1, json!({"phase":"coding"})))
            .expect("initial projection");

        let first = cache
            .hook_projection("session")
            .expect("hook projection")
            .expect("cached projection");
        assert!(first.deliver, "a fresh projection must be delivered");
        cache
            .mark_hook_delivered("session", &first.digest)
            .expect("delivery mark");

        let second = cache
            .hook_projection("session")
            .expect("hook projection")
            .expect("cached projection");
        assert!(
            !second.deliver,
            "an unchanged projection is not re-delivered"
        );
        assert_eq!(second.digest, first.digest);
        assert_eq!(second.value, json!({"phase":"coding"}));

        cache
            .put(input("session", 2, json!({"phase":"test-running"})))
            .expect("moved projection");
        let third = cache
            .hook_projection("session")
            .expect("hook projection")
            .expect("cached projection");
        assert!(third.deliver, "a moved projection must be delivered again");
        assert_ne!(third.digest, first.digest);
    }

    #[test]
    fn stale_delivery_marks_are_ignored_and_redelivery_can_be_forced() {
        let cache = ContextCache::new(65_536);
        cache
            .put(input("session", 1, json!({"phase":"coding"})))
            .expect("initial projection");
        let first = cache
            .hook_projection("session")
            .expect("hook projection")
            .expect("cached projection");

        cache
            .mark_hook_delivered("session", "not-the-digest")
            .expect("stale mark is a no-op");
        let still_fresh = cache
            .hook_projection("session")
            .expect("hook projection")
            .expect("cached projection");
        assert!(
            still_fresh.deliver,
            "a mark for a foreign digest must not suppress delivery"
        );

        cache
            .mark_hook_delivered("session", &first.digest)
            .expect("delivery mark");
        cache
            .mark_hook_redelivery("session")
            .expect("forced redelivery");
        let forced = cache
            .hook_projection("session")
            .expect("hook projection")
            .expect("cached projection");
        assert!(
            forced.deliver,
            "a compact rehydrate forces one full redelivery of the same digest"
        );
        assert_eq!(forced.digest, first.digest);
    }

    #[test]
    fn invalidate_drops_the_delivery_state_with_the_projection() {
        let cache = ContextCache::new(65_536);
        cache
            .put(input("session", 1, json!({"phase":"coding"})))
            .expect("initial projection");
        let first = cache
            .hook_projection("session")
            .expect("hook projection")
            .expect("cached projection");
        cache
            .mark_hook_delivered("session", &first.digest)
            .expect("delivery mark");
        cache.invalidate("session").expect("invalidate");
        assert!(
            cache
                .hook_projection("session")
                .expect("hook projection")
                .is_none(),
            "an invalidated session has no hook projection at all"
        );
    }
}
