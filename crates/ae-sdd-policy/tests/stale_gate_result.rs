use std::str::FromStr;

use ae_sdd_domain::{
    ConfigDigest, FencingToken, GateId, GateImplementationDigest, GateKey, GateOutcome, GateResult,
    InputFingerprint, InventoryGeneration, PolicyDigest, StateRevision, ToolchainDigest,
    WorkItemId, WorkspaceId,
};
use ae_sdd_policy::{GateDirective, GateTruth};

fn key(revision: u64) -> GateKey {
    GateKey::new(
        GateId::new("G-14").expect("test Gate ID is valid"),
        GateImplementationDigest::digest(b"gate-v1"),
        PolicyDigest::digest(b"policy-v1"),
        WorkspaceId::from_str("00000000-0000-0000-0000-000000000111")
            .expect("test workspace ID is valid"),
        WorkItemId::new("PRD-AE-SDD-RUST-DAEMON-001").expect("test work item ID is valid"),
        None,
        StateRevision::new(revision),
        FencingToken::new(8),
        InventoryGeneration::new(3),
        ToolchainDigest::digest(b"rustc-1.97.1"),
        ConfigDigest::digest(b"config-v1"),
        InputFingerprint::digest(b"input-v1"),
    )
}

#[test]
fn formerly_passing_result_cannot_pass_after_freshness_change() {
    let snapshot = key(7);
    let result = GateResult::new(snapshot.clone(), GateOutcome::Pass);

    let fresh = GateTruth::judge_result(&result, &snapshot);
    assert!(fresh.transition_permitted());

    let stale = GateTruth::judge_result(&result, &key(8));
    assert!(!stale.transition_permitted());
    assert_eq!(stale.directive(), GateDirective::Reevaluate);
    assert_eq!(stale.correction_delta(), 0);
}
