//! Toolset requirement and the ToolsetPort trait.

use ae_sdd_contracts::execution::{VerificationExecutionPlan, VerificationReceipt};
use ae_sdd_contracts::{
    ControlPlaneError, ControlPlaneErrorCode, MethodologyRef, RetryClass, SchemaVersion,
    VerificationContractId,
};
use ae_sdd_domain::{ArtifactDigest, InputFingerprint, WorkItemId};
use serde::{Deserialize, Serialize};

use crate::receipt::validate_against_plan;

/// Bounded mandatory toolset requirement derived from a Methodology dependency.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolsetRequirement {
    schema_version: SchemaVersion,
    verification_contract_id: VerificationContractId,
    input_fingerprint: InputFingerprint,
    methodology_digest: ArtifactDigest,
    mandatory: bool,
}

impl ToolsetRequirement {
    /// Constructs a toolset requirement bound to a methodology digest.
    pub const fn new(
        schema_version: SchemaVersion,
        verification_contract_id: VerificationContractId,
        input_fingerprint: InputFingerprint,
        methodology_digest: ArtifactDigest,
        mandatory: bool,
    ) -> Self {
        Self {
            schema_version,
            verification_contract_id,
            input_fingerprint,
            methodology_digest,
            mandatory,
        }
    }

    /// Returns whether the toolset is mandatory for gate admission.
    pub const fn mandatory(&self) -> bool {
        self.mandatory
    }

    /// Returns the verification contract identifier.
    pub fn verification_contract_id(&self) -> &VerificationContractId {
        &self.verification_contract_id
    }

    /// Returns the input fingerprint bound to the requirement.
    pub const fn input_fingerprint(&self) -> InputFingerprint {
        self.input_fingerprint
    }

    /// Returns the Methodology entry digest that derived the requirement.
    pub const fn methodology_digest(&self) -> ArtifactDigest {
        self.methodology_digest
    }
}

/// Query used to derive a toolset requirement for one verification contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolsetQuery {
    /// Contract schema version.
    pub schema_version: SchemaVersion,
    /// Methodology reference backing the verification contract.
    pub methodology_ref: MethodologyRef,
    /// Verification contract identifier declared by the Methodology catalog.
    pub verification_contract_id: VerificationContractId,
    /// Work Item owning the verification.
    pub work_item_id: WorkItemId,
    /// Input fingerprint the verification must be bound to.
    pub input_fingerprint: InputFingerprint,
    /// Whether the toolset is mandatory for gate admission.
    pub mandatory: bool,
}

/// Adapter-facing toolset port.
///
/// Implementations are C1-owned; Part D only consumes the trait so the pure
/// policy layer can be tested with a frozen mock.
pub trait ToolsetPort {
    /// Derives a toolset requirement for the supplied query.
    fn require(&self, query: &ToolsetQuery) -> Result<ToolsetRequirement, ControlPlaneError>;

    /// Records a verification receipt against the originating plan.
    fn record_receipt(
        &self,
        plan: &VerificationExecutionPlan,
        receipt: &VerificationReceipt,
    ) -> Result<(), ControlPlaneError> {
        validate_against_plan(plan, receipt)
            .map_err(|error| control_plane_error(error.error_code()))
    }
}

fn control_plane_error(code: ControlPlaneErrorCode) -> ControlPlaneError {
    ControlPlaneError {
        schema_version: SchemaVersion::V1,
        code,
        retry: RetryClass::NoRetry,
        message_key: ae_sdd_contracts::MessageKey::invariant_fallback(),
        remediation: Vec::new(),
        details_digest: None,
    }
}

// The serde derives below are intentionally minimal: the requirement is the
// only payload this crate needs to serialise for diagnostic purposes.
impl Serialize for ToolsetRequirement {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ToolsetRequirement", 5)?;
        state.serialize_field("schemaVersion", &self.schema_version)?;
        state.serialize_field(
            "verificationContractId",
            self.verification_contract_id.as_str(),
        )?;
        state.serialize_field("inputFingerprint", self.input_fingerprint.as_bytes())?;
        state.serialize_field("methodologyDigest", self.methodology_digest.as_bytes())?;
        state.serialize_field("mandatory", &self.mandatory)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for ToolsetRequirement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Wire {
            schema_version: SchemaVersion,
            verification_contract_id: VerificationContractId,
            input_fingerprint: [u8; 32],
            methodology_digest: [u8; 32],
            mandatory: bool,
        }
        let wire = Wire::deserialize(deserializer)?;
        Ok(Self {
            schema_version: wire.schema_version,
            verification_contract_id: wire.verification_contract_id,
            input_fingerprint: InputFingerprint::from_array(wire.input_fingerprint),
            methodology_digest: ArtifactDigest::from_array(wire.methodology_digest),
            mandatory: wire.mandatory,
        })
    }
}
