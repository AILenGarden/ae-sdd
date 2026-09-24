use db_operator_native::routing::{DaemonAttempt, RoutingError, execute_daemon_only};

#[tokio::test]
async fn unavailable_daemon_fails_closed() {
    let result =
        execute_daemon_only(|| async { DaemonAttempt::<String, String>::Unavailable }).await;

    assert_eq!(result, Err(RoutingError::DaemonUnavailable));
}

#[tokio::test]
async fn fatal_daemon_error_is_preserved() {
    let result = execute_daemon_only(|| async {
        DaemonAttempt::<String, String>::Fatal("protocol mismatch".to_owned())
    })
    .await;

    assert_eq!(
        result,
        Err(RoutingError::Fatal("protocol mismatch".to_owned()))
    );
}
