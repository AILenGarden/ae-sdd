//! RA-derived route evidence and the authoritative frozen engineering route.

use ae_sdd_domain::{
    ArtifactDigest, DecisionDigest, InputFingerprint, StateRevision, WorkItemId, WorkScale,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ConflictDimension, DocumentId, RequirementConflict, SchemaVersion, SeriesId,
    document::{DocumentVersionError, DocumentVersionId},
    serde_domain,
    series::RouteDecision,
};

/// Verification state of the collected RA Series receipt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptStatus {
    /// The Series result and cleanup receipts were collected.
    Collected,
    /// The collected result was also validated as the bound SRS.
    Verified,
}

/// Evidence that the RA Series closed over one exact SRS and scale decision.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequirementAnalysisEvidence {
    #[serde(with = "serde_domain::work_item_id")]
    work_item_id: WorkItemId,
    series_id: SeriesId,
    document_id: DocumentId,
    version: u32,
    #[serde(with = "serde_domain::artifact_digest")]
    ra_content_digest: ArtifactDigest,
    #[serde(with = "serde_domain::state_revision")]
    source_revision: StateRevision,
    #[serde(with = "serde_domain::artifact_digest")]
    ra_receipt_digest: ArtifactDigest,
    ra_receipt_status: ReceiptStatus,
    #[serde(with = "serde_domain::work_scale")]
    scale: WorkScale,
    #[serde(with = "serde_domain::artifact_digest")]
    scale_evidence_digest: ArtifactDigest,
    #[serde(with = "serde_domain::artifact_digest")]
    closure_receipt_set_digest: ArtifactDigest,
}

impl RequirementAnalysisEvidence {
    /// Builds a complete RA-to-route evidence binding.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        work_item_id: WorkItemId,
        series_id: SeriesId,
        document_id: DocumentId,
        version: u32,
        ra_content_digest: ArtifactDigest,
        source_revision: StateRevision,
        ra_receipt_digest: ArtifactDigest,
        ra_receipt_status: ReceiptStatus,
        scale: WorkScale,
        scale_evidence_digest: ArtifactDigest,
        closure_receipt_set_digest: ArtifactDigest,
    ) -> Self {
        Self {
            work_item_id,
            series_id,
            document_id,
            version,
            ra_content_digest,
            source_revision,
            ra_receipt_digest,
            ra_receipt_status,
            scale,
            scale_evidence_digest,
            closure_receipt_set_digest,
        }
    }

    /// Returns the Work Item bound to this RA result.
    pub const fn work_item_id(&self) -> &WorkItemId {
        &self.work_item_id
    }

    /// Returns the RA Series identity.
    pub const fn series_id(&self) -> &SeriesId {
        &self.series_id
    }

    /// Returns the bound SRS document identity.
    pub const fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    /// Returns the bound SRS version.
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Reconstructs the complete document-version identity.
    pub fn document_version(&self) -> Result<DocumentVersionId, DocumentVersionError> {
        DocumentVersionId::derive(
            self.document_id.clone(),
            self.ra_content_digest,
            self.version,
        )
    }

    /// Returns the exact SRS content digest.
    pub const fn ra_content_digest(&self) -> &ArtifactDigest {
        &self.ra_content_digest
    }

    /// Returns the state revision from which the evidence was projected.
    pub const fn source_revision(&self) -> StateRevision {
        self.source_revision
    }

    /// Returns the collected RA Series receipt digest.
    pub const fn ra_receipt_digest(&self) -> ArtifactDigest {
        self.ra_receipt_digest
    }

    /// Returns the validation status of the RA receipt.
    pub const fn ra_receipt_status(&self) -> ReceiptStatus {
        self.ra_receipt_status
    }

    /// Returns the SRS-derived scale.
    pub const fn scale(&self) -> WorkScale {
        self.scale
    }

    /// Returns the digest of the six-dimension scale evidence.
    pub const fn scale_evidence_digest(&self) -> ArtifactDigest {
        self.scale_evidence_digest
    }

    /// Returns the digest binding the G-RA-1 through G-RA-4 receipts.
    pub const fn closure_receipt_set_digest(&self) -> ArtifactDigest {
        self.closure_receipt_set_digest
    }
}

