use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Condvar, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use ae_sdd_domain::{
    CancellationCode, ErrorCode, GateCancellation, GateError, GateId, GateKey, GateKeyDigest,
    GateOutcome, GateResult, GateTimeout,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{GateExecutor, GateInputError, GateRegistry};

const CANCELLATION_POLL: Duration = Duration::from_millis(5);
const DEFAULT_CACHE_CAPACITY: usize = 4_096;

#[derive(Clone, Debug)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    reason: CancellationCode,
}

impl CancellationToken {
    pub fn new(reason: CancellationCode) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            reason,
        }
    }

    pub fn caller() -> Self {
        Self::new(
            CancellationCode::new("CALLER_CANCELLED").expect("constant cancellation code is valid"),
        )
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn outcome(&self) -> GateOutcome {
        GateOutcome::Cancelled(GateCancellation::new(self.reason.clone()))
    }
}

#[derive(Clone, Debug)]
pub struct GateRunRequest {
    pub key: GateKey,
    pub deadline: Duration,
    pub cancellation: CancellationToken,
}

impl GateRunRequest {
    pub fn new(
        key: GateKey,
        deadline: Duration,
        cancellation: CancellationToken,
    ) -> Result<Self, GateSchedulerError> {
        if deadline.is_zero() {
            return Err(GateSchedulerError::ZeroDeadline);
        }
        Ok(Self {
            key,
            deadline,
            cancellation,
        })
    }
}

pub trait GateFreshnessSource: Send + Sync + 'static {
    fn current_key(&self, snapshot: &GateKey) -> Result<GateKey, GateInputError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EchoFreshness;

impl GateFreshnessSource for EchoFreshness {
    fn current_key(&self, snapshot: &GateKey) -> Result<GateKey, GateInputError> {
        Ok(snapshot.clone())
    }
}

/// Point-in-time scheduler counters. `gates_evaluated` counts real executor
/// invocations: a long-lived scheduler must keep it flat while Gate keys stay
/// unchanged, which is the observable proof that fresh outcomes are reused.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GateSchedulerStats {
    /// Runs answered from the completed-outcome cache.
    pub cache_hits: u64,
    /// Runs that had to start or join an in-flight evaluation.
    pub cache_misses: u64,
    /// Times the executor was actually invoked (cache hits, unknown Gates and
    /// pre-cancelled runs excluded).
    pub gates_evaluated: u64,
}

pub struct GateScheduler<E: GateExecutor, F: GateFreshnessSource> {
    inner: Arc<SchedulerInner<E, F>>,
}

impl<E: GateExecutor, F: GateFreshnessSource> Clone for GateScheduler<E, F> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

struct SchedulerInner<E: GateExecutor, F: GateFreshnessSource> {
    executor: E,
    freshness: F,
    cache: Mutex<GateCache>,
    flights: Mutex<BTreeMap<GateKeyDigest, Arc<Flight>>>,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    gates_evaluated: AtomicU64,
}

struct CacheEntry {
    gate_id: GateId,
    outcome: GateOutcome,
}

struct GateCache {
    capacity: NonZeroUsize,
    entries: BTreeMap<GateKeyDigest, CacheEntry>,
    order: VecDeque<GateKeyDigest>,
}

