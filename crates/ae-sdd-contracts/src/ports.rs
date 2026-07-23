//! Low-level extension descriptors and adapter-facing ports.

use ae_sdd_domain::{
    AgentRole, ArtifactDigest, BootId, CapabilityId, DecisionDigest, EventSequence, EvidenceRef,
    GateId, InputFingerprint, PolicyDigest, ProjectPathScope, ResultDigest, SessionId,
    StateRevision, TurnId, WorkItemId, WorkspaceId,
};
use ae_sdd_protocol::{GateOutcomeKind, OperationScope};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    BoundedText, ControlPlaneError, MutationIntentId, MutationTarget, OperationName,
    RuntimeModuleKey, RuntimeModuleName, SchemaVersion, serde_domain,
};

/// Maximum number of descriptors, proof evidence refs, or applied intents in one port payload.
pub const MAX_PORT_ITEMS: usize = 128;

/// Query for role- and policy-filtered operation descriptors.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationQuery {
    /// Contract schema version.
    pub schema_version: SchemaVersion,
    /// Registered workspace.
    #[serde(with = "serde_domain::workspace_id")]
    pub workspace_id: WorkspaceId,
    /// Authenticated Agent role.
    #[serde(with = "serde_domain::agent_role")]
    pub role: AgentRole,
    /// Requested authorization scope.
    pub scope: OperationScope,
    /// Optional exact operation names.
    pub operation_names: Vec<OperationName>,
    /// Policy snapshot digest.
    #[serde(with = "serde_domain::policy_digest")]
    pub policy_digest: PolicyDigest,
}

/// Versioned operation metadata exposed by, but not owning, the operations registry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationDescriptor {
    /// Contract schema version.
    pub schema_version: SchemaVersion,
    /// Stable operation name.
    pub name: OperationName,
    /// Descriptor version.
    pub version: BoundedText<32>,
    /// Allowed operation scopes.
    pub scopes: Vec<OperationScope>,
    /// Minimum role expected by the descriptor.
    #[serde(with = "serde_domain::agent_role")]
    pub required_role: AgentRole,
    /// Allowed project path scopes.
    #[serde(with = "serde_domain::project_path_scopes")]
    pub allowed_path_scopes: Vec<ProjectPathScope>,
    /// Canonical input schema digest.
    #[serde(with = "serde_domain::artifact_digest")]
    pub input_schema_digest: ArtifactDigest,
    /// Canonical output schema digest.
    #[serde(with = "serde_domain::artifact_digest")]
    pub output_schema_digest: ArtifactDigest,
    /// Whether identical idempotency input replays without another side effect.
    pub idempotent: bool,
}

/// Adapter-facing operation descriptor provider.
pub trait OperationDescriptorProvider {
    /// Lists descriptors authorized by the query; the operations registry remains authoritative.
    fn list(&self, input: &OperationQuery) -> Result<Vec<OperationDescriptor>, ControlPlaneError>;
}

/// Typed request for one fresh Gate proof.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GateProofRequest {
    /// Contract schema version.
    pub schema_version: SchemaVersion,
    /// Registered workspace.
    #[serde(with = "serde_domain::workspace_id")]
    pub workspace_id: WorkspaceId,
    /// Work Item being evaluated.
    #[serde(with = "serde_domain::work_item_id")]
    pub work_item_id: WorkItemId,
    /// Authenticated Agent session.
    #[serde(with = "serde_domain::session_id")]
    pub session_id: SessionId,
    /// Optional turn correlation.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_turn_id"
    )]
    pub turn_id: Option<TurnId>,
    /// Gate identity.
    #[serde(with = "serde_domain::gate_id")]
    pub gate_id: GateId,
    /// Required state revision.
    #[serde(with = "serde_domain::state_revision")]
    pub required_revision: StateRevision,
    /// Gate input fingerprint.
    #[serde(with = "serde_domain::input_fingerprint")]
    pub input_fingerprint: InputFingerprint,
    /// Policy snapshot digest.
    #[serde(with = "serde_domain::policy_digest")]
    pub policy_digest: PolicyDigest,
}

impl GateProofRequest {
    /// Constructs a typed Gate proof request.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        schema_version: SchemaVersion,
        workspace_id: WorkspaceId,
        work_item_id: WorkItemId,
        session_id: SessionId,
        turn_id: Option<TurnId>,
        gate_id: GateId,
        required_revision: StateRevision,
        input_fingerprint: InputFingerprint,
        policy_digest: PolicyDigest,
    ) -> Self {
        Self {
            schema_version,
            workspace_id,
            work_item_id,
            session_id,
            turn_id,
            gate_id,
            required_revision,
            input_fingerprint,
            policy_digest,
        }
    }
}

/// Error returned when a Gate proof exceeds its frozen v1 bounds.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GateProofError {
    /// Evidence collection exceeded the v1 item limit.
    #[error("Gate proof exceeds the frozen v1 evidence limit")]
    EvidenceLimitExceeded,
    /// Proof expiry was missing.
    #[error("Gate proof requires a non-zero expiry instant")]
    MissingExpiry,
}

/// Fresh, digest-bound Gate proof consumed by policy and HookGuard adapters.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(try_from = "GateProofWire", into = "GateProofWire")]
pub struct GateProof {
    schema_version: SchemaVersion,
    gate_id: GateId,
    outcome: GateOutcomeKind,
    source_revision: StateRevision,
    proof_digest: ArtifactDigest,
    evidence_refs: Vec<EvidenceRef>,
    expires_at_unix_ms: u64,
}

