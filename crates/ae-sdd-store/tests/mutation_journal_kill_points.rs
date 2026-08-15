mod support;

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use ae_sdd_domain::{ArtifactDigest, ProjectRelativePath};
use ae_sdd_store::{
    CommitFaultPort, CommitPoint, DurableFileSystem, InMemoryRuntimeRepository, JournalStatus,
    LeaseOwner, LeaseProof, MutationTarget, ProjectMutationStore, RecoveryDisposition,
    StdCrossProcessLock, StdDurableFileSystem, StoreError,
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

#[derive(Debug)]
struct ReappearingFileSystem {
    target: PathBuf,
    reads: AtomicUsize,
    reappear_on_read: usize,
}

impl ReappearingFileSystem {
    fn new(target: PathBuf, reappear_on_read: usize) -> Self {
        Self {
            target,
            reads: AtomicUsize::new(0),
            reappear_on_read,
        }
    }
}

impl DurableFileSystem for ReappearingFileSystem {
    fn read(&self, path: &Path) -> Result<Option<Vec<u8>>, StoreError> {
        if path == self.target && self.reads.fetch_add(1, Ordering::AcqRel) == self.reappear_on_read
        {
            StdDurableFileSystem.write_atomic_durable(path, b"replacement draft")?;
        }
        StdDurableFileSystem.read(path)
    }

    fn write_atomic_durable(&self, path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
        StdDurableFileSystem.write_atomic_durable(path, bytes)
    }

    fn remove_file_durable(&self, path: &Path) -> Result<bool, StoreError> {
        StdDurableFileSystem.remove_file_durable(path)
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), StoreError> {
        StdDurableFileSystem.create_dir_all(path)
    }

    fn sync_directory(&self, path: &Path) -> Result<(), StoreError> {
        StdDurableFileSystem.sync_directory(path)
    }

    fn list_files(&self, path: &Path) -> Result<Vec<PathBuf>, StoreError> {
        StdDurableFileSystem.list_files(path)
    }
}

#[derive(Debug)]
struct DisappearingFileSystem {
    target: PathBuf,
}

impl DurableFileSystem for DisappearingFileSystem {
    fn read(&self, path: &Path) -> Result<Option<Vec<u8>>, StoreError> {
        StdDurableFileSystem.read(path)
    }

    fn write_atomic_durable(&self, path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
        StdDurableFileSystem.write_atomic_durable(path, bytes)
    }

    fn remove_file_durable(&self, path: &Path) -> Result<bool, StoreError> {
        if path == self.target {
            StdDurableFileSystem.remove_file_durable(path)?;
        }
        StdDurableFileSystem.remove_file_durable(path)
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), StoreError> {
        StdDurableFileSystem.create_dir_all(path)
    }

    fn sync_directory(&self, path: &Path) -> Result<(), StoreError> {
        StdDurableFileSystem.sync_directory(path)
    }

    fn list_files(&self, path: &Path) -> Result<Vec<PathBuf>, StoreError> {
        StdDurableFileSystem.list_files(path)
    }
}

#[test]
fn initially_absent_delete_target_commits_as_already_applied() {
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
    let draft = ProjectRelativePath::new(".hermes/already-absent.md").unwrap();
    let mut request = support::request(
        &fixture,
        LeaseProof::from(&lease),
        250,
        "already-absent-delete",
        false,
    );
    request.targets.push(MutationTarget::delete(
        draft.clone(),
        ArtifactDigest::digest(b"captured draft"),
    ));

    store
        .commit(request)
        .expect("an absent delete target is already applied");
    assert!(!fixture.temp.path().join(draft.as_str()).exists());
}

#[test]
fn delete_target_reappearing_after_absent_validation_is_rejected() {
    let fixture = support::fixture();
    let draft = ProjectRelativePath::new(".hermes/reappearing-draft.md").unwrap();
    let draft_path = fixture.paths.resolve(&draft);
    std::fs::create_dir_all(draft_path.parent().unwrap()).unwrap();
    let store = ProjectMutationStore::new(
        fixture.paths.clone(),
        ReappearingFileSystem::new(draft_path.clone(), 1),
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
    let mut request = support::request(
        &fixture,
        LeaseProof::from(&lease),
        251,
        "reappearing-delete",
        false,
    );
    request.targets.push(MutationTarget::delete(
        draft,
        ArtifactDigest::digest(b"captured draft"),
    ));

    let result = store.commit(request);
    assert!(
        matches!(result, Err(StoreError::ExternalStateConflict { .. })),
        "{result:?}"
    );
    assert_eq!(std::fs::read(draft_path).unwrap(), b"replacement draft");
}

#[test]
fn initially_absent_delete_target_reappearing_during_apply_is_rejected() {
    let fixture = support::fixture();
    let draft = ProjectRelativePath::new(".hermes/apply-reappearing-draft.md").unwrap();
    let draft_path = fixture.paths.resolve(&draft);
    std::fs::create_dir_all(draft_path.parent().unwrap()).unwrap();
    let store = ProjectMutationStore::new(
        fixture.paths.clone(),
        ReappearingFileSystem::new(draft_path.clone(), 2),
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
    );
    let lease = store
        .acquire_lease(
            support::lease_id(2),
            LeaseOwner::new("root-session").expect("owner is valid"),
            support::at("2026-07-23T00:00:00Z"),
            support::at("2026-07-23T00:05:00Z"),
        )
        .expect("lease is acquired");
    let mut request = support::request(
        &fixture,
        LeaseProof::from(&lease),
        252,
        "apply-reappearing-delete",
        false,
    );
    request.targets.push(MutationTarget::delete(
        draft,
        ArtifactDigest::digest(b"captured draft"),
    ));

    let result = store.commit(request);
    assert!(
        matches!(
            result,
            Err(StoreError::ExternalStateConflict {
                revision,
                expected_digest,
                observed_digest,
            }) if revision.get() == 0
                && expected_digest == ArtifactDigest::digest(b"captured draft")
                && observed_digest == ArtifactDigest::digest(b"replacement draft")
        ),
        "{result:?}"
    );
    assert_eq!(std::fs::read(draft_path).unwrap(), b"replacement draft");
    assert!(store.repository().events().is_empty());
}

#[test]
fn expected_delete_target_disappearing_during_apply_is_rejected() {
    let fixture = support::fixture();
    let draft = ProjectRelativePath::new(".hermes/apply-disappearing-draft.md").unwrap();
    let draft_path = fixture.paths.resolve(&draft);
    std::fs::create_dir_all(draft_path.parent().unwrap()).unwrap();
    std::fs::write(&draft_path, b"captured draft").unwrap();
    let store = ProjectMutationStore::new(
        fixture.paths.clone(),
        DisappearingFileSystem {
            target: draft_path.clone(),
        },
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
    );
    let lease = store
        .acquire_lease(
            support::lease_id(3),
            LeaseOwner::new("root-session").expect("owner is valid"),
            support::at("2026-07-23T00:00:00Z"),
            support::at("2026-07-23T00:05:00Z"),
        )
        .expect("lease is acquired");
    let mut request = support::request(
        &fixture,
        LeaseProof::from(&lease),
        253,
        "apply-disappearing-delete",
        false,
    );
    request.targets.push(MutationTarget::delete(
        draft,
        ArtifactDigest::digest(b"captured draft"),
    ));

    let result = store.commit(request);
    assert!(
        matches!(
            result,
            Err(StoreError::ExternalStateConflict {
                revision,
                expected_digest,
                observed_digest,
            }) if revision.get() == 0
                && expected_digest == ArtifactDigest::digest(b"captured draft")
                && observed_digest == ArtifactDigest::digest([])
        ),
        "{result:?}"
    );
    assert!(!draft_path.exists());
    assert!(store.repository().events().is_empty());
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

#[test]
fn crashes_after_destination_write_or_draft_delete_recover_one_committed_outcome() {
    for (index, point) in [
        CommitPoint::AfterTargetReplace(0),
        CommitPoint::AfterTargetReplace(1),
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = support::fixture();
        let destination = ProjectRelativePath::new("ae-sdd-doc/Story/STORY-WI-1.md").unwrap();
        let draft = ProjectRelativePath::new("drafts/STORY-WI-1.md").unwrap();
        let draft_path = fixture.temp.path().join(draft.as_str());
        std::fs::create_dir_all(draft_path.parent().unwrap()).unwrap();
        std::fs::write(&draft_path, b"draft story").unwrap();

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
                LeaseOwner::new("root-session").unwrap(),
                support::at("2026-07-23T00:00:00Z"),
                support::at("2026-07-23T00:05:00Z"),
            )
            .unwrap();
        let mut request = support::request(
            &fixture,
            LeaseProof::from(&lease),
            230 + u128::try_from(index).unwrap(),
            &format!("document-delete-kill-{index}"),
            false,
        );
        request.targets.push(
            MutationTarget::write(destination.clone(), None, b"saved story".to_vec()).unwrap(),
        );
        request.targets.push(MutationTarget::delete(
            draft.clone(),
            ArtifactDigest::digest(b"draft story"),
        ));

        assert!(matches!(
            store.commit(request.clone()),
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
            .expect("recovery completes the prepared write/delete transaction");
        assert_eq!(
            reports[0].disposition,
            RecoveryDisposition::CompletedFromStaged
        );
        assert_eq!(
            std::fs::read(fixture.temp.path().join(destination.as_str())).unwrap(),
            b"saved story"
        );
        assert!(!draft_path.exists(), "source draft must be durably deleted");
        assert!(
            restarted
                .commit(request)
                .expect("committed receipt rebuilds")
                .replayed
        );
    }
}

#[cfg(windows)]
#[test]
fn mutation_rejects_case_alias_target_paths() {
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
            LeaseOwner::new("root-session").unwrap(),
            support::at("2026-07-23T00:00:00Z"),
            support::at("2026-07-23T00:05:00Z"),
        )
        .unwrap();
    let mut request = support::request(
        &fixture,
        LeaseProof::from(&lease),
        240,
        "case-alias-targets",
        false,
    );
    request.targets.push(
        MutationTarget::write(
            ProjectRelativePath::new("drafts/Story.md").unwrap(),
            None,
            b"first".to_vec(),
        )
        .unwrap(),
    );
    request.targets.push(
        MutationTarget::write(
            ProjectRelativePath::new("drafts/story.md").unwrap(),
            None,
            b"second".to_vec(),
        )
        .unwrap(),
    );

    assert!(matches!(
        store.commit(request),
        Err(StoreError::InvalidJournal { .. })
    ));
}

#[test]
fn prepared_delete_recovery_rejects_digest_drift_as_external_state_conflict() {
    let fixture = support::fixture();
    let destination = ProjectRelativePath::new("ae-sdd-doc/Story/STORY-WI-1.md").unwrap();
    let draft = ProjectRelativePath::new("drafts/STORY-WI-1.md").unwrap();
    let draft_path = fixture.temp.path().join(draft.as_str());
    std::fs::create_dir_all(draft_path.parent().unwrap()).unwrap();
    std::fs::write(&draft_path, b"draft story").unwrap();

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
            LeaseOwner::new("root-session").unwrap(),
            support::at("2026-07-23T00:00:00Z"),
            support::at("2026-07-23T00:05:00Z"),
        )
        .unwrap();
    let mut request = support::request(
        &fixture,
        LeaseProof::from(&lease),
        240,
        "document-delete-drift",
        false,
    );
    request
        .targets
        .push(MutationTarget::write(destination, None, b"saved story".to_vec()).unwrap());
    request.targets.push(MutationTarget::delete(
        draft,
        ArtifactDigest::digest(b"draft story"),
    ));
    assert!(matches!(
        store.commit(request),
        Err(StoreError::InjectedFault { .. })
    ));

    std::fs::write(&draft_path, b"foreign edit").unwrap();
    let restarted = ProjectMutationStore::new(
        fixture.paths,
        StdDurableFileSystem,
        StdCrossProcessLock,
        InMemoryRuntimeRepository::new(fixture.event_store_id),
    );
    assert!(matches!(
        restarted.recover(support::at("2026-07-23T00:02:00Z")),
        Err(StoreError::ExternalStateConflict {
            expected_digest,
            observed_digest,
            ..
        }) if expected_digest == ArtifactDigest::digest(b"draft story")
            && observed_digest == ArtifactDigest::digest(b"foreign edit")
    ));
    assert_eq!(std::fs::read(draft_path).unwrap(), b"foreign edit");
}
