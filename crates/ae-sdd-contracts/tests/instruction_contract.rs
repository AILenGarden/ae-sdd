//! `InstructionEnvelope` contract tests (`ae-sdd-daemon-design.md` §10.2).

use ae_sdd_contracts::{
    BoundedText, ContextProjectionRef, InstructionEnvelope, InstructionError, InstructionIdentity,
    InstructionTransaction, SchemaVersion, SeriesId, SeriesKind, SeriesSubNode, SkillId, SkillRef,
};
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, ContextProjectionId, DecisionDigest, DelegationId, EpochMillis,
    FlowRunId, InputFingerprint, InstructionId, PolicyDigest, SeriesRunId, StateRevision,
    WorkItemId,
};
use uuid::Uuid;

fn identity() -> InstructionIdentity {
    InstructionIdentity {
        instruction_id: InstructionId::from_uuid(Uuid::from_u128(0x01)),
        work_item_id: WorkItemId::new("WI-1").expect("work item id"),
        flow_run_id: FlowRunId::from_uuid(Uuid::from_u128(0x02)),
        series_id: SeriesId::new("SER-STORY-1").expect("series id"),
        series_run_id: SeriesRunId::from_uuid(Uuid::from_u128(0x03)),
        delegation_id: DelegationId::from_uuid(Uuid::from_u128(0x04)),
    }
}

fn transaction() -> InstructionTransaction {
    InstructionTransaction::new(
        BoundedText::new("draft the story for retry-safe delegation").expect("objective"),
        vec![BoundedText::new("story-document").expect("output")],
        BoundedText::new("story-report/v1").expect("schema"),
    )
    .expect("transaction")
}

fn projection() -> ContextProjectionRef {
    ContextProjectionRef::new(
        ContextProjectionId::from_uuid(Uuid::from_u128(0x05)),
        ArtifactDigest::digest(b"projection"),
    )
}

fn skills() -> Vec<SkillRef> {
    vec![SkillRef::new(
        SkillId::new("phase1-design.story-generate").expect("skill id"),
        ArtifactDigest::digest(b"skill"),
    )]
}

fn envelope(main_node: &str) -> Result<InstructionEnvelope, InstructionError> {
    InstructionEnvelope::issue(
        identity(),
        StateRevision::new(12),
        DecisionDigest::digest(b"route decision"),
        InputFingerprint::digest(b"route input"),
        SeriesKind::new(main_node).expect("series kind"),
        SeriesSubNode::CollectContext,
        AgentRole::Series,
        transaction(),
        skills(),
        projection(),
        vec![BoundedText::new("write-document").expect("action")],
        EpochMillis::new(1_700_000_000_000),
        PolicyDigest::digest(b"policy"),
    )
}

/// §10.2's closing line requires the envelope bind state revision, policy
/// digest, role and deadline. All four are non-optional fields, so this test
/// proves they survive the wire rather than that they merely exist.
#[test]
fn an_envelope_binds_revision_policy_role_and_deadline_across_the_wire() {
    let issued = envelope("story").expect("envelope issues");

    assert_eq!(issued.schema_version(), SchemaVersion::V1);
    assert_eq!(issued.state_revision(), StateRevision::new(12));
    assert_eq!(issued.role(), AgentRole::Series);
    assert_eq!(issued.policy_digest(), PolicyDigest::digest(b"policy"));
    assert_eq!(issued.expires_at(), EpochMillis::new(1_700_000_000_000));

    let encoded = serde_json::to_value(&issued).expect("serialize envelope");
    assert_eq!(
        encoded["expiresAt"],
        serde_json::json!(1_700_000_000_000_u64),
        "the deadline is Unix milliseconds as a bare number, per \
         constraints/api.md's deadlineUnixMs"
    );
    assert_eq!(
        encoded["subNode"],
        serde_json::json!("collect-context"),
        "sub-nodes keep their kebab-case wire spelling"
    );
    assert_eq!(
        serde_json::from_value::<InstructionEnvelope>(encoded).expect("round trip"),
        issued
    );
}

/// §11.1 makes `currentMainNode` and this field draw from one list, so an
/// envelope naming `story-generate` must be refused. That spelling is a legal
/// `SkillId` — and appears as one in this envelope's `skillRefs` — which is
/// exactly why the two fields cannot share a validation rule.
#[test]
fn an_envelope_refuses_a_main_node_outside_the_frozen_vocabulary() {
    assert!(matches!(
        envelope("story-generate"),
        Err(InstructionError::MainNodeNotFrozen { .. })
    ));

    let issued = envelope("story").expect("envelope issues");
    assert_eq!(
        issued.skill_refs()[0].id().as_str(),
        "phase1-design.story-generate",
        "the same generate spelling is legal as a skill identity"
    );
}

