use serde::{Deserialize, Serialize};

/// Schema identifier for the stable, redacted JSON-RPC error data object.
pub const ERROR_DATA_SCHEMA_V1: &str = "ae-sdd-error/v1";

/// Stable machine-readable error codes known to protocol v1.
///
/// Each variant has one unique JSON-RPC server-error number. Unknown values
/// fail closed during deserialization; protocol-minor additions require
/// capability negotiation before a client relies on them.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StableErrorCode {
    /// The local daemon endpoint could not be reached.
    DaemonUnavailable,
    /// A connection attempted a business method before handshake.
    HandshakeRequired,
    /// Client and daemon protocol majors do not overlap.
    ProtocolVersionUnsupported,
    /// A negotiated optional capability is absent.
    CapabilityUnsupported,
    /// OS peer identity or endpoint token authentication failed.
    EndpointAuthFailed,
    /// Expected boot or policy identity differs from the active daemon.
    EndpointStale,
    /// A new request is not admitted while the daemon drains.
    DaemonDraining,
    /// A workspace path escapes the configured allowed roots.
    WorkspaceOutsideAllowedRoot,
    /// Project identity does not match the registered workspace.
    ProjectMismatch,
    /// Project content changed without a corresponding authoritative revision.
    ExternalStateConflict,
    /// Expected project revision is no longer current.
    RevisionConflict,
    /// A protected mutation omitted an active writer lease.
    LeaseRequired,
    /// Another owner holds the active writer lease.
    LeaseConflict,
    /// The caller's writer lease expired.
    LeaseExpired,
    /// A newer writer generation superseded the caller.
    StaleFencingToken,
    /// An idempotency key was reused with a different canonical payload.
    IdempotencyKeyReused,
    /// An operation requiring explicit user approval has none.
    ConfirmationRequired,
    /// Gate inputs changed before the result could be consumed.
    StaleGateResult,
    /// Gate infrastructure or implementation failed.
    GateError,
    /// Gate execution exceeded its deadline.
    GateTimeout,
    /// A required Gate returned blocking business findings.
    GateBlocked,
    /// Session heartbeat or capability validity expired.
    SessionExpired,
    /// Turn identity does not belong to the trusted session.
    TurnIdentityMismatch,
    /// Session role or scoped grant forbids the requested action.
    RoleOperationForbidden,
    /// A delegation would exceed the physical Agent depth limit.
    RunDepthExceeded,
    /// Host runtime lacks a required create, attest, pressure, or compact capability.
    HostCapabilityUnsupported,
    /// A host action was not acknowledged before its deadline.
    HostAckTimeout,
    /// An authenticated host explicitly rejected an action.
    HostAckRejected,
    /// Host ACK, child claim, physical identity, or lineage attestation failed.
    DelegationAttestationFailed,
    /// A bounded child result violates its schema, path, hash, or deliverable contract.
    ChildResultInvalid,
    /// A bounded child result or summary exceeds its byte budget.
    ChildResultTooLarge,
    /// Context projection source revision or digest is stale.
    ContextRevisionStale,
    /// A context projection cannot fit its role-aware byte budget.
    ContextBudgetExceeded,
    /// Host runtime cannot provide the trusted compact contract.
    CompactUnsupported,
    /// A compact acknowledgement was not received before its deadline.
    CompactAckTimeout,
    /// Compact ACK identity, action, session, or generation correlation is invalid.
    CompactAckInvalid,
    /// An event cursor no longer exists in the active event-store epoch.
    EventCursorGap,
    /// A bounded event subscriber could not keep up.
    SubscriberBackpressure,
    /// A job has crossed a point where cancellation is legal.
    JobNotCancellable,
    /// A legacy scope resolver found multiple candidates.
    ScopeAmbiguous,
    /// The selected typed operation is not registered.
    OperationNotRegistered,
    /// The typed operation request violates its strict schema.
    OperationSchemaInvalid,
    /// The approved plan or a required context drifted from the execution capsule.
    ExecutionCapsuleStale,
    /// A slice state transition or ordinal is not legal for the active queue.
    ExecutionSliceInvalid,
    /// Bounded investigation lacks the machine-verified progress to proceed.
    ExecutionProgressRequired,
    /// An execution resource such as the daemon-wide Cargo lock is held.
    ExecutionResourceBusy,
    /// A capsule, tool output, or investigation batch exceeded its budget.
    ExecutionBudgetExceeded,
}

