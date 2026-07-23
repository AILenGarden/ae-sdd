//! Frozen Host Adapter action and acknowledgement contracts.

use ae_sdd_domain::{
    AgentRole, ClaimId, ContextDigest, ContextGeneration, DelegationId, HostAckId, HostActionId,
    InputFingerprint, SessionId,
};
use ae_sdd_protocol::{HostAckOutcome, HostActionKind, StableErrorCode};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use thiserror::Error;

use crate::{
    AdapterId, BoundedText, ContextBundleId, ContractValueError, HostTaskId, MessageKey,
    SchemaVersion, compact::CompactRequest, serde_domain,
};

/// Maximum UTF-8 byte length of one host message action.
pub const MAX_HOST_MESSAGE_BYTES: usize = 32_768;

/// Maximum UTF-8 byte length of an auditable host cancellation reason.
pub const MAX_HOST_CANCEL_REASON_BYTES: usize = 1_024;

/// Maximum wait duration accepted by the frozen Host Adapter contract.
pub const MAX_HOST_WAIT_MILLIS: u64 = 3_600_000;

/// Typed create-session payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateHostAction {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::delegation_id")]
    delegation_id: DelegationId,
    #[serde(with = "serde_domain::agent_role")]
    child_role: AgentRole,
    context_bundle_id: ContextBundleId,
}

impl CreateHostAction {
    /// Validates a physical child-session creation payload.
    pub fn new(
        schema_version: SchemaVersion,
        delegation_id: DelegationId,
        child_role: AgentRole,
        context_bundle_id: ContextBundleId,
    ) -> Result<Self, HostContractError> {
        if child_role == AgentRole::Root {
            return Err(HostContractError::RootChildForbidden);
        }
        Ok(Self {
            schema_version,
            delegation_id,
            child_role,
            context_bundle_id,
        })
    }

    /// Returns the delegation being materialized.
    #[must_use]
    pub const fn delegation_id(&self) -> DelegationId {
        self.delegation_id
    }

    /// Returns the daemon-owned child role.
    #[must_use]
    pub const fn child_role(&self) -> AgentRole {
        self.child_role
    }

    /// Returns the bounded context bundle identity supplied to the child.
    #[must_use]
    pub const fn context_bundle_id(&self) -> &ContextBundleId {
        &self.context_bundle_id
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateHostActionWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::delegation_id")]
    delegation_id: DelegationId,
    #[serde(with = "serde_domain::agent_role")]
    child_role: AgentRole,
    context_bundle_id: ContextBundleId,
}

impl<'de> Deserialize<'de> for CreateHostAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = CreateHostActionWire::deserialize(deserializer)?;
        Self::new(
            wire.schema_version,
            wire.delegation_id,
            wire.child_role,
            wire.context_bundle_id,
        )
        .map_err(de::Error::custom)
    }
}

/// Typed bounded message payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendHostAction {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::session_id")]
    session_id: SessionId,
    message_key: MessageKey,
    content: BoundedText<MAX_HOST_MESSAGE_BYTES>,
}

impl SendHostAction {
    /// Validates a bounded host message.
    pub fn new(
        schema_version: SchemaVersion,
        session_id: SessionId,
        message_key: MessageKey,
        content: impl Into<Box<str>>,
    ) -> Result<Self, HostContractError> {
        let content = BoundedText::new(content)?;
        if content.is_empty() {
            return Err(HostContractError::EmptyMessage);
        }
        Ok(Self {
            schema_version,
            session_id,
            message_key,
            content,
        })
    }

    /// Returns the target physical session.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Returns the retry-safe message identity.
    #[must_use]
    pub const fn message_key(&self) -> &MessageKey {
        &self.message_key
    }

    /// Returns the bounded message body.
    #[must_use]
    pub const fn content(&self) -> &BoundedText<MAX_HOST_MESSAGE_BYTES> {
        &self.content
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SendHostActionWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::session_id")]
    session_id: SessionId,
    message_key: MessageKey,
    content: BoundedText<MAX_HOST_MESSAGE_BYTES>,
}

impl<'de> Deserialize<'de> for SendHostAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SendHostActionWire::deserialize(deserializer)?;
        Self::new(
            wire.schema_version,
            wire.session_id,
            wire.message_key,
            wire.content.as_str(),
        )
        .map_err(de::Error::custom)
    }
}

