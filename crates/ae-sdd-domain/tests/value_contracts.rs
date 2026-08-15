use std::str::FromStr;

use ae_sdd_domain::{
    AgentLineage, AgentRole, ArtifactDigest, ArtifactKind, ArtifactRef, BootId, CancellationCode,
    CapabilityId, ConfigDigest, DelegationId, DeliverableContract, DeliverableContractError,
    DeliverableId, DeliverableRequirement, ErrorCode, EvidenceDigest, EvidenceId, EvidenceRef,
    FencingToken, FindingCode, FlowRunId, GateCancellation, GateError, GateFailure, GateFinding,
    GateFreshness, GateId, GateImplementationDigest, GateKey, GateOutcome, GateResult, GateTimeout,
    GrantViolation, InputFingerprint, InventoryGeneration, OperationId, PolicyDigest, ProjectKey,
    ProjectPathScope, ProjectRelativePath, ProjectRelativePathError, RequestId, ScopedGrant,
    SeriesRunId, SessionId, StateRevision, StoryId, ToolchainDigest, VerificationId, WorkItemId,
    WorkspaceId,
};
use uuid::Uuid;

fn path(value: &str) -> ProjectRelativePath {
    ProjectRelativePath::new(value).expect("test path is valid")
}

fn evidence() -> EvidenceRef {
    EvidenceRef::new(
        EvidenceId::new("evidence-1").expect("valid evidence ID"),
        VerificationId::new("V-021").expect("valid verification ID"),
        path("evidence/v-021.json"),
        EvidenceDigest::digest(b"coverage evidence"),
        21,
    )
}

fn gate_key() -> GateKey {
    GateKey::new(
        GateId::new("G-14").expect("valid gate ID"),
        GateImplementationDigest::digest(b"gate-v1"),
        PolicyDigest::digest(b"policy-v1"),
        WorkspaceId::from_uuid(Uuid::from_u128(1)),
        WorkItemId::new("PRD-AE-SDD-RUST-DAEMON-001").expect("valid work item ID"),
        Some(StoryId::new("STORY-AE-SDD-C1-INTEGRATION-001").expect("valid story ID")),
        StateRevision::new(7),
        FencingToken::new(3),
        InventoryGeneration::new(5),
        ToolchainDigest::digest(b"rustc-1.97.1"),
        ConfigDigest::digest(b"config-v1"),
        InputFingerprint::digest(b"input-v1"),
    )
}

#[test]
fn artifact_evidence_and_gate_value_objects_preserve_every_field() {
    let artifact = ArtifactRef::new(
        ArtifactKind::new("rust-source").expect("valid artifact kind"),
        path("crates/ae-sdd-domain/src/lib.rs"),
        ArtifactDigest::digest(b"artifact"),
        42,
    );
    assert_eq!(artifact.kind().as_str(), "rust-source");
    assert_eq!(artifact.path().as_str(), "crates/ae-sdd-domain/src/lib.rs");
    assert_eq!(artifact.digest(), ArtifactDigest::digest(b"artifact"));
    assert_eq!(artifact.byte_length(), 42);

    let evidence = evidence();
    assert_eq!(evidence.evidence_id().as_str(), "evidence-1");
    assert_eq!(evidence.verification_id().as_str(), "V-021");
    assert_eq!(evidence.path().as_str(), "evidence/v-021.json");
    assert_eq!(
        evidence.digest(),
        EvidenceDigest::digest(b"coverage evidence")
    );
    assert_eq!(evidence.byte_length(), 21);

    let finding = GateFinding::new(
        FindingCode::new("COVERAGE_LOW").expect("valid finding code"),
        [evidence.clone()],
    );
    assert_eq!(finding.code().as_str(), "COVERAGE_LOW");
    assert_eq!(finding.evidence(), [evidence]);

    let failure = GateFailure::new([finding.clone()]).expect("non-empty failure");
    assert_eq!(failure.findings(), [finding]);

    let gate_error = GateError::new(
        ErrorCode::new("COVERAGE_TOOL_FAILED").expect("valid error code"),
        true,
    );
    assert_eq!(gate_error.code().as_str(), "COVERAGE_TOOL_FAILED");
    assert!(gate_error.retryable());

    let timeout = GateTimeout::new(250).expect("positive timeout");
    assert_eq!(timeout.deadline_ms(), 250);
    let cancellation = GateCancellation::new(
        CancellationCode::new("CALLER_CANCELLED").expect("valid cancellation code"),
    );
    assert_eq!(cancellation.reason().as_str(), "CALLER_CANCELLED");

    let key = gate_key();
    assert_eq!(key.gate_id().as_str(), "G-14");
    assert_eq!(
        key.gate_implementation(),
        GateImplementationDigest::digest(b"gate-v1")
    );
    assert_eq!(key.policy(), PolicyDigest::digest(b"policy-v1"));
    assert_eq!(
        key.workspace_id(),
        WorkspaceId::from_uuid(Uuid::from_u128(1))
    );
    assert_eq!(key.work_item_id().as_str(), "PRD-AE-SDD-RUST-DAEMON-001");
    assert_eq!(
        key.story_id().map(StoryId::as_str),
        Some("STORY-AE-SDD-C1-INTEGRATION-001")
    );
    assert_eq!(key.state_revision(), StateRevision::new(7));
    assert_eq!(key.fencing_token(), FencingToken::new(3));
    assert_eq!(key.inventory_generation(), InventoryGeneration::new(5));
    assert_eq!(key.toolchain(), ToolchainDigest::digest(b"rustc-1.97.1"));
    assert_eq!(key.configuration(), ConfigDigest::digest(b"config-v1"));
    assert_eq!(key.input(), InputFingerprint::digest(b"input-v1"));

    let result = GateResult::new(key.clone(), GateOutcome::Pass);
    assert_eq!(result.key(), &key);
    assert_eq!(result.outcome(), &GateOutcome::Pass);
    assert_eq!(key.freshness_against(&key), GateFreshness::Fresh);
}

