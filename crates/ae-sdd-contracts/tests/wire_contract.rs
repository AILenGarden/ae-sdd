use ae_sdd_contracts::{
    MethodologyRef, MethodologyVariant, ReasonCode, RouteDecision, RouteDecisionId,
    RouteDisposition, SchemaVersion, SeriesKind, SkillId, SpecKind, TaskKind,
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, DecisionDigest, DesignRoute, InputFingerprint,
    ProjectRelativePath, WorkItemId, WorkScale,
};

fn artifact(path: &str, contents: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new("methodology-slice").expect("artifact kind"),
        ProjectRelativePath::new(path).expect("project-relative path"),
        ArtifactDigest::digest(contents),
        u64::try_from(contents.len()).expect("fixture length fits u64"),
    )
}

#[test]
fn methodology_reference_round_trips_and_rejects_unknown_fields() {
    let reference = MethodologyRef::new(
        SchemaVersion::V1,
        SkillId::new("phase1-design.requirement-analysis").expect("valid skill id"),
        SeriesKind::new("requirement-analysis").expect("series kind"),
        MethodologyVariant::new("builtin-v1").expect("variant"),
        artifact("runtime/skills/requirement-analysis/compact.md", b"compact"),
        Some(artifact(
            "runtime/skills/requirement-analysis/fallback.md",
            b"fallback",
        )),
        ArtifactDigest::digest(b"entry"),
        ArtifactDigest::digest(b"catalog"),
    )
    .expect("valid methodology reference");

    let json = serde_json::to_string(&reference).expect("serialize");
    let decoded: MethodologyRef = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, reference);

    let with_unknown = json.replacen('{', "{\"unexpected\":true,", 1);
    assert!(serde_json::from_str::<MethodologyRef>(&with_unknown).is_err());
    assert!(SkillId::new("../escape").is_err());
}

#[test]
fn route_decision_round_trips_using_domain_owned_scale_and_digests() {
    let decision = RouteDecision::new(
        SchemaVersion::V1,
        RouteDecisionId::new("route-STORY-001-r7").expect("route decision id"),
        WorkItemId::new("STORY-001").expect("work item id"),
        TaskKind::Implementation,
        WorkScale::Large,
        DesignRoute::Dr,
        RouteDisposition::AwaitUserApproval,
        vec![ReasonCode::new("route.low-confidence").expect("reason code")],
        vec![
            SeriesKind::new("requirement-analysis").expect("series kind"),
            SeriesKind::new("design-review").expect("series kind"),
        ],
        vec![SpecKind::RequirementAnalysis, SpecKind::DesignReview],
        InputFingerprint::digest(b"typed route facts"),
        None,
        DecisionDigest::digest(b"route decision"),
    )
    .expect("route decision");

    assert_eq!(decision.disposition(), RouteDisposition::AwaitUserApproval);
    assert!(!decision.is_approved());

    let json = serde_json::to_string(&decision).expect("serialize");
    let decoded: RouteDecision = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, decision);

    let micro = json.replace("\"large\"", "\"micro\"");
    let decoded_micro: RouteDecision = serde_json::from_str(&micro).expect("micro is supported");
    assert_eq!(decoded_micro.scale(), WorkScale::Micro);
}

