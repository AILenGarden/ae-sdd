use std::time::Duration;

use db_operator_native::audit::{AuditEvent, AuditLogger};

#[test]
fn audit_log_is_async_and_contains_no_sql_or_credentials() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("audit.jsonl");
    let logger = AuditLogger::start(path.clone()).unwrap();

    logger.record(AuditEvent {
        request_id: "request-1".to_owned(),
        operation: "query".to_owned(),
        connection: Some("reporting-prod".to_owned()),
        outcome: "QUERY_OK".to_owned(),
        elapsed_micros: 250,
    });
    drop(logger);

    for _ in 0..20 {
        if path.exists() && std::fs::metadata(&path).unwrap().len() > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("request-1"));
    assert!(content.contains("reporting-prod"));
    for forbidden in ["SELECT", "password", "username", "hostname"] {
        assert!(!content.contains(forbidden));
    }
}
