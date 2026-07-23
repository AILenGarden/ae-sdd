use std::fs;
use std::sync::Arc;

use ae_sdd_domain::{ArtifactDigest, BootId, EventStoreId};
use ae_sdd_integrations::NativeBusinessAdapter;
use ae_sdd_protocol::WorkspaceMode;
use ae_sdd_runtime::{
    BusinessOperationPort, BusinessWorkspace, MemoryPersistence, PersistencePort,
};
use serde_json::json;
use tempfile::TempDir;
use uuid::Uuid;

fn setup() -> (
    TempDir,
    NativeBusinessAdapter,
    Arc<MemoryPersistence>,
    BusinessWorkspace,
) {
    let root = TempDir::new().expect("tempdir");
    let event_store_id = EventStoreId::from_uuid(Uuid::from_u128(21));
    let persistence = Arc::new(MemoryPersistence::new(event_store_id));
    let port: Arc<dyn PersistencePort> = persistence.clone();
    let workspace = BusinessWorkspace {
        workspace_id: Uuid::from_u128(22).to_string(),
        canonical_root: root.path().to_string_lossy().into_owned(),
        project_key: "delegation-test".to_owned(),
        mode: WorkspaceMode::RustCanary,
        agent_role: None,
        agent_grant: None,
        caller_kind: None,
        inventory_generation: 1,
    };
    let adapter = NativeBusinessAdapter::new(
        root.path().join("runtime.sqlite3"),
        event_store_id,
        BootId::from_uuid(Uuid::from_u128(23)),
        "0".repeat(64),
        port,
    );
    (root, adapter, persistence, workspace)
}

#[test]
fn artifact_and_memory_receipts_bind_authoritative_files_and_purge_namespace() {
    let (root, adapter, persistence, workspace) = setup();
    fs::create_dir_all(root.path().join("src")).expect("src directory");
    let bytes = b"validated child artifact";
    fs::write(root.path().join("src/result.txt"), bytes).expect("artifact");
    let result = json!({
        "deliverables":[{
            "id":"deliverable-1",
            "kind":"source",
            "path":"src/result.txt",
            "digest":ArtifactDigest::digest(bytes).to_string(),
            "byteLength":bytes.len(),
        }],
        "memorySnapshotDigest":"a".repeat(64),
    });
    persistence
        .store_record(
            "delegation-memory/v1",
            "delegation-1",
            &json!({
                "schemaVersion":"delegation-memory/v1",
                "delegationId":"delegation-1",
                "workspaceId":workspace.workspace_id,
                "status":"active",
                "entries":[{"must":"be purged"}],
            }),
        )
        .expect("memory namespace");

    let artifact_receipt = adapter
        .validate_delegation_artifacts(&workspace, "delegation-1", &result)
        .expect("artifact receipt");
    let cleanup_receipt = adapter
        .cleanup_delegation_memory(&workspace, "delegation-1", &result, &artifact_receipt)
        .expect("cleanup receipt");
    let cleaned = persistence
        .load_record("delegation-memory/v1", "delegation-1")
        .expect("read namespace")
        .expect("namespace exists");

    assert_eq!(artifact_receipt["artifacts"][0]["path"], "src/result.txt");
    assert_eq!(
        cleanup_receipt["schemaVersion"],
        "delegation-memory-cleanup/v1"
    );
    assert_eq!(cleaned["status"], "cleaned");
    assert_eq!(cleaned["payloadPurged"], true);
    assert!(cleaned.get("entries").is_none());
    assert_eq!(
        adapter
            .cleanup_delegation_memory(&workspace, "delegation-1", &result, &artifact_receipt,)
            .expect("cleanup replay"),
        cleanup_receipt
    );
}

#[test]
fn forged_digest_and_workspace_escape_fail_closed() {
    let (root, adapter, _, workspace) = setup();
    fs::write(root.path().join("result.txt"), "actual").expect("artifact");
    let forged = json!({
        "deliverables":[{
            "id":"deliverable-1",
            "kind":"source",
            "path":"result.txt",
            "digest":"0".repeat(64),
            "byteLength":6,
        }],
        "memorySnapshotDigest":"a".repeat(64),
    });
    assert!(
        adapter
            .validate_delegation_artifacts(&workspace, "delegation-2", &forged)
            .is_err()
    );

    let escaped = json!({
        "deliverables":[{
            "id":"deliverable-1",
            "kind":"source",
            "path":"../outside.txt",
            "digest":"0".repeat(64),
            "byteLength":0,
        }],
        "memorySnapshotDigest":"a".repeat(64),
    });
    assert!(
        adapter
            .validate_delegation_artifacts(&workspace, "delegation-2", &escaped)
            .is_err()
    );
}
