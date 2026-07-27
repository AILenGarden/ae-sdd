use std::fs;

use ae_sdd_domain::{AgentRole, ProjectPathScope, ProjectRelativePath, ScopedGrant};
use ae_sdd_integrations::SqliteRuntimePersistence;
use ae_sdd_protocol::{StableErrorCode, WorkspaceMode};
use ae_sdd_runtime::{BusinessWorkspace, PersistencePort};
use serde_json::{Value, json};
use tempfile::TempDir;

#[path = "../src/jobs/memory/mod.rs"]
mod memory;

use memory::TrustedMemoryIdentity;

fn root_grant() -> ScopedGrant {
    ScopedGrant::new([], [], [ProjectPathScope::ProjectRoot])
}

fn subtree_grant(path: &str) -> ScopedGrant {
    ScopedGrant::new(
        [],
        [],
        [ProjectPathScope::Subtree(
            ProjectRelativePath::new(path).expect("valid subtree"),
        )],
    )
}

fn workspace(
    root: &TempDir,
    mode: WorkspaceMode,
    role: AgentRole,
    grant: ScopedGrant,
) -> BusinessWorkspace {
    BusinessWorkspace {
        workspace_id: "workspace-memory".to_owned(),
        canonical_root: fs::canonicalize(root.path())
            .expect("canonical root")
            .to_string_lossy()
            .into_owned(),
        project_key: "memory-project".to_owned(),
        mode,
        agent_role: Some(role),
        agent_grant: Some(grant),
        caller_kind: None,
        inventory_generation: 7,
    }
}

fn identity(session: &str, role: AgentRole, key: &str) -> TrustedMemoryIdentity {
    TrustedMemoryIdentity {
        job_id: format!("job-{session}"),
        boot_id: "memory-test-boot".to_owned(),
        session_id: session.to_owned(),
        root_session_id: if role == AgentRole::Root {
            session.to_owned()
        } else {
            "root-session".to_owned()
        },
        delegation_id: (role != AgentRole::Root).then(|| format!("delegation-{session}")),
        context_generation: 3,
        attestation_ref: format!("memory-test-attestation:{session}"),
        attestation_digest: "a".repeat(64),
        identity_digest: "b".repeat(64),
        idempotency_key: key.to_owned(),
    }
}

fn run(
    persistence: &dyn PersistencePort,
    workspace: &BusinessWorkspace,
    work_item: &str,
    identity: &TrustedMemoryIdentity,
    entrypoint: &str,
    arguments: Value,
) -> Result<Value, ae_sdd_runtime::RuntimeError> {
    memory::execute(
        workspace,
        Some(work_item),
        persistence,
        Some(identity),
        entrypoint,
        &arguments,
    )
}

