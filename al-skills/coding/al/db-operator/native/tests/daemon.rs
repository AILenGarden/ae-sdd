use std::time::Duration;

use db_operator_native::daemon::DaemonRuntime;
use db_operator_native::protocol::{Operation, Request};
use db_operator_native::registry::WriteLevel;
use db_operator_native::security::CapabilityVerifier;

#[tokio::test]
async fn status_reports_warm_runtime_without_open_database_pools() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = DaemonRuntime::new(
        directory.path().join("schema-cache.sqlite3"),
        Duration::from_secs(60),
        CapabilityVerifier::from_token("test-capability"),
    )
    .unwrap();
    let response = runtime
        .process(Request {
            version: db_operator_native::protocol::PROTOCOL_VERSION,
            request_id: "status-1".to_owned(),
            capability: "test-capability".to_owned(),
            operation: Operation::Status,
        })
        .await;

    assert!(response.ok);
    assert_eq!(response.runtime.unwrap().state, "warm");
    let result = response.result.unwrap();
    assert_eq!(result["pool_count"], 0);
    assert!(result.get("pid").is_none());
    assert!(result.get("cache_path").is_none());
}

#[tokio::test]
async fn invalid_capability_is_rejected_before_dispatch() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = DaemonRuntime::new(
        directory.path().join("schema-cache.sqlite3"),
        Duration::from_secs(60),
        CapabilityVerifier::from_token("correct-capability"),
    )
    .unwrap();
    let response = runtime
        .process(Request {
            version: db_operator_native::protocol::PROTOCOL_VERSION,
            request_id: "status-2".to_owned(),
            capability: "wrong-capability".to_owned(),
            operation: Operation::Status,
        })
        .await;

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "AUTHENTICATION_FAILED");
}

#[tokio::test]
async fn scoped_capability_rejects_another_connection_before_registry_access() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = DaemonRuntime::new(
        directory.path().join("schema-cache.sqlite3"),
        Duration::from_secs(60),
        CapabilityVerifier::from_token_for("correct-capability", "reporting-prod"),
    )
    .unwrap();
    let response = runtime
        .process(Request {
            version: db_operator_native::protocol::PROTOCOL_VERSION,
            request_id: "query-other".to_owned(),
            capability: "correct-capability".to_owned(),
            operation: Operation::Query {
                connection: "finance-prod".to_owned(),
                sql: "SELECT 1".to_owned(),
                write_level: WriteLevel::None,
            },
        })
        .await;

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "AUTHORIZATION_FAILED");
}

#[tokio::test]
async fn default_capability_rejects_write_request_before_database_access() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("connections.json"),
        r#"{
          "version": 3,
          "default": "reporting-prod",
          "connections": {
            "reporting-prod": {
              "engine": "postgresql",
              "endpoint": {"mode": "tcp", "host": "db.internal", "port": 5432},
              "database": "reporting",
              "username": "report_reader",
              "secret_ref": "vault://db-operator/reporting-prod",
              "read_only": true,
              "write_level": "ddl"
            }
          }
        }"#,
    )
    .unwrap();
    let runtime = DaemonRuntime::new(
        directory.path().join("schema-cache.sqlite3"),
        Duration::from_secs(60),
        CapabilityVerifier::from_token_for("read-only-token", "reporting-prod"),
    )
    .unwrap();
    let response = runtime
        .process(Request {
            version: db_operator_native::protocol::PROTOCOL_VERSION,
            request_id: "write-with-read-capability".to_owned(),
            capability: "read-only-token".to_owned(),
            operation: Operation::Query {
                connection: "reporting-prod".to_owned(),
                sql: "UPDATE users SET name = 'blocked'".to_owned(),
                write_level: WriteLevel::Dml,
            },
        })
        .await;

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "AUTHORIZATION_FAILED");
}

#[tokio::test]
async fn scoped_status_reports_only_alias_and_engine_metadata() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("connections.json"),
        r#"{
          "version": 3,
          "default": "reporting-prod",
          "connections": {
            "reporting-prod": {
              "engine": "postgresql",
              "endpoint": {"mode": "tcp", "host": "db.internal", "port": 5432},
              "database": "reporting",
              "username": "report_reader",
              "secret_ref": "vault://db-operator/reporting-prod",
              "ssl": {"mode": "require", "ca_file": null, "client_cert": null, "client_key": null},
              "environment": "test",
              "tags": [],
              "limits": {"connect_timeout_seconds": 10, "statement_timeout_seconds": 30, "max_rows": 500},
              "read_only": true
            }
          }
        }"#,
    )
    .unwrap();
    let runtime = DaemonRuntime::new(
        directory.path().join("schema-cache.sqlite3"),
        Duration::from_secs(60),
        CapabilityVerifier::from_token_for("correct-capability", "reporting-prod"),
    )
    .unwrap();
    let response = runtime
        .process(Request {
            version: db_operator_native::protocol::PROTOCOL_VERSION,
            request_id: "status-scoped".to_owned(),
            capability: "correct-capability".to_owned(),
            operation: Operation::Status,
        })
        .await;

    assert!(response.ok);
    let result = response.result.unwrap();
    assert_eq!(result["connection"], "reporting-prod");
    assert_eq!(result["engine"], "postgresql");
    for forbidden in ["host", "port", "username", "password", "database", "url"] {
        assert!(result.get(forbidden).is_none(), "status leaked {forbidden}");
    }
}
