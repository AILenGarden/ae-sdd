mod support;

use std::{fmt::Debug, str::FromStr};

use ae_sdd_domain::{
    ArtifactDigest, BootId, CompactId, ContextDigest, ContextGeneration, ContextProjectionId,
    ContextRevision, DecisionDigest, DelegationId, EventSequence, EventStoreId, HostAckId,
    HostActionId, InputFingerprint, InventoryGeneration, OperationId, PolicyDigest, RequestId,
    ResultDigest, SessionId, StateRevision, WorkItemId, WorkspaceId,
};
use ae_sdd_store::{
    ChildResultRecord, CompactCycleRecord, ContextPressureSampleRecord, ContextProjectionRecord,
    DelegationRecord, DelegationRequestReceipt, HookEventReceipt, HostAckReceipt, HostActionRecord,
    HostAdapterRecord, IdempotencyKey, InMemoryRuntimeRepository, MemoryCleanupReceipt,
    OperationReceipt, RuntimeEventDraft, RuntimeEventPayload, RuntimeRepository,
    SqliteRuntimeRepository, StoreError, SupervisorCheckpointRecord, UtcTimestamp,
};
use uuid::Uuid;

fn uuid<T>(value: u128) -> T
where
    T: FromStr,
    T::Err: Debug,
{
    Uuid::from_u128(value)
        .to_string()
        .parse()
        .expect("typed UUID fixture parses")
}

fn timestamp() -> UtcTimestamp {
    support::at("2026-07-23T00:05:00Z")
}

fn operation_receipt(
    workspace_id: WorkspaceId,
    work_item_id: WorkItemId,
    key: &str,
    marker: u64,
) -> OperationReceipt {
    OperationReceipt {
        workspace_id,
        work_item_id,
        idempotency_key: IdempotencyKey::new(key).expect("idempotency key is valid"),
        payload_digest: InputFingerprint::digest(marker.to_be_bytes()),
        operation: OperationId::new("state.transition").expect("operation is valid"),
        revision_before: StateRevision::new(marker - 1),
        revision_after: StateRevision::new(marker),
        fencing_token: marker.into(),
        result_digest: ResultDigest::digest(marker.to_be_bytes()),
        mutation_id: uuid::<RequestId>(1_000 + u128::from(marker)),
        committed_at: timestamp(),
    }
}

fn inline_event(
    workspace_id: WorkspaceId,
    work_item_id: WorkItemId,
    marker: u64,
) -> RuntimeEventDraft {
    RuntimeEventDraft {
        boot_id: uuid::<BootId>(900),
        workspace_id,
        session_id: Some(uuid::<SessionId>(901)),
        work_item_id,
        event_type: "state.committed".into(),
        schema_version: 1,
        payload: RuntimeEventPayload::InlineJson(
            serde_json::to_vec(&serde_json::json!({"marker":marker})).expect("payload serializes"),
        ),
        committed_at: timestamp(),
    }
}

fn artifact_event(workspace_id: WorkspaceId, work_item_id: WorkItemId) -> RuntimeEventDraft {
    RuntimeEventDraft {
        boot_id: uuid::<BootId>(902),
        workspace_id,
        session_id: None,
        work_item_id,
        event_type: "artifact.committed".into(),
        schema_version: 2,
        payload: RuntimeEventPayload::ArtifactRef {
            project_relative_path: "ae-sdd-doc/evidence/event.json".into(),
            digest: ArtifactDigest::digest(b"artifact event"),
            byte_length: 14,
        },
        committed_at: timestamp(),
    }
}

