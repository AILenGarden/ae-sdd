use ae_sdd_contracts::execution::{
    EnvironmentRef, ExecutionStep, ExecutionStepError, VerificationExecutionPlan,
};
use ae_sdd_contracts::resource::{
    ContextBundleRef, DocumentTxnOperation, DocumentTxnPlan, LoadedContextProof,
    ResourceContractError,
};
use ae_sdd_contracts::review::{
    ReviewBudget, ReviewExitDisposition, ReviewExitReceipt, ReviewSession, ReviewStatus, ReviewTier,
};
use ae_sdd_contracts::{
    BoundedText, ContextBundleId, DocumentTxnId, ExecutionId, ExecutionStepId, MethodologyRef,
    MethodologyVariant, ReviewId, ReviewerRole, SchemaVersion, SeriesKind, SkillId, WorkerId,
};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, ContextDigest, EvidenceDigest, InputFingerprint,
    InventoryGeneration, ProjectRelativePath, StateRevision, WorkItemId,
};

fn artifact(kind: &str, path: &str, bytes: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new(kind).expect("artifact kind"),
        ProjectRelativePath::new(path).expect("relative path"),
        ArtifactDigest::digest(bytes),
        bytes.len() as u64,
    )
}

fn context_ref() -> ContextBundleRef {
    ContextBundleRef::from_artifacts(
        SchemaVersion::V1,
        ContextBundleId::new("ctx-001").expect("context id"),
        WorkItemId::new("STORY-001").expect("work item"),
        vec![
            artifact("story", "design/STORY-001.md", b"story"),
            artifact("constraints", "constraints/api.md", b"constraints"),
            artifact("thinking", "standards/thinking.md", b"thinking"),
            artifact("verification", "design/verification.md", b"verification"),
        ],
    )
    .expect("context ref")
}

fn methodology_ref() -> MethodologyRef {
    MethodologyRef::new(
        SchemaVersion::V1,
        SkillId::new("phase2-coding").unwrap(),
        SeriesKind::new("coding").unwrap(),
        MethodologyVariant::new("builtin-v1").unwrap(),
        artifact("methodology", "runtime/coding/compact.md", b"compact"),
        Some(artifact(
            "methodology",
            "runtime/coding/fallback.md",
            b"fallback",
        )),
        ArtifactDigest::digest(b"entry"),
        ArtifactDigest::digest(b"catalog"),
    )
    .unwrap()
}

#[test]
fn context_bundle_canonicalizes_artifacts_and_recomputes_identity() {
    let story = artifact("story", "design/STORY-001.md", b"story");
    let constraints = artifact("constraints", "constraints/api.md", b"constraints");
    let work_item = WorkItemId::new("STORY-001").unwrap();

    let first = ContextBundleRef::from_artifacts(
        SchemaVersion::V1,
        ContextBundleId::new("ctx-canonical").unwrap(),
        work_item.clone(),
        vec![story.clone(), constraints.clone()],
    )
    .unwrap();
    let reordered = ContextBundleRef::from_artifacts(
        SchemaVersion::V1,
        ContextBundleId::new("ctx-canonical").unwrap(),
        work_item,
        vec![constraints.clone(), story.clone()],
    )
    .unwrap();

    assert_eq!(first, reordered);
    assert_eq!(first.schema_version(), SchemaVersion::V1);
    assert_eq!(first.artifact_refs(), &[constraints, story]);
    assert_eq!(first.byte_length(), 16);
}

