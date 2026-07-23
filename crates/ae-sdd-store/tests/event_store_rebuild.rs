mod support;

use std::fs;

use ae_sdd_domain::EventStoreId;
use ae_sdd_store::{
    LeaseOwner, LeaseProof, ProjectMutationStore, RuntimeRepository, SqliteRuntimeRepository,
    StdCrossProcessLock, StdDurableFileSystem,
};
use uuid::Uuid;

#[test]
fn deleting_runtime_database_rebuilds_receipt_and_event_from_committed_project_journal() {
    let fixture = support::fixture();
    let database = fixture.temp.path().join("runtime.sqlite3");
    let first_store_id = EventStoreId::from_uuid(Uuid::from_u128(300));
    let first = SqliteRuntimeRepository::open(
        &database,
        first_store_id,
        &support::at("2026-07-23T00:00:00Z"),
    )
    .expect("first runtime database opens");
    let store = ProjectMutationStore::new(
        fixture.paths.clone(),
        StdDurableFileSystem,
        StdCrossProcessLock,
        first,
    );
    let lease = store
        .acquire_lease(
            support::lease_id(1),
            LeaseOwner::new("root-session").expect("owner is valid"),
            support::at("2026-07-23T00:00:00Z"),
            support::at("2026-07-23T00:05:00Z"),
        )
        .expect("lease is acquired");
    let request = support::request(
        &fixture,
        LeaseProof::from(&lease),
        301,
        "rebuild-after-db-loss",
        false,
    );
    let committed = store.commit(request.clone()).expect("mutation commits");
    assert_eq!(committed.event.event_store_id, first_store_id);
    drop(store);

    fs::remove_file(&database).expect("runtime database is deleted");
    for suffix in ["-wal", "-shm"] {
        let sidecar = fixture.temp.path().join(format!("runtime.sqlite3{suffix}"));
        if sidecar.exists() {
            fs::remove_file(sidecar).expect("SQLite sidecar is deleted");
        }
    }

    let rebuilt_store_id = EventStoreId::from_uuid(Uuid::from_u128(302));
    let rebuilt_repository = SqliteRuntimeRepository::open(
        &database,
        rebuilt_store_id,
        &support::at("2026-07-23T00:02:00Z"),
    )
    .expect("replacement runtime database opens");
    let rebuilt = ProjectMutationStore::new(
        fixture.paths,
        StdDurableFileSystem,
        StdCrossProcessLock,
        rebuilt_repository,
    );

    let replay = rebuilt
        .commit(request)
        .expect("COMMITTED project journal rebuilds runtime metadata");
    assert!(replay.replayed);
    assert_eq!(replay.event.event_store_id, rebuilt_store_id);
    assert_eq!(replay.event.event_sequence.get(), 1);
    assert_eq!(rebuilt.repository().event_store_id(), rebuilt_store_id);
}
