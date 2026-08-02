use ae_sdd_contracts::{
    ContextBundleId, IdempotencyKey, MethodologyRef, MethodologyResolution, MethodologyVariant,
    OverrideDisposition, OverrideLayer, OverrideTrace, ProcessSnapshot, ReasonCode, RetryPolicy,
    RouteDecision, RouteDecisionId, RouteDisposition, SchemaVersion, SeriesId, SeriesInput,
    SeriesKind, SeriesPlan, SeriesPlanDecision, SeriesReceipt, SeriesReceiptStatus, SkillId,
    SpecKind, TaskKind, resource::ContextBundleRef,
};
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, ArtifactKind, ArtifactRef, ContextDigest, DecisionDigest,
    DelegationId, DeliverableContract, DeliverableId, DeliverableRequirement, DesignRoute,
    EventSequence, InputFingerprint, OperationId, ProcessPhase, ProjectPathScope,
    ProjectRelativePath, ResultDigest, SeriesRunId, SessionId, StateRevision, WorkItemId,
    WorkScale,
};
use ae_sdd_flow::{
    ControlAction, ControlPlaneError, ControlPlaneRuntime, SeriesPlanner, SeriesPlannerError,
};

fn artifact(path: &str, contents: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new("series-fixture").expect("artifact kind"),
        ProjectRelativePath::new(path).expect("project-relative path"),
        ArtifactDigest::digest(contents),
        u64::try_from(contents.len()).expect("fixture length"),
    )
}

fn candidate(
    work_item_id: &WorkItemId,
    kind: &str,
    series_id: &str,
    state_revision: StateRevision,
    input_fingerprint: InputFingerprint,
) -> (SeriesPlan, MethodologyResolution) {
    let series_kind = SeriesKind::new(kind).expect("series kind");
    let skill_id = SkillId::new(format!("phase.{kind}")).expect("skill id");
    let methodology = MethodologyRef::new(
        SchemaVersion::V1,
        skill_id.clone(),
        series_kind.clone(),
        MethodologyVariant::new("builtin-v1").expect("variant"),
        artifact(&format!("runtime/skills/{kind}/compact.md"), b"compact"),
        None,
        ArtifactDigest::digest(format!("entry:{kind}")),
        ArtifactDigest::digest(b"catalog-v1"),
    )
    .expect("methodology ref");
    let plan = SeriesPlan::new(
        SchemaVersion::V1,
        SeriesId::new(series_id).expect("series id"),
        work_item_id.clone(),
        None,
        series_kind,
        AgentRole::Series,
        methodology.clone(),
        ContextBundleRef::new(
            SchemaVersion::V1,
            ContextBundleId::new(format!("context-{series_id}")).expect("context id"),
            work_item_id.clone(),
            vec![artifact("ae-sdd-doc/Story/STORY-FLOW-001.md", b"story")],
            ContextDigest::digest(format!("context:{kind}")),
            5,
        )
        .expect("context ref"),
        DeliverableContract::bounded_default([DeliverableRequirement::new(
            DeliverableId::new(format!("deliverable-{kind}")).expect("deliverable id"),
            ArtifactKind::new("document").expect("artifact kind"),
            ProjectRelativePath::new(format!("ae-sdd-doc/{kind}.md")).expect("deliverable path"),
        )])
        .expect("deliverable contract"),
        vec![OperationId::new("document.save").expect("operation")],
        vec![ProjectPathScope::Subtree(
            ProjectRelativePath::new("ae-sdd-doc").expect("path scope"),
        )],
        Vec::new(),
        state_revision,
        input_fingerprint,
        1_900_000_000_000,
        RetryPolicy::new(2, 250, 2_000).expect("retry policy"),
        DecisionDigest::digest(format!("plan:{kind}")),
    )
    .expect("series plan");
    let resolution = MethodologyResolution::new(
        SchemaVersion::V1,
        methodology,
        OverrideLayer::BuiltIn,
        vec![OverrideTrace::new(
            SchemaVersion::V1,
            OverrideLayer::BuiltIn,
            skill_id,
            OverrideDisposition::Selected,
            ReasonCode::new("methodology.builtin_selected").expect("reason"),
            ArtifactDigest::digest(format!("entry:{kind}")),
        )],
        DecisionDigest::digest(format!("resolution:{kind}")),
    )
    .expect("resolution");
    (plan, resolution)
}

