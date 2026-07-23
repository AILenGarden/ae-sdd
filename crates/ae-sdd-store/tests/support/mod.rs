#![allow(dead_code)]

use std::{fs, str::FromStr};

use ae_sdd_domain::{
    BootId, EventStoreId, InputFingerprint, LeaseId, OperationId, ProjectRelativePath, RequestId,
    ResultDigest, WorkItemId, WorkspaceId,
};
use ae_sdd_store::{
    AuthoritySnapshot, IdempotencyKey, JournalEvent, LeaseProof, MutationRequest, MutationTarget,
    ProjectStorePaths, RuntimeEventPayload, StateAuthority, UtcTimestamp,
};
use tempfile::TempDir;
use uuid::Uuid;

pub struct Fixture {
    pub temp: TempDir,
    pub paths: ProjectStorePaths,
    pub workspace_id: WorkspaceId,
    pub event_store_id: EventStoreId,
    pub expected: AuthoritySnapshot,
}

pub fn fixture() -> Fixture {
    let temp = tempfile::tempdir().expect("temp workspace is created");
    let relative =
        ProjectRelativePath::new(".auto-engineering/WI-1/state.json").expect("state path is valid");
    let absolute = temp.path().join(relative.as_str());
    fs::create_dir_all(absolute.parent().expect("state path has a parent"))
        .expect("state directory is created");
    let initial = state_bytes(0, 0, "before");
    fs::write(&absolute, &initial).expect("initial state is written");
    let paths = ProjectStorePaths::new(temp.path(), relative).expect("store paths are valid");
    Fixture {
        temp,
        paths,
        workspace_id: WorkspaceId::from_uuid(Uuid::from_u128(10)),
        event_store_id: EventStoreId::from_uuid(Uuid::from_u128(11)),
        expected: StateAuthority::inspect(&initial).expect("initial state is valid"),
    }
}

pub fn request(
    fixture: &Fixture,
    proof: LeaseProof,
    mutation_number: u128,
    idempotency_key: &str,
    include_artifact: bool,
) -> MutationRequest {
    let after = state_bytes(1, proof.fencing_token.get(), "after");
    let mut targets = vec![
        MutationTarget::new(
            fixture.paths.state_file().clone(),
            Some(fixture.expected.digest()),
            after,
        )
        .expect("state target is valid"),
    ];
    if include_artifact {
        targets.push(
            MutationTarget::new(
                ProjectRelativePath::new("ae-sdd-doc/Story/STORY-WI-1.md")
                    .expect("artifact path is valid"),
                None,
                b"committed story".to_vec(),
            )
            .expect("artifact target is valid"),
        );
    }
    let payload = serde_json::to_vec(&serde_json::json!({"kind":"state_committed"}))
        .expect("event payload serializes");
    MutationRequest {
        mutation_id: RequestId::from_uuid(Uuid::from_u128(mutation_number)),
        workspace_id: fixture.workspace_id,
        work_item_id: WorkItemId::new("WI-1").expect("work item ID is valid"),
        operation: OperationId::new("state.transition").expect("operation ID is valid"),
        idempotency_key: IdempotencyKey::new(idempotency_key).expect("key is valid"),
        canonical_payload_digest: InputFingerprint::digest(b"transition-to-coding"),
        expected_authority: fixture.expected,
        lease_proof: proof,
        targets,
        event: JournalEvent {
            boot_id: BootId::from_uuid(Uuid::from_u128(12)),
            session_id: None,
            event_type: "state.committed".into(),
            schema_version: 1,
            payload: RuntimeEventPayload::InlineJson(payload),
        },
        result_digest: ResultDigest::digest(b"transition-result"),
        prepared_at: at("2026-07-23T00:01:00Z"),
        committed_at: at("2026-07-23T00:01:01Z"),
    }
}

pub fn state_bytes(revision: u64, fencing: u64, value: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "lastFencingToken": fencing,
        "revision": revision,
        "value": value
    }))
    .expect("state fixture serializes")
}

pub fn at(value: &str) -> UtcTimestamp {
    UtcTimestamp::from_str(value).expect("timestamp fixture is valid")
}

pub fn lease_id(value: u128) -> LeaseId {
    LeaseId::from_uuid(Uuid::from_u128(value))
}
