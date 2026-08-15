use ae_sdd_operations::{
    FieldKind, OperationName, OperationRequestError, validate_operation_payload,
};
use serde_json::{Value, json};

fn create_payload(provided: Value) -> Value {
    json!({"entryNode":"PRD","providedDocuments":provided})
}

fn prd(doc_id: &str, path: &str) -> Value {
    json!({"intent":"PRD","docId":doc_id,"path":path})
}

fn dr(doc_id: &str, path: &str, parent: Option<&str>) -> Value {
    match parent {
        Some(parent) => json!({"intent":"DR","docId":doc_id,"path":path,"parentDocId":parent}),
        None => json!({"intent":"DR","docId":doc_id,"path":path}),
    }
}

fn story(doc_id: &str, path: &str, parent: Option<&str>) -> Value {
    match parent {
        Some(parent) => {
            json!({"intent":"STORY","docId":doc_id,"path":path,"parentDocId":parent})
        }
        None => json!({"intent":"STORY","docId":doc_id,"path":path}),
    }
}

#[test]
fn workitem_create_exposes_provided_documents_as_an_optional_array() {
    let fields = OperationName::WorkItemCreate
        .spec()
        .fields
        .iter()
        .map(|field| (field.name, field.kind, field.required))
        .collect::<Vec<_>>();
    assert_eq!(
        fields,
        vec![
            ("entryNode", FieldKind::String, true),
            ("requestedIntent", FieldKind::String, false),
            ("storyName", FieldKind::String, false),
            ("providedDocuments", FieldKind::Array, false),
        ]
    );
}

#[test]
fn a_complete_provided_documents_tree_is_accepted() {
    let payload = create_payload(json!([
        prd("PRD-001", "docs/PRD-001.md"),
        dr("DR-001", "docs/DR-001.md", Some("PRD-001")),
        dr("DR-002", "docs/DR-002.md", Some("PRD-001")),
        story("STORY-001", "docs/STORY-001.md", Some("DR-001")),
        story("STORY-002", "docs/STORY-002.md", None),
    ]));
    validate_operation_payload(OperationName::WorkItemCreate, &payload)
        .expect("a well-formed providedDocuments tree validates");
}

#[test]
fn provided_documents_is_optional() {
    validate_operation_payload(OperationName::WorkItemCreate, &json!({"entryNode":"DR"}))
        .expect("workitem.create keeps working without providedDocuments");
}

#[test]
fn a_route_intake_accepts_a_requested_design_intent() {
    validate_operation_payload(
        OperationName::WorkItemCreate,
        &json!({"entryNode":"ROUTE","requestedIntent":"DR"}),
    )
    .expect("a direct DR request remains a hint on the unified ROUTE intake");
}

#[test]
fn a_route_intake_rejects_an_unknown_requested_design_intent() {
    assert!(matches!(
        validate_operation_payload(
            OperationName::WorkItemCreate,
            &json!({"entryNode":"ROUTE","requestedIntent":"PRD"}),
        ),
        Err(OperationRequestError::InvalidRequestedIntent(_))
    ));
}

#[test]
fn a_requested_design_intent_requires_the_unified_route_entry() {
    assert!(matches!(
        validate_operation_payload(
            OperationName::WorkItemCreate,
            &json!({"entryNode":"DR","requestedIntent":"DR"}),
        ),
        Err(OperationRequestError::InvalidRequestedIntent(_))
    ));
}

#[test]
fn entries_must_be_objects_with_known_keys() {
    for provided in [
        json!(["PRD-001"]),
        json!([{"intent":"PRD","docId":"PRD-001","path":"docs/PRD-001.md","note":"x"}]),
        json!([{"intent":"PRD","docId":"PRD-001"}]),
        json!([{"intent":"PRD","path":"docs/PRD-001.md"}]),
        json!([{"docId":"PRD-001","path":"docs/PRD-001.md"}]),
    ] {
        let label = provided.to_string();
        assert!(
            matches!(
                validate_operation_payload(
                    OperationName::WorkItemCreate,
                    &create_payload(provided)
                ),
                Err(OperationRequestError::InvalidProvidedDocuments(_))
            ),
            "entry must be rejected: {label}"
        );
    }
}

