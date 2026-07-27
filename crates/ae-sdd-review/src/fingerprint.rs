use ae_sdd_contracts::review::ReviewFinding;
use ae_sdd_domain::ArtifactDigest;
use sha2::Digest;

use crate::error::ReviewSupervisorError;

/// Frozen finding limit used by deduplication (mirrors the contracts constant).
const MAX_REVIEW_FINDINGS: usize = 128;

/// Stable digest of a finding used for deduplication and replay comparisons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct FindingDigest([u8; 32]);

impl FindingDigest {
    /// Returns the underlying 32-byte digest.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Returns the byte digest of one finding's deterministic fingerprint.
///
/// `ReviewFinding` already derives `Serialize` via `deny_unknown_fields`
/// camelCase wire, so its canonical JSON is the fingerprint source of truth.
fn fingerprint_bytes(finding: &ReviewFinding) -> [u8; 32] {
    let canonical = serde_json::to_vec(finding).unwrap_or_else(|_| Vec::new());
    let mut hasher = sha2::Sha256::new();
    hasher.update(canonical);
    hasher.finalize().into()
}

/// Deterministic textual fingerprint of one finding.
#[must_use]
pub fn finding_fingerprint(finding: &ReviewFinding) -> FindingDigest {
    FindingDigest(fingerprint_bytes(finding))
}

/// Returns the contract digest form used by Review Batch v2 receipts.
pub(crate) fn finding_artifact_digest(finding: &ReviewFinding) -> ArtifactDigest {
    ArtifactDigest::from_array(fingerprint_bytes(finding))
}

/// Removes duplicate findings while preserving the first-seen order.
///
/// Returns the deduplicated slice or an error when the deduplicated collection
/// still exceeds the frozen finding limit.
pub fn dedup_findings(
    findings: &[ReviewFinding],
) -> Result<Vec<ReviewFinding>, ReviewSupervisorError> {
    use std::collections::BTreeSet;
    let mut seen: BTreeSet<[u8; 32]> = BTreeSet::new();
    let mut deduped = Vec::with_capacity(findings.len());
    for finding in findings {
        let digest = finding_fingerprint(finding).as_bytes();
        if seen.insert(digest) {
            deduped.push(finding.clone());
        }
    }
    if deduped.len() > MAX_REVIEW_FINDINGS {
        return Err(ReviewSupervisorError::InvalidCollectedInput(
            "deduplicated findings exceed MAX_REVIEW_FINDINGS",
        ));
    }
    Ok(deduped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ae_sdd_contracts::review::{ReviewFinding, ReviewFindingSeverity};
    use ae_sdd_contracts::{BoundedText, ReasonCode};

    fn finding(code: &str, summary: &str) -> ReviewFinding {
        ReviewFinding::new(
            ReasonCode::new(code).unwrap(),
            ReviewFindingSeverity::Major,
            BoundedText::<1024>::new(summary).unwrap(),
        )
    }

    #[test]
    fn dedup_preserves_first_seen_order() {
        let a = finding("rc-1", "first");
        let b = finding("rc-2", "second");
        let dup_a = finding("rc-1", "first");
        let deduped = dedup_findings(&[a.clone(), b.clone(), dup_a]).unwrap();
        assert_eq!(deduped.len(), 2);
        assert_eq!(finding_fingerprint(&deduped[0]), finding_fingerprint(&a));
        assert_eq!(finding_fingerprint(&deduped[1]), finding_fingerprint(&b));
    }

    #[test]
    fn fingerprint_is_stable_under_reorder() {
        let a = finding("rc-a", "summary-a");
        let b = finding("rc-b", "summary-b");
        let fp_a = finding_fingerprint(&a);
        let fp_b = finding_fingerprint(&b);
        assert_ne!(fp_a, fp_b);
        assert_eq!(fp_a, finding_fingerprint(&finding("rc-a", "summary-a")));
    }
}
