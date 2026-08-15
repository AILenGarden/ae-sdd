mod support;

use std::str::FromStr;

use ae_sdd_domain::{
    ArtifactDigest, BootId, ContextDigest, ContextGeneration, ContextProjectionId, ContextRevision,
    DecisionDigest, DelegationId, EventStoreId, HostAckId, HostActionId, InputFingerprint,
    InventoryGeneration, OperationId, PolicyDigest, RequestId, ResultDigest, SessionId,
    StateRevision, WorkItemId, WorkspaceId,
};
use ae_sdd_store::{
    CompactCycleRecord, ContextPressureSampleRecord, ContextProjectionRecord, DelegationRecord,
    HostAckReceipt, HostActionRecord, HostAdapterRecord, IdempotencyKey, OperationReceipt,
    RuntimeEventDraft, RuntimeEventPayload, RuntimeRepository, SqliteRuntimeRepository, StoreError,
    SupervisorCheckpointRecord, latest_runtime_schema_version,
};
use rusqlite::Connection;
use uuid::Uuid;

fn uuid<T>(value: u128) -> T
where
    T: FromStr,
    T::Err: std::fmt::Debug,
{
    Uuid::from_u128(value)
        .to_string()
        .parse()
        .expect("typed UUID fixture parses")
}

fn event(workspace_id: WorkspaceId, work_item_id: WorkItemId, marker: u64) -> RuntimeEventDraft {
    RuntimeEventDraft {
        boot_id: uuid::<BootId>(900),
        workspace_id,
        session_id: None,
        work_item_id,
        event_type: "state.committed".into(),
        schema_version: 1,
        payload: RuntimeEventPayload::InlineJson(
            serde_json::to_vec(&serde_json::json!({"marker":marker})).expect("payload serializes"),
        ),
        committed_at: support::at("2026-07-23T00:01:00Z"),
    }
}

fn receipt(
    workspace_id: WorkspaceId,
    work_item_id: WorkItemId,
    key: &str,
    marker: u64,
) -> OperationReceipt {
    OperationReceipt {
        workspace_id,
        work_item_id,
        idempotency_key: IdempotencyKey::new(key).expect("key is valid"),
        payload_digest: InputFingerprint::digest(marker.to_be_bytes()),
        operation: OperationId::new("state.transition").expect("operation is valid"),
        revision_before: StateRevision::new(marker - 1),
        revision_after: StateRevision::new(marker),
        fencing_token: marker.into(),
        result_digest: ResultDigest::digest(marker.to_be_bytes()),
        mutation_id: RequestId::from_uuid(Uuid::from_u128(1000 + u128::from(marker))),
        committed_at: support::at("2026-07-23T00:01:00Z"),
    }
}

fn lease_control_receipt(
    workspace_id: WorkspaceId,
    work_item_id: WorkItemId,
    key: &str,
    revision: u64,
) -> OperationReceipt {
    let mut receipt = receipt(workspace_id, work_item_id, key, revision + 1);
    receipt.operation = OperationId::new("lease.acquire").expect("operation is valid");
    receipt.revision_before = StateRevision::new(revision);
    receipt.revision_after = StateRevision::new(revision);
    receipt
}

