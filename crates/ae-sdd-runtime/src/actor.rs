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
    actors: Mutex<BTreeMap<(String, String), Arc<ActorSlot>>>,
}

#[derive(Debug)]
struct ActorSlot {
    admitted: AtomicUsize,
    execution: Mutex<()>,
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
    pub fn new(mailbox_capacity: usize) -> Self {
        Self {
            mailbox_capacity: mailbox_capacity.max(1),
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
            Arc::clone(
                actors
                    .entry((workspace_id.to_owned(), work_item_id.to_owned()))
                    .or_insert_with(|| {
                        Arc::new(ActorSlot {
                            admitted: AtomicUsize::new(0),
                            execution: Mutex::new(()),
                        })
                    }),
            )
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
                Ok(_guard) => return operation(),
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
}
