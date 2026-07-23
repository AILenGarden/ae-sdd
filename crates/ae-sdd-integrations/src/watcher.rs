use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::{IntegrationError, IntegrationResult};

/// Reason an inventory consumer must discard deltas and run a full reconcile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FullReconcileReason {
    /// Backend reported a watcher error or OS event gap.
    BackendError,
    /// The bounded callback channel could not retain every delta.
    Overflow,
}

/// Watcher output. It is inventory invalidation input, never business truth.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WatchSignal {
    /// One bounded group of paths may need inventory refresh.
    Changed(Vec<PathBuf>),
    /// Deltas are incomplete and the inventory must be fully reconciled.
    FullReconcile(FullReconcileReason),
}

/// Capacity-bounded notify adapter with fail-safe gap semantics.
pub struct BoundedWorkspaceWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<WatchSignal>,
    overflowed: Arc<AtomicBool>,
}

impl BoundedWorkspaceWatcher {
    /// Starts recursively watching a workspace root.
    pub fn start(root: &Path, capacity: usize) -> IntegrationResult<Self> {
        let (sender, receiver) = sync_channel(capacity.max(1));
        let overflowed = Arc::new(AtomicBool::new(false));
        let callback_overflow = Arc::clone(&overflowed);
        let mut watcher = notify::recommended_watcher(move |event| {
            publish_watch_event(&sender, &callback_overflow, event);
        })
        .map_err(notify_error)?;
        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(notify_error)?;
        Ok(Self {
            _watcher: watcher,
            receiver,
            overflowed,
        })
    }

    /// Receives the next invalidation signal within a bounded wait.
    pub fn receive_timeout(&self, timeout: Duration) -> IntegrationResult<Option<WatchSignal>> {
        if self.overflowed.swap(false, Ordering::AcqRel) {
            drain(&self.receiver);
            return Ok(Some(WatchSignal::FullReconcile(
                FullReconcileReason::Overflow,
            )));
        }
        match self.receiver.recv_timeout(timeout) {
            Ok(signal) => Ok(Some(signal)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err(IntegrationError::Io(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "workspace watcher callback disconnected",
                )))
            }
        }
    }
}

fn publish_watch_event(
    sender: &SyncSender<WatchSignal>,
    overflowed: &AtomicBool,
    event: notify::Result<Event>,
) {
    let signal = match event {
        Ok(event) => WatchSignal::Changed(event.paths),
        Err(_) => WatchSignal::FullReconcile(FullReconcileReason::BackendError),
    };
    if sender.try_send(signal).is_err() {
        overflowed.store(true, Ordering::Release);
    }
}

fn drain(receiver: &Receiver<WatchSignal>) {
    loop {
        match receiver.try_recv() {
            Ok(_) => {}
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
        }
    }
}

fn notify_error(error: notify::Error) -> IntegrationError {
    IntegrationError::Io(std::io::Error::other(error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_callback_overflow_forces_full_reconcile() {
        let (sender, receiver) = sync_channel(1);
        let overflowed = AtomicBool::new(false);
        publish_watch_event(&sender, &overflowed, Ok(Event::new(notify::EventKind::Any)));
        publish_watch_event(&sender, &overflowed, Ok(Event::new(notify::EventKind::Any)));
        assert!(overflowed.load(Ordering::Acquire));
        drain(&receiver);
    }
}
