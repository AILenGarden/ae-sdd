//! Daemon-wide execution resource arbitration: one fair, TTL-bounded lease
//! for Cargo invocations across all sessions of a daemon boot.
//!
//! Several agents racing `cargo` on one machine queue on the target dir and
//! destabilize every deadline.  The arbiter serializes them: the first
//! session is [`ResourceDecision::Allow`]ed, concurrent requesters are
//! [`ResourceDecision::Defer`]red with a bounded retry hint and queued FIFO,
//! and a lease expires after its TTL so a forgotten lease never deadlocks the
//! daemon.  While held, the lease is backed by an OS-exclusive lock on an
//! explicit lock file under the per-user runtime state dir, so a daemon crash
//! releases the lock for the next boot.
//!
//! Hard rules (implementation plan Task 10): the lock path is always an
//! explicit, fully resolved path injected by the daemon composition root —
//! never a workspace root and never an unresolved environment variable; a
//! lock file that cannot be opened or is held by another process defers
//! fail-closed instead of parallelizing Cargo.
//!
//! The module is deliberately free of crate-internal imports so the
//! integration tests can include it verbatim and drive it directly.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

/// Resources serialized by the daemon-wide arbiter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceKind {
    /// The Cargo toolchain (build/test/clippy invocations).
    Cargo,
}

/// Arbitration outcome for one acquire attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceDecision {
    /// The lease is granted to the requesting session.
    Allow,
    /// The lease is held elsewhere; retry after the bounded hint.
    Defer {
        /// Bounded retry hint in milliseconds.
        retry_after_ms: u64,
    },
}

/// Atomic lease effect returned with one arbitration decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceAcquisition {
    /// The caller already held a live lease and merely re-entered it.
    Reentered,
    /// This acquire attempt installed a new lease, including a same-session
    /// regrant after TTL expiry.
    Granted,
    /// No lease was installed for the caller.
    Deferred,
}

/// One decision plus the lease effect observed under the arbiter mutex.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceAcquireOutcome {
    /// Allow/defer decision exposed to the Hook policy.
    pub decision: ResourceDecision,
    /// Whether this exact attempt installed the lease.
    pub acquisition: ResourceAcquisition,
}

/// One lease acquire attempt.
#[derive(Clone, Copy, Debug)]
pub struct CargoAcquireRequest<'a> {
    /// Authenticated session requesting the lease.
    pub session_id: &'a str,
    /// Explicit, fully resolved lock file under the per-user runtime state
    /// dir; `None` degrades to in-process arbitration only.
    pub lock_path: Option<&'a Path>,
    /// Current time in Unix milliseconds (injectable clock).
    pub now_unix_ms: u64,
    /// Lease time-to-live in milliseconds; a lease at or beyond its TTL is
    /// released for the next waiter.
    pub ttl_ms: u64,
    /// Bounded retry hint returned with a deferral.
    pub retry_after_ms: u64,
    /// Maximum queued waiters; beyond it a requester defers unqueued.
    pub queue_capacity: usize,
}

/// RAII guard holding the OS-exclusive lock on the Cargo lock file.
///
/// Dropping the guard closes the descriptor, which releases the OS lock even
/// when the explicit unlock fails — that is the crash-release property the
/// next daemon boot relies on.
#[derive(Debug)]
struct StdFileLockGuard {
    file: File,
}

impl StdFileLockGuard {
    /// Opens the explicit lock file and takes the OS-exclusive lock without
    /// blocking.  Returns `Ok(None)` when the lock is contended or the file
    /// cannot be opened (fail-closed: the caller defers, never parallelizes).
    ///
    /// The parent directory is intentionally not created: the path must point
    /// inside the existing per-user runtime state dir, and a missing parent
    /// means a misconfigured path the daemon must not silently materialize.
    fn try_acquire(path: &Path) -> Result<Option<Self>, std::io::Error> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { file })),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(error)) => Err(error),
        }
    }
}

impl Drop for StdFileLockGuard {
    fn drop(&mut self) {
        // Best-effort explicit unlock; closing the descriptor on drop releases
        // the OS lock regardless, so a failure here needs no further action.
        let _released = self.file.unlock();
    }
}

/// Active lease holder.
#[derive(Debug)]
struct LeaseHolder {
    session_id: Box<str>,
    acquired_at_unix_ms: u64,
    _guard: Option<StdFileLockGuard>,
}

#[derive(Debug, Default)]
struct CargoLeaseState {
    holder: Option<LeaseHolder>,
    queue: VecDeque<Box<str>>,
}

/// Fair, TTL-bounded daemon-wide Cargo lease.
///
/// Lock discipline: one leaf mutex, never held across I/O beyond the bounded
/// lock-file open, and recovered rather than panicked on poisoning because
/// the lease state is rebuildable (the OS lock is the cross-process truth).
#[derive(Debug, Default)]
pub struct CargoResourceArbiter {
    state: Mutex<CargoLeaseState>,
}

