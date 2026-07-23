use ae_sdd_contracts::execution::{
    EnvironmentRef, ExecutionStep, ExecutionStepError, VerificationExecutionPlan,
};
use ae_sdd_contracts::resource::{
    ContextBundleRef, DocumentTxnOperation, DocumentTxnPlan, LoadedContextProof,
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
    ContextBundleRef::new(
        SchemaVersion::V1,
        ContextBundleId::new("ctx-001").expect("context id"),
        WorkItemId::new("STORY-001").expect("work item"),
        vec![artifact("story", "design/STORY-001.md", b"story")],
        ContextDigest::digest(b"context"),
        64 * 1024,
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
