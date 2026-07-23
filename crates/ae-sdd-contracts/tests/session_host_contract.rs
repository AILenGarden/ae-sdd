use ae_sdd_contracts::{
    AdapterId, ContextBundleId, ExternalSessionKey, HostTaskId, IdempotencyKey, MessageKey,
    SchemaVersion,
    compact::{CompactAck, CompactRequest, RehydrateReceipt},
    host::{AttestedAck, AttestedHostResult, HostAction, HostActionBody, MAX_HOST_MESSAGE_BYTES},
    session::{MAX_SESSION_CAPABILITIES, SessionBootstrapRequest, SessionBootstrapResponse},
};
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, ArtifactKind, ArtifactRef, BootId, CapabilityId, ClaimId, CompactId,
    ContextDigest, ContextGeneration, DelegationId, HostAckId, HostActionId, InputFingerprint,
    InventoryGeneration, ProjectRelativePath, SessionId, WorkspaceId,
};
use ae_sdd_protocol::{HostAckOutcome, HostActionKind, StableErrorCode};

fn parse<T>(value: &str) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
{
    value.parse().expect("fixture ID parses")
}

fn snapshot_ref() -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new("context-snapshot").expect("artifact kind"),
        ProjectRelativePath::new(".ae-sdd/snapshots/compact-1.json").expect("relative path"),
        ArtifactDigest::digest(b"snapshot"),
        8,
    )
}

#[test]
fn session_bootstrap_is_role_bound_bounded_and_strict() {
    let workspace_id: WorkspaceId = parse("00000000-0000-0000-0000-000000000001");
    let delegation_id: DelegationId = parse("00000000-0000-0000-0000-000000000002");
    let request = SessionBootstrapRequest::new(
        SchemaVersion::V1,
        workspace_id,
        ExternalSessionKey::new("codex-thread-42").expect("external session key"),
        AdapterId::new("codex-app-server").expect("adapter id"),
        AgentRole::Series,
        true,
        Some(delegation_id),
        vec![
            CapabilityId::new("host.create").expect("capability"),
            CapabilityId::new("host.compact").expect("capability"),
        ],
        Some(ContextBundleId::new("bundle-story-42").expect("context bundle")),
    )
    .expect("valid bootstrap request");

    let json = serde_json::to_string(&request).expect("serialize bootstrap request");
    let decoded: SessionBootstrapRequest =
        serde_json::from_str(&json).expect("deserialize bootstrap request");
    assert_eq!(decoded, request);
    assert_eq!(decoded.capabilities().len(), 2);

    let with_unknown = json.replacen('{', "{\"unexpected\":true,", 1);
    assert!(serde_json::from_str::<SessionBootstrapRequest>(&with_unknown).is_err());

    let root_with_delegation = SessionBootstrapRequest::new(
        SchemaVersion::V1,
        workspace_id,
        ExternalSessionKey::new("root-thread").expect("external session key"),
        AdapterId::new("codex-app-server").expect("adapter id"),
        AgentRole::Root,
        true,
        Some(delegation_id),
        Vec::new(),
        None,
    );
    assert!(root_with_delegation.is_err());

    let child_without_delegation = SessionBootstrapRequest::new(
        SchemaVersion::V1,
        workspace_id,
        ExternalSessionKey::new("child-thread").expect("external session key"),
        AdapterId::new("codex-app-server").expect("adapter id"),
        AgentRole::Task,
        true,
        None,
        Vec::new(),
        None,
    );
    assert!(child_without_delegation.is_err());

    let too_many = (0..=MAX_SESSION_CAPABILITIES)
        .map(|index| CapabilityId::new(format!("capability.{index}")))
        .collect::<Result<Vec<_>, _>>()
        .expect("bounded capability IDs");
    assert!(
        SessionBootstrapRequest::new(
            SchemaVersion::V1,
            workspace_id,
            ExternalSessionKey::new("root-thread").expect("external session key"),
            AdapterId::new("codex-app-server").expect("adapter id"),
            AgentRole::Root,
            true,
            None,
            too_many,
            None,
        )
        .is_err()
    );

    let response = SessionBootstrapResponse::new(
        SchemaVersion::V1,
        parse::<BootId>("00000000-0000-0000-0000-000000000003"),
        workspace_id,
        parse::<SessionId>("00000000-0000-0000-0000-000000000004"),
        AgentRole::Series,
        true,
        ContextGeneration::new(7),
        InventoryGeneration::new(11),
        "signed-capability-token",
        1_900_000_000_000,
    )
    .expect("valid bootstrap response");
    let response_json = serde_json::to_string(&response).expect("serialize response");
    let response_round_trip: SessionBootstrapResponse =
        serde_json::from_str(&response_json).expect("deserialize response");
    assert_eq!(response_round_trip, response);
}