impl CargoResourceArbiter {
    /// Creates an idle arbiter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attempts to take the lease for one session.
    ///
    /// Semantics:
    ///
    /// - the current holder re-entering within its TTL is allowed without
    ///   refreshing the lease clock;
    /// - a lease at or beyond its TTL is released first (crash/forgotten
    ///   lease recovery);
    /// - a free lease goes only to the queue front (or to anyone when the
    ///   queue is empty), keeping waiters FIFO;
    /// - everyone else defers with the bounded retry hint and is queued once,
    ///   bounded by `queue_capacity`.
    #[allow(dead_code)]
    pub fn acquire(
        &self,
        kind: ResourceKind,
        request: &CargoAcquireRequest<'_>,
    ) -> ResourceDecision {
        self.acquire_with_effect(kind, request).decision
    }

    /// Attempts to take the lease and atomically reports whether this call
    /// installed it. Callers that compensate failed transactions must use
    /// this method instead of inspecting the holder before acquisition.
    pub fn acquire_with_effect(
        &self,
        kind: ResourceKind,
        request: &CargoAcquireRequest<'_>,
    ) -> ResourceAcquireOutcome {
        match kind {
            ResourceKind::Cargo => self.acquire_cargo(request),
        }
    }

    /// Releases the lease held by the session, if any, and removes it from
    /// the waiter queue.  Releasing a session that holds nothing is a no-op.
    pub fn release(&self, kind: ResourceKind, session_id: &str) {
        match kind {
            ResourceKind::Cargo => {
                let mut state = self.lock();
                if state
                    .holder
                    .as_ref()
                    .is_some_and(|holder| &*holder.session_id == session_id)
                {
                    // Dropping the holder drops the OS lock guard.
                    state.holder = None;
                }
                state.queue.retain(|queued| &**queued != session_id);
            }
        }
    }

    /// Returns the session currently holding the lease, for diagnostics.
    // Exercised by the integration tests that include this module verbatim.
    #[allow(dead_code)]
    pub fn holder_session(&self) -> Option<Box<str>> {
        self.lock()
            .holder
            .as_ref()
            .map(|holder| holder.session_id.clone())
    }

    /// Returns the session's FIFO position (0 = next), or `None` when the
    /// session is not queued.
    // Exercised by the integration tests that include this module verbatim.
    #[allow(dead_code)]
    pub fn queue_position(&self, session_id: &str) -> Option<usize> {
        self.lock()
            .queue
            .iter()
            .position(|queued| &**queued == session_id)
    }

    fn acquire_cargo(&self, request: &CargoAcquireRequest<'_>) -> ResourceAcquireOutcome {
        let mut state = self.lock();
        if state.holder.as_ref().is_some_and(|holder| {
            request
                .now_unix_ms
                .saturating_sub(holder.acquired_at_unix_ms)
                >= request.ttl_ms
        }) {
            // The holder crashed, hung or forgot its PostTool release: the
            // lease expires and the OS lock guard drops here.
            state.holder = None;
        }
        if let Some(holder) = &state.holder {
            if &*holder.session_id == request.session_id {
                return ResourceAcquireOutcome {
                    decision: ResourceDecision::Allow,
                    acquisition: ResourceAcquisition::Reentered,
                };
            }
            return Self::queue_and_defer(&mut state, request);
        }
        let may_take = state
            .queue
            .front()
            .is_none_or(|front| &**front == request.session_id);
        if !may_take {
            return Self::queue_and_defer(&mut state, request);
        }
        let guard = match request.lock_path {
            None => None,
            Some(path) => match StdFileLockGuard::try_acquire(path) {
                Ok(Some(guard)) => Some(guard),
                Ok(None) | Err(_) => {
                    // OS-level contention or an unusable lock path: stay
                    // queued and defer fail-closed, never parallelize Cargo.
                    return Self::queue_and_defer(&mut state, request);
                }
            },
        };
        if state
            .queue
            .front()
            .is_some_and(|front| &**front == request.session_id)
        {
            state.queue.pop_front();
        }
        state.holder = Some(LeaseHolder {
            session_id: request.session_id.into(),
            acquired_at_unix_ms: request.now_unix_ms,
            _guard: guard,
        });
        ResourceAcquireOutcome {
            decision: ResourceDecision::Allow,
            acquisition: ResourceAcquisition::Granted,
        }
    }

    fn queue_and_defer(
        state: &mut CargoLeaseState,
        request: &CargoAcquireRequest<'_>,
    ) -> ResourceAcquireOutcome {
        if !state
            .queue
            .iter()
            .any(|queued| &**queued == request.session_id)
            && state.queue.len() < request.queue_capacity
        {
            state.queue.push_back(request.session_id.into());
        }
        ResourceAcquireOutcome {
            decision: ResourceDecision::Defer {
                retry_after_ms: request.retry_after_ms,
            },
            acquisition: ResourceAcquisition::Deferred,
        }
    }

    fn lock(&self) -> MutexGuard<'_, CargoLeaseState> {
        // A poisoned lease mutex still holds rebuildable state; the OS lock
        // remains the cross-process truth.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