/// Versioned mapping from an RA scale to downstream engineering depth.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteMappingVersion {
    /// Initial SRS scale to route-depth mapping.
    V1,
}

/// Complete typed basis used to derive and verify a route candidate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteBindingInput {
    ra_evidence: RequirementAnalysisEvidence,
    mapping_version: RouteMappingVersion,
}

impl RouteBindingInput {
    /// Creates a complete route binding from verified RA evidence.
    pub const fn new(
        ra_evidence: RequirementAnalysisEvidence,
        mapping_version: RouteMappingVersion,
    ) -> Self {
        Self {
            ra_evidence,
            mapping_version,
        }
    }

    /// Returns the verified RA evidence.
    pub const fn ra_evidence(&self) -> &RequirementAnalysisEvidence {
        &self.ra_evidence
    }

    /// Returns the mapping revision used for classification.
    pub const fn mapping_version(&self) -> RouteMappingVersion {
        self.mapping_version
    }

    /// Recomputes the exact RA-derived route input fingerprint.
    pub fn fingerprint(&self) -> InputFingerprint {
        let evidence = &self.ra_evidence;
        let mut bytes = Vec::with_capacity(384);
        bytes.extend_from_slice(b"ae-sdd-route-binding/v1\0");
        encode_text(&mut bytes, evidence.work_item_id.as_str());
        encode_text(&mut bytes, evidence.series_id.as_str());
        encode_text(&mut bytes, evidence.document_id.as_str());
        bytes.extend_from_slice(&evidence.version.to_be_bytes());
        bytes.extend_from_slice(evidence.ra_content_digest.as_bytes());
        bytes.extend_from_slice(&evidence.source_revision.get().to_be_bytes());
        bytes.extend_from_slice(evidence.ra_receipt_digest.as_bytes());
        bytes.push(match evidence.ra_receipt_status {
            ReceiptStatus::Collected => 0,
            ReceiptStatus::Verified => 1,
        });
        bytes.push(scale_tag(evidence.scale));
        bytes.extend_from_slice(evidence.scale_evidence_digest.as_bytes());
        bytes.extend_from_slice(evidence.closure_receipt_set_digest.as_bytes());
        bytes.push(match self.mapping_version {
            RouteMappingVersion::V1 => 0,
        });
        InputFingerprint::digest(bytes)
    }
}

/// User approval bound to one exact SRS version and route candidate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteApprovalReceipt {
    confirmation_id: String,
    approved_by: String,
    approved_at: String,
    bound_document_id: DocumentId,
    bound_version: u32,
    #[serde(with = "serde_domain::artifact_digest")]
    bound_content_digest: ArtifactDigest,
    #[serde(with = "serde_domain::work_scale")]
    bound_scale: WorkScale,
    #[serde(with = "serde_domain::decision_digest")]
    bound_route_candidate_digest: DecisionDigest,
}

impl RouteApprovalReceipt {
    /// Creates a user approval bound to one document version and candidate.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        confirmation_id: String,
        approved_by: String,
        approved_at: String,
        bound_document_id: DocumentId,
        bound_version: u32,
        bound_content_digest: ArtifactDigest,
        bound_scale: WorkScale,
        bound_route_candidate_digest: DecisionDigest,
    ) -> Self {
        Self {
            confirmation_id,
            approved_by,
            approved_at,
            bound_document_id,
            bound_version,
            bound_content_digest,
            bound_scale,
            bound_route_candidate_digest,
        }
    }

    /// Returns whether this receipt binds the exact evidence and candidate.
    pub fn binds(
        &self,
        evidence: &RequirementAnalysisEvidence,
        route_candidate_digest: DecisionDigest,
    ) -> bool {
        !self.confirmation_id.trim().is_empty()
            && !self.approved_by.trim().is_empty()
            && !self.approved_at.trim().is_empty()
            && self.bound_document_id == evidence.document_id
            && self.bound_version == evidence.version
            && self.bound_content_digest == evidence.ra_content_digest
            && self.bound_scale == evidence.scale
            && self.bound_route_candidate_digest == route_candidate_digest
    }
}

