//! Document version identity.
//!
//! `ae-sdd-daemon-design.md` §4.1 specifies `DocumentVersionId` as *derived*:
//! "由 `DocumentId` + `contentDigest` + `version` 确定". It is therefore frozen
//! here as a derivation, not as an opaque newtype. Freezing the opaque shape
//! would hide the very rule the design states, and would let two callers derive
//! different ids for the same version without anything detecting it.

use ae_sdd_domain::ArtifactDigest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::DocumentId;

/// Why a document version could not be derived.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum DocumentVersionError {
    /// Versions are 1-based; a zero version is not a content version.
    #[error("document version must start at 1")]
    ZeroVersion,
}

/// An immutable content version of one logical document.
///
/// Two instances are equal exactly when all three inputs match, so a path
/// change cannot alter the identity and a content change cannot reuse it.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DocumentVersionId {
    document_id: DocumentId,
    content_digest: ArtifactDigest,
    version: u32,
}

impl DocumentVersionId {
    /// Derives a version identity from its three frozen inputs.
    pub fn derive(
        document_id: DocumentId,
        content_digest: ArtifactDigest,
        version: u32,
    ) -> Result<Self, DocumentVersionError> {
        if version == 0 {
            return Err(DocumentVersionError::ZeroVersion);
        }
        Ok(Self {
            document_id,
            content_digest,
            version,
        })
    }

    /// Returns the stable logical document identity.
    pub const fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    /// Returns the content digest this version was derived from.
    pub const fn content_digest(&self) -> &ArtifactDigest {
        &self.content_digest
    }

    /// Returns the 1-based version ordinal.
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Returns the canonical wire encoding of the derivation.
    ///
    /// The encoding is `{documentId}@{version}#{contentDigest}` so a reader can
    /// recover all three inputs, and so two independently derived ids for the
    /// same version compare equal as text as well as structurally.
    pub fn to_wire(&self) -> String {
        format!(
            "{}@{}#{}",
            self.document_id,
            self.version,
            hex_digest(&self.content_digest)
        )
    }
}

fn hex_digest(digest: &ArtifactDigest) -> String {
    digest
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Which Spec document kind a route requires be bound.
///
/// §5.5 freezes `requiredSpecKinds` alongside `requiredSeries` because the two are
/// not the same fact: a Series is *work to run*, a Spec kind is *a document that
/// must exist and be bound*. §7.1 line 342 makes the distinction load-bearing — a
/// micro task runs Coding against an approved `executionPlan` and explicitly
/// 不要求独立 CodingPlan Markdown, so "runs coding" and "requires a CodingPlan
/// Spec" have to be separately expressible or micro collapses into small.
///
/// The five values are the 最低持久化设计产物 column of the §7.1 table: RA Spec,
/// DR Spec, Story Spec, TestCase Spec, CodingPlan Spec. Coding, Test and Review
/// appear in the route arrows but produce no Spec of their own, so they are absent
/// here by design rather than by omission.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecKind {
    /// The RA Spec every scale requires (§5.4: RA 是所有规模的必经 Series).
    RequirementAnalysis,
    /// The DR Spec only a large task requires (§7.1 line 345).
    DesignReview,
    /// A Story Spec, required from medium upward.
    Story,
    /// A TestCase Spec, required from medium upward.
    TestCase,
    /// A CodingPlan Spec, required from small upward but *not* by micro.
    CodingPlan,
}

impl SpecKind {
    /// Returns the frozen wire encoding.
    ///
    /// Written out rather than derived from the serde rename so a reader can see
    /// the frozen spelling without evaluating a macro, and so a rename cannot
    /// silently change the wire form on one side only.
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::RequirementAnalysis => "requirement_analysis",
            Self::DesignReview => "design_review",
            Self::Story => "story",
            Self::TestCase => "test_case",
            Self::CodingPlan => "coding_plan",
        }
    }

    /// Parses a frozen wire encoding, failing closed on anything else.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "requirement_analysis" => Some(Self::RequirementAnalysis),
            "design_review" => Some(Self::DesignReview),
            "story" => Some(Self::Story),
            "test_case" => Some(Self::TestCase),
            "coding_plan" => Some(Self::CodingPlan),
            _ => None,
        }
    }
}