#[test]
fn memory_survives_restart_and_replays_mutations_durably() {
    let root = TempDir::new().expect("tempdir");
    fs::write(
        root.path().join("constraints.md"),
        "must use durable receipts\nsecret transcript line\n",
    )
    .expect("source fixture");
    let database = root.path().join("runtime.sqlite3");
    let first = SqliteRuntimePersistence::open(&database).expect("first persistence");
    let workspace = workspace(
        &root,
        WorkspaceMode::RustSoleWriter,
        AgentRole::Root,
        root_grant(),
    );
    let creator = identity("root-session", AgentRole::Root, "create-1");
    let arguments = json!({
        "entityType":"story",
        "entityId":"STORY-001",
        "sources":["constraints=constraints.md"],
        "context":{
            "series_chain":["route","story"],
            "current_series":"story",
            "next_step":"coding",
            "constraints":["must remain session scoped"],
            "pending_items":[{
                "id":"P1",
                "description":"verify isolation",
                "owner":"task",
                "status":"open"
            }]
        }
    });
    let created = run(
        &first,
        &workspace,
        "WORK-MEMORY",
        &creator,
        "memory.create",
        arguments.clone(),
    )
    .expect("create succeeds");
    assert_eq!(created["outcome"], "PASS");
    assert_eq!(created["replayed"], false);
    let replayed = run(
        &first,
        &workspace,
        "WORK-MEMORY",
        &creator,
        "memory.create",
        arguments.clone(),
    )
    .expect("create replays");
    assert_eq!(replayed["replayed"], true);
    let conflict = run(
        &first,
        &workspace,
        "WORK-MEMORY",
        &creator,
        "memory.create",
        json!({"entityType":"story","entityId":"STORY-002"}),
    )
    .expect_err("same idempotency key with another payload is rejected");
    assert_eq!(conflict.code(), StableErrorCode::IdempotencyKeyReused);
    drop(first);

    let reopened = SqliteRuntimePersistence::open(&database).expect("reopen persistence");
    let read = run(
        &reopened,
        &workspace,
        "WORK-MEMORY",
        &identity("root-session", AgentRole::Root, "unused-read-key"),
        "memory.read",
        json!({"entityType":"story","entityId":"STORY-001"}),
    )
    .expect("read after restart");
    assert_eq!(read["found"], true);
    assert!(
        read["boot"]
            .as_str()
            .is_some_and(|text| text.contains("STORY-001"))
    );
    assert!(
        read["context"]
            .as_str()
            .is_some_and(|text| text.contains("session scoped"))
    );
    let common = run(
        &reopened,
        &workspace,
        "WORK-MEMORY",
        &identity("root-session", AgentRole::Root, "unused-common-key"),
        "memory.common",
        json!({"action":"read"}),
    )
    .expect("common read");
    assert!(
        common["context"]
            .as_str()
            .is_some_and(|text| text.contains("durable receipts"))
    );
    assert!(!common.to_string().contains("secret transcript line"));
}

#[test]
fn search_and_reads_never_cross_role_session_work_item_or_generation() {
    let root = TempDir::new().expect("tempdir");
    let persistence =
        SqliteRuntimePersistence::open(root.path().join("runtime.sqlite3")).expect("persistence");
    let root_workspace = workspace(
        &root,
        WorkspaceMode::RustSoleWriter,
        AgentRole::Root,
        root_grant(),
    );
    run(
        &persistence,
        &root_workspace,
        "WORK-A",
        &identity("root-a", AgentRole::Root, "create-a"),
        "memory.create",
        json!({
            "entityType":"coding",
            "entityId":"A",
            "context":{"constraints":["worker scratch sentinel"]}
        }),
    )
    .expect("create root memory");

    let reviewer_workspace = workspace(
        &root,
        WorkspaceMode::RustSoleWriter,
        AgentRole::Reviewer,
        ScopedGrant::default(),
    );
    let outsiders = [
        (
            root_workspace.clone(),
            "WORK-A",
            identity("root-b", AgentRole::Root, "read-other-session"),
        ),
        (
            root_workspace.clone(),
            "WORK-B",
            identity("root-a", AgentRole::Root, "read-other-work"),
        ),
        (
            reviewer_workspace,
            "WORK-A",
            identity("reviewer-a", AgentRole::Reviewer, "read-reviewer"),
        ),
    ];
    for (workspace, work_item, outsider) in outsiders {
        let read = run(
            &persistence,
            &workspace,
            work_item,
            &outsider,
            "memory.read",
            json!({"entityType":"coding","entityId":"A"}),
        )
        .expect("isolated read");
        assert_eq!(read["found"], false);
        let search = run(
            &persistence,
            &workspace,
            work_item,
            &outsider,
            "memory.search",
            json!({"query":"worker scratch sentinel"}),
        )
        .expect("isolated search");
        assert_eq!(search["count"], 0);
    }

    let mut stale_generation = identity("root-a", AgentRole::Root, "stale-generation");
    stale_generation.context_generation = 4;
    let stale = run(
        &persistence,
        &root_workspace,
        "WORK-A",
        &stale_generation,
        "memory.read",
        json!({"entityType":"coding","entityId":"A"}),
    )
    .expect("stale generation has another namespace");
    assert_eq!(stale["found"], false);
}

