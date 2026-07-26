use std::{collections::BTreeSet, path::Path, str::FromStr};

use ae_sdd_domain::EventStoreId;
use ae_sdd_store::{SQLITE_RUNTIME_MIGRATIONS, SqliteRuntimeRepository, StoreError, UtcTimestamp};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use uuid::Uuid;

fn now() -> UtcTimestamp {
    UtcTimestamp::from_str("2026-07-27T00:00:00Z").expect("timestamp")
}

fn event_store_id(value: u128) -> EventStoreId {
    EventStoreId::from_uuid(Uuid::from_u128(value))
}

fn assert_catalog(connection: &Connection) {
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user version");
    assert_eq!(version, 11);
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM schema_migration", [], |row| {
            row.get(0)
        })
        .expect("migration count");
    assert_eq!(count, 11);
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
fn migration_versions_and_names_are_unique() {
    let mut versions = BTreeSet::new();
    let mut names = BTreeSet::new();
    for migration in SQLITE_RUNTIME_MIGRATIONS {
        assert!(
            versions.insert(migration.version),
            "duplicate migration version {}",
            migration.version
        );
        assert!(
            names.insert(migration.name),
            "duplicate migration name {}",
            migration.name
        );
    }
    assert_eq!(versions.len(), 11);
    let latest = SQLITE_RUNTIME_MIGRATIONS
        .last()
        .expect("migration catalog is non-empty");
    assert_eq!(latest.version, 11);
    assert_eq!(latest.name, "0011_execution_supervisor_v1");
}

#[test]
fn empty_and_version_ten_databases_reach_exact_0011_catalog() {
    for prefix in [0, 10] {
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
    apply_prefix(&path, 10);
    let connection = Connection::open(&path).expect("inspect database");
    connection
        .execute(
            "UPDATE schema_migration SET checksum=?1 WHERE version=9",
            ["0".repeat(64)],
        )
        .expect("corrupt checksum");
    drop(connection);

    let error = SqliteRuntimeRepository::open(&path, event_store_id(1), &now())
        .expect_err("checksum mismatch must fail");
    assert!(matches!(error, StoreError::DatabaseIncompatible { .. }));
}

#[test]
fn execution_supervisor_checkpoint_table_enforces_rebuildable_shape() {
    let root = TempDir::new().expect("temp root");
    let path = root.path().join("runtime.sqlite3");
    drop(
        SqliteRuntimeRepository::open(&path, event_store_id(1), &now())
            .expect("migration succeeds"),
    );
    let connection = Connection::open(&path).expect("inspect database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("FK on");
    connection
        .execute(
            "INSERT INTO workspace(workspace_id,canonical_root,project_key,mode,inventory_generation,dirty,created_at,updated_at) VALUES('ws-1','/root','ae-sdd','shadow',0,0,'2026-07-27T00:00:00Z','2026-07-27T00:00:00Z')",
            [],
        )
        .expect("workspace parent row");
    let capsule_digest = "a".repeat(64);
    let queue_digest = "b".repeat(64);
    let insert = "INSERT INTO execution_supervisor_checkpoint_v1(workspace_id,work_item_id,session_id,capsule_digest,queue_digest,active_ordinal,no_progress_batches,source_cache_hits,source_cache_misses,updated_event_seq,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'2026-07-27T00:00:00Z')";
    connection
        .execute(
            insert,
            params![
                "ws-1",
                "WI-1",
                "session-1",
                capsule_digest,
                queue_digest,
                2,
                1,
                7,
                3,
                42
            ],
        )
        .expect("valid rebuildable checkpoint row");
    let duplicate = connection.execute(
        insert,
        params![
            "ws-1",
            "WI-1",
            "session-1",
            capsule_digest,
            queue_digest,
            2,
            1,
            7,
            3,
            43
        ],
    );
    assert!(
        duplicate.is_err(),
        "workspace/work item/session primary key must reject duplicates"
    );
    let malformed_digest = connection.execute(
        insert,
        params![
            "ws-1",
            "WI-2",
            "session-2",
            "not-a-digest",
            queue_digest,
            0,
            0,
            0,
            0,
            0
        ],
    );
    assert!(
        malformed_digest.is_err(),
        "digest CHECK must reject malformed capsule digests"
    );
    let negative_counter = connection.execute(
        insert,
        params![
            "ws-1",
            "WI-3",
            "session-3",
            capsule_digest,
            queue_digest,
            0,
            -1,
            0,
            0,
            0
        ],
    );
    assert!(
        negative_counter.is_err(),
        "counter CHECK must reject a negative no-progress counter"
    );
    let missing_parent = connection.execute(
        insert,
        params![
            "ws-missing",
            "WI-4",
            "session-4",
            capsule_digest,
            queue_digest,
            0,
            0,
            0,
            0,
            0
        ],
    );
    assert!(
        missing_parent.is_err(),
        "workspace foreign key must reject orphan checkpoints"
    );
}