#[test]
fn host_actions_are_typed_bounded_and_acknowledgements_are_correlated() {
    let session_id: SessionId = parse("00000000-0000-0000-0000-000000000010");
    let adapter_id = AdapterId::new("codex-app-server").expect("adapter id");
    let body = HostActionBody::send(
        session_id,
        MessageKey::new("message-1").expect("message key"),
        "continue with the approved plan",
    )
    .expect("bounded send body");
    let action = HostAction::new(
        SchemaVersion::V1,
        parse::<HostActionId>("00000000-0000-0000-0000-000000000011"),
        adapter_id.clone(),
        1,
        InputFingerprint::digest(b"host action"),
        1_900_000_000_000,
        body,
    )
    .expect("valid host action");
    assert_eq!(action.kind(), HostActionKind::Send);

    let json = serde_json::to_string(&action).expect("serialize action");
    let decoded: HostAction = serde_json::from_str(&json).expect("deserialize action");
    assert_eq!(decoded, action);
    assert!(json.contains("\"kind\":\"send\""));
    assert!(!json.contains("serde_json"));

    assert!(
        HostActionBody::send(
            session_id,
            MessageKey::new("message-too-large").expect("message key"),
            "x".repeat(MAX_HOST_MESSAGE_BYTES + 1),
        )
        .is_err()
    );

    let accepted = AttestedAck::accepted(
        SchemaVersion::V1,
        parse::<HostAckId>("00000000-0000-0000-0000-000000000012"),
        &action,
        Some(HostTaskId::new("host-task-1").expect("host task id")),
        Some(session_id),
        None,
        1_800_000_000_000,
    )
    .expect("correlated acknowledgement");
    assert_eq!(accepted.outcome(), HostAckOutcome::Accepted);
    assert!(accepted.validate_for(&action).is_ok());

    let rejected = AttestedAck::rejected(
        SchemaVersion::V1,
        parse::<HostAckId>("00000000-0000-0000-0000-000000000013"),
        &action,
        HostAckOutcome::Rejected,
        StableErrorCode::HostAckRejected,
        1_800_000_000_001,
    )
    .expect("typed rejection");
    assert_eq!(
        rejected.error_code(),
        Some(StableErrorCode::HostAckRejected)
    );

    let unknown = json.replacen('{', "{\"extra\":0,", 1);
    assert!(serde_json::from_str::<HostAction>(&unknown).is_err());
}

#[test]
fn attested_child_result_binds_claim_session_role_and_action() {
    let delegation_id: DelegationId = parse("00000000-0000-0000-0000-000000000020");
    let claim_id: ClaimId = parse("00000000-0000-0000-0000-000000000021");
    let child_session_id: SessionId = parse("00000000-0000-0000-0000-000000000022");
    let action = HostAction::new(
        SchemaVersion::V1,
        parse::<HostActionId>("00000000-0000-0000-0000-000000000023"),
        AdapterId::new("codex-app-server").expect("adapter id"),
        4,
        InputFingerprint::digest(b"attest child"),
        1_900_000_000_000,
        HostActionBody::attest(
            delegation_id,
            claim_id,
            child_session_id,
            AgentRole::Task,
            ContextDigest::digest(b"physical-session-proof"),
        )
        .expect("valid attest body"),
    )
    .expect("valid attest action");
    let ack = AttestedAck::accepted(
        SchemaVersion::V1,
        parse::<HostAckId>("00000000-0000-0000-0000-000000000024"),
        &action,
        None,
        Some(child_session_id),
        None,
        1_800_000_000_000,
    )
    .expect("accepted attest action");
    let result = AttestedHostResult::new(
        SchemaVersion::V1,
        &action,
        ack,
        delegation_id,
        claim_id,
        child_session_id,
        AgentRole::Task,
        ContextDigest::digest(b"physical-session-proof"),
    )
    .expect("attested child result");

    let json = serde_json::to_string(&result).expect("serialize attested result");
    let decoded: AttestedHostResult = serde_json::from_str(&json).expect("deserialize result");
    assert_eq!(decoded, result);
}

