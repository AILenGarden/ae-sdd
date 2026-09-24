use std::path::Path;

#[test]
fn no_connection_path_inspects_database_account_grants() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for file in ["main.rs", "database.rs", "daemon.rs"] {
        let source = std::fs::read_to_string(root.join(file)).unwrap();
        for forbidden in [
            "connect_verified",
            "verify_read_only_privileges",
            "SHOW GRANTS",
            "has_table_privilege",
            "has_sequence_privilege",
            "has_schema_privilege",
            "has_database_privilege",
            "has_function_privilege",
            "pg_auth_members",
        ] {
            assert!(
                !source.contains(forbidden),
                "{file} still gates connections on {forbidden}"
            );
        }
    }
}

#[tokio::test]
async fn disallowed_sql_is_rejected_before_credentials_or_connection() {
    use db_operator_native::{
        daemon::DaemonRuntime,
        protocol::{Operation, PROTOCOL_VERSION, Request},
        registry::WriteLevel,
        security::CapabilityVerifier,
    };
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("connections.json"),
        r#"{
        "version": 3, "default": "configured",
        "connections": {"configured": {
            "engine":"mysql", "endpoint":{"mode":"tcp","host":"127.0.0.1","port":1},
            "username":"account-with-extra-grants", "secret_ref":"vault://db-operator/configured",
            "write_level":"none"
        }}
    }"#,
    )
    .unwrap();
    let runtime = DaemonRuntime::new(
        directory.path().join("schema-cache.sqlite3"),
        std::time::Duration::from_secs(60),
        CapabilityVerifier::from_token_for("test-token", "configured"),
    )
    .unwrap();
    for sql in [
        "UPDATE users SET name = 'blocked'",
        "CREATE TABLE blocked (id INT)",
        "GRANT SELECT ON *.* TO 'other'",
    ] {
        let response = runtime
            .process(Request {
                version: PROTOCOL_VERSION,
                request_id: "policy-test".into(),
                capability: "test-token".into(),
                operation: Operation::Query {
                    connection: "configured".into(),
                    sql: sql.into(),
                    write_level: WriteLevel::None,
                },
            })
            .await;
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "SQL_POLICY_REJECTED");
    }
}