#[test]
fn clean_all_and_common_are_limited_to_the_callers_exact_namespace() {
    let root = TempDir::new().expect("tempdir");
    let persistence =
        SqliteRuntimePersistence::open(root.path().join("runtime.sqlite3")).expect("persistence");
    let workspace = workspace(
        &root,
        WorkspaceMode::RustSoleWriter,
        AgentRole::Root,
        root_grant(),
    );
    for (session, entity) in [("root-a", "A"), ("root-b", "B")] {
        run(
            &persistence,
            &workspace,
            "WORK-MEMORY",
            &identity(session, AgentRole::Root, &format!("create-{entity}")),
            "memory.create",
            json!({"entityType":"coding","entityId":entity}),
        )
        .expect("create isolated memory");
    }
    run(
        &persistence,
        &workspace,
        "WORK-MEMORY",
        &identity("root-a", AgentRole::Root, "clean-all-a"),
        "memory.clean-all",
        json!({}),
    )
    .expect("clean own namespace");
    let first = run(
        &persistence,
        &workspace,
        "WORK-MEMORY",
        &identity("root-a", AgentRole::Root, "read-a"),
        "memory.read",
        json!({"entityType":"coding","entityId":"A"}),
    )
    .expect("read first");
    let second = run(
        &persistence,
        &workspace,
        "WORK-MEMORY",
        &identity("root-b", AgentRole::Root, "read-b"),
        "memory.read",
        json!({"entityType":"coding","entityId":"B"}),
    )
    .expect("read second");
    assert_eq!(first["found"], false);
    assert_eq!(second["found"], true);

    run(
        &persistence,
        &workspace,
        "WORK-MEMORY",
        &identity("root-b", AgentRole::Root, "common-update-b"),
        "memory.common",
        json!({"action":"update","content":"B-only common"}),
    )
    .expect("common update");
    let other_common = run(
        &persistence,
        &workspace,
        "WORK-MEMORY",
        &identity("root-a", AgentRole::Root, "common-read-a"),
        "memory.common",
        json!({"action":"read"}),
    )
    .expect("other common read");
    assert_ne!(other_common["context"], "B-only common");
}

#[test]
fn mutation_mode_and_source_grant_fail_closed() {
    let root = TempDir::new().expect("tempdir");
    fs::create_dir(root.path().join("allowed")).expect("allowed dir");
    fs::write(
        root.path().join("allowed/source.md"),
        "must stay inside grant",
    )
    .expect("allowed source");
    fs::write(root.path().join("outside.md"), "must not be read").expect("outside source");
    let persistence =
        SqliteRuntimePersistence::open(root.path().join("runtime.sqlite3")).expect("persistence");
    let shadow = workspace(&root, WorkspaceMode::Shadow, AgentRole::Root, root_grant());
    let error = run(
        &persistence,
        &shadow,
        "WORK-MEMORY",
        &identity("root-shadow", AgentRole::Root, "shadow-create"),
        "memory.create",
        json!({"entityType":"story","entityId":"S"}),
    )
    .expect_err("shadow cannot mutate memory");
    assert_eq!(error.code(), StableErrorCode::RoleOperationForbidden);

    let task_workspace = workspace(
        &root,
        WorkspaceMode::RustSoleWriter,
        AgentRole::Task,
        subtree_grant("allowed"),
    );
    let denied = run(
        &persistence,
        &task_workspace,
        "WORK-MEMORY",
        &identity("task-a", AgentRole::Task, "denied-source"),
        "memory.create",
        json!({
            "entityType":"coding",
            "entityId":"TASK-A",
            "sources":["constraints=outside.md"]
        }),
    )
    .expect_err("source outside grant is denied");
    assert_eq!(denied.code(), StableErrorCode::RoleOperationForbidden);
    let allowed = run(
        &persistence,
        &task_workspace,
        "WORK-MEMORY",
        &identity("task-a", AgentRole::Task, "allowed-source"),
        "memory.create",
        json!({
            "entityType":"coding",
            "entityId":"TASK-A",
            "sources":["constraints=allowed/source.md"]
        }),
    )
    .expect("source inside grant is accepted");
    assert_eq!(allowed["outcome"], "PASS");
}