#[test]
fn context_bundle_rejects_caller_claims_that_do_not_match_canonical_content() {
    let artifacts = vec![
        artifact("story", "design/STORY-001.md", b"story"),
        artifact("constraints", "constraints/api.md", b"constraints"),
    ];
    let canonical = ContextBundleRef::from_artifacts(
        SchemaVersion::V1,
        ContextBundleId::new("ctx-claims").unwrap(),
        WorkItemId::new("STORY-001").unwrap(),
        artifacts.clone(),
    )
    .unwrap();

    assert_eq!(
        ContextBundleRef::new(
            SchemaVersion::V1,
            ContextBundleId::new("ctx-claims").unwrap(),
            WorkItemId::new("STORY-001").unwrap(),
            artifacts.clone(),
            canonical.bundle_digest(),
            canonical.byte_length(),
        )
        .unwrap(),
        canonical
    );
    assert_eq!(
        ContextBundleRef::new(
            SchemaVersion::V1,
            ContextBundleId::new("ctx-claims").unwrap(),
            WorkItemId::new("STORY-001").unwrap(),
            artifacts.clone(),
            ContextDigest::digest(b"forged"),
            canonical.byte_length(),
        )
        .unwrap(),
        canonical
    );
    assert_eq!(
        ContextBundleRef::new(
            SchemaVersion::V1,
            ContextBundleId::new("ctx-claims").unwrap(),
            WorkItemId::new("STORY-001").unwrap(),
            artifacts,
            canonical.bundle_digest(),
            canonical.byte_length() + 1,
        )
        .unwrap(),
        canonical
    );

    let forged_wire = serde_json::to_string(&canonical).unwrap().replace(
        &canonical.bundle_digest().to_string(),
        &ContextDigest::digest(b"forged wire").to_string(),
    );
    assert!(serde_json::from_str::<ContextBundleRef>(&forged_wire).is_err());
}

#[test]
fn loaded_context_proof_requires_all_four_contexts_and_exposes_freshness_inputs() {
    let story = artifact("story", "design/STORY-001.md", b"story");
    let constraints = artifact("constraints", "constraints/api.md", b"constraints");
    let thinking = artifact("thinking", "standards/thinking.md", b"thinking");
    let verification = artifact("verification", "design/verification.md", b"verification");
    let work_item = WorkItemId::new("STORY-001").unwrap();
    let context = ContextBundleRef::from_artifacts(
        SchemaVersion::V1,
        ContextBundleId::new("ctx-proof").unwrap(),
        work_item.clone(),
        vec![
            verification.clone(),
            story.clone(),
            thinking.clone(),
            constraints.clone(),
        ],
    )
    .unwrap();
    let methodology = methodology_ref();

    let proof = LoadedContextProof::new(
        SchemaVersion::V1,
        work_item.clone(),
        context,
        story.clone(),
        constraints.clone(),
        thinking.clone(),
        verification.clone(),
        methodology.clone(),
        StateRevision::new(7),
        InventoryGeneration::new(3),
        1_725_000_000_000,
    )
    .unwrap();

    assert_eq!(proof.schema_version(), SchemaVersion::V1);
    assert_eq!(proof.story_ref(), &story);
    assert_eq!(proof.constraints_ref(), &constraints);
    assert_eq!(proof.thinking_engine_ref(), &thinking);
    assert_eq!(proof.verification_ref(), &verification);
    assert_eq!(proof.methodology_ref(), &methodology);
    assert_eq!(proof.state_revision(), StateRevision::new(7));
    assert_eq!(proof.inventory_generation(), InventoryGeneration::new(3));
    assert_eq!(proof.computed_at_unix_ms(), 1_725_000_000_000);

    let incomplete = ContextBundleRef::from_artifacts(
        SchemaVersion::V1,
        ContextBundleId::new("ctx-incomplete").unwrap(),
        work_item.clone(),
        vec![story.clone()],
    )
    .unwrap();
    assert!(
        LoadedContextProof::new(
            SchemaVersion::V1,
            work_item,
            incomplete,
            story,
            constraints,
            thinking,
            verification,
            methodology,
            StateRevision::new(7),
            InventoryGeneration::new(3),
            1_725_000_000_000,
        )
        .is_err()
    );
}