/// §11.4 refuses an unauthorized request without advancing the node, and §9.1
/// requires the transaction name its adopted SKILL assets. An envelope granting
/// no actions could only produce refusals; one citing no skill has no method.
#[test]
fn an_envelope_refuses_empty_grants_and_missing_method_assets() {
    let no_actions = InstructionEnvelope::issue(
        identity(),
        StateRevision::new(12),
        DecisionDigest::digest(b"route decision"),
        InputFingerprint::digest(b"route input"),
        SeriesKind::new("story").expect("series kind"),
        SeriesSubNode::Draft,
        AgentRole::Series,
        transaction(),
        skills(),
        projection(),
        Vec::new(),
        EpochMillis::new(1),
        PolicyDigest::digest(b"policy"),
    );
    assert!(matches!(
        no_actions,
        Err(InstructionError::NoAllowedActions)
    ));

    let no_skills = InstructionEnvelope::issue(
        identity(),
        StateRevision::new(12),
        DecisionDigest::digest(b"route decision"),
        InputFingerprint::digest(b"route input"),
        SeriesKind::new("story").expect("series kind"),
        SeriesSubNode::Draft,
        AgentRole::Series,
        transaction(),
        Vec::new(),
        projection(),
        vec![BoundedText::new("write-document").expect("action")],
        EpochMillis::new(1),
        PolicyDigest::digest(b"policy"),
    );
    assert!(matches!(no_skills, Err(InstructionError::NoSkillRefs)));

    assert!(matches!(
        InstructionTransaction::new(
            BoundedText::new("do a thing").expect("objective"),
            Vec::new(),
            BoundedText::new("report/v1").expect("schema"),
        ),
        Err(InstructionError::NoRequiredOutputs)
    ));
}

/// The deadline boundary is inclusive, so an envelope is expired at exactly its
/// `expiresAt`. §10.2 uses the deadline to stop an old instruction replaying on
/// new state; an exclusive boundary would leave one instant where a stale
/// envelope is still honoured.
#[test]
fn the_deadline_boundary_fails_closed() {
    let issued = envelope("story").expect("envelope issues");
    let deadline = issued.expires_at();

    assert!(!issued.is_expired_at(EpochMillis::new(deadline.get() - 1)));
    assert!(issued.is_expired_at(deadline));
    assert!(issued.is_expired_at(EpochMillis::new(deadline.get() + 1)));
}

/// A wire payload that would fail [`InstructionEnvelope::issue`] must also fail
/// to decode, or the constructor's guarantees stop at the process boundary.
#[test]
fn the_wire_cannot_produce_an_envelope_the_constructor_would_refuse() {
    let issued = envelope("story").expect("envelope issues");
    let mut encoded = serde_json::to_value(&issued).expect("serialize");

    encoded["allowedActions"] = serde_json::json!([]);
    assert!(
        serde_json::from_value::<InstructionEnvelope>(encoded.clone()).is_err(),
        "an actionless envelope must not decode"
    );

    encoded["allowedActions"] = serde_json::json!(["write-document"]);
    encoded["mainNode"] = serde_json::json!("story-generate");
    assert!(
        serde_json::from_value::<InstructionEnvelope>(encoded).is_err(),
        "a non-frozen main node must not decode"
    );
}

/// §9.1 line 454 requires the envelope carry "前置状态 revision 与 route decision
/// digest". Only the revision was present, so an envelope named no authority for
/// the work it authorised: an instruction issued against a route that had since
/// been superseded was indistinguishable from one issued against the current
/// decision. The fingerprint comes with the digest because §5.5's re-route rule
/// turns on the inputs changing — the digest says *which* decision, the
/// fingerprint says *what it was decided from*.
#[test]
fn an_envelope_binds_the_route_decision_it_executes_under() {
    let envelope = envelope("story").expect("frozen main node");
    assert_eq!(
        envelope.decision_digest(),
        DecisionDigest::digest(b"route decision")
    );
    assert_eq!(
        envelope.input_fingerprint(),
        InputFingerprint::digest(b"route input")
    );

    let encoded = serde_json::to_value(&envelope).expect("serialize envelope");
    for key in ["stateRevision", "decisionDigest", "inputFingerprint"] {
        assert!(
            encoded.get(key).is_some(),
            "§9.1 line 454 requires {key} on the envelope: {encoded}"
        );
    }
    assert_eq!(
        serde_json::from_value::<InstructionEnvelope>(encoded.clone()).expect("round trip"),
        envelope
    );

    for key in ["decisionDigest", "inputFingerprint"] {
        let mut stripped = encoded.clone();
        stripped
            .as_object_mut()
            .expect("object")
            .remove(key)
            .expect("binding present before removal");
        assert!(
            serde_json::from_value::<InstructionEnvelope>(stripped).is_err(),
            "an envelope missing {key} cannot show its authority, so it must fail closed"
        );
    }
}
