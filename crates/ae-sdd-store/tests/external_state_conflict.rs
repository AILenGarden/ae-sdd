mod support;

use std::fs;

use ae_sdd_store::{
    InMemoryRuntimeRepository, LeaseOwner, LeaseProof, ProjectMutationStore, StdCrossProcessLock,
    StdDurableFileSystem, StoreError,
};

#[test]
fn same_revision_with_changed_hash_is_an_explicit_external_conflict() {
    let fixture = support::fixture();
    let store = ProjectMutationStore::new(
        fixture.paths.clone(),
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
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
        104,
        "external-conflict",
        false,
    );
    fs::write(
        fixture.paths.state_path(),
        support::state_bytes(0, 0, "externally-edited"),
    )
    .expect("external edit is written");

    assert!(matches!(
        store.commit(request),
        Err(StoreError::ExternalStateConflict { .. })
    ));
    assert!(store.repository().events().is_empty());
}
