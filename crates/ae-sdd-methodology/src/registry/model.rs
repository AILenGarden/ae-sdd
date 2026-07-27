use std::fmt;

use ae_sdd_contracts::{OverrideDisposition, OverrideLayer, SkillId};
use ae_sdd_domain::{ArtifactDigest, DecisionDigest, ProjectRelativePath};
use thiserror::Error;

use crate::OverrideAuthorization;

/// A validated plugin or other layered-registry contender.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryCandidate {
    pub(super) name: SkillId,
    pub(super) target: ProjectRelativePath,
    pub(super) layer: OverrideLayer,
    pub(super) source_digest: ArtifactDigest,
    pub(super) content_digest: ArtifactDigest,
    pub(super) authorization: OverrideAuthorization,
}

impl RegistryCandidate {
    /// Constructs a candidate from adapter-validated identity and content metadata.
    pub fn new(
        name: SkillId,
        target: ProjectRelativePath,
        layer: OverrideLayer,
        source_digest: ArtifactDigest,
        content_digest: ArtifactDigest,
        authorization: OverrideAuthorization,
    ) -> Result<Self, RegistryCandidateError> {
        if layer == OverrideLayer::BuiltIn {
            return Err(RegistryCandidateError::BuiltInLayer);
        }
        Ok(Self {
            name,
            target,
            layer,
            source_digest,
            content_digest,
            authorization,
        })
    }

    /// Returns the candidate's registry-unique name.
    pub const fn name(&self) -> &SkillId {
        &self.name
    }

    /// Returns the logical override or provided target.
    pub const fn target(&self) -> &ProjectRelativePath {
        &self.target
    }

    /// Returns the L1/L2/L3 registry layer.
    pub const fn layer(&self) -> OverrideLayer {
        self.layer
    }

    /// Returns the immutable registry source snapshot digest.
    pub const fn source_digest(&self) -> ArtifactDigest {
        self.source_digest
    }

    /// Returns the candidate content digest bound to winner selection.
    pub const fn content_digest(&self) -> ArtifactDigest {
        self.content_digest
    }

    /// Returns the external authorization result.
    pub const fn authorization(&self) -> OverrideAuthorization {
        self.authorization
    }
}

/// Candidate construction error.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RegistryCandidateError {
    /// Built-in fallback is not a physical registry layer.
    #[error("built-in fallback cannot be registered as a registry candidate")]
    BuiltInLayer,
}

/// Stable reason attached to one canonical registry trace item.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RegistryTraceReason {
    /// This candidate won its target at the highest valid layer.
    Selected,
    /// A higher-priority candidate won the same target.
    HigherPrioritySelected,
    /// External policy denied this candidate.
    Unauthorized,
    /// The same layer declared the same candidate name more than once.
    SameLayerNameConflict,
    /// The same layer declared more than one candidate for one target.
    SameLayerTargetConflict,
    /// Another violation caused the complete registry decision to fail closed.
    ResolutionBlocked,
}

/// One complete, canonical registry contender trace item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryTrace {
    pub(super) layer: OverrideLayer,
    pub(super) name: SkillId,
    pub(super) target: ProjectRelativePath,
    pub(super) disposition: OverrideDisposition,
    pub(super) reason: RegistryTraceReason,
    pub(super) source_digest: ArtifactDigest,
    pub(super) content_digest: ArtifactDigest,
}

impl RegistryTrace {
    /// Returns the contender layer.
    pub const fn layer(&self) -> OverrideLayer {
        self.layer
    }

    /// Returns the contender name.
    pub const fn name(&self) -> &SkillId {
        &self.name
    }

    /// Returns the contender target.
    pub const fn target(&self) -> &ProjectRelativePath {
        &self.target
    }

    /// Returns whether the contender was selected, shadowed, or rejected.
    pub const fn disposition(&self) -> OverrideDisposition {
        self.disposition
    }

    /// Returns the stable trace reason.
    pub const fn reason(&self) -> RegistryTraceReason {
        self.reason
    }

    /// Returns the registry source snapshot digest.
    pub const fn source_digest(&self) -> ArtifactDigest {
        self.source_digest
    }

    /// Returns the content digest used by the decision.
    pub const fn content_digest(&self) -> ArtifactDigest {
        self.content_digest
    }
}

/// One fail-closed violation detected across the complete registry snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryViolation {
    /// The input exceeded its frozen candidate budget.
    CandidateLimit {
        /// Frozen maximum.
        limit: usize,
        /// Supplied count.
        actual: usize,
    },
    /// One candidate did not pass authorization.
    Unauthorized {
        /// Registry layer.
        layer: OverrideLayer,
        /// Candidate name.
        name: SkillId,
        /// Candidate target.
        target: ProjectRelativePath,
    },
    /// A name appeared more than once in the same layer.
    SameLayerNameConflict {
        /// Conflicting layer.
        layer: OverrideLayer,
        /// Duplicate name.
        name: SkillId,
    },
    /// A target had more than one contender in the same layer.
    SameLayerTargetConflict {
        /// Conflicting layer.
        layer: OverrideLayer,
        /// Duplicate target.
        target: ProjectRelativePath,
    },
}

/// One selected candidate for a logical registry target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryWinner {
    pub(super) candidate: RegistryCandidate,
}

impl RegistryWinner {
    /// Returns the content-bound selected candidate.
    pub const fn candidate(&self) -> &RegistryCandidate {
        &self.candidate
    }
}

/// Successful deterministic resolution of every target in one registry snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryResolution {
    pub(super) winners: Vec<RegistryWinner>,
    pub(super) trace: Vec<RegistryTrace>,
    pub(super) decision_digest: DecisionDigest,
}

impl RegistryResolution {
    /// Returns winners in canonical target order.
    pub fn winners(&self) -> &[RegistryWinner] {
        &self.winners
    }

    /// Looks up the selected candidate for one target.
    pub fn winner_for(&self, target: &ProjectRelativePath) -> Option<&RegistryWinner> {
        self.winners
            .iter()
            .find(|winner| winner.candidate.target == *target)
    }

    /// Returns the complete canonical contender trace.
    pub fn trace(&self) -> &[RegistryTrace] {
        &self.trace
    }

    /// Returns the content-bound canonical decision digest.
    pub const fn decision_digest(&self) -> DecisionDigest {
        self.decision_digest
    }
}

/// Fail-closed registry result retaining all bounded contenders and violations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryResolveError {
    pub(super) violations: Vec<RegistryViolation>,
    pub(super) trace: Vec<RegistryTrace>,
    pub(super) decision_digest: DecisionDigest,
}

impl RegistryResolveError {
    /// Returns every canonical violation detected in the snapshot.
    pub fn violations(&self) -> &[RegistryViolation] {
        &self.violations
    }

    /// Returns the complete canonical contender trace when within the input budget.
    pub fn trace(&self) -> &[RegistryTrace] {
        &self.trace
    }

    /// Returns the canonical failed-decision digest.
    pub const fn decision_digest(&self) -> DecisionDigest {
        self.decision_digest
    }
}

impl fmt::Display for RegistryResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "registry resolution failed closed with {} violation(s)",
            self.violations.len()
        )
    }
}

impl std::error::Error for RegistryResolveError {}
