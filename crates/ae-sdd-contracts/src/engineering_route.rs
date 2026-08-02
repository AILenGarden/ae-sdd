//! The authoritative engineering route, frozen only after RA closes.
//!
//! `ae-sdd-daemon-design.md` §2 and §5.4 split intake from routing: the Hook
//! records a *provisional* [`crate::BootstrapAssessment`], RA runs as the first
//! business Series for every scale, and only then is the route frozen. This
//! module makes that ordering unforgeable — an `EngineeringRoute` cannot be
//! constructed without evidence that RA actually closed, and cannot be frozen
//! while a route-blocking [`crate::RequirementConflict`] is open.
//!
//! Without this, "route-first" survives as a convention that any caller can
//! quietly break by building a route decision straight from the assessment.

use ae_sdd_domain::{ArtifactDigest, InputFingerprint, StateRevision};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ConflictDimension, DocumentId, RequirementConflict, SchemaVersion, SeriesId,
    document::{DocumentVersionError, DocumentVersionId},
    serde_domain,
    series::RouteDecision,
};

/// Evidence that the RA Series closed and produced a Spec.
///
/// §5.4 requires RA to bind a `DocumentId` and a content version, so a route
/// cannot cite "RA ran" without naming what RA produced.
/// Derived directly rather than via `try_from`, because [`Self::new`] is
/// infallible: there is no constructor guard for deserialization to skip.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequirementAnalysisEvidence {
    series_id: SeriesId,
    /// The RA document this evidence is about.
    ///
    /// The struct doc above already states §5.4 requires binding a `DocumentId`;
    /// without the field, closure evidence proved only "some RA content had this
    /// digest", not *which logical document* it belongs to. Two Work Items whose RA
    /// happened to share content would produce interchangeable evidence.
    document_id: DocumentId,
    /// 1-based content version, completing the §4.1 line 132 triple together with
    /// `document_id` and the existing `ra_content_digest`.
    ///
    /// A raw `u32` for the same reason as `RequirementSourceRef::Prd`:
    /// [`DocumentVersionId`] carries no serde derive. [`Self::document_version`]
    /// assembles it.
    version: u32,
    #[serde(with = "serde_domain::artifact_digest")]
    ra_content_digest: ArtifactDigest,
    #[serde(with = "serde_domain::state_revision")]
    source_revision: StateRevision,
}

impl RequirementAnalysisEvidence {
    /// Builds RA closure evidence.
    pub const fn new(
        series_id: SeriesId,
        document_id: DocumentId,
        version: u32,
        ra_content_digest: ArtifactDigest,
        source_revision: StateRevision,
    ) -> Self {
        Self {
            series_id,
            document_id,
            version,
            ra_content_digest,
            source_revision,
        }
    }

    /// Returns the RA Series that produced this evidence.
    pub const fn series_id(&self) -> &SeriesId {
        &self.series_id
    }

    /// Returns the logical RA document this evidence binds.
    pub const fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    /// The exact RA document version this closure evidence stands on.
    ///
    /// This is what the two added fields are for: a route citing "RA ran" must name
    /// *which version* it read, or a later RA revision leaves the citation pointing
    /// at content that no longer exists — the §5.4 line 258 case where an existing
    /// RA covers only part of the new requirements and must produce a new version
    /// rather than being silently reused.
    pub fn document_version(&self) -> Result<DocumentVersionId, DocumentVersionError> {
        DocumentVersionId::derive(
            self.document_id.clone(),
            self.ra_content_digest,
            self.version,
        )
    }

    /// Returns the digest of the RA content the route was frozen against.
    pub const fn ra_content_digest(&self) -> &ArtifactDigest {
        &self.ra_content_digest
    }

    /// Returns the state revision RA closed at.
    pub const fn source_revision(&self) -> StateRevision {
        self.source_revision
    }
}

