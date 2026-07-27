//! Frozen Tier → required reviewer-role matrix and budget policy.

use std::collections::BTreeSet;

use ae_sdd_contracts::ReviewerRole;
use ae_sdd_contracts::review::ReviewTier;

/// Minimum reviewer count for a tier to be satisfiable.
pub const fn min_reviewers(tier: ReviewTier) -> usize {
    match tier {
        ReviewTier::Tier1 => 1,
        ReviewTier::Tier2 => 2,
        ReviewTier::Tier3 => 3,
    }
}

/// Returns the deterministic required-role set for the given tier.
///
/// Tiers compose a strict superset chain: Tier3 ⊇ Tier2 ⊇ Tier1.
/// Callers must pass the returned set as the authoritative required-role
/// collection when constructing a `ReviewSession`.
#[must_use]
pub fn required_roles_for(tier: ReviewTier) -> Vec<ReviewerRole> {
    let roles: &[&str] = match tier {
        ReviewTier::Tier1 => &["engineering"],
        ReviewTier::Tier2 => &["engineering", "security"],
        ReviewTier::Tier3 => &["engineering", "security", "architecture"],
    };
    roles
        .iter()
        .map(|raw| ReviewerRole::new(*raw).expect("frozen reviewer role"))
        .collect()
}

/// Returns true when the supplied required-role set is the exact frozen
/// projection of the tier, ignoring order and duplicates.
#[must_use]
pub fn matches_tier_matrix(tier: ReviewTier, required: &[ReviewerRole]) -> bool {
    let expected = required_roles_for(tier);
    let expected_set: BTreeSet<String> = expected
        .iter()
        .map(|role| role.as_str().to_owned())
        .collect();
    let actual: BTreeSet<String> = required
        .iter()
        .map(|role| role.as_str().to_owned())
        .collect();
    expected_set == actual
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_matrix_is_strictly_nesting() {
        let t1: BTreeSet<String> = required_roles_for(ReviewTier::Tier1)
            .iter()
            .map(|r| r.as_str().to_owned())
            .collect();
        let t2: BTreeSet<String> = required_roles_for(ReviewTier::Tier2)
            .iter()
            .map(|r| r.as_str().to_owned())
            .collect();
        let t3: BTreeSet<String> = required_roles_for(ReviewTier::Tier3)
            .iter()
            .map(|r| r.as_str().to_owned())
            .collect();
        assert!(t1.is_subset(&t2));
        assert!(t2.is_subset(&t3));
        assert_eq!(t1.len(), 1);
        assert_eq!(t2.len(), 2);
        assert_eq!(t3.len(), 3);
    }
}