/// A fixed physical attempt id for receipts under test.
///
/// §9.1 line 452 requires the Series transaction define `seriesId/seriesRunId/
/// workItemId`; these tests exercise one attempt, so a single constant keeps the
/// receipts comparable while still naming the attempt.
fn series_run_id() -> SeriesRunId {
    "00000000-0000-0000-0000-0000000000a1"
        .parse::<SeriesRunId>()
        .expect("series run id")
}

fn series_input() -> SeriesInput {
    series_input_with_receipts(Vec::new())
}

fn series_input_with_receipts(existing_receipts: Vec<SeriesReceipt>) -> SeriesInput {
    series_input_with_candidate_fingerprint(
        existing_receipts,
        InputFingerprint::digest(b"series input r7"),
    )
}

fn series_input_with_candidate_fingerprint(
    existing_receipts: Vec<SeriesReceipt>,
    candidate_fingerprint: InputFingerprint,
) -> SeriesInput {
    series_input_with_order(existing_receipts, candidate_fingerprint, false)
}

fn series_input_with_order(
    existing_receipts: Vec<SeriesReceipt>,
    candidate_fingerprint: InputFingerprint,
    reverse_candidates: bool,
) -> SeriesInput {
    let work_item_id = WorkItemId::new("STORY-FLOW-001").expect("work item");
    let state_revision = StateRevision::new(7);
    let input_fingerprint = InputFingerprint::digest(b"series input r7");
    let (requirement_plan, requirement_resolution) = candidate(
        &work_item_id,
        "requirement-analysis",
        "series-flow-ra-r7",
        state_revision,
        candidate_fingerprint,
    );
    let (story_plan, story_resolution) = candidate(
        &work_item_id,
        "story",
        "series-flow-story-r7",
        state_revision,
        candidate_fingerprint,
    );
    let mut resolutions = vec![requirement_resolution, story_resolution];
    let mut plans = vec![requirement_plan, story_plan];
    if reverse_candidates {
        resolutions.reverse();
        plans.reverse();
    }
    let route = RouteDecision::new(
        SchemaVersion::V1,
        RouteDecisionId::new("route-flow-r7").expect("route id"),
        work_item_id.clone(),
        TaskKind::Implementation,
        WorkScale::Medium,
        DesignRoute::Story,
        RouteDisposition::Approved,
        vec![ReasonCode::new("route.medium_story").expect("reason")],
        vec![
            SeriesKind::new("requirement-analysis").expect("series kind"),
            SeriesKind::new("story").expect("series kind"),
        ],
        vec![SpecKind::RequirementAnalysis, SpecKind::Story],
        InputFingerprint::digest(b"route input"),
        None,
        DecisionDigest::digest(b"route decision"),
    )
    .expect("route decision");
    SeriesInput::new(
        SchemaVersion::V1,
        route,
        ProcessSnapshot::new(
            SchemaVersion::V1,
            work_item_id,
            ProcessPhase::RequirementAnalyzed,
            None,
            state_revision,
            ArtifactDigest::digest(b"state r7"),
        ),
        existing_receipts,
        resolutions,
        plans,
        AgentRole::Root,
        state_revision,
        input_fingerprint,
        IdempotencyKey::new("series-next-flow-r7").expect("idempotency key"),
    )
    .expect("series input")
}

#[test]
fn first_missing_route_series_is_dispatched_deterministically() {
    let input = series_input();

    let first = SeriesPlanner::next(&input).expect("series decision");
    let replay = SeriesPlanner::next(&input).expect("series replay");

    assert_eq!(first, replay);
    match first {
        SeriesPlanDecision::RunSeries { plan, .. } => {
            assert_eq!(plan.series_kind().as_str(), "requirement-analysis");
            assert_eq!(plan.series_id().as_str(), "series-flow-ra-r7");
        }
        other => panic!("expected RunSeries, got {other:?}"),
    }
}

