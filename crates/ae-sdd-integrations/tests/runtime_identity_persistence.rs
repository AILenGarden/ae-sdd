use std::sync::Arc;

use ae_sdd_integrations::SqliteRuntimePersistence;
use ae_sdd_protocol::{StableErrorCode, WorkspaceMode};
use ae_sdd_runtime::{
    DurableEvent, GrantPathWire, PersistencePort, RuntimeDelegationAttestationRecord,
    RuntimeDelegationHostActionRecord, RuntimeDelegationRecord, RuntimeIdentityKind,
    RuntimeIdentitySnapshot, RuntimeIdentityTransition, RuntimeJobRecord, RuntimeJobStatus,
    RuntimeJobTransition, RuntimeSessionRecord, RuntimeWorkspaceRecord, ScopedGrantWire,
    WireAgentRole,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

fn digest(value: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(value.as_ref()))
}

fn workspace(id: &str, root: &str) -> RuntimeWorkspaceRecord {
    RuntimeWorkspaceRecord {
        workspace_id: id.to_owned(),
        canonical_root: root.to_owned(),
        project_key: format!("project-{id}"),
        mode: WorkspaceMode::Shadow,
        inventory_generation: 1,
        dirty: false,
        created_at_unix_ms: 1_000,
        updated_at_unix_ms: 1_000,
    }
}

fn transition(
    operation: &str,
    scope: &str,
    key: &str,
    request: &str,
    snapshot: RuntimeIdentitySnapshot,
) -> RuntimeIdentityTransition {
    RuntimeIdentityTransition {
        operation: operation.to_owned(),
        scope_digest: digest(scope),
        idempotency_key: key.to_owned(),
        request_digest: digest(request),
        expected_workspace_mode: None,
        expected_inventory_generation: None,
        expected_session_status: None,
        expected_delegation_status: None,
        expected_context_generation: None,
        snapshot,
        committed_at_unix_ms: 1_000,
    }
}

fn event(kind: &str, workspace_id: &str) -> DurableEvent {
    let payload = json!({"kind":kind});
    DurableEvent {
        event_store_id: String::new(),
        event_seq: 0,
        boot_id: "boot-test".to_owned(),
        kind: kind.to_owned(),
        workspace_id: Some(workspace_id.to_owned()),
        session_id: None,
        work_item_id: Some("STORY-C1".to_owned()),
        payload_digest: digest(serde_json::to_vec(&payload).expect("payload")),
        payload,
    }
}

