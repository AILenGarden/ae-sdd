use rusqlite::{Connection, params};

const RUNTIME_BASE: &str = include_str!("../../../migrations/0001_runtime_base.sql");
const LEASE_CONTROL: &str = include_str!("../../../migrations/0002_lease_control_receipts.sql");
const ROUTE_SERIES: &str = include_str!("../../../migrations/0003_route_series_plan.sql");
const WORK_ITEM_LIFECYCLE: &str = include_str!("../../../migrations/0004_work_item_lifecycle.sql");

#[test]
fn route_and_series_projection_freezes_identity_digest_status_and_cursor_indexes() {
    for required in [
        "route_decision_projection",
        "schema_version",
        "work_item_id",
        "route_revision",
        "input_fingerprint",
        "decision_digest",
        "approval_binding_digest",
        "series_plan_projection",
        "series_id",
        "status",
        "plan_digest",
        "result_ref",
        "last_event_seq",
        "CREATE UNIQUE INDEX",
        "PRAGMA user_version = 3",
    ] {
        assert!(ROUTE_SERIES.contains(required), "0003 missing {required}");
    }
}

#[test]
fn route_projection_accepts_every_frozen_wire_disposition_and_rejects_legacy_spelling() {
    let connection = Connection::open_in_memory().expect("in-memory SQLite");
    connection
        .execute_batch(&format!("{RUNTIME_BASE}\n{LEASE_CONTROL}\n{ROUTE_SERIES}"))
        .expect("apply route projection migrations");
    for (revision, disposition) in ["await_user_approval", "approved", "denied", "superseded"]
        .into_iter()
        .enumerate()
    {
        connection
            .execute(
                "INSERT INTO route_decision_projection (
                    workspace_id, work_item_id, schema_version, route_revision,
                    input_fingerprint, decision_digest, disposition, scale,
                    design_route, created_at, updated_at
                 ) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, 'small', 'coding_plan', ?7, ?7)",
                params![
                    "workspace-route-contract",
                    format!("work-item-{revision}"),
                    i64::try_from(revision).expect("bounded fixture revision"),
                    format!("{revision:064x}"),
                    format!("{:064x}", revision + 100),
                    disposition,
                    "2026-07-24T00:00:00Z",
                ],
            )
            .unwrap_or_else(|error| panic!("wire disposition {disposition} rejected: {error}"));
    }
    let legacy = connection.execute(
        "INSERT INTO route_decision_projection (
            workspace_id, work_item_id, schema_version, route_revision,
            input_fingerprint, decision_digest, disposition, scale,
            design_route, created_at, updated_at
         ) VALUES (?1, ?2, 1, 99, ?3, ?4, 'awaiting_approval', 'small',
                   'coding_plan', ?5, ?5)",
        params![
            "workspace-route-contract",
            "legacy-spelling",
            "a".repeat(64),
            "b".repeat(64),
            "2026-07-24T00:00:00Z",
        ],
    );
    assert!(legacy.is_err());
}

#[test]
fn series_projection_foreign_key_binds_the_route_input_fingerprint() {
    let connection = Connection::open_in_memory().expect("in-memory SQLite");
    connection
        .execute_batch(&format!(
            "PRAGMA foreign_keys = ON;\n{RUNTIME_BASE}\n{LEASE_CONTROL}\n{ROUTE_SERIES}"
        ))
        .expect("apply route projection migrations with foreign keys");
    let route_fingerprint = "a".repeat(64);
    connection
        .execute(
            "INSERT INTO route_decision_projection (
                workspace_id, work_item_id, schema_version, route_revision,
                input_fingerprint, decision_digest, disposition, scale,
                design_route, created_at, updated_at
             ) VALUES ('workspace', 'work-item', 1, 7, ?1, ?2, 'approved',
                       'small', 'coding_plan', ?3, ?3)",
            params![route_fingerprint, "b".repeat(64), "2026-07-24T00:00:00Z"],
        )
        .expect("insert authoritative route snapshot");

    let insert_series = |series_id: &str, fingerprint: &str, plan_digest: &str| {
        connection.execute(
            "INSERT INTO series_plan_projection (
                workspace_id, work_item_id, series_id, schema_version, route_revision,
                input_fingerprint, series_kind, status, plan_digest,
                methodology_digest, created_at, updated_at
             ) VALUES ('workspace', 'work-item', ?1, 1, 7, ?2, 'coding', 'planned',
                       ?3, ?4, ?5, ?5)",
            params![
                series_id,
                fingerprint,
                plan_digest,
                "e".repeat(64),
                "2026-07-24T00:00:00Z"
            ],
        )
    };

    assert!(
        insert_series("series-mismatch", &"c".repeat(64), &"d".repeat(64)).is_err(),
        "Series with a different input fingerprint was attached to the Route"
    );
    insert_series("series-match", &route_fingerprint, &"f".repeat(64))
        .expect("matching Route revision and fingerprint must remain insertable");
}

#[test]
fn lifecycle_projection_freezes_command_plan_confirmation_evidence_intents_and_children() {
    for required in [
        "lifecycle_plan_projection",
        "command_digest",
        "plan_digest",
        "expected_revision",
        "confirmation_binding_digest",
        "lifecycle_evidence_ref",
        "lifecycle_mutation_intent",
        "intent_id",
        "expected_digest",
        "prd_completion_projection",
        "dependencies_satisfied",
        "residual_risks_cleared",
        "gates_passed",
        "review_passed",
        "prd_child_completion_projection",
        "story_id",
        "completed",
        "PRAGMA user_version = 4",
    ] {
        assert!(
            WORK_ITEM_LIFECYCLE.contains(required),
            "0004 missing {required}"
        );
    }
}

#[test]
fn future_projections_reference_but_do_not_duplicate_durable_event_or_operation_truth() {
    let combined = format!("{ROUTE_SERIES}\n{WORK_ITEM_LIFECYCLE}").to_ascii_lowercase();
    assert!(!combined.contains("create table runtime_event"));
    assert!(!combined.contains("create table operation_receipt"));
    assert!(!combined.contains("alter table runtime_event"));
    assert!(!combined.contains("alter table operation_receipt"));
}
