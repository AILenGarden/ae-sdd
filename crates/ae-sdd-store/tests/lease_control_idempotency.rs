mod support;

use std::str::FromStr;

use ae_sdd_domain::{BootId, InputFingerprint, OperationId, RequestId, SessionId, WorkItemId};
use ae_sdd_store::{
    IdempotencyKey, InMemoryRuntimeRepository, LeaseControlAction, LeaseControlRequest,
    LeaseLedger, LeaseOwner, LeaseProof, ProjectMutationStore, SqliteRuntimeRepository,
    StdCrossProcessLock, StdDurableFileSystem,
};
use uuid::Uuid;

use support::{at, fixture, lease_id};

fn request(
    fixture: &support::Fixture,
    number: u128,
    operation: &str,
    key: &str,
    payload: &[u8],
    action: LeaseControlAction,
) -> LeaseControlRequest {
    LeaseControlRequest {
        mutation_id: RequestId::from_uuid(Uuid::from_u128(number)),
        workspace_id: fixture.workspace_id,
        work_item_id: WorkItemId::new("WI-1").expect("work item"),
        operation: OperationId::new(operation).expect("operation"),
        idempotency_key: IdempotencyKey::new(key).expect("idempotency key"),
        canonical_payload_digest: InputFingerprint::digest(payload),
        action,
        boot_id: BootId::from_uuid(Uuid::from_u128(90)),
        session_id: Some(
            SessionId::from_str(&Uuid::from_u128(91).to_string()).expect("session identity"),
        ),
        committed_at: at("2026-07-23T00:00:01Z"),
    }
}

#[test]
fn acquire_replays_durably_and_rejects_same_key_with_another_payload() {
    let fixture = fixture();
    let store = ProjectMutationStore::new(
        fixture.paths.clone(),
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
    );
    let owner = LeaseOwner::new(Uuid::from_u128(91).to_string()).expect("owner");
    let acquire = request(
        &fixture,
        100,
        "lease.acquire",
        "lease-acquire-1",
        br#"{"owner":{"role":"root"},"ttlSeconds":300}"#,
        LeaseControlAction::Acquire {
            lease_id: lease_id(101),
            owner,
            now: at("2026-07-23T00:00:00Z"),
            expires_at: at("2026-07-23T00:05:00Z"),
        },
    );

    let first = store
        .commit_lease_control(acquire.clone())
        .expect("first acquire commits");
    let replay = store
        .commit_lease_control(acquire)
        .expect("exact retry replays");

    assert!(!first.mutation.replayed);
    assert!(replay.mutation.replayed);
    assert_eq!(first.data, replay.data);
    assert_eq!(first.data["leaseId"], lease_id(101).to_string());
    assert_eq!(first.mutation.receipt.revision_before.get(), 0);
    assert_eq!(first.mutation.receipt.revision_after.get(), 0);

    let conflict = request(
        &fixture,
        102,
        "lease.acquire",
        "lease-acquire-1",
        br#"{"owner":{"role":"root"},"ttlSeconds":600}"#,
        LeaseControlAction::Acquire {
            lease_id: lease_id(102),
            owner: LeaseOwner::new(Uuid::from_u128(91).to_string()).expect("owner"),
            now: at("2026-07-23T00:00:02Z"),
            expires_at: at("2026-07-23T00:10:02Z"),
        },
    );
    assert!(matches!(
        store.commit_lease_control(conflict),
        Err(ae_sdd_store::StoreError::IdempotencyKeyReused { .. })
    ));
}

#[test]
fn renew_and_release_replay_after_the_ledger_has_advanced() {
    let fixture = fixture();
    let store = ProjectMutationStore::new(
        fixture.paths.clone(),
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
    );
    let owner = LeaseOwner::new(Uuid::from_u128(91).to_string()).expect("owner");
    let lease_id = lease_id(110);
    let acquired = store
        .commit_lease_control(request(
            &fixture,
            110,
            "lease.acquire",
            "lease-acquire-2",
            b"acquire",
            LeaseControlAction::Acquire {
                lease_id,
                owner: owner.clone(),
                now: at("2026-07-23T00:00:00Z"),
                expires_at: at("2026-07-23T00:05:00Z"),
            },
        ))
        .expect("acquire");
    let proof = LeaseProof {
        lease_id,
        owner,
        fencing_token: ae_sdd_domain::FencingToken::new(
            acquired.data["fencingToken"].as_u64().expect("token"),
        ),
    };
    let renew = request(
        &fixture,
        111,
        "lease.renew",
        "lease-renew-2",
        b"renew",
        LeaseControlAction::Renew {
            proof: proof.clone(),
            now: at("2026-07-23T00:01:00Z"),
            expires_at: at("2026-07-23T00:10:00Z"),
        },
    );
    let renewed = store.commit_lease_control(renew.clone()).expect("renew");
    assert!(
        store
            .commit_lease_control(renew)
            .expect("renew replay")
            .mutation
            .replayed
    );

    let release = request(
        &fixture,
        112,
        "lease.release",
        "lease-release-2",
        b"release",
        LeaseControlAction::Release {
            proof,
            now: at("2026-07-23T00:02:00Z"),
        },
    );
    let released = store
        .commit_lease_control(release.clone())
        .expect("release");
    let replay = store.commit_lease_control(release).expect("release replay");
    assert_eq!(renewed.data["fencingToken"], released.data["fencingToken"]);
    assert_eq!(released.data, replay.data);
    assert!(replay.mutation.replayed);
    assert_eq!(released.data["status"], "released");
}