#[test]
fn running_series_is_awaited_without_duplicate_dispatch() {
    let baseline = series_input();
    let plan = &baseline.candidate_plans()[0];
    let receipt = SeriesReceipt::new(
        SchemaVersion::V1,
        plan.series_id().clone(),
        series_run_id(),
        plan.plan_digest(),
        SeriesReceiptStatus::Running,
        StateRevision::new(7),
        InputFingerprint::digest(b"series input r7"),
        None,
        None,
        Some(
            "00000000-0000-0000-0000-000000000101"
                .parse::<SessionId>()
                .expect("session id"),
        ),
        Some(
            "00000000-0000-0000-0000-000000000102"
                .parse::<DelegationId>()
                .expect("delegation id"),
        ),
        Some(EventSequence::new(10)),
        false,
        false,
        ResultDigest::digest(b"running receipt"),
    )
    .expect("running receipt");
    let input = series_input_with_receipts(vec![receipt]);

    let decision = SeriesPlanner::next(&input).expect("series decision");

    assert!(matches!(
        decision,
        SeriesPlanDecision::AwaitSeries { ref series_id, .. }
            if series_id.as_str() == "series-flow-ra-r7"
    ));
    let control = ControlPlaneRuntime::next(ArtifactDigest::digest(b"catalog-v1"), &input)
        .expect("running Series maps to a control decision");
    assert!(matches!(
        control.action(),
        ControlAction::AwaitSeries { series_id, .. }
            if series_id.as_str() == "series-flow-ra-r7"
    ));
}

#[test]
fn staged_series_without_validation_or_cleanup_is_not_collectable() {
    let baseline = series_input();
    let plan = &baseline.candidate_plans()[0];
    let result = artifact(".ae-sdd/results/series-flow-ra-r7.json", b"result");
    let receipt = SeriesReceipt::new(
        SchemaVersion::V1,
        plan.series_id().clone(),
        series_run_id(),
        plan.plan_digest(),
        SeriesReceiptStatus::ResultStaged,
        StateRevision::new(7),
        InputFingerprint::digest(b"series input r7"),
        Some(result),
        Some(ResultDigest::digest(b"result")),
        None,
        None,
        Some(EventSequence::new(11)),
        false,
        false,
        ResultDigest::digest(b"staged receipt"),
    )
    .expect("staged receipt");
    let input = series_input_with_receipts(vec![receipt]);

    let decision = SeriesPlanner::next(&input).expect("series decision");

    assert!(matches!(
        decision,
        SeriesPlanDecision::AwaitSeries { ref series_id, .. }
            if series_id.as_str() == "series-flow-ra-r7"
    ));
}

#[test]
fn staged_series_is_collectable_only_after_validation_and_cleanup() {
    let baseline = series_input();
    let plan = &baseline.candidate_plans()[0];
    let receipt = SeriesReceipt::new(
        SchemaVersion::V1,
        plan.series_id().clone(),
        series_run_id(),
        plan.plan_digest(),
        SeriesReceiptStatus::ResultStaged,
        StateRevision::new(7),
        InputFingerprint::digest(b"series input r7"),
        Some(artifact(
            ".ae-sdd/results/series-flow-ra-r7.json",
            b"result",
        )),
        Some(ResultDigest::digest(b"result")),
        None,
        None,
        Some(EventSequence::new(12)),
        true,
        true,
        ResultDigest::digest(b"collectable receipt"),
    )
    .expect("collectable receipt");
    let input = series_input_with_receipts(vec![receipt]);

    let decision = SeriesPlanner::next(&input).expect("series decision");

    assert!(matches!(
        decision,
        SeriesPlanDecision::CollectSeries { ref series_id, .. }
            if series_id.as_str() == "series-flow-ra-r7"
    ));
    let control = ControlPlaneRuntime::next(ArtifactDigest::digest(b"catalog-v1"), &input)
        .expect("collectable Series maps to a control decision");
    assert!(matches!(
        control.action(),
        ControlAction::CollectSeries { series_id, .. }
            if series_id.as_str() == "series-flow-ra-r7"
    ));
}

