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

pub const MUTATION_JOURNAL_SCHEMA_VERSION: u32 = 1;
pub const MAX_MUTATION_TARGETS: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationTarget {
    path: ProjectRelativePath,
    before_digest: Option<ArtifactDigest>,
    after_bytes: Vec<u8>,
}

impl MutationTarget {
    pub fn new(
        path: ProjectRelativePath,
        before_digest: Option<ArtifactDigest>,
        after_bytes: Vec<u8>,
    ) -> Result<Self, StoreError> {
        if after_bytes.is_empty() {
            return Err(StoreError::InvalidJournal {
                reason: "mutation target must not be empty".into(),
            });
        }
        Ok(Self {
            path,
            before_digest,
            after_bytes,
        })
    }

    pub const fn path(&self) -> &ProjectRelativePath {
        &self.path
    }

    pub const fn before_digest(&self) -> Option<ArtifactDigest> {
        self.before_digest
    }

    pub fn after_bytes(&self) -> &[u8] {
        &self.after_bytes
    }

    pub fn after_digest(&self) -> ArtifactDigest {
        ArtifactDigest::digest(&self.after_bytes)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetDescriptor {
    pub path: ProjectRelativePath,
    pub before_digest: Option<ArtifactDigest>,
    pub after_digest: ArtifactDigest,
    pub byte_length: u64,
    pub staged_ref: ProjectRelativePath,
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
#[serde(rename_all = "camelCase")]
struct TargetWire {
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

impl JournalWire {
    fn from_entry(entry: &MutationJournalEntry) -> Result<Self, StoreError> {
        let payload = WireRuntimePayload::from_payload(&entry.event.payload)?;
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
            target_files: entry
                .target_files
                .iter()
                .map(|target| TargetWire {
                    path: target.path.to_string().into_boxed_str(),
                    before_digest: target
                        .before_digest
                        .map(|digest| digest.to_string().into_boxed_str()),
                    after_digest: target.after_digest.to_string().into_boxed_str(),
                    byte_length: target.byte_length,
                    staged_ref: target.staged_ref.to_string().into_boxed_str(),
                })
                .collect(),
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
        if self.schema_version != MUTATION_JOURNAL_SCHEMA_VERSION {
            return Err(StoreError::InvalidJournal {
                reason: format!("unsupported schema version {}", self.schema_version)
                    .into_boxed_str(),
            });
        }
        let target_files = self
            .target_files
            .into_iter()
            .map(|target| {
                Ok(TargetDescriptor {
                    path: ProjectRelativePath::from_str(&target.path)?,
                    before_digest: target
                        .before_digest
                        .map(|value| ArtifactDigest::from_str(&value))
                        .transpose()
                        .map_err(|error| StoreError::InvalidJournal {
                            reason: error.to_string().into_boxed_str(),
                        })?,
                    after_digest: ArtifactDigest::from_str(&target.after_digest).map_err(
                        |error| StoreError::InvalidJournal {
                            reason: error.to_string().into_boxed_str(),
                        },
                    )?,
                    byte_length: target.byte_length,
                    staged_ref: ProjectRelativePath::from_str(&target.staged_ref)?,
                })
            })
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
