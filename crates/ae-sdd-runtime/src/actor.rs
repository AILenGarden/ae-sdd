use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ae_sdd_protocol::StableErrorCode;

use crate::{RuntimeError, RuntimeResult};

/// Bounded per-Work-Item serialization registry.
#[derive(Debug)]
pub struct WorkItemActors {
    mailbox_capacity: usize,
    maximum_actors: usize,
    maximum_per_workspace: usize,
    idle_ttl: Duration,
    actors: Mutex<BTreeMap<(String, String), Arc<ActorSlot>>>,
}

#[derive(Debug)]
struct ActorSlot {
    admitted: AtomicUsize,
    execution: Mutex<()>,
    last_used: Mutex<Instant>,
}

struct Admission<'a> {
    admitted: &'a AtomicUsize,
}

impl Drop for Admission<'_> {
    fn drop(&mut self) {
        self.admitted.fetch_sub(1, Ordering::AcqRel);
    }
}

impl WorkItemActors {
    /// Creates a registry with an explicit mailbox bound per actor.
    #[must_use]
    pub fn new(
        mailbox_capacity: usize,
        maximum_actors: usize,
        maximum_per_workspace: usize,
        idle_ttl_ms: u64,
    ) -> Self {
        Self {
            mailbox_capacity: mailbox_capacity.max(1),
            maximum_actors: maximum_actors.max(1),
            maximum_per_workspace: maximum_per_workspace.max(1),
            idle_ttl: Duration::from_millis(idle_ttl_ms.max(1)),
            actors: Mutex::new(BTreeMap::new()),
        }
    }

    /// Executes one call serially for a Work Item, respecting admission and deadline bounds.
    pub fn execute<T>(
        &self,
        workspace_id: &str,
        work_item_id: &str,
        deadline_ms: u64,
        operation: impl FnOnce() -> RuntimeResult<T>,
    ) -> RuntimeResult<T> {
        let actor = {
            let mut actors = self.actors.lock().map_err(|_| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "Work Item actor registry is poisoned",
                )
            })?;
            self.evict_idle(&mut actors);
            let key = (workspace_id.to_owned(), work_item_id.to_owned());
            if !actors.contains_key(&key) {
                let workspace_count = actors
                    .keys()
                    .filter(|(workspace, _)| workspace == workspace_id)
                    .count();
                if actors.len() >= self.maximum_actors
                    || workspace_count >= self.maximum_per_workspace
                {
                    return Err(RuntimeError::new(
                        StableErrorCode::SubscriberBackpressure,
                        "Work Item actor registry capacity is exhausted",
                    ));
                }
            }
            Arc::clone(actors.entry(key).or_insert_with(|| {
                Arc::new(ActorSlot {
                    admitted: AtomicUsize::new(0),
                    execution: Mutex::new(()),
                    last_used: Mutex::new(Instant::now()),
                })
            }))
        };

        let previous = actor.admitted.fetch_add(1, Ordering::AcqRel);
        if previous >= self.mailbox_capacity {
            actor.admitted.fetch_sub(1, Ordering::AcqRel);
            return Err(RuntimeError::new(
                StableErrorCode::SubscriberBackpressure,
                "Work Item mailbox capacity is exhausted",
            ));
        }
        let _admission = Admission {
            admitted: &actor.admitted,
        };

        let started = Instant::now();
        let maximum = Duration::from_millis(deadline_ms.max(1));
        loop {
            match actor.execution.try_lock() {
                Ok(_guard) => {
                    let result = operation();
                    actor.touch();
                    return result;
                }
                Err(std::sync::TryLockError::WouldBlock) if started.elapsed() < maximum => {
                    std::thread::park_timeout(Duration::from_micros(100));
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    return Err(RuntimeError::new(
                        StableErrorCode::GateTimeout,
                        "Work Item actor deadline expired in the bounded mailbox",
                    ));
                }
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    return Err(RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "Work Item actor is poisoned",
                    ));
                }
            }
        }
    }

    fn evict_idle(&self, actors: &mut BTreeMap<(String, String), Arc<ActorSlot>>) {
        actors.retain(|_, actor| {
            actor.admitted.load(Ordering::Acquire) != 0
                || Arc::strong_count(actor) != 1
                || actor
                    .last_used
                    .lock()
                    .map_or(true, |last_used| last_used.elapsed() < self.idle_ttl)
        });
    }
}

impl ActorSlot {
    fn touch(&self) {
        if let Ok(mut last_used) = self.last_used.lock() {
            *last_used = Instant::now();
        }
    }
}