#[test]
fn collected_series_advances_to_the_next_route_candidate() {
    let baseline = series_input();
    let plan = &baseline.candidate_plans()[0];
    let receipt = SeriesReceipt::new(
        SchemaVersion::V1,
        plan.series_id().clone(),
        series_run_id(),
        plan.plan_digest(),
        SeriesReceiptStatus::Collected,
        StateRevision::new(7),
        InputFingerprint::digest(b"series input r7"),
        Some(artifact(
            ".ae-sdd/results/series-flow-ra-r7.json",
            b"result",
        )),
        Some(ResultDigest::digest(b"result")),
        None,
        None,
        Some(EventSequence::new(13)),
        true,
        true,
        ResultDigest::digest(b"collected receipt"),
    )
    .expect("collected receipt");
    let input = series_input_with_receipts(vec![receipt]);

    let decision = SeriesPlanner::next(&input).expect("series decision");

    match decision {
        SeriesPlanDecision::RunSeries { plan, .. } => {
            assert_eq!(plan.series_kind().as_str(), "story");
            assert_eq!(plan.series_id().as_str(), "series-flow-story-r7");
        }
        other => panic!("expected second RunSeries, got {other:?}"),
    }
}

#[test]
fn all_collected_series_complete_with_a_deterministic_projection_digest() {
    let baseline = series_input();
    let receipts = baseline
        .candidate_plans()
        .iter()
        .enumerate()
        .map(|(index, plan)| {
            SeriesReceipt::new(
                SchemaVersion::V1,
                plan.series_id().clone(),
                series_run_id(),
                plan.plan_digest(),
                SeriesReceiptStatus::Collected,
                StateRevision::new(7),
                InputFingerprint::digest(b"series input r7"),
                Some(artifact(
                    &format!(".ae-sdd/results/{}.json", plan.series_id().as_str()),
                    b"result",
                )),
                Some(ResultDigest::digest(format!("result:{index}"))),
                None,
                None,
                Some(EventSequence::new(
                    u64::try_from(20 + index).expect("event sequence"),
                )),
                true,
                true,
                ResultDigest::digest(format!("collected:{index}")),
            )
            .expect("collected receipt")
        })
        .collect();
    let input = series_input_with_receipts(receipts);

    let first = SeriesPlanner::next(&input).expect("complete decision");
    let replay = SeriesPlanner::next(&input).expect("complete replay");

    assert_eq!(first, replay);
    assert!(matches!(first, SeriesPlanDecision::Complete { .. }));
    let control = ControlPlaneRuntime::next(ArtifactDigest::digest(b"catalog-v1"), &input)
        .expect("complete Series projection maps to a control decision");
    assert!(matches!(control.action(), ControlAction::Complete { .. }));
}

#[test]
fn unapproved_route_never_dispatches_a_series() {
    let work_item_id = WorkItemId::new("STORY-FLOW-AWAIT").expect("work item");
    let route = RouteDecision::new(
        SchemaVersion::V1,
        RouteDecisionId::new("route-flow-await").expect("route id"),
        work_item_id.clone(),
        TaskKind::Implementation,
        WorkScale::Large,
        DesignRoute::Dr,
        RouteDisposition::AwaitUserApproval,
        vec![ReasonCode::new("route.approval_required").expect("reason")],
        vec![SeriesKind::new("requirement-analysis").expect("series kind")],
        vec![SpecKind::RequirementAnalysis],
        InputFingerprint::digest(b"route awaiting approval"),
        None,
        DecisionDigest::digest(b"route awaiting approval"),
    )
    .expect("route decision");
    let input = SeriesInput::new(
        SchemaVersion::V1,
        route,
        ProcessSnapshot::new(
            SchemaVersion::V1,
            work_item_id,
            ProcessPhase::RequirementAnalyzed,
            None,
            StateRevision::new(8),
            ArtifactDigest::digest(b"state r8"),
        ),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        AgentRole::Root,
        StateRevision::new(8),
        InputFingerprint::digest(b"series awaiting approval"),
        IdempotencyKey::new("series-await-route").expect("idempotency key"),
    )
    .expect("series input");

    let decision = SeriesPlanner::next(&input).expect("series decision");

    assert!(matches!(
        decision,
        SeriesPlanDecision::AwaitRouteApproval { ref decision_id, .. }
            if decision_id.as_str() == "route-flow-await"
    ));
    let control = ControlPlaneRuntime::next(ArtifactDigest::digest(b"catalog-v1"), &input)
        .expect("unapproved route maps to a control decision");
    assert!(matches!(
        control.action(),
        ControlAction::AwaitRouteApproval { decision_id, .. }
            if decision_id.as_str() == "route-flow-await"
    ));
}

