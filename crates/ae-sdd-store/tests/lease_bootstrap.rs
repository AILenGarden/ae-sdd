mod support;

use ae_sdd_domain::FencingToken;
use ae_sdd_store::{
    InMemoryRuntimeRepository, LeaseOwner, ProjectMutationStore, StdCrossProcessLock,
    StdDurableFileSystem,
};

#[test]
fn lease_acquire_and_break_do_not_require_an_existing_lease() {
    let fixture = support::fixture();
    let repository = InMemoryRuntimeRepository::new(fixture.event_store_id);
    let store = ProjectMutationStore::new(
        fixture.paths,
        StdDurableFileSystem,
        StdCrossProcessLock,
        repository,
    );

    let acquired = store
        .acquire_lease(
            support::lease_id(1),
            LeaseOwner::new("root-session").expect("owner is valid"),
            support::at("2026-07-23T00:00:00Z"),
            support::at("2026-07-23T00:05:00Z"),
        )
        .expect("bootstrap lease acquire succeeds");
    assert_eq!(acquired.fencing_token(), FencingToken::new(1));

    let tombstone = store
        .break_lease(
            LeaseOwner::new("operator").expect("actor is valid"),
            "explicit recovery",
            support::at("2026-07-23T00:01:00Z"),
        )
        .expect("bootstrap lease break succeeds")
        .expect("an active lease produces a tombstone");
    assert_eq!(tombstone.fencing_token, FencingToken::new(1));
}