/// Typed host wait payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WaitHostAction {
    schema_version: SchemaVersion,
    host_task_id: HostTaskId,
    timeout_ms: u64,
}

impl WaitHostAction {
    /// Validates a bounded host wait.
    pub fn new(
        schema_version: SchemaVersion,
        host_task_id: HostTaskId,
        timeout_ms: u64,
    ) -> Result<Self, HostContractError> {
        if timeout_ms == 0 || timeout_ms > MAX_HOST_WAIT_MILLIS {
            return Err(HostContractError::InvalidWaitTimeout {
                maximum: MAX_HOST_WAIT_MILLIS,
                actual: timeout_ms,
            });
        }
        Ok(Self {
            schema_version,
            host_task_id,
            timeout_ms,
        })
    }

    /// Returns the target host task.
    #[must_use]
    pub const fn host_task_id(&self) -> &HostTaskId {
        &self.host_task_id
    }

    /// Returns the bounded wait duration.
    #[must_use]
    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WaitHostActionWire {
    schema_version: SchemaVersion,
    host_task_id: HostTaskId,
    timeout_ms: u64,
}

impl<'de> Deserialize<'de> for WaitHostAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = WaitHostActionWire::deserialize(deserializer)?;
        Self::new(wire.schema_version, wire.host_task_id, wire.timeout_ms)
            .map_err(de::Error::custom)
    }
}

/// Exact host cancellation target.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "targetKind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HostCancelTarget {
    /// Cancel one host task without broadening to its full session.
    HostTask {
        /// Stable host task identity.
        host_task_id: HostTaskId,
    },
    /// Cancel one physical host session.
    Session {
        /// Trusted daemon session identity.
        #[serde(with = "serde_domain::session_id")]
        session_id: SessionId,
    },
}

/// Typed host cancellation payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelHostAction {
    schema_version: SchemaVersion,
    target: HostCancelTarget,
    reason: BoundedText<MAX_HOST_CANCEL_REASON_BYTES>,
}

impl CancelHostAction {
    /// Validates a targeted host cancellation.
    pub fn new(
        schema_version: SchemaVersion,
        target: HostCancelTarget,
        reason: impl Into<Box<str>>,
    ) -> Result<Self, HostContractError> {
        let reason = BoundedText::new(reason)?;
        if reason.is_empty() {
            return Err(HostContractError::EmptyCancelReason);
        }
        Ok(Self {
            schema_version,
            target,
            reason,
        })
    }

    /// Returns the exact cancellation target.
    #[must_use]
    pub const fn target(&self) -> &HostCancelTarget {
        &self.target
    }

    /// Returns the bounded auditable reason.
    #[must_use]
    pub const fn reason(&self) -> &BoundedText<MAX_HOST_CANCEL_REASON_BYTES> {
        &self.reason
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CancelHostActionWire {
    schema_version: SchemaVersion,
    target: HostCancelTarget,
    reason: BoundedText<MAX_HOST_CANCEL_REASON_BYTES>,
}

impl<'de> Deserialize<'de> for CancelHostAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = CancelHostActionWire::deserialize(deserializer)?;
        Self::new(wire.schema_version, wire.target, wire.reason.as_str()).map_err(de::Error::custom)
    }
}

/// Typed physical child-attestation payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttestHostAction {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::delegation_id")]
    delegation_id: DelegationId,
    #[serde(with = "serde_domain::claim_id")]
    claim_id: ClaimId,
    #[serde(with = "serde_domain::session_id")]
    child_session_id: SessionId,
    #[serde(with = "serde_domain::agent_role")]
    child_role: AgentRole,
    #[serde(with = "serde_domain::context_digest")]
    attestation_digest: ContextDigest,
}