#[test]
fn orphan_receipt_is_rejected_instead_of_ignored() {
    let receipt = SeriesReceipt::new(
        SchemaVersion::V1,
        SeriesId::new("series-orphan-r7").expect("series id"),
        series_run_id(),
        DecisionDigest::digest(b"orphan plan"),
        SeriesReceiptStatus::Running,
        StateRevision::new(7),
        InputFingerprint::digest(b"series input r7"),
        None,
        None,
        None,
        None,
        Some(EventSequence::new(30)),
        false,
        false,
        ResultDigest::digest(b"orphan receipt"),
    )
    .expect("orphan receipt fixture");
    let input = series_input_with_receipts(vec![receipt]);

    let error = SeriesPlanner::next(&input).expect_err("orphan must fail closed");

    assert_eq!(error, SeriesPlannerError::OrphanReceipt);
}

#[test]
fn receipt_reusing_a_series_id_for_another_plan_is_rejected() {
    let baseline = series_input();
    let plan = &baseline.candidate_plans()[0];
    let receipt = SeriesReceipt::new(
        SchemaVersion::V1,
        plan.series_id().clone(),
        series_run_id(),
        DecisionDigest::digest(b"different plan"),
        SeriesReceiptStatus::Running,
        StateRevision::new(7),
        InputFingerprint::digest(b"series input r7"),
        None,
        None,
        None,
        None,
        Some(EventSequence::new(31)),
        false,
        false,
        ResultDigest::digest(b"conflicting receipt"),
    )
    .expect("conflicting receipt fixture");
    let input = series_input_with_receipts(vec![receipt]);

    let error = SeriesPlanner::next(&input).expect_err("plan conflict must fail closed");

    assert_eq!(error, SeriesPlannerError::ReceiptPlanConflict);
}

#[test]
fn receipt_from_another_state_revision_is_rejected_as_stale() {
    let baseline = series_input();
    let plan = &baseline.candidate_plans()[0];
    let receipt = SeriesReceipt::new(
        SchemaVersion::V1,
        plan.series_id().clone(),
        series_run_id(),
        plan.plan_digest(),
        SeriesReceiptStatus::Running,
        StateRevision::new(6),
        InputFingerprint::digest(b"series input r7"),
        None,
        None,
        None,
        None,
        Some(EventSequence::new(32)),
        false,
        false,
        ResultDigest::digest(b"stale revision receipt"),
    )
    .expect("stale receipt fixture");
    let input = series_input_with_receipts(vec![receipt]);

    let error = SeriesPlanner::next(&input).expect_err("stale receipt must fail closed");

    assert_eq!(error, SeriesPlannerError::StaleReceipt);
}

#[test]
fn receipt_from_another_input_fingerprint_is_rejected_as_stale() {
    let baseline = series_input();
    let plan = &baseline.candidate_plans()[0];
    let receipt = SeriesReceipt::new(
        SchemaVersion::V1,
        plan.series_id().clone(),
        series_run_id(),
        plan.plan_digest(),
        SeriesReceiptStatus::Running,
        StateRevision::new(7),
        InputFingerprint::digest(b"different series input"),
        None,
        None,
        None,
        None,
        Some(EventSequence::new(33)),
        false,
        false,
        ResultDigest::digest(b"stale fingerprint receipt"),
    )
    .expect("stale receipt fixture");
    let input = series_input_with_receipts(vec![receipt]);

    let error = SeriesPlanner::next(&input).expect_err("stale receipt must fail closed");

    assert_eq!(error, SeriesPlannerError::StaleReceipt);
}