#[test]
fn migration_is_repeatable_and_event_sequence_survives_restart() {
    let temp = tempfile::tempdir().expect("temp directory is created");
    let database = temp.path().join("runtime.sqlite3");
    let created_at = support::at("2026-07-23T00:00:00Z");
    let proposed = EventStoreId::from_uuid(Uuid::from_u128(700));
    let workspace_id = WorkspaceId::from_uuid(Uuid::from_u128(701));
    let work_item_id = WorkItemId::new("WI-1").expect("work item is valid");
    let first = SqliteRuntimeRepository::open(&database, proposed, &created_at)
        .expect("database opens and migrates");
    first
        .integrity_check()
        .expect("database is internally consistent");
    assert_eq!(
        first.pragma_value("foreign_keys").expect("PRAGMA reads"),
        "1"
    );
    assert_eq!(
        first.pragma_value("user_version").expect("PRAGMA reads"),
        latest_runtime_schema_version().to_string()
    );
    let (_, first_event) = first
        .index_committed_mutation(
            &receipt(workspace_id, work_item_id.clone(), "first", 1),
            &event(workspace_id, work_item_id.clone(), 1),
        )
        .expect("first event commits");
    assert_eq!(first_event.event_sequence.get(), 1);
    drop(first);

    let reopened = SqliteRuntimeRepository::open(
        &database,
        EventStoreId::from_uuid(Uuid::from_u128(999)),
        &created_at,
    )
    .expect("published migration repeats safely");
    assert_eq!(reopened.event_store_id(), proposed);
    let (_, second_event) = reopened
        .index_committed_mutation(
            &receipt(workspace_id, work_item_id.clone(), "second", 2),
            &event(workspace_id, work_item_id, 2),
        )
        .expect("second event commits after restart");
    assert_eq!(second_event.event_sequence.get(), 2);
    assert_eq!(second_event.event_store_id, proposed);
    assert_eq!(
        reopened.pragma_value("user_version").expect("PRAGMA reads"),
        latest_runtime_schema_version().to_string()
    );
}

#[test]
fn published_schema_15_database_remains_readable() {
    const PUBLISHED_SCHEMA_15_CHECKSUM: &str =
        "22f76d56b69d94dad4715c813f0103427fb9419118c3dcf23752916626a89718";

    let temp = tempfile::tempdir().expect("temp directory is created");
    let database = temp.path().join("runtime.sqlite3");
    let created_at = support::at("2026-08-10T13:37:05Z");
    let event_store_id = EventStoreId::from_uuid(Uuid::from_u128(704));
    let repository = SqliteRuntimeRepository::open(&database, event_store_id, &created_at)
        .expect("pre-schema-15 database opens");
    drop(repository);

    let connection = Connection::open(&database).expect("fixture database opens");
    connection
        .execute(
            "INSERT OR IGNORE INTO schema_migration(version,name,checksum,applied_at) VALUES(15,?1,?2,?3)",
            (
                "0015_series_review_authority_v1",
                PUBLISHED_SCHEMA_15_CHECKSUM,
                created_at.to_string(),
            ),
        )
        .expect("published migration catalog row is recorded");
    connection
        .pragma_update(None, "user_version", 15)
        .expect("published schema version is recorded");
    drop(connection);

    let reopened = SqliteRuntimeRepository::open(&database, event_store_id, &created_at)
        .expect("published schema 15 remains readable after a package upgrade");
    assert_eq!(
        reopened.pragma_value("user_version").expect("PRAGMA reads"),
        "15"
    );
}

#[test]
fn lease_control_receipts_can_keep_the_project_revision_unchanged() {
    let temp = tempfile::tempdir().expect("temp directory is created");
    let database = temp.path().join("runtime.sqlite3");
    let created_at = support::at("2026-07-23T00:00:00Z");
    let event_store_id = EventStoreId::from_uuid(Uuid::from_u128(705));
    let workspace_id = WorkspaceId::from_uuid(Uuid::from_u128(706));
    let work_item_id = WorkItemId::new("WI-LEASE").expect("work item is valid");
    let repository = SqliteRuntimeRepository::open(&database, event_store_id, &created_at)
        .expect("database opens and migrates");
    repository
        .index_committed_mutation(
            &lease_control_receipt(workspace_id, work_item_id.clone(), "acquire", 3),
            &event(workspace_id, work_item_id.clone(), 3),
        )
        .expect("same-revision receipt commits");
    drop(repository);

    let reopened = SqliteRuntimeRepository::open(&database, event_store_id, &created_at)
        .expect("published migrations are repeatable");
    reopened
        .index_committed_mutation(
            &lease_control_receipt(workspace_id, work_item_id.clone(), "renew", 3),
            &event(workspace_id, work_item_id, 4),
        )
        .expect("same-revision receipt commits after restart");
    assert_eq!(
        reopened.pragma_value("user_version").expect("PRAGMA reads"),
        latest_runtime_schema_version().to_string()
    );
}

