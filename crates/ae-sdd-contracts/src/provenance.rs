//! Independent computation of the two proof dimensions.
//!
//! `ae-sdd-daemon-audit-report.md` F-10 records the live defect this freezes
//! against: `service_host.rs` assigns `decision_digest` to `input_fingerprint`
//! and then *requires them equal* on read. The two answer different questions —
//! a decision digest proves "which decision the daemon made", an input
//! fingerprint proves "what state, documents and rules that decision stood on".
//! Collapsing them destroys input-freshness: a Spec or state change becomes
//! undetectable because the fingerprint only moves when the decision moves.
//!
//! The audit fixes the canonical inputs, so this module computes the
//! fingerprint from all five rather than leaving each caller to invent a subset.

use ae_sdd_domain::{
    ArtifactDigest, InputFingerprint, InventoryGeneration, PolicyDigest, StateRevision,
};

use crate::DocumentVersionId;

/// The canonical inputs of an [`InputFingerprint`].
///
/// F-10: "`inputFingerprint` 由 state revision、DocumentVersion refs、context
/// bundle、policy digest 和 inventory generation 规范计算". All five are
/// required, so a caller cannot silently narrow the freshness domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FingerprintInputs {
    state_revision: StateRevision,
    document_versions: Vec<DocumentVersionId>,
    context_bundle_digest: ArtifactDigest,
    policy_digest: PolicyDigest,
    inventory_generation: InventoryGeneration,
}

impl FingerprintInputs {
    /// Collects the five canonical fingerprint inputs.
    pub fn new(
        state_revision: StateRevision,
        mut document_versions: Vec<DocumentVersionId>,
        context_bundle_digest: ArtifactDigest,
        policy_digest: PolicyDigest,
        inventory_generation: InventoryGeneration,
    ) -> Self {
        // Sorted so two callers observing the same versions in different order
        // still produce one fingerprint.
        document_versions.sort();
        document_versions.dedup();
        Self {
            state_revision,
            document_versions,
            context_bundle_digest,
            policy_digest,
            inventory_generation,
        }
    }

    /// Computes the fingerprint over all five inputs.
    ///
    /// Field lengths are encoded so no two distinct input sets can serialise to
    /// the same byte string by concatenation.
    pub fn fingerprint(&self) -> InputFingerprint {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"ae-sdd/input-fingerprint/v1");
        bytes.extend_from_slice(&self.state_revision.get().to_be_bytes());
        bytes.extend_from_slice(&(self.document_versions.len() as u64).to_be_bytes());
        for version in &self.document_versions {
            let wire = version.to_wire();
            bytes.extend_from_slice(&(wire.len() as u64).to_be_bytes());
            bytes.extend_from_slice(wire.as_bytes());
        }
        bytes.extend_from_slice(self.context_bundle_digest.as_bytes());
        bytes.extend_from_slice(self.policy_digest.as_bytes());
        bytes.extend_from_slice(&self.inventory_generation.get().to_be_bytes());
        InputFingerprint::digest(bytes)
    }

    /// Returns the state revision the fingerprint covers.
    pub const fn state_revision(&self) -> StateRevision {
        self.state_revision
    }

    /// Returns the document versions the fingerprint covers.
    pub fn document_versions(&self) -> &[DocumentVersionId] {
        &self.document_versions
    }
}
