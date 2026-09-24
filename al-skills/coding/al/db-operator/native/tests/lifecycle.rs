use std::time::Duration;

use db_operator_native::lifecycle::{DaemonState, Lifecycle, LifecycleAction};

#[test]
fn lifecycle_releases_pools_and_remains_warm() {
    let mut lifecycle = Lifecycle::new(Duration::from_secs(60));

    assert_eq!(lifecycle.state(), DaemonState::Warm);
    lifecycle.record_pool_opened(Duration::ZERO);
    assert_eq!(lifecycle.state(), DaemonState::Hot);

    assert_eq!(
        lifecycle.tick(Duration::from_secs(59)),
        LifecycleAction::None
    );
    assert_eq!(
        lifecycle.tick(Duration::from_secs(60)),
        LifecycleAction::ClosePools
    );
    assert_eq!(lifecycle.state(), DaemonState::Warm);

    lifecycle.record_activity(Duration::from_secs(120));
    assert_eq!(
        lifecycle.tick(Duration::from_secs(86_520)),
        LifecycleAction::None
    );
    assert_eq!(lifecycle.state(), DaemonState::Warm);
}

#[test]
fn lifecycle_reports_idle_time_from_last_activity() {
    let mut lifecycle = Lifecycle::new(Duration::from_secs(60));
    lifecycle.record_activity(Duration::from_secs(120));

    assert_eq!(
        lifecycle.idle_for(Duration::from_secs(180)),
        Duration::from_secs(60)
    );
}