#[test]
fn operation_idempotency_rejects_a_mutated_payload() {
    let repository = SqliteRuntimeRepository::open_in_memory(
        EventStoreId::from_uuid(Uuid::from_u128(710)),
        &support::at("2026-07-23T00:00:00Z"),
    )
    .expect("database opens");
    let workspace_id = WorkspaceId::from_uuid(Uuid::from_u128(711));
    let work_item_id = WorkItemId::new("WI-1").expect("work item is valid");
    let original = receipt(workspace_id, work_item_id.clone(), "same-key", 1);
    repository
        .index_committed_mutation(&original, &event(workspace_id, work_item_id.clone(), 1))
        .expect("original request commits");
    let changed = receipt(workspace_id, work_item_id.clone(), "same-key", 2);

    assert!(matches!(
        repository.index_committed_mutation(&changed, &event(workspace_id, work_item_id, 2)),
        Err(StoreError::IdempotencyKeyReused { .. })
    ));
}

#[test]
fn lifecycle_records_and_supervisor_checkpoint_use_typed_tables() {
    let repository = SqliteRuntimeRepository::open_in_memory(
        EventStoreId::from_uuid(Uuid::from_u128(720)),
        &support::at("2026-07-23T00:00:00Z"),
    )
    .expect("database opens");
    let workspace_id = uuid::<WorkspaceId>(721);
    let root_session = uuid::<SessionId>(722);
    let child_session = uuid::<SessionId>(723);
    let delegation_id = uuid::<DelegationId>(724);
    let timestamp = support::at("2026-07-23T00:05:00Z");
    let delegation = DelegationRecord {
        delegation_id,
        workspace_id,
        root_session_id: root_session,
        parent_session_id: root_session,
        child_session_id: Some(child_session),
        parent_delegation_id: None,
        role: "series".into(),
        input_revision: StateRevision::new(1),
        input_fingerprint: InputFingerprint::digest(b"input"),
        status: "running".into(),
        deadline: timestamp.clone(),
        receipt_digest: ResultDigest::digest(b"delegation"),
    };
    repository
        .persist_delegation(&delegation)
        .expect("delegation persists");
    assert_eq!(
        repository
            .delegation(delegation_id)
            .expect("delegation query succeeds"),
        Some(delegation)
    );

    let adapter = HostAdapterRecord {
        adapter_id: "codex".into(),
        capability_digest: ArtifactDigest::digest(b"capabilities"),
        status: "active".into(),
        last_command_sequence: 1,
        heartbeat_at: timestamp.clone(),
        created_at: timestamp.clone(),
        updated_at: timestamp.clone(),
    };
    repository
        .persist_host_adapter(&adapter)
        .expect("host adapter persists");
    let action_id = uuid::<HostActionId>(725);
    repository
        .persist_host_action(&HostActionRecord {
            action_id,
            adapter_id: "codex".into(),
            kind: "compact".into(),
            command_sequence: 1,
            request_digest: InputFingerprint::digest(b"compact"),
            session_id: Some(child_session),
            context_generation: Some(ContextGeneration::new(1)),
            ack_status: "pending".into(),
            deadline: timestamp.clone(),
        })
        .expect("host action persists");
    let ack = HostAckReceipt {
        ack_id: uuid::<HostAckId>(726),
        action_id,
        adapter_id: "codex".into(),
        response_digest: ResultDigest::digest(b"ack"),
        acknowledged_at: timestamp.clone(),
    };
    assert_eq!(repository.put_host_ack(&ack).expect("ACK persists"), ack);

    repository
        .persist_pressure_sample(&ContextPressureSampleRecord {
            adapter_id: "codex".into(),
            session_id: child_session,
            context_generation: ContextGeneration::new(1),
            sample_sequence: 1,
            used_tokens: 800,
            context_window_tokens: 1000,
            source: "host".into(),
            observed_at: timestamp.clone(),
        })
        .expect("pressure sample persists");
    repository
        .persist_context_projection(&ContextProjectionRecord {
            projection_id: uuid::<ContextProjectionId>(727),
            session_id: child_session,
            context_revision: ContextRevision::new(1),
            source_revision: StateRevision::new(1),
            policy_digest: PolicyDigest::digest(b"policy"),
            inventory_generation: InventoryGeneration::new(1),
            digest: ContextDigest::digest(b"projection"),
            byte_budget: 4096,
            expires_at: timestamp.clone(),
        })
        .expect("projection persists");
    repository
        .persist_compact_cycle(&CompactCycleRecord {
            compact_id: uuid(728),
            session_id: child_session,
            snapshot_ref: ".ae-sdd/context/snapshot.json".into(),
            previous_generation: ContextGeneration::new(1),
            next_generation: ContextGeneration::new(2),
            host_action_id: action_id,
            status: "host_acknowledged".into(),
            deadline: timestamp.clone(),
            restored_digest: None,
        })
        .expect("compact cycle persists");

    let checkpoint = SupervisorCheckpointRecord {
        workspace_id,
        work_item_id: WorkItemId::new("WI-1").expect("work item is valid"),
        last_event_sequence: 2_u64.into(),
        last_event_digest: ArtifactDigest::digest(b"event"),
        state_revision: StateRevision::new(1),
        input_fingerprint: InputFingerprint::digest(b"input"),
        policy_digest: PolicyDigest::digest(b"policy"),
        last_decision_digest: DecisionDigest::digest(b"decision"),
        health: "healthy".into(),
        updated_at: timestamp,
    };
    repository
        .persist_supervisor_checkpoint(&checkpoint)
        .expect("checkpoint persists");
    assert_eq!(
        repository
            .supervisor_checkpoint(workspace_id, &checkpoint.work_item_id)
            .expect("checkpoint query succeeds"),
        Some(checkpoint)
    );
}