#[test]
fn compact_ack_and_rehydrate_receipt_are_separate_and_generation_checked() {
    let session_id: SessionId = parse("00000000-0000-0000-0000-000000000030");
    let request = CompactRequest::new(
        SchemaVersion::V1,
        parse::<CompactId>("00000000-0000-0000-0000-000000000031"),
        session_id,
        AdapterId::new("codex-app-server").expect("adapter id"),
        snapshot_ref(),
        ContextGeneration::new(7),
        ContextGeneration::new(8),
        1_900_000_000_000,
        IdempotencyKey::new("compact-session-7").expect("idempotency key"),
    )
    .expect("valid compact request");
    let action = HostAction::new(
        SchemaVersion::V1,
        parse::<HostActionId>("00000000-0000-0000-0000-000000000032"),
        request.adapter_id().clone(),
        9,
        InputFingerprint::digest(b"compact host action"),
        request.deadline_unix_ms(),
        HostActionBody::compact(request.clone()),
    )
    .expect("compact host action");
    let ack = AttestedAck::accepted(
        SchemaVersion::V1,
        parse::<HostAckId>("00000000-0000-0000-0000-000000000033"),
        &action,
        None,
        Some(session_id),
        Some(ContextGeneration::new(7)),
        1_800_000_000_000,
    )
    .expect("accepted compact action");
    let compact_ack = CompactAck::new(SchemaVersion::V1, &request, &action, ack)
        .expect("generation-correlated compact ACK");

    let ack_json = serde_json::to_string(&compact_ack).expect("serialize compact ACK");
    assert!(!ack_json.contains("restoredProjectionDigest"));
    let ack_round_trip: CompactAck =
        serde_json::from_str(&ack_json).expect("deserialize compact ACK");
    assert_eq!(ack_round_trip, compact_ack);

    let receipt = RehydrateReceipt::new(
        SchemaVersion::V1,
        &request,
        &compact_ack,
        ContextGeneration::new(8),
        ContextDigest::digest(b"restored projection"),
        1_800_000_000_100,
    )
    .expect("valid rehydrate receipt");
    let receipt_json = serde_json::to_string(&receipt).expect("serialize rehydrate receipt");
    assert!(receipt_json.contains("restoredProjectionDigest"));

    assert!(
        CompactRequest::new(
            SchemaVersion::V1,
            parse::<CompactId>("00000000-0000-0000-0000-000000000034"),
            session_id,
            AdapterId::new("codex-app-server").expect("adapter id"),
            snapshot_ref(),
            ContextGeneration::new(7),
            ContextGeneration::new(9),
            1_900_000_000_000,
            IdempotencyKey::new("invalid-generation").expect("idempotency key"),
        )
        .is_err()
    );
    assert!(
        RehydrateReceipt::new(
            SchemaVersion::V1,
            &request,
            &compact_ack,
            ContextGeneration::new(9),
            ContextDigest::digest(b"wrong generation"),
            1_800_000_000_100,
        )
        .is_err()
    );

    let invalid_generation = ack_json.replace("\"nextGeneration\":8", "\"nextGeneration\":9");
    assert!(serde_json::from_str::<CompactAck>(&invalid_generation).is_err());
}
