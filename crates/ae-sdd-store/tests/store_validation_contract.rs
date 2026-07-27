mod support;

use std::{fs, str::FromStr};

use ae_sdd_domain::{
    ArtifactDigest, FencingToken, InputFingerprint, ProjectRelativePath, StateRevision,
};
use ae_sdd_store::{
    IdempotencyKey, InMemoryRuntimeRepository, LeaseLedger, LeaseOwner, LeaseProof, MutationTarget,
    ProjectMutationStore, ProjectStorePaths, RuntimeEventDraft, RuntimeEventPayload,
    StateAuthority, StdCrossProcessLock, StdDurableFileSystem, StoreError, UtcTimestamp,
};

fn project_store(
    fixture: &support::Fixture,
) -> ProjectMutationStore<StdDurableFileSystem, StdCrossProcessLock, InMemoryRuntimeRepository> {
    ProjectMutationStore::new(
        fixture.paths.clone(),
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
    )
}

fn acquire_proof(
    store: &ProjectMutationStore<
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository,
    >,
    number: u128,
) -> LeaseProof {
    let record = store
        .acquire_lease(
            support::lease_id(number),
            LeaseOwner::new(format!("owner:{number}")).expect("owner is valid"),
            support::at("2026-07-23T00:00:00Z"),
            support::at("2026-07-23T00:10:00Z"),
        )
        .expect("lease is acquired");
    LeaseProof::from(&record)
}