impl GateCache {
    fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity,
            entries: BTreeMap::new(),
            order: VecDeque::new(),
        }
    }

    fn get(&mut self, key: GateKeyDigest) -> Option<GateOutcome> {
        let outcome = self.entries.get(&key)?.outcome.clone();
        self.promote(key);
        Some(outcome)
    }

    fn insert(&mut self, key: GateKeyDigest, gate_id: GateId, outcome: GateOutcome) {
        self.entries.insert(key, CacheEntry { gate_id, outcome });
        self.promote(key);
        while self.entries.len() > self.capacity.get() {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }

    fn remove(&mut self, key: &GateKeyDigest) -> bool {
        let removed = self.entries.remove(key).is_some();
        if removed {
            self.order.retain(|candidate| candidate != key);
        }
        removed
    }

    fn remove_gates(&mut self, gate_ids: &BTreeSet<&str>) -> usize {
        let stale: Vec<GateKeyDigest> = self
            .entries
            .iter()
            .filter(|(_, entry)| gate_ids.contains(entry.gate_id.as_str()))
            .map(|(key, _)| *key)
            .collect();
        let removed = stale.len();
        for key in &stale {
            self.entries.remove(key);
        }
        if removed > 0 {
            self.order.retain(|candidate| !stale.contains(candidate));
        }
        removed
    }

    fn clear(&mut self) -> usize {
        let count = self.entries.len();
        self.entries.clear();
        self.order.clear();
        count
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn promote(&mut self, key: GateKeyDigest) {
        self.order.retain(|candidate| *candidate != key);
        self.order.push_back(key);
    }
}

struct Flight {
    state: Mutex<Option<GateOutcome>>,
    completed: Condvar,
}

impl Flight {
    fn new() -> Self {
        Self {
            state: Mutex::new(None),
            completed: Condvar::new(),
        }
    }
}

impl<E: GateExecutor, F: GateFreshnessSource> GateScheduler<E, F> {
    pub fn new(executor: E, freshness: F) -> Self {
        Self::with_cache_capacity(
            executor,
            freshness,
            NonZeroUsize::new(DEFAULT_CACHE_CAPACITY)
                .expect("default Gate cache capacity is non-zero"),
        )
    }

    pub fn with_cache_capacity(executor: E, freshness: F, cache_capacity: NonZeroUsize) -> Self {
        Self {
            inner: Arc::new(SchedulerInner {
                executor,
                freshness,
                cache: Mutex::new(GateCache::new(cache_capacity)),
                flights: Mutex::new(BTreeMap::new()),
                cache_hits: AtomicU64::new(0),
                cache_misses: AtomicU64::new(0),
                gates_evaluated: AtomicU64::new(0),
            }),
        }
    }

    pub fn run(&self, request: GateRunRequest) -> GateResult {
        if request.cancellation.is_cancelled() {
            return self.normalize(request.key, request.cancellation.outcome());
        }
        let digest = canonical_gate_key_digest(&request.key);
        if let Some(outcome) = lock(&self.inner.cache).get(digest) {
            self.inner.cache_hits.fetch_add(1, Ordering::Relaxed);
            return self.normalize(request.key, outcome);
        }
        self.inner.cache_misses.fetch_add(1, Ordering::Relaxed);

        let (flight, leader) = {
            let mut flights = lock(&self.inner.flights);
            if let Some(existing) = flights.get(&digest) {
                (Arc::clone(existing), false)
            } else {
                let flight = Arc::new(Flight::new());
                flights.insert(digest, Arc::clone(&flight));
                (flight, true)
            }
        };
        if leader {
            self.spawn(
                digest,
                Arc::clone(&flight),
                request.key.clone(),
                request.deadline,
                request.cancellation.clone(),
            );
        }
        let outcome = wait_for_flight(&flight, request.deadline, &request.cancellation);
        self.normalize(request.key, outcome)
    }

    pub fn clear_cache(&self) -> usize {
        lock(&self.inner.cache).clear()
    }

    pub fn invalidate(&self, keys: impl IntoIterator<Item = GateKeyDigest>) -> usize {
        let mut cache = lock(&self.inner.cache);
        keys.into_iter().filter(|key| cache.remove(key)).count()
    }

    /// Drops every cached outcome belonging to one of `gate_ids`, regardless
    /// of the key digest it was recorded under. Used by incremental
    /// selector-based invalidation.
    pub fn invalidate_gates<'a>(&self, gate_ids: impl IntoIterator<Item = &'a str>) -> usize {
        let gate_ids: BTreeSet<&str> = gate_ids.into_iter().collect();
        if gate_ids.is_empty() {
            return 0;
        }
        lock(&self.inner.cache).remove_gates(&gate_ids)
    }

    pub fn cache_len(&self) -> usize {
        lock(&self.inner.cache).len()
    }

    pub fn inflight_len(&self) -> usize {
        lock(&self.inner.flights).len()
    }

    /// Snapshot of the cumulative scheduler counters.
    pub fn stats(&self) -> GateSchedulerStats {
        GateSchedulerStats {
            cache_hits: self.inner.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.inner.cache_misses.load(Ordering::Relaxed),
            gates_evaluated: self.inner.gates_evaluated.load(Ordering::Relaxed),
        }
    }

    fn spawn(
        &self,
        digest: GateKeyDigest,
        flight: Arc<Flight>,
        key: GateKey,
        deadline: Duration,
        cancellation: CancellationToken,
    ) {
        let inner = Arc::clone(&self.inner);
        thread::spawn(move || {
            let started = Instant::now();
            let outcome = if cancellation.is_cancelled() {
                cancellation.outcome()
            } else if let Some(specification) = GateRegistry::get(key.gate_id().as_str()) {
                inner.gates_evaluated.fetch_add(1, Ordering::Relaxed);
                match catch_unwind(AssertUnwindSafe(|| {
                    inner.executor.evaluate(specification, &key, &cancellation)
                })) {
                    Ok(outcome) => outcome,
                    Err(_) => GateOutcome::Error(GateError::new(
                        ErrorCode::new("GATE_EXECUTOR_PANIC")
                            .expect("constant error code is valid"),
                        false,
                    )),
                }
            } else {
                GateOutcome::Error(GateError::new(
                    ErrorCode::new("UNKNOWN_GATE").expect("constant error code is valid"),
                    false,
                ))
            };
            let outcome = if cancellation.is_cancelled() {
                cancellation.outcome()
            } else if started.elapsed() >= deadline {
                timeout_outcome(deadline)
            } else {
                outcome
            };

            let cacheable = match &outcome {
                GateOutcome::Pass | GateOutcome::Fail(_) => true,
                GateOutcome::Error(error) => !error.retryable(),
                GateOutcome::Timeout(_) | GateOutcome::Cancelled(_) | GateOutcome::Stale(_) => {
                    false
                }
            };
            if cacheable {
                lock(&inner.cache).insert(digest, key.gate_id().clone(), outcome.clone());
            }
            *lock(&flight.state) = Some(outcome);
            flight.completed.notify_all();
            lock(&inner.flights).remove(&digest);
        });
    }

    fn normalize(&self, snapshot: GateKey, outcome: GateOutcome) -> GateResult {
        let current = match self.inner.freshness.current_key(&snapshot) {
            Ok(current) => current,
            Err(error) => {
                return GateResult::new(
                    snapshot,
                    GateOutcome::Error(GateError::new(error.code().clone(), error.retryable())),
                );
            }
        };
        let candidate = GateResult::new(snapshot.clone(), outcome);
        GateResult::new(snapshot, candidate.outcome_against(&current))
    }
}