#[test]
fn control_runtime_binds_catalog_route_and_series_provenance() {
    let input = series_input();
    let catalog_digest = ArtifactDigest::digest(b"catalog-v1");

    let first = ControlPlaneRuntime::next(catalog_digest, &input).expect("control decision");
    let replay = ControlPlaneRuntime::next(catalog_digest, &input).expect("control replay");

    assert_eq!(first, replay);
    assert_eq!(first.provenance().catalog_digest(), catalog_digest);
    assert_eq!(
        first.provenance().route_digest(),
        input.route().decision_digest()
    );
    assert_ne!(
        first.provenance().series_digest(),
        DecisionDigest::from_array([0; 32])
    );
    assert!(matches!(
        first.action(),
        ControlAction::RunSeries { plan, .. }
            if plan.series_id().as_str() == "series-flow-ra-r7"
    ));
    assert_ne!(first.decision_digest(), DecisionDigest::from_array([0; 32]));
}

#[test]
fn control_runtime_rejects_a_different_catalog_snapshot() {
    let input = series_input();

    let error = ControlPlaneRuntime::next(ArtifactDigest::digest(b"other catalog"), &input)
        .expect_err("catalog mismatch must fail closed");

    assert_eq!(error, ControlPlaneError::CatalogDigestMismatch);
}

#[test]
fn cancelled_required_series_is_a_terminal_planner_error() {
    let baseline = series_input();
    let plan = &baseline.candidate_plans()[0];
    let receipt = SeriesReceipt::new(
        SchemaVersion::V1,
        plan.series_id().clone(),
        series_run_id(),
        plan.plan_digest(),
        SeriesReceiptStatus::Cancelled,
        StateRevision::new(7),
        InputFingerprint::digest(b"series input r7"),
        None,
        None,
        None,
        None,
        Some(EventSequence::new(34)),
        false,
        false,
        ResultDigest::digest(b"cancelled receipt"),
    )
    .expect("cancelled receipt");
    let input = series_input_with_receipts(vec![receipt]);

    let error = SeriesPlanner::next(&input).expect_err("cancelled Series is terminal");

    assert_eq!(error, SeriesPlannerError::TerminalReceipt);
}

#[test]
fn candidate_plan_from_another_input_fingerprint_is_rejected() {
    let input = series_input_with_candidate_fingerprint(
        Vec::new(),
        InputFingerprint::digest(b"stale candidate input"),
    );

    let error = SeriesPlanner::next(&input).expect_err("stale plan must fail closed");

    assert_eq!(error, SeriesPlannerError::StalePlan);
}

#[test]
fn downstream_receipt_plan_conflict_is_rejected_before_dispatch() {
    let baseline = series_input();
    let downstream = &baseline.candidate_plans()[1];
    let receipt = SeriesReceipt::new(
        SchemaVersion::V1,
        downstream.series_id().clone(),
        series_run_id(),
        DecisionDigest::digest(b"wrong downstream plan"),
        SeriesReceiptStatus::Running,
        StateRevision::new(7),
        InputFingerprint::digest(b"series input r7"),
        None,
        None,
        None,
        None,
        Some(EventSequence::new(35)),
        false,
        false,
        ResultDigest::digest(b"downstream conflict"),
    )
    .expect("downstream receipt");
    let input = series_input_with_receipts(vec![receipt]);

    let error = SeriesPlanner::next(&input).expect_err("all receipt bindings are validated first");

    assert_eq!(error, SeriesPlannerError::ReceiptPlanConflict);
}