fn assert_corrupt_event_rejected(set_clause: &str, expected_reason: &str, marker: u128) {
    let temp = tempfile::tempdir().expect("temp directory is created");
    let database = temp.path().join("runtime.sqlite3");
    let created_at = support::at("2026-07-23T00:00:00Z");
    let event_store_id = uuid::<EventStoreId>(800 + marker);
    let workspace_id = uuid::<WorkspaceId>(900 + marker);
    let work_item_id =
        WorkItemId::new(format!("WI-CORRUPT-{marker}")).expect("work item fixture is valid");
    let repository = SqliteRuntimeRepository::open(&database, event_store_id, &created_at)
        .expect("database opens");
    repository
        .index_committed_mutation(
            &receipt(workspace_id, work_item_id.clone(), "corrupt-row", 1),
            &event(workspace_id, work_item_id, 1),
        )
        .expect("fixture event commits");
    drop(repository);

    let connection = Connection::open(&database).expect("fixture database opens for corruption");
    connection
        .pragma_update(None, "ignore_check_constraints", true)
        .expect("corruption fixture bypasses SQLite CHECK constraints only while mutating");
    connection
        .execute(
            &format!("UPDATE runtime_event SET {set_clause} WHERE event_seq=1"),
            [],
        )
        .expect("fixture corruption is applied");
    drop(connection);

    let repository = SqliteRuntimeRepository::open(&database, event_store_id, &created_at)
        .expect("database remains structurally readable");
    let error = repository
        .operation_receipt(workspace_id, "corrupt-row")
        .expect_err("corrupt event row must fail closed");
    assert!(
        matches!(
            &error,
            StoreError::DatabaseIncompatible { reason } if reason.contains(expected_reason)
        ),
        "unexpected corruption mapping: {error:?}"
    );
}

