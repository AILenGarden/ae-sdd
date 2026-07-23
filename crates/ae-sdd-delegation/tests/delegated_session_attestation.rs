mod support;

use ae_sdd_delegation::DelegationStatus;

use support::{create_action, physical_proof, requested_delegation, session};

#[test]
fn delegation_runs_only_after_create_action_ack_and_child_claim() {
    let mut delegation = requested_delegation();
    let action = create_action();

    delegation
        .dispatch_create(&action)
        .expect("create action dispatched");
    assert_eq!(delegation.status(), DelegationStatus::Spawning);

    delegation
        .attest(physical_proof(&action))
        .expect("matching proof attests child");
    assert_eq!(delegation.status(), DelegationStatus::Running);

    assert_ne!(
        session(1),
        session(2),
        "parent and child are physical identities"
    );
}
