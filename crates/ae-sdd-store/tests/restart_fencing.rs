mod support;

use std::fs;

use ae_sdd_store::{
    InMemoryRuntimeRepository, LeaseOwner, LeaseProof, ProjectMutationStore, StdCrossProcessLock,
    StdDurableFileSystem,
};

#[test]
fn expired_generation_cannot_commit_after_a_new_lease_is_acquired() {
    let fixture = support::fixture();
    let initial = fs::read(fixture.paths.state_path()).expect("initial state is readable");
    let store = ProjectMutationStore::new(
        fixture.paths.clone(),
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
    );
    let first = store
        .acquire_lease(
            support::lease_id(1),
            LeaseOwner::new("first-session").expect("owner is valid"),
            support::at("2026-07-23T00:00:00Z"),
            support::at("2026-07-23T00:01:00Z"),
        )
        .expect("first lease is acquired");
    store
        .acquire_lease(
            support::lease_id(2),
            LeaseOwner::new("second-session").expect("owner is valid"),
            support::at("2026-07-23T00:02:00Z"),
            support::at("2026-07-23T00:03:00Z"),
        )
        .expect("expired lease is replaced");

    let stale_request = support::request(
        &fixture,
        LeaseProof::from(&first),
        101,
        "stale-generation",
        false,
    );
    assert!(store.commit(stale_request).is_err());
    assert_eq!(
        fs::read(fixture.paths.state_path()).expect("state remains readable"),
        initial
    );
    assert!(store.repository().events().is_empty());
}