#[test]
fn corrupt_sqlite_event_rows_fail_closed_with_specific_error_mapping() {
    for (marker, set_clause, expected_reason) in [
        (
            1,
            "payload_json='tampered'",
            "inline payload failed integrity validation",
        ),
        (2, "payload_ref='artifact.json'", "invalid payload union"),
        (3, "byte_len=-1", "runtime_event.byte_len is negative"),
        (
            4,
            "schema_version=4294967296",
            "schema version is out of range",
        ),
        (
            5,
            "boot_id='not-a-uuid'",
            "runtime_event.boot_id is invalid",
        ),
    ] {
        assert_corrupt_event_rejected(set_clause, expected_reason, marker);
    }
}

#[test]
fn sqlite_lifecycle_writes_reject_invalid_ranges_and_conflicting_replays() {
    let repository = SqliteRuntimeRepository::open_in_memory(
        uuid::<EventStoreId>(950),
        &support::at("2026-07-23T00:00:00Z"),
    )
    .expect("database opens");
    let timestamp = support::at("2026-07-23T00:05:00Z");
    let session_id = uuid::<SessionId>(951);
    let action_id = uuid::<HostActionId>(952);

    let orphan_ack = HostAckReceipt {
        ack_id: uuid::<HostAckId>(953),
        action_id,
        adapter_id: "codex".into(),
        response_digest: ResultDigest::digest(b"orphan"),
        acknowledged_at: timestamp.clone(),
    };
    assert!(matches!(
        repository.put_host_ack(&orphan_ack),
        Err(StoreError::PersistenceConflict {
            entity: "host_ack.action"
        })
    ));

    let pressure =
        |sample_sequence, used_tokens, context_window_tokens| ContextPressureSampleRecord {
            adapter_id: "codex".into(),
            session_id,
            context_generation: ContextGeneration::new(1),
            sample_sequence,
            used_tokens,
            context_window_tokens,
            source: "host".into(),
            observed_at: timestamp.clone(),
        };
    for invalid in [pressure(1, 0, 0), pressure(2, 2, 1)] {
        assert!(matches!(
            repository.persist_pressure_sample(&invalid),
            Err(StoreError::PersistenceConflict {
                entity: "context_pressure_sample.tokens"
            })
        ));
    }
    assert!(matches!(
        repository.persist_pressure_sample(&pressure(u64::MAX, 1, 1)),
        Err(StoreError::DatabaseIncompatible { reason })
            if reason.contains("context_pressure_sample.sample_seq exceeds SQLite INTEGER range")
    ));

    assert!(matches!(
        repository.persist_compact_cycle(&CompactCycleRecord {
            compact_id: uuid(954),
            session_id,
            snapshot_ref: ".ae-sdd/context/snapshot.json".into(),
            previous_generation: ContextGeneration::new(2),
            next_generation: ContextGeneration::new(2),
            host_action_id: action_id,
            status: "pending".into(),
            deadline: timestamp.clone(),
            restored_digest: None,
        }),
        Err(StoreError::PersistenceConflict {
            entity: "compact_cycle.generation"
        })
    ));

    let projection = ContextProjectionRecord {
        projection_id: uuid::<ContextProjectionId>(955),
        session_id,
        context_revision: ContextRevision::new(1),
        source_revision: StateRevision::new(1),
        policy_digest: PolicyDigest::digest(b"policy"),
        inventory_generation: InventoryGeneration::new(1),
        digest: ContextDigest::digest(b"projection"),
        byte_budget: 4096,
        expires_at: timestamp.clone(),
    };
    repository
        .persist_context_projection(&projection)
        .expect("projection fixture persists");
    let mut conflicting_projection = projection;
    conflicting_projection.digest = ContextDigest::digest(b"different");
    assert!(matches!(
        repository.persist_context_projection(&conflicting_projection),
        Err(StoreError::PersistenceConflict {
            entity: "context_projection"
        })
    ));

    let workspace_id = uuid::<WorkspaceId>(956);
    let checkpoint = SupervisorCheckpointRecord {
        workspace_id,
        work_item_id: WorkItemId::new("WI-CONFLICT").expect("work item is valid"),
        last_event_sequence: 2_u64.into(),
        last_event_digest: ArtifactDigest::digest(b"event"),
        state_revision: StateRevision::new(1),
        input_fingerprint: InputFingerprint::digest(b"input"),
        policy_digest: PolicyDigest::digest(b"policy"),
        last_decision_digest: DecisionDigest::digest(b"decision"),
        health: "healthy".into(),
        updated_at: timestamp.clone(),
    };
    repository
        .persist_supervisor_checkpoint(&checkpoint)
        .expect("checkpoint fixture persists");
    let mut conflicting_checkpoint = checkpoint;
    conflicting_checkpoint.health = "degraded".into();
    assert!(matches!(
        repository.persist_supervisor_checkpoint(&conflicting_checkpoint),
        Err(StoreError::PersistenceConflict {
            entity: "supervisor_checkpoint.cursor"
        })
    ));

    let delegation_id = uuid::<DelegationId>(957);
    let delegation = DelegationRecord {
        delegation_id,
        workspace_id,
        root_session_id: session_id,
        parent_session_id: session_id,
        child_session_id: None,
        parent_delegation_id: None,
        role: "series".into(),
        input_revision: StateRevision::new(1),
        input_fingerprint: InputFingerprint::digest(b"input"),
        status: "running".into(),
        deadline: timestamp,
        receipt_digest: ResultDigest::digest(b"delegation"),
    };
    repository
        .persist_delegation(&delegation)
        .expect("delegation fixture persists");
    let mut conflicting_delegation = delegation;
    conflicting_delegation.role = "reviewer".into();
    assert!(matches!(
        repository.persist_delegation(&conflicting_delegation),
        Err(StoreError::PersistenceConflict {
            entity: "delegation"
        })
    ));
}

