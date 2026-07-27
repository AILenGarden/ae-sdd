use crate::ArtifactDigest;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProcessPhase {
    Initialized,
    RouteSelected,
    RequirementAnalyzed,
    DrGenerated,
    StoryGenerated,
    TestcaseGenerated,
    CodingProcess,
    Coding,
    TestRunning,
    CodeReviewed,
    Completed,
    Paused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorkScale {
    Large,
    Medium,
    Small,
    Micro,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DesignRoute {
    Dr,
    Story,
    CodingPlan,
}

/// Orthogonal completion dimension tracked next to `ProcessPhase`.
///
/// The milestone records how far the terminal completion chain progressed:
/// fresh focused and workspace verification advances to
/// `ImplementationVerified`, a finalized evidence ledger to `ReviewReady`,
/// and a passing Review with fresh final Gates to `GovernanceClosed`. Only
/// `GovernanceClosed` may commit the terminal `Completed` phase. The declared
/// variant order is the chain order, so earlier milestones compare smaller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompletionMilestone {
    /// No completion evidence has been verified yet.
    None,
    /// Focused and workspace verification are fresh for the bound code state.
    ImplementationVerified,
    /// The execution evidence ledger was finalized for the verified state.
    ReviewReady,
    /// Review passed and the final Gates are fresh for the bound digests.
    GovernanceClosed,
}

impl CompletionMilestone {
    /// Rolls the milestone back to the earliest still-fresh point.
    ///
    /// Any code or verification digest change invalidates
    /// `ImplementationVerified`, any evidence digest change invalidates
    /// `ReviewReady`, and any Review input or final Gate digest change
    /// invalidates `GovernanceClosed`. When several inputs change at once the
    /// earliest affected point wins, so a changed input never keeps
    /// `GovernanceClosed`.
    pub fn invalidate(self, bound: &CompletionDigestSet, observed: &CompletionDigestSet) -> Self {
        let mut rolled = self;
        if rolled >= Self::GovernanceClosed
            && (bound.review_input_digest() != observed.review_input_digest()
                || bound.gate_digest() != observed.gate_digest())
        {
            rolled = Self::ReviewReady;
        }
        if rolled >= Self::ReviewReady && bound.evidence_digest() != observed.evidence_digest() {
            rolled = Self::ImplementationVerified;
        }
        if rolled >= Self::ImplementationVerified
            && (bound.code_digest() != observed.code_digest()
                || bound.verification_digest() != observed.verification_digest())
        {
            rolled = Self::None;
        }
        rolled
    }
}

/// Input digests bound when a completion milestone was reached.
///
/// Each slot records the digest observed when the corresponding milestone
/// stage advanced; comparing the bound set against a freshly observed set
/// decides which milestone stage is still fresh. `ZERO` marks the unbound
/// state, so any real observation differs from it and fails closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CompletionDigestSet {
    code_digest: ArtifactDigest,
    verification_digest: ArtifactDigest,
    evidence_digest: ArtifactDigest,
    review_input_digest: ArtifactDigest,
    gate_digest: ArtifactDigest,
}

impl CompletionDigestSet {
    /// The unbound digest set; every slot is the zero digest.
    pub const ZERO: Self = Self {
        code_digest: ArtifactDigest::from_array([0; 32]),
        verification_digest: ArtifactDigest::from_array([0; 32]),
        evidence_digest: ArtifactDigest::from_array([0; 32]),
        review_input_digest: ArtifactDigest::from_array([0; 32]),
        gate_digest: ArtifactDigest::from_array([0; 32]),
    };

    /// Creates a fully bound digest set.
    pub const fn new(
        code_digest: ArtifactDigest,
        verification_digest: ArtifactDigest,
        evidence_digest: ArtifactDigest,
        review_input_digest: ArtifactDigest,
        gate_digest: ArtifactDigest,
    ) -> Self {
        Self {
            code_digest,
            verification_digest,
            evidence_digest,
            review_input_digest,
            gate_digest,
        }
    }

    /// Returns the digest of the verified code state.
    pub const fn code_digest(&self) -> ArtifactDigest {
        self.code_digest
    }

    /// Returns the digest of the focused and workspace verification.
    pub const fn verification_digest(&self) -> ArtifactDigest {
        self.verification_digest
    }

    /// Returns the digest of the finalized evidence authority.
    pub const fn evidence_digest(&self) -> ArtifactDigest {
        self.evidence_digest
    }

    /// Returns the digest of the Review input batch.
    pub const fn review_input_digest(&self) -> ArtifactDigest {
        self.review_input_digest
    }

    /// Returns the digest of the final Gate evaluation inputs.
    pub const fn gate_digest(&self) -> ArtifactDigest {
        self.gate_digest
    }

