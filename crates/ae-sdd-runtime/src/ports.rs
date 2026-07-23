use std::collections::BTreeMap;
use std::sync::Mutex;

use ae_sdd_domain::EventStoreId;
use ae_sdd_protocol::{RequestParams, RpcMethod, StableErrorCode};
use serde_json::Value;

use crate::{DurableEvent, IdempotencyReceipt, RuntimeError, RuntimeResult};

/// Clock used for deadlines, TTL, and deterministic tests.
pub trait ClockPort: Send + Sync {
    /// Current Unix time in milliseconds.
    fn now_unix_ms(&self) -> u64;
}

/// Canonical workspace identity resolved by the platform boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedWorkspace {
    /// Canonical absolute root used as the alias identity.
    pub canonical_root: String,
    /// True when the root is inside an explicitly allowed parent.
    pub inside_allowed_root: bool,
}

/// Filesystem-backed path resolution port.
pub trait WorkspaceResolverPort: Send + Sync {
    /// Canonicalizes and validates a requested workspace root.
    fn resolve(&self, requested_root: &str) -> RuntimeResult<ResolvedWorkspace>;
}

/// Runtime-derived authoritative workspace context passed to business adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BusinessWorkspace {
    /// Stable registered identity.
    pub workspace_id: String,
    /// Canonical root established by the path resolver.
    pub canonical_root: String,
    /// Exact registered project identity.
    pub project_key: String,
}

/// Durable runtime metadata and event port.
///
/// Implementations must commit event sequence allocation and record insertion
/// atomically. Records are versioned JSON values; authoritative project state
/// remains outside this metadata store.
pub trait PersistencePort: Send + Sync {
    /// Durable event-store epoch.
    fn event_store_id(&self) -> RuntimeResult<EventStoreId>;
    /// Latest committed global event sequence.
    fn latest_event_sequence(&self) -> RuntimeResult<u64>;
    /// Appends one bounded event and allocates the next global sequence.
    fn append_event(&self, event: DurableEvent) -> RuntimeResult<DurableEvent>;
    /// Atomically appends an event and stores its idempotency receipt.
    ///
    /// Implementations allocate one sequence, apply it to both values, and
    /// commit both records in one transaction.
    fn commit_event_and_receipt(
        &self,
        event: DurableEvent,
        receipt: IdempotencyReceipt,
    ) -> RuntimeResult<(DurableEvent, IdempotencyReceipt)>;
    /// Reads an ordered bounded event page after a cursor.
    fn events_after(&self, after: u64, limit: usize) -> RuntimeResult<Vec<DurableEvent>>;
    /// Oldest available event sequence, or zero for an empty store.
    fn oldest_event_sequence(&self) -> RuntimeResult<u64>;
    /// Reads an idempotency receipt by namespaced key.
    fn load_receipt(&self, scope: &str, key: &str) -> RuntimeResult<Option<IdempotencyReceipt>>;
    /// Atomically stores a receipt, rejecting a conflicting existing payload.
    fn store_receipt(&self, receipt: &IdempotencyReceipt) -> RuntimeResult<()>;
    /// Reads one durable versioned aggregate projection.
    fn load_record(&self, namespace: &str, key: &str) -> RuntimeResult<Option<Value>>;
    /// Atomically upserts one durable versioned aggregate projection.
    fn store_record(&self, namespace: &str, key: &str, value: &Value) -> RuntimeResult<()>;
}

/// Business-operation boundary for authoritative state, Gates, and jobs.
///
/// The runtime never substitutes a local Gate/state implementation when this
/// port rejects or is unavailable.
pub trait BusinessOperationPort: Send + Sync {
    /// Executes a typed post-handshake method at the authoritative boundary.
    fn execute(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
        workspace: Option<&BusinessWorkspace>,
    ) -> RuntimeResult<Value>;
}

/// Fail-closed default for installations missing authoritative business ports.
#[derive(Clone, Copy, Debug, Default)]
pub struct RejectingBusinessPort;

impl BusinessOperationPort for RejectingBusinessPort {
    fn execute(
        &self,
        method: RpcMethod,
        _params: &RequestParams<Value>,
        _workspace: Option<&BusinessWorkspace>,
    ) -> RuntimeResult<Value> {
        let code = if method == RpcMethod::GateEvaluate {
            StableErrorCode::GateError
        } else {
            StableErrorCode::OperationNotRegistered
        };
        Err(RuntimeError::new(
            code,
            "authoritative business operation port is unavailable",
        ))
    }
}

/// Deterministic in-memory persistence used by contract tests.
#[derive(Debug)]
pub struct MemoryPersistence {
    event_store_id: EventStoreId,
    inner: Mutex<MemoryState>,
}

