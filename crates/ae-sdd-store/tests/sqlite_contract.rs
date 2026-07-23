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
    SupervisorCheckpointRecord,
};
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
        "2"
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
        "2"
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
        .expect("version two migration is repeatable");
    reopened
        .index_committed_mutation(
            &lease_control_receipt(workspace_id, work_item_id.clone(), "renew", 3),
            &event(workspace_id, work_item_id, 4),
        )
        .expect("same-revision receipt commits after restart");
    assert_eq!(
        reopened.pragma_value("user_version").expect("PRAGMA reads"),
        "2"
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