#[test]
fn intent_must_be_a_document_series() {
    let payload = create_payload(json!([
        {"intent":"CODING_PLAN","docId":"CP-001","path":"docs/CP-001.md"}
    ]));
    assert!(matches!(
        validate_operation_payload(OperationName::WorkItemCreate, &payload),
        Err(OperationRequestError::InvalidProvidedDocuments(_))
    ));
}

#[test]
fn doc_id_must_be_non_empty_and_unique() {
    for provided in [
        json!([{"intent":"PRD","docId":"","path":"docs/PRD-001.md"}]),
        json!([
            prd("PRD-001", "docs/PRD-001.md"),
            dr("PRD-001", "docs/DR-001.md", Some("PRD-001"))
        ]),
    ] {
        let label = provided.to_string();
        assert!(
            matches!(
                validate_operation_payload(
                    OperationName::WorkItemCreate,
                    &create_payload(provided)
                ),
                Err(OperationRequestError::InvalidProvidedDocuments(_))
            ),
            "docId rule must be enforced: {label}"
        );
    }
}

#[test]
fn path_must_be_non_empty_text() {
    for provided in [
        json!([{"intent":"PRD","docId":"PRD-001","path":""}]),
        json!([{"intent":"PRD","docId":"PRD-001","path":7}]),
    ] {
        let label = provided.to_string();
        assert!(
            matches!(
                validate_operation_payload(
                    OperationName::WorkItemCreate,
                    &create_payload(provided)
                ),
                Err(OperationRequestError::InvalidProvidedDocuments(_))
                    | Err(OperationRequestError::PayloadFieldType(_))
            ),
            "path rule must be enforced: {label}"
        );
    }
}

#[test]
fn parent_doc_id_must_reference_a_provided_document_of_the_parent_series() {
    let dangling = create_payload(json!([story(
        "STORY-001",
        "docs/STORY-001.md",
        Some("DR-404")
    )]));
    assert!(matches!(
        validate_operation_payload(OperationName::WorkItemCreate, &dangling),
        Err(OperationRequestError::InvalidProvidedDocuments(_))
    ));

    let story_under_prd = create_payload(json!([
        prd("PRD-001", "docs/PRD-001.md"),
        story("STORY-001", "docs/STORY-001.md", Some("PRD-001")),
    ]));
    assert!(matches!(
        validate_operation_payload(OperationName::WorkItemCreate, &story_under_prd),
        Err(OperationRequestError::InvalidProvidedDocuments(_))
    ));

    let dr_under_dr = create_payload(json!([
        dr("DR-001", "docs/DR-001.md", None),
        dr("DR-002", "docs/DR-002.md", Some("DR-001")),
    ]));
    assert!(matches!(
        validate_operation_payload(OperationName::WorkItemCreate, &dr_under_dr),
        Err(OperationRequestError::InvalidProvidedDocuments(_))
    ));

    let prd_with_parent = create_payload(json!([
        prd("PRD-001", "docs/PRD-001.md"),
        {"intent":"PRD","docId":"PRD-002","path":"docs/PRD-002.md","parentDocId":"PRD-001"},
    ]));
    assert!(matches!(
        validate_operation_payload(OperationName::WorkItemCreate, &prd_with_parent),
        Err(OperationRequestError::InvalidProvidedDocuments(_))
    ));
}

#[test]
fn the_tree_is_bounded_to_sixty_four_entries() {
    let mut entries = (0..64)
        .map(|index| {
            json!({"intent":"DR","docId":format!("DR-{index:03}"),"path":format!("docs/DR-{index:03}.md")})
        })
        .collect::<Vec<_>>();
    validate_operation_payload(
        OperationName::WorkItemCreate,
        &create_payload(Value::Array(entries.clone())),
    )
    .expect("64 entries stay within the bound");
    entries.push(json!({"intent":"DR","docId":"DR-064","path":"docs/DR-064.md"}));
    assert!(matches!(
        validate_operation_payload(
            OperationName::WorkItemCreate,
            &create_payload(Value::Array(entries))
        ),
        Err(OperationRequestError::InvalidProvidedDocuments(_))
    ));
}

#[test]
fn provided_documents_must_be_an_array() {
    assert!(matches!(
        validate_operation_payload(
            OperationName::WorkItemCreate,
            &create_payload(json!({"intent":"PRD"}))
        ),
        Err(OperationRequestError::PayloadFieldType("providedDocuments"))
    ));
}
