use std::str::FromStr;

use ae_sdd_contracts::ports::{GateProof, GateProofProvider, GateProofRequest};
use ae_sdd_contracts::{ControlPlaneError, SchemaVersion};
use ae_sdd_domain::{
    ArtifactDigest, GateId, InputFingerprint, PolicyDigest, SessionId, StateRevision, WorkItemId,
    WorkspaceId,
};
use ae_sdd_protocol::GateOutcomeKind;

#[derive(Clone)]
struct StaticGateProof(GateProof);

impl GateProofProvider for StaticGateProof {
    fn evaluate(&self, _input: &GateProofRequest) -> Result<GateProof, ControlPlaneError> {
        Ok(self.0.clone())
    }
}

#[test]
fn gate_proof_port_is_low_level_typed_and_round_trips() {
    let workspace_id =
        WorkspaceId::from_str("00000000-0000-0000-0000-000000000001").expect("workspace id");
    let session_id =
        SessionId::from_str("00000000-0000-0000-0000-000000000002").expect("session id");
    let request = GateProofRequest::new(
        SchemaVersion::V1,
        workspace_id,
        WorkItemId::new("STORY-001").expect("work item id"),
        session_id,
        None,
        GateId::new("G-08").expect("gate id"),
        StateRevision::new(7),
        InputFingerprint::digest(b"gate input"),
        PolicyDigest::digest(b"policy"),
    );
    let proof = GateProof::new(
        SchemaVersion::V1,
        GateId::new("G-08").expect("gate id"),
        GateOutcomeKind::Pass,
        StateRevision::new(7),
        ArtifactDigest::digest(b"proof"),
        Vec::new(),
        1_785_000_900_000,
    )
    .expect("gate proof");
    let provider = StaticGateProof(proof.clone());

    assert_eq!(provider.evaluate(&request).expect("evaluate"), proof);
    let json = serde_json::to_string(&proof).expect("serialize");
    assert_eq!(
        serde_json::from_str::<GateProof>(&json).expect("deserialize"),
        proof
    );
}
