mod support;

use ae_sdd_delegation::{
    DelegationCreateReceipt, DelegationIdempotencyError, DelegationReplayDecision,
};
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

    assert_eq!(receipt.workspace_id(), workspace);
    assert_eq!(receipt.parent_session_id(), session(1));
    assert_eq!(receipt.idempotency_key(), "key");
    assert_eq!(receipt.delegation_id(), delegation(1));
    assert_eq!(receipt.response_digest(), ResultDigest::digest(b"response"));
    assert_eq!(
        receipt
            .replay(workspace, session(1), "another-key", digest)
            .expect("different key is a new request"),
        DelegationReplayDecision::NewRequest
    );
}

#[test]
fn receipt_rejects_empty_and_overlong_idempotency_keys() {
    let workspace = WorkspaceId::from_uuid(Uuid::from_u128(1));
    for key in [String::new(), "x".repeat(257)] {
        assert_eq!(
            DelegationCreateReceipt::new(
                workspace,
                session(1),
                key,
                ResultDigest::digest(b"request"),
                delegation(1),
                ResultDigest::digest(b"response"),
            ),
            Err(DelegationIdempotencyError::InvalidKey)
        );
    }
}
