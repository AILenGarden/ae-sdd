use db_operator_native::database::{position_by_column_name, profile_fingerprint, redact_connection_error};
use db_operator_native::registry::WriteLevel;
use db_operator_native::registry::{ConnectionProfile, Endpoint, Engine, Limits, SslProfile};

#[test]
fn schema_snapshot_column_lookup_ignores_the_server_reported_case() {
    // MySQL returns information_schema labels in upper case; a case-sensitive
    // lookup made every schema refresh fail with "no column found for name".
    let mysql_labels = ["TABLE_SCHEMA", "TABLE_NAME", "COLUMN_NAME", "ORDINAL_POSITION"];
    for name in ["table_schema", "table_name", "column_name", "ordinal_position"] {
        assert_eq!(
            position_by_column_name(mysql_labels, name),
            mysql_labels.iter().position(|label| label.eq_ignore_ascii_case(name)),
            "lookup of {name} must ignore case"
        );
    }
    assert_eq!(position_by_column_name(mysql_labels, "table_schema"), Some(0));
    assert_eq!(position_by_column_name(mysql_labels, "missing_column"), None);
    let postgres_labels = ["table_schema", "table_name"];
    assert_eq!(position_by_column_name(postgres_labels, "TABLE_SCHEMA"), Some(0));
}

#[test]
fn schema_snapshot_reads_columns_the_way_mysql_reports_them() {
    // MySQL declares information_schema text columns as BLOB-family types and counters
    // as UNSIGNED. Reading them as String or i64 silently yielded no value at all, so
    // the cached schema lost data_type and ordinal_position while still "refreshing".
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("database.rs"),
    )
    .unwrap();
    let mysql_snapshot = source
        .split("async fn mysql_schema_snapshot")
        .nth(1)
        .and_then(|rest| rest.split("async fn postgres_schema_snapshot").next())
        .expect("mysql snapshot");
    for required in [
        "named_text(&row, \"table_schema\")",
        "named_text(&row, \"column_type\")",
        "named_text(&row, \"column_default\")",
        "mysql_named_unsigned(&row, \"ordinal_position\")",
    ] {
        assert!(
            mysql_snapshot.contains(required),
            "MySQL snapshot must read {required}"
        );
    }
    assert!(
        !mysql_snapshot.contains("named_string("),
        "the String-based reader cannot decode MySQL information_schema columns"
    );
    // The text reader must go through raw bytes and decode UTF-8 itself.
    assert!(source.contains("fn named_text"), "text reader missing");
    assert!(
        source.contains("try_get::<Option<&[u8]>, _>"),
        "text reader must accept blob bytes"
    );
}

#[test]
fn profile_fingerprint_changes_when_connection_metadata_changes() {
    let mut profile = ConnectionProfile {
        engine: Engine::Mysql,
        endpoint: Endpoint {
            mode: "tcp".to_owned(),
            host: "db-a.internal".to_owned(),
            port: 3306,
        },
        database: Some("analytics".to_owned()),
        username: "reader".to_owned(),
        secret_ref: "vault://db-operator/warehouse".to_owned(),
        ssl: SslProfile::default(),
        environment: "prod".to_owned(),
        tags: vec![],
        limits: Limits::default(),
        write_level: WriteLevel::None,
    };
    let first = profile_fingerprint(&profile).unwrap();
    profile.endpoint.host = "db-b.internal".to_owned();
    let second = profile_fingerprint(&profile).unwrap();

    assert_ne!(first, second);
    assert_eq!(first.len(), 64);
}

#[test]
fn database_errors_hide_all_connection_metadata() {
    let profile = ConnectionProfile {
        engine: Engine::Mysql,
        endpoint: Endpoint {
            mode: "tcp".to_owned(),
            host: "db.internal.example".to_owned(),
            port: 3306,
        },
        database: Some("analytics".to_owned()),
        username: "reader_account".to_owned(),
        secret_ref: "vault://db-operator/warehouse".to_owned(),
        ssl: SslProfile {
            mode: "verify-full".to_owned(),
            ca_file: Some("C:/private/ca.pem".to_owned()),
            client_cert: None,
            client_key: Some("C:/private/client.key".to_owned()),
        },
        environment: "prod".to_owned(),
        tags: vec![],
        limits: Limits::default(),
        write_level: WriteLevel::None,
    };
    let redacted = redact_connection_error(
        "mysql://reader_account:top-secret@db.internal.example:3306/analytics C:/private/client.key",
        &profile,
        "top-secret",
    );

    for forbidden in [
        "reader_account",
        "top-secret",
        "db.internal.example",
        "3306",
        "C:/private/client.key",
    ] {
        assert!(!redacted.contains(forbidden), "leaked {forbidden}");
    }
}

#[test]
fn database_errors_keep_unrelated_words_readable_for_short_identifiers() {
    let profile = ConnectionProfile {
        engine: Engine::Mysql,
        endpoint: Endpoint {
            mode: "tcp".to_owned(),
            host: "dbserver".to_owned(),
            port: 3306,
        },
        database: None,
        username: "u".to_owned(),
        secret_ref: "vault://db-operator/u".to_owned(),
        ssl: SslProfile::default(),
        environment: "prod".to_owned(),
        tags: vec![],
        limits: Limits::default(),
        write_level: WriteLevel::None,
    };
    let redacted = redact_connection_error(
        "connection failed: pool timed out while waiting for an open connection",
        &profile,
        "pw",
    );

    assert!(
        redacted.contains("timed out"),
        "short user name corrupted unrelated words: {redacted}"
    );
    assert!(!redacted.contains(":3306"), "leaked port in address form");
    assert!(redacted.contains("connection"));
}
