mod support;

use std::sync::atomic::{AtomicBool, Ordering};

use ae_sdd_store::{
    CommitFaultPort, CommitPoint, InMemoryRuntimeRepository, LeaseOwner, LeaseProof,
    ProjectMutationStore, RecoveryDisposition, StdCrossProcessLock, StdDurableFileSystem,
    StoreError,
};

#[derive(Debug, Default)]
struct FailAfterFirstReplace(AtomicBool);

impl CommitFaultPort for FailAfterFirstReplace {
    fn reached(&self, point: CommitPoint) -> Result<(), StoreError> {
        if point == CommitPoint::AfterTargetReplace(0) && !self.0.swap(true, Ordering::AcqRel) {
            return Err(StoreError::InjectedFault {
                point: "after_target_replace",
            });
        }
        Ok(())
    }
}

#[test]
fn partial_target_commit_is_completed_from_verified_staged_content() {
    let fixture = support::fixture();
    let failing = ProjectMutationStore::with_faults(
        fixture.paths.clone(),
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
        FailAfterFirstReplace::default(),
    );
    let lease = failing
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
        102,
        "crash-recovery",
        true,
    );
    assert!(matches!(
        failing.commit(request.clone()),
        Err(StoreError::InjectedFault { .. })
    ));

    let recovered = ProjectMutationStore::new(
        fixture.paths.clone(),
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
    );
    let reports = recovered
        .recover(support::at("2026-07-23T00:02:00Z"))
        .expect("restart recovery succeeds");
    assert_eq!(reports.len(), 1);
    assert_eq!(
        reports[0].disposition,
        RecoveryDisposition::CompletedFromStaged
    );

    let replayed = recovered
        .commit(request)
        .expect("retry rebuilds SQLite receipt from committed project journal");
    assert!(replayed.replayed);
    assert_eq!(recovered.repository().events().len(), 1);
    assert_eq!(
        std::fs::read(fixture.temp.path().join("ae-sdd-doc/Story/STORY-WI-1.md"))
            .expect("artifact was recovered"),
        b"committed story"
    );
    let state = std::fs::read(fixture.paths.state_path()).expect("state is readable");
    assert_eq!(
        ae_sdd_store::StateAuthority::inspect(&state)
            .expect("state is valid")
            .revision()
            .get(),
        1
    );
}

#[derive(Debug, Default)]
struct FailAfterStaging;

impl CommitFaultPort for FailAfterStaging {
    fn reached(&self, point: CommitPoint) -> Result<(), StoreError> {
        if point == CommitPoint::AfterStagedTargets {
            return Err(StoreError::InjectedFault {
                point: "after_staged_targets",
            });
        }
        Ok(())
    }
}

#[test]
fn prepared_without_any_applied_target_is_aborted_on_restart() {
    let fixture = support::fixture();
    let failing = ProjectMutationStore::with_faults(
        fixture.paths.clone(),
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
        FailAfterStaging,
    );
    let lease = failing
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
        103,
        "abort-recovery",
        false,
    );
    assert!(failing.commit(request).is_err());

    let recovered = ProjectMutationStore::new(
        fixture.paths,
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
    );
    let reports = recovered
        .recover(support::at("2026-07-23T00:02:00Z"))
        .expect("recovery succeeds");
    assert_eq!(
        reports[0].disposition,
        RecoveryDisposition::AbortedUnapplied
    );
    assert!(recovered.repository().events().is_empty());
}