/// §5.5 lines 265-272 name exactly six facts the daemon freezes once RA closes:
/// `taskKind`, `finalScale`, `selectedDesign`, `requiredSeries`,
/// `requiredSpecKinds`, and the route rationale plus decision digest. Two were
/// absent — `taskKind` and `requiredSpecKinds` — which left the authoritative task
/// kind existing nowhere after RA (the assessment carries only a *proposal*) and
/// made "which Specs must be bound" unanswerable from the decision.
#[test]
fn a_route_decision_carries_all_six_frozen_facts() {
    let decision = RouteDecision::new(
        SchemaVersion::V1,
        RouteDecisionId::new("route-STORY-006-r1").expect("route decision id"),
        WorkItemId::new("STORY-006").expect("work item id"),
        TaskKind::SelfUpdate,
        WorkScale::Micro,
        DesignRoute::CodingPlan,
        RouteDisposition::Approved,
        vec![ReasonCode::new("route.classified").expect("reason code")],
        vec![SeriesKind::new("requirement-analysis").expect("series kind")],
        vec![SpecKind::RequirementAnalysis],
        InputFingerprint::digest(b"six frozen facts"),
        None,
        DecisionDigest::digest(b"six frozen facts"),
    )
    .expect("route decision");

    let encoded = serde_json::to_value(&decision).expect("serialize");
    for key in [
        "taskKind",
        "scale",
        "designRoute",
        "requiredSeries",
        "requiredSpecKinds",
        "reasonCodes",
        "decisionDigest",
    ] {
        assert!(
            encoded.get(key).is_some(),
            "§5.5 freezes {key} on the decision: {encoded}"
        );
    }
    assert_eq!(decision.task_kind(), TaskKind::SelfUpdate);
    assert_eq!(
        decision.required_spec_kinds(),
        [SpecKind::RequirementAnalysis]
    );
    assert_eq!(
        serde_json::from_value::<RouteDecision>(encoded.clone()).expect("round trip"),
        decision
    );

    for key in ["taskKind", "requiredSpecKinds"] {
        let mut stripped = encoded.clone();
        stripped
            .as_object_mut()
            .expect("object")
            .remove(key)
            .expect("frozen fact present before removal");
        assert!(
            serde_json::from_value::<RouteDecision>(stripped).is_err(),
            "a decision missing {key} must fail closed, not decode with the fact absent"
        );
    }

    let no_specs = serde_json::json!({"requiredSpecKinds": []});
    let mut empty = encoded;
    empty.as_object_mut().expect("object").insert(
        "requiredSpecKinds".to_owned(),
        no_specs["requiredSpecKinds"].clone(),
    );
    assert!(
        serde_json::from_value::<RouteDecision>(empty).is_err(),
        "§5.4 makes RA mandatory at every scale, so zero required Specs must be refused"
    );
}

/// `requiredSeries` and `requiredSpecKinds` answer different questions, so the
/// contract must let them differ. §7.1 line 342 is the case that proves it: a
/// micro task runs Coding from an approved `executionPlan` while requiring no
/// standalone CodingPlan Spec, so a single list cannot express both facts.
#[test]
fn required_series_and_required_spec_kinds_are_independent() {
    let decision = RouteDecision::new(
        SchemaVersion::V1,
        RouteDecisionId::new("route-STORY-007-r1").expect("route decision id"),
        WorkItemId::new("STORY-007").expect("work item id"),
        TaskKind::Implementation,
        WorkScale::Medium,
        DesignRoute::Story,
        RouteDisposition::Approved,
        vec![ReasonCode::new("route.classified").expect("reason code")],
        vec![
            SeriesKind::new("requirement-analysis").expect("series kind"),
            SeriesKind::new("story").expect("series kind"),
        ],
        vec![
            SpecKind::RequirementAnalysis,
            SpecKind::Story,
            SpecKind::TestCase,
            SpecKind::CodingPlan,
        ],
        InputFingerprint::digest(b"independent lists"),
        None,
        DecisionDigest::digest(b"independent lists"),
    )
    .expect("the two lists need not be the same length or contents");
    assert_eq!(decision.required_series().len(), 2);
    assert_eq!(decision.required_spec_kinds().len(), 4);

    let round_tripped: RouteDecision =
        serde_json::from_value(serde_json::to_value(&decision).expect("serialize"))
            .expect("round trip");
    assert_eq!(round_tripped, decision);
}

/// The five `SpecKind` values are the §7.1 最低持久化设计产物 column. Their wire
/// spellings are frozen here so a rename cannot silently change the encoding on
/// one side, and `from_wire` must fail closed on anything else — a route that
/// required an unrecognised Spec kind would otherwise bind no document at all.
#[test]
fn spec_kind_wire_spellings_are_frozen_and_fail_closed() {
    for (kind, wire) in [
        (SpecKind::RequirementAnalysis, "requirement_analysis"),
        (SpecKind::DesignReview, "design_review"),
        (SpecKind::Story, "story"),
        (SpecKind::TestCase, "test_case"),
        (SpecKind::CodingPlan, "coding_plan"),
    ] {
        assert_eq!(kind.as_wire(), wire);
        assert_eq!(SpecKind::from_wire(wire), Some(kind));
        assert_eq!(
            serde_json::to_value(kind).expect("serialize"),
            serde_json::Value::String(wire.to_owned()),
            "the serde encoding and as_wire must not drift apart"
        );
        assert_eq!(
            serde_json::from_value::<SpecKind>(serde_json::json!(wire)).expect("deserialize"),
            kind
        );
    }
    for rejected in ["coding-plan", "codingPlan", "testcase", "requirement", ""] {
        assert_eq!(
            SpecKind::from_wire(rejected),
            None,
            "{rejected} is not frozen"
        );
        assert!(
            serde_json::from_value::<SpecKind>(serde_json::json!(rejected)).is_err(),
            "{rejected} must fail closed on the wire too"
        );
    }
}