#[derive(Debug, Default)]
struct MemoryState {
    events: Vec<DurableEvent>,
    receipts: BTreeMap<(String, String), IdempotencyReceipt>,
    records: BTreeMap<(String, String), Value>,
}

impl MemoryPersistence {
    /// Creates an empty store with an explicit epoch identity.
    #[must_use]
    pub fn new(event_store_id: EventStoreId) -> Self {
        Self {
            event_store_id,
            inner: Mutex::new(MemoryState::default()),
        }
    }

    fn lock(&self) -> RuntimeResult<std::sync::MutexGuard<'_, MemoryState>> {
        self.inner.lock().map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "runtime metadata lock is poisoned",
            )
        })
    }
}

impl PersistencePort for MemoryPersistence {
    fn event_store_id(&self) -> RuntimeResult<EventStoreId> {
        Ok(self.event_store_id)
    }

    fn latest_event_sequence(&self) -> RuntimeResult<u64> {
        Ok(self
            .lock()?
            .events
            .last()
            .map_or(0, |event| event.event_seq))
    }

    fn append_event(&self, mut event: DurableEvent) -> RuntimeResult<DurableEvent> {
        let mut state = self.lock()?;
        let next = state
            .events
            .last()
            .map_or(1, |previous| previous.event_seq.saturating_add(1));
        if next == 0 {
            return Err(RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "global event sequence overflow",
            ));
        }
        event.event_store_id = self.event_store_id.to_string();
        event.event_seq = next;
        state.events.push(event.clone());
        Ok(event)
    }

    fn commit_event_and_receipt(
        &self,
        mut event: DurableEvent,
        mut receipt: IdempotencyReceipt,
    ) -> RuntimeResult<(DurableEvent, IdempotencyReceipt)> {
        let mut state = self.lock()?;
        let key = (receipt.scope.clone(), receipt.key.clone());
        if let Some(existing) = state.receipts.get(&key) {
            if existing.request_digest != receipt.request_digest {
                return Err(RuntimeError::new(
                    StableErrorCode::IdempotencyKeyReused,
                    "idempotency key was reused with a different payload",
                ));
            }
            let existing_event = state
                .events
                .iter()
                .find(|item| item.event_seq == existing.event_seq)
                .cloned()
                .ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "receipt points to a missing durable event",
                    )
                })?;
            return Ok((existing_event, existing.clone()));
        }
        let next = state
            .events
            .last()
            .map_or(1, |previous| previous.event_seq.saturating_add(1));
        if next == 0 {
            return Err(RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "global event sequence overflow",
            ));
        }
        event.event_store_id = self.event_store_id.to_string();
        event.event_seq = next;
        receipt.event_seq = next;
        state.events.push(event.clone());
        state.receipts.insert(key, receipt.clone());
        Ok((event, receipt))
    }

    fn events_after(&self, after: u64, limit: usize) -> RuntimeResult<Vec<DurableEvent>> {
        Ok(self
            .lock()?
            .events
            .iter()
            .filter(|event| event.event_seq > after)
            .take(limit)
            .cloned()
            .collect())
    }

    fn oldest_event_sequence(&self) -> RuntimeResult<u64> {
        Ok(self
            .lock()?
            .events
            .first()
            .map_or(0, |event| event.event_seq))
    }

    fn load_receipt(&self, scope: &str, key: &str) -> RuntimeResult<Option<IdempotencyReceipt>> {
        Ok(self
            .lock()?
            .receipts
            .get(&(scope.to_owned(), key.to_owned()))
            .cloned())
    }

    fn store_receipt(&self, receipt: &IdempotencyReceipt) -> RuntimeResult<()> {
        let mut state = self.lock()?;
        let key = (receipt.scope.clone(), receipt.key.clone());
        if let Some(existing) = state.receipts.get(&key) {
            if existing.request_digest != receipt.request_digest {
                return Err(RuntimeError::new(
                    StableErrorCode::IdempotencyKeyReused,
                    "idempotency key was reused with a different payload",
                ));
            }
            return Ok(());
        }
        state.receipts.insert(key, receipt.clone());
        Ok(())
    }

    fn load_record(&self, namespace: &str, key: &str) -> RuntimeResult<Option<Value>> {
        Ok(self
            .lock()?
            .records
            .get(&(namespace.to_owned(), key.to_owned()))
            .cloned())
    }

    fn store_record(&self, namespace: &str, key: &str, value: &Value) -> RuntimeResult<()> {
        self.lock()?
            .records
            .insert((namespace.to_owned(), key.to_owned()), value.clone());
        Ok(())
    }
}
