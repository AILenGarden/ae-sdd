use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use ae_sdd_domain::{
    ArtifactDigest, BootId, FencingToken, InputFingerprint, LeaseId, OperationId,
    ProjectRelativePath, RequestId, ResultDigest, SessionId, StateRevision, WorkItemId,
    WorkspaceId,
};
use serde_json::{Value, json};

use crate::{
    AuthoritySnapshot, CrossProcessLockPort, DurableFileSystem, IdempotencyKey, JournalEvent,
    JournalStatus, LeaseLedger, LeaseOwner, LeaseProof, LeaseRecord, LeaseTombstone,
    MutationJournalEntry, MutationTarget, OperationReceipt, RecoveryDisposition, RecoveryReport,
    RuntimeEventPayload, RuntimeEventRecord, RuntimeRepository, StateAuthority, StoreError,
    TargetDescriptor, UtcTimestamp,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectStorePaths {
    workspace_root: PathBuf,
    state_file: ProjectRelativePath,
    lease_file: ProjectRelativePath,
    state_dir: ProjectRelativePath,
    journal_dir: ProjectRelativePath,
    lock_file: ProjectRelativePath,
}

impl ProjectStorePaths {
    pub fn new(
        workspace_root: impl AsRef<Path>,
        state_file: ProjectRelativePath,
    ) -> Result<Self, StoreError> {
        let requested_root = workspace_root.as_ref();
        let workspace_root = requested_root
            .canonicalize()
            .map_err(|error| StoreError::io(requested_root, error))?;
        if !workspace_root.is_dir() {
            return Err(StoreError::InvalidState {
                reason: "workspace root must be a directory".into(),
            });
        }
        let state_path = Path::new(state_file.as_str());
        let state_parent = state_path
            .parent()
            .ok_or_else(|| StoreError::InvalidState {
                reason: "state file must have a project-relative parent directory".into(),
            })?;
        if state_parent.as_os_str().is_empty() {
            return Err(StoreError::InvalidState {
                reason: "state file must not be placed at workspace root".into(),
            });
        }
        let state_dir =
            ProjectRelativePath::new(state_parent.to_string_lossy().replace('\\', "/"))?;
        let journal_dir =
            ProjectRelativePath::new(format!("{}/mutation-journal/v1", state_dir.as_str()))?;
        let lock_file = ProjectRelativePath::new(format!("{}/.state.lock", state_dir.as_str()))?;
        let lease_file =
            ProjectRelativePath::new(format!("{}/state.lease.json", state_dir.as_str()))?;
        Ok(Self {
            workspace_root,
            state_file,
            lease_file,
            state_dir,
            journal_dir,
            lock_file,
        })
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub const fn state_file(&self) -> &ProjectRelativePath {
        &self.state_file
    }

    pub const fn journal_dir_relative(&self) -> &ProjectRelativePath {
        &self.journal_dir
    }

    pub fn state_path(&self) -> PathBuf {
        self.resolve(&self.state_file)
    }

    pub fn lease_path(&self) -> PathBuf {
        self.resolve(&self.lease_file)
    }

    pub fn journal_dir(&self) -> PathBuf {
        self.resolve(&self.journal_dir)
    }

    pub fn lock_path(&self) -> PathBuf {
        self.resolve(&self.lock_file)
    }

    pub fn resolve(&self, relative: &ProjectRelativePath) -> PathBuf {
        self.workspace_root.join(relative.as_str())
    }

    fn journal_path(&self, revision: StateRevision, mutation_id: RequestId) -> PathBuf {
        self.journal_dir()
            .join(format!("{}-{}.json", revision.get(), mutation_id))
    }

    fn staged_ref(
        &self,
        mutation_id: RequestId,
        index: usize,
    ) -> Result<ProjectRelativePath, StoreError> {
        Ok(ProjectRelativePath::new(format!(
            "{}/staged/{mutation_id}/{index}.bin",
            self.journal_dir.as_str()
        ))?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationRequest {
    pub mutation_id: RequestId,
    pub workspace_id: WorkspaceId,
    pub work_item_id: WorkItemId,
    pub operation: OperationId,
    pub idempotency_key: IdempotencyKey,
    pub canonical_payload_digest: InputFingerprint,
    pub expected_authority: AuthoritySnapshot,
    pub lease_proof: LeaseProof,
    pub targets: Vec<MutationTarget>,
    pub event: JournalEvent,
    pub result_digest: ResultDigest,
    pub prepared_at: UtcTimestamp,
    pub committed_at: UtcTimestamp,
}

/// One lease-ledger control action committed under the Work Item lock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeaseControlAction {
    Acquire {
        lease_id: LeaseId,
        owner: LeaseOwner,
        now: UtcTimestamp,
        expires_at: UtcTimestamp,
    },
    Renew {
        proof: LeaseProof,
        now: UtcTimestamp,
        expires_at: UtcTimestamp,
    },
    Release {
        proof: LeaseProof,
        now: UtcTimestamp,
    },
    Break {
        actor: LeaseOwner,
        reason: Box<str>,
        now: UtcTimestamp,
    },
}

impl LeaseControlAction {
    fn operation_name(&self) -> &'static str {
        match self {
            Self::Acquire { .. } => "lease.acquire",
            Self::Renew { .. } => "lease.renew",
            Self::Release { .. } => "lease.release",
            Self::Break { .. } => "lease.break",
        }
    }
}

/// Durable idempotent request for acquire, renew, or release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaseControlRequest {
    pub mutation_id: RequestId,
    pub workspace_id: WorkspaceId,
    pub work_item_id: WorkItemId,
    pub operation: OperationId,
    pub idempotency_key: IdempotencyKey,
    pub canonical_payload_digest: InputFingerprint,
    pub action: LeaseControlAction,
    pub boot_id: BootId,
    pub session_id: Option<SessionId>,
    pub committed_at: UtcTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommittedMutation {
    pub receipt: OperationReceipt,
    pub event: RuntimeEventRecord,
    pub journal_path: PathBuf,
    pub replayed: bool,
}

/// Lease control result plus its durable mutation receipt.
#[derive(Clone, Debug, PartialEq)]
pub struct CommittedLeaseControl {
    pub mutation: CommittedMutation,
    pub data: Value,
}

/// Read-only result of fully validating a lease control action.
#[derive(Clone, Debug, PartialEq)]
pub struct LeaseControlPreview {
    pub revision: StateRevision,
    pub data: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitPoint {
    AfterPreparedJournal,
    AfterStagedTargets,
    AfterTargetReplace(usize),
    AfterCommittedJournal,
}

pub trait CommitFaultPort: Send + Sync {
    fn reached(&self, point: CommitPoint) -> Result<(), StoreError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoCommitFault;

impl CommitFaultPort for NoCommitFault {
    fn reached(&self, _point: CommitPoint) -> Result<(), StoreError> {
        Ok(())
    }
}

pub struct ProjectMutationStore<F, L, R, C = NoCommitFault> {
    paths: ProjectStorePaths,
    files: F,
    locks: L,
    repository: R,
    faults: C,
}

impl<F, L, R> ProjectMutationStore<F, L, R, NoCommitFault> {
    pub const fn new(paths: ProjectStorePaths, files: F, locks: L, repository: R) -> Self {
        Self {
            paths,
            files,
            locks,
            repository,
            faults: NoCommitFault,
        }
    }
}

impl<F, L, R, C> ProjectMutationStore<F, L, R, C> {
    pub const fn with_faults(
        paths: ProjectStorePaths,
        files: F,
        locks: L,
        repository: R,
        faults: C,
    ) -> Self {
        Self {
            paths,
            files,
            locks,
            repository,
            faults,
        }
    }

    pub const fn paths(&self) -> &ProjectStorePaths {
        &self.paths
    }

    pub const fn repository(&self) -> &R {
        &self.repository
    }
}

impl<F, L, R, C> ProjectMutationStore<F, L, R, C>
where
    F: DurableFileSystem,
    L: CrossProcessLockPort,
    R: RuntimeRepository,
    C: CommitFaultPort,
{
    pub fn acquire_lease(
        &self,
        lease_id: LeaseId,
        owner: LeaseOwner,
        now: UtcTimestamp,
        expires_at: UtcTimestamp,
    ) -> Result<LeaseRecord, StoreError> {
        let _guard = self.locks.lock_exclusive(&self.paths.lock_path())?;
        let state = self.load_authority()?;
        let mut ledger = self.load_lease_ledger(state.last_fencing_token())?;
        let record = ledger.acquire(lease_id, owner, now, expires_at)?;
        self.persist_lease_ledger(&ledger)?;
        Ok(record)
    }

    pub fn renew_lease(
        &self,
        proof: &LeaseProof,
        now: &UtcTimestamp,
        expires_at: UtcTimestamp,
    ) -> Result<LeaseRecord, StoreError> {
        let _guard = self.locks.lock_exclusive(&self.paths.lock_path())?;
        let state = self.load_authority()?;
        let mut ledger = self.load_lease_ledger(state.last_fencing_token())?;
        let record = ledger.renew(proof, now, expires_at)?;
        self.persist_lease_ledger(&ledger)?;
        Ok(record)
    }

    pub fn release_lease(
        &self,
        proof: &LeaseProof,
        now: UtcTimestamp,
    ) -> Result<LeaseTombstone, StoreError> {
        let _guard = self.locks.lock_exclusive(&self.paths.lock_path())?;
        let state = self.load_authority()?;
        let mut ledger = self.load_lease_ledger(state.last_fencing_token())?;
        let tombstone = ledger.release(proof, now)?;
        self.persist_lease_ledger(&ledger)?;
        Ok(tombstone)
    }

    /// Validates the active lease and fencing generation without mutation.
    pub fn validate_lease_proof(
        &self,
        proof: &LeaseProof,
        now: &UtcTimestamp,
    ) -> Result<(), StoreError> {
        let _guard = self.locks.lock_exclusive(&self.paths.lock_path())?;
        let state = self.load_authority()?;
        let mut ledger = self.load_lease_ledger(state.last_fencing_token())?;
        ledger.validate(proof, now)
    }

    pub fn break_lease(
        &self,
        actor: LeaseOwner,
        reason: impl Into<Box<str>>,
        now: UtcTimestamp,
    ) -> Result<Option<LeaseTombstone>, StoreError> {
        let _guard = self.locks.lock_exclusive(&self.paths.lock_path())?;
        let state = self.load_authority()?;
        let mut ledger = self.load_lease_ledger(state.last_fencing_token())?;
        let tombstone = ledger.break_active(actor, reason, now)?;
        self.persist_lease_ledger(&ledger)?;
        Ok(tombstone)
    }

    /// Validates a lease control action, including idempotency scope and the
    /// current ledger, without writing a ledger, journal, event, or receipt.
    pub fn preview_lease_control(
        &self,
        request: &LeaseControlRequest,
    ) -> Result<LeaseControlPreview, StoreError> {
        let _guard = self.locks.lock_exclusive(&self.paths.lock_path())?;
        self.validate_lease_control_identity(request)?;
        self.validate_idempotency_read_only(
            request.workspace_id,
            &request.work_item_id,
            &request.operation,
            &request.idempotency_key,
            request.canonical_payload_digest,
        )?;
        let authority = self.load_authority()?;
        let mut ledger = self.load_lease_ledger(authority.last_fencing_token())?;
        let (data, _, _) = apply_lease_control(&mut ledger, &request.action)?;
        Ok(LeaseControlPreview {
            revision: authority.revision(),
            data,
        })
    }

    /// Commits a lease control mutation and its idempotency receipt in the
    /// project mutation journal. Exact retries return the original result;
    /// key reuse with another operation or payload is rejected.
    pub fn commit_lease_control(
        &self,
        request: LeaseControlRequest,
    ) -> Result<CommittedLeaseControl, StoreError> {
        let _guard = self.locks.lock_exclusive(&self.paths.lock_path())?;
        self.files.create_dir_all(&self.paths.journal_dir())?;
        self.validate_lease_control_identity(&request)?;
        if let Some(replayed) = self.lookup_or_rebuild_receipt(
            request.workspace_id,
            &request.work_item_id,
            &request.operation,
            &request.idempotency_key,
            request.canonical_payload_digest,
        )? {
            let data = lease_control_result(&replayed)?;
            return Ok(CommittedLeaseControl {
                mutation: replayed,
                data,
            });
        }

        let authority = self.load_authority()?;
        let before_bytes = self.files.read(&self.paths.lease_path())?;
        let before_digest = before_bytes.as_deref().map(ArtifactDigest::digest);
        let mut ledger = self.load_lease_ledger(authority.last_fencing_token())?;
        let (mut data, fencing_token, prepared_at) =
            apply_lease_control(&mut ledger, &request.action)?;
        let after_bytes = ledger.to_canonical_json()?;
        let target =
            MutationTarget::new(self.paths.lease_file.clone(), before_digest, after_bytes)?;
        if matches!(&request.action, LeaseControlAction::Break { .. }) {
            let object = data
                .as_object_mut()
                .ok_or_else(|| StoreError::InvalidJournal {
                    reason: "lease break result must be an object".into(),
                })?;
            object.insert(
                "ledgerBeforeDigest".to_owned(),
                before_digest.map_or(Value::Null, |digest| json!(digest.to_string())),
            );
            object.insert(
                "ledgerAfterDigest".to_owned(),
                json!(target.after_digest().to_string()),
            );
        }
        let descriptor = TargetDescriptor {
            path: target.path().clone(),
            before_digest: target.before_digest(),
            after_digest: target.after_digest(),
            byte_length: u64::try_from(target.after_bytes().len()).unwrap_or(u64::MAX),
            staged_ref: self.paths.staged_ref(request.mutation_id, 0)?,
        };
        let result_bytes =
            serde_json::to_vec(&data).map_err(|error| StoreError::InvalidJournal {
                reason: format!("lease control result could not be serialized: {error}")
                    .into_boxed_str(),
            })?;
        let event_bytes = serde_json::to_vec(&json!({
            "operation":request.operation.to_string(),
            "data":data.clone(),
        }))
        .map_err(|error| StoreError::InvalidJournal {
            reason: format!("lease control event could not be serialized: {error}")
                .into_boxed_str(),
        })?;
        let mut journal = MutationJournalEntry::prepared_control(
            request.mutation_id,
            request.workspace_id,
            request.work_item_id.clone(),
            request.operation.clone(),
            &request.idempotency_key,
            request.canonical_payload_digest,
            ResultDigest::digest(&result_bytes),
            authority.revision(),
            fencing_token,
            vec![descriptor],
            JournalEvent {
                boot_id: request.boot_id,
                session_id: request.session_id,
                event_type: request.action.operation_name().into(),
                schema_version: 1,
                payload: RuntimeEventPayload::InlineJson(event_bytes),
            },
            prepared_at,
        )?;
        let journal_path = self
            .paths
            .journal_path(authority.revision(), request.mutation_id);
        self.files
            .write_atomic_durable(&journal_path, &journal.to_canonical_json()?)?;
        self.faults.reached(CommitPoint::AfterPreparedJournal)?;

        let staged_path = self.paths.resolve(&journal.target_files[0].staged_ref);
        self.files
            .write_atomic_durable(&staged_path, target.after_bytes())?;
        self.faults.reached(CommitPoint::AfterStagedTargets)?;
        self.revalidate_before_replace(std::slice::from_ref(&target))?;
        self.files
            .write_atomic_durable(&self.paths.lease_path(), target.after_bytes())?;
        self.faults.reached(CommitPoint::AfterTargetReplace(0))?;
        self.verify_after_replace(&journal.target_files)?;

        journal.commit(request.committed_at.clone())?;
        self.files
            .write_atomic_durable(&journal_path, &journal.to_canonical_json()?)?;
        self.faults.reached(CommitPoint::AfterCommittedJournal)?;
        let receipt = journal.operation_receipt(request.idempotency_key)?;
        let event = journal.event.clone().into_draft(
            journal.workspace_id,
            journal.work_item_id.clone(),
            request.committed_at,
        );
        let (receipt, event) = self.repository.index_committed_mutation(&receipt, &event)?;
        Ok(CommittedLeaseControl {
            mutation: CommittedMutation {
                receipt,
                event,
                journal_path,
                replayed: false,
            },
            data,
        })
    }

    pub fn commit(&self, request: MutationRequest) -> Result<CommittedMutation, StoreError> {
        let _guard = self.locks.lock_exclusive(&self.paths.lock_path())?;
        self.files.create_dir_all(&self.paths.journal_dir())?;

        if let Some(replayed) = self.lookup_or_rebuild_receipt(
            request.workspace_id,
            &request.work_item_id,
            &request.operation,
            &request.idempotency_key,
            request.canonical_payload_digest,
        )? {
            return Ok(replayed);
        }

        let current_bytes =
            self.files
                .read(&self.paths.state_path())?
                .ok_or_else(|| StoreError::InvalidState {
                    reason: "authoritative state file does not exist".into(),
                })?;
        let observed = StateAuthority::inspect(&current_bytes)?;
        if request.lease_proof.fencing_token < observed.last_fencing_token() {
            return Err(StoreError::StaleFencingToken {
                minimum: observed.last_fencing_token(),
                observed: request.lease_proof.fencing_token,
            });
        }
        let mut ledger = self.load_lease_ledger(observed.last_fencing_token())?;
        ledger.validate(&request.lease_proof, &request.prepared_at)?;
        StateAuthority::verify_unchanged(request.expected_authority, observed)?;

        let revision_after =
            observed
                .revision()
                .checked_next()
                .map_err(|error| StoreError::InvalidState {
                    reason: error.to_string().into_boxed_str(),
                })?;
        let (ordered_targets, descriptors) = self.validate_targets(&request, observed)?;
        let mut journal = MutationJournalEntry::prepared(
            request.mutation_id,
            request.workspace_id,
            request.work_item_id.clone(),
            request.operation.clone(),
            &request.idempotency_key,
            request.canonical_payload_digest,
            request.result_digest,
            observed.revision(),
            revision_after,
            request.lease_proof.fencing_token,
            descriptors,
            request.event.clone(),
            request.prepared_at.clone(),
        )?;
        let journal_path = self.paths.journal_path(revision_after, request.mutation_id);
        self.files
            .write_atomic_durable(&journal_path, &journal.to_canonical_json()?)?;
        self.faults.reached(CommitPoint::AfterPreparedJournal)?;

        for (target, descriptor) in ordered_targets.iter().zip(&journal.target_files) {
            let staged_path = self.paths.resolve(&descriptor.staged_ref);
            self.files
                .write_atomic_durable(&staged_path, target.after_bytes())?;
        }
        self.faults.reached(CommitPoint::AfterStagedTargets)?;

        self.revalidate_before_replace(&ordered_targets)?;
        for (index, (target, descriptor)) in ordered_targets
            .iter()
            .zip(&journal.target_files)
            .enumerate()
        {
            self.files.write_atomic_durable(
                &self.paths.resolve(&descriptor.path),
                target.after_bytes(),
            )?;
            self.faults
                .reached(CommitPoint::AfterTargetReplace(index))?;
        }
        self.verify_after_replace(&journal.target_files)?;

        journal.commit(request.committed_at.clone())?;
        self.files
            .write_atomic_durable(&journal_path, &journal.to_canonical_json()?)?;
        self.faults.reached(CommitPoint::AfterCommittedJournal)?;

        let receipt = journal.operation_receipt(request.idempotency_key)?;
        let event = journal.event.clone().into_draft(
            journal.workspace_id,
            journal.work_item_id.clone(),
            request.committed_at,
        );
        let (receipt, event) = self.repository.index_committed_mutation(&receipt, &event)?;
        Ok(CommittedMutation {
            receipt,
            event,
            journal_path,
            replayed: false,
        })
    }

    /// Replays a committed operation before a caller evaluates current-state
    /// CAS preconditions. A matching semantic key and payload is side-effect
    /// free; a different payload fails with `IdempotencyKeyReused`.
    pub fn replay_committed(
        &self,
        workspace_id: WorkspaceId,
        work_item_id: &WorkItemId,
        operation: &OperationId,
        idempotency_key: &IdempotencyKey,
        payload_digest: InputFingerprint,
    ) -> Result<Option<CommittedMutation>, StoreError> {
        let _guard = self.locks.lock_exclusive(&self.paths.lock_path())?;
        self.files.create_dir_all(&self.paths.journal_dir())?;
        self.lookup_or_rebuild_receipt(
            workspace_id,
            work_item_id,
            operation,
            idempotency_key,
            payload_digest,
        )
    }

    /// Runs the complete mutation preflight under the project lock without
    /// staging files, appending events, or storing an idempotency receipt.
    pub fn validate_mutation(&self, request: &MutationRequest) -> Result<(), StoreError> {
        let _guard = self.locks.lock_exclusive(&self.paths.lock_path())?;
        self.validate_idempotency_read_only(
            request.workspace_id,
            &request.work_item_id,
            &request.operation,
            &request.idempotency_key,
            request.canonical_payload_digest,
        )?;
        let current_bytes =
            self.files
                .read(&self.paths.state_path())?
                .ok_or_else(|| StoreError::InvalidState {
                    reason: "authoritative state file does not exist".into(),
                })?;
        let observed = StateAuthority::inspect(&current_bytes)?;
        if request.lease_proof.fencing_token < observed.last_fencing_token() {
            return Err(StoreError::StaleFencingToken {
                minimum: observed.last_fencing_token(),
                observed: request.lease_proof.fencing_token,
            });
        }
        let mut ledger = self.load_lease_ledger(observed.last_fencing_token())?;
        ledger.validate(&request.lease_proof, &request.prepared_at)?;
        StateAuthority::verify_unchanged(request.expected_authority, observed)?;
        let revision_after =
            observed
                .revision()
                .checked_next()
                .map_err(|error| StoreError::InvalidState {
                    reason: error.to_string().into_boxed_str(),
                })?;
        let (_, descriptors) = self.validate_targets(request, observed)?;
        MutationJournalEntry::prepared(
            request.mutation_id,
            request.workspace_id,
            request.work_item_id.clone(),
            request.operation.clone(),
            &request.idempotency_key,
            request.canonical_payload_digest,
            request.result_digest,
            observed.revision(),
            revision_after,
            request.lease_proof.fencing_token,
            descriptors,
            request.event.clone(),
            request.prepared_at.clone(),
        )?;
        Ok(())
    }

    pub fn recover(&self, now: UtcTimestamp) -> Result<Vec<RecoveryReport>, StoreError> {
        let _guard = self.locks.lock_exclusive(&self.paths.lock_path())?;
        let mut reports = Vec::new();
        for path in self.files.list_files(&self.paths.journal_dir())? {
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let bytes = self
                .files
                .read(&path)?
                .ok_or_else(|| StoreError::InvalidJournal {
                    reason: "journal disappeared during recovery".into(),
                })?;
            let mut entry = MutationJournalEntry::from_json(&bytes)?;
            if entry.status != JournalStatus::Prepared {
                reports.push(RecoveryReport {
                    mutation_id: entry.mutation_id,
                    disposition: RecoveryDisposition::AlreadyTerminal(entry.status),
                });
                continue;
            }

            let mut after_count = 0_usize;
            let mut pending = Vec::new();
            for descriptor in &entry.target_files {
                let target_path = self.paths.resolve(&descriptor.path);
                let current = self.files.read(&target_path)?;
                let current_digest = current.as_deref().map(ArtifactDigest::digest);
                if current_digest == Some(descriptor.after_digest) {
                    after_count += 1;
                } else if current_digest == descriptor.before_digest {
                    pending.push(descriptor.clone());
                } else {
                    return Err(StoreError::JournalConflict { path: target_path });
                }
            }

            if after_count == 0 {
                entry.abort(now.clone(), "ABORTED_RESTART: no target was applied")?;
                self.files
                    .write_atomic_durable(&path, &entry.to_canonical_json()?)?;
                reports.push(RecoveryReport {
                    mutation_id: entry.mutation_id,
                    disposition: RecoveryDisposition::AbortedUnapplied,
                });
                continue;
            }

            for descriptor in &pending {
                let staged_path = self.paths.resolve(&descriptor.staged_ref);
                let staged =
                    self.files
                        .read(&staged_path)?
                        .ok_or_else(|| StoreError::JournalConflict {
                            path: staged_path.clone(),
                        })?;
                if ArtifactDigest::digest(&staged) != descriptor.after_digest
                    || u64::try_from(staged.len()).unwrap_or(u64::MAX) != descriptor.byte_length
                {
                    return Err(StoreError::JournalConflict { path: staged_path });
                }
                self.files
                    .write_atomic_durable(&self.paths.resolve(&descriptor.path), &staged)?;
            }
            self.verify_after_replace(&entry.target_files)?;
            entry.commit(now.clone())?;
            self.files
                .write_atomic_durable(&path, &entry.to_canonical_json()?)?;
            reports.push(RecoveryReport {
                mutation_id: entry.mutation_id,
                disposition: RecoveryDisposition::CompletedFromStaged,
            });
        }
        Ok(reports)
    }

    fn validate_lease_control_identity(
        &self,
        request: &LeaseControlRequest,
    ) -> Result<(), StoreError> {
        if request.operation.to_string() == request.action.operation_name() {
            Ok(())
        } else {
            Err(StoreError::InvalidJournal {
                reason: "lease control action does not match operation identity".into(),
            })
        }
    }

    fn validate_idempotency_read_only(
        &self,
        workspace_id: WorkspaceId,
        work_item_id: &WorkItemId,
        operation: &OperationId,
        idempotency_key: &IdempotencyKey,
        payload_digest: InputFingerprint,
    ) -> Result<(), StoreError> {
        let Some((receipt, _)) = self
            .repository
            .operation_receipt(workspace_id, idempotency_key.as_str())?
        else {
            return Ok(());
        };
        if receipt.payload_digest != payload_digest
            || &receipt.work_item_id != work_item_id
            || &receipt.operation != operation
        {
            return Err(StoreError::IdempotencyKeyReused {
                expected: receipt.payload_digest,
                observed: payload_digest,
            });
        }
        Ok(())
    }

    fn lookup_or_rebuild_receipt(
        &self,
        workspace_id: WorkspaceId,
        work_item_id: &WorkItemId,
        operation: &OperationId,
        idempotency_key: &IdempotencyKey,
        payload_digest: InputFingerprint,
    ) -> Result<Option<CommittedMutation>, StoreError> {
        if let Some((receipt, event)) = self
            .repository
            .operation_receipt(workspace_id, idempotency_key.as_str())?
        {
            if receipt.payload_digest != payload_digest
                || &receipt.work_item_id != work_item_id
                || &receipt.operation != operation
            {
                return Err(StoreError::IdempotencyKeyReused {
                    expected: receipt.payload_digest,
                    observed: payload_digest,
                });
            }
            return Ok(Some(CommittedMutation {
                journal_path: self
                    .paths
                    .journal_path(receipt.revision_after, receipt.mutation_id),
                receipt,
                event,
                replayed: true,
            }));
        }

        for journal_path in self.files.list_files(&self.paths.journal_dir())? {
            if journal_path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let Some(bytes) = self.files.read(&journal_path)? else {
                continue;
            };
            let entry = MutationJournalEntry::from_json(&bytes)?;
            if entry.status != JournalStatus::Committed
                || entry.workspace_id != workspace_id
                || entry.idempotency_key_digest != idempotency_key.digest()
            {
                continue;
            }
            if entry.canonical_payload_digest != payload_digest
                || &entry.work_item_id != work_item_id
                || &entry.operation != operation
            {
                return Err(StoreError::IdempotencyKeyReused {
                    expected: entry.canonical_payload_digest,
                    observed: payload_digest,
                });
            }
            let receipt = entry.operation_receipt(idempotency_key.clone())?;
            let committed_at = receipt.committed_at.clone();
            let event =
                entry
                    .event
                    .into_draft(entry.workspace_id, entry.work_item_id, committed_at);
            let (receipt, event) = self.repository.index_committed_mutation(&receipt, &event)?;
            return Ok(Some(CommittedMutation {
                receipt,
                event,
                journal_path,
                replayed: true,
            }));
        }
        Ok(None)
    }

    fn validate_targets(
        &self,
        request: &MutationRequest,
        before: AuthoritySnapshot,
    ) -> Result<(Vec<MutationTarget>, Vec<TargetDescriptor>), StoreError> {
        if request.targets.is_empty() {
            return Err(StoreError::InvalidJournal {
                reason: "mutation has no targets".into(),
            });
        }
        let mut paths = BTreeSet::new();
        let mut targets = request.targets.clone();
        for target in &targets {
            if !paths.insert(target.path().clone()) {
                return Err(StoreError::InvalidJournal {
                    reason: "mutation contains duplicate target paths".into(),
                });
            }
            let current = self.files.read(&self.paths.resolve(target.path()))?;
            let current_digest = current.as_deref().map(ArtifactDigest::digest);
            if current_digest != target.before_digest() {
                return Err(StoreError::JournalConflict {
                    path: self.paths.resolve(target.path()),
                });
            }
        }
        let state_target = targets
            .iter()
            .find(|target| target.path() == self.paths.state_file())
            .ok_or_else(|| StoreError::InvalidJournal {
                reason: "mutation must include the authoritative state file".into(),
            })?;
        if state_target.before_digest() != Some(before.digest()) {
            return Err(StoreError::ExternalStateConflict {
                revision: before.revision(),
                expected_digest: before.digest(),
                observed_digest: state_target
                    .before_digest()
                    .unwrap_or_else(|| ArtifactDigest::digest([])),
            });
        }
        let after_state = StateAuthority::inspect(state_target.after_bytes())?;
        StateAuthority::verify_successor(before, after_state, request.lease_proof.fencing_token)?;

        targets.sort_by(|left, right| {
            let left_state = left.path() == self.paths.state_file();
            let right_state = right.path() == self.paths.state_file();
            left_state
                .cmp(&right_state)
                .then_with(|| left.path().cmp(right.path()))
        });
        let descriptors = targets
            .iter()
            .enumerate()
            .map(|(index, target)| {
                Ok(TargetDescriptor {
                    path: target.path().clone(),
                    before_digest: target.before_digest(),
                    after_digest: target.after_digest(),
                    byte_length: u64::try_from(target.after_bytes().len()).unwrap_or(u64::MAX),
                    staged_ref: self.paths.staged_ref(request.mutation_id, index)?,
                })
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        Ok((targets, descriptors))
    }

    fn revalidate_before_replace(&self, targets: &[MutationTarget]) -> Result<(), StoreError> {
        for target in targets {
            let path = self.paths.resolve(target.path());
            let current = self.files.read(&path)?;
            if current.as_deref().map(ArtifactDigest::digest) != target.before_digest() {
                if target.path() == self.paths.state_file() {
                    let observed = current
                        .as_deref()
                        .map(StateAuthority::inspect)
                        .transpose()?;
                    if let Some(observed) = observed {
                        return Err(StoreError::ExternalStateConflict {
                            revision: observed.revision(),
                            expected_digest: target
                                .before_digest()
                                .unwrap_or_else(|| ArtifactDigest::digest([])),
                            observed_digest: observed.digest(),
                        });
                    }
                }
                return Err(StoreError::JournalConflict { path });
            }
        }
        Ok(())
    }

    fn verify_after_replace(&self, targets: &[TargetDescriptor]) -> Result<(), StoreError> {
        for target in targets {
            let path = self.paths.resolve(&target.path);
            let current = self
                .files
                .read(&path)?
                .ok_or_else(|| StoreError::JournalConflict { path: path.clone() })?;
            if ArtifactDigest::digest(&current) != target.after_digest
                || u64::try_from(current.len()).unwrap_or(u64::MAX) != target.byte_length
            {
                return Err(StoreError::JournalConflict { path });
            }
        }
        Ok(())
    }

    fn load_authority(&self) -> Result<AuthoritySnapshot, StoreError> {
        let bytes =
            self.files
                .read(&self.paths.state_path())?
                .ok_or_else(|| StoreError::InvalidState {
                    reason: "authoritative state file does not exist".into(),
                })?;
        StateAuthority::inspect(&bytes)
    }

    fn load_lease_ledger(
        &self,
        state_fencing_token: FencingToken,
    ) -> Result<LeaseLedger, StoreError> {
        let Some(bytes) = self.files.read(&self.paths.lease_path())? else {
            return Ok(LeaseLedger::empty(state_fencing_token));
        };
        let ledger = LeaseLedger::from_json(&bytes)?;
        if ledger.last_fencing_token() < state_fencing_token {
            return Err(StoreError::StaleFencingToken {
                minimum: state_fencing_token,
                observed: ledger.last_fencing_token(),
            });
        }
        Ok(ledger)
    }

    fn persist_lease_ledger(&self, ledger: &LeaseLedger) -> Result<(), StoreError> {
        self.files
            .write_atomic_durable(&self.paths.lease_path(), &ledger.to_canonical_json()?)
    }
}

fn apply_lease_control(
    ledger: &mut LeaseLedger,
    action: &LeaseControlAction,
) -> Result<(Value, FencingToken, UtcTimestamp), StoreError> {
    match action {
        LeaseControlAction::Acquire {
            lease_id,
            owner,
            now,
            expires_at,
        } => {
            let record =
                ledger.acquire(*lease_id, owner.clone(), now.clone(), expires_at.clone())?;
            Ok((
                json!({
                    "leaseId":record.lease_id().to_string(),
                    "fencingToken":record.fencing_token().get(),
                    "expiresAt":record.expires_at().to_string(),
                }),
                record.fencing_token(),
                now.clone(),
            ))
        }
        LeaseControlAction::Renew {
            proof,
            now,
            expires_at,
        } => {
            let record = ledger.renew(proof, now, expires_at.clone())?;
            Ok((
                json!({
                    "leaseId":record.lease_id().to_string(),
                    "fencingToken":record.fencing_token().get(),
                    "expiresAt":record.expires_at().to_string(),
                }),
                record.fencing_token(),
                now.clone(),
            ))
        }
        LeaseControlAction::Release { proof, now } => {
            let tombstone = ledger.release(proof, now.clone())?;
            Ok((
                json!({
                    "leaseId":tombstone.lease_id.to_string(),
                    "fencingToken":tombstone.fencing_token.get(),
                    "status":"released",
                }),
                tombstone.fencing_token,
                now.clone(),
            ))
        }
        LeaseControlAction::Break { actor, reason, now } => {
            let tombstone = ledger.break_active(actor.clone(), reason.clone(), now.clone())?;
            let data = tombstone.as_ref().map_or_else(
                || {
                    json!({
                        "broken":false,
                        "status":"absent",
                        "actor":actor.as_str(),
                        "reason":reason,
                        "fencingToken":ledger.last_fencing_token().get(),
                    })
                },
                |tombstone| {
                    json!({
                        "broken":true,
                        "status":"broken",
                        "actor":actor.as_str(),
                        "reason":reason,
                        "leaseId":tombstone.lease_id.to_string(),
                        "owner":tombstone.owner.as_str(),
                        "fencingToken":tombstone.fencing_token.get(),
                        "endedAt":tombstone.ended_at.to_string(),
                    })
                },
            );
            Ok((data, ledger.last_fencing_token(), now.clone()))
        }
    }
}

fn lease_control_result(mutation: &CommittedMutation) -> Result<Value, StoreError> {
    let RuntimeEventPayload::InlineJson(bytes) = &mutation.event.draft.payload else {
        return Err(StoreError::InvalidJournal {
            reason: "lease control receipt references a non-inline event".into(),
        });
    };
    let event: Value =
        serde_json::from_slice(bytes).map_err(|error| StoreError::InvalidJournal {
            reason: format!("lease control receipt event is invalid: {error}").into_boxed_str(),
        })?;
    let data = event
        .get("data")
        .cloned()
        .ok_or_else(|| StoreError::InvalidJournal {
            reason: "lease control receipt event is missing data".into(),
        })?;
    let data_bytes = serde_json::to_vec(&data).map_err(|error| StoreError::InvalidJournal {
        reason: format!("lease control receipt data is invalid: {error}").into_boxed_str(),
    })?;
    if ResultDigest::digest(data_bytes) != mutation.receipt.result_digest {
        return Err(StoreError::InvalidJournal {
            reason: "lease control receipt result digest does not match its event".into(),
        });
    }
    Ok(data)
}
