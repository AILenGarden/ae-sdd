use std::{collections::BTreeSet, path::Path, str::FromStr};

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
    UtcTimestamp::from_str("2026-07-27T00:00:00Z").expect("timestamp")
}

fn event_store_id(value: u128) -> EventStoreId {
    EventStoreId::from_uuid(Uuid::from_u128(value))
}

fn assert_catalog(connection: &Connection) {
    // Expectations derive from the published catalog, so adding a migration needs
    // no edit here. The comparison still bites: the database's user_version comes
    // from executing each migration's own `PRAGMA user_version=N`, which is a
    // declaration independent of the RuntimeMigration.version this reads.
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
        SQLITE_RUNTIME_MIGRATIONS.len()
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
    assert_eq!(versions.len(), SQLITE_RUNTIME_MIGRATIONS.len());
    let latest = SQLITE_RUNTIME_MIGRATIONS
        .last()
        .expect("migration catalog is non-empty");
    assert_eq!(latest.version, latest_runtime_schema_version());
}

/// Structural invariants that keep the derived expectations honest: without these
/// a degenerate catalog (gaps, reordering, a name that disagrees with its version)
/// would satisfy every `len()`-based assertion tautologically.
#[test]
fn migration_catalog_is_contiguous_from_one_and_self_describing() {
    assert!(
        !SQLITE_RUNTIME_MIGRATIONS.is_empty(),
        "catalog must not be empty"
    );
    for (index, migration) in SQLITE_RUNTIME_MIGRATIONS.iter().enumerate() {
        let expected_version = i64::try_from(index + 1).expect("index fits");
        assert_eq!(
            migration.version, expected_version,
            "migration '{}' breaks the contiguous 1..=n ordering",
            migration.name
        );
        let prefix = format!("{expected_version:04}_");
        assert!(
            migration.name.starts_with(&prefix),
            "migration name '{}' must start with '{prefix}' to match its version",
            migration.name
        );
        // 0001..0008 wrote `= N`, 0009 onward `=N`; both files are checksum-frozen,
        // so accept either spelling rather than rewriting sealed SQL.
        let sets_version = [
            format!("PRAGMA user_version={expected_version};"),
            format!("PRAGMA user_version = {expected_version};"),
        ]
        .iter()
        .any(|needle| migration.sql.contains(needle.as_str()));
        assert!(
            sets_version,
            "migration '{}' must set user_version to {expected_version}",
            migration.name
        );
    }
    assert_eq!(
        latest_runtime_schema_version(),
        i64::try_from(SQLITE_RUNTIME_MIGRATIONS.len()).expect("catalog length fits"),
        "head version must equal the catalog length under contiguous numbering"
    );
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

/// A migrated database must carry the one-clean-batch constraint in its live
/// schema, including when it arrives by upgrade from an older version that
/// still allowed two.
#[test]
fn migrated_review_projections_only_allow_clean_target_one() {
    for prefix in [0, 11] {
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
        for table in [
            "review_session_v2_projection",
            "review_exit_receipt_v2_projection",
        ] {
            let ddl: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("live table DDL");
            assert!(
                ddl.contains("CHECK(clean_target=1)"),
                "{table} must pin clean_target to 1 (prefix {prefix})"
            );
            assert!(
                !ddl.contains("clean_target=2"),
                "{table} must not retain a two-batch branch (prefix {prefix})"
            );
        }
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
