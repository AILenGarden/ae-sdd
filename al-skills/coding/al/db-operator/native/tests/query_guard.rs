use std::time::{Duration, Instant};

use db_operator_native::query_guard::{GateError, QueryGate, with_wall_clock_timeout};

#[test]
fn query_gate_limits_concurrency_and_rate() {
    let gate = QueryGate::new(1, 2, Duration::from_secs(60));
    let now = Instant::now();

    let first = gate.try_enter(now).unwrap();
    assert_eq!(
        gate.try_enter(now).unwrap_err(),
        GateError::ConcurrencyLimited
    );
    drop(first);

    let second = gate.try_enter(now).unwrap();
    drop(second);
    assert_eq!(gate.try_enter(now).unwrap_err(), GateError::RateLimited);
    assert!(gate.try_enter(now + Duration::from_secs(61)).is_ok());
}

#[tokio::test]
async fn wall_clock_timeout_cancels_slow_work() {
    let result = with_wall_clock_timeout(Duration::from_millis(10), async {
        tokio::time::sleep(Duration::from_secs(1)).await;
        42
    })
    .await;

    assert_eq!(result, Err(GateError::TimedOut));
}
