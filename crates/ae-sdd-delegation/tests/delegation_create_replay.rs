mod support;

use ae_sdd_delegation::{
    DelegationCreateReceipt, DelegationIdempotencyError, DelegationReplayDecision,
};
use ae_sdd_domain::{ResultDigest, WorkspaceId};
use uuid::Uuid;

use support::{delegation, session};

#[test]
fn matching_retry_replays_original_delegation_and_mutated_payload_conflicts() {
    let workspace = WorkspaceId::from_uuid(Uuid::from_u128(1));
    let request = ResultDigest::digest(b"canonical request");
    let response = ResultDigest::digest(b"canonical response");
    let receipt = DelegationCreateReceipt::new(
        workspace,
        session(1),
        "create-series-1",
        request,
        delegation(10),
        response,
    )
    .expect("valid receipt");

    assert_eq!(
        receipt.replay(workspace, session(1), "create-series-1", request),
        Ok(DelegationReplayDecision::Replay {
            delegation_id: delegation(10),
            response_digest: response,
        })
    );
    assert_eq!(
        receipt.replay(
            workspace,
            session(1),
            "create-series-1",
            ResultDigest::digest(b"mutated request"),
        ),
        Err(DelegationIdempotencyError::KeyReused)
    );
}
