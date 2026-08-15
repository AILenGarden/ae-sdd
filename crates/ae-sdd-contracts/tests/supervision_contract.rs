//! Supervision event contract tests (`ae-sdd-daemon-design.md` §11.3, §6.2).

use ae_sdd_contracts::{
    BoundedText, ConflictDimension, DocumentId, RequirementRulingEvent, RequirementSourceRef,
    SchemaVersion, SeriesId, SeriesProgressEvent, SeriesSubNode, SupervisionEventError,
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, EpochMillis, EventSequence, ProjectRelativePath,
    SeriesRunId, SessionId, StateRevision, TurnId,
};
use uuid::Uuid;

fn oral() -> RequirementSourceRef {
    RequirementSourceRef::Oral {
        session_id: SessionId::from_uuid(Uuid::from_u128(0x11)),
        turn_id: TurnId::from_uuid(Uuid::from_u128(0x12)),
        summary: BoundedText::new("user wants retry-safe delegation").expect("summary"),
        confirmed: true,
    }
}

fn prd() -> RequirementSourceRef {
    RequirementSourceRef::Prd {
        document_id: DocumentId::new("DOC-PRD-1").expect("document id"),
        path: ProjectRelativePath::new("docs/prd/retry.md").expect("path"),
        content_digest: ArtifactDigest::digest(b"prd"),
        version: 2,
        extracted_rule: BoundedText::new("a retry must not reuse the run id").expect("rule"),
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
        observed_behaviour: BoundedText::new("retry reuses the run id").expect("behaviour"),
    }
}

/// §11.3 requires global ordering, and its closing line forbids an event from
/// carrying reasoning or document text. The round-trip proves the ordering and
/// revision survive; the field count proves nothing unbounded was added.
#[test]
fn a_progress_event_carries_global_order_and_nothing_unbounded() {
    let event = SeriesProgressEvent::observe(
        EventSequence::new(42),
        SeriesId::new("SER-STORY-1").expect("series id"),
        "00000000-0000-0000-0000-0000000000a1"
            .parse::<SeriesRunId>()
            .expect("series run id"),
        SeriesSubNode::Validate,
        StateRevision::new(9),
        EpochMillis::new(1_700_000_000_000),
    );

    assert_eq!(event.schema_version(), SchemaVersion::V1);
    assert_eq!(event.sequence(), EventSequence::new(42));
    assert_eq!(event.sub_node(), SeriesSubNode::Validate);

    let encoded = serde_json::to_value(&event).expect("serialize");
    let object = encoded.as_object().expect("object");
    // 6 -> 7 on 2026-08-02 when `seriesRunId` was added so an observation can name
    // the attempt it came from (§9.1 line 452). The guard did its job: it caught the
    // addition and forced this note. §11.3 still caps what may be recorded — a typed
    // run identity is bounded, unlike the reasoning or document text it forbids.
    assert_eq!(
        object.len(),
        7,
        "an added field is a contract change; §11.3 caps what may be recorded"
    );
    assert_eq!(encoded["subNode"], serde_json::json!("validate"));
    assert_eq!(encoded["sequence"], serde_json::json!(42));

    assert_eq!(
        serde_json::from_value::<SeriesProgressEvent>(encoded).expect("round trip"),
        event
    );
}

/// §6.2 rule 5 requires a ruling to retain the rejected branch and its reason
/// while leaving the original inputs unrewritten. This test proves the rejected
/// source is stored rather than inferable, and that it survives the wire intact.
#[test]
fn a_ruling_retains_the_rejected_branch_and_its_reason() {
    let ruling = RequirementRulingEvent::decide(
        EventSequence::new(7),
        ConflictDimension::Security,
        prd(),
        vec![prototype()],
        BoundedText::new("PRD governs auth; the demo predates the rule").expect("rationale"),
        StateRevision::new(11),
        EpochMillis::new(1_700_000_000_500),
    )
    .expect("ruling decides");

    assert_eq!(ruling.dimension(), ConflictDimension::Security);
    assert_eq!(ruling.chosen(), &prd());
    assert_eq!(ruling.rejected(), &[prototype()]);

    let encoded = serde_json::to_value(&ruling).expect("serialize");
    assert_eq!(
        encoded["rejected"].as_array().map(Vec::len),
        Some(1),
        "the rejected branch is retained on the wire, not dropped once decided"
    );
    assert_eq!(
        encoded["chosen"]["kind"],
        serde_json::json!("prd"),
        "the adopted source keeps its own variant tag"
    );
    assert_eq!(
        serde_json::from_value::<RequirementRulingEvent>(encoded).expect("round trip"),
        ruling
    );
}

