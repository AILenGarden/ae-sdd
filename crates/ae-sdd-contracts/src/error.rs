//! Stable cross-Part control-plane errors.

use std::fmt;

use ae_sdd_domain::ArtifactDigest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{MessageKey, ReasonCode, SchemaVersion};

/// Stable semantic error codes shared by all control-plane Parts.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ControlPlaneErrorCode {
    /// Methodology schema is not supported by this runtime.
    MethodologySchemaUnsupported,
    /// Methodology identity or variant appeared more than once.
    MethodologyEntryDuplicate,
    /// Methodology content did not match its declared digest.
    MethodologyDigestMismatch,
    /// A required compact Methodology slice was absent.
    MethodologyCompactMissing,
    /// A plugin attempted an unauthorized override.
    PluginOverrideDenied,
    /// More than one plugin contender remained at the same layer.
    PluginConflict,
    /// Route policy requires explicit user approval.
    RouteApprovalRequired,
    /// Typed route inputs conflict with one another.
    RouteInputConflict,
    /// A Series identity was replayed with different inputs.
    SeriesPlanConflict,
    /// A Series receipt is missing, stale, or outside its contract.
    SeriesReceiptInvalid,
    /// Lifecycle transition policy denied the command.
    LifecycleTransitionDenied,
    /// PRD completion invariants are not satisfied.
    LifecyclePrdIncomplete,
    /// A protected operation needs user confirmation.
    ConfirmationRequired,
    /// A confirmation is not bound to the current digest or revision.
    ConfirmationMismatch,
    /// The selected Host Adapter lacks a required capability.
    HostCapabilityUnsupported,
    /// A correlated Host acknowledgement did not arrive before its deadline.
    HostAckTimeout,
    /// A Host Adapter explicitly rejected the action.
    HostAckRejected,
    /// Host attestation did not prove the requested delegation identity.
    DelegationAttestationFailed,
    /// The Host cannot perform an authenticated compact operation.
    CompactUnsupported,
    /// A resource could not be resolved from an authoritative source.
    ResourceNotFound,
    /// A resource path escaped its project containment boundary.
    ResourceContainmentDenied,
    /// A loaded context proof is stale for the requested revision.
    ContextProofStale,
    /// A document transaction conflicts with the authoritative revision.
    DocumentConflict,
    /// Review infrastructure was invalid and cannot produce PASS.
    ReviewInvalidInfra,
    /// Review exhausted progress or budget without a valid exit.
    ReviewStalled,
    /// A verification execution plan violates worker isolation rules.
    ExecutionPlanInvalid,
    /// A verification worker failed to produce a valid receipt.
    ExecutionFailed,
    /// A contract payload violates schema, bounds, or cross-field invariants.
    ContractValidationFailed,
}

/// Retry classification for a stable control-plane error.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryClass {
    /// Retrying the same request cannot succeed.
    NoRetry,
    /// Correct the typed input before retrying.
    AfterInputRepair,
    /// Obtain explicit user action before retrying.
    AfterUserAction,
    /// A bounded retry may succeed after transient infrastructure recovery.
    Transient,
}

/// Stable remediation item suitable for bounded Agent projections.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Remediation {
    /// Stable remediation category.
    pub code: ReasonCode,
    /// Localization- and prose-independent message key.
    pub message_key: MessageKey,
}

/// Cross-Part error envelope without secrets, host paths, or transcripts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlPlaneError {
    /// Contract schema version.
    pub schema_version: SchemaVersion,
    /// Stable semantic error code.
    pub code: ControlPlaneErrorCode,
    /// Retry policy.
    pub retry: RetryClass,
    /// Stable message key.
    pub message_key: MessageKey,
    /// Bounded remediation list.
    pub remediation: Vec<Remediation>,
    /// Optional digest of redacted structured details held elsewhere.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_artifact_digest"
    )]
    pub details_digest: Option<ArtifactDigest>,
}

impl fmt::Display for ControlPlaneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message_key)
    }
}

impl std::error::Error for ControlPlaneError {}

/// Structural contract validation failure used before semantic processing.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ContractValidationError {
    /// A bounded collection exceeded its limit.
    #[error("{field} exceeds its {max_items}-item limit")]
    CollectionTooLarge {
        /// Field name.
        field: &'static str,
        /// Frozen v1 item limit.
        max_items: usize,
    },
    /// A mandatory bounded collection was empty.
    #[error("{field} cannot be empty")]
    EmptyCollection {
        /// Field name.
        field: &'static str,
    },
    /// Cross-field state was impossible.
    #[error("contract invariant failed: {0}")]
    Invariant(&'static str),
}

mod optional_artifact_digest {
    use std::str::FromStr;

    use ae_sdd_domain::ArtifactDigest;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(
        value: &Option<ArtifactDigest>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(|digest| digest.to_string()).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<ArtifactDigest>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| ArtifactDigest::from_str(&value).map_err(de::Error::custom))
            .transpose()
    }
}
