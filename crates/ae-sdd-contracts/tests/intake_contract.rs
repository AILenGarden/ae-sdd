use ae_sdd_contracts::{
    AssessmentFact, BootstrapAssessment, BootstrapAssessmentError, BoundedText, ConflictDimension,
    DocumentId, DocumentVersionError, DocumentVersionId, EngineeringRoute, EngineeringRouteError,
    FingerprintInputs, InputSource, ReasonCode, ReceiptStatus, RequirementAnalysisEvidence,
    RequirementConflict, RequirementConflictError, RequirementSourceRef, RouteApprovalReceipt,
    RouteBindingInput, RouteDecisionId, RouteMappingVersion, SchemaVersion, SeriesId, SeriesKind,
    SpecGraphId, SpecKind, TaskKind,
    series::{RouteDecision, RouteDisposition},
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, DecisionDigest, DesignRoute, InputFingerprint,
    InventoryGeneration, PolicyDigest, ProjectRelativePath, SessionId, StateRevision, TurnId,
    WorkItemId, WorkScale,
};
use uuid::Uuid;

fn fact(dimension: &str, value: &str) -> AssessmentFact {
    AssessmentFact::new(
        BoundedText::new(dimension).expect("dimension"),
        BoundedText::new(value).expect("value"),
    )
}