fn exercise_repository(repository: &dyn RuntimeRepository) {
    let workspace_id = uuid::<WorkspaceId>(10);
    let work_item_id = WorkItemId::new("WI-REPOSITORY").expect("work item is valid");

    let receipt = operation_receipt(workspace_id, work_item_id.clone(), "operation-1", 1);
    let event = inline_event(workspace_id, work_item_id.clone(), 1);
    let (stored_receipt, stored_event) = repository
        .index_committed_mutation(&receipt, &event)
        .expect("first operation commits");
    assert_eq!(stored_receipt, receipt);
    assert_eq!(stored_event.event_sequence, EventSequence::new(1));
    assert_eq!(
        repository
            .index_committed_mutation(&receipt, &event)
            .expect("exact operation retry replays"),
        (stored_receipt.clone(), stored_event.clone())
    );
    assert_eq!(
        repository
            .operation_receipt(workspace_id, "operation-1")
            .expect("receipt lookup succeeds"),
        Some((stored_receipt, stored_event))
    );
    assert!(
        repository
            .operation_receipt(workspace_id, "missing")
            .expect("missing lookup succeeds")
            .is_none()
    );

    let mut conflicting_receipt = receipt.clone();
    conflicting_receipt.payload_digest = InputFingerprint::digest(b"different payload");
    assert!(matches!(
        repository.index_committed_mutation(&conflicting_receipt, &event),
        Err(StoreError::IdempotencyKeyReused { .. })
    ));

    let artifact_receipt =
        operation_receipt(workspace_id, work_item_id.clone(), "operation-artifact", 2);
    let artifact_event = artifact_event(workspace_id, work_item_id.clone());
    repository
        .index_committed_mutation(&artifact_receipt, &artifact_event)
        .expect("artifact-ref event commits");
    let (_, loaded_artifact_event) = repository
        .operation_receipt(workspace_id, "operation-artifact")
        .expect("artifact receipt lookup succeeds")
        .expect("artifact receipt exists");
    assert_eq!(loaded_artifact_event.draft, artifact_event);

    let root_session_id = uuid::<SessionId>(20);
    let child_session_id = uuid::<SessionId>(21);
    let delegation_id = uuid::<DelegationId>(22);
    let delegation = DelegationRecord {
        delegation_id,
        workspace_id,
        root_session_id,
        parent_session_id: root_session_id,
        child_session_id: Some(child_session_id),
        parent_delegation_id: None,
        role: "series".into(),
        input_revision: StateRevision::new(2),
        input_fingerprint: InputFingerprint::digest(b"delegation input"),
        status: "running".into(),
        deadline: timestamp(),
        receipt_digest: ResultDigest::digest(b"delegation receipt"),
    };
    assert!(
        repository
            .delegation(delegation_id)
            .expect("missing delegation lookup succeeds")
            .is_none()
    );
    repository
        .persist_delegation(&delegation)
        .expect("delegation persists");
    assert_eq!(
        repository
            .delegation(delegation_id)
            .expect("delegation lookup succeeds"),
        Some(delegation.clone())
    );
    let mut completed_delegation = delegation.clone();
    completed_delegation.status = "completed".into();
    repository
        .persist_delegation(&completed_delegation)
        .expect("mutable delegation fields update");
    assert_eq!(
        repository
            .delegation(delegation_id)
            .expect("updated delegation lookup succeeds"),
        Some(completed_delegation)
    );

    let delegation_receipt = DelegationRequestReceipt {
        workspace_id,
        parent_session_id: root_session_id,
        idempotency_key: IdempotencyKey::new("delegation-1").expect("key is valid"),
        request_digest: InputFingerprint::digest(b"delegation request"),
        delegation_id,
        response_digest: ResultDigest::digest(b"delegation response"),
        created_at: timestamp(),
    };
    assert_eq!(
        repository
            .put_delegation_request_receipt(&delegation_receipt)
            .expect("delegation receipt persists"),
        delegation_receipt
    );
    assert_eq!(
        repository
            .put_delegation_request_receipt(&delegation_receipt)
            .expect("delegation receipt replays"),
        delegation_receipt
    );
    let mut changed_delegation_receipt = delegation_receipt.clone();
    changed_delegation_receipt.request_digest = InputFingerprint::digest(b"changed request");
    assert!(matches!(
        repository.put_delegation_request_receipt(&changed_delegation_receipt),
        Err(StoreError::IdempotencyKeyReused { .. })
    ));

    let child_result = ChildResultRecord {
        delegation_id,
        schema_version: 1,
        result_digest: ResultDigest::digest(b"child result"),
        byte_length: 12,
        validation_status: "valid".into(),
        artifact_ref: "ae-sdd-doc/evidence/child.json".into(),
        created_at: timestamp(),
        updated_at: timestamp(),
    };
    repository
        .persist_child_result(&child_result)
        .expect("child result persists");
    repository
        .persist_child_result(&child_result)
        .expect("same child result is idempotent");
    let mut changed_child_result = child_result.clone();
    changed_child_result.byte_length += 1;
    assert!(matches!(
        repository.persist_child_result(&changed_child_result),
        Err(StoreError::PersistenceConflict { .. })
    ));

    let cleanup = MemoryCleanupReceipt {
        delegation_id,
        namespace: "delegation:22".into(),
        snapshot_digest: ArtifactDigest::digest(b"snapshot"),
        cleanup_digest: ResultDigest::digest(b"cleanup"),
        cleaned_at: timestamp(),
    };
    repository
        .persist_memory_cleanup(&cleanup)
        .expect("cleanup receipt persists");
    repository
        .persist_memory_cleanup(&cleanup)
        .expect("same cleanup receipt is idempotent");
    let mut changed_cleanup = cleanup.clone();
    changed_cleanup.namespace = "delegation:changed".into();
    assert!(matches!(
        repository.persist_memory_cleanup(&changed_cleanup),
        Err(StoreError::PersistenceConflict { .. })
    ));

    let adapter = HostAdapterRecord {
        adapter_id: "codex".into(),
        capability_digest: ArtifactDigest::digest(b"capabilities"),
        status: "active".into(),
        last_command_sequence: 1,
        heartbeat_at: timestamp(),
        created_at: timestamp(),
        updated_at: timestamp(),
    };
    repository
        .persist_host_adapter(&adapter)
        .expect("host adapter persists");
    let mut updated_adapter = adapter.clone();
    updated_adapter.status = "ready".into();
    updated_adapter.last_command_sequence = 2;
    repository
        .persist_host_adapter(&updated_adapter)
        .expect("host adapter updates");

    let action_id = uuid::<HostActionId>(30);
    let action = HostActionRecord {
        action_id,
        adapter_id: "codex".into(),
        kind: "compact".into(),
        command_sequence: 1,
        request_digest: InputFingerprint::digest(b"compact request"),
        session_id: Some(child_session_id),
        context_generation: Some(ContextGeneration::new(1)),
        ack_status: "pending".into(),
        deadline: timestamp(),
    };
    repository
        .persist_host_action(&action)
        .expect("host action persists");
    repository
        .persist_host_action(&action)
        .expect("same host action is idempotent");
    let command_collision = HostActionRecord {
        action_id: uuid::<HostActionId>(31),
        ..action.clone()
    };
    assert!(repository.persist_host_action(&command_collision).is_err());

    let ack = HostAckReceipt {
        ack_id: uuid::<HostAckId>(32),
        action_id,
        adapter_id: "codex".into(),
        response_digest: ResultDigest::digest(b"host ack"),
        acknowledged_at: timestamp(),
    };
    assert_eq!(
        repository.put_host_ack(&ack).expect("host ACK persists"),
        ack
    );
    assert_eq!(
        repository.put_host_ack(&ack).expect("host ACK replays"),
        ack
    );
    let mut changed_ack = ack.clone();
    changed_ack.response_digest = ResultDigest::digest(b"changed ACK");
    assert!(matches!(
        repository.put_host_ack(&changed_ack),
        Err(StoreError::PersistenceConflict { .. })
    ));

    let pressure = ContextPressureSampleRecord {
        adapter_id: "codex".into(),
        session_id: child_session_id,
        context_generation: ContextGeneration::new(1),
        sample_sequence: 1,
        used_tokens: 800,
        context_window_tokens: 1_000,
        source: "host".into(),
        observed_at: timestamp(),
    };
    repository
        .persist_pressure_sample(&pressure)
        .expect("pressure sample persists");
    repository
        .persist_pressure_sample(&pressure)
        .expect("same pressure sample is idempotent");
    let mut changed_pressure = pressure.clone();
    changed_pressure.used_tokens = 900;
    assert!(matches!(
        repository.persist_pressure_sample(&changed_pressure),
        Err(StoreError::PersistenceConflict { .. })
    ));

    let projection = ContextProjectionRecord {
        projection_id: uuid::<ContextProjectionId>(40),
        session_id: child_session_id,
        context_revision: ContextRevision::new(1),
        source_revision: StateRevision::new(2),
        policy_digest: PolicyDigest::digest(b"policy"),
        inventory_generation: InventoryGeneration::new(1),
        digest: ContextDigest::digest(b"projection"),
        byte_budget: 4_096,
        expires_at: timestamp(),
    };
    repository
        .persist_context_projection(&projection)
        .expect("context projection persists");
    repository
        .persist_context_projection(&projection)
        .expect("same projection is idempotent");
    let mut changed_projection = projection.clone();
    changed_projection.digest = ContextDigest::digest(b"changed projection");
    assert!(matches!(
        repository.persist_context_projection(&changed_projection),
        Err(StoreError::PersistenceConflict { .. })
    ));
    let revision_collision = ContextProjectionRecord {
        projection_id: uuid::<ContextProjectionId>(41),
        ..projection.clone()
    };
    assert!(
        repository
            .persist_context_projection(&revision_collision)
            .is_err()
    );

    let compact = CompactCycleRecord {
        compact_id: uuid::<CompactId>(50),
        session_id: child_session_id,
        snapshot_ref: ".ae-sdd/context/snapshot.json".into(),
        previous_generation: ContextGeneration::new(1),
        next_generation: ContextGeneration::new(2),
        host_action_id: action_id,
        status: "host_acknowledged".into(),
        deadline: timestamp(),
        restored_digest: None,
    };
    repository
        .persist_compact_cycle(&compact)
        .expect("compact cycle persists");
    let mut restored_compact = compact.clone();
    restored_compact.status = "context_restored".into();
    restored_compact.restored_digest = Some(ContextDigest::digest(b"restored"));
    repository
        .persist_compact_cycle(&restored_compact)
        .expect("compact cycle mutable fields update");

    let checkpoint = SupervisorCheckpointRecord {
        workspace_id,
        work_item_id: work_item_id.clone(),
        last_event_sequence: EventSequence::new(2),
        last_event_digest: ArtifactDigest::digest(b"event-2"),
        state_revision: StateRevision::new(2),
        input_fingerprint: InputFingerprint::digest(b"input"),
        policy_digest: PolicyDigest::digest(b"policy"),
        last_decision_digest: DecisionDigest::digest(b"decision"),
        health: "healthy".into(),
        updated_at: timestamp(),
    };
    assert!(
        repository
            .supervisor_checkpoint(workspace_id, &work_item_id)
            .expect("missing checkpoint lookup succeeds")
            .is_none()
    );
    repository
        .persist_supervisor_checkpoint(&checkpoint)
        .expect("checkpoint persists");
    repository
        .persist_supervisor_checkpoint(&checkpoint)
        .expect("same checkpoint is idempotent");
    assert_eq!(
        repository
            .supervisor_checkpoint(workspace_id, &work_item_id)
            .expect("checkpoint lookup succeeds"),
        Some(checkpoint.clone())
    );
    let mut advanced_checkpoint = checkpoint.clone();
    advanced_checkpoint.last_event_sequence = EventSequence::new(3);
    advanced_checkpoint.last_event_digest = ArtifactDigest::digest(b"event-3");
    repository
        .persist_supervisor_checkpoint(&advanced_checkpoint)
        .expect("checkpoint advances");
    assert!(matches!(
        repository.persist_supervisor_checkpoint(&checkpoint),
        Err(StoreError::PersistenceConflict { .. })
    ));
    let mut same_cursor_conflict = advanced_checkpoint.clone();
    same_cursor_conflict.health = "degraded".into();
    assert!(matches!(
        repository.persist_supervisor_checkpoint(&same_cursor_conflict),
        Err(StoreError::PersistenceConflict { .. })
    ));

    let hook_receipt = HookEventReceipt {
        session_id: child_session_id,
        hook_event_id: "hook-1".into(),
        request_digest: InputFingerprint::digest(b"hook request"),
        decision_digest: ResultDigest::digest(b"hook decision"),
        event_sequence: Some(EventSequence::new(1)),
        created_at: timestamp(),
    };
    assert_eq!(
        repository
            .put_hook_event_receipt(&hook_receipt)
            .expect("hook receipt persists"),
        hook_receipt
    );
    assert_eq!(
        repository
            .put_hook_event_receipt(&hook_receipt)
            .expect("hook receipt replays"),
        hook_receipt
    );
    let mut changed_hook = hook_receipt.clone();
    changed_hook.request_digest = InputFingerprint::digest(b"changed hook request");
    assert!(matches!(
        repository.put_hook_event_receipt(&changed_hook),
        Err(StoreError::IdempotencyKeyReused { .. })
    ));
}

#[test]
fn in_memory_repository_enforces_the_full_runtime_contract() {
    let event_store_id = uuid::<EventStoreId>(1);
    let repository = InMemoryRuntimeRepository::new(event_store_id);

    assert_eq!(repository.event_store_id(), event_store_id);
    exercise_repository(&repository);
    assert_eq!(repository.events().len(), 2);
}

#[test]
fn sqlite_repository_enforces_the_full_runtime_contract() {
    let event_store_id = uuid::<EventStoreId>(2);
    let repository = SqliteRuntimeRepository::open_in_memory(event_store_id, &timestamp())
        .expect("SQLite repository opens");

    assert_eq!(repository.event_store_id(), event_store_id);
    exercise_repository(&repository);
}