/// §6.2 rule 4 holds the flow in `awaiting_user` until a real ruling arrives, so
/// a ruling that settles nothing must not construct. A self-contradictory ruling
/// and an empty-rejection ruling would each release that hold on a non-decision.
#[test]
fn a_ruling_that_settles_nothing_is_refused() {
    let rationale = BoundedText::new("because").expect("rationale");

    assert!(matches!(
        RequirementRulingEvent::decide(
            EventSequence::new(1),
            ConflictDimension::Scope,
            prd(),
            vec![prd()],
            rationale.clone(),
            StateRevision::new(1),
            EpochMillis::new(1),
        ),
        Err(SupervisionEventError::ChosenSourceAlsoRejected)
    ));

    assert!(matches!(
        RequirementRulingEvent::decide(
            EventSequence::new(1),
            ConflictDimension::Scope,
            prd(),
            Vec::new(),
            rationale,
            StateRevision::new(1),
            EpochMillis::new(1),
        ),
        Err(SupervisionEventError::NoRejectedSources)
    ));

    assert!(matches!(
        RequirementRulingEvent::decide(
            EventSequence::new(1),
            ConflictDimension::Scope,
            prd(),
            vec![oral()],
            BoundedText::new("   ").expect("blank rationale"),
            StateRevision::new(1),
            EpochMillis::new(1),
        ),
        Err(SupervisionEventError::EmptyRationale)
    ));
}

/// A wire payload the constructor would refuse must also fail to decode, or the
/// `awaiting_user` hold is enforced only in-process.
#[test]
fn the_wire_cannot_produce_a_ruling_that_settles_nothing() {
    let ruling = RequirementRulingEvent::decide(
        EventSequence::new(7),
        ConflictDimension::Data,
        prd(),
        vec![oral()],
        BoundedText::new("PRD is authoritative on the data model").expect("rationale"),
        StateRevision::new(11),
        EpochMillis::new(1),
    )
    .expect("ruling decides");

    let mut encoded = serde_json::to_value(&ruling).expect("serialize");
    encoded["rejected"] = serde_json::json!([]);
    assert!(
        serde_json::from_value::<RequirementRulingEvent>(encoded.clone()).is_err(),
        "a ruling rejecting nothing must not decode"
    );

    encoded["rejected"] = serde_json::json!([encoded["chosen"].clone()]);
    assert!(
        serde_json::from_value::<RequirementRulingEvent>(encoded).is_err(),
        "a ruling adopting and rejecting the same source must not decode"
    );
}

/// §9.1 line 452 requires a Series transaction define `seriesId/seriesRunId/
/// workItemId`, and §4.1 makes a retry a *new* `SeriesRunId` under the same
/// `SeriesId`. Progress events carried only `seriesId`, so two attempts of one
/// Series emitted indistinguishable observations — a replay could not attribute an
/// advance to the retry rather than the attempt that failed, which is exactly the
/// distinction §11.4's stale marking depends on.
#[test]
fn a_progress_event_names_the_attempt_it_observed() {
    let first_attempt = "00000000-0000-0000-0000-0000000000a1"
        .parse::<SeriesRunId>()
        .expect("series run id");
    let retry = "00000000-0000-0000-0000-0000000000a2"
        .parse::<SeriesRunId>()
        .expect("series run id");
    let series_id = SeriesId::new("SER-STORY-1").expect("series id");

    let observe = |run: SeriesRunId| {
        SeriesProgressEvent::observe(
            EventSequence::new(42),
            series_id.clone(),
            run,
            SeriesSubNode::Validate,
            StateRevision::new(9),
            EpochMillis::new(1_700_000_000_000),
        )
    };
    let original = observe(first_attempt);
    let retried = observe(retry);

    assert_eq!(original.series_run_id(), &first_attempt);
    assert_ne!(
        original, retried,
        "same Series, same sub-node, same revision: only the attempt differs, and \
         that must be enough to tell the two observations apart"
    );

    let encoded = serde_json::to_value(&original).expect("serialize event");
    assert_eq!(
        encoded
            .get("seriesRunId")
            .and_then(serde_json::Value::as_str),
        Some("00000000-0000-0000-0000-0000000000a1"),
        "the attempt must be on the wire, not only in memory: {encoded}"
    );
    assert_eq!(
        serde_json::from_value::<SeriesProgressEvent>(encoded.clone()).expect("round trip"),
        original
    );

    let mut stripped = encoded;
    stripped
        .as_object_mut()
        .expect("object")
        .remove("seriesRunId")
        .expect("seriesRunId present before removal");
    assert!(
        serde_json::from_value::<SeriesProgressEvent>(stripped).is_err(),
        "an observation that cannot name its attempt must fail closed"
    );
}
