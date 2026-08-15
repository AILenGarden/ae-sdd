use std::str::FromStr;

use ae_sdd_domain::{
    ArtifactDigest, BootId, FencingToken, InputFingerprint, OperationId, ProjectRelativePath,
    RequestId, ResultDigest, SessionId, StateRevision, WorkItemId, WorkspaceId,
};
use serde::{Deserialize, Serialize};

use crate::{
    IdempotencyKey, OperationReceipt, RuntimeEventDraft, RuntimeEventPayload, StoreError,
    UtcTimestamp, model::WireRuntimePayload,
};

const LEGACY_MUTATION_JOURNAL_SCHEMA_VERSION: u32 = 1;
pub const MUTATION_JOURNAL_SCHEMA_VERSION: u32 = 2;
pub const MAX_MUTATION_TARGETS: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MutationTarget {
    Write {
        path: ProjectRelativePath,
        before_digest: Option<ArtifactDigest>,
        after_bytes: Vec<u8>,
    },
    Delete {
        path: ProjectRelativePath,
        expected_before_digest: ArtifactDigest,
    },
}

impl MutationTarget {
    pub fn new(
        path: ProjectRelativePath,
        before_digest: Option<ArtifactDigest>,
        after_bytes: Vec<u8>,
    ) -> Result<Self, StoreError> {
        Self::write(path, before_digest, after_bytes)
    }

    pub fn write(
        path: ProjectRelativePath,
        before_digest: Option<ArtifactDigest>,
        after_bytes: Vec<u8>,
    ) -> Result<Self, StoreError> {
        if after_bytes.is_empty() {
            return Err(StoreError::InvalidJournal {
                reason: "mutation target must not be empty".into(),
            });
        }
        Ok(Self::Write {
            path,
            before_digest,
            after_bytes,
        })
    }

    pub const fn delete(path: ProjectRelativePath, expected_before_digest: ArtifactDigest) -> Self {
        Self::Delete {
            path,
            expected_before_digest,
        }
    }

    pub const fn path(&self) -> &ProjectRelativePath {
        match self {
            Self::Write { path, .. } | Self::Delete { path, .. } => path,
        }
    }

    pub const fn before_digest(&self) -> Option<ArtifactDigest> {
        match self {
            Self::Write { before_digest, .. } => *before_digest,
            Self::Delete {
                expected_before_digest,
                ..
            } => Some(*expected_before_digest),
        }
    }