impl AttestHostAction {
    /// Validates a typed physical child identity claim.
    pub fn new(
        schema_version: SchemaVersion,
        delegation_id: DelegationId,
        claim_id: ClaimId,
        child_session_id: SessionId,
        child_role: AgentRole,
        attestation_digest: ContextDigest,
    ) -> Result<Self, HostContractError> {
        if child_role == AgentRole::Root {
            return Err(HostContractError::RootChildForbidden);
        }
        Ok(Self {
            schema_version,
            delegation_id,
            claim_id,
            child_session_id,
            child_role,
            attestation_digest,
        })
    }

    /// Returns the delegation being attested.
    #[must_use]
    pub const fn delegation_id(&self) -> DelegationId {
        self.delegation_id
    }

    /// Returns the single-use physical child claim.
    #[must_use]
    pub const fn claim_id(&self) -> ClaimId {
        self.claim_id
    }

    /// Returns the claimed child session.
    #[must_use]
    pub const fn child_session_id(&self) -> SessionId {
        self.child_session_id
    }

    /// Returns the daemon-owned child role.
    #[must_use]
    pub const fn child_role(&self) -> AgentRole {
        self.child_role
    }

    /// Returns the digest of the authenticated physical-session proof.
    #[must_use]
    pub const fn attestation_digest(&self) -> ContextDigest {
        self.attestation_digest
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AttestHostActionWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::delegation_id")]
    delegation_id: DelegationId,
    #[serde(with = "serde_domain::claim_id")]
    claim_id: ClaimId,
    #[serde(with = "serde_domain::session_id")]
    child_session_id: SessionId,
    #[serde(with = "serde_domain::agent_role")]
    child_role: AgentRole,
    #[serde(with = "serde_domain::context_digest")]
    attestation_digest: ContextDigest,
}

impl<'de> Deserialize<'de> for AttestHostAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = AttestHostActionWire::deserialize(deserializer)?;
        Self::new(
            wire.schema_version,
            wire.delegation_id,
            wire.claim_id,
            wire.child_session_id,
            wire.child_role,
            wire.attestation_digest,
        )
        .map_err(de::Error::custom)
    }
}

/// Typed host compact payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactHostAction {
    schema_version: SchemaVersion,
    request: CompactRequest,
}

impl CompactHostAction {
    /// Wraps a generation-checked compact request for host dispatch.
    #[must_use]
    pub const fn new(request: CompactRequest) -> Self {
        Self {
            schema_version: request.schema_version(),
            request,
        }
    }

    /// Returns the generation-checked compact request.
    #[must_use]
    pub const fn request(&self) -> &CompactRequest {
        &self.request
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompactHostActionWire {
    schema_version: SchemaVersion,
    request: CompactRequest,
}

impl<'de> Deserialize<'de> for CompactHostAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = CompactHostActionWire::deserialize(deserializer)?;
        if wire.schema_version != wire.request.schema_version() {
            return Err(de::Error::custom("compact action schema version mismatch"));
        }
        Ok(Self::new(wire.request))
    }
}

/// Exact typed payload of a Host Adapter command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum HostActionBody {
    /// Create a physical child Agent session.
    Create(CreateHostAction),
    /// Send bounded input to a physical session.
    Send(SendHostAction),
    /// Wait for a bounded host-side lifecycle interval.
    Wait(WaitHostAction),
    /// Cancel one typed host target.
    Cancel(CancelHostAction),
    /// Attest a physical child identity and claim.
    Attest(AttestHostAction),
    /// Compact one exact session generation.
    Compact(CompactHostAction),
}

impl HostActionBody {
    /// Constructs a typed physical child creation command.
    pub fn create(
        delegation_id: DelegationId,
        child_role: AgentRole,
        context_bundle_id: ContextBundleId,
    ) -> Result<Self, HostContractError> {
        CreateHostAction::new(
            SchemaVersion::V1,
            delegation_id,
            child_role,
            context_bundle_id,
        )
        .map(Self::Create)
    }

    /// Constructs a typed bounded message command.
    pub fn send(
        session_id: SessionId,
        message_key: MessageKey,
        content: impl Into<Box<str>>,
    ) -> Result<Self, HostContractError> {
        SendHostAction::new(SchemaVersion::V1, session_id, message_key, content).map(Self::Send)
    }

