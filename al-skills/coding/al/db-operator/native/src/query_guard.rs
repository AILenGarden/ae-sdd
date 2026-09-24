use std::collections::VecDeque;
use std::future::Future;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateError {
    ConcurrencyLimited,
    RateLimited,
    TimedOut,
}

#[derive(Debug)]
pub struct QueryGate {
    active: AtomicUsize,
    max_concurrent: usize,
    max_per_window: usize,
    window: Duration,
    accepted: Mutex<VecDeque<Instant>>,
}

impl QueryGate {
    pub fn new(max_concurrent: usize, max_per_window: usize, window: Duration) -> Self {
        Self {
            active: AtomicUsize::new(0),
            max_concurrent: max_concurrent.max(1),
            max_per_window: max_per_window.max(1),
            window,
            accepted: Mutex::new(VecDeque::new()),
        }
    }

    pub fn try_enter(&self, now: Instant) -> Result<QueryPermit<'_>, GateError> {
        self.active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < self.max_concurrent).then_some(active + 1)
            })
            .map_err(|_| GateError::ConcurrencyLimited)?;

        let accepted = self.accept_rate(now);
        if !accepted {
            self.active.fetch_sub(1, Ordering::AcqRel);
            return Err(GateError::RateLimited);
        }
        Ok(QueryPermit { gate: self })
    }

    fn accept_rate(&self, now: Instant) -> bool {
        let mut accepted = self.accepted.lock().expect("query rate mutex poisoned");
        while accepted
            .front()
            .is_some_and(|oldest| now.saturating_duration_since(*oldest) >= self.window)
        {
            accepted.pop_front();
        }
        if accepted.len() >= self.max_per_window {
            return false;
        }
        accepted.push_back(now);
        true
    }
}

#[derive(Debug)]
pub struct QueryPermit<'a> {
    gate: &'a QueryGate,
}

impl Drop for QueryPermit<'_> {
    fn drop(&mut self) {
        self.gate.active.fetch_sub(1, Ordering::AcqRel);
    }
}

pub async fn with_wall_clock_timeout<T, F>(timeout: Duration, future: F) -> Result<T, GateError>
where
    F: Future<Output = T>,
{
    tokio::time::timeout(timeout, future)
        .await
        .map_err(|_| GateError::TimedOut)
}