    pub fn write_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Write { after_bytes, .. } => Some(after_bytes),
            Self::Delete { .. } => None,
        }
    }

    pub const fn is_delete(&self) -> bool {
        matches!(self, Self::Delete { .. })
    }

    pub fn after_bytes(&self) -> &[u8] {
        self.write_bytes().unwrap_or_default()
    }

    pub fn after_digest(&self) -> ArtifactDigest {
        ArtifactDigest::digest(self.after_bytes())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetDescriptor {
    Write {
        path: ProjectRelativePath,
        before_digest: Option<ArtifactDigest>,
        after_digest: ArtifactDigest,
        byte_length: u64,
        staged_ref: ProjectRelativePath,
    },
    Delete {
        path: ProjectRelativePath,
        expected_before_digest: ArtifactDigest,
    },
}

impl TargetDescriptor {
    pub const fn write(
        path: ProjectRelativePath,
        before_digest: Option<ArtifactDigest>,
        after_digest: ArtifactDigest,
        byte_length: u64,
        staged_ref: ProjectRelativePath,
    ) -> Self {
        Self::Write {
            path,
            before_digest,
            after_digest,
            byte_length,
            staged_ref,
        }
    }

    pub const fn delete(path: ProjectRelativePath, expected_before_digest: ArtifactDigest) -> Self {
        Self::Delete {
            path,
            expected_before_digest,
        }
    }

    pub const fn path(&self) -> &ProjectRelativePath {
        match self {
            Self::Write { path, .. } | Self::Delete { path, .. } => path,
        }
    }

    pub const fn before_digest(&self) -> Option<ArtifactDigest> {
        match self {
            Self::Write { before_digest, .. } => *before_digest,
            Self::Delete {
                expected_before_digest,
                ..
            } => Some(*expected_before_digest),
        }
    }

    pub const fn write_after(&self) -> Option<(ArtifactDigest, u64, &ProjectRelativePath)> {
        match self {
            Self::Write {
                after_digest,
                byte_length,
                staged_ref,
                ..
            } => Some((*after_digest, *byte_length, staged_ref)),
            Self::Delete { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalEvent {
    pub boot_id: BootId,
    pub session_id: Option<SessionId>,
    pub event_type: Box<str>,
    pub schema_version: u32,
    pub payload: RuntimeEventPayload,
}

impl JournalEvent {
    pub fn into_draft(
        self,
        workspace_id: WorkspaceId,
        work_item_id: WorkItemId,
        committed_at: UtcTimestamp,
    ) -> RuntimeEventDraft {
        RuntimeEventDraft {
            boot_id: self.boot_id,
            workspace_id,
            session_id: self.session_id,
            work_item_id,
            event_type: self.event_type,
            schema_version: self.schema_version,
            payload: self.payload,
            committed_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalReceipt {
    pub result_digest: ResultDigest,
    pub committed_at: UtcTimestamp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalStatus {
    Prepared,
    Committed,
    Aborted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationJournalEntry {
    pub schema_version: u32,
    pub mutation_id: RequestId,
    pub workspace_id: WorkspaceId,
    pub work_item_id: WorkItemId,
    pub operation: OperationId,
    pub idempotency_key_digest: InputFingerprint,
    pub canonical_payload_digest: InputFingerprint,
    pub planned_result_digest: ResultDigest,
    pub revision_before: StateRevision,
    pub revision_after: StateRevision,
    pub fencing_token: FencingToken,
    pub target_files: Vec<TargetDescriptor>,
    pub event: JournalEvent,
    pub status: JournalStatus,
    pub prepared_at: UtcTimestamp,
    pub receipt: Option<JournalReceipt>,
    pub aborted_at: Option<UtcTimestamp>,
    pub abort_reason: Option<Box<str>>,
}

impl MutationJournalEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn prepared(
        mutation_id: RequestId,
        workspace_id: WorkspaceId,
        work_item_id: WorkItemId,
        operation: OperationId,
        idempotency_key: &IdempotencyKey,
        canonical_payload_digest: InputFingerprint,
        planned_result_digest: ResultDigest,
        revision_before: StateRevision,
        revision_after: StateRevision,
        fencing_token: FencingToken,
        target_files: Vec<TargetDescriptor>,
        event: JournalEvent,
        prepared_at: UtcTimestamp,
    ) -> Result<Self, StoreError> {
        Self::prepare(
            mutation_id,
            workspace_id,
            work_item_id,
            operation,
            idempotency_key,
            canonical_payload_digest,
            planned_result_digest,
            revision_before,
            revision_after,
            fencing_token,
            target_files,
            event,
            prepared_at,
            true,
        )
    }

    /// Creates a journal entry for a lease-ledger control mutation that does
    /// not advance project-state revision.
    #[allow(clippy::too_many_arguments)]
    pub fn prepared_control(
        mutation_id: RequestId,
        workspace_id: WorkspaceId,
        work_item_id: WorkItemId,
        operation: OperationId,
        idempotency_key: &IdempotencyKey,
        canonical_payload_digest: InputFingerprint,
        planned_result_digest: ResultDigest,
        revision: StateRevision,
        fencing_token: FencingToken,
        target_files: Vec<TargetDescriptor>,
        event: JournalEvent,
        prepared_at: UtcTimestamp,
    ) -> Result<Self, StoreError> {
        Self::prepare(
            mutation_id,
            workspace_id,
            work_item_id,
            operation,
            idempotency_key,
            canonical_payload_digest,
            planned_result_digest,
            revision,
            revision,
            fencing_token,
            target_files,
            event,
            prepared_at,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare(
        mutation_id: RequestId,
        workspace_id: WorkspaceId,
        work_item_id: WorkItemId,
        operation: OperationId,
        idempotency_key: &IdempotencyKey,
        canonical_payload_digest: InputFingerprint,
        planned_result_digest: ResultDigest,
        revision_before: StateRevision,
        revision_after: StateRevision,
        fencing_token: FencingToken,
        target_files: Vec<TargetDescriptor>,
        event: JournalEvent,
        prepared_at: UtcTimestamp,
        advances_revision: bool,
    ) -> Result<Self, StoreError> {
        if target_files.is_empty() || target_files.len() > MAX_MUTATION_TARGETS {
            return Err(StoreError::InvalidJournal {
                reason: "mutation must contain between 1 and 128 targets".into(),
            });
        }
        let expected_revision = if advances_revision {
            revision_before
                .checked_next()
                .map_err(|error| StoreError::InvalidJournal {
                    reason: error.to_string().into_boxed_str(),
                })?
        } else {
            revision_before
        };
        if revision_after != expected_revision {
            return Err(StoreError::InvalidJournal {
                reason: "journal revision relation does not match its mutation kind".into(),
            });
        }
        event
            .payload
            .validate()
            .map_err(|error| StoreError::InvalidJournal {
                reason: error.to_string().into_boxed_str(),
            })?;
        Ok(Self {
            schema_version: MUTATION_JOURNAL_SCHEMA_VERSION,
            mutation_id,
            workspace_id,
            work_item_id,
            operation,
            idempotency_key_digest: idempotency_key.digest(),
            canonical_payload_digest,
            planned_result_digest,
            revision_before,
            revision_after,
            fencing_token,
            target_files,
            event,
            status: JournalStatus::Prepared,
            prepared_at,
            receipt: None,
            aborted_at: None,
            abort_reason: None,
        })
    }

    pub fn commit(&mut self, committed_at: UtcTimestamp) -> Result<(), StoreError> {
        if self.status != JournalStatus::Prepared {
            return Err(StoreError::InvalidJournal {
                reason: "only PREPARED journal entries can commit".into(),
            });
        }
        self.status = JournalStatus::Committed;
        self.receipt = Some(JournalReceipt {
            result_digest: self.planned_result_digest,
            committed_at,
        });
        Ok(())
    }

    pub fn abort(
        &mut self,
        aborted_at: UtcTimestamp,
        reason: impl Into<Box<str>>,
    ) -> Result<(), StoreError> {
        if self.status != JournalStatus::Prepared {
            return Err(StoreError::InvalidJournal {
                reason: "only PREPARED journal entries can abort".into(),
            });
        }
        let reason = reason.into();
        if reason.is_empty() || reason.len() > 1024 {
            return Err(StoreError::InvalidJournal {
                reason: "abort reason must be present and bounded".into(),
            });
        }
        self.status = JournalStatus::Aborted;
        self.aborted_at = Some(aborted_at);
        self.abort_reason = Some(reason);
        Ok(())
    }

    pub fn to_canonical_json(&self) -> Result<Vec<u8>, StoreError> {
        let wire = JournalWire::from_entry(self)?;
        serde_json::to_vec(&wire).map_err(|error| StoreError::InvalidJournal {
            reason: error.to_string().into_boxed_str(),
        })
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, StoreError> {
        let wire: JournalWire =
            serde_json::from_slice(bytes).map_err(|error| StoreError::InvalidJournal {
                reason: error.to_string().into_boxed_str(),
            })?;
        wire.into_entry()
    }

    pub fn operation_receipt(
        &self,
        idempotency_key: IdempotencyKey,
    ) -> Result<OperationReceipt, StoreError> {
        if idempotency_key.digest() != self.idempotency_key_digest {
            return Err(StoreError::IdempotencyKeyReused {
                expected: self.idempotency_key_digest,
                observed: idempotency_key.digest(),
            });
        }
        let receipt = self
            .receipt
            .as_ref()
            .ok_or_else(|| StoreError::InvalidJournal {
                reason: "committed journal is missing its receipt".into(),
            })?;
        Ok(OperationReceipt {
            workspace_id: self.workspace_id,
            work_item_id: self.work_item_id.clone(),
            idempotency_key,
            payload_digest: self.canonical_payload_digest,
            operation: self.operation.clone(),
            revision_before: self.revision_before,
            revision_after: self.revision_after,
            fencing_token: self.fencing_token,
            result_digest: receipt.result_digest,
            mutation_id: self.mutation_id,
            committed_at: receipt.committed_at.clone(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoveryDisposition {
    AlreadyTerminal(JournalStatus),
    AbortedUnapplied,
    CompletedFromStaged,
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryReport {
    pub mutation_id: RequestId,
    pub disposition: RecoveryDisposition,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JournalWire {
    schema_version: u32,
    mutation_id: Box<str>,
    workspace_id: Box<str>,
    work_item_id: Box<str>,
    operation: Box<str>,
    idempotency_key_digest: Box<str>,
    canonical_payload_digest: Box<str>,
    planned_result_digest: Box<str>,
    revision_before: u64,
    revision_after: u64,
    fencing_token: u64,
    target_files: Vec<TargetWire>,
    event: EventWire,
    status: StatusWire,
    prepared_at: Box<str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    receipt: Option<ReceiptWire>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aborted_at: Option<Box<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    abort_reason: Option<Box<str>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum TargetWire {
    Tagged(TaggedTargetWire),
    LegacyWrite(LegacyWriteTargetWire),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum TaggedTargetWire {
    Write {
        path: Box<str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        before_digest: Option<Box<str>>,
        after_digest: Box<str>,
        byte_length: u64,
        staged_ref: Box<str>,
    },
    Delete {
        path: Box<str>,
        expected_before_digest: Box<str>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyWriteTargetWire {
    path: Box<str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    before_digest: Option<Box<str>>,
    after_digest: Box<str>,
    byte_length: u64,
    staged_ref: Box<str>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventWire {
    boot_id: Box<str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<Box<str>>,
    event_type: Box<str>,
    schema_version: u32,
    #[serde(flatten)]
    payload: WireRuntimePayload,
}

#[derive(Debug, Serialize, Deserialize)]
enum StatusWire {
    #[serde(rename = "PREPARED")]
    Prepared,
    #[serde(rename = "COMMITTED")]
    Committed,
    #[serde(rename = "ABORTED")]
    Aborted,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReceiptWire {
    result_digest: Box<str>,
    committed_at: Box<str>,
}

fn target_wire_from_descriptor(
    schema_version: u32,
    target: &TargetDescriptor,
) -> Result<TargetWire, StoreError> {
    let write_fields = |path: &ProjectRelativePath,
                        before_digest: Option<ArtifactDigest>,
                        after_digest: ArtifactDigest,
                        byte_length: u64,
                        staged_ref: &ProjectRelativePath| {
        (
            path.to_string().into_boxed_str(),
            before_digest.map(|digest| digest.to_string().into_boxed_str()),
            after_digest.to_string().into_boxed_str(),
            byte_length,
            staged_ref.to_string().into_boxed_str(),
        )
    };
    match (schema_version, target) {
        (
            LEGACY_MUTATION_JOURNAL_SCHEMA_VERSION,
            TargetDescriptor::Write {
                path,
                before_digest,
                after_digest,
                byte_length,
                staged_ref,
            },
        ) => {
            let (path, before_digest, after_digest, byte_length, staged_ref) = write_fields(
                path,
                *before_digest,
                *after_digest,
                *byte_length,
                staged_ref,
            );
            Ok(TargetWire::LegacyWrite(LegacyWriteTargetWire {
                path,
                before_digest,
                after_digest,
                byte_length,
                staged_ref,
            }))
        }
        (
            MUTATION_JOURNAL_SCHEMA_VERSION,
            TargetDescriptor::Write {
                path,
                before_digest,
                after_digest,
                byte_length,
                staged_ref,
            },
        ) => {
            let (path, before_digest, after_digest, byte_length, staged_ref) = write_fields(
                path,
                *before_digest,
                *after_digest,
                *byte_length,
                staged_ref,
            );
            Ok(TargetWire::Tagged(TaggedTargetWire::Write {
                path,
                before_digest,
                after_digest,
                byte_length,
                staged_ref,
            }))
        }
        (
            MUTATION_JOURNAL_SCHEMA_VERSION,
            TargetDescriptor::Delete {
                path,
                expected_before_digest,
            },
        ) => Ok(TargetWire::Tagged(TaggedTargetWire::Delete {
            path: path.to_string().into_boxed_str(),
            expected_before_digest: expected_before_digest.to_string().into_boxed_str(),
        })),
        (LEGACY_MUTATION_JOURNAL_SCHEMA_VERSION, TargetDescriptor::Delete { .. }) => {
            Err(StoreError::InvalidJournal {
                reason: "journal v1 cannot encode delete targets".into(),
            })
        }
        (unsupported, _) => Err(StoreError::InvalidJournal {
            reason: format!("unsupported schema version {unsupported}").into_boxed_str(),
        }),
    }
}

fn target_descriptor_from_wire(
    schema_version: u32,
    target: TargetWire,
) -> Result<TargetDescriptor, StoreError> {
    let parse_digest = |value: &str| {
        ArtifactDigest::from_str(value).map_err(|error| StoreError::InvalidJournal {
            reason: error.to_string().into_boxed_str(),
        })
    };
    let write_descriptor = |target: LegacyWriteTargetWire| {
        Ok::<TargetDescriptor, StoreError>(TargetDescriptor::write(
            ProjectRelativePath::from_str(&target.path)?,
            target
                .before_digest
                .as_deref()
                .map(&parse_digest)
                .transpose()?,
            parse_digest(&target.after_digest)?,
            target.byte_length,
            ProjectRelativePath::from_str(&target.staged_ref)?,
        ))
    };
    match (schema_version, target) {
        (LEGACY_MUTATION_JOURNAL_SCHEMA_VERSION, TargetWire::LegacyWrite(target)) => {
            write_descriptor(target)
        }
        (
            MUTATION_JOURNAL_SCHEMA_VERSION,
            TargetWire::Tagged(TaggedTargetWire::Write {
                path,
                before_digest,
                after_digest,
                byte_length,
                staged_ref,
            }),
        ) => write_descriptor(LegacyWriteTargetWire {
            path,
            before_digest,
            after_digest,
            byte_length,
            staged_ref,
        }),
        (
            MUTATION_JOURNAL_SCHEMA_VERSION,
            TargetWire::Tagged(TaggedTargetWire::Delete {
                path,
                expected_before_digest,
            }),
        ) => Ok(TargetDescriptor::delete(
            ProjectRelativePath::from_str(&path)?,
            parse_digest(&expected_before_digest)?,
        )),
        (LEGACY_MUTATION_JOURNAL_SCHEMA_VERSION, TargetWire::Tagged(_)) => {
            Err(StoreError::InvalidJournal {
                reason: "journal v1 target must use the legacy write shape".into(),
            })
        }
        (MUTATION_JOURNAL_SCHEMA_VERSION, TargetWire::LegacyWrite(_)) => {
            Err(StoreError::InvalidJournal {
                reason: "journal v2 target must declare write or delete kind".into(),
            })
        }
        (unsupported, _) => Err(StoreError::InvalidJournal {
            reason: format!("unsupported schema version {unsupported}").into_boxed_str(),
        }),
    }
}

impl JournalWire {
    fn from_entry(entry: &MutationJournalEntry) -> Result<Self, StoreError> {
        let payload = WireRuntimePayload::from_payload(&entry.event.payload)?;
        let target_files = entry
            .target_files
            .iter()
            .map(|target| target_wire_from_descriptor(entry.schema_version, target))
            .collect::<Result<Vec<_>, StoreError>>()?;
        Ok(Self {
            schema_version: entry.schema_version,
            mutation_id: entry.mutation_id.to_string().into_boxed_str(),
            workspace_id: entry.workspace_id.to_string().into_boxed_str(),
            work_item_id: entry.work_item_id.to_string().into_boxed_str(),
            operation: entry.operation.to_string().into_boxed_str(),
            idempotency_key_digest: entry.idempotency_key_digest.to_string().into_boxed_str(),
            canonical_payload_digest: entry.canonical_payload_digest.to_string().into_boxed_str(),
            planned_result_digest: entry.planned_result_digest.to_string().into_boxed_str(),
            revision_before: entry.revision_before.get(),
            revision_after: entry.revision_after.get(),
            fencing_token: entry.fencing_token.get(),
            target_files,
            event: EventWire {
                boot_id: entry.event.boot_id.to_string().into_boxed_str(),
                session_id: entry
                    .event
                    .session_id
                    .map(|session_id| session_id.to_string().into_boxed_str()),
                event_type: entry.event.event_type.clone(),
                schema_version: entry.event.schema_version,
                payload,
            },
            status: match entry.status {
                JournalStatus::Prepared => StatusWire::Prepared,
                JournalStatus::Committed => StatusWire::Committed,
                JournalStatus::Aborted => StatusWire::Aborted,
            },
            prepared_at: entry.prepared_at.to_string().into_boxed_str(),
            receipt: entry.receipt.as_ref().map(|receipt| ReceiptWire {
                result_digest: receipt.result_digest.to_string().into_boxed_str(),
                committed_at: receipt.committed_at.to_string().into_boxed_str(),
            }),
            aborted_at: entry
                .aborted_at
                .as_ref()
                .map(|timestamp| timestamp.to_string().into_boxed_str()),
            abort_reason: entry.abort_reason.clone(),
        })
    }

    fn into_entry(self) -> Result<MutationJournalEntry, StoreError> {
        if !matches!(
            self.schema_version,
            LEGACY_MUTATION_JOURNAL_SCHEMA_VERSION | MUTATION_JOURNAL_SCHEMA_VERSION
        ) {
            return Err(StoreError::InvalidJournal {
                reason: format!("unsupported schema version {}", self.schema_version)
                    .into_boxed_str(),
            });
        }
        let target_files = self
            .target_files
            .into_iter()
            .map(|target| target_descriptor_from_wire(self.schema_version, target))
            .collect::<Result<Vec<_>, StoreError>>()?;
        if target_files.is_empty() || target_files.len() > MAX_MUTATION_TARGETS {
            return Err(StoreError::InvalidJournal {
                reason: "journal target count is outside the supported range".into(),
            });
        }
        let payload = self.event.payload.into_payload()?;
        payload.validate()?;
        let receipt = self
            .receipt
            .map(|receipt| {
                Ok::<JournalReceipt, StoreError>(JournalReceipt {
                    result_digest: ResultDigest::from_str(&receipt.result_digest).map_err(
                        |error| StoreError::InvalidJournal {
                            reason: error.to_string().into_boxed_str(),
                        },
                    )?,
                    committed_at: UtcTimestamp::from_str(&receipt.committed_at)?,
                })
            })
            .transpose()?;
        let entry = MutationJournalEntry {
            schema_version: self.schema_version,
            mutation_id: RequestId::from_str(&self.mutation_id)?,
            workspace_id: WorkspaceId::from_str(&self.workspace_id)?,
            work_item_id: WorkItemId::from_str(&self.work_item_id)?,
            operation: OperationId::from_str(&self.operation)?,
            idempotency_key_digest: InputFingerprint::from_str(&self.idempotency_key_digest)
                .map_err(|error| StoreError::InvalidJournal {
                    reason: error.to_string().into_boxed_str(),
                })?,
            canonical_payload_digest: InputFingerprint::from_str(&self.canonical_payload_digest)
                .map_err(|error| StoreError::InvalidJournal {
                    reason: error.to_string().into_boxed_str(),
                })?,
            planned_result_digest: ResultDigest::from_str(&self.planned_result_digest).map_err(
                |error| StoreError::InvalidJournal {
                    reason: error.to_string().into_boxed_str(),
                },
            )?,
            revision_before: StateRevision::new(self.revision_before),
            revision_after: StateRevision::new(self.revision_after),
            fencing_token: FencingToken::new(self.fencing_token),
            target_files,
            event: JournalEvent {
                boot_id: BootId::from_str(&self.event.boot_id)?,
                session_id: self
                    .event
                    .session_id
                    .map(|value| SessionId::from_str(&value))
                    .transpose()?,
                event_type: self.event.event_type,
                schema_version: self.event.schema_version,
                payload,
            },
            status: match self.status {
                StatusWire::Prepared => JournalStatus::Prepared,
                StatusWire::Committed => JournalStatus::Committed,
                StatusWire::Aborted => JournalStatus::Aborted,
            },
            prepared_at: UtcTimestamp::from_str(&self.prepared_at)?,
            receipt,
            aborted_at: self
                .aborted_at
                .map(|value| UtcTimestamp::from_str(&value))
                .transpose()?,
            abort_reason: self.abort_reason,
        };
        entry.validate_terminal_shape()?;
        Ok(entry)
    }
}

impl MutationJournalEntry {
    fn validate_terminal_shape(&self) -> Result<(), StoreError> {
        let valid = match self.status {
            JournalStatus::Prepared => {
                self.receipt.is_none() && self.aborted_at.is_none() && self.abort_reason.is_none()
            }
            JournalStatus::Committed => {
                self.receipt.is_some() && self.aborted_at.is_none() && self.abort_reason.is_none()
            }
            JournalStatus::Aborted => {
                self.receipt.is_none() && self.aborted_at.is_some() && self.abort_reason.is_some()
            }
        };
        if !valid {
            return Err(StoreError::InvalidJournal {
                reason: "journal terminal fields do not match status".into(),
            });
        }
        Ok(())
    }
}

impl WireRuntimePayload {
    fn from_payload(payload: &RuntimeEventPayload) -> Result<Self, StoreError> {
        payload.validate()?;
        match payload {
            RuntimeEventPayload::InlineJson(bytes) => Ok(Self {
                payload_json: Some(serde_json::from_slice(bytes).map_err(|error| {
                    StoreError::InvalidJournal {
                        reason: error.to_string().into_boxed_str(),
                    }
                })?),
                payload_ref: None,
                payload_digest: payload.digest().to_string().into_boxed_str(),
                byte_length: payload.byte_length(),
            }),
            RuntimeEventPayload::ArtifactRef {
                project_relative_path,
                digest,
                byte_length,
            } => Ok(Self {
                payload_json: None,
                payload_ref: Some(project_relative_path.clone()),
                payload_digest: digest.to_string().into_boxed_str(),
                byte_length: *byte_length,
            }),
        }
    }

    fn into_payload(self) -> Result<RuntimeEventPayload, StoreError> {
        let digest = ArtifactDigest::from_str(&self.payload_digest).map_err(|error| {
            StoreError::InvalidJournal {
                reason: error.to_string().into_boxed_str(),
            }
        })?;
        match (self.payload_json, self.payload_ref) {
            (Some(value), None) => {
                let bytes =
                    serde_json::to_vec(&value).map_err(|error| StoreError::InvalidJournal {
                        reason: error.to_string().into_boxed_str(),
                    })?;
                if ArtifactDigest::digest(&bytes) != digest
                    || u64::try_from(bytes.len()).unwrap_or(u64::MAX) != self.byte_length
                {
                    return Err(StoreError::InvalidJournal {
                        reason: "inline event payload digest or length is invalid".into(),
                    });
                }
                Ok(RuntimeEventPayload::InlineJson(bytes))
            }
            (None, Some(project_relative_path)) => Ok(RuntimeEventPayload::ArtifactRef {
                project_relative_path,
                digest,
                byte_length: self.byte_length,
            }),
            _ => Err(StoreError::InvalidJournal {
                reason: "event must contain exactly one of payloadJson or payloadRef".into(),
            }),
        }
    }
}