fn wait_for_flight(
    flight: &Flight,
    deadline: Duration,
    cancellation: &CancellationToken,
) -> GateOutcome {
    let started = Instant::now();
    let mut state = lock(&flight.state);
    loop {
        if let Some(outcome) = state.clone() {
            return outcome;
        }
        if cancellation.is_cancelled() {
            return cancellation.outcome();
        }
        let elapsed = started.elapsed();
        if elapsed >= deadline {
            return timeout_outcome(deadline);
        }
        let remaining = deadline.saturating_sub(elapsed).min(CANCELLATION_POLL);
        state = match flight.completed.wait_timeout(state, remaining) {
            Ok((state, _)) => state,
            Err(poisoned) => poisoned.into_inner().0,
        };
    }
}

fn timeout_outcome(deadline: Duration) -> GateOutcome {
    let deadline_ms = u64::try_from(deadline.as_millis())
        .unwrap_or(u64::MAX)
        .max(1);
    GateOutcome::Timeout(GateTimeout::new(deadline_ms).expect("deadline is non-zero"))
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn canonical_gate_key_digest(key: &GateKey) -> GateKeyDigest {
    let mut hasher = Sha256::new();
    hasher.update(b"ae-sdd-gate-key/v1\0");
    hash_bytes(&mut hasher, key.gate_id().as_str().as_bytes());
    hasher.update(key.gate_implementation().as_bytes());
    hasher.update(key.policy().as_bytes());
    hasher.update(key.workspace_id().as_uuid().as_bytes());
    hash_bytes(&mut hasher, key.work_item_id().as_str().as_bytes());
    match key.story_id() {
        Some(story) => {
            hasher.update([1]);
            hash_bytes(&mut hasher, story.as_str().as_bytes());
        }
        None => hasher.update([0]),
    }
    hasher.update(key.state_revision().get().to_be_bytes());
    hasher.update(key.fencing_token().get().to_be_bytes());
    hasher.update(key.inventory_generation().get().to_be_bytes());
    hasher.update(key.toolchain().as_bytes());
    hasher.update(key.configuration().as_bytes());
    hasher.update(key.input().as_bytes());
    GateKeyDigest::from_array(hasher.finalize().into())
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum GateSchedulerError {
    #[error("Gate deadline must be greater than zero")]
    ZeroDeadline,
}

#[cfg(test)]
pub(crate) mod tests_support {
    use ae_sdd_domain::{
        ConfigDigest, FencingToken, GateId, GateImplementationDigest, InputFingerprint,
        InventoryGeneration, PolicyDigest, StateRevision, StoryId, ToolchainDigest, WorkItemId,
        WorkspaceId,
    };
    use uuid::Uuid;

    use super::GateKey;

    pub fn gate_key(id: &str, revision: u64) -> GateKey {
        GateKey::new(
            GateId::new(id).expect("valid gate ID"),
            GateImplementationDigest::digest(b"implementation-v1"),
            PolicyDigest::digest(b"policy-v1"),
            WorkspaceId::from_uuid(Uuid::from_u128(1)),
            WorkItemId::new("PRD-AE-SDD-RUST-DAEMON-001").expect("valid work item"),
            Some(StoryId::new("STORY-AE-SDD-RUST-DAEMON-001").expect("valid story")),
            StateRevision::new(revision),
            FencingToken::new(8),
            InventoryGeneration::new(3),
            ToolchainDigest::digest(b"rustc-1.97.1"),
            ConfigDigest::digest(b"config-v1"),
            InputFingerprint::digest(b"inputs-v1"),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Barrier,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use crate::GateSpec;

    struct CountingExecutor {
        calls: AtomicUsize,
        delay: Duration,
        outcome: GateOutcome,
    }

    impl GateExecutor for CountingExecutor {
        fn evaluate(
            &self,
            _specification: &'static GateSpec,
            _key: &GateKey,
            _cancellation: &CancellationToken,
        ) -> GateOutcome {
            self.calls.fetch_add(1, Ordering::SeqCst);
            thread::sleep(self.delay);
            self.outcome.clone()
        }
    }

    #[test]
    fn identical_concurrent_jobs_are_singleflight() {
        let scheduler = GateScheduler::new(
            CountingExecutor {
                calls: AtomicUsize::new(0),
                delay: Duration::from_millis(25),
                outcome: GateOutcome::Pass,
            },
            EchoFreshness,
        );
        let key = tests_support::gate_key("G-14", 1);
        let first = scheduler.clone();
        let second = scheduler.clone();
        let first_key = key.clone();
        let first_thread = thread::spawn(move || {
            first.run(
                GateRunRequest::new(
                    first_key,
                    Duration::from_millis(250),
                    CancellationToken::caller(),
                )
                .expect("valid request"),
            )
        });
        let second_thread = thread::spawn(move || {
            second.run(
                GateRunRequest::new(key, Duration::from_millis(250), CancellationToken::caller())
                    .expect("valid request"),
            )
        });

        assert!(matches!(
            first_thread.join().expect("first thread").outcome(),
            GateOutcome::Pass
        ));
        assert!(matches!(
            second_thread.join().expect("second thread").outcome(),
            GateOutcome::Pass
        ));
        assert_eq!(scheduler.inner.executor.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn deadlines_and_cancellation_remain_distinct() {
        let scheduler = GateScheduler::new(
            CountingExecutor {
                calls: AtomicUsize::new(0),
                delay: Duration::from_millis(50),
                outcome: GateOutcome::Pass,
            },
            EchoFreshness,
        );
        let timeout = scheduler.run(
            GateRunRequest::new(
                tests_support::gate_key("G-14", 1),
                Duration::from_millis(5),
                CancellationToken::caller(),
            )
            .expect("valid request"),
        );
        assert!(matches!(timeout.outcome(), GateOutcome::Timeout(_)));

        let cancellation = CancellationToken::caller();
        cancellation.cancel();
        let cancelled = scheduler.run(
            GateRunRequest::new(
                tests_support::gate_key("G-13", 1),
                Duration::from_millis(100),
                cancellation,
            )
            .expect("valid request"),
        );
        assert!(matches!(cancelled.outcome(), GateOutcome::Cancelled(_)));
    }

    #[test]
    fn completed_gate_cache_is_capacity_bounded() {
        let scheduler = GateScheduler::with_cache_capacity(
            CountingExecutor {
                calls: AtomicUsize::new(0),
                delay: Duration::ZERO,
                outcome: GateOutcome::Pass,
            },
            EchoFreshness,
            NonZeroUsize::new(2).expect("non-zero capacity"),
        );
        for revision in 1..=3 {
            let result = scheduler.run(
                GateRunRequest::new(
                    tests_support::gate_key("G-14", revision),
                    Duration::from_millis(250),
                    CancellationToken::caller(),
                )
                .expect("valid request"),
            );
            assert!(matches!(result.outcome(), GateOutcome::Pass));
        }

        assert_eq!(scheduler.cache_len(), 2);
    }

    #[test]
    fn deterministic_errors_are_cached_but_retryable_errors_are_not() {
        for (retryable, expected_evaluations) in [(false, 1), (true, 2)] {
            let scheduler = GateScheduler::new(
                CountingExecutor {
                    calls: AtomicUsize::new(0),
                    delay: Duration::ZERO,
                    outcome: GateOutcome::Error(GateError::new(
                        ErrorCode::new("SCANNER_SCOPE_EMPTY").expect("valid error code"),
                        retryable,
                    )),
                },
                EchoFreshness,
            );
            let key = tests_support::gate_key("G-14", 1);

            for _ in 0..2 {
                let result = scheduler.run(
                    GateRunRequest::new(
                        key.clone(),
                        Duration::from_millis(250),
                        CancellationToken::caller(),
                    )
                    .expect("valid request"),
                );
                assert!(matches!(result.outcome(), GateOutcome::Error(_)));
            }

            assert_eq!(scheduler.stats().gates_evaluated, expected_evaluations);
        }
    }

    struct CooperativelyCancelledExecutor {
        started: Arc<Barrier>,
    }

    impl GateExecutor for CooperativelyCancelledExecutor {
        fn evaluate(
            &self,
            _specification: &'static GateSpec,
            _key: &GateKey,
            cancellation: &CancellationToken,
        ) -> GateOutcome {
            self.started.wait();
            while !cancellation.is_cancelled() {
                thread::yield_now();
            }
            cancellation.outcome()
        }
    }

    #[test]
    fn in_flight_cancellation_reaches_executor_and_is_not_cached() {
        let started = Arc::new(Barrier::new(2));
        let scheduler = GateScheduler::new(
            CooperativelyCancelledExecutor {
                started: Arc::clone(&started),
            },
            EchoFreshness,
        );
        let cancellation = CancellationToken::caller();
        let caller_token = cancellation.clone();
        let caller = scheduler.clone();
        let thread = thread::spawn(move || {
            caller.run(
                GateRunRequest::new(
                    tests_support::gate_key("G-14", 1),
                    Duration::from_secs(1),
                    caller_token,
                )
                .expect("valid request"),
            )
        });

        started.wait();
        cancellation.cancel();
        let result = thread.join().expect("caller thread");
        assert!(matches!(result.outcome(), GateOutcome::Cancelled(_)));
        assert_eq!(scheduler.cache_len(), 0);
    }

    #[test]
    fn invalidate_gates_drops_only_matching_gate_entries() {
        let scheduler = GateScheduler::new(
            CountingExecutor {
                calls: AtomicUsize::new(0),
                delay: Duration::ZERO,
                outcome: GateOutcome::Pass,
            },
            EchoFreshness,
        );
        for gate in ["G-13", "G-14"] {
            let result = scheduler.run(
                GateRunRequest::new(
                    tests_support::gate_key(gate, 1),
                    Duration::from_millis(250),
                    CancellationToken::caller(),
                )
                .expect("valid request"),
            );
            assert!(matches!(result.outcome(), GateOutcome::Pass));
        }
        assert_eq!(scheduler.cache_len(), 2);
        assert_eq!(scheduler.stats().gates_evaluated, 2);
        assert_eq!(scheduler.invalidate_gates(["G-99"]), 0);

        assert_eq!(scheduler.invalidate_gates(["G-14"]), 1);
        assert_eq!(scheduler.cache_len(), 1);

        for gate in ["G-13", "G-14"] {
            let result = scheduler.run(
                GateRunRequest::new(
                    tests_support::gate_key(gate, 1),
                    Duration::from_millis(250),
                    CancellationToken::caller(),
                )
                .expect("valid request"),
            );
            assert!(matches!(result.outcome(), GateOutcome::Pass));
        }

        let stats = scheduler.stats();
        assert_eq!(
            stats.gates_evaluated, 3,
            "only the invalidated Gate re-evaluates"
        );
        assert_eq!(stats.cache_hits, 1, "the untouched Gate stays cached");
        assert_eq!(stats.cache_misses, 3);
    }
}