    /// Rebinds the verified code state digest.
    pub const fn with_code_digest(mut self, digest: ArtifactDigest) -> Self {
        self.code_digest = digest;
        self
    }

    /// Rebinds the verification digest.
    pub const fn with_verification_digest(mut self, digest: ArtifactDigest) -> Self {
        self.verification_digest = digest;
        self
    }

    /// Rebinds the finalized evidence digest.
    pub const fn with_evidence_digest(mut self, digest: ArtifactDigest) -> Self {
        self.evidence_digest = digest;
        self
    }

    /// Rebinds the Review input digest.
    pub const fn with_review_input_digest(mut self, digest: ArtifactDigest) -> Self {
        self.review_input_digest = digest;
        self
    }

    /// Rebinds the final Gate input digest.
    pub const fn with_gate_digest(mut self, digest: ArtifactDigest) -> Self {
        self.gate_digest = digest;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_phase_is_vocabulary_not_a_transition_table() {
        let phases = [
            ProcessPhase::Initialized,
            ProcessPhase::RouteSelected,
            ProcessPhase::RequirementAnalyzed,
            ProcessPhase::DrGenerated,
            ProcessPhase::StoryGenerated,
            ProcessPhase::TestcaseGenerated,
            ProcessPhase::CodingProcess,
            ProcessPhase::Coding,
            ProcessPhase::TestRunning,
            ProcessPhase::CodeReviewed,
            ProcessPhase::Completed,
            ProcessPhase::Paused,
        ];

        assert_eq!(phases.len(), 12);
    }

    fn digest(label: u8) -> ArtifactDigest {
        ArtifactDigest::from_array([label; 32])
    }

    fn bound() -> CompletionDigestSet {
        CompletionDigestSet::new(digest(1), digest(2), digest(3), digest(4), digest(5))
    }

    #[test]
    fn milestone_order_follows_the_completion_chain() {
        assert!(CompletionMilestone::None < CompletionMilestone::ImplementationVerified);
        assert!(CompletionMilestone::ImplementationVerified < CompletionMilestone::ReviewReady);
        assert!(CompletionMilestone::ReviewReady < CompletionMilestone::GovernanceClosed);
    }

    #[test]
    fn unchanged_digests_never_roll_back() {
        for milestone in [
            CompletionMilestone::None,
            CompletionMilestone::ImplementationVerified,
            CompletionMilestone::ReviewReady,
            CompletionMilestone::GovernanceClosed,
        ] {
            assert_eq!(milestone.invalidate(&bound(), &bound()), milestone);
        }
    }

    #[test]
    fn each_input_change_rolls_back_to_the_earliest_affected_point() {
        let bound = bound();
        let cases = [
            (
                bound.with_code_digest(digest(11)),
                CompletionMilestone::None,
            ),
            (
                bound.with_verification_digest(digest(12)),
                CompletionMilestone::None,
            ),
            (
                bound.with_evidence_digest(digest(13)),
                CompletionMilestone::ImplementationVerified,
            ),
            (
                bound.with_review_input_digest(digest(14)),
                CompletionMilestone::ReviewReady,
            ),
            (
                bound.with_gate_digest(digest(15)),
                CompletionMilestone::ReviewReady,
            ),
            (
                bound
                    .with_evidence_digest(digest(13))
                    .with_review_input_digest(digest(14)),
                CompletionMilestone::ImplementationVerified,
            ),
            (
                bound
                    .with_verification_digest(digest(12))
                    .with_gate_digest(digest(15)),
                CompletionMilestone::None,
            ),
        ];

        for (observed, expected) in cases {
            let rolled = CompletionMilestone::GovernanceClosed.invalidate(&bound, &observed);
            assert_eq!(rolled, expected);
            assert_ne!(
                rolled,
                CompletionMilestone::GovernanceClosed,
                "a changed input must never keep GovernanceClosed",
            );
        }
    }

    #[test]
    fn invalidation_only_checks_dimensions_the_milestone_depends_on() {
        let bound = bound();
        let drifted = CompletionDigestSet::ZERO
            .with_evidence_digest(digest(3))
            .with_review_input_digest(digest(4))
            .with_gate_digest(digest(5));

        assert_eq!(
            CompletionMilestone::ImplementationVerified.invalidate(
                &CompletionDigestSet::ZERO
                    .with_code_digest(digest(1))
                    .with_verification_digest(digest(2)),
                &drifted
                    .with_code_digest(digest(1))
                    .with_verification_digest(digest(2)),
            ),
            CompletionMilestone::ImplementationVerified,
            "evidence, review, and Gate drift must not touch ImplementationVerified",
        );
        assert_eq!(
            CompletionMilestone::None.invalidate(&CompletionDigestSet::ZERO, &bound),
            CompletionMilestone::None,
            "a milestone cannot advance through invalidation",
        );
    }
}
