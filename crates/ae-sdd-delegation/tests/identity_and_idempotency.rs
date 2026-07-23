mod support;

use ae_sdd_delegation::{DelegationCreateReceipt, DelegationReplayDecision};
use ae_sdd_domain::{ResultDigest, WorkspaceId};
use uuid::Uuid;

use support::{delegation, session};

#[test]
fn receipt_identity_is_scoped_by_workspace_and_parent_session() {
    let workspace = WorkspaceId::from_uuid(Uuid::from_u128(1));
    let digest = ResultDigest::digest(b"request");
    let receipt = DelegationCreateReceipt::new(
        workspace,
        session(1),
        "key",
        digest,
        delegation(1),
        ResultDigest::digest(b"response"),
    )
    .expect("valid receipt");

    assert_eq!(
        receipt
            .replay(
                WorkspaceId::from_uuid(Uuid::from_u128(2)),
                session(1),
                "key",
                digest,
            )
            .expect("different workspace is not a replay"),
        DelegationReplayDecision::NewRequest
    );
    assert_eq!(
        receipt
            .replay(workspace, session(2), "key", digest)
            .expect("different parent is not a replay"),
        DelegationReplayDecision::NewRequest
    );
}
