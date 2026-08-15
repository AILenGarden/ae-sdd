mod support;

use std::{fs, str::FromStr};

use ae_sdd_domain::{
    ArtifactDigest, FencingToken, InputFingerprint, ProjectRelativePath, ResultDigest,
    StateRevision,
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
fn commit_replays_receipt_and_fails_closed_for_missing_or_stale_authority() {
    let fixture = support::fixture();
    let store = project_store(&fixture);
    let proof = acquire_proof(&store, 350);
    let request = support::request(&fixture, proof, 351, "commit-replay", false);
    assert!(
        store
            .committed_by_idempotency_key(request.workspace_id, &request.idempotency_key)
            .expect("missing committed-key lookup succeeds")
            .is_none()
    );

    let committed = store.commit(request.clone()).expect("mutation commits");
    let replayed = store
        .commit(request.clone())
        .expect("committed mutation replays");
    assert!(!committed.replayed);
    assert!(replayed.replayed);
    assert_eq!(replayed.receipt, committed.receipt);
    assert_eq!(replayed.event, committed.event);
    assert_eq!(replayed.journal_path, committed.journal_path);
    assert_eq!(store.repository().events().len(), 1);
    let by_key = store
        .committed_by_idempotency_key(request.workspace_id, &request.idempotency_key)
        .expect("committed-key lookup succeeds")
        .expect("committed mutation is found by key");
    assert!(by_key.replayed);
    assert_eq!(by_key.receipt, committed.receipt);

    let mut reused = request.clone();
    reused.canonical_payload_digest = InputFingerprint::digest(b"different payload");
    assert!(matches!(
        store.validate_mutation(&reused),
        Err(StoreError::IdempotencyKeyReused { expected, observed })
            if expected == request.canonical_payload_digest
                && observed == reused.canonical_payload_digest
    ));

    let missing_fixture = support::fixture();
    let missing_store = project_store(&missing_fixture);
    let missing_proof = acquire_proof(&missing_store, 352);
    let missing_request = support::request(
        &missing_fixture,
        missing_proof,
        353,
        "commit-missing-state",
        false,
    );
    fs::remove_file(missing_fixture.paths.state_path()).expect("state is removed");
    assert!(matches!(
        missing_store.commit(missing_request),
        Err(StoreError::InvalidState { .. })
    ));
    assert!(missing_store.repository().events().is_empty());

    let stale_fixture = support::fixture();
    let stale_store = project_store(&stale_fixture);
    let stale_proof = acquire_proof(&stale_store, 354);
    let stale_request = support::request(
        &stale_fixture,
        stale_proof.clone(),
        355,
        "commit-stale-fencing",
        false,
    );
    fs::write(
        stale_fixture.paths.state_path(),
        support::state_bytes(0, stale_proof.fencing_token.get() + 1, "advanced fencing"),
    )
    .expect("state fencing is advanced");
    assert!(matches!(
        stale_store.commit(stale_request),
        Err(StoreError::StaleFencingToken { minimum, observed })
            if minimum == FencingToken::new(stale_proof.fencing_token.get() + 1)
                && observed == stale_proof.fencing_token
    ));
    assert!(stale_store.repository().events().is_empty());
}

#[test]
fn commit_binds_an_initially_absent_delete_to_its_actual_outcome() {
    let fixture = support::fixture();
    let store = project_store(&fixture);
    let proof = acquire_proof(&store, 356);
    let mut request = support::request(&fixture, proof, 357, "already-absent-outcome", false);
    let draft = ProjectRelativePath::new("drafts/already-absent.md").expect("path is valid");
    request.targets.push(MutationTarget::delete(
        draft.clone(),
        ArtifactDigest::digest(b"captured draft"),
    ));
    let mut already_absent_event = request.event.clone();
    already_absent_event.event_type = "draft.already-absent".into();
    let already_absent_result = ResultDigest::digest(b"already absent");

    let committed = store
        .commit_with_delete_outcome(
            request,
            draft.clone(),
            already_absent_event,
            already_absent_result,
        )
        .expect("initially absent delete commits with its actual outcome");
    assert_eq!(committed.receipt.result_digest, already_absent_result);
    assert_eq!(
        committed.event.draft.event_type.as_ref(),
        "draft.already-absent"
    );
    assert!(!fixture.paths.resolve(&draft).exists());
}

#[test]
fn commit_and_preflight_reject_exhausted_authority_revision() {
    let fixture = support::fixture();
    let store = project_store(&fixture);
    let proof = acquire_proof(&store, 358);
    let exhausted_bytes = support::state_bytes(u64::MAX, proof.fencing_token.get(), "exhausted");
    fs::write(fixture.paths.state_path(), &exhausted_bytes).expect("exhausted state is written");
    let exhausted = StateAuthority::inspect(&exhausted_bytes).expect("exhausted state is valid");
    let mut request = support::request(&fixture, proof, 359, "exhausted-revision", false);
    request.expected_authority = exhausted;

    assert!(matches!(
        store.validate_mutation(&request),
        Err(StoreError::InvalidState { .. })
    ));
    assert!(matches!(
        store.commit(request),
        Err(StoreError::InvalidState { .. })
    ));
    assert!(store.repository().events().is_empty());
}

#[test]
fn commit_rejects_delete_outcome_that_is_not_a_mutation_target() {
    let fixture = support::fixture();
    let store = project_store(&fixture);
    let proof = acquire_proof(&store, 360);
    let request = support::request(&fixture, proof, 361, "unbound-delete-outcome", false);
    let unbound = ProjectRelativePath::new("drafts/not-a-target.md").expect("path is valid");

    let error = store
        .commit_with_delete_outcome(
            request.clone(),
            unbound,
            request.event.clone(),
            request.result_digest,
        )
        .expect_err("unbound delete outcome is rejected");
    assert!(
        matches!(error, StoreError::InvalidJournal { ref reason }
            if reason.as_ref() == "delete outcome path is not a mutation delete target"),
        "unexpected error: {error:?}"
    );
    assert_eq!(
        fs::read(fixture.paths.state_path()).expect("state remains readable"),
        support::state_bytes(0, 0, "before")
    );
    assert!(store.repository().events().is_empty());
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

    let conflicting_write_path =
        ProjectRelativePath::new("evidence/existing.txt").expect("conflicting write path is valid");
    let conflicting_write_absolute = fixture
        .paths
        .workspace_root()
        .join(conflicting_write_path.as_str());
    fs::create_dir_all(
        conflicting_write_absolute
            .parent()
            .expect("conflicting write has a parent"),
    )
    .expect("conflicting write directory is created");
    fs::write(&conflicting_write_absolute, b"external").expect("external write is created");
    let mut conflicting_write = base.clone();
    conflicting_write.targets.push(
        MutationTarget::new(conflicting_write_path, None, b"replacement".to_vec())
            .expect("write target is valid"),
    );
    let conflicting_write_error = store
        .validate_mutation(&conflicting_write)
        .expect_err("existing file must reject an unbound write");
    assert!(
        matches!(
            &conflicting_write_error,
            StoreError::JournalConflict { path } if path == &conflicting_write_absolute
        ),
        "unexpected write conflict mapping: {conflicting_write_error:?}"
    );

    let drifting_delete_path =
        ProjectRelativePath::new("drafts/drifting.md").expect("delete path is valid");
    let drifting_delete_absolute = fixture.temp.path().join(drifting_delete_path.as_str());
    fs::create_dir_all(
        drifting_delete_absolute
            .parent()
            .expect("delete target has a parent"),
    )
    .expect("delete target directory is created");
    fs::write(&drifting_delete_absolute, b"changed externally").expect("delete target is created");
    let mut drifting_delete = base.clone();
    drifting_delete.targets.push(MutationTarget::delete(
        drifting_delete_path,
        ArtifactDigest::digest(b"expected original"),
    ));
    assert!(matches!(
        store.validate_mutation(&drifting_delete),
        Err(StoreError::ExternalStateConflict { .. })
    ));

    let mut state_delete = base.clone();
    state_delete.targets = vec![MutationTarget::delete(
        fixture.paths.state_file().clone(),
        fixture.expected.digest(),
    )];
    assert!(matches!(
        store.validate_mutation(&state_delete),
        Err(StoreError::InvalidJournal { .. })
    ));

    let unbound_outcome_path =
        ProjectRelativePath::new("drafts/not-a-target.md").expect("outcome path is valid");
    assert!(matches!(
        store.commit_with_delete_outcome(
            base.clone(),
            unbound_outcome_path,
            base.event.clone(),
            base.result_digest,
        ),
        Err(StoreError::InvalidJournal { .. })
    ));

    let ledger_fixture = support::fixture();
    let ledger_store = project_store(&ledger_fixture);
    let ledger_proof = acquire_proof(&ledger_store, 700);
    fs::write(
        ledger_fixture.paths.state_path(),
        support::state_bytes(0, ledger_proof.fencing_token.get() + 1, "advanced fencing"),
    )
    .expect("state fencing is advanced");
    assert!(matches!(
        ledger_store.validate_lease_proof(&ledger_proof, &support::at("2026-07-23T00:01:00Z")),
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