#[test]
fn identity_bundle_receipt_is_atomic_and_replays_before_cas() {
    let root = TempDir::new().expect("temp root");
    let persistence = Arc::new(
        SqliteRuntimePersistence::open(root.path().join("runtime.sqlite3")).expect("persistence"),
    );
    let workspace_id = "00000000-0000-0000-0000-000000000001";
    let workspace = workspace(workspace_id, "D:/workspace-one");
    let registration = transition(
        "workspace.register",
        "workspace.register\0D:/workspace-one",
        "register-1",
        "request-1",
        RuntimeIdentitySnapshot {
            identity_kind: RuntimeIdentityKind::Workspace,
            workspace: workspace.clone(),
            session: None,
            delegation: None,
            host_action: None,
            attestation: None,
            current_boot_receipt: None,
            response: json!({"workspaceId":workspace_id}),
            replayed: false,
        },
    );
    let first = persistence
        .commit_identity_bundle(registration.clone())
        .expect("first registration");
    assert!(!first.replayed);
    let replay = persistence
        .commit_identity_bundle(registration.clone())
        .expect("durable replay");
    assert!(replay.replayed);
    let mut conflict = registration;
    conflict.request_digest = digest("different-request");
    assert_eq!(
        persistence
            .commit_identity_bundle(conflict)
            .expect_err("changed request conflicts")
            .code(),
        StableErrorCode::IdempotencyKeyReused
    );

    let root_session_id = "00000000-0000-0000-0000-000000000010";
    let root_session = RuntimeSessionRecord {
        session_id: root_session_id.to_owned(),
        agent_id: "root-agent".to_owned(),
        workspace_id: workspace_id.to_owned(),
        external_key_hash: digest("root-external"),
        role: WireAgentRole::Root,
        root_session_id: root_session_id.to_owned(),
        parent_session_id: None,
        delegation_id: None,
        engaged: false,
        current_work_item: Some("STORY-C1".to_owned()),
        grant: ScopedGrantWire {
            operations: vec!["review.record".to_owned()],
            capabilities: vec!["review".to_owned()],
            paths: vec![GrantPathWire::ProjectRoot],
        },
        context_generation: 0,
        expires_at_unix_ms: 60_000,
        status: "active".to_owned(),
        created_at_unix_ms: 1_000,
        updated_at_unix_ms: 1_000,
    };
    persistence
        .commit_identity_bundle(transition(
            "session.open",
            "session.open\0boot-test\0workspace\0root-external",
            "open-root",
            "open-root-request",
            RuntimeIdentitySnapshot {
                identity_kind: RuntimeIdentityKind::Session,
                workspace: workspace.clone(),
                session: Some(root_session.clone()),
                delegation: None,
                host_action: None,
                attestation: None,
                current_boot_receipt: None,
                response: json!({"sessionId":root_session_id}),
                replayed: false,
            },
        ))
        .expect("root session");

    let adapter = json!({"schemaVersion":"host-adapter/v1","capabilities":["create","attest"]});
    persistence
        .store_record("host-adapter/v1", "adapter-1", &adapter)
        .expect("typed adapter mirror");
    let delegation_id = "00000000-0000-0000-0000-000000000020";
    let action_id = "00000000-0000-0000-0000-000000000021";
    let child_session_id = "00000000-0000-0000-0000-000000000022";
    let action = json!({
        "actionId":action_id,
        "adapterId":"adapter-1",
        "commandSeq":1,
        "kind":"create",
        "delegationId":delegation_id,
        "compactId":null,
        "sessionId":null,
        "contextGeneration":null,
        "deadlineUnixMs":60_000
    });
    persistence
        .store_record("host-action/v1", action_id, &action)
        .expect("typed action mirror");
    let ack_id = "00000000-0000-0000-0000-000000000023";
    let ack = json!({
        "ackId":ack_id,
        "actionId":action_id,
        "commandSeq":1,
        "outcome":"accepted",
        "hostTaskId":"task-1",
        "sessionId":child_session_id
    });
    persistence
        .store_record("host-ack/v1", ack_id, &ack)
        .expect("typed ACK mirror");
    let action_digest = digest(serde_json::to_vec(&action).expect("action"));
    let ack_digest = digest(serde_json::to_vec(&ack).expect("ack"));
    let grant = ScopedGrantWire {
        operations: vec!["review.record".to_owned()],
        capabilities: vec!["review".to_owned()],
        paths: vec![GrantPathWire::ProjectRoot],
    };
    let child = RuntimeSessionRecord {
        session_id: child_session_id.to_owned(),
        agent_id: "pending-child".to_owned(),
        workspace_id: workspace_id.to_owned(),
        external_key_hash: digest(child_session_id),
        role: WireAgentRole::Reviewer,
        root_session_id: root_session_id.to_owned(),
        parent_session_id: Some(root_session_id.to_owned()),
        delegation_id: Some(delegation_id.to_owned()),
        engaged: false,
        current_work_item: Some("STORY-C1".to_owned()),
        grant: grant.clone(),
        context_generation: 0,
        expires_at_unix_ms: 60_000,
        status: "opening".to_owned(),
        created_at_unix_ms: 1_000,
        updated_at_unix_ms: 1_000,
    };
    let delegation = RuntimeDelegationRecord {
        delegation_id: delegation_id.to_owned(),
        workspace_id: workspace_id.to_owned(),
        work_item_id: Some("STORY-C1".to_owned()),
        root_session_id: root_session_id.to_owned(),
        parent_session_id: root_session_id.to_owned(),
        child_session_id: Some(child_session_id.to_owned()),
        parent_delegation_id: None,
        role: WireAgentRole::Reviewer,
        input_revision: 1,
        input_fingerprint: digest("input"),
        status: "running".to_owned(),
        deadline_unix_ms: 60_000,
        receipt_digest: digest("delegation-receipt"),
        created_at_unix_ms: 1_000,
        updated_at_unix_ms: 1_000,
    };
    let accepted = RuntimeIdentitySnapshot {
        identity_kind: RuntimeIdentityKind::Delegation,
        workspace: workspace.clone(),
        session: Some(child),
        delegation: Some(delegation),
        host_action: Some(RuntimeDelegationHostActionRecord {
            workspace_id: workspace_id.to_owned(),
            delegation_id: delegation_id.to_owned(),
            host_action_id: action_id.to_owned(),
            parent_session_id: root_session_id.to_owned(),
            action_digest: action_digest.clone(),
            created_at_unix_ms: 1_000,
        }),
        attestation: Some(RuntimeDelegationAttestationRecord {
            workspace_id: workspace_id.to_owned(),
            delegation_id: delegation_id.to_owned(),
            physical_session_id: child_session_id.to_owned(),
            host_action_id: action_id.to_owned(),
            host_ack_id: ack_id.to_owned(),
            action_digest,
            ack_digest,
            claim_digest: digest("single-use-claim"),
            grant,
            attestation_ref: format!("delegation:{delegation_id}"),
            attestation_digest: digest("attestation"),
            accepted_boot_id: "boot-test".to_owned(),
            accepted_at_unix_ms: 1_000,
            expires_at_unix_ms: 60_000,
        }),
        current_boot_receipt: None,
        response: json!({"delegationId":delegation_id,"status":"running"}),
        replayed: false,
    };
    persistence
        .commit_identity_bundle(transition(
            "delegation.accept",
            "delegation.accept\0workspace\0delegation",
            "accept-1",
            "accept-request",
            accepted,
        ))
        .expect("strict delegation identity bundle");

    let mut invalid = RuntimeIdentitySnapshot {
        identity_kind: RuntimeIdentityKind::Delegation,
        workspace,
        session: None,
        delegation: None,
        host_action: None,
        attestation: None,
        current_boot_receipt: None,
        response: json!({"capabilityToken":"must-not-persist"}),
        replayed: false,
    };
    invalid.response["capabilityToken"] = Value::String("secret".to_owned());
    assert!(
        persistence
            .commit_identity_bundle(transition(
                "delegation.accept",
                "bad-scope",
                "bad-key",
                "bad-request",
                invalid,
            ))
            .is_err()
    );
}