/// Why an engineering route could not be frozen.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EngineeringRouteError {
    /// §6.2 rule 4: a material conflict must send the flow to `awaiting_user`
    /// instead of freezing a route.
    #[error("cannot freeze a route while a {} conflict is open", dimension.as_wire())]
    BlockingConflictOpen {
        /// The dimension that blocked the freeze.
        dimension: ConflictDimension,
    },
    /// The decision was not fingerprinted against the RA revision it cites.
    #[error("route decision fingerprint is not bound to the cited RA evidence")]
    EvidenceNotBound,
}

/// The authoritative route for a Work Item, frozen after RA.
///
/// This is the counterpart to [`crate::BootstrapAssessment`]: the assessment is
/// a proposal that stays `provisional`, and this type is authority. Holding an
/// instance is itself proof that RA closed with no route-blocking conflict.
/// Deserialization re-checks `EvidenceNotBound` but *cannot* re-check
/// `BlockingConflictOpen`, and that asymmetry is deliberate rather than an
/// oversight.
///
/// [`Self::freeze`] takes `open_conflicts` as an external parameter and never
/// stores it, so a decoded value carries no record of the conflict set that
/// authorised it. `EvidenceNotBound` is different: it reads
/// `decision.input_fingerprint()`, a stored field, so the wire can enforce it.
///
/// The consequence a reader needs: decoding an `EngineeringRoute` proves RA
/// evidence is bound, but it does **not** re-prove that no route-blocking
/// conflict was open. That check lives at the freeze boundary, and a caller
/// reconstructing a route from bytes must not treat decode success as
/// re-authorisation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "EngineeringRouteWire", into = "EngineeringRouteWire")]
pub struct EngineeringRoute {
    schema_version: SchemaVersion,
    decision: RouteDecision,
    evidence: RequirementAnalysisEvidence,
}

impl EngineeringRoute {
    /// Freezes a route, rejecting any state that §6.2 rule 4 forbids routing in.
    ///
    /// `open_conflicts` is the full conflict set RA recorded. A single
    /// route-blocking dimension is enough to refuse the freeze; non-material
    /// conflicts are recorded elsewhere and do not block.
    pub fn freeze(
        schema_version: SchemaVersion,
        decision: RouteDecision,
        evidence: RequirementAnalysisEvidence,
        open_conflicts: &[RequirementConflict],
    ) -> Result<Self, EngineeringRouteError> {
        if let Some(blocking) = open_conflicts
            .iter()
            .find(|conflict| conflict.blocks_routing())
        {
            return Err(EngineeringRouteError::BlockingConflictOpen {
                dimension: blocking.dimension(),
            });
        }
        if decision.input_fingerprint() == InputFingerprint::digest(b"") {
            return Err(EngineeringRouteError::EvidenceNotBound);
        }
        Ok(Self {
            schema_version,
            decision,
            evidence,
        })
    }

    /// Returns the contract schema version.
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the frozen route decision.
    pub const fn decision(&self) -> &RouteDecision {
        &self.decision
    }

    /// Returns the RA evidence that authorised this freeze.
    pub const fn evidence(&self) -> &RequirementAnalysisEvidence {
        &self.evidence
    }
}

/// Wire form of [`EngineeringRoute`].
///
/// Deserialization passes an empty conflict slice to [`EngineeringRoute::freeze`]
/// because the conflict set is not part of the encoding. That is safe only
/// because it cannot *weaken* the guard: an empty slice means "no blocking
/// conflict found here", and the real check already ran at the original freeze.
/// It would be unsafe to invent a conflict set, which is why none is fabricated.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EngineeringRouteWire {
    schema_version: SchemaVersion,
    decision: RouteDecision,
    evidence: RequirementAnalysisEvidence,
}

impl From<EngineeringRoute> for EngineeringRouteWire {
    fn from(value: EngineeringRoute) -> Self {
        Self {
            schema_version: value.schema_version,
            decision: value.decision,
            evidence: value.evidence,
        }
    }
}

impl TryFrom<EngineeringRouteWire> for EngineeringRoute {
    type Error = EngineeringRouteError;

    fn try_from(value: EngineeringRouteWire) -> Result<Self, Self::Error> {
        Self::freeze(value.schema_version, value.decision, value.evidence, &[])
    }
}