    /// Constructs a typed bounded wait command.
    pub fn wait(host_task_id: HostTaskId, timeout_ms: u64) -> Result<Self, HostContractError> {
        WaitHostAction::new(SchemaVersion::V1, host_task_id, timeout_ms).map(Self::Wait)
    }

    /// Constructs a typed targeted cancellation command.
    pub fn cancel(
        target: HostCancelTarget,
        reason: impl Into<Box<str>>,
    ) -> Result<Self, HostContractError> {
        CancelHostAction::new(SchemaVersion::V1, target, reason).map(Self::Cancel)
    }

    /// Constructs a typed physical child-attestation command.
    pub fn attest(
        delegation_id: DelegationId,
        claim_id: ClaimId,
        child_session_id: SessionId,
        child_role: AgentRole,
        attestation_digest: ContextDigest,
    ) -> Result<Self, HostContractError> {
        AttestHostAction::new(
            SchemaVersion::V1,
            delegation_id,
            claim_id,
            child_session_id,
            child_role,
            attestation_digest,
        )
        .map(Self::Attest)
    }

    /// Constructs a typed generation-checked compact command.
    #[must_use]
    pub fn compact(request: CompactRequest) -> Self {
        Self::Compact(CompactHostAction::new(request))
    }

    /// Returns the protocol-owned action kind.
    #[must_use]
    pub const fn kind(&self) -> HostActionKind {
        match self {
            Self::Create(_) => HostActionKind::Create,
            Self::Send(_) => HostActionKind::Send,
            Self::Wait(_) => HostActionKind::Wait,
            Self::Cancel(_) => HostActionKind::Cancel,
            Self::Attest(_) => HostActionKind::Attest,
            Self::Compact(_) => HostActionKind::Compact,
        }
    }

    const fn schema_version(&self) -> SchemaVersion {
        match self {
            Self::Create(value) => value.schema_version,
            Self::Send(value) => value.schema_version,
            Self::Wait(value) => value.schema_version,
            Self::Cancel(value) => value.schema_version,
            Self::Attest(value) => value.schema_version,
            Self::Compact(value) => value.schema_version,
        }
    }
}

/// Durable typed command dispatched to an authenticated Host Adapter.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostAction {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::host_action_id")]
    action_id: HostActionId,
    adapter_id: AdapterId,
    command_seq: u64,
    #[serde(with = "serde_domain::input_fingerprint")]
    request_digest: InputFingerprint,
    deadline_unix_ms: u64,
    body: HostActionBody,
}

impl HostAction {
    /// Validates and constructs a Host Adapter command.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        action_id: HostActionId,
        adapter_id: AdapterId,
        command_seq: u64,
        request_digest: InputFingerprint,
        deadline_unix_ms: u64,
        body: HostActionBody,
    ) -> Result<Self, HostContractError> {
        if command_seq == 0 {
            return Err(HostContractError::ZeroCommandSequence);
        }
        if deadline_unix_ms == 0 {
            return Err(HostContractError::ZeroDeadline);
        }
        if schema_version != body.schema_version() {
            return Err(HostContractError::SchemaVersionMismatch);
        }
        if let HostActionBody::Compact(compact) = &body {
            let request = compact.request();
            if request.adapter_id() != &adapter_id || request.deadline_unix_ms() != deadline_unix_ms
            {
                return Err(HostContractError::CompactActionMismatch);
            }
        }
        Ok(Self {
            schema_version,
            action_id,
            adapter_id,
            command_seq,
            request_digest,
            deadline_unix_ms,
            body,
        })
    }

    /// Returns the contract schema version.
    #[must_use]
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the durable host action identity.
    #[must_use]
    pub const fn action_id(&self) -> HostActionId {
        self.action_id
    }

    /// Returns the authenticated target adapter.
    #[must_use]
    pub const fn adapter_id(&self) -> &AdapterId {
        &self.adapter_id
    }

    /// Returns the adapter-scoped monotonic command sequence.
    #[must_use]
    pub const fn command_seq(&self) -> u64 {
        self.command_seq
    }

    /// Returns the canonical request fingerprint bound into the ACK.
    #[must_use]
    pub const fn request_digest(&self) -> InputFingerprint {
        self.request_digest
    }

    /// Returns the absolute acknowledgement deadline.
    #[must_use]
    pub const fn deadline_unix_ms(&self) -> u64 {
        self.deadline_unix_ms
    }

    /// Returns the exact typed command body.
    #[must_use]
    pub const fn body(&self) -> &HostActionBody {
        &self.body
    }

    /// Returns the protocol-owned action kind.
    #[must_use]
    pub const fn kind(&self) -> HostActionKind {
        self.body.kind()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HostActionWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::host_action_id")]
    action_id: HostActionId,
    adapter_id: AdapterId,
    command_seq: u64,
    #[serde(with = "serde_domain::input_fingerprint")]
    request_digest: InputFingerprint,
    deadline_unix_ms: u64,
    body: HostActionBody,
}

