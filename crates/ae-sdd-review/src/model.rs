//! Reviewer identity and collected-review types consumed by the supervisor.

use std::collections::BTreeSet;

use ae_sdd_contracts::ReviewerRole;
use ae_sdd_contracts::review::{ReviewFinding, ReviewStatus};
use ae_sdd_domain::InputFingerprint;
use serde::{Deserialize, Serialize};

/// Serialiser bridge that mirrors `InputFingerprint` without requiring it to
/// derive `Serialize` itself (the domain digest type intentionally keeps a
/// narrow trait surface). The supervisor only serialises its own payloads; it
/// never persists the input fingerprint wire form.
#[allow(dead_code)]
mod input_fingerprint_bridge {
    use ae_sdd_domain::InputFingerprint;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(super) fn serialize<S>(value: &InputFingerprint, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.as_bytes().serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<InputFingerprint, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = <[u8; 32]>::deserialize(deserializer)?;
        Ok(InputFingerprint::from_array(bytes))
    }
}

/// Maximum delegation lineage depth permitted for a reviewer (mirrors
/// `ae_sdd_domain::delegation::MAX_DELEGATION_DEPTH`).
pub const MAX_REVIEWER_LINEAGE_DEPTH: u8 = 2;

/// Physical reviewer identity supplied by an attested host adapter.
///
/// All fields are caller-asserted; the supervisor validates them against the
/// author session, deduplication rules and lineage bounds. The C1 adapter is
/// responsible for binding these assertions to authenticated identities before
/// invoking the supervisor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewerIdentity {
    reviewer_role: ReviewerRole,
    physical_session_id: String,
    author_session_id: String,
    lineage_depth: u8,
    attested: bool,
}

impl ReviewerIdentity {
    /// Constructs a reviewer identity.
    pub fn new(
        reviewer_role: ReviewerRole,
        physical_session_id: impl Into<String>,
        author_session_id: impl Into<String>,
        lineage_depth: u8,
        attested: bool,
    ) -> Result<Self, ReviewerIdentityError> {
        let physical_session_id = physical_session_id.into();
        let author_session_id = author_session_id.into();
        if physical_session_id.is_empty() || physical_session_id.len() > 256 {
            return Err(ReviewerIdentityError::InvalidSessionId);
        }
        if author_session_id.is_empty() || author_session_id.len() > 256 {
            return Err(ReviewerIdentityError::InvalidSessionId);
        }
        Ok(Self {
            reviewer_role,
            physical_session_id,
            author_session_id,
            lineage_depth,
            attested,
        })
    }

    /// Returns the reviewer role.
    pub fn reviewer_role(&self) -> &ReviewerRole {
        &self.reviewer_role
    }

    /// Returns the physical session hosting the reviewer.
    pub fn physical_session_id(&self) -> &str {
        &self.physical_session_id
    }

    /// Returns the author session that produced the work under review.
    pub fn author_session_id(&self) -> &str {
        &self.author_session_id
    }

    /// Returns the delegation lineage depth.
    pub const fn lineage_depth(&self) -> u8 {
        self.lineage_depth
    }

    /// Returns whether the host adapter attested the identity.
    pub const fn attested(&self) -> bool {
        self.attested
    }
}

/// Structural error for a malformed reviewer identity.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReviewerIdentityError {
    /// A session id was empty or exceeded its bound.
    #[error("reviewer session id is empty or exceeds 256 bytes")]
    InvalidSessionId,
}
impl<'de> Deserialize<'de> for ReviewerIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Wire {
            reviewer_role: ReviewerRole,
            physical_session_id: String,
            author_session_id: String,
            lineage_depth: u8,
            attested: bool,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::new(
            wire.reviewer_role,
            wire.physical_session_id,
            wire.author_session_id,
            wire.lineage_depth,
            wire.attested,
        )
        .map_err(serde::de::Error::custom)
    }
}

/// Collected reviewer inputs that the supervisor evaluates against a session.
///
/// `asserted_status` is the terminal status the caller (C1 adapter, which owns
/// wall-clock and round/budget counters) asserts for this collection. The pure
/// supervisor validates identity independence and finding deduplication, then
/// delegates PASS/drift/required-completed enforcement to the frozen
/// `ReviewExitReceipt` contract.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CollectedReview {
    completed_roles: Vec<ReviewerRole>,
    reviewer_identities: Vec<ReviewerIdentity>,
    #[serde(with = "input_fingerprint_bridge")]
    observed_input_fingerprint: InputFingerprint,
    asserted_status: ReviewStatus,
    findings: Vec<ReviewFinding>,
}

impl CollectedReview {
    /// Constructs a collected review input.
    pub fn new(
        completed_roles: Vec<ReviewerRole>,
        reviewer_identities: Vec<ReviewerIdentity>,
        observed_input_fingerprint: InputFingerprint,
        asserted_status: ReviewStatus,
        findings: Vec<ReviewFinding>,
    ) -> Self {
        Self {
            completed_roles,
            reviewer_identities,
            observed_input_fingerprint,
            asserted_status,
            findings,
        }
    }

    /// Returns the roles that reportedly completed.
    pub fn completed_roles(&self) -> &[ReviewerRole] {
        &self.completed_roles
    }

    /// Returns the reviewer identities backing the completion claim.
    pub fn reviewer_identities(&self) -> &[ReviewerIdentity] {
        &self.reviewer_identities
    }

    /// Returns the observed input fingerprint.
    pub const fn observed_input_fingerprint(&self) -> InputFingerprint {
        self.observed_input_fingerprint
    }

    /// Returns the caller-asserted terminal status.
    pub const fn asserted_status(&self) -> ReviewStatus {
        self.asserted_status
    }

    /// Returns the raw findings collected from reviewers.
    pub fn findings(&self) -> &[ReviewFinding] {
        &self.findings
    }

    /// Returns the set of physical session ids backing the review.
    pub fn physical_sessions(&self) -> BTreeSet<&str> {
        self.reviewer_identities
            .iter()
            .map(ReviewerIdentity::physical_session_id)
            .collect()
    }
}