/// `ae-sdd-daemon-design.md` §11.1 freezes the main-node vocabulary, and those
/// values *are* the logical `SeriesKind`s. Two spellings had drifted from it:
/// `dr` (a `DesignRoute` value, a different axis) and the
/// `{kind}-generate`/`-review`/`-update` split used by the methodology
/// catalog's `routePredicates`. Neither is a legal main node, so a route that
/// emitted them could never be matched against a frozen Series graph.
#[test]
fn main_node_vocabulary_is_frozen_to_logical_series_kinds() {
    for name in ae_sdd_contracts::MAIN_NODE_SERIES_KINDS {
        assert!(
            SeriesKind::new(name).is_ok(),
            "frozen main node must be a legal SeriesKind: {name}"
        );
    }
    assert_eq!(
        ae_sdd_contracts::MAIN_NODE_SERIES_KINDS,
        [
            "requirement-analysis",
            "design-review",
            "story",
            "testcase",
            "coding-plan",
            "coding",
            "test",
            "review",
        ],
        "the frozen order follows RA -> DR -> Story -> TestCase -> CodingPlan -> Coding -> Test -> Review"
    );
    for rejected in ["dr", "story-generate", "testcase-generate", "dr-review"] {
        assert!(
            !ae_sdd_contracts::MAIN_NODE_SERIES_KINDS.contains(&rejected),
            "{rejected} is not a main node: it is either another axis or a sub-node activity"
        );
    }
}

/// §11.1 splits the two axes: a main node is a logical Series, a sub-node is an
/// activity inside one Series. The two vocabularies must stay disjoint, because
/// the moment `draft` could be read as a Series or `story` as a sub-node, a
/// progress projection could not say which axis it was reporting on.
///
/// This is also what rules out the methodology catalog's old
/// `{kind}-generate`/`-review` *`seriesKind`* values. The drift was in that
/// field, not in `routePredicates`: every predicate value in the frozen catalog
/// is already a legal main node, while 29 of 31 `seriesKind` values were not.
#[test]
fn main_node_and_sub_node_vocabularies_are_disjoint_axes() {
    for sub_node in ae_sdd_contracts::SERIES_SUB_NODES {
        assert!(
            !ae_sdd_contracts::MAIN_NODE_SERIES_KINDS.contains(&sub_node),
            "{sub_node} is a Series-internal activity, never a Series identity"
        );
    }
    for main_node in ae_sdd_contracts::MAIN_NODE_SERIES_KINDS {
        assert!(
            !ae_sdd_contracts::SERIES_SUB_NODES.contains(&main_node),
            "{main_node} is a Series identity, never a sub-node"
        );
    }
    assert_eq!(
        ae_sdd_contracts::SERIES_SUB_NODES,
        [
            "resolve-spec",
            "collect-context",
            "draft",
            "validate",
            "await-user",
        ],
        "the traversal order is frozen by this contract, not by §11.1, which \
         lists the sub-nodes with \"例如\" and states no order"
    );
}

/// D-02 adds a third axis, so the disjointness obligation extends to it.
///
/// An activity names the skill *role* serving a Series. It must collide with
/// neither of the other two: if `review` could also be read as a main node, a
/// catalog entry for the `review` Series could not be told apart from the
/// review-role slice of some other Series.
///
/// `review` is the case that proves the axes are genuinely independent — it is a
/// legal main node *and* a legal activity, and those are different facts. The
/// pair carries the meaning: `(review, review)` is code review as its own
/// Series, `(story, review)` is reviewing a Story inside the `story` Series.
#[test]
fn activity_axis_is_disjoint_from_sub_nodes_and_pairs_with_main_nodes() {
    for activity in ae_sdd_contracts::SERIES_ACTIVITIES {
        assert!(
            !ae_sdd_contracts::SERIES_SUB_NODES.contains(&activity),
            "{activity} is a skill role, never a traversal position"
        );
    }
    assert_eq!(
        ae_sdd_contracts::SERIES_ACTIVITIES,
        ["generate", "review", "update", "fix", "execute"],
        "the frozen activity vocabulary"
    );
    assert!(
        ae_sdd_contracts::MAIN_NODE_SERIES_KINDS.contains(&"review")
            && ae_sdd_contracts::SERIES_ACTIVITIES.contains(&"review"),
        "`review` is deliberately both a Series identity and a skill role; \
         the (seriesKind, activity) pair is what disambiguates them"
    );
}

