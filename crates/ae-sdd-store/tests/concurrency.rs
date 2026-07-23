mod support;

use std::{sync::Arc, thread};

use ae_sdd_store::{
    InMemoryRuntimeRepository, LeaseOwner, LeaseProof, ProjectMutationStore, RuntimeRepository,
    StdCrossProcessLock, StdDurableFileSystem,
};

#[test]
fn competing_retries_produce_one_commit_and_one_global_event() {
    let fixture = support::fixture();
    let repository = InMemoryRuntimeRepository::new(fixture.event_store_id);
    let store = Arc::new(ProjectMutationStore::new(
        fixture.paths.clone(),
        StdDurableFileSystem,
        StdCrossProcessLock,
        repository,
    ));
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
        100,
        "same-semantic-request",
        false,
    );

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let store = Arc::clone(&store);
            let request = request.clone();
            thread::spawn(move || store.commit(request).expect("retry is safe"))
        })
        .collect();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("worker joins"))
        .collect();

    assert_eq!(results.iter().filter(|result| !result.replayed).count(), 1);
    assert_eq!(results.iter().filter(|result| result.replayed).count(), 1);
    assert_eq!(store.repository().events().len(), 1);
    assert_eq!(store.repository().event_store_id(), fixture.event_store_id);
}