#[test]
fn document_plan_binds_staged_content_cas_and_recomputed_plan_digest() {
    let target = ProjectRelativePath::new("design/STORY-001.md").unwrap();
    let staged = artifact(
        "staged-document",
        ".ae-sdd/staging/doc-txn-001/STORY-001.md",
        b"new story",
    );
    let expected_before = ArtifactDigest::digest(b"old story");
    let operation =
        DocumentTxnOperation::save_staged(target.clone(), staged.clone(), Some(expected_before))
            .unwrap();

    assert_eq!(operation.path(), &target);
    assert_eq!(operation.staged_content_ref(), Some(&staged));
    assert_eq!(operation.expected_before_digest(), Some(expected_before));

    let plan = DocumentTxnPlan::new(
        SchemaVersion::V1,
        DocumentTxnId::new("doc-txn-staged").unwrap(),
        WorkItemId::new("STORY-001").unwrap(),
        vec![operation],
        InputFingerprint::digest(b"document plan staged"),
    )
    .unwrap();
    let replay = DocumentTxnPlan::new(
        SchemaVersion::V1,
        DocumentTxnId::new("doc-txn-staged").unwrap(),
        WorkItemId::new("STORY-001").unwrap(),
        vec![DocumentTxnOperation::save_staged(target, staged, Some(expected_before)).unwrap()],
        InputFingerprint::digest(b"document plan staged"),
    )
    .unwrap();

    assert_eq!(plan, replay);
    assert_eq!(plan.schema_version(), SchemaVersion::V1);
    assert_eq!(plan.transaction_id().as_str(), "doc-txn-staged");
    assert_eq!(plan.work_item_id().as_str(), "STORY-001");
    assert_eq!(
        plan.input_fingerprint(),
        InputFingerprint::digest(b"document plan staged")
    );
    assert_eq!(plan.plan_digest(), replay.plan_digest());

    let json = serde_json::to_string(&plan).unwrap();
    assert!(json.contains("stagedContentRef"));
    assert!(json.contains("expectedBeforeDigest"));
    assert!(json.contains("planDigest"));
    let forged = json.replace(
        &plan.plan_digest().to_string(),
        &ArtifactDigest::digest(b"forged plan").to_string(),
    );
    assert!(serde_json::from_str::<DocumentTxnPlan>(&forged).is_err());
}

#[test]
fn resource_contracts_round_trip_and_reject_unknown_or_unbounded_input() {
    let context = context_ref();
    let json = serde_json::to_string(&context).expect("serialize context ref");
    assert!(json.contains("schemaVersion"));
    assert!(json.contains("contextId"));
    assert_eq!(
        serde_json::from_str::<ContextBundleRef>(&json).unwrap(),
        context
    );

    let unknown = json.replacen('{', "{\"unexpected\":true,", 1);
    assert!(serde_json::from_str::<ContextBundleRef>(&unknown).is_err());

    let too_many = (0..=ContextBundleRef::MAX_ARTIFACTS)
        .map(|_| artifact("story", "design/STORY-001.md", b"story"))
        .collect();
    assert!(
        ContextBundleRef::new(
            SchemaVersion::V1,
            ContextBundleId::new("ctx-002").unwrap(),
            WorkItemId::new("STORY-001").unwrap(),
            too_many,
            ContextDigest::digest(b"context"),
            1024,
        )
        .is_err()
    );
}

#[test]
fn loaded_context_proof_binds_required_inputs_and_document_plan_is_typed() {
    let context = context_ref();
    let story = artifact("story", "design/STORY-001.md", b"story");
    let proof = LoadedContextProof::new(
        SchemaVersion::V1,
        WorkItemId::new("STORY-001").unwrap(),
        context,
        story,
        artifact("constraints", "constraints/api.md", b"constraints"),
        artifact("thinking", "standards/thinking.md", b"thinking"),
        artifact("verification", "design/verification.md", b"verification"),
        methodology_ref(),
        StateRevision::new(7),
        InventoryGeneration::new(3),
        1_725_000_000_000,
    )
    .expect("proof");
    let proof_json = serde_json::to_string(&proof).unwrap();
    assert_eq!(
        serde_json::from_str::<LoadedContextProof>(&proof_json).unwrap(),
        proof
    );

    let plan = DocumentTxnPlan::new(
        SchemaVersion::V1,
        DocumentTxnId::new("doc-txn-001").unwrap(),
        WorkItemId::new("STORY-001").unwrap(),
        vec![
            DocumentTxnOperation::save(
                ProjectRelativePath::new("design/STORY-001.md").unwrap(),
                ArtifactDigest::digest(b"new story"),
                42,
            )
            .unwrap(),
        ],
        InputFingerprint::digest(b"document plan"),
    )
    .expect("document plan");
    let plan_json = serde_json::to_string(&plan).unwrap();
    assert_eq!(
        serde_json::from_str::<DocumentTxnPlan>(&plan_json).unwrap(),
        plan
    );
}