/// [`SeriesSubNode`] must stay in lockstep with [`SERIES_SUB_NODES`].
///
/// Two representations of one frozen vocabulary can drift: a value added to the
/// array without a variant leaves a sub-node no envelope can name, and a variant
/// added without an array entry leaves a position the traversal order omits.
/// Round-tripping every array entry through the enum catches both directions.
#[test]
fn sub_node_enum_and_array_cannot_drift_apart() {
    for wire in ae_sdd_contracts::SERIES_SUB_NODES {
        let parsed = ae_sdd_contracts::SeriesSubNode::from_wire(wire)
            .unwrap_or_else(|| panic!("{wire} is in the array but has no variant"));
        assert_eq!(
            parsed.as_wire(),
            wire,
            "the enum must round-trip the array's exact wire spelling"
        );
        assert_eq!(
            serde_json::to_value(parsed).expect("serialize"),
            serde_json::json!(wire),
            "serde must agree with as_wire, not invent its own casing"
        );
    }

    assert!(
        ae_sdd_contracts::SeriesSubNode::from_wire("draft-story").is_none(),
        "an unrecognised sub-node must fail closed"
    );

    for activity in ae_sdd_contracts::SERIES_ACTIVITIES {
        assert!(
            ae_sdd_contracts::SeriesSubNode::from_wire(activity).is_none(),
            "{activity} is an activity, and the two axes must not accept each \
             other's values"
        );
    }
}

/// §11.2's graph, frozen as a typed contract per its own closing instruction:
/// "具体状态枚举属于 typed contract；实现不得直接复制本图文本作为未版本化字符串
/// 状态机."
#[test]
fn the_series_lifecycle_spine_follows_the_documented_order() {
    use ae_sdd_contracts::SeriesLifecycleState as S;

    let spine = [
        S::Planned,
        S::AwaitingSpecBinding,
        S::Ready,
        S::SpawnRequested,
        S::Claimed,
        S::Running,
    ];
    for pair in spine.windows(2) {
        assert!(
            pair[0].can_advance_to(pair[1]),
            "{:?} -> {:?} is on §11.2's spine",
            pair[0],
            pair[1]
        );
    }

    // The tail resumes after the branch at line 592.
    assert!(S::Running.can_advance_to(S::ResultStaged));
    assert!(S::ResultStaged.can_advance_to(S::Validated));
    assert!(S::Validated.can_advance_to(S::Completed));

    // Skipping a spine step is not a legal single transition.
    assert!(
        !S::Planned.can_advance_to(S::Ready),
        "spec binding cannot be skipped; §8.1 decides it"
    );
    assert!(
        !S::Claimed.can_advance_to(S::ResultStaged),
        "a claimed Series must run before it can stage a result"
    );
}

/// Line 597 grants every non-terminal state an edge to the four failure
/// terminals, and `completed` must not be one of them.
#[test]
fn only_non_terminal_states_reach_the_failure_terminals() {
    use ae_sdd_contracts::SeriesLifecycleState as S;

    let non_terminal = [
        S::Planned,
        S::AwaitingSpecBinding,
        S::Ready,
        S::SpawnRequested,
        S::Claimed,
        S::Running,
        S::AwaitingUser,
        S::AwaitingGate,
        S::Retrying,
        S::ResultStaged,
        S::Validated,
    ];
    for state in non_terminal {
        assert!(!state.is_terminal(), "{state:?} is 非终态");
        for terminal in S::FAILURE_TERMINAL {
            assert!(
                state.can_advance_to(terminal),
                "§11.2 line 597 gives {state:?} an edge to {terminal:?}"
            );
        }
    }

    for terminal in S::FAILURE_TERMINAL {
        assert!(terminal.is_terminal());
        assert!(
            terminal.next_states().is_empty(),
            "{terminal:?} admits no further transition"
        );
    }

    assert!(
        S::Completed.is_terminal(),
        "§10 line 670 requires a 合法终态 to collect, so completed is terminal"
    );
    assert!(
        S::Completed.next_states().is_empty(),
        "a collected Series must not be able to later report failure"
    );
    assert!(
        !S::Completed.can_advance_to(S::Failed),
        "completed is not in FAILURE_TERMINAL precisely to prevent this edge"
    );
}