/// Why a candidate could not be frozen as route authority.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EngineeringRouteError {
    /// A material RA conflict remains open.
    #[error("cannot freeze a route while a {} conflict is open", dimension.as_wire())]
    BlockingConflictOpen {
        /// Conflict dimension that prevents routing.
        dimension: ConflictDimension,
    },
    /// Candidate fingerprint does not match the typed RA binding.
    #[error("route decision fingerprint does not match the RA binding")]
    FingerprintMismatch,
    /// User approval does not bind the exact SRS and candidate.
    #[error("route approval receipt is not bound to the RA document and candidate")]
    ApprovalUnbound,
    /// Route freeze was attempted with an older schema.
    #[error("engineering route requires schema v2")]
    SchemaVersionMismatch,
}

/// Authoritative route, constructible only after the RA binding Gate passes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "EngineeringRouteWire", into = "EngineeringRouteWire")]
pub struct EngineeringRoute {
    schema_version: SchemaVersion,
    decision: RouteDecision,
    evidence: RequirementAnalysisEvidence,
    approval_receipt: RouteApprovalReceipt,
}

impl EngineeringRoute {
    /// Validates the candidate, approval, and conflicts before freezing authority.
    pub fn freeze(
        schema_version: SchemaVersion,
        binding_input: &RouteBindingInput,
        decision: RouteDecision,
        approval_receipt: &RouteApprovalReceipt,
        open_conflicts: &[RequirementConflict],
    ) -> Result<Self, EngineeringRouteError> {
        if schema_version != SchemaVersion::V2 {
            return Err(EngineeringRouteError::SchemaVersionMismatch);
        }
        if let Some(blocking) = open_conflicts
            .iter()
            .find(|conflict| conflict.blocks_routing())
        {
            return Err(EngineeringRouteError::BlockingConflictOpen {
                dimension: blocking.dimension(),
            });
        }
        let evidence = binding_input.ra_evidence();
        if decision.work_item_id() != evidence.work_item_id()
            || decision.scale() != evidence.scale()
            || decision.input_fingerprint() != binding_input.fingerprint()
        {
            return Err(EngineeringRouteError::FingerprintMismatch);
        }
        if !approval_receipt.binds(evidence, decision.decision_digest()) {
            return Err(EngineeringRouteError::ApprovalUnbound);
        }
        Ok(Self {
            schema_version,
            decision,
            evidence: evidence.clone(),
            approval_receipt: approval_receipt.clone(),
        })
    }

    /// Returns the frozen route schema version.
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the frozen route decision.
    pub const fn decision(&self) -> &RouteDecision {
        &self.decision
    }

    /// Returns the RA evidence that authorized this route.
    pub const fn evidence(&self) -> &RequirementAnalysisEvidence {
        &self.evidence
    }

    /// Returns the user approval bound at the freeze boundary.
    pub const fn approval_receipt(&self) -> &RouteApprovalReceipt {
        &self.approval_receipt
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EngineeringRouteWire {
    schema_version: SchemaVersion,
    decision: RouteDecision,
    evidence: RequirementAnalysisEvidence,
    approval_receipt: RouteApprovalReceipt,
}

impl From<EngineeringRoute> for EngineeringRouteWire {
    fn from(value: EngineeringRoute) -> Self {
        Self {
            schema_version: value.schema_version,
            decision: value.decision,
            evidence: value.evidence,
            approval_receipt: value.approval_receipt,
        }
    }
}

impl TryFrom<EngineeringRouteWire> for EngineeringRoute {
    type Error = EngineeringRouteError;

    fn try_from(value: EngineeringRouteWire) -> Result<Self, Self::Error> {
        let binding = RouteBindingInput::new(value.evidence, RouteMappingVersion::V1);
        Self::freeze(
            value.schema_version,
            &binding,
            value.decision,
            &value.approval_receipt,
            &[],
        )
    }
}

fn encode_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

const fn scale_tag(scale: WorkScale) -> u8 {
    match scale {
        WorkScale::Large => 0,
        WorkScale::Medium => 1,
        WorkScale::Small => 2,
        WorkScale::Micro => 3,
    }
}