impl<'de> Deserialize<'de> for HostAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = HostActionWire::deserialize(deserializer)?;
        Self::new(
            wire.schema_version,
            wire.action_id,
            wire.adapter_id,
            wire.command_seq,
            wire.request_digest,
            wire.deadline_unix_ms,
            wire.body,
        )
        .map_err(de::Error::custom)
    }
}

/// Authenticated, request-correlated acknowledgement from a Host Adapter.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttestedAck {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::host_ack_id")]
    ack_id: HostAckId,
    #[serde(with = "serde_domain::host_action_id")]
    action_id: HostActionId,
    adapter_id: AdapterId,
    command_seq: u64,
    #[serde(with = "serde_domain::input_fingerprint")]
    request_digest: InputFingerprint,
    outcome: HostAckOutcome,
    host_task_id: Option<HostTaskId>,
    #[serde(with = "optional_session_id")]
    session_id: Option<SessionId>,
    #[serde(with = "optional_context_generation")]
    observed_generation: Option<ContextGeneration>,
    error_code: Option<StableErrorCode>,
    observed_at_unix_ms: u64,
}

impl AttestedAck {
    /// Constructs an accepted ACK by copying its immutable correlation fields.
    #[allow(clippy::too_many_arguments)]
    pub fn accepted(
        schema_version: SchemaVersion,
        ack_id: HostAckId,
        action: &HostAction,
        host_task_id: Option<HostTaskId>,
        session_id: Option<SessionId>,
        observed_generation: Option<ContextGeneration>,
        observed_at_unix_ms: u64,
    ) -> Result<Self, HostContractError> {
        let value = Self::from_parts(
            schema_version,
            ack_id,
            action.action_id,
            action.adapter_id.clone(),
            action.command_seq,
            action.request_digest,
            HostAckOutcome::Accepted,
            host_task_id,
            session_id,
            observed_generation,
            None,
            observed_at_unix_ms,
        )?;
        value.validate_for(action)?;
        Ok(value)
    }

