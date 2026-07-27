//! Migration 0007 contract test: review_runtime SQL aligns with frozen DTO.

use std::collections::BTreeSet;

const MIGRATION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../migrations/0007_review_runtime.sql"
));

fn table_names(sql: &str) -> BTreeSet<String> {
    let lower = sql.to_ascii_lowercase();
    lower
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("create table if not exists ")
                .or_else(|| trimmed.strip_prefix("create table "))
        })
        .map(|rest| {
            rest.split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches('(')
                .to_owned()
        })
        .filter(|name| !name.is_empty())
        .collect()
}

#[test]
fn migration_0007_defines_expected_review_tables() {
    let tables = table_names(MIGRATION);
    for expected in &[
        "review_session_projection",
        "review_round_projection",
        "review_finding_projection",
        "review_exit_receipt_projection",
    ] {
        assert!(
            tables.contains(*expected),
            "migration 0007 must define table '{expected}'"
        );
    }
}

#[test]
fn migration_0007_sets_user_version_to_7() {
    assert!(
        MIGRATION.contains("PRAGMA user_version = 7"),
        "migration 0007 must set user_version to 7"
    );
}

#[test]
fn migration_0007_finding_projection_has_dedup_unique_constraint() {
    assert!(
        MIGRATION.contains("UNIQUE(workspace_id, finding_fingerprint)"),
        "review_finding_projection must enforce dedup via finding_fingerprint unique constraint"
    );
}

#[test]
fn migration_0007_exit_receipt_has_digest_unique_constraint() {
    assert!(
        MIGRATION.contains("UNIQUE(workspace_id, review_id, receipt_digest)"),
        "review_exit_receipt_projection must enforce receipt_digest uniqueness"
    );
}

#[test]
fn migration_0007_session_fingerprint_length_is_64() {
    assert!(
        MIGRATION.contains("length(input_fingerprint) = 64"),
        "review_session_projection must enforce 64-char fingerprint length"
    );
}

#[test]
fn migration_0007_references_runtime_event() {
    assert!(
        MIGRATION.contains("REFERENCES runtime_event(event_seq)"),
        "review projections must reference runtime_event for durable replay"
    );
}

const CLEAN_TARGET_MIGRATION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../migrations/0012_review_clean_target_unify.sql"
));

#[test]
fn migration_0012_rebuilds_both_clean_target_projections() {
    let tables = table_names(CLEAN_TARGET_MIGRATION);
    for expected in &[
        "review_session_v2_projection",
        "review_exit_receipt_v2_projection",
    ] {
        assert!(
            tables.contains(*expected),
            "migration 0012 must rebuild table '{expected}'"
        );
    }
}

#[test]
fn migration_0012_pins_clean_target_to_one() {
    assert!(
        CLEAN_TARGET_MIGRATION.contains("clean_target INTEGER NOT NULL CHECK(clean_target=1)"),
        "clean_target must be constrained to exactly 1"
    );
    assert!(
        !CLEAN_TARGET_MIGRATION.contains("clean_target=2"),
        "no tier or repair class may still require two clean batches"
    );
}

#[test]
fn migration_0012_sets_user_version_to_12() {
    assert!(
        CLEAN_TARGET_MIGRATION.contains("PRAGMA user_version=12"),
        "migration 0012 must set user_version to 12"
    );
}
