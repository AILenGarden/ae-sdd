use std::{path::Path, str::FromStr};

use ae_sdd_domain::EventStoreId;
use ae_sdd_store::{
    SQLITE_RUNTIME_MIGRATIONS, SqliteRuntimeRepository, StoreError, UtcTimestamp,
    latest_runtime_schema_version,
};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use uuid::Uuid;

fn now() -> UtcTimestamp {
    UtcTimestamp::from_str("2026-07-26T00:00:00Z").expect("timestamp")
}

fn event_store_id(value: u128) -> EventStoreId {
    EventStoreId::from_uuid(Uuid::from_u128(value))
}

fn assert_catalog(connection: &Connection) {
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user version");
    assert_eq!(version, latest_runtime_schema_version());
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM schema_migration", [], |row| {
            row.get(0)
        })
        .expect("migration count");
    assert_eq!(
        usize::try_from(count).expect("catalog count fits"),
        SQLITE_RUNTIME_MIGRATIONS.len(),
        "catalog rows and the published migration list must agree"
    );
    let violations: String = connection
        .query_row(
            "SELECT COALESCE(group_concat(\"table\"), '') FROM pragma_foreign_key_check",
            [],
            |row| row.get(0),
        )
        .expect("foreign key check");
    assert!(
        violations.is_empty(),
        "foreign key violations: {violations}"
    );
    for migration in SQLITE_RUNTIME_MIGRATIONS {
        let (name, checksum): (String, String) = connection
            .query_row(
                "SELECT name,checksum FROM schema_migration WHERE version=?1",
                [migration.version],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("catalog row");
        assert_eq!(name, migration.name);
        assert_eq!(
            checksum,
            hex::encode(Sha256::digest(migration.sql.as_bytes()))
        );
    }
}

fn apply_prefix(path: &Path, count: usize) {
    let mut connection = Connection::open(path).expect("fixture database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("FK on");
    let transaction = connection.transaction().expect("fixture transaction");
    for migration in &SQLITE_RUNTIME_MIGRATIONS[..count] {
        transaction
            .execute_batch(migration.sql)
            .expect("prefix migration");
        transaction
            .execute(
                "INSERT INTO schema_migration(version,name,checksum,applied_at) VALUES(?1,?2,?3,?4)",
                params![
                    migration.version,
                    migration.name,
                    hex::encode(Sha256::digest(migration.sql.as_bytes())),
                    now().to_string(),
                ],
            )
            .expect("catalog prefix");
    }
    transaction.commit().expect("fixture commit");
}

#[test]
fn empty_and_version_eight_databases_reach_exact_0010_catalog() {
    for prefix in [0, 8] {
        let root = TempDir::new().expect("temp root");
        let path = root.path().join(format!("runtime-{prefix}.sqlite3"));
        if prefix > 0 {
            apply_prefix(&path, prefix);
        }
        drop(
            SqliteRuntimeRepository::open(&path, event_store_id(prefix as u128 + 1), &now())
                .expect("migration succeeds"),
        );
        let connection = Connection::open(&path).expect("inspect database");
        assert_catalog(&connection);
        drop(connection);
        drop(
            SqliteRuntimeRepository::open(&path, event_store_id(999), &now())
                .expect("reopen is repeatable"),
        );
    }
}

#[test]
fn corrupt_recorded_checksum_fails_closed() {
    let root = TempDir::new().expect("temp root");
    let path = root.path().join("runtime.sqlite3");
    apply_prefix(&path, 8);
    let connection = Connection::open(&path).expect("inspect database");
    connection
        .execute(
            "UPDATE schema_migration SET checksum=?1 WHERE version=7",
            ["0".repeat(64)],
        )
        .expect("corrupt checksum");
    drop(connection);

    let error = SqliteRuntimeRepository::open(&path, event_store_id(1), &now())
        .expect_err("checksum mismatch must fail");
    assert!(matches!(error, StoreError::DatabaseIncompatible { .. }));
}
