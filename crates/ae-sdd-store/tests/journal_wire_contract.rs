mod support;

use ae_sdd_domain::{
    ArtifactDigest, BootId, FencingToken, InputFingerprint, OperationId, ProjectRelativePath,
    RequestId, ResultDigest, SessionId, StateRevision, WorkItemId, WorkspaceId,
};
use ae_sdd_store::{
    DurableFileSystem, IdempotencyKey, InMemoryFileSystem, JournalEvent, JournalStatus,
    MutationJournalEntry, MutationTarget, RuntimeEventPayload, StdDurableFileSystem, StoreError,
    TargetDescriptor,
};
use serde_json::Value;
use uuid::Uuid;

fn descriptor() -> TargetDescriptor {
    TargetDescriptor::write(
        ProjectRelativePath::new(".auto-engineering/WI-1/state.json")
            .expect("target path is valid"),
        Some(ArtifactDigest::digest(b"before")),
        ArtifactDigest::digest(b"after"),
        5,
        ProjectRelativePath::new(".auto-engineering/WI-1/mutation-journal/v1/staged/request/0.bin")
            .expect("staged path is valid"),
    )
}

fn event(payload: RuntimeEventPayload) -> JournalEvent {
    JournalEvent {
        boot_id: BootId::from_uuid(Uuid::from_u128(10)),
        session_id: Some(SessionId::from_uuid(Uuid::from_u128(11))),
        event_type: "state.committed".into(),
        schema_version: 1,
        payload,
    }
}

fn prepared(payload: RuntimeEventPayload) -> MutationJournalEntry {
    MutationJournalEntry::prepared(
        RequestId::from_uuid(Uuid::from_u128(12)),
        WorkspaceId::from_uuid(Uuid::from_u128(13)),
        WorkItemId::new("WI-1").expect("work item is valid"),
        OperationId::new("state.transition").expect("operation is valid"),
        &IdempotencyKey::new("journal-1").expect("idempotency key is valid"),
        InputFingerprint::digest(b"payload"),
        ResultDigest::digest(b"result"),
        StateRevision::new(0),
        StateRevision::new(1),
        FencingToken::new(1),
        vec![descriptor()],
        event(payload),
        support::at("2026-07-23T00:00:00Z"),
    )
    .expect("journal prepares")
}

fn json(entry: &MutationJournalEntry) -> Value {
    serde_json::from_slice(
        &entry
            .to_canonical_json()
            .expect("journal serializes canonically"),
    )
    .expect("journal JSON parses")
}

fn assert_invalid(value: Value) {
    let bytes = serde_json::to_vec(&value).expect("mutated JSON serializes");
    assert!(MutationJournalEntry::from_json(&bytes).is_err());
}

