#![allow(dead_code)]

use ae_sdd_domain::{
    AgentRole, ArtifactDigest, ArtifactKind, ArtifactRef, CompactId, ContextGeneration,
    ContextProjectionId, ContextRevision, DelegationId, HostAckId, HostActionId,
    InventoryGeneration, PolicyDigest, ProjectRelativePath, SessionId, StateRevision,
};
use ae_sdd_host::{HostAck, HostAckOutcome, HostAction, HostActionKind, HostAdapterId};
use uuid::Uuid;

pub fn session(seed: u128) -> SessionId {
    SessionId::from_uuid(Uuid::from_u128(seed))
}

pub fn delegation(seed: u128) -> DelegationId {
    DelegationId::from_uuid(Uuid::from_u128(seed))
}

pub fn artifact(path: &str, content: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new("memory").expect("valid kind"),
        ProjectRelativePath::new(path).expect("valid path"),
        ArtifactDigest::digest(content),
        u64::try_from(content.len()).expect("fixture length"),
    )
}

pub fn compact_action(compact_id: CompactId, generation: ContextGeneration) -> HostAction {
    HostAction::new(
        HostActionId::from_uuid(Uuid::from_u128(20)),
        HostAdapterId::new("codex").expect("valid adapter"),
        1,
        HostActionKind::Compact,
        None,
        Some(compact_id),
        Some(session(1)),
        Some(generation),
        2_000,
        [1; 32],
    )
    .expect("valid compact action")
}

pub fn compact_ack(action: &HostAction) -> HostAck {
    HostAck::new(
        HostAckId::from_uuid(Uuid::from_u128(21)),
        action.action_id(),
        action.adapter_id().clone(),
        action.command_seq(),
        HostAckOutcome::Accepted,
        None,
        Some(session(1)),
    )
    .expect("valid ACK")
}

pub struct ProjectionIds {
    pub projection_id: ContextProjectionId,
    pub session_id: SessionId,
    pub delegation_id: Option<DelegationId>,
    pub role: AgentRole,
    pub source_revision: StateRevision,
    pub context_revision: ContextRevision,
    pub policy_digest: PolicyDigest,
    pub inventory_generation: InventoryGeneration,
}

pub fn root_projection_ids(revision: u64) -> ProjectionIds {
    ProjectionIds {
        projection_id: ContextProjectionId::from_uuid(Uuid::from_u128(50 + u128::from(revision))),
        session_id: session(1),
        delegation_id: None,
        role: AgentRole::Root,
        source_revision: StateRevision::new(7),
        context_revision: ContextRevision::new(revision),
        policy_digest: PolicyDigest::digest(b"policy"),
        inventory_generation: InventoryGeneration::new(3),
    }
}