/// A retry mints a new `SeriesRunId` (§4.1), so `retrying` re-spawns rather than
/// resuming the attempt that failed.
#[test]
fn a_retry_respawns_instead_of_resuming_the_same_run() {
    use ae_sdd_contracts::SeriesLifecycleState as S;

    assert!(
        S::Retrying.can_advance_to(S::SpawnRequested),
        "§4.1: a retry produces a new physical run, which must be spawned"
    );
    assert!(
        !S::Retrying.can_advance_to(S::Running),
        "resuming would let one SeriesRunId cover two attempts, which is the \
         conflation §4.1 exists to prevent"
    );

    // A block on a live attempt does resume, because nothing new is spawned.
    assert!(S::AwaitingUser.can_advance_to(S::Running));
    assert!(S::AwaitingGate.can_advance_to(S::Running));
}

/// The wire encoding must round-trip and fail closed, matching the diagram's
/// snake_case spelling exactly.
#[test]
fn lifecycle_states_round_trip_their_documented_spelling() {
    use ae_sdd_contracts::SeriesLifecycleState as S;

    for (state, wire) in [
        (S::AwaitingSpecBinding, "awaiting_spec_binding"),
        (S::SpawnRequested, "spawn_requested"),
        (S::ResultStaged, "result_staged"),
        (S::AwaitingUser, "awaiting_user"),
        (S::Interrupted, "interrupted"),
    ] {
        assert_eq!(state.as_wire(), wire);
        assert_eq!(S::from_wire(wire), Some(state));
        assert_eq!(
            serde_json::to_value(state).expect("serialize"),
            serde_json::json!(wire),
            "serde must agree with as_wire rather than invent its own casing"
        );
    }

    assert!(
        S::from_wire("awaiting-spec-binding").is_none(),
        "§11.2 spells these with underscores; kebab-case must fail closed"
    );
    assert!(S::from_wire("done").is_none());
}

/// The conceptual graph and the durable receipt status are two enums for one
/// lifecycle, so the mapping between them must be total and must reach every
/// receipt variant. A `SeriesReceiptStatus` no conceptual state maps to would be
/// unreachable; a conceptual state with no mapping would be unreportable.
#[test]
fn every_lifecycle_state_projects_onto_a_reachable_receipt_status() {
    use ae_sdd_contracts::{SeriesLifecycleState as S, SeriesReceiptStatus as R};

    let all = [
        S::Planned,
        S::AwaitingSpecBinding,
        S::Ready,
        S::SpawnRequested,
        S::Claimed,
        S::Running,
        S::AwaitingUser,
        S::AwaitingGate,
        S::Retrying,
        S::ResultStaged,
        S::Validated,
        S::Completed,
        S::Failed,
        S::Cancelled,
        S::Stale,
        S::Interrupted,
    ];

    let mut reached = std::collections::BTreeSet::new();
    for state in all {
        reached.insert(format!("{:?}", state.to_receipt_status()));
    }

    for status in [
        R::Planned,
        R::Running,
        R::ResultStaged,
        R::Collected,
        R::Cancelled,
        R::Failed,
    ] {
        assert!(
            reached.contains(&format!("{status:?}")),
            "{status:?} is unreachable from any conceptual state, so no Series \
             could ever report it"
        );
    }

    // The lossy edges are asserted so a later change has to confront them.
    assert_eq!(S::Stale.to_receipt_status(), R::Cancelled);
    assert_eq!(S::Interrupted.to_receipt_status(), R::Cancelled);
    assert_eq!(
        S::Validated.to_receipt_status(),
        S::Completed.to_receipt_status(),
        "validation and collection are one durable outcome per §11.4"
    );

    // A blocked Series is still physically running, which is what stops a
    // supervisor from treating awaiting_user as a terminal stall.
    assert_eq!(S::AwaitingUser.to_receipt_status(), R::Running);
    assert_eq!(S::Retrying.to_receipt_status(), R::Running);
}