#[test]
fn acquire_replay_survives_repository_reopen() {
    let fixture = fixture();
    let database = fixture.temp.path().join("runtime.sqlite3");
    let owner = LeaseOwner::new(Uuid::from_u128(91).to_string()).expect("owner");
    let acquire = request(
        &fixture,
        120,
        "lease.acquire",
        "lease-acquire-durable",
        b"durable-acquire",
        LeaseControlAction::Acquire {
            lease_id: lease_id(121),
            owner,
            now: at("2026-07-23T00:00:00Z"),
            expires_at: at("2026-07-23T00:05:00Z"),
        },
    );

    let first = {
        let repository = SqliteRuntimeRepository::open(
            &database,
            fixture.event_store_id,
            &at("2026-07-23T00:00:00Z"),
        )
        .expect("repository opens");
        let store = ProjectMutationStore::new(
            fixture.paths.clone(),
            StdDurableFileSystem,
            StdCrossProcessLock,
            repository,
        );
        store
            .commit_lease_control(acquire.clone())
            .expect("acquire commits")
    };

    let reopened = SqliteRuntimeRepository::open(
        &database,
        fixture.event_store_id,
        &at("2026-07-23T00:00:01Z"),
    )
    .expect("repository reopens");
    let reopened_store = ProjectMutationStore::new(
        fixture.paths.clone(),
        StdDurableFileSystem,
        StdCrossProcessLock,
        reopened,
    );
    let replay = reopened_store
        .commit_lease_control(acquire)
        .expect("exact retry replays after reopen");

    assert!(!first.mutation.replayed);
    assert!(replay.mutation.replayed);
    assert_eq!(first.data, replay.data);
    assert_eq!(first.mutation.receipt, replay.mutation.receipt);
}

#[test]
fn admin_break_is_previewable_audited_and_durably_replayed() {
    let fixture = fixture();
    let store = ProjectMutationStore::new(
        fixture.paths.clone(),
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
    );
    let owner = LeaseOwner::new(Uuid::from_u128(91).to_string()).expect("owner");
    store
        .commit_lease_control(request(
            &fixture,
            130,
            "lease.acquire",
            "lease-acquire-before-break",
            b"acquire-before-break",
            LeaseControlAction::Acquire {
                lease_id: lease_id(131),
                owner,
                now: at("2026-07-23T00:00:00Z"),
                expires_at: at("2026-07-23T00:05:00Z"),
            },
        ))
        .expect("acquire");
    let before = std::fs::read(fixture.paths.lease_path()).expect("ledger before break");
    let actor = LeaseOwner::new("admin:operator").expect("admin actor");
    let mut break_request = request(
        &fixture,
        132,
        "lease.break",
        "lease-break-1",
        b"admin-break",
        LeaseControlAction::Break {
            actor,
            reason: "owner process is no longer alive".into(),
            now: at("2026-07-23T00:01:00Z"),
        },
    );
    break_request.session_id = None;

    let preview = store
        .preview_lease_control(&break_request)
        .expect("break preview");
    assert_eq!(preview.data["broken"], true);
    assert_eq!(
        std::fs::read(fixture.paths.lease_path()).expect("ledger after preview"),
        before
    );

    let first = store
        .commit_lease_control(break_request.clone())
        .expect("break commits");
    let replay = store
        .commit_lease_control(break_request)
        .expect("break replays");
    assert!(!first.mutation.replayed);
    assert!(replay.mutation.replayed);
    assert_eq!(first.data, replay.data);
    assert_eq!(first.data["status"], "broken");
    assert_eq!(first.data["actor"], "admin:operator");
    assert_eq!(first.data["reason"], "owner process is no longer alive");
    assert!(first.data["ledgerBeforeDigest"].as_str().is_some());
    assert!(first.data["ledgerAfterDigest"].as_str().is_some());
    let ledger = LeaseLedger::from_json(
        &std::fs::read(fixture.paths.lease_path()).expect("ledger after break"),
    )
    .expect("valid ledger");
    assert!(ledger.active().is_none());
    assert!(
        ledger
            .tombstones()
            .last()
            .expect("break tombstone")
            .reason
            .contains("admin:operator")
    );
}