#[test]
fn document_plan_v2_delete_round_trips_and_rejects_v1() {
    let target = ProjectRelativePath::new("ae-sdd-doc/Story/STORY-001.md").unwrap();
    let draft = ProjectRelativePath::new(".hermes/draft-STORY-001.md").unwrap();
    let expected_draft = ArtifactDigest::digest(b"draft");
    let save = DocumentTxnOperation::save(
        target.clone(),
        ArtifactDigest::digest(b"saved story"),
        b"saved story".len() as u64,
    )
    .unwrap();
    let delete = DocumentTxnOperation::delete(draft.clone(), expected_draft);

    assert_eq!(
        DocumentTxnPlan::new(
            SchemaVersion::V1,
            DocumentTxnId::new("doc-txn-delete-v1").unwrap(),
            WorkItemId::new("STORY-001").unwrap(),
            vec![delete.clone()],
            InputFingerprint::digest(b"v1 delete"),
        ),
        Err(ResourceContractError::DeleteRequiresV2)
    );

    let plan = DocumentTxnPlan::new(
        SchemaVersion::V2,
        DocumentTxnId::new("doc-txn-delete-v2").unwrap(),
        WorkItemId::new("STORY-001").unwrap(),
        vec![save.clone(), delete.clone()],
        InputFingerprint::digest(b"v2 delete"),
    )
    .unwrap();
    let encoded = serde_json::to_value(&plan).unwrap();
    assert_eq!(encoded["operations"][1]["kind"], "delete");
    assert_eq!(encoded["operations"][1]["path"], draft.as_str());
    assert_eq!(
        encoded["operations"][1]["expectedBeforeDigest"],
        expected_draft.to_string()
    );
    assert_eq!(
        serde_json::from_value::<DocumentTxnPlan>(encoded).unwrap(),
        plan
    );

    let replay = DocumentTxnPlan::new(
        SchemaVersion::V2,
        DocumentTxnId::new("doc-txn-delete-v2").unwrap(),
        WorkItemId::new("STORY-001").unwrap(),
        vec![save.clone(), delete],
        InputFingerprint::digest(b"v2 delete"),
    )
    .unwrap();
    assert_eq!(replay.plan_digest(), plan.plan_digest());

    let changed_delete =
        DocumentTxnOperation::delete(draft.clone(), ArtifactDigest::digest(b"changed draft"));
    let changed = DocumentTxnPlan::new(
        SchemaVersion::V2,
        DocumentTxnId::new("doc-txn-delete-v2").unwrap(),
        WorkItemId::new("STORY-001").unwrap(),
        vec![save.clone(), changed_delete],
        InputFingerprint::digest(b"v2 delete"),
    )
    .unwrap();
    assert_ne!(changed.plan_digest(), plan.plan_digest());

    assert_eq!(
        DocumentTxnPlan::new(
            SchemaVersion::V2,
            DocumentTxnId::new("doc-txn-duplicate-v2").unwrap(),
            WorkItemId::new("STORY-001").unwrap(),
            vec![save, DocumentTxnOperation::delete(target, expected_draft)],
            InputFingerprint::digest(b"duplicate target"),
        ),
        Err(ResourceContractError::DuplicateOperationPath)
    );
}

#[test]
fn review_exit_cannot_pass_for_stalled_or_invalid_infrastructure() {
    let session = ReviewSession::new(
        SchemaVersion::V1,
        ReviewId::new("review-001").unwrap(),
        ReviewTier::Tier2,
        vec![ReviewerRole::new("security").unwrap()],
        InputFingerprint::digest(b"input"),
        InputFingerprint::digest(b"rules"),
        1,
        0,
        ReviewBudget::new(3, 32, 60_000).unwrap(),
    )
    .unwrap();
    assert_eq!(session.status(), ReviewStatus::Running);
    let session_json = serde_json::to_string(&session).unwrap();
    let unknown = session_json.replacen('{', "{\"unexpected\":true,", 1);
    assert!(serde_json::from_str::<ReviewSession>(&unknown).is_err());
    assert!(
        serde_json::from_str::<ReviewBudget>(
            r#"{"maxRounds":0,"maxFindings":32,"maxDurationMs":60000}"#
        )
        .is_err()
    );

    for status in [ReviewStatus::Stalled, ReviewStatus::InvalidInfra] {
        let receipt = ReviewExitReceipt::new(
            SchemaVersion::V1,
            &session,
            status,
            ReviewExitDisposition::Pass,
            InputFingerprint::digest(b"input"),
            vec![ReviewerRole::new("security").unwrap()],
            vec![],
        );
        assert!(receipt.is_err(), "{status:?} must not produce PASS");
    }

    assert!(
        ReviewExitReceipt::new(
            SchemaVersion::V1,
            &session,
            ReviewStatus::Completed,
            ReviewExitDisposition::Pass,
            InputFingerprint::digest(b"drifted input"),
            vec![ReviewerRole::new("security").unwrap()],
            vec![],
        )
        .is_err()
    );
    assert!(
        ReviewExitReceipt::new(
            SchemaVersion::V1,
            &session,
            ReviewStatus::Completed,
            ReviewExitDisposition::Pass,
            InputFingerprint::digest(b"input"),
            vec![],
            vec![],
        )
        .is_err()
    );
    let pass = ReviewExitReceipt::new(
        SchemaVersion::V1,
        &session,
        ReviewStatus::Completed,
        ReviewExitDisposition::Pass,
        InputFingerprint::digest(b"input"),
        vec![ReviewerRole::new("security").unwrap()],
        vec![],
    )
    .unwrap();
    assert!(pass.is_pass());
    assert_eq!(
        serde_json::from_str::<ReviewExitReceipt>(&serde_json::to_string(&pass).unwrap()).unwrap(),
        pass
    );
}

