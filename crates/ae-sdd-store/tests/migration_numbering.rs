//! D-02 item 5: the migration numbering contract.
//!
//! The runtime already refuses a migration whose SQL stamps the wrong
//! `user_version` (`sqlite.rs` compares the observed pragma against
//! `RuntimeMigration::version` after each batch). That check fires only when a
//! real database is migrated, so a numbering mistake surfaces at startup on a
//! developer or production machine rather than in CI.
//!
//! These assertions move the same failures earlier and add the two the runtime
//! cannot make: that a registry entry's `version` agrees with the number in its
//! own `name`, and that no migration file exists on disk without being
//! registered. An unregistered file is the quiet one — it never applies, so the
//! schema it defines is simply absent and every error appears downstream.

use ae_sdd_store::{SQLITE_RUNTIME_MIGRATIONS, latest_runtime_schema_version};

/// A registry entry's `version` and the number embedded in its `name` are two
/// independent declarations of the same fact, so they can disagree.
#[test]
fn every_registry_version_matches_the_number_in_its_own_name() {
    for migration in SQLITE_RUNTIME_MIGRATIONS {
        let prefix = migration
            .name
            .split('_')
            .next()
            .expect("a migration name has a numeric prefix");
        let parsed: i64 = prefix
            .parse()
            .unwrap_or_else(|_| panic!("{} has a non-numeric prefix", migration.name));
        assert_eq!(
            parsed, migration.version,
            "{} declares version {} but is named for {parsed}",
            migration.name, migration.version
        );
        assert_eq!(
            prefix.len(),
            4,
            "{} must keep the zero-padded four-digit prefix so lexical and \
             numeric order agree",
            migration.name
        );
    }
}

/// Versions must be contiguous from 1 with no gaps or repeats.
///
/// `sqlite.rs` applies every migration whose `version > current_version` in
/// registry order, so a gap would leave `user_version` unable to reach the head
/// value, and a duplicate would make the applied set depend on iteration order.
#[test]
fn migration_versions_are_contiguous_from_one() {
    let versions: Vec<i64> = SQLITE_RUNTIME_MIGRATIONS
        .iter()
        .map(|migration| migration.version)
        .collect();

    for (index, version) in versions.iter().enumerate() {
        let expected = i64::try_from(index + 1).expect("index fits");
        assert_eq!(
            *version, expected,
            "registry position {index} holds version {version}; numbering must \
             be contiguous from 1 and in ascending order"
        );
    }

    assert_eq!(
        versions.last().copied(),
        Some(latest_runtime_schema_version()),
        "the head version must be the last registry entry, since \
         latest_runtime_schema_version reads exactly that"
    );
}

/// Every `.sql` file in `migrations/` must be registered exactly once.
///
/// A file present on disk but absent from the registry is never executed, so its
/// tables never exist. Nothing else in the suite would notice: the count check in
/// `migration_catalog_0011` compares the catalog against the registry, and both
/// would agree while the file sat unused.
#[test]
fn every_migration_file_on_disk_is_registered_exactly_once() {
    let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
    let mut on_disk: Vec<String> = std::fs::read_dir(directory)
        .expect("migrations directory")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".sql"))
        .map(|name| name.trim_end_matches(".sql").to_owned())
        .collect();
    on_disk.sort();

    let mut registered: Vec<String> = SQLITE_RUNTIME_MIGRATIONS
        .iter()
        .map(|migration| migration.name.to_owned())
        .collect();
    registered.sort();

    assert_eq!(
        on_disk, registered,
        "every migration file must be registered exactly once; a file only on \
         disk never runs, and a registry entry with no file cannot compile"
    );
}