#[test]
fn typed_job_transitions_use_cas_and_persist_session_expired_stale() {
    let root = TempDir::new().expect("temp root");
    let persistence =
        SqliteRuntimePersistence::open(root.path().join("runtime.sqlite3")).expect("persistence");
    let workspace_id = "00000000-0000-0000-0000-000000000101";
    let workspace = workspace(workspace_id, "D:/workspace-job");
    persistence
        .commit_identity_bundle(transition(
            "workspace.register",
            "workspace-job",
            "register-job",
            "register-job-request",
            RuntimeIdentitySnapshot {
                identity_kind: RuntimeIdentityKind::Workspace,
                workspace,
                session: None,
                delegation: None,
                host_action: None,
                attestation: None,
                current_boot_receipt: None,
                response: json!({"workspaceId":workspace_id}),
                replayed: false,
            },
        ))
        .expect("workspace");
    let key = "job-key";
    let mut record = RuntimeJobRecord {
        job_id: "00000000-0000-0000-0000-000000000102".to_owned(),
        workspace_id: workspace_id.to_owned(),
        work_item_id: Some("STORY-C1".to_owned()),
        session_id: None,
        root_session_id: None,
        delegation_id: None,
        agent_role: None,
        context_generation: None,
        submission_boot_id: None,
        attestation_ref: None,
        attestation_digest: None,
        grant: None,
        identity_digest: None,
        workspace_mode: WorkspaceMode::Shadow,
        inventory_generation: 1,
        entrypoint: "classify".to_owned(),
        arguments: json!({"text":"bounded"}),
        submission_scope_digest: digest("job-scope"),
        submission_idempotency_key: key.to_owned(),
        submission_idempotency_key_digest: digest(key),
        request_digest: digest("job-request"),
        source_revision: Some(1),
        input_fingerprint: Some(digest("job-input")),
        deadline_unix_ms: 60_000,
        status: RuntimeJobStatus::Queued,
        row_version: 0,
        result: None,
        error_code: None,
        mutation_id: None,
        receipt_locator: None,
        project_receipt_digest: None,
        submitted_event_seq: 0,
        last_event_seq: 0,
        created_at_unix_ms: 1_000,
        started_at_unix_ms: None,
        finished_at_unix_ms: None,
        updated_at_unix_ms: 1_000,
    };
    record = persistence
        .commit_job_transition(RuntimeJobTransition {
            record,
            expected_status: None,
            expected_row_version: None,
            event: event("job.submitted", workspace_id),
        })
        .expect("submit");
    assert!(record.submitted_event_seq > 0);
    record.status = RuntimeJobStatus::Running;
    record.row_version = 1;
    record.started_at_unix_ms = Some(2_000);
    record.updated_at_unix_ms = 2_000;
    record = persistence
        .commit_job_transition(RuntimeJobTransition {
            record,
            expected_status: Some(RuntimeJobStatus::Queued),
            expected_row_version: Some(0),
            event: event("job.started", workspace_id),
        })
        .expect("start");
    record.status = RuntimeJobStatus::Stale;
    record.row_version = 2;
    record.result = Some(json!({"errorCode":"SESSION_EXPIRED"}));
    record.finished_at_unix_ms = Some(3_000);
    record.updated_at_unix_ms = 3_000;
    let stale = persistence
        .commit_job_transition(RuntimeJobTransition {
            record,
            expected_status: Some(RuntimeJobStatus::Running),
            expected_row_version: Some(1),
            event: event("job.stale", workspace_id),
        })
        .expect("stale");
    assert_eq!(stale.status, RuntimeJobStatus::Stale);
    assert_eq!(stale.error_code, None);
    assert_eq!(
        persistence
            .load_job(&stale.job_id)
            .expect("load")
            .expect("job")
            .result,
        Some(json!({"errorCode":"SESSION_EXPIRED"}))
    );
}