#[test]
fn state_authority_rejects_invalid_state_and_conflicting_successors() {
    let oversized = vec![b' '; 16 * 1024 * 1024 + 1];
    assert!(matches!(
        StateAuthority::inspect(&oversized),
        Err(StoreError::PayloadTooLarge { .. })
    ));
    assert!(matches!(
        StateAuthority::inspect(b"{"),
        Err(StoreError::InvalidState { .. })
    ));
    assert!(matches!(
        StateAuthority::inspect(b"[]"),
        Err(StoreError::InvalidState { .. })
    ));
    assert!(matches!(
        StateAuthority::inspect(br#"{"lastFencingToken":0}"#),
        Err(StoreError::InvalidState { .. })
    ));
    assert!(matches!(
        StateAuthority::inspect(br#"{"revision":0}"#),
        Err(StoreError::InvalidState { .. })
    ));

    let before_bytes = support::state_bytes(7, 3, "before");
    let before = StateAuthority::inspect(&before_bytes).expect("before state is valid");
    assert_eq!(before.revision(), StateRevision::new(7));
    assert_eq!(before.last_fencing_token(), FencingToken::new(3));
    assert_eq!(before.digest(), ArtifactDigest::digest(&before_bytes));
    StateAuthority::verify_unchanged(before, before).expect("same snapshot is unchanged");

    let other_revision = StateAuthority::inspect(&support::state_bytes(8, 3, "before"))
        .expect("other revision is valid");
    assert!(matches!(
        StateAuthority::verify_unchanged(before, other_revision),
        Err(StoreError::RevisionConflict { .. })
    ));
    let changed = StateAuthority::inspect(&support::state_bytes(7, 3, "changed"))
        .expect("changed state is valid");
    assert!(matches!(
        StateAuthority::verify_unchanged(before, changed),
        Err(StoreError::ExternalStateConflict { .. })
    ));

    let successor =
        StateAuthority::inspect(&support::state_bytes(8, 3, "after")).expect("successor is valid");
    StateAuthority::verify_successor(before, successor, FencingToken::new(3))
        .expect("valid successor passes");
    let skipped = StateAuthority::inspect(&support::state_bytes(9, 3, "after"))
        .expect("skipped state is valid");
    assert!(matches!(
        StateAuthority::verify_successor(before, skipped, FencingToken::new(3)),
        Err(StoreError::RevisionConflict { .. })
    ));
    assert!(matches!(
        StateAuthority::verify_successor(before, successor, FencingToken::new(4)),
        Err(StoreError::StaleFencingToken { .. })
    ));

    let exhausted = StateAuthority::inspect(&support::state_bytes(u64::MAX, 3, "exhausted"))
        .expect("maximum revision state is valid");
    assert!(matches!(
        StateAuthority::verify_successor(exhausted, successor, FencingToken::new(3)),
        Err(StoreError::InvalidState { .. })
    ));
}

#[test]
fn timestamp_key_and_runtime_event_validation_fail_closed() {
    let now = UtcTimestamp::now();
    assert_eq!(now.as_timestamp().to_string(), now.to_string());
    assert!(matches!(
        UtcTimestamp::from_str("not-a-timestamp"),
        Err(StoreError::InvalidState { .. })
    ));

    assert!(matches!(
        IdempotencyKey::new(""),
        Err(StoreError::InvalidIdempotencyKey { .. })
    ));
    assert!(matches!(
        IdempotencyKey::new("x".repeat(257)),
        Err(StoreError::InvalidIdempotencyKey { .. })
    ));
    assert!(matches!(
        IdempotencyKey::new("not portable"),
        Err(StoreError::InvalidIdempotencyKey { .. })
    ));

    assert!(matches!(
        RuntimeEventPayload::InlineJson(vec![b'0'; 64 * 1024 + 1]).validate(),
        Err(StoreError::PayloadTooLarge { .. })
    ));
    assert!(matches!(
        RuntimeEventPayload::InlineJson(b"not-json".to_vec()).validate(),
        Err(StoreError::InvalidJournal { .. })
    ));
    assert!(matches!(
        RuntimeEventPayload::InlineJson(br#"{ "value": 1 }"#.to_vec()).validate(),
        Err(StoreError::InvalidJournal { .. })
    ));
    assert!(
        RuntimeEventPayload::ArtifactRef {
            project_relative_path: "../escape.json".into(),
            digest: ArtifactDigest::digest(b"escape"),
            byte_length: 1,
        }
        .validate()
        .is_err()
    );
    assert!(matches!(
        RuntimeEventPayload::ArtifactRef {
            project_relative_path: "evidence/event.json".into(),
            digest: ArtifactDigest::digest(b"event"),
            byte_length: 0,
        }
        .validate(),
        Err(StoreError::InvalidJournal { .. })
    ));
    let artifact = RuntimeEventPayload::ArtifactRef {
        project_relative_path: "evidence/event.json".into(),
        digest: ArtifactDigest::digest(b"event"),
        byte_length: 5,
    };
    artifact.validate().expect("artifact reference is valid");
    assert_eq!(artifact.digest(), ArtifactDigest::digest(b"event"));
    assert_eq!(artifact.byte_length(), 5);

    let fixture = support::fixture();
    let mut draft = RuntimeEventDraft {
        boot_id: "00000000-0000-0000-0000-000000000012"
            .parse()
            .expect("boot ID parses"),
        workspace_id: fixture.workspace_id,
        session_id: None,
        work_item_id: "WI-1".parse().expect("work item parses"),
        event_type: "state.committed".into(),
        schema_version: 1,
        payload: RuntimeEventPayload::InlineJson(br#"{"ok":true}"#.to_vec()),
        committed_at: support::at("2026-07-23T00:01:00Z"),
    };
    draft.validate().expect("event draft is valid");
    draft.event_type = "bad event".into();
    assert!(matches!(
        draft.validate(),
        Err(StoreError::InvalidJournal { .. })
    ));
    draft.event_type = "state.committed".into();
    draft.schema_version = 0;
    assert!(matches!(
        draft.validate(),
        Err(StoreError::InvalidJournal { .. })
    ));
}

#[test]
fn lease_lifecycle_and_persisted_wire_validation_cover_negative_paths() {
    assert!(matches!(
        LeaseOwner::new(""),
        Err(StoreError::InvalidState { .. })
    ));
    assert!(matches!(
        LeaseOwner::new("x".repeat(257)),
        Err(StoreError::InvalidState { .. })
    ));
    assert!(matches!(
        LeaseOwner::new("owner\ncontrol"),
        Err(StoreError::InvalidState { .. })
    ));

    let mut ledger = LeaseLedger::empty(FencingToken::new(0));
    let owner = LeaseOwner::new("owner:root").expect("owner is valid");
    assert!(matches!(
        ledger.acquire(
            support::lease_id(100),
            owner.clone(),
            support::at("2026-07-23T00:01:00Z"),
            support::at("2026-07-23T00:01:00Z"),
        ),
        Err(StoreError::InvalidState { .. })
    ));
    let record = ledger
        .acquire(
            support::lease_id(100),
            owner.clone(),
            support::at("2026-07-23T00:01:00Z"),
            support::at("2026-07-23T00:05:00Z"),
        )
        .expect("lease is acquired");
    assert_eq!(record.owner(), &owner);
    assert_eq!(record.acquired_at(), &support::at("2026-07-23T00:01:00Z"));
    assert!(record.is_active_at(&support::at("2026-07-23T00:02:00Z")));
    assert!(matches!(
        ledger.acquire(
            support::lease_id(101),
            owner.clone(),
            support::at("2026-07-23T00:02:00Z"),
            support::at("2026-07-23T00:06:00Z"),
        ),
        Err(StoreError::LeaseConflict)
    ));

    let proof = LeaseProof::from(&record);
    assert!(matches!(
        ledger.renew(
            &proof,
            &support::at("2026-07-23T00:02:00Z"),
            support::at("2026-07-23T00:04:00Z"),
        ),
        Err(StoreError::InvalidState { .. })
    ));
    let mut wrong_owner = proof.clone();
    wrong_owner.owner = LeaseOwner::new("owner:other").expect("owner is valid");
    assert!(matches!(
        ledger.validate(&wrong_owner, &support::at("2026-07-23T00:02:00Z")),
        Err(StoreError::LeaseMismatch { .. })
    ));
    assert!(matches!(
        ledger.break_active(owner.clone(), "", support::at("2026-07-23T00:02:00Z")),
        Err(StoreError::InvalidState { .. })
    ));
    ledger
        .release(&proof, support::at("2026-07-23T00:02:00Z"))
        .expect("lease releases");
    assert!(
        ledger
            .break_active(
                owner.clone(),
                "already absent",
                support::at("2026-07-23T00:03:00Z")
            )
            .expect("breaking an absent lease succeeds")
            .is_none()
    );

    assert!(matches!(
        LeaseLedger::from_json(b"not-json"),
        Err(StoreError::InvalidState { .. })
    ));
    let lease_id = support::lease_id(200).to_string();
    let invalid_wires = [
        serde_json::json!({
            "schemaVersion": 2,
            "lastFencingToken": 0,
            "active": null,
            "tombstones": []
        }),
        serde_json::json!({
            "schemaVersion": 1,
            "lastFencingToken": 0,
            "active": {
                "leaseId": lease_id,
                "owner": "owner",
                "fencingToken": 1,
                "acquiredAt": "2026-07-23T00:00:00Z",
                "expiresAt": "2026-07-23T00:05:00Z"
            },
            "tombstones": []
        }),
        serde_json::json!({
            "schemaVersion": 1,
            "lastFencingToken": 1,
            "active": {
                "leaseId": support::lease_id(201).to_string(),
                "owner": "owner",
                "fencingToken": 1,
                "acquiredAt": "2026-07-23T00:05:00Z",
                "expiresAt": "2026-07-23T00:05:00Z"
            },
            "tombstones": []
        }),
        serde_json::json!({
            "schemaVersion": 1,
            "lastFencingToken": 1,
            "active": null,
            "tombstones": [{
                "leaseId": support::lease_id(202).to_string(),
                "owner": "owner",
                "fencingToken": 1,
                "endedAt": "2026-07-23T00:05:00Z",
                "reason": ""
            }]
        }),
    ];
    for wire in invalid_wires {
        let bytes = serde_json::to_vec(&wire).expect("wire serializes");
        assert!(matches!(
            LeaseLedger::from_json(&bytes),
            Err(StoreError::InvalidState { .. })
        ));
    }
}

#[test]
fn project_store_direct_lease_and_preflight_apis_are_side_effect_free_until_commit() {
    let fixture = support::fixture();
    let store = project_store(&fixture);
    assert_eq!(store.paths(), &fixture.paths);
    assert_eq!(
        store.paths().workspace_root(),
        fixture
            .temp
            .path()
            .canonicalize()
            .expect("workspace root canonicalizes")
    );
    assert_eq!(
        store.paths().journal_dir_relative().as_str(),
        ".auto-engineering/WI-1/mutation-journal/v1"
    );

    let proof = acquire_proof(&store, 300);
    store
        .validate_lease_proof(&proof, &support::at("2026-07-23T00:01:00Z"))
        .expect("active proof validates");
    let renewed = store
        .renew_lease(
            &proof,
            &support::at("2026-07-23T00:01:00Z"),
            support::at("2026-07-23T00:20:00Z"),
        )
        .expect("lease renews");
    let renewed_proof = LeaseProof::from(&renewed);

    let request = support::request(&fixture, renewed_proof.clone(), 301, "preflight", true);
    assert!(
        store
            .replay_committed(
                request.workspace_id,
                &request.work_item_id,
                &request.operation,
                &request.idempotency_key,
                request.canonical_payload_digest,
            )
            .expect("missing replay lookup succeeds")
            .is_none()
    );
    store
        .validate_mutation(&request)
        .expect("valid mutation preflight succeeds");
    assert!(store.repository().events().is_empty());
    assert!(!request.targets[1].path().as_str().is_empty());

    let committed = store.commit(request.clone()).expect("mutation commits");
    assert!(!committed.replayed);
    let replayed = store
        .replay_committed(
            request.workspace_id,
            &request.work_item_id,
            &request.operation,
            &request.idempotency_key,
            request.canonical_payload_digest,
        )
        .expect("committed replay lookup succeeds")
        .expect("committed replay exists");
    assert!(replayed.replayed);
    assert_eq!(replayed.receipt, committed.receipt);
    assert!(matches!(
        store.replay_committed(
            request.workspace_id,
            &request.work_item_id,
            &request.operation,
            &request.idempotency_key,
            InputFingerprint::digest(b"changed"),
        ),
        Err(StoreError::IdempotencyKeyReused { .. })
    ));

    store
        .release_lease(&renewed_proof, support::at("2026-07-23T00:02:00Z"))
        .expect("direct lease release succeeds");
    assert!(
        store
            .break_lease(
                LeaseOwner::new("admin").expect("admin owner is valid"),
                "already absent",
                support::at("2026-07-23T00:03:00Z"),
            )
            .expect("breaking absent lease succeeds")
            .is_none()
    );
}

#[test]
fn mutation_preflight_rejects_missing_stale_and_malformed_targets() {
    let fixture = support::fixture();
    let store = project_store(&fixture);
    let proof = acquire_proof(&store, 400);
    let base = support::request(&fixture, proof.clone(), 401, "negative-preflight", false);

    let mut no_targets = base.clone();
    no_targets.targets.clear();
    assert!(matches!(
        store.validate_mutation(&no_targets),
        Err(StoreError::InvalidJournal { .. })
    ));

    let mut duplicate = base.clone();
    duplicate.targets.push(duplicate.targets[0].clone());
    assert!(matches!(
        store.validate_mutation(&duplicate),
        Err(StoreError::InvalidJournal { .. })
    ));

    let mut without_state = base.clone();
    without_state.targets = vec![
        MutationTarget::new(
            ProjectRelativePath::new("evidence/result.txt").expect("path is valid"),
            None,
            b"result".to_vec(),
        )
        .expect("target is valid"),
    ];
    assert!(matches!(
        store.validate_mutation(&without_state),
        Err(StoreError::InvalidJournal { .. })
    ));

    let mut wrong_revision = base.clone();
    wrong_revision.targets = vec![
        MutationTarget::new(
            fixture.paths.state_file().clone(),
            Some(fixture.expected.digest()),
            support::state_bytes(2, proof.fencing_token.get(), "after"),
        )
        .expect("target is valid"),
    ];
    assert!(matches!(
        store.validate_mutation(&wrong_revision),
        Err(StoreError::RevisionConflict { .. })
    ));

    let mut wrong_fencing = base.clone();
    wrong_fencing.targets = vec![
        MutationTarget::new(
            fixture.paths.state_file().clone(),
            Some(fixture.expected.digest()),
            support::state_bytes(1, proof.fencing_token.get() + 1, "after"),
        )
        .expect("target is valid"),
    ];
    assert!(matches!(
        store.validate_mutation(&wrong_fencing),
        Err(StoreError::StaleFencingToken { .. })
    ));

    let mut invalid_event = base.clone();
    invalid_event.event.payload = RuntimeEventPayload::InlineJson(b"not-json".to_vec());
    assert!(matches!(
        store.validate_mutation(&invalid_event),
        Err(StoreError::InvalidJournal { .. })
    ));

    let missing_fixture = support::fixture();
    let missing_store = project_store(&missing_fixture);
    let missing_proof = LeaseProof {
        lease_id: support::lease_id(500),
        owner: LeaseOwner::new("owner:missing").expect("owner is valid"),
        fencing_token: FencingToken::new(1),
    };
    let missing_request =
        support::request(&missing_fixture, missing_proof, 501, "missing-state", false);
    fs::remove_file(missing_fixture.paths.state_path()).expect("state is removed");
    assert!(matches!(
        missing_store.validate_mutation(&missing_request),
        Err(StoreError::InvalidState { .. })
    ));

    let stale_fixture = support::fixture();
    let stale_store = project_store(&stale_fixture);
    let stale_proof = acquire_proof(&stale_store, 600);
    let stale_request =
        support::request(&stale_fixture, stale_proof, 601, "stale-authority", false);
    fs::write(
        stale_fixture.paths.state_path(),
        support::state_bytes(0, 2, "externally advanced fencing"),
    )
    .expect("state is replaced");
    assert!(matches!(
        stale_store.validate_mutation(&stale_request),
        Err(StoreError::StaleFencingToken { .. })
    ));
}

#[test]
fn project_store_paths_reject_non_directories_and_root_level_state_files() {
    let temp = tempfile::tempdir().expect("temporary directory is created");
    let file_root = temp.path().join("not-a-directory");
    fs::write(&file_root, b"file").expect("file root is written");
    assert!(matches!(
        ProjectStorePaths::new(
            &file_root,
            ProjectRelativePath::new("nested/state.json").expect("state path is valid")
        ),
        Err(StoreError::InvalidState { .. })
    ));
    assert!(matches!(
        ProjectStorePaths::new(
            temp.path().join("missing"),
            ProjectRelativePath::new("nested/state.json").expect("state path is valid")
        ),
        Err(StoreError::Io { .. })
    ));
    assert!(matches!(
        ProjectStorePaths::new(
            temp.path(),
            ProjectRelativePath::new("state.json").expect("state path is valid")
        ),
        Err(StoreError::InvalidState { .. })
    ));
}