#[test]
fn journal_lifecycle_enforces_target_revision_and_terminal_contracts() {
    let target_path = ProjectRelativePath::new("evidence/result.txt").expect("path is valid");
    assert!(matches!(
        MutationTarget::new(target_path.clone(), None, Vec::new()),
        Err(StoreError::InvalidJournal { .. })
    ));
    let target = MutationTarget::new(target_path.clone(), None, b"result".to_vec())
        .expect("target is valid");
    assert_eq!(target.path(), &target_path);
    assert_eq!(target.before_digest(), None);
    assert_eq!(target.after_bytes(), b"result");
    assert_eq!(target.after_digest(), ArtifactDigest::digest(b"result"));

    let idempotency_key = IdempotencyKey::new("journal-1").expect("key is valid");
    let args = || {
        (
            RequestId::from_uuid(Uuid::from_u128(12)),
            WorkspaceId::from_uuid(Uuid::from_u128(13)),
            WorkItemId::new("WI-1").expect("work item is valid"),
            OperationId::new("state.transition").expect("operation is valid"),
            InputFingerprint::digest(b"payload"),
            ResultDigest::digest(b"result"),
        )
    };
    let (mutation_id, workspace_id, work_item_id, operation, payload_digest, result_digest) =
        args();
    assert!(matches!(
        MutationJournalEntry::prepared(
            mutation_id,
            workspace_id,
            work_item_id.clone(),
            operation.clone(),
            &idempotency_key,
            payload_digest,
            result_digest,
            StateRevision::new(0),
            StateRevision::new(1),
            FencingToken::new(1),
            Vec::new(),
            event(RuntimeEventPayload::InlineJson(br#"{"ok":true}"#.to_vec())),
            support::at("2026-07-23T00:00:00Z"),
        ),
        Err(StoreError::InvalidJournal { .. })
    ));

    let too_many_targets = vec![descriptor(); 129];
    assert!(matches!(
        MutationJournalEntry::prepared(
            mutation_id,
            workspace_id,
            work_item_id.clone(),
            operation.clone(),
            &idempotency_key,
            payload_digest,
            result_digest,
            StateRevision::new(0),
            StateRevision::new(1),
            FencingToken::new(1),
            too_many_targets,
            event(RuntimeEventPayload::InlineJson(br#"{"ok":true}"#.to_vec())),
            support::at("2026-07-23T00:00:00Z"),
        ),
        Err(StoreError::InvalidJournal { .. })
    ));
    assert!(matches!(
        MutationJournalEntry::prepared(
            mutation_id,
            workspace_id,
            work_item_id.clone(),
            operation.clone(),
            &idempotency_key,
            payload_digest,
            result_digest,
            StateRevision::new(0),
            StateRevision::new(2),
            FencingToken::new(1),
            vec![descriptor()],
            event(RuntimeEventPayload::InlineJson(br#"{"ok":true}"#.to_vec())),
            support::at("2026-07-23T00:00:00Z"),
        ),
        Err(StoreError::InvalidJournal { .. })
    ));
    assert!(matches!(
        MutationJournalEntry::prepared(
            mutation_id,
            workspace_id,
            work_item_id,
            operation,
            &idempotency_key,
            payload_digest,
            result_digest,
            StateRevision::new(u64::MAX),
            StateRevision::new(0),
            FencingToken::new(1),
            vec![descriptor()],
            event(RuntimeEventPayload::InlineJson(br#"{"ok":true}"#.to_vec())),
            support::at("2026-07-23T00:00:00Z"),
        ),
        Err(StoreError::InvalidJournal { .. })
    ));

    let mut committed = prepared(RuntimeEventPayload::InlineJson(br#"{"ok":true}"#.to_vec()));
    assert!(matches!(
        committed.operation_receipt(idempotency_key.clone()),
        Err(StoreError::InvalidJournal { .. })
    ));
    committed
        .commit(support::at("2026-07-23T00:00:01Z"))
        .expect("prepared journal commits");
    assert!(matches!(
        committed.commit(support::at("2026-07-23T00:00:02Z")),
        Err(StoreError::InvalidJournal { .. })
    ));
    assert!(matches!(
        committed.abort(
            support::at("2026-07-23T00:00:02Z"),
            "cannot abort committed"
        ),
        Err(StoreError::InvalidJournal { .. })
    ));
    assert!(matches!(
        committed.operation_receipt(IdempotencyKey::new("wrong-key").expect("key is valid")),
        Err(StoreError::IdempotencyKeyReused { .. })
    ));
    let receipt = committed
        .operation_receipt(idempotency_key)
        .expect("committed receipt loads");
    assert_eq!(receipt.revision_after, StateRevision::new(1));

    let mut aborted = prepared(RuntimeEventPayload::InlineJson(br#"{"ok":true}"#.to_vec()));
    assert!(matches!(
        aborted.abort(support::at("2026-07-23T00:00:01Z"), ""),
        Err(StoreError::InvalidJournal { .. })
    ));
    aborted
        .abort(
            support::at("2026-07-23T00:00:01Z"),
            "restart before replacement",
        )
        .expect("prepared journal aborts");
    assert!(matches!(
        aborted.abort(support::at("2026-07-23T00:00:02Z"), "cannot abort twice"),
        Err(StoreError::InvalidJournal { .. })
    ));
    assert!(matches!(
        aborted.commit(support::at("2026-07-23T00:00:02Z")),
        Err(StoreError::InvalidJournal { .. })
    ));
}

#[test]
fn mutation_targets_distinguish_write_and_digest_bound_delete() {
    let path = ProjectRelativePath::new("drafts/STORY-001.md").unwrap();
    let before = ArtifactDigest::digest(b"draft");
    let write = MutationTarget::write(path.clone(), Some(before), b"saved".to_vec()).unwrap();
    assert_eq!(
        write,
        MutationTarget::new(path.clone(), Some(before), b"saved".to_vec()).unwrap()
    );
    assert_eq!(write.write_bytes(), Some(b"saved".as_slice()));
    assert!(!write.is_delete());

    let delete = MutationTarget::delete(path.clone(), before);
    assert_eq!(delete.path(), &path);
    assert_eq!(delete.before_digest(), Some(before));
    assert_eq!(delete.write_bytes(), None);
    assert!(delete.is_delete());
}

#[test]
fn durable_file_removal_reports_removed_then_already_absent() {
    let memory = InMemoryFileSystem::default();
    let memory_path = std::path::Path::new("drafts/STORY-001.md");
    memory.insert(memory_path, b"draft".to_vec());
    assert!(memory.remove_file_durable(memory_path).unwrap());
    assert!(!memory.remove_file_durable(memory_path).unwrap());

    let workspace = tempfile::tempdir().unwrap();
    let disk_path = workspace.path().join("drafts/STORY-001.md");
    std::fs::create_dir_all(disk_path.parent().unwrap()).unwrap();
    std::fs::write(&disk_path, b"draft").unwrap();
    let disk = StdDurableFileSystem;
    assert!(disk.remove_file_durable(&disk_path).unwrap());
    assert!(!disk.remove_file_durable(&disk_path).unwrap());
}

#[test]
fn journal_wire_round_trips_all_statuses_and_payload_variants() {
    let prepared_inline = prepared(RuntimeEventPayload::InlineJson(br#"{"ok":true}"#.to_vec()));
    let inline_bytes = prepared_inline
        .to_canonical_json()
        .expect("inline journal serializes");
    assert_eq!(
        MutationJournalEntry::from_json(&inline_bytes).expect("inline journal parses"),
        prepared_inline
    );

    let artifact_payload = RuntimeEventPayload::ArtifactRef {
        project_relative_path: "evidence/runtime-event.json".into(),
        digest: ArtifactDigest::digest(b"runtime event"),
        byte_length: 13,
    };
    let prepared_artifact = prepared(artifact_payload);
    let artifact_bytes = prepared_artifact
        .to_canonical_json()
        .expect("artifact journal serializes");
    assert_eq!(
        MutationJournalEntry::from_json(&artifact_bytes).expect("artifact journal parses"),
        prepared_artifact
    );

    let mut committed = prepared(RuntimeEventPayload::InlineJson(br#"{"ok":true}"#.to_vec()));
    committed
        .commit(support::at("2026-07-23T00:00:01Z"))
        .expect("journal commits");
    assert_eq!(
        MutationJournalEntry::from_json(
            &committed
                .to_canonical_json()
                .expect("committed journal serializes")
        )
        .expect("committed journal parses")
        .status,
        JournalStatus::Committed
    );

    let mut aborted = prepared(RuntimeEventPayload::InlineJson(br#"{"ok":true}"#.to_vec()));
    aborted
        .abort(
            support::at("2026-07-23T00:00:01Z"),
            "restart before replacement",
        )
        .expect("journal aborts");
    assert_eq!(
        MutationJournalEntry::from_json(
            &aborted
                .to_canonical_json()
                .expect("aborted journal serializes")
        )
        .expect("aborted journal parses")
        .status,
        JournalStatus::Aborted
    );
}

#[test]
fn journal_wire_reads_v1_and_round_trips_v2_tagged_targets() {
    let mut v1 = json(&prepared(RuntimeEventPayload::InlineJson(
        br#"{"ok":true}"#.to_vec(),
    )));
    v1["schemaVersion"] = 1.into();
    v1["targetFiles"][0]
        .as_object_mut()
        .expect("legacy target is an object")
        .remove("kind");
    assert_eq!(v1["schemaVersion"], 1);
    assert!(v1["targetFiles"][0].get("kind").is_none());
    MutationJournalEntry::from_json(&serde_json::to_vec(&v1).unwrap())
        .expect("legacy v1 write target remains readable");

    let mut v2 = v1;
    v2["schemaVersion"] = 2.into();
    v2["targetFiles"][0]["kind"] = "write".into();
    let deleted_path = "drafts/STORY-001.md";
    let expected_before = ArtifactDigest::digest(b"draft");
    v2["targetFiles"]
        .as_array_mut()
        .expect("targetFiles is an array")
        .push(serde_json::json!({
            "kind":"delete",
            "path":deleted_path,
            "expectedBeforeDigest":expected_before.to_string(),
        }));

    let parsed = MutationJournalEntry::from_json(&serde_json::to_vec(&v2).unwrap())
        .expect("v2 tagged write/delete targets parse");
    let round_trip = json(&parsed);
    assert_eq!(round_trip["schemaVersion"], 2);
    assert_eq!(round_trip["targetFiles"][0]["kind"], "write");
    assert_eq!(round_trip["targetFiles"][1]["kind"], "delete");
    assert_eq!(round_trip["targetFiles"][1]["path"], deleted_path);
    assert_eq!(
        round_trip["targetFiles"][1]["expectedBeforeDigest"],
        expected_before.to_string()
    );
    assert!(round_trip["targetFiles"][1].get("stagedRef").is_none());
    assert!(round_trip["targetFiles"][1].get("afterDigest").is_none());
}

#[test]
fn journal_wire_rejects_invalid_identity_targets_payloads_and_terminal_fields() {
    let base = prepared(RuntimeEventPayload::InlineJson(br#"{"ok":true}"#.to_vec()));

    let mut invalid = json(&base);
    invalid["schemaVersion"] = 3.into();
    assert_invalid(invalid);

    let mut invalid = json(&base);
    invalid["targetFiles"] = Value::Array(Vec::new());
    assert_invalid(invalid);

    for (field, value) in [
        ("path", "../escape.json"),
        ("beforeDigest", "not-a-digest"),
        ("afterDigest", "not-a-digest"),
        ("stagedRef", "../escape.bin"),
    ] {
        let mut invalid = json(&base);
        invalid["targetFiles"][0][field] = value.into();
        assert_invalid(invalid);
    }

    for field in [
        "idempotencyKeyDigest",
        "canonicalPayloadDigest",
        "plannedResultDigest",
    ] {
        let mut invalid = json(&base);
        invalid[field] = "not-a-digest".into();
        assert_invalid(invalid);
    }

    let mut invalid = json(&base);
    invalid["mutationId"] = "not-a-uuid".into();
    assert_invalid(invalid);
    let mut invalid = json(&base);
    invalid["workspaceId"] = "not-a-uuid".into();
    assert_invalid(invalid);
    let mut invalid = json(&base);
    invalid["event"]["bootId"] = "not-a-uuid".into();
    assert_invalid(invalid);
    let mut invalid = json(&base);
    invalid["event"]["sessionId"] = "not-a-uuid".into();
    assert_invalid(invalid);
    let mut invalid = json(&base);
    invalid["preparedAt"] = "not-a-timestamp".into();
    assert_invalid(invalid);

    let mut invalid = json(&base);
    invalid["event"]["payloadDigest"] = "not-a-digest".into();
    assert_invalid(invalid);
    let mut invalid = json(&base);
    invalid["event"]["byteLen"] = 999.into();
    assert_invalid(invalid);
    let mut invalid = json(&base);
    invalid["event"]["payloadRef"] = "evidence/duplicate.json".into();
    assert_invalid(invalid);
    let mut invalid = json(&base);
    invalid["event"]
        .as_object_mut()
        .expect("event is an object")
        .remove("payloadJson");
    assert_invalid(invalid);

    let mut invalid = json(&base);
    invalid["receipt"] = serde_json::json!({
        "resultDigest": ResultDigest::digest(b"result").to_string(),
        "committedAt": "2026-07-23T00:00:01Z"
    });
    assert_invalid(invalid);

    let mut committed = prepared(RuntimeEventPayload::InlineJson(br#"{"ok":true}"#.to_vec()));
    committed
        .commit(support::at("2026-07-23T00:00:01Z"))
        .expect("journal commits");
    let mut invalid = json(&committed);
    invalid
        .as_object_mut()
        .expect("journal is an object")
        .remove("receipt");
    assert_invalid(invalid);
    let mut invalid = json(&committed);
    invalid["receipt"]["resultDigest"] = "not-a-digest".into();
    assert_invalid(invalid);

    let mut aborted = prepared(RuntimeEventPayload::InlineJson(br#"{"ok":true}"#.to_vec()));
    aborted
        .abort(
            support::at("2026-07-23T00:00:01Z"),
            "restart before replacement",
        )
        .expect("journal aborts");
    let mut invalid = json(&aborted);
    invalid
        .as_object_mut()
        .expect("journal is an object")
        .remove("abortReason");
    assert_invalid(invalid);
}