#[test]
fn lineage_grants_and_deliverables_reject_every_widening_or_duplicate() {
    let root_session = SessionId::from_uuid(Uuid::from_u128(10));
    let series_session = SessionId::from_uuid(Uuid::from_u128(11));
    let task_session = SessionId::from_uuid(Uuid::from_u128(12));
    let series_delegation = DelegationId::from_uuid(Uuid::from_u128(20));
    let task_delegation = DelegationId::from_uuid(Uuid::from_u128(21));
    let root = AgentLineage::root(root_session);
    let series = root
        .spawn_child(series_session, series_delegation, AgentRole::Series)
        .expect("root may create a series");
    let task = series
        .spawn_child(task_session, task_delegation, AgentRole::Task)
        .expect("series may create a task");

    assert_eq!(task.root_identity().session_id(), root_session);
    assert_eq!(task.nodes().len(), 3);
    assert_eq!(task.nodes()[0].identity().role(), AgentRole::Root);
    assert_eq!(task.nodes()[0].via_delegation(), None);
    assert_eq!(task.nodes()[1].via_delegation(), Some(series_delegation));
    assert!(
        root.spawn_child(
            root_session,
            DelegationId::from_uuid(Uuid::from_u128(22)),
            AgentRole::Series
        )
        .is_err()
    );
    assert!(
        series
            .spawn_child(
                SessionId::from_uuid(Uuid::from_u128(13)),
                series_delegation,
                AgentRole::Task,
            )
            .is_err()
    );

    let operation = OperationId::new("artifact.read").expect("valid operation");
    let capability = CapabilityId::new("host.report").expect("valid capability");
    let parent = ScopedGrant::new(
        [operation.clone()],
        [capability.clone()],
        [ProjectPathScope::ProjectRoot],
    );
    assert_eq!(
        parent.operations(),
        &std::collections::BTreeSet::from([operation.clone()])
    );
    assert_eq!(
        parent.capabilities(),
        &std::collections::BTreeSet::from([capability.clone()])
    );
    assert_eq!(
        parent.paths(),
        &std::collections::BTreeSet::from([ProjectPathScope::ProjectRoot])
    );
    assert!(ProjectPathScope::ProjectRoot.contains_path(&path("any/path")));
    assert!(
        !ProjectPathScope::Subtree(path("crates/domain")).contains(&ProjectPathScope::ProjectRoot)
    );

    let capability_widening = ScopedGrant::new(
        [operation.clone()],
        [CapabilityId::new("host.admin").expect("valid capability")],
        [ProjectPathScope::ProjectRoot],
    );
    assert!(matches!(
        parent.validate_child(&capability_widening),
        Err(GrantViolation::CapabilityNotGranted(_))
    ));

    let first = DeliverableRequirement::new(
        DeliverableId::new("source").expect("valid deliverable ID"),
        ArtifactKind::new("rust-source").expect("valid artifact kind"),
        path("src/lib.rs"),
    );
    assert_eq!(first.id().as_str(), "source");
    assert_eq!(first.kind().as_str(), "rust-source");
    assert_eq!(first.path().as_str(), "src/lib.rs");
    assert_eq!(
        DeliverableContract::new([first.clone()], 0, 0),
        Err(DeliverableContractError::ZeroResultBudget)
    );
    assert_eq!(
        DeliverableContract::new([first.clone()], 10, 0),
        Err(DeliverableContractError::InvalidSummaryBudget {
            summary: 0,
            result: 10,
        })
    );
    assert_eq!(
        DeliverableContract::new([first.clone()], 10, 11),
        Err(DeliverableContractError::InvalidSummaryBudget {
            summary: 11,
            result: 10,
        })
    );

    let duplicate_id = DeliverableRequirement::new(
        first.id().clone(),
        ArtifactKind::new("evidence").expect("valid artifact kind"),
        path("evidence.json"),
    );
    assert!(matches!(
        DeliverableContract::new([first.clone(), duplicate_id], 10, 5),
        Err(DeliverableContractError::DuplicateId(_))
    ));
    let duplicate_path = DeliverableRequirement::new(
        DeliverableId::new("evidence").expect("valid deliverable ID"),
        ArtifactKind::new("evidence").expect("valid artifact kind"),
        first.path().clone(),
    );
    assert!(matches!(
        DeliverableContract::new([first.clone(), duplicate_path], 10, 5),
        Err(DeliverableContractError::DuplicatePath(_))
    ));
    let contract = DeliverableContract::new([first], 10, 5).expect("valid contract");
    assert_eq!(contract.required().len(), 1);
}

