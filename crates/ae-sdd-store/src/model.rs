use std::{fmt, str::FromStr};

use ae_sdd_domain::{
    ArtifactDigest, BootId, CompactId, ContextDigest, ContextGeneration, ContextProjectionId,
    ContextRevision, DecisionDigest, DelegationId, EventSequence, EventStoreId, FencingToken,
    HostAckId, HostActionId, InputFingerprint, InventoryGeneration, OperationId, PolicyDigest,
    RequestId, ResultDigest, SessionId, StateRevision, WorkItemId, WorkspaceId,
};
use serde::{Deserialize, Serialize};

use crate::StoreError;

pub const MAX_RUNTIME_EVENT_PAYLOAD_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UtcTimestamp(jiff::Timestamp);

impl UtcTimestamp {
    pub fn now() -> Self {
        Self(jiff::Timestamp::now())
    }

    pub const fn as_timestamp(&self) -> &jiff::Timestamp {
        &self.0
    }
}

impl fmt::Display for UtcTimestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for UtcTimestamp {
    type Err = StoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value
            .parse::<jiff::Timestamp>()
            .map(Self)
            .map_err(|error| StoreError::InvalidState {
                reason: format!("invalid UTC timestamp: {error}").into_boxed_str(),
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdempotencyKey(Box<str>);

impl IdempotencyKey {
    pub const MAX_BYTES: usize = 256;

    pub fn new(value: impl Into<Box<str>>) -> Result<Self, StoreError> {
        let value = value.into();
        if value.is_empty() {
            return Err(StoreError::InvalidIdempotencyKey {
                reason: "key must not be empty",
            });
        }
        if value.len() > Self::MAX_BYTES {
            return Err(StoreError::InvalidIdempotencyKey {
                reason: "key exceeds 256 bytes",
            });
        }
        if value
            .bytes()
            .any(|byte| !byte.is_ascii_alphanumeric() && !b"._:-".contains(&byte))
        {
            return Err(StoreError::InvalidIdempotencyKey {
                reason: "key contains a non-portable character",
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn digest(&self) -> InputFingerprint {
        InputFingerprint::digest(self.0.as_bytes())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationReceipt {
    pub workspace_id: WorkspaceId,
    pub work_item_id: WorkItemId,
    pub idempotency_key: IdempotencyKey,
    pub payload_digest: InputFingerprint,
    pub operation: OperationId,
    pub revision_before: StateRevision,
    pub revision_after: StateRevision,
    pub fencing_token: FencingToken,
    pub result_digest: ResultDigest,
    pub mutation_id: RequestId,
    pub committed_at: UtcTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeEventPayload {
    InlineJson(Vec<u8>),
    ArtifactRef {
        project_relative_path: Box<str>,
        digest: ArtifactDigest,
        byte_length: u64,
    },
}

impl RuntimeEventPayload {
    pub fn validate(&self) -> Result<(), StoreError> {
        match self {
            Self::InlineJson(bytes) => {
                if bytes.len() > MAX_RUNTIME_EVENT_PAYLOAD_BYTES {
                    return Err(StoreError::PayloadTooLarge {
                        maximum: MAX_RUNTIME_EVENT_PAYLOAD_BYTES,
                        actual: bytes.len(),
                    });
                }
                let value: serde_json::Value =
                    serde_json::from_slice(bytes).map_err(|error| StoreError::InvalidJournal {
                        reason: format!("event payload is not JSON: {error}").into_boxed_str(),
                    })?;
                let canonical =
                    serde_json::to_vec(&value).map_err(|error| StoreError::InvalidJournal {
                        reason: format!("event payload cannot be canonicalized: {error}")
                            .into_boxed_str(),
                    })?;
                if &canonical != bytes {
                    return Err(StoreError::InvalidJournal {
                        reason: "inline event JSON must be canonical compact JSON".into(),
                    });
                }
                Ok(())
            }
            Self::ArtifactRef {
                project_relative_path,
                byte_length,
                ..
            } => {
                ae_sdd_domain::ProjectRelativePath::new(project_relative_path.clone())?;
                if *byte_length == 0 {
                    return Err(StoreError::InvalidJournal {
                        reason: "event payloadRef byte length must be non-zero".into(),
                    });
                }
                Ok(())
            }
        }
    }

    pub fn digest(&self) -> ArtifactDigest {
        match self {
            Self::InlineJson(bytes) => ArtifactDigest::digest(bytes),
            Self::ArtifactRef { digest, .. } => *digest,
        }
    }

    pub fn byte_length(&self) -> u64 {
        match self {
            Self::InlineJson(bytes) => u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            Self::ArtifactRef { byte_length, .. } => *byte_length,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeEventDraft {
    pub boot_id: BootId,
    pub workspace_id: WorkspaceId,
    pub session_id: Option<SessionId>,
    pub work_item_id: WorkItemId,
    pub event_type: Box<str>,
    pub schema_version: u32,
    pub payload: RuntimeEventPayload,
    pub committed_at: UtcTimestamp,
}

impl RuntimeEventDraft {
    pub fn validate(&self) -> Result<(), StoreError> {
        if self.event_type.is_empty()
            || self.event_type.len() > 128
            || self
                .event_type
                .bytes()
                .any(|byte| !byte.is_ascii_alphanumeric() && !b"._:-".contains(&byte))
        {
            return Err(StoreError::InvalidJournal {
                reason: "event type is invalid".into(),
            });
        }
        if self.schema_version == 0 {
            return Err(StoreError::InvalidJournal {
                reason: "event schema version must be non-zero".into(),
            });
        }
        self.payload.validate()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeEventRecord {
    pub event_store_id: EventStoreId,
    pub event_sequence: EventSequence,
    pub draft: RuntimeEventDraft,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DelegationRecord {
    pub delegation_id: DelegationId,
    pub workspace_id: WorkspaceId,
    pub root_session_id: SessionId,
    pub parent_session_id: SessionId,
    pub child_session_id: Option<SessionId>,
    pub parent_delegation_id: Option<DelegationId>,
    pub role: Box<str>,
    pub input_revision: StateRevision,
    pub input_fingerprint: InputFingerprint,
    pub status: Box<str>,
    pub deadline: UtcTimestamp,
    pub receipt_digest: ResultDigest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DelegationRequestReceipt {
    pub workspace_id: WorkspaceId,
    pub parent_session_id: SessionId,
    pub idempotency_key: IdempotencyKey,
    pub request_digest: InputFingerprint,
    pub delegation_id: DelegationId,
    pub response_digest: ResultDigest,
    pub created_at: UtcTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildResultRecord {
    pub delegation_id: DelegationId,
    pub schema_version: u32,
    pub result_digest: ResultDigest,
    pub byte_length: u64,
    pub validation_status: Box<str>,
    pub artifact_ref: Box<str>,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryCleanupReceipt {
    pub delegation_id: DelegationId,
    pub namespace: Box<str>,
    pub snapshot_digest: ArtifactDigest,
    pub cleanup_digest: ResultDigest,
    pub cleaned_at: UtcTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostAdapterRecord {
    pub adapter_id: Box<str>,
    pub capability_digest: ArtifactDigest,
    pub status: Box<str>,
    pub last_command_sequence: u64,
    pub heartbeat_at: UtcTimestamp,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostActionRecord {
    pub action_id: HostActionId,
    pub adapter_id: Box<str>,
    pub kind: Box<str>,
    pub command_sequence: u64,
    pub request_digest: InputFingerprint,
    pub session_id: Option<SessionId>,
    pub context_generation: Option<ContextGeneration>,
    pub ack_status: Box<str>,
    pub deadline: UtcTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostAckReceipt {
    pub ack_id: HostAckId,
    pub action_id: HostActionId,
    pub adapter_id: Box<str>,
    pub response_digest: ResultDigest,
    pub acknowledged_at: UtcTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextPressureSampleRecord {
    pub adapter_id: Box<str>,
    pub session_id: SessionId,
    pub context_generation: ContextGeneration,
    pub sample_sequence: u64,
    pub used_tokens: u64,
    pub context_window_tokens: u64,
    pub source: Box<str>,
    pub observed_at: UtcTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextProjectionRecord {
    pub projection_id: ContextProjectionId,
    pub session_id: SessionId,
    pub context_revision: ContextRevision,
    pub source_revision: StateRevision,
    pub policy_digest: PolicyDigest,
    pub inventory_generation: InventoryGeneration,
    pub digest: ContextDigest,
    pub byte_budget: u64,
    pub expires_at: UtcTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactCycleRecord {
    pub compact_id: CompactId,
    pub session_id: SessionId,
    pub snapshot_ref: Box<str>,
    pub previous_generation: ContextGeneration,
    pub next_generation: ContextGeneration,
    pub host_action_id: HostActionId,
    pub status: Box<str>,
    pub deadline: UtcTimestamp,
    pub restored_digest: Option<ContextDigest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupervisorCheckpointRecord {
    pub workspace_id: WorkspaceId,
    pub work_item_id: WorkItemId,
    pub last_event_sequence: EventSequence,
    pub last_event_digest: ArtifactDigest,
    pub state_revision: StateRevision,
    pub input_fingerprint: InputFingerprint,
    pub policy_digest: PolicyDigest,
    pub last_decision_digest: DecisionDigest,
    pub health: Box<str>,
    pub updated_at: UtcTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookEventReceipt {
    pub session_id: SessionId,
    pub hook_event_id: Box<str>,
    pub request_digest: InputFingerprint,
    pub decision_digest: ResultDigest,
    pub event_sequence: Option<EventSequence>,
    pub created_at: UtcTimestamp,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct WireRuntimePayload {
    #[serde(rename = "payloadJson", skip_serializing_if = "Option::is_none")]
    pub payload_json: Option<serde_json::Value>,
    #[serde(rename = "payloadRef", skip_serializing_if = "Option::is_none")]
    pub payload_ref: Option<Box<str>>,
    #[serde(rename = "payloadDigest")]
    pub payload_digest: Box<str>,
    #[serde(rename = "byteLen")]
    pub byte_length: u64,
}
