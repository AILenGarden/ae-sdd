use ae_sdd_domain::{
    ConfigDigest, FencingToken, GateId, GateImplementationDigest, GateKey, InputFingerprint,
    InventoryGeneration, PolicyDigest, StateRevision, StoryId, ToolchainDigest, WorkItemId,
    WorkspaceId,
};
use uuid::Uuid;

pub fn gate_key(id: &str, revision: u64) -> GateKey {
    GateKey::new(
        GateId::new(id).expect("valid gate ID"),
        GateImplementationDigest::digest(b"implementation-v1"),
        PolicyDigest::digest(b"policy-v1"),
        WorkspaceId::from_uuid(Uuid::from_u128(1)),
        WorkItemId::new("PRD-AE-SDD-RUST-DAEMON-001").expect("valid work item"),
        Some(StoryId::new("STORY-AE-SDD-RUST-DAEMON-001").expect("valid story")),
        StateRevision::new(revision),
        FencingToken::new(8),
        InventoryGeneration::new(3),
        ToolchainDigest::digest(b"rustc-1.97.1"),
        ConfigDigest::digest(b"config-v1"),
        InputFingerprint::digest(b"inputs-v1"),
    )
}