impl GateProof {
    /// Constructs and validates a Gate proof.
    pub fn new(
        schema_version: SchemaVersion,
        gate_id: GateId,
        outcome: GateOutcomeKind,
        source_revision: StateRevision,
        proof_digest: ArtifactDigest,
        evidence_refs: Vec<EvidenceRef>,
        expires_at_unix_ms: u64,
    ) -> Result<Self, GateProofError> {
        if evidence_refs.len() > MAX_PORT_ITEMS {
            return Err(GateProofError::EvidenceLimitExceeded);
        }
        if expires_at_unix_ms == 0 {
            return Err(GateProofError::MissingExpiry);
        }
        Ok(Self {
            schema_version,
            gate_id,
            outcome,
            source_revision,
            proof_digest,
            evidence_refs,
            expires_at_unix_ms,
        })
    }

    /// Returns the Gate outcome.
    pub const fn outcome(&self) -> GateOutcomeKind {
        self.outcome
    }
}

impl<'de> Deserialize<'de> for GateProof {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        GateProofWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GateProofWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::gate_id")]
    gate_id: GateId,
    outcome: GateOutcomeKind,
    #[serde(with = "serde_domain::state_revision")]
    source_revision: StateRevision,
    #[serde(with = "serde_domain::artifact_digest")]
    proof_digest: ArtifactDigest,
    #[serde(with = "serde_domain::evidence_refs")]
    evidence_refs: Vec<EvidenceRef>,
    expires_at_unix_ms: u64,
}

impl TryFrom<GateProofWire> for GateProof {
    type Error = GateProofError;

    fn try_from(value: GateProofWire) -> Result<Self, Self::Error> {
        Self::new(
            value.schema_version,
            value.gate_id,
            value.outcome,
            value.source_revision,
            value.proof_digest,
            value.evidence_refs,
            value.expires_at_unix_ms,
        )
    }
}

impl From<GateProof> for GateProofWire {
    fn from(value: GateProof) -> Self {
        Self {
            schema_version: value.schema_version,
            gate_id: value.gate_id,
            outcome: value.outcome,
            source_revision: value.source_revision,
            proof_digest: value.proof_digest,
            evidence_refs: value.evidence_refs,
            expires_at_unix_ms: value.expires_at_unix_ms,
        }
    }
}

/// Adapter-facing fresh Gate proof provider.
pub trait GateProofProvider {
    /// Evaluates a Gate against the exact typed request.
    fn evaluate(&self, input: &GateProofRequest) -> Result<GateProof, ControlPlaneError>;
}

/// Runtime module metadata frozen without creating a contracts-to-runtime dependency.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeModuleDescriptor {
    /// Contract schema version.
    pub schema_version: SchemaVersion,
    /// Stable module key used by the composition root.
    pub key: RuntimeModuleKey,
    /// Human-readable stable module name.
    pub name: RuntimeModuleName,
    /// Module version.
    pub version: BoundedText<32>,
    /// Advertised capabilities.
    #[serde(with = "capability_ids")]
    pub capabilities: Vec<CapabilityId>,
    /// Public contract schema digest.
    #[serde(with = "serde_domain::artifact_digest")]
    pub contract_schema_digest: ArtifactDigest,
    /// Build artifact digest.
    #[serde(with = "serde_domain::artifact_digest")]
    pub build_digest: ArtifactDigest,
}

/// Bounded context passed by the application composition root.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeModuleContext {
    /// Daemon boot identity.
    #[serde(with = "serde_domain::boot_id")]
    pub boot_id: BootId,
    /// Registered workspace.
    #[serde(with = "serde_domain::workspace_id")]
    pub workspace_id: WorkspaceId,
    /// Policy snapshot digest.
    #[serde(with = "serde_domain::policy_digest")]
    pub policy_digest: PolicyDigest,
}

/// One successfully applied lifecycle mutation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MutationApplied {
    /// Intent identity.
    pub intent_id: MutationIntentId,
    /// Logical or project-relative target.
    pub target: MutationTarget,
    /// Optional expected before-image digest.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "optional_artifact_digest"
    )]
    pub expected_digest: Option<ArtifactDigest>,
    /// Actual committed digest.
    #[serde(with = "serde_domain::artifact_digest")]
    pub actual_digest: ArtifactDigest,
}

/// Durable receipt returned by the C1 mutation authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MutationReceipt {
    /// Contract schema version.
    pub schema_version: SchemaVersion,
    /// Lifecycle plan digest.
    #[serde(with = "serde_domain::decision_digest")]
    pub plan_digest: DecisionDigest,
    /// Committed successor revision.
    #[serde(with = "serde_domain::state_revision")]
    pub committed_revision: StateRevision,
    /// Applied mutations in deterministic plan order.
    pub applied: Vec<MutationApplied>,
    /// Durable event cursor after commit.
    #[serde(with = "serde_domain::event_sequence")]
    pub event_sequence: EventSequence,
    /// Canonical receipt digest.
    #[serde(with = "serde_domain::result_digest")]
    pub receipt_digest: ResultDigest,
}

/// C1 sole-writer boundary; implementations provide CAS, fencing, journal, and idempotency.
pub trait MutationIntentApplier {
    /// Applies an already-authorized lifecycle plan transactionally.
    fn apply(&self, plan: &crate::LifecyclePlan) -> Result<MutationReceipt, ControlPlaneError>;
}

mod optional_turn_id {
    use std::str::FromStr;

    use ae_sdd_domain::TurnId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(value: &Option<TurnId>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(|id| id.to_string()).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<TurnId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| TurnId::from_str(&value).map_err(de::Error::custom))
            .transpose()
    }
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

mod capability_ids {
    use ae_sdd_domain::CapabilityId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(value: &[CapabilityId], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<CapabilityId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<String>::deserialize(deserializer)?
            .into_iter()
            .map(|value| CapabilityId::new(value).map_err(de::Error::custom))
            .collect()
    }
}