/// §5.3 requires the Agent to "list the judging facts, not just return an
/// enum". A report carrying only `scaleProposal` is an unsupported guess, so the
/// contract has to reject it rather than record it as an audit fact.
#[test]
fn assessment_without_facts_is_rejected() {
    let error = BootstrapAssessment::new(
        SchemaVersion::V1,
        TaskKind::Implementation,
        WorkScale::Medium,
        vec![InputSource::Oral],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect_err("a factless proposal must not be accepted");

    assert_eq!(error, BootstrapAssessmentError::MissingFacts);
}

/// §5.3 requires the input-source list to come back from the Agent. An empty
/// list would let RA skip source reconciliation entirely, which §6.1 forbids.
#[test]
fn assessment_without_an_input_source_is_rejected() {
    let error = BootstrapAssessment::new(
        SchemaVersion::V1,
        TaskKind::SelfUpdate,
        WorkScale::Micro,
        Vec::new(),
        vec![fact("interfaceImpact", "none")],
        Vec::new(),
        Vec::new(),
    )
    .expect_err("an assessment with no declared source must not be accepted");

    assert_eq!(error, BootstrapAssessmentError::MissingInputSource);
}

/// §6.1 allows any combination of the three sources, and §6.2 rule 3 forbids a
/// silent precedence order. Storing them deduplicated and sorted keeps the set
/// canonical so no reader can infer a winner from arrival order.
#[test]
fn input_sources_are_canonical_and_carry_no_precedence() {
    let assessment = BootstrapAssessment::new(
        SchemaVersion::V1,
        TaskKind::Implementation,
        WorkScale::Large,
        vec![
            InputSource::Prd,
            InputSource::Oral,
            InputSource::Prd,
            InputSource::Prototype,
        ],
        vec![fact("interfaceImpact", "3 related APIs")],
        Vec::new(),
        Vec::new(),
    )
    .expect("assessment");

    assert_eq!(
        assessment.input_sources(),
        [InputSource::Oral, InputSource::Prototype, InputSource::Prd],
        "duplicates collapse and order is canonical, not arrival order"
    );
}

/// The whole point of §5.3 is that this report is a *proposal*. Round-tripping
/// pins the wire encoding so a later slice cannot quietly rename the fields and
/// have the daemon read a proposal as authority.
#[test]
fn assessment_round_trips_its_frozen_wire_encoding() {
    let assessment = BootstrapAssessment::new(
        SchemaVersion::V1,
        TaskKind::SelfUpdate,
        WorkScale::Medium,
        vec![InputSource::Oral],
        vec![fact("interfaceImpact", "3 related APIs")],
        vec![BoundedText::new("scale may change after RA").expect("uncertainty")],
        Vec::new(),
    )
    .expect("assessment");

    let encoded = serde_json::to_value(&assessment).expect("serialize");
    assert_eq!(encoded["taskKindProposal"], "self_update");
    assert_eq!(encoded["scaleProposal"], "medium");
    assert_eq!(encoded["inputSources"], serde_json::json!(["oral"]));

    let decoded: BootstrapAssessment = serde_json::from_value(encoded).expect("deserialize");
    assert_eq!(decoded, assessment, "the encoding must be lossless");
}

/// An unknown task kind must fail closed rather than defaulting to
/// `implementation`, which would silently route a self-update as project work.
#[test]
fn unknown_task_kind_fails_closed() {
    assert_eq!(
        TaskKind::from_wire("self_update"),
        Some(TaskKind::SelfUpdate)
    );
    assert_eq!(TaskKind::from_wire("selfUpdate"), None);
    assert_eq!(TaskKind::from_wire(""), None);
    assert_eq!(InputSource::from_wire("demo"), None);
    assert_eq!(InputSource::from_wire("prd"), Some(InputSource::Prd));
}

fn oral() -> RequirementSourceRef {
    RequirementSourceRef::Oral {
        session_id: SessionId::from_uuid(Uuid::from_u128(0x11)),
        turn_id: TurnId::from_uuid(Uuid::from_u128(0x12)),
        summary: BoundedText::new("user asked for retry-safe delegation").expect("summary"),
        confirmed: false,
    }
}

fn prototype() -> RequirementSourceRef {
    RequirementSourceRef::Prototype {
        artifact: ArtifactRef::new(
            ArtifactKind::new("prototype").expect("kind"),
            ProjectRelativePath::new("demo/flow.html").expect("path"),
            ArtifactDigest::digest(b"demo"),
            4,
        ),
        observed_behaviour: BoundedText::new("retry button re-runs the series").expect("behaviour"),
    }
}

fn prd() -> RequirementSourceRef {
    RequirementSourceRef::Prd {
        document_id: DocumentId::new("DOC-PRD-001").expect("document id"),
        path: ProjectRelativePath::new("docs/prd/retry.md").expect("path"),
        content_digest: ArtifactDigest::digest(b"prd"),
        version: 3,
        extracted_rule: BoundedText::new("a retry must not reuse the run id").expect("rule"),
    }
}

/// §6.2 rule 6 draws a hard line: a prototype proves observable behaviour only
/// and never a backend rule; a PRD declares intent only and never proves the
/// existing implementation. Collapsing these into one "source" notion is what
/// lets a demo be cited as proof of a backend rule.
#[test]
fn prototype_and_prd_prove_different_things() {
    assert!(
        !prototype().proves_backend_rule(),
        "a demo must not establish a backend rule"
    );
    assert!(
        prototype().proves_existing_implementation(),
        "a demo does show what the code currently does"
    );
    assert!(
        prd().proves_backend_rule(),
        "a PRD does declare a backend rule"
    );
    assert!(
        !prd().proves_existing_implementation(),
        "a PRD declares intent, so it cannot prove current behaviour"
    );
    assert_eq!(oral().source(), InputSource::Oral);
}

/// A "conflict" citing one source is not a conflict. Accepting it would let RA
/// manufacture an `awaiting_user` stop from a single input.
#[test]
fn a_conflict_needs_two_competing_sources() {
    let error = RequirementConflict::new(
        ConflictDimension::Scope,
        BoundedText::new("scope disagreement").expect("statement"),
        vec![oral()],
    )
    .expect_err("one source cannot conflict with itself");

    assert_eq!(error, RequirementConflictError::InsufficientSources);
}

/// §6.2 rule 4 fixes which dimensions force `awaiting_user`. This pins the whole
/// set so a later slice cannot quietly downgrade `security` to advisory.
#[test]
fn material_dimensions_block_routing_and_other_does_not() {
    for dimension in [
        ConflictDimension::Scope,
        ConflictDimension::Acceptance,
        ConflictDimension::Data,
        ConflictDimension::Security,
        ConflictDimension::Route,
    ] {
        let conflict = RequirementConflict::new(
            dimension,
            BoundedText::new("material clash").expect("statement"),
            vec![prototype(), prd()],
        )
        .expect("conflict");
        assert!(
            conflict.blocks_routing(),
            "{} must stop routing per §6.2 rule 4",
            dimension.as_wire()
        );
    }

    let other = RequirementConflict::new(
        ConflictDimension::Other,
        BoundedText::new("wording nit").expect("statement"),
        vec![oral(), prd()],
    )
    .expect("conflict");
    assert!(
        !other.blocks_routing(),
        "a non-material conflict is recorded but does not by itself block the route"
    );
    assert_eq!(
        ConflictDimension::from_wire("security"),
        Some(ConflictDimension::Security)
    );
    assert_eq!(ConflictDimension::from_wire("scale"), None);
}

fn ra_evidence() -> RequirementAnalysisEvidence {
    RequirementAnalysisEvidence::new(
        WorkItemId::new("ROUTE-001").expect("work item id"),
        SeriesId::new("SERIES-RA-001").expect("series id"),
        DocumentId::new("DOC-RA-001").expect("document id"),
        2,
        ArtifactDigest::digest(b"ra content"),
        StateRevision::new(7),
        ArtifactDigest::digest(b"collected RA receipt"),
        ReceiptStatus::Verified,
        WorkScale::Medium,
        ArtifactDigest::digest(b"six-dimension scale evidence"),
        ArtifactDigest::digest(b"G-RA-1..4 closure receipts"),
    )
}

fn route_binding() -> RouteBindingInput {
    RouteBindingInput::new(ra_evidence(), RouteMappingVersion::V1)
}

fn route_decision(binding: &RouteBindingInput) -> RouteDecision {
    RouteDecision::new(
        SchemaVersion::V2,
        RouteDecisionId::new("route-ROUTE-001-r1").expect("decision id"),
        WorkItemId::new("ROUTE-001").expect("work item id"),
        TaskKind::Implementation,
        WorkScale::Medium,
        DesignRoute::Story,
        RouteDisposition::Approved,
        vec![ReasonCode::new("route.ra-closed").expect("reason")],
        vec![
            SeriesKind::new("story").expect("series kind"),
            SeriesKind::new("testcase").expect("series kind"),
            SeriesKind::new("coding-plan").expect("series kind"),
        ],
        vec![SpecKind::Story, SpecKind::TestCase, SpecKind::CodingPlan],
        binding.fingerprint(),
        None,
        DecisionDigest::digest(b"decision"),
    )
    .expect("route decision")
}

fn route_approval(binding: &RouteBindingInput, decision: &RouteDecision) -> RouteApprovalReceipt {
    RouteApprovalReceipt::new(
        "route:approved-r1".to_owned(),
        "user:owner".to_owned(),
        "2026-08-10T00:00:00Z".to_owned(),
        binding.ra_evidence().document_id().clone(),
        binding.ra_evidence().version(),
        *binding.ra_evidence().ra_content_digest(),
        binding.ra_evidence().scale(),
        decision.decision_digest(),
    )
}

/// §6.2 rule 4 sends the flow to `awaiting_user` on a material conflict and
/// forbids routing from continuing. Expressing that as a constructor guard makes
/// it unforgeable: no caller can hold an authoritative route while a scope,
/// acceptance, data, security or route conflict is still open.
#[test]
fn a_route_cannot_freeze_while_a_material_conflict_is_open() {
    let blocking = RequirementConflict::new(
        ConflictDimension::Security,
        BoundedText::new("PRD and demo disagree on auth").expect("statement"),
        vec![prototype(), prd()],
    )
    .expect("conflict");

    let binding = route_binding();
    let decision = route_decision(&binding);
    let approval = route_approval(&binding, &decision);
    let error = EngineeringRoute::freeze(
        SchemaVersion::V2,
        &binding,
        decision,
        &approval,
        &[blocking],
    )
    .expect_err("a security conflict must block the freeze");

    assert_eq!(
        error,
        EngineeringRouteError::BlockingConflictOpen {
            dimension: ConflictDimension::Security
        }
    );
}

/// A non-material conflict is recorded but must not stop the route, otherwise
/// every wording nit would deadlock the flow.
#[test]
fn a_route_freezes_over_non_material_conflicts_and_carries_its_ra_evidence() {
    let nit = RequirementConflict::new(
        ConflictDimension::Other,
        BoundedText::new("wording nit").expect("statement"),
        vec![oral(), prd()],
    )
    .expect("conflict");

    let binding = route_binding();
    let decision = route_decision(&binding);
    let approval = route_approval(&binding, &decision);
    let route = EngineeringRoute::freeze(SchemaVersion::V2, &binding, decision, &approval, &[nit])
        .expect("a non-material conflict must not block the freeze");

    assert_eq!(
        route.evidence().series_id().as_str(),
        "SERIES-RA-001",
        "the frozen route names the RA Series that authorised it"
    );
    assert_eq!(
        route.decision().required_series().len(),
        3,
        "post-route planning excludes the already collected RA Series"
    );
}

/// §4.1 defines `DocumentVersionId` as derived from `DocumentId +
/// contentDigest + version`. Freezing the derivation rather than an opaque
/// newtype is what makes this checkable: the same three inputs must always
/// produce the same identity, and any differing input must produce a different
/// one. An opaque id would let two writers mint different ids for one version.
#[test]
fn document_version_identity_is_a_pure_function_of_its_three_inputs() {
    let document = DocumentId::new("DOC-RA-001").expect("document id");
    let digest = ArtifactDigest::digest(b"ra v1");

    let first = DocumentVersionId::derive(document.clone(), digest, 1).expect("derive");
    let again = DocumentVersionId::derive(document.clone(), digest, 1).expect("derive");
    assert_eq!(first, again, "identical inputs must derive one identity");
    assert_eq!(first.to_wire(), again.to_wire(), "and one wire encoding");

    let next_version = DocumentVersionId::derive(document.clone(), digest, 2).expect("derive");
    assert_ne!(first, next_version, "a new version is a new identity");

    let edited =
        DocumentVersionId::derive(document, ArtifactDigest::digest(b"ra v2"), 1).expect("derive");
    assert_ne!(
        first, edited,
        "changed content cannot reuse an existing version identity"
    );

    let other_document = DocumentVersionId::derive(
        DocumentId::new("DOC-DR-001").expect("document id"),
        digest,
        1,
    )
    .expect("derive");
    assert_ne!(
        first, other_document,
        "identical content in a different document is a different version"
    );
}

/// Versions are 1-based. Accepting 0 would make "no version yet" and "first
/// version" indistinguishable.
#[test]
fn a_zero_document_version_is_rejected() {
    let error = DocumentVersionId::derive(
        DocumentId::new("DOC-RA-001").expect("document id"),
        ArtifactDigest::digest(b"content"),
        0,
    )
    .expect_err("version 0 is not a content version");

    assert_eq!(error, DocumentVersionError::ZeroVersion);
}

fn fingerprint_inputs(revision: u64, content: &[u8]) -> FingerprintInputs {
    FingerprintInputs::new(
        StateRevision::new(revision),
        vec![
            DocumentVersionId::derive(
                DocumentId::new("DOC-RA-001").expect("document id"),
                ArtifactDigest::digest(content),
                1,
            )
            .expect("derive"),
        ],
        ArtifactDigest::digest(b"context bundle"),
        PolicyDigest::digest(b"policy"),
        InventoryGeneration::new(3),
    )
}

/// F-10: a decision digest proves *which decision*, an input fingerprint proves
/// *what the decision stood on*. If the fingerprint only moves when the decision
/// moves, a Spec edit or state advance becomes invisible. Each of the five
/// canonical inputs must therefore move the fingerprint on its own.
#[test]
fn every_canonical_input_moves_the_fingerprint_independently() {
    let base = fingerprint_inputs(7, b"ra v1");
    let baseline = base.fingerprint();

    assert_eq!(
        baseline,
        fingerprint_inputs(7, b"ra v1").fingerprint(),
        "the same five inputs must produce one fingerprint"
    );
    assert_ne!(
        baseline,
        fingerprint_inputs(8, b"ra v1").fingerprint(),
        "a state advance must be visible in the fingerprint"
    );
    assert_ne!(
        baseline,
        fingerprint_inputs(7, b"ra v2").fingerprint(),
        "a Spec content change must be visible in the fingerprint"
    );

    let other_context = FingerprintInputs::new(
        StateRevision::new(7),
        base.document_versions().to_vec(),
        ArtifactDigest::digest(b"different bundle"),
        PolicyDigest::digest(b"policy"),
        InventoryGeneration::new(3),
    );
    assert_ne!(
        baseline,
        other_context.fingerprint(),
        "a context bundle change must be visible"
    );

    let other_policy = FingerprintInputs::new(
        StateRevision::new(7),
        base.document_versions().to_vec(),
        ArtifactDigest::digest(b"context bundle"),
        PolicyDigest::digest(b"policy v2"),
        InventoryGeneration::new(3),
    );
    assert_ne!(
        baseline,
        other_policy.fingerprint(),
        "a policy change must be visible"
    );

    let other_inventory = FingerprintInputs::new(
        StateRevision::new(7),
        base.document_versions().to_vec(),
        ArtifactDigest::digest(b"context bundle"),
        PolicyDigest::digest(b"policy"),
        InventoryGeneration::new(4),
    );
    assert_ne!(
        baseline,
        other_inventory.fingerprint(),
        "an inventory generation change must be visible"
    );
}

/// Document version order is an observation artefact, not a fact. Two callers
/// listing the same versions in different order must agree, or replay would
/// wrongly report the inputs as changed.
#[test]
fn document_version_order_does_not_change_the_fingerprint() {
    let first = DocumentVersionId::derive(
        DocumentId::new("DOC-RA-001").expect("document id"),
        ArtifactDigest::digest(b"ra"),
        1,
    )
    .expect("derive");
    let second = DocumentVersionId::derive(
        DocumentId::new("DOC-DR-001").expect("document id"),
        ArtifactDigest::digest(b"dr"),
        2,
    )
    .expect("derive");

    let forward = FingerprintInputs::new(
        StateRevision::new(9),
        vec![first.clone(), second.clone()],
        ArtifactDigest::digest(b"bundle"),
        PolicyDigest::digest(b"policy"),
        InventoryGeneration::new(1),
    );
    let reversed = FingerprintInputs::new(
        StateRevision::new(9),
        vec![second, first],
        ArtifactDigest::digest(b"bundle"),
        PolicyDigest::digest(b"policy"),
        InventoryGeneration::new(1),
    );

    assert_eq!(
        forward.fingerprint(),
        reversed.fingerprint(),
        "observation order must not be mistaken for an input change"
    );
}

/// D-02 item 1 freezes the wire encoding of the three intake types that had
/// constructor validation but no serde contract.
///
/// A round-trip assertion alone would pass with a plain `derive(Deserialize)`,
/// which reconstructs a value without consulting the constructor. This test
/// therefore also feeds each type a payload its constructor would refuse. A
/// 1-source conflict is the sharp case: `RequirementConflict::new` requires two
/// competing sources because §6.2 rule 3 forbids resolving a clash by
/// precedence, and a conflict with one source has nothing to clash with.
#[test]
fn intake_wire_encodings_are_frozen_and_reject_what_constructors_refuse() {
    let conflict = RequirementConflict::new(
        ConflictDimension::Security,
        BoundedText::new("PRD and demo disagree on auth").expect("statement"),
        vec![prototype(), prd()],
    )
    .expect("conflict");

    let encoded = serde_json::to_value(&conflict).expect("serialize conflict");
    assert_eq!(
        encoded["sources"].as_array().map(Vec::len),
        Some(2),
        "both competing sources must survive the wire, not a merged conclusion"
    );
    assert_eq!(
        serde_json::from_value::<RequirementConflict>(encoded).expect("round trip"),
        conflict
    );

    let one_source = serde_json::json!({
        "dimension": "security",
        "statement": "PRD and demo disagree on auth",
        "sources": [serde_json::to_value(prd()).expect("serialize source")],
    });
    assert!(
        serde_json::from_value::<RequirementConflict>(one_source).is_err(),
        "a 1-source conflict must fail on the wire exactly as it fails in new()"
    );

    for source in [oral(), prototype(), prd()] {
        let json = serde_json::to_value(&source).expect("serialize source");
        assert_eq!(
            serde_json::from_value::<RequirementSourceRef>(json).expect("round trip"),
            source,
            "each source variant keeps its distinct evidence on the wire"
        );
    }

    let unknown_variant = serde_json::json!({"kind": "telepathy", "summary": "x"});
    assert!(
        serde_json::from_value::<RequirementSourceRef>(unknown_variant).is_err(),
        "an unrecognised source kind must fail closed, not degrade to oral"
    );
}

/// §5.4 line 256 requires RA closure bind "既有 `DocumentId` 和本次读取的内容版本".
/// The struct's own doc comment already asserted that requirement while the struct
/// carried neither field — evidence proved "some RA content had this digest" but
/// not which logical document, so two Work Items with coincidentally identical RA
/// content produced interchangeable evidence.
#[test]
fn ra_closure_evidence_names_a_complete_document_version() {
    let evidence = ra_evidence();
    let version = evidence
        .document_version()
        .expect("version 2 is a valid content version");
    assert_eq!(
        version.document_id(),
        &DocumentId::new("DOC-RA-001").expect("document id")
    );
    assert_eq!(version.version(), 2);
    assert_eq!(
        version.content_digest(),
        &ArtifactDigest::digest(b"ra content")
    );
    assert_eq!(
        version.to_wire(),
        format!(
            "DOC-RA-001@2#{}",
            ArtifactDigest::digest(b"ra content")
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        ),
        "the derived identity recovers all three inputs"
    );

    let encoded = serde_json::to_value(&evidence).expect("serialize evidence");
    for key in ["documentId", "version", "raContentDigest"] {
        assert!(
            encoded.get(key).is_some(),
            "the §4.1 line 132 triple must be on the wire: {encoded}"
        );
    }
    assert_eq!(
        serde_json::from_value::<RequirementAnalysisEvidence>(encoded.clone()).expect("round trip"),
        evidence
    );

    for key in ["documentId", "version"] {
        let mut stripped = encoded.clone();
        stripped
            .as_object_mut()
            .expect("object")
            .remove(key)
            .expect("field present before removal");
        assert!(
            serde_json::from_value::<RequirementAnalysisEvidence>(stripped).is_err(),
            "RA evidence missing {key} cannot name a version, so it must fail closed"
        );
    }

    // Same Series and same digest, different document: the evidence must no longer
    // be interchangeable, which is the whole reason `document_id` was added.
    let other_document = RequirementAnalysisEvidence::new(
        WorkItemId::new("ROUTE-001").expect("work item id"),
        SeriesId::new("SERIES-RA-001").expect("series id"),
        DocumentId::new("DOC-RA-002").expect("document id"),
        2,
        ArtifactDigest::digest(b"ra content"),
        StateRevision::new(7),
        ArtifactDigest::digest(b"collected RA receipt"),
        ReceiptStatus::Verified,
        WorkScale::Medium,
        ArtifactDigest::digest(b"six-dimension scale evidence"),
        ArtifactDigest::digest(b"G-RA-1..4 closure receipts"),
    );
    assert_ne!(other_document, evidence);
    assert_ne!(
        other_document.document_version().expect("valid version"),
        version
    );
}

/// §6.1 line 302 requires an oral source cite its original *turn*, not just its
/// session. A session-only citation cannot locate what the user said: one session
/// carries many turns, so the turn that stated a requirement and a later turn that
/// revised it would decode to the same reference.
#[test]
fn an_oral_source_without_its_turn_fails_to_decode() {
    let mut encoded = serde_json::to_value(oral()).expect("serialize oral");
    assert_eq!(
        encoded.get("turn_id").and_then(serde_json::Value::as_str),
        Some("00000000-0000-0000-0000-000000000012"),
        "the oral wire form must carry its turn reference under the frozen snake_case key"
    );
    encoded
        .as_object_mut()
        .expect("object")
        .remove("turn_id")
        .expect("turn_id present before removal");
    assert!(
        serde_json::from_value::<RequirementSourceRef>(encoded).is_err(),
        "a turn-less oral citation must fail closed rather than decode as session-only"
    );
}

/// §6.1 line 304 lists four facts a PRD source must keep — id, path, digest and
/// version — and §4.1 line 132 makes the id, digest and version the three inputs
/// of a `DocumentVersionId`. The point of carrying `version` is that the citation
/// can name that identity; without it the reference cannot distinguish two content
/// versions of one document.
#[test]
fn a_prd_source_names_a_complete_document_version() {
    let source = prd();
    let derived = source
        .document_version()
        .expect("a PRD source names a document version")
        .expect("version 3 is a valid content version");
    assert_eq!(derived.version(), 3);
    assert_eq!(
        derived.document_id(),
        &DocumentId::new("DOC-PRD-001").expect("document id")
    );
    assert_eq!(derived.content_digest(), &ArtifactDigest::digest(b"prd"));

    let encoded = serde_json::to_value(&source).expect("serialize prd");
    assert_eq!(
        encoded.get("path").and_then(serde_json::Value::as_str),
        Some("docs/prd/retry.md"),
        "§6.1 line 304 requires the path on the wire, not only in memory"
    );
    assert_eq!(
        encoded.get("version").and_then(serde_json::Value::as_u64),
        Some(3)
    );
    let path_dropped = {
        let mut stripped = encoded;
        stripped
            .as_object_mut()
            .expect("object")
            .remove("path")
            .expect("path present before removal");
        stripped
    };
    assert!(
        serde_json::from_value::<RequirementSourceRef>(path_dropped).is_err(),
        "§6.1 line 304 requires the path, so a path-less PRD citation must fail closed"
    );

    let zero_version = RequirementSourceRef::Prd {
        document_id: DocumentId::new("DOC-PRD-001").expect("document id"),
        path: ProjectRelativePath::new("docs/prd/retry.md").expect("path"),
        content_digest: ArtifactDigest::digest(b"prd"),
        version: 0,
        extracted_rule: BoundedText::new("a retry must not reuse the run id").expect("rule"),
    };
    assert_eq!(
        zero_version.document_version().expect("a PRD source"),
        Err(DocumentVersionError::ZeroVersion),
        "a zero version is surfaced, not silently defaulted to 1"
    );

    assert!(
        oral().document_version().is_none() && prototype().document_version().is_none(),
        "only a PRD citation names a document version"
    );
}

/// The `EngineeringRoute` wire contract, including the guard it cannot restore.
///
/// `EvidenceNotBound` reads a stored field, so an unbound-evidence payload must
/// fail to decode. `BlockingConflictOpen` reads `freeze`'s external
/// `open_conflicts` parameter, which is never stored, so decode cannot re-run it
/// — this test pins that limit so nobody later reads decode success as proof
/// that no conflict was open.
#[test]
fn engineering_route_wire_restores_evidence_binding_but_not_conflict_freedom() {
    let binding = RouteBindingInput::new(
        RequirementAnalysisEvidence::new(
            WorkItemId::new("ROUTE-001").expect("work item id"),
            SeriesId::new("SER-RA-1").expect("series id"),
            DocumentId::new("DOC-RA-1").expect("document id"),
            1,
            ArtifactDigest::digest(b"ra"),
            StateRevision::new(7),
            ArtifactDigest::digest(b"receipt"),
            ReceiptStatus::Verified,
            WorkScale::Medium,
            ArtifactDigest::digest(b"scale evidence"),
            ArtifactDigest::digest(b"closure"),
        ),
        RouteMappingVersion::V1,
    );
    let decision = route_decision(&binding);
    let approval = route_approval(&binding, &decision);
    let route = EngineeringRoute::freeze(SchemaVersion::V2, &binding, decision, &approval, &[])
        .expect("route freezes with no open conflicts");

    let encoded = serde_json::to_value(&route).expect("serialize route");
    assert_eq!(
        serde_json::from_value::<EngineeringRoute>(encoded.clone()).expect("round trip"),
        route
    );

    assert!(
        encoded.get("openConflicts").is_none(),
        "the conflict set is not part of the encoding, which is why decode \
         cannot re-check BlockingConflictOpen"
    );

    let mut unbound = encoded.clone();
    unbound["decision"]["inputFingerprint"] =
        serde_json::json!(InputFingerprint::digest(b"").to_string());
    let err = serde_json::from_value::<EngineeringRoute>(unbound).unwrap_err();
    assert!(
        err.to_string().contains("fingerprint"),
        "the wire must recompute the RA binding fingerprint; got: {err}"
    );

    let mut approval_tampered = encoded;
    approval_tampered["approvalReceipt"]["boundVersion"] = serde_json::json!(99);
    let err = serde_json::from_value::<EngineeringRoute>(approval_tampered).unwrap_err();
    assert!(err.to_string().contains("approval"), "got: {err}");
}

/// D-02 item 4 freezes the *independent* computation of the two proof
/// dimensions. "Independent" here does not mean unrelated, and stating it as
/// symmetric independence would be wrong: a decision digest covers the inputs
/// the decision stood on, so moving an input moves both values. The real
/// contract is an asymmetry.
///
/// Forward: changing any canonical input moves the fingerprint, which is what
/// makes a Spec edit or state advance detectable.
///
/// Reverse, and this is the direction F-10 broke: the fingerprint must never be
/// *derived from* the decision. `service_host.rs` assigned `decision_digest` to
/// `input_fingerprint` and then required them equal on read, which left the
/// fingerprint moving only when the decision moved — the freshness dimension
/// silently disappeared while both fields still looked populated.
///
/// The two computations are also domain-separated, but that property is
/// deliberately *not* asserted here: any test comparing a fingerprint to a
/// decision digest passes whether or not the separators exist, so the assertion
/// would be unfalsifiable. Deleting the separator from `provenance.rs` was
/// confirmed not to fail such a check. What this test proves is the pure-function
/// property below, which is the half F-10 actually broke.
#[test]
fn the_fingerprint_is_never_derivable_from_a_decision_digest() {
    let inputs = fingerprint_inputs(7, b"ra v1");
    let fingerprint = inputs.fingerprint();

    // The forward direction: an input moving must move the fingerprint, or
    // freshness is unobservable.
    assert_ne!(
        fingerprint,
        fingerprint_inputs(8, b"ra v1").fingerprint(),
        "a state revision advance must be visible in the fingerprint"
    );
    assert_ne!(
        fingerprint,
        fingerprint_inputs(7, b"ra v2").fingerprint(),
        "a document content change must be visible in the fingerprint"
    );

    // The reverse direction: the fingerprint is a pure function of its five
    // canonical inputs and nothing else. Two callers on the same inputs agree
    // regardless of what decision either of them went on to make.
    assert_eq!(
        fingerprint,
        fingerprint_inputs(7, b"ra v1").fingerprint(),
        "the fingerprint depends on inputs alone, so no decision can perturb it"
    );
}

/// `SpecGraphId` is frozen ahead of the registry that will own it, and was until
/// now the only identity in D-02 item 2's list with no test at all.
///
/// §8.3 makes Spec relations a directed graph rather than a forced tree, and §8.4
/// rule 3 creates a new graph when a resolved `DocumentId` belongs to none. So a
/// graph is addressable independently of its documents, and its identity has to
/// validate on the wire before any storage exists — otherwise the first consumer
/// silently defines the format.
#[test]
fn a_spec_graph_identity_validates_on_the_wire_before_its_registry_exists() {
    let id = SpecGraphId::new("GRAPH-RA-001").expect("spec graph id");

    let encoded = serde_json::to_value(&id).expect("serialize");
    assert_eq!(
        encoded,
        serde_json::json!("GRAPH-RA-001"),
        "a portable identifier is transparent on the wire, not a wrapper object"
    );
    assert_eq!(
        serde_json::from_value::<SpecGraphId>(encoded).expect("round trip"),
        id
    );

    assert!(
        SpecGraphId::new("").is_err(),
        "an empty graph identity must be refused"
    );
    assert!(
        serde_json::from_value::<SpecGraphId>(serde_json::json!("")).is_err(),
        "the wire must refuse what the constructor refuses, or the first consumer \
         to decode an empty identity defines the format"
    );
}

/// The two constructor guarantees a derived `Deserialize` silently skipped.
///
/// Both were verified against the pre-fix type: `"facts": []` decoded with zero
/// facts, and `["prd","oral","prd","oral"]` decoded verbatim as
/// `[Prd, Oral, Prd, Oral]`. The second matters more than it looks. §6.2 rule 3
/// forbids resolving competing sources by precedence, and an order preserved from
/// the wire is an implied precedence — the first-listed source reads as the
/// primary one. Canonicalizing in `new` and then bypassing `new` on decode meant
/// the rule held in process and lapsed across the boundary.
#[test]
fn a_decoded_assessment_is_validated_and_canonicalized_like_a_constructed_one() {
    let factless = serde_json::json!({
        "schemaVersion":"v1",
        "taskKindProposal":"implementation",
        "scaleProposal":"small",
        "inputSources":["oral"],
        "facts":[],
        "uncertainties":[],
        "userQuestions":[],
    });
    assert!(
        serde_json::from_value::<BootstrapAssessment>(factless).is_err(),
        "an assessment with no facts must fail on the wire exactly as new() \
         fails it"
    );

    let unsorted_with_duplicates = serde_json::json!({
        "schemaVersion":"v1",
        "taskKindProposal":"implementation",
        "scaleProposal":"small",
        "inputSources":["prd","oral","prd","oral"],
        "facts":[{"dimension":"scope","value":"retry-safe delegation"}],
        "uncertainties":[],
        "userQuestions":[],
    });
    let decoded: BootstrapAssessment =
        serde_json::from_value(unsorted_with_duplicates).expect("valid once canonicalized");
    assert_eq!(
        decoded.input_sources().len(),
        2,
        "duplicates must collapse on decode, not survive as repeated sources"
    );
    assert_eq!(
        decoded.input_sources(),
        &[InputSource::Oral, InputSource::Prd],
        "decode must reach the same canonical order as new(), so no source \
         inherits precedence from its position in the payload"
    );

    let sourceless = serde_json::json!({
        "schemaVersion":"v1",
        "taskKindProposal":"implementation",
        "scaleProposal":"small",
        "inputSources":[],
        "facts":[{"dimension":"scope","value":"x"}],
        "uncertainties":[],
        "userQuestions":[],
    });
    assert!(
        serde_json::from_value::<BootstrapAssessment>(sourceless).is_err(),
        "§5.2 requires at least one input source; the wire must enforce it"
    );
}