    /// Constructs a rejected or failed ACK with a stable protocol error code.
    pub fn rejected(
        schema_version: SchemaVersion,
        ack_id: HostAckId,
        action: &HostAction,
        outcome: HostAckOutcome,
        error_code: StableErrorCode,
        observed_at_unix_ms: u64,
    ) -> Result<Self, HostContractError> {
        if outcome == HostAckOutcome::Accepted {
            return Err(HostContractError::RejectedAckMustNotBeAccepted);
        }
        Self::from_parts(
            schema_version,
            ack_id,
            action.action_id,
            action.adapter_id.clone(),
            action.command_seq,
            action.request_digest,
            outcome,
            None,
            None,
            None,
            Some(error_code),
            observed_at_unix_ms,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        schema_version: SchemaVersion,
        ack_id: HostAckId,
        action_id: HostActionId,
        adapter_id: AdapterId,
        command_seq: u64,
        request_digest: InputFingerprint,
        outcome: HostAckOutcome,
        host_task_id: Option<HostTaskId>,
        session_id: Option<SessionId>,
        observed_generation: Option<ContextGeneration>,
        error_code: Option<StableErrorCode>,
        observed_at_unix_ms: u64,
    ) -> Result<Self, HostContractError> {
        if command_seq == 0 {
            return Err(HostContractError::ZeroCommandSequence);
        }
        if observed_at_unix_ms == 0 {
            return Err(HostContractError::ZeroObservationTime);
        }
        match (outcome, error_code) {
            (HostAckOutcome::Accepted, None)
            | (HostAckOutcome::Rejected | HostAckOutcome::Failed, Some(_)) => {}
            (HostAckOutcome::Accepted, Some(_)) => {
                return Err(HostContractError::AcceptedAckHasError);
            }
            (HostAckOutcome::Rejected | HostAckOutcome::Failed, None) => {
                return Err(HostContractError::RejectedAckMissingError);
            }
        }
        Ok(Self {
            schema_version,
            ack_id,
            action_id,
            adapter_id,
            command_seq,
            request_digest,
            outcome,
            host_task_id,
            session_id,
            observed_generation,
            error_code,
            observed_at_unix_ms,
        })
    }

    /// Validates immutable ACK correlation and action-specific bindings.
    pub fn validate_for(&self, action: &HostAction) -> Result<(), HostContractError> {
        if self.schema_version != action.schema_version
            || self.action_id != action.action_id
            || self.adapter_id != action.adapter_id
            || self.command_seq != action.command_seq
            || self.request_digest != action.request_digest
        {
            return Err(HostContractError::AckCorrelationMismatch);
        }
        if self.outcome != HostAckOutcome::Accepted {
            return Ok(());
        }
        match action.body() {
            HostActionBody::Create(_) if self.host_task_id.is_none() => {
                Err(HostContractError::AcceptedCreateMissingHostTask)
            }
            HostActionBody::Attest(attest)
                if self.session_id != Some(attest.child_session_id()) =>
            {
                Err(HostContractError::AckSessionMismatch)
            }
            HostActionBody::Compact(compact)
                if self.session_id != Some(compact.request().session_id())
                    || self.observed_generation
                        != Some(compact.request().previous_generation()) =>
            {
                Err(HostContractError::AckGenerationMismatch)
            }
            _ => Ok(()),
        }
    }

    /// Returns the contract schema version.
    #[must_use]
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the ACK identity.
    #[must_use]
    pub const fn ack_id(&self) -> HostAckId {
        self.ack_id
    }

    /// Returns the correlated host action identity.
    #[must_use]
    pub const fn action_id(&self) -> HostActionId {
        self.action_id
    }

    /// Returns the authenticated adapter identity.
    #[must_use]
    pub const fn adapter_id(&self) -> &AdapterId {
        &self.adapter_id
    }

    /// Returns the correlated command sequence.
    #[must_use]
    pub const fn command_seq(&self) -> u64 {
        self.command_seq
    }

    /// Returns the correlated canonical request fingerprint.
    #[must_use]
    pub const fn request_digest(&self) -> InputFingerprint {
        self.request_digest
    }

    /// Returns the protocol-owned acknowledgement outcome.
    #[must_use]
    pub const fn outcome(&self) -> HostAckOutcome {
        self.outcome
    }

    /// Returns the optional physical host task identity.
    #[must_use]
    pub const fn host_task_id(&self) -> Option<&HostTaskId> {
        self.host_task_id.as_ref()
    }

    /// Returns the optional physical session identity.
    #[must_use]
    pub const fn session_id(&self) -> Option<SessionId> {
        self.session_id
    }

    /// Returns the optional context generation observed by the host.
    #[must_use]
    pub const fn observed_generation(&self) -> Option<ContextGeneration> {
        self.observed_generation
    }

    /// Returns a stable failure code for rejected or failed actions.
    #[must_use]
    pub const fn error_code(&self) -> Option<StableErrorCode> {
        self.error_code
    }

    /// Returns the host observation timestamp.
    #[must_use]
    pub const fn observed_at_unix_ms(&self) -> u64 {
        self.observed_at_unix_ms
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AttestedAckWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::host_ack_id")]
    ack_id: HostAckId,
    #[serde(with = "serde_domain::host_action_id")]
    action_id: HostActionId,
    adapter_id: AdapterId,
    command_seq: u64,
    #[serde(with = "serde_domain::input_fingerprint")]
    request_digest: InputFingerprint,
    outcome: HostAckOutcome,
    host_task_id: Option<HostTaskId>,
    #[serde(with = "optional_session_id")]
    session_id: Option<SessionId>,
    #[serde(with = "optional_context_generation")]
    observed_generation: Option<ContextGeneration>,
    error_code: Option<StableErrorCode>,
    observed_at_unix_ms: u64,
}

impl<'de> Deserialize<'de> for AttestedAck {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = AttestedAckWire::deserialize(deserializer)?;
        Self::from_parts(
            wire.schema_version,
            wire.ack_id,
            wire.action_id,
            wire.adapter_id,
            wire.command_seq,
            wire.request_digest,
            wire.outcome,
            wire.host_task_id,
            wire.session_id,
            wire.observed_generation,
            wire.error_code,
            wire.observed_at_unix_ms,
        )
        .map_err(de::Error::custom)
    }
}

/// Physical child result whose claim, role and session match an attest action.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttestedHostResult {
    schema_version: SchemaVersion,
    ack: AttestedAck,
    #[serde(with = "serde_domain::delegation_id")]
    delegation_id: DelegationId,
    #[serde(with = "serde_domain::claim_id")]
    claim_id: ClaimId,
    #[serde(with = "serde_domain::session_id")]
    child_session_id: SessionId,
    #[serde(with = "serde_domain::agent_role")]
    child_role: AgentRole,
    #[serde(with = "serde_domain::context_digest")]
    attestation_digest: ContextDigest,
}

