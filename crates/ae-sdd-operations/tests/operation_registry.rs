use std::collections::BTreeSet;

use ae_sdd_operations::{
    OPERATION_COUNT, OPERATION_REGISTRY, OperationName, operation_schema_digest,
};

const NAMES: [&str; OPERATION_COUNT] = [
    "document.resolve",
    "document.save",
    "evidence.finalize",
    "evidence.record",
    "execution.plan.approve",
    "execution.plan.set",
    "gate.check",
    "lease.acquire",
    "lease.break",
    "lease.release",
    "lease.renew",
    "lease.status",
    "review.record",
    "state.next_actions",
    "state.transition",
    "verification.plan",
    "workitem.complete",
    "workitem.get",
];

#[test]
fn registry_is_exact_unique_and_bootstrap_flags_are_explicit() {
    assert_eq!(OperationName::ALL.map(OperationName::as_str), NAMES);
    assert_eq!(OPERATION_REGISTRY.len(), OPERATION_COUNT);
    assert_eq!(
        NAMES.into_iter().collect::<BTreeSet<_>>().len(),
        OPERATION_COUNT
    );

    let acquire = OperationName::LeaseAcquire.spec();
    assert!(acquire.writes);
    assert!(!acquire.requires_lease);
    assert!(acquire.requires_idempotency);

    let lease_break = OperationName::LeaseBreak.spec();
    assert!(lease_break.writes);
    assert!(!lease_break.requires_lease);
    assert!(lease_break.requires_idempotency);

    let transition = OperationName::StateTransition.spec();
    assert!(transition.requires_lease);
    assert!(transition.requires_revision);
    assert!(transition.requires_idempotency);
    assert!(transition.requires_confirmation);
}

#[test]
fn registry_digest_is_deterministic_and_not_a_placeholder() {
    let first = operation_schema_digest();
    assert_eq!(first, operation_schema_digest());
    assert_eq!(first.len(), 64);
    assert!(
        first
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    assert_ne!(first, "0".repeat(64));
}