#[test]
fn execution_step_has_no_shell_or_secret_value_surface() {
    let step = ExecutionStep::new(
        SchemaVersion::V1,
        ExecutionStepId::new("step-001").unwrap(),
        artifact("program", "tools/cargo.exe", b"program"),
        vec![BoundedText::<256>::new("test").unwrap()],
        Some(ProjectRelativePath::new("workspace").unwrap()),
        vec![EnvironmentRef::new("CARGO_HOME").unwrap()],
    )
    .unwrap();
    let json = serde_json::to_string(&step).unwrap();
    assert!(json.contains("programRef"));
    assert!(json.contains("envRefs"));
    assert!(!json.contains("shell"));
    assert!(!json.contains("secret"));
    assert!(serde_json::from_str::<ExecutionStep>(&json).is_ok());
    let unknown = json.replacen('{', "{\"shell\":\"cmd /c cargo test\",", 1);
    assert!(serde_json::from_str::<ExecutionStep>(&unknown).is_err());
    assert!(EnvironmentRef::new("API_TOKEN=secret").is_err());

    let shell_like = json.replace("tools/cargo.exe", "cmd.exe /c cargo test");
    assert!(serde_json::from_str::<ExecutionStep>(&shell_like).is_err());
    let too_many_args = vec![BoundedText::<256>::new("x").unwrap(); ExecutionStep::MAX_ARGS + 1];
    assert!(matches!(
        ExecutionStep::new(
            SchemaVersion::V1,
            ExecutionStepId::new("step-002").unwrap(),
            artifact("program", "tools/cargo.exe", b"program"),
            too_many_args,
            None,
            vec![],
        ),
        Err(ExecutionStepError::CollectionLimitExceeded)
    ));
}

#[test]
fn verification_plan_is_bounded_and_receipt_requires_real_status() {
    let step = ExecutionStep::new(
        SchemaVersion::V1,
        ExecutionStepId::new("step-001").unwrap(),
        artifact("program", "tools/cargo.exe", b"program"),
        vec![],
        None,
        vec![],
    )
    .unwrap();
    let plan = VerificationExecutionPlan::new(
        SchemaVersion::V1,
        ExecutionId::new("exec-001").unwrap(),
        WorkItemId::new("STORY-001").unwrap(),
        InputFingerprint::digest(b"verification"),
        vec![step],
    )
    .unwrap();
    assert_eq!(
        serde_json::from_str::<VerificationExecutionPlan>(&serde_json::to_string(&plan).unwrap())
            .unwrap(),
        plan
    );

    let receipt = plan.receipt(
        WorkerId::new("worker-001").unwrap(),
        ae_sdd_protocol::JobStatus::Pass,
        Some(0),
        EvidenceDigest::digest(b"stdout"),
        EvidenceDigest::digest(b"stderr"),
        1_725_000_000_000,
        1_725_000_000_100,
        false,
        false,
    );
    assert!(receipt.is_ok());
    let receipt = receipt.unwrap();
    assert_eq!(
        serde_json::from_str::<ae_sdd_contracts::execution::VerificationReceipt>(
            &serde_json::to_string(&receipt).unwrap()
        )
        .unwrap(),
        receipt
    );
}
