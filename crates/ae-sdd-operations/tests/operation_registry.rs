use std::collections::BTreeSet;

use ae_sdd_operations::{
    FieldKind, OPERATION_COUNT, OPERATION_REGISTRY, OperationName, operation_schema_digest,
    validate_operation_payload,
};
use serde_json::{Value, json};

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
    "review.contribute",
    "review.finalize",
    "review.record",
    "route.decide",
    "state.next_actions",
    "state.transition",
    "verification.plan",
    "workitem.complete",
    "workitem.create",
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
    assert!(
        !transition.requires_confirmation,
        "lifecycle policy owns phase-specific confirmation"
    );
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
    assert!(!start.requires_lease);
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
    assert!(!record.requires_lease);
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

#[test]
fn execution_plan_verification_describes_its_nested_item_schema() {
    let verification = OperationName::ExecutionPlanSet
        .spec()
        .fields
        .iter()
        .find(|field| field.name == "verification")
        .expect("execution.plan.set verification field");

    assert_eq!(verification.kind, FieldKind::Array);
    assert_eq!(verification.item_kind, Some(FieldKind::Object));
    assert_eq!(
        verification
            .items
            .expect("verification object item fields")
            .iter()
            .map(|field| (field.name, field.kind, field.required))
            .collect::<Vec<_>>(),
        vec![
            ("id", FieldKind::String, true),
            ("acId", FieldKind::String, true),
            ("boundary", FieldKind::String, true),
            ("command", FieldKind::StringOrArray, true),
            ("expected", FieldKind::String, true),
        ]
    );
}

#[test]
fn execution_plan_verification_rejects_missing_nested_required_fields() {
    let payload = |verification: Value| {
        json!({
            "goal": "Implement the approved slice",
            "changedPaths": ["src/lib.rs"],
            "verification": verification,
        })
    };
    let invalid = [
        (
            "missing required field",
            payload(json!([{
                "id": "V-1",
                "acId": "AC-1",
                "command": "cargo test",
                "expected": "tests pass",
            }])),
            "verification[0].boundary",
        ),
        (
            "wrong nested field type",
            payload(json!([{
                "id": "V-1",
                "acId": "AC-1",
                "boundary": "unit",
                "command": "cargo test",
                "expected": false,
            }])),
            "verification[0].expected",
        ),
        (
            "unknown nested field",
            payload(json!([{
                "id": "V-1",
                "acId": "AC-1",
                "boundary": "unit",
                "command": "cargo test",
                "expected": "tests pass",
                "extra": true,
            }])),
            "verification[0].extra",
        ),
        (
            "wrong array item type",
            payload(json!(["not an object"])),
            "verification[0]",
        ),
        (
            "empty command array",
            payload(json!([{
                "id": "V-1",
                "acId": "AC-1",
                "boundary": "unit",
                "command": [],
                "expected": "tests pass",
            }])),
            "verification[0].command",
        ),
        (
            "non-string command array",
            payload(json!([{
                "id": "V-1",
                "acId": "AC-1",
                "boundary": "unit",
                "command": [1],
                "expected": "tests pass",
            }])),
            "verification[0].command[0]",
        ),
        (
            "mixed command array",
            payload(json!([{
                "id": "V-1",
                "acId": "AC-1",
                "boundary": "unit",
                "command": ["cargo test", false],
                "expected": "tests pass",
            }])),
            "verification[0].command[1]",
        ),
        (
            "empty nested required string",
            payload(json!([{
                "id": "",
                "acId": "AC-1",
                "boundary": "unit",
                "command": "cargo test",
                "expected": "tests pass",
            }])),
            "verification[0].id",
        ),
    ];

    for (case, payload, expected_path) in invalid {
        let error = match validate_operation_payload(OperationName::ExecutionPlanSet, &payload) {
            Ok(()) => panic!("{case} must be rejected"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains(expected_path),
            "{case} must identify {expected_path:?}, got: {error}"
        );
    }
}

#[test]
fn document_save_describes_optional_keep_draft_boolean() {
    let keep_draft = OperationName::DocumentSave
        .spec()
        .fields
        .iter()
        .find(|field| field.name == "keepDraft")
        .expect("document.save keepDraft field");

    assert_eq!(keep_draft.kind, FieldKind::Boolean);
    assert!(!keep_draft.required);
}

#[test]
fn document_save_keeps_doc_id_statically_optional() {
    let doc_id = OperationName::DocumentSave
        .spec()
        .fields
        .iter()
        .find(|field| field.name == "docId")
        .expect("document.save docId field");

    assert_eq!(doc_id.kind, FieldKind::String);
    assert!(!doc_id.required);
}

#[test]
fn document_save_conditionally_requires_a_valid_story_doc_id() {
    for (case, doc_id) in [
        ("missing", None),
        ("null", Some(Value::Null)),
        ("empty", Some(json!(""))),
        ("non-Story", Some(json!("DR-ROUTE-001"))),
        ("missing suffix", Some(json!("STORY-"))),
        ("illegal identifier", Some(json!("STORY-ROUTE/001"))),
    ] {
        let mut payload = json!({"intent":"STORY","contentFile":"story.md"});
        if let Some(doc_id) = doc_id {
            payload["docId"] = doc_id;
        }

        let error = match validate_operation_payload(OperationName::DocumentSave, &payload) {
            Ok(()) => panic!("{case} Story docId must be rejected"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("docId"), "{case}: {error}");
    }

    validate_operation_payload(
        OperationName::DocumentSave,
        &json!({
            "intent":"STORY",
            "contentFile":"story.md",
            "docId":"STORY-ROUTE-001"
        }),
    )
    .expect("valid Story docId");

    for intent in ["RA", "DR", "TESTCASE", "CODING_PLAN"] {
        validate_operation_payload(
            OperationName::DocumentSave,
            &json!({"intent":intent,"contentFile":"document.md"}),
        )
        .unwrap_or_else(|error| panic!("{intent} docId remains optional: {error}"));
    }
}