impl AttestedHostResult {
    /// Validates and constructs an attested physical child result.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        action: &HostAction,
        ack: AttestedAck,
        delegation_id: DelegationId,
        claim_id: ClaimId,
        child_session_id: SessionId,
        child_role: AgentRole,
        attestation_digest: ContextDigest,
    ) -> Result<Self, HostContractError> {
        ack.validate_for(action)?;
        if schema_version != action.schema_version || schema_version != ack.schema_version {
            return Err(HostContractError::SchemaVersionMismatch);
        }
        if ack.outcome != HostAckOutcome::Accepted {
            return Err(HostContractError::AttestationAckNotAccepted);
        }
        let HostActionBody::Attest(attest) = action.body() else {
            return Err(HostContractError::AttestationActionRequired);
        };
        if attest.delegation_id != delegation_id
            || attest.claim_id != claim_id
            || attest.child_session_id != child_session_id
            || attest.child_role != child_role
            || attest.attestation_digest != attestation_digest
            || ack.session_id != Some(child_session_id)
        {
            return Err(HostContractError::AttestationBindingMismatch);
        }
        Ok(Self {
            schema_version,
            ack,
            delegation_id,
            claim_id,
            child_session_id,
            child_role,
            attestation_digest,
        })
    }

    /// Returns the correlated authenticated ACK.
    #[must_use]
    pub const fn ack(&self) -> &AttestedAck {
        &self.ack
    }

    /// Returns the attested delegation.
    #[must_use]
    pub const fn delegation_id(&self) -> DelegationId {
        self.delegation_id
    }

    /// Returns the single-use claim identity.
    #[must_use]
    pub const fn claim_id(&self) -> ClaimId {
        self.claim_id
    }

    /// Returns the trusted child session identity.
    #[must_use]
    pub const fn child_session_id(&self) -> SessionId {
        self.child_session_id
    }

    /// Returns the trusted daemon-owned child role.
    #[must_use]
    pub const fn child_role(&self) -> AgentRole {
        self.child_role
    }

    /// Returns the authenticated physical-session proof digest.
    #[must_use]
    pub const fn attestation_digest(&self) -> ContextDigest {
        self.attestation_digest
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AttestedHostResultWire {
    schema_version: SchemaVersion,
    ack: AttestedAck,
    #[serde(with = "serde_domain::delegation_id")]
    delegation_id: DelegationId,
    #[serde(with = "serde_domain::claim_id")]
    claim_id: ClaimId,
    #[serde(with = "serde_domain::session_id")]
    child_session_id: SessionId,
    #[serde(with = "serde_domain::agent_role")]
    child_role: AgentRole,
    #[serde(with = "serde_domain::context_digest")]
    attestation_digest: ContextDigest,
}

impl<'de> Deserialize<'de> for AttestedHostResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = AttestedHostResultWire::deserialize(deserializer)?;
        if wire.schema_version != wire.ack.schema_version
            || wire.ack.outcome != HostAckOutcome::Accepted
            || wire.ack.session_id != Some(wire.child_session_id)
            || wire.child_role == AgentRole::Root
        {
            return Err(de::Error::custom("invalid attested host result binding"));
        }
        Ok(Self {
            schema_version: wire.schema_version,
            ack: wire.ack,
            delegation_id: wire.delegation_id,
            claim_id: wire.claim_id,
            child_session_id: wire.child_session_id,
            child_role: wire.child_role,
            attestation_digest: wire.attestation_digest,
        })
    }
}

