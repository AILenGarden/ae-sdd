use ae_sdd_store::{StateAuthority, StoreError};
use proptest::prelude::*;

proptest! {
    #[test]
    fn same_revision_with_any_distinct_content_never_looks_unchanged(
        revision in 0_u64..1_000_000,
        fencing in 0_u64..1_000_000,
        left in any::<u64>(),
        right in any::<u64>(),
    ) {
        prop_assume!(left != right);
        let left_bytes = serde_json::to_vec(&serde_json::json!({
            "lastFencingToken": fencing,
            "nonce": left,
            "revision": revision,
        })).expect("state serializes");
        let right_bytes = serde_json::to_vec(&serde_json::json!({
            "lastFencingToken": fencing,
            "nonce": right,
            "revision": revision,
        })).expect("state serializes");
        let expected = StateAuthority::inspect(&left_bytes).expect("state is valid");
        let observed = StateAuthority::inspect(&right_bytes).expect("state is valid");

        let is_conflict = matches!(
            StateAuthority::verify_unchanged(expected, observed),
            Err(StoreError::ExternalStateConflict { .. })
        );
        prop_assert!(is_conflict);
    }
}