#[test]
fn identifiers_digests_counters_and_paths_cover_conversion_boundaries() {
    let uuid = Uuid::from_u128(99);
    let boot = BootId::from(uuid);
    assert_eq!(boot.as_uuid(), &uuid);
    assert_eq!(boot.into_uuid(), uuid);
    let request = RequestId::from_str(&uuid.to_string()).expect("UUID parses");
    assert_eq!(request.to_string(), uuid.to_string());

    let project = ProjectKey::new("ae-sdd").expect("valid project key");
    assert_eq!(project.as_str(), "ae-sdd");
    assert_eq!(project.to_string(), "ae-sdd");
    assert_eq!(ProjectKey::from_str("ae-sdd").unwrap(), project);
    assert_eq!(ProjectKey::try_from("ae-sdd"), Ok(project.clone()));
    assert_eq!(ProjectKey::try_from("ae-sdd".to_owned()), Ok(project));
    assert!(ProjectKey::new("").is_err());
    assert!(ProjectKey::new("x".repeat(ProjectKey::MAX_BYTES + 1)).is_err());
    assert!(ProjectKey::new("-invalid").is_err());
    assert!(ProjectKey::new("invalid/value").is_err());

    let digest = ArtifactDigest::digest(b"digest");
    assert_eq!(digest.as_bytes(), &digest.into_array());
    assert_eq!(digest.to_hex(), digest.to_string());

    let revision = StateRevision::from(7);
    assert_eq!(revision.get(), 7);
    assert_eq!(u64::from(revision), 7);
    assert_eq!(revision.checked_next(), Ok(StateRevision::new(8)));

    let parsed = ProjectRelativePath::from_str("src/lib.rs").expect("path parses");
    assert_eq!(parsed.to_string(), "src/lib.rs");
    assert_eq!(
        ProjectRelativePath::try_from("src/lib.rs".to_owned()),
        Ok(parsed)
    );
    assert_eq!(
        ProjectRelativePath::new(""),
        Err(ProjectRelativePathError::Empty)
    );
    let too_long = "a".repeat(4_097);
    assert!(matches!(
        ProjectRelativePath::new(too_long),
        Err(ProjectRelativePathError::TooLong { .. })
    ));
    assert_eq!(
        ProjectRelativePath::new("src/name."),
        Err(ProjectRelativePathError::NonPortableSuffix)
    );
}

/// `ae-sdd-daemon-design.md` §4.1 freezes `FlowRunId` as one main-flow run
/// instance and `SeriesRunId` as one physical execution attempt of a Series.
/// Both are daemon-minted UUIDs, so they belong with the other UUID identities
/// rather than being represented as bare strings at call sites.
#[test]
fn flow_run_and_series_run_are_uuid_identities() {
    let flow = Uuid::from_u128(0x0192_3f5a_7b1c_7000_8000_0000_0000_0001);
    let series = Uuid::from_u128(0x0192_3f5a_7b1c_7000_8000_0000_0000_0002);

    let flow_run = FlowRunId::from_uuid(flow);
    let series_run = SeriesRunId::from_uuid(series);

    assert_eq!(flow_run.into_uuid(), flow, "FlowRunId round-trips its UUID");
    assert_eq!(
        series_run.into_uuid(),
        series,
        "SeriesRunId round-trips its UUID"
    );
    assert_eq!(
        flow_run
            .to_string()
            .parse::<FlowRunId>()
            .expect("FlowRunId"),
        flow_run,
        "FlowRunId parses back from its canonical text"
    );
    assert_eq!(
        series_run
            .to_string()
            .parse::<SeriesRunId>()
            .expect("SeriesRunId"),
        series_run,
        "SeriesRunId parses back from its canonical text"
    );
    assert!(
        "not-a-uuid".parse::<FlowRunId>().is_err(),
        "a non-UUID must not become a FlowRunId"
    );
}