/// Validation error for Host Adapter contracts.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HostContractError {
    /// A physical child action attempted to create or attest a root role.
    #[error("Host Adapter cannot create or attest a root Agent")]
    RootChildForbidden,
    /// A host message was empty.
    #[error("host message cannot be empty")]
    EmptyMessage,
    /// A wait duration was zero or exceeded the fixed limit.
    #[error("host wait must be in 1..={maximum} milliseconds (actual: {actual})")]
    InvalidWaitTimeout {
        /// Maximum accepted wait duration.
        maximum: u64,
        /// Observed wait duration.
        actual: u64,
    },
    /// A cancellation omitted its auditable reason.
    #[error("host cancellation reason cannot be empty")]
    EmptyCancelReason,
    /// A host action used command sequence zero.
    #[error("host command sequence must be greater than zero")]
    ZeroCommandSequence,
    /// A host action omitted its absolute deadline.
    #[error("host action deadline must be greater than zero")]
    ZeroDeadline,
    /// An ACK omitted its observation timestamp.
    #[error("host acknowledgement observation timestamp must be greater than zero")]
    ZeroObservationTime,
    /// Nested contract schema versions did not match.
    #[error("nested host contract schema versions do not match")]
    SchemaVersionMismatch,
    /// The compact body did not match its host action envelope.
    #[error("compact request adapter or deadline does not match the host action")]
    CompactActionMismatch,
    /// A rejected ACK constructor was called with an accepted outcome.
    #[error("rejected acknowledgement cannot use the accepted outcome")]
    RejectedAckMustNotBeAccepted,
    /// An accepted ACK carried a failure code.
    #[error("accepted acknowledgement cannot carry an error code")]
    AcceptedAckHasError,
    /// A rejected or failed ACK omitted its stable error code.
    #[error("rejected or failed acknowledgement requires a stable error code")]
    RejectedAckMissingError,
    /// Immutable ACK correlation fields did not match the action.
    #[error("host acknowledgement correlation mismatch")]
    AckCorrelationMismatch,
    /// An accepted create action omitted its physical host task identity.
    #[error("accepted create acknowledgement requires a host task identity")]
    AcceptedCreateMissingHostTask,
    /// An accepted attest ACK named the wrong physical session.
    #[error("host acknowledgement session does not match the action")]
    AckSessionMismatch,
    /// An accepted compact ACK named the wrong session or generation.
    #[error("host acknowledgement generation does not match the compact action")]
    AckGenerationMismatch,
    /// An attestation result was built from a non-attest action.
    #[error("attested host result requires an attest action")]
    AttestationActionRequired,
    /// An attestation result used a non-accepted ACK.
    #[error("attested host result requires an accepted ACK")]
    AttestationAckNotAccepted,
    /// Child claim, role, session or proof did not match the attest action.
    #[error("attested host result does not match the action binding")]
    AttestationBindingMismatch,
    /// A bounded contract value was invalid.
    #[error(transparent)]
    ContractValue(#[from] ContractValueError),
}

mod optional_session_id {
    use std::str::FromStr;

    use super::*;

    pub(super) fn serialize<S>(value: &Option<SessionId>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(|id| id.to_string()).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<SessionId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| SessionId::from_str(&value).map_err(de::Error::custom))
            .transpose()
    }
}

mod optional_context_generation {
    use super::*;

    pub(super) fn serialize<S>(
        value: &Option<ContextGeneration>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(ContextGeneration::get).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Option<ContextGeneration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Option::<u64>::deserialize(deserializer)?.map(ContextGeneration::new))
    }
}
