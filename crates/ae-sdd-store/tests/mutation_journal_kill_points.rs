mod support;

use std::sync::atomic::{AtomicBool, Ordering};

use ae_sdd_store::{
    CommitFaultPort, CommitPoint, InMemoryRuntimeRepository, JournalStatus, LeaseOwner, LeaseProof,
    ProjectMutationStore, RecoveryDisposition, StdCrossProcessLock, StdDurableFileSystem,
    StoreError,
};

#[derive(Debug)]
struct FailOnceAt {
    point: CommitPoint,
    fired: AtomicBool,
}

impl FailOnceAt {
    const fn new(point: CommitPoint) -> Self {
        Self {
            point,
            fired: AtomicBool::new(false),
        }
    }
}

impl CommitFaultPort for FailOnceAt {
    fn reached(&self, point: CommitPoint) -> Result<(), StoreError> {
        if point == self.point && !self.fired.swap(true, Ordering::AcqRel) {
            return Err(StoreError::InjectedFault {
                point: "story_v008_kill_point",
            });
        }
        Ok(())
    }
}

#[test]
fn crashes_before_any_target_replace_recover_as_aborted() {
    for (index, point) in [
        CommitPoint::AfterPreparedJournal,
        CommitPoint::AfterStagedTargets,
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = support::fixture();
        let store = ProjectMutationStore::with_faults(
            fixture.paths.clone(),
            StdDurableFileSystem,
            StdCrossProcessLock,
            InMemoryRuntimeRepository::new(fixture.event_store_id),
            FailOnceAt::new(point),
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
            200 + u128::try_from(index).expect("fixture index fits u128"),
            &format!("pre-target-kill-{index}"),
            false,
        );
        assert!(matches!(
            store.commit(request),
            Err(StoreError::InjectedFault { .. })
        ));
        let restarted = ProjectMutationStore::new(
            fixture.paths,
            StdDurableFileSystem,
            StdCrossProcessLock,
            InMemoryRuntimeRepository::new(fixture.event_store_id),
        );
        let reports = restarted
            .recover(support::at("2026-07-23T00:02:00Z"))
            .expect("recovery succeeds");
        assert_eq!(reports.len(), 1);
        assert_eq!(
            reports[0].disposition,
            RecoveryDisposition::AbortedUnapplied
        );
    }
}

#[test]
fn crash_after_first_replace_completes_from_staged_files() {
    let fixture = support::fixture();
    let store = ProjectMutationStore::with_faults(
        fixture.paths.clone(),
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
        FailOnceAt::new(CommitPoint::AfterTargetReplace(0)),
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
        210,
        "partial-target-kill",
        true,
    );
    assert!(store.commit(request).is_err());
    let restarted = ProjectMutationStore::new(
        fixture.paths,
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
    );
    let reports = restarted
        .recover(support::at("2026-07-23T00:02:00Z"))
        .expect("recovery succeeds");
    assert_eq!(
        reports[0].disposition,
        RecoveryDisposition::CompletedFromStaged
    );
}

#[test]
fn crash_after_committed_journal_never_regresses_to_prepared_or_aborted() {
    let fixture = support::fixture();
    let store = ProjectMutationStore::with_faults(
        fixture.paths.clone(),
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
        FailOnceAt::new(CommitPoint::AfterCommittedJournal),
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
        220,
        "post-commit-kill",
        false,
    );
    assert!(store.commit(request.clone()).is_err());
    let restarted = ProjectMutationStore::new(
        fixture.paths,
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
    );
    let reports = restarted
        .recover(support::at("2026-07-23T00:02:00Z"))
        .expect("recovery succeeds");
    assert_eq!(
        reports[0].disposition,
        RecoveryDisposition::AlreadyTerminal(JournalStatus::Committed)
    );
    assert!(
        restarted
            .commit(request)
            .expect("receipt is rebuilt")
            .replayed
    );
}
