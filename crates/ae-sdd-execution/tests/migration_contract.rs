//! Migration 0008 contract test: execution_receipts SQL aligns with frozen DTO.

use std::collections::BTreeSet;

const MIGRATION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../migrations/0008_execution_receipts.sql"
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
fn migration_0008_defines_expected_execution_tables() {
    let tables = table_names(MIGRATION);
    for expected in &[
        "verification_execution_plan_projection",
        "verification_step_projection",
        "verification_receipt_projection",
    ] {
        assert!(
            tables.contains(*expected),
            "migration 0008 must define table '{expected}'"
        );
    }
}

#[test]
fn migration_0008_sets_user_version_to_8() {
    assert!(
        MIGRATION.contains("PRAGMA user_version = 8"),
        "migration 0008 must set user_version to 8"
    );
}

#[test]
fn migration_0008_plan_has_work_item_fingerprint_unique() {
    assert!(
        MIGRATION.contains("UNIQUE(workspace_id, work_item_id, input_fingerprint)"),
        "verification_execution_plan_projection must enforce work_item+fingerprint uniqueness"
    );
}

#[test]
fn migration_0008_receipt_has_pass_consistency_check() {
    assert!(
        MIGRATION.contains("status = 'pass'"),
        "verification_receipt_projection must enforce PASS consistency"
    );
    assert!(
        MIGRATION.contains("exit_code = 0"),
        "PASS consistency must require exit_code = 0"
    );
}

#[test]
fn migration_0008_receipt_has_timeout_flag_check() {
    assert!(
        MIGRATION.contains("status = 'timeout'"),
        "verification_receipt_projection must enforce timeout flag consistency"
    );
}

#[test]
fn migration_0008_receipt_has_digest_unique() {
    assert!(
        MIGRATION.contains("UNIQUE(workspace_id, receipt_digest)"),
        "verification_receipt_projection must enforce receipt_digest uniqueness"
    );
}

#[test]
fn migration_0008_references_runtime_event() {
    assert!(
        MIGRATION.contains("REFERENCES runtime_event(event_seq)"),
        "execution projections must reference runtime_event for durable replay"
    );
}