#[test]
fn downstream_terminal_receipt_halts_before_new_dispatch() {
    let baseline = series_input();
    let downstream = &baseline.candidate_plans()[1];
    let receipt = SeriesReceipt::new(
        SchemaVersion::V1,
        downstream.series_id().clone(),
        series_run_id(),
        downstream.plan_digest(),
        SeriesReceiptStatus::Failed,
        StateRevision::new(7),
        InputFingerprint::digest(b"series input r7"),
        None,
        None,
        None,
        None,
        Some(EventSequence::new(36)),
        false,
        false,
        ResultDigest::digest(b"downstream failed"),
    )
    .expect("downstream terminal receipt");
    let input = series_input_with_receipts(vec![receipt]);

    let error = SeriesPlanner::next(&input).expect_err("terminal receipt halts planning");

    assert_eq!(error, SeriesPlannerError::TerminalReceipt);
}

#[test]
fn complete_projection_digest_ignores_candidate_and_receipt_permutation() {
    let baseline = series_input();
    let receipts = baseline
        .candidate_plans()
        .iter()
        .enumerate()
        .map(|(index, plan)| {
            SeriesReceipt::new(
                SchemaVersion::V1,
                plan.series_id().clone(),
                series_run_id(),
                plan.plan_digest(),
                SeriesReceiptStatus::Collected,
                StateRevision::new(7),
                InputFingerprint::digest(b"series input r7"),
                Some(artifact(
                    &format!(".ae-sdd/results/{}.json", plan.series_id().as_str()),
                    b"result",
                )),
                Some(ResultDigest::digest(format!("permutation-result:{index}"))),
                None,
                None,
                Some(EventSequence::new(
                    u64::try_from(40 + index).expect("event sequence"),
                )),
                true,
                true,
                ResultDigest::digest(format!("permutation-receipt:{index}")),
            )
            .expect("collected receipt")
        })
        .collect::<Vec<_>>();
    let forward = series_input_with_order(
        receipts.clone(),
        InputFingerprint::digest(b"series input r7"),
        false,
    );
    let mut reversed_receipts = receipts;
    reversed_receipts.reverse();
    let reversed = series_input_with_order(
        reversed_receipts,
        InputFingerprint::digest(b"series input r7"),
        true,
    );

    let forward_decision = SeriesPlanner::next(&forward).expect("forward complete");
    let reversed_decision = SeriesPlanner::next(&reversed).expect("reversed complete");

    assert_eq!(forward_decision, reversed_decision);
}

#[test]
fn control_provenance_ignores_candidate_and_resolution_permutation() {
    let forward = series_input_with_order(
        Vec::new(),
        InputFingerprint::digest(b"series input r7"),
        false,
    );
    let reversed = series_input_with_order(
        Vec::new(),
        InputFingerprint::digest(b"series input r7"),
        true,
    );
    let catalog_digest = ArtifactDigest::digest(b"catalog-v1");

    let forward_decision =
        ControlPlaneRuntime::next(catalog_digest, &forward).expect("forward control");
    let reversed_decision =
        ControlPlaneRuntime::next(catalog_digest, &reversed).expect("reversed control");

    assert_eq!(forward_decision, reversed_decision);
    assert_eq!(
        forward_decision.provenance().series_digest(),
        reversed_decision.provenance().series_digest()
    );
}

#[test]
fn planner_and_control_errors_have_stable_nonempty_diagnostics() {
    for error in [
        SeriesPlannerError::ContractEncoding,
        SeriesPlannerError::MissingRequiredSeries,
        SeriesPlannerError::MissingCandidate,
        SeriesPlannerError::ReceiptPlanConflict,
        SeriesPlannerError::OrphanReceipt,
        SeriesPlannerError::StaleReceipt,
        SeriesPlannerError::StalePlan,
        SeriesPlannerError::TerminalReceipt,
    ] {
        assert!(!error.to_string().is_empty());
    }
    for error in [
        ControlPlaneError::CatalogDigestMismatch,
        ControlPlaneError::ContractEncoding,
        ControlPlaneError::Series(SeriesPlannerError::MissingCandidate),
    ] {
        assert!(!error.to_string().is_empty());
    }
}