impl StableErrorCode {
    /// All protocol-v1 stable errors in numeric mapping order.
    pub const ALL: &'static [Self] = &[
        Self::DaemonUnavailable,
        Self::HandshakeRequired,
        Self::ProtocolVersionUnsupported,
        Self::CapabilityUnsupported,
        Self::EndpointAuthFailed,
        Self::EndpointStale,
        Self::DaemonDraining,
        Self::WorkspaceOutsideAllowedRoot,
        Self::ProjectMismatch,
        Self::ExternalStateConflict,
        Self::RevisionConflict,
        Self::LeaseRequired,
        Self::LeaseConflict,
        Self::LeaseExpired,
        Self::StaleFencingToken,
        Self::IdempotencyKeyReused,
        Self::ConfirmationRequired,
        Self::StaleGateResult,
        Self::GateError,
        Self::GateTimeout,
        Self::GateBlocked,
        Self::SessionExpired,
        Self::TurnIdentityMismatch,
        Self::RoleOperationForbidden,
        Self::RunDepthExceeded,
        Self::HostCapabilityUnsupported,
        Self::HostAckTimeout,
        Self::HostAckRejected,
        Self::DelegationAttestationFailed,
        Self::ChildResultInvalid,
        Self::ChildResultTooLarge,
        Self::ContextRevisionStale,
        Self::ContextBudgetExceeded,
        Self::CompactUnsupported,
        Self::CompactAckTimeout,
        Self::CompactAckInvalid,
        Self::EventCursorGap,
        Self::SubscriberBackpressure,
        Self::JobNotCancellable,
        Self::ScopeAmbiguous,
        Self::OperationNotRegistered,
        Self::OperationSchemaInvalid,
        Self::ExecutionCapsuleStale,
        Self::ExecutionSliceInvalid,
        Self::ExecutionProgressRequired,
        Self::ExecutionResourceBusy,
        Self::ExecutionBudgetExceeded,
    ];

    /// Returns the exact stable ASCII wire code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DaemonUnavailable => "DAEMON_UNAVAILABLE",
            Self::HandshakeRequired => "HANDSHAKE_REQUIRED",
            Self::ProtocolVersionUnsupported => "PROTOCOL_VERSION_UNSUPPORTED",
            Self::CapabilityUnsupported => "CAPABILITY_UNSUPPORTED",
            Self::EndpointAuthFailed => "ENDPOINT_AUTH_FAILED",
            Self::EndpointStale => "ENDPOINT_STALE",
            Self::DaemonDraining => "DAEMON_DRAINING",
            Self::WorkspaceOutsideAllowedRoot => "WORKSPACE_OUTSIDE_ALLOWED_ROOT",
            Self::ProjectMismatch => "PROJECT_MISMATCH",
            Self::ExternalStateConflict => "EXTERNAL_STATE_CONFLICT",
            Self::RevisionConflict => "REVISION_CONFLICT",
            Self::LeaseRequired => "LEASE_REQUIRED",
            Self::LeaseConflict => "LEASE_CONFLICT",
            Self::LeaseExpired => "LEASE_EXPIRED",
            Self::StaleFencingToken => "STALE_FENCING_TOKEN",
            Self::IdempotencyKeyReused => "IDEMPOTENCY_KEY_REUSED",
            Self::ConfirmationRequired => "CONFIRMATION_REQUIRED",
            Self::StaleGateResult => "STALE_GATE_RESULT",
            Self::GateError => "GATE_ERROR",
            Self::GateTimeout => "GATE_TIMEOUT",
            Self::GateBlocked => "GATE_BLOCKED",
            Self::SessionExpired => "SESSION_EXPIRED",
            Self::TurnIdentityMismatch => "TURN_IDENTITY_MISMATCH",
            Self::RoleOperationForbidden => "ROLE_OPERATION_FORBIDDEN",
            Self::RunDepthExceeded => "RUN_DEPTH_EXCEEDED",
            Self::HostCapabilityUnsupported => "HOST_CAPABILITY_UNSUPPORTED",
            Self::HostAckTimeout => "HOST_ACK_TIMEOUT",
            Self::HostAckRejected => "HOST_ACK_REJECTED",
            Self::DelegationAttestationFailed => "DELEGATION_ATTESTATION_FAILED",
            Self::ChildResultInvalid => "CHILD_RESULT_INVALID",
            Self::ChildResultTooLarge => "CHILD_RESULT_TOO_LARGE",
            Self::ContextRevisionStale => "CONTEXT_REVISION_STALE",
            Self::ContextBudgetExceeded => "CONTEXT_BUDGET_EXCEEDED",
            Self::CompactUnsupported => "COMPACT_UNSUPPORTED",
            Self::CompactAckTimeout => "COMPACT_ACK_TIMEOUT",
            Self::CompactAckInvalid => "COMPACT_ACK_INVALID",
            Self::EventCursorGap => "EVENT_CURSOR_GAP",
            Self::SubscriberBackpressure => "SUBSCRIBER_BACKPRESSURE",
            Self::JobNotCancellable => "JOB_NOT_CANCELLABLE",
            Self::ScopeAmbiguous => "SCOPE_AMBIGUOUS",
            Self::OperationNotRegistered => "OPERATION_NOT_REGISTERED",
            Self::OperationSchemaInvalid => "OPERATION_SCHEMA_INVALID",
            Self::ExecutionCapsuleStale => "EXECUTION_CAPSULE_STALE",
            Self::ExecutionSliceInvalid => "EXECUTION_SLICE_INVALID",
            Self::ExecutionProgressRequired => "EXECUTION_PROGRESS_REQUIRED",
            Self::ExecutionResourceBusy => "EXECUTION_RESOURCE_BUSY",
            Self::ExecutionBudgetExceeded => "EXECUTION_BUDGET_EXCEEDED",
        }
    }

    /// Returns this error's unique number in JSON-RPC's reserved server range.
    #[must_use]
    pub const fn json_rpc_code(self) -> i32 {
        match self {
            Self::DaemonUnavailable => -32_000,
            Self::HandshakeRequired => -32_001,
            Self::ProtocolVersionUnsupported => -32_002,
            Self::CapabilityUnsupported => -32_003,
            Self::EndpointAuthFailed => -32_004,
            Self::EndpointStale => -32_005,
            Self::DaemonDraining => -32_006,
            Self::WorkspaceOutsideAllowedRoot => -32_010,
            Self::ProjectMismatch => -32_011,
            Self::ExternalStateConflict => -32_012,
            Self::RevisionConflict => -32_013,
            Self::LeaseRequired => -32_020,
            Self::LeaseConflict => -32_021,
            Self::LeaseExpired => -32_022,
            Self::StaleFencingToken => -32_023,
            Self::IdempotencyKeyReused => -32_024,
            Self::ConfirmationRequired => -32_025,
            Self::StaleGateResult => -32_030,
            Self::GateError => -32_031,
            Self::GateTimeout => -32_032,
            Self::GateBlocked => -32_033,
            Self::SessionExpired => -32_040,
            Self::TurnIdentityMismatch => -32_041,
            Self::RoleOperationForbidden => -32_050,
            Self::RunDepthExceeded => -32_051,
            Self::HostCapabilityUnsupported => -32_052,
            Self::HostAckTimeout => -32_053,
            Self::HostAckRejected => -32_054,
            Self::DelegationAttestationFailed => -32_055,
            Self::ChildResultInvalid => -32_060,
            Self::ChildResultTooLarge => -32_061,
            Self::ContextRevisionStale => -32_062,
            Self::ContextBudgetExceeded => -32_063,
            Self::CompactUnsupported => -32_070,
            Self::CompactAckTimeout => -32_071,
            Self::CompactAckInvalid => -32_072,
            Self::EventCursorGap => -32_080,
            Self::SubscriberBackpressure => -32_081,
            Self::JobNotCancellable => -32_082,
            Self::ScopeAmbiguous => -32_090,
            Self::OperationNotRegistered => -32_091,
            Self::OperationSchemaInvalid => -32_092,
            Self::ExecutionCapsuleStale => -32_093,
            Self::ExecutionSliceInvalid => -32_094,
            Self::ExecutionProgressRequired => -32_095,
            Self::ExecutionResourceBusy => -32_096,
            Self::ExecutionBudgetExceeded => -32_097,
        }
    }

    /// Returns the conservative default retry classification.
    ///
    /// A `true` value still requires the caller to follow the stable
    /// remediation and recompute stale inputs before retrying.
    #[must_use]
    pub const fn retryable_by_default(self) -> bool {
        matches!(
            self,
            Self::DaemonUnavailable
                | Self::EndpointStale
                | Self::DaemonDraining
                | Self::RevisionConflict
                | Self::LeaseRequired
                | Self::LeaseConflict
                | Self::LeaseExpired
                | Self::StaleFencingToken
                | Self::ConfirmationRequired
                | Self::StaleGateResult
                | Self::GateError
                | Self::GateTimeout
                | Self::GateBlocked
                | Self::SessionExpired
                | Self::HostAckTimeout
                | Self::HostAckRejected
                | Self::ChildResultInvalid
                | Self::ChildResultTooLarge
                | Self::ContextRevisionStale
                | Self::ContextBudgetExceeded
                | Self::CompactAckTimeout
                | Self::EventCursorGap
                | Self::SubscriberBackpressure
                | Self::ScopeAmbiguous
                | Self::OperationSchemaInvalid
                | Self::ExecutionCapsuleStale
                | Self::ExecutionProgressRequired
                | Self::ExecutionResourceBusy
                | Self::ExecutionBudgetExceeded
        )
    }
}

