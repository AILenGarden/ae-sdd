use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonState {
    Warm,
    Hot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleAction {
    None,
    ClosePools,
}

#[derive(Debug, Clone)]
pub struct Lifecycle {
    state: DaemonState,
    last_activity: Duration,
    pool_idle_timeout: Duration,
}

impl Lifecycle {
    pub fn new(pool_idle_timeout: Duration) -> Self {
        Self {
            state: DaemonState::Warm,
            last_activity: Duration::ZERO,
            pool_idle_timeout,
        }
    }

    pub fn state(&self) -> DaemonState {
        self.state
    }

    pub fn record_activity(&mut self, now: Duration) {
        self.last_activity = now;
    }

    pub fn record_pool_opened(&mut self, now: Duration) {
        self.record_activity(now);
        self.state = DaemonState::Hot;
    }

    pub fn idle_for(&self, now: Duration) -> Duration {
        now.saturating_sub(self.last_activity)
    }

    pub fn tick(&mut self, now: Duration) -> LifecycleAction {
        let idle = self.idle_for(now);
        if self.state == DaemonState::Hot && idle >= self.pool_idle_timeout {
            self.state = DaemonState::Warm;
            return LifecycleAction::ClosePools;
        }
        LifecycleAction::None
    }
}
