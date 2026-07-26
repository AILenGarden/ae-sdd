use std::collections::BTreeSet;

use ae_sdd_operations::{
    FieldKind, OPERATION_COUNT, OPERATION_REGISTRY, OperationName, operation_schema_digest,
};

const NAMES: [&str; OPERATION_COUNT] = [
    "document.resolve",
    "document.save",
    "evidence.finalize",
    "evidence.record",
    "execution.plan.approve",
    "execution.plan.set",
    "execution.resume",
    "execution.slice.record",
    "execution.slice.start",
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

#[test]
fn execution_supervisor_operations_have_frozen_preconditions_and_fields() {
    let resume = OperationName::ExecutionResume.spec();
    assert_eq!(resume.operation.as_str(), "execution.resume");
    assert!(resume.requires_workspace);
    assert!(resume.requires_work_item);
    assert!(!resume.writes);
    assert!(!resume.requires_lease);
    assert!(!resume.requires_revision);
    assert!(!resume.requires_idempotency);
    assert!(!resume.requires_confirmation);
    assert_eq!(
        resume
            .fields
            .iter()
            .map(|field| (field.name, field.kind, field.required))
            .collect::<Vec<_>>(),
        vec![
            ("knownCapsuleDigest", FieldKind::String, false),
            ("knownContextRevision", FieldKind::Integer, false),
        ]
    );

    let start = OperationName::ExecutionSliceStart.spec();
    assert_eq!(start.operation.as_str(), "execution.slice.start");
    assert!(start.requires_workspace);
    assert!(start.requires_work_item);
    assert!(start.writes);
    assert!(start.requires_lease);
    assert!(start.requires_revision);
    assert!(start.requires_idempotency);
    assert!(!start.requires_confirmation);
    assert_eq!(
        start
            .fields
            .iter()
            .map(|field| (field.name, field.kind, field.required))
            .collect::<Vec<_>>(),
        vec![
            ("activeOrdinal", FieldKind::Integer, true),
            ("queueDigest", FieldKind::String, true),
        ]
    );

    let record = OperationName::ExecutionSliceRecord.spec();
    assert_eq!(record.operation.as_str(), "execution.slice.record");
    assert!(record.requires_workspace);
    assert!(record.requires_work_item);
    assert!(record.writes);
    assert!(record.requires_lease);
    assert!(record.requires_revision);
    assert!(record.requires_idempotency);
    assert!(!record.requires_confirmation);
    assert_eq!(
        record
            .fields
            .iter()
            .map(|field| (field.name, field.kind, field.required))
            .collect::<Vec<_>>(),
        vec![
            ("sliceId", FieldKind::String, true),
            ("status", FieldKind::String, true),
            ("progressDigest", FieldKind::String, false),
        ]
    );
}

#[test]
fn verification_plan_wrapper_requires_durable_toolset_authority() {
    let verification = OperationName::VerificationPlan.spec();
    let fields = verification
        .fields
        .iter()
        .map(|field| (field.name, field.kind, field.required))
        .collect::<Vec<_>>();

    assert_eq!(
        fields,
        vec![
            ("toolsetJobId", FieldKind::String, true),
            ("plan", FieldKind::Object, true),
            ("receiptId", FieldKind::String, true),
            ("receiptDigest", FieldKind::String, true),
            ("sourceRevision", FieldKind::Integer, true),
            ("planDigest", FieldKind::String, true),
            ("methodologyDigest", FieldKind::String, true),
            ("policyDigest", FieldKind::String, true),
            ("inputFingerprint", FieldKind::String, true),
            ("changedPaths", FieldKind::Array, true),
            ("sinceFingerprint", FieldKind::String, false),
            ("persist", FieldKind::Boolean, true),
        ]
    );
}