#[test]
fn sqlite_rejects_unknown_pragmas_future_schemas_and_nonempty_version_zero_databases() {
    let created_at = support::at("2026-07-23T00:00:00Z");
    let repository =
        SqliteRuntimeRepository::open_in_memory(uuid::<EventStoreId>(980), &created_at)
            .expect("database opens");
    assert!(matches!(
        repository.pragma_value("cache_size"),
        Err(StoreError::DatabaseIncompatible { reason })
            if reason.contains("PRAGMA name is not in the compile-time allowlist")
    ));

    let future = tempfile::tempdir().expect("future schema temp directory is created");
    let future_database = future.path().join("runtime.sqlite3");
    let connection = Connection::open(&future_database).expect("future schema database opens");
    connection
        .pragma_update(None, "user_version", latest_runtime_schema_version() + 1)
        .expect("future schema version is written");
    drop(connection);
    assert!(matches!(
        SqliteRuntimeRepository::open(&future_database, uuid::<EventStoreId>(981), &created_at),
        Err(StoreError::DatabaseIncompatible { reason })
            if reason.contains("unsupported runtime schema version")
    ));

    let unversioned = tempfile::tempdir().expect("unversioned temp directory is created");
    let unversioned_database = unversioned.path().join("runtime.sqlite3");
    let connection = Connection::open(&unversioned_database).expect("unversioned database opens");
    connection
        .execute("CREATE TABLE unexpected(id INTEGER PRIMARY KEY)", [])
        .expect("unversioned user table is created");
    drop(connection);
    assert!(matches!(
        SqliteRuntimeRepository::open(
            &unversioned_database,
            uuid::<EventStoreId>(982),
            &created_at,
        ),
        Err(StoreError::DatabaseIncompatible { reason })
            if reason.contains("version-zero runtime database is not empty")
    ));
}