/// Versioned, redacted application data attached to a JSON-RPC error.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcErrorData {
    /// Error data schema identifier.
    pub schema_version: String,
    /// Stable machine-readable error code.
    pub stable_code: StableErrorCode,
    /// Conservative retry classification.
    pub retryable: bool,
    /// Redacted actionable remediation for the caller.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
    /// Correlation identity that is safe to expose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Digest of separately stored diagnostic details, never their secret contents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details_digest: Option<String>,
}

/// JSON-RPC error object with a single canonical stable-code mapping.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcErrorObject {
    /// Unique numeric code in the JSON-RPC server-error range.
    pub code: i32,
    /// Stable ASCII message suitable for diagnostics, not machine branching.
    pub message: String,
    /// Versioned, redacted machine-readable details.
    pub data: RpcErrorData,
}

impl RpcErrorObject {
    /// Constructs an error using the canonical numeric and retry mappings.
    #[must_use]
    pub fn new(
        stable_code: StableErrorCode,
        message: impl Into<String>,
        remediation: Option<String>,
        request_id: Option<String>,
    ) -> Self {
        Self {
            code: stable_code.json_rpc_code(),
            message: message.into(),
            data: RpcErrorData {
                schema_version: ERROR_DATA_SCHEMA_V1.to_owned(),
                stable_code,
                retryable: stable_code.retryable_by_default(),
                remediation,
                request_id,
                details_digest: None,
            },
        }
    }
}
