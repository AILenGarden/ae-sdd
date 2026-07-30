use std::fs;
use std::sync::Arc;

use ae_sdd_domain::{AgentRole, BootId, EventStoreId};
use ae_sdd_integrations::NativeBusinessAdapter;
use ae_sdd_protocol::{PROTOCOL_VERSION_V1, RequestParams, RpcMethod, StableErrorCode, WorkspaceMode};
use ae_sdd_runtime::{BusinessOperationPort, BusinessWorkspace, MemoryPersistence, PersistencePort};
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

fn workspace(root: &TempDir) -> BusinessWorkspace {
    BusinessWorkspace {
        workspace_id: Uuid::from_u128(21).to_string(),
        canonical_root: root.path().to_string_lossy().into_owned(),
        project_key: "adopt-test".to_owned(),
        mode: WorkspaceMode::RustCanary,
        agent_role: None,
        agent_grant: None,
        caller_kind: None,
        inventory_generation: 1,
    }
}

fn adapter(root: &TempDir) -> NativeBusinessAdapter {
    let event_store_id = EventStoreId::from_uuid(Uuid::from_u128(22));
    let persistence = Arc::new(MemoryPersistence::new(event_store_id));
    let port: Arc<dyn PersistencePort> = persistence;
    NativeBusinessAdapter::new(
        root.path().join("runtime.sqlite3"),
        event_store_id,
        BootId::from_uuid(Uuid::from_u128(23)),
        "0".repeat(64),
        port,
    )
}

fn params(work_item: Option<&str>, payload: Value, key: Option<&str>) -> RequestParams<Value> {
    RequestParams {
        protocol_version: PROTOCOL_VERSION_V1.to_owned(),
        workspace_id: Some(Uuid::from_u128(21).to_string()),
        agent_id: Some("agent-root".to_owned()),
        session_id: Some(Uuid::from_u128(24).to_string()),
        capability_token: None,
        turn_id: None,
        work_item_id: work_item.map(str::to_owned),
        lease_id: None,
        fencing_token: None,
        expected_revision: None,
        idempotency_key: key.map(str::to_owned),
        confirmation: None,
        deadline_ms: 1_000,
        payload,
    }
}

fn create(
    adapter: &NativeBusinessAdapter,
    workspace: &BusinessWorkspace,
    work_item: &str,
    payload: Value,
) -> Value {
    adapter
        .execute(
            RpcMethod::OperationExecute,
            &params(
                Some(work_item),
                json!({"operation":"workitem.create","payload":payload}),
                Some(&format!("create-{work_item}")),
            ),
            Some(workspace),
        )
        .expect("workitem.create with providedDocuments succeeds")
}

fn read_state(root: &TempDir, response: &Value) -> Value {
    let relative = response["data"]["statePath"]
        .as_str()
        .expect("statePath is reported");
    let bytes = fs::read(root.path().join(relative)).expect("state is on disk");
    serde_json::from_slice(&bytes).expect("state is JSON")
}

fn provide(root: &TempDir, path: &str, content: &str) {
    let absolute = root.path().join(path);
    fs::create_dir_all(absolute.parent().expect("document parent")).expect("document directory");
    fs::write(&absolute, content).expect("provided document");
}

fn prd(doc_id: &str, path: &str) -> Value {
    json!({"intent":"PRD","docId":doc_id,"path":path})
}

fn dr(doc_id: &str, path: &str, parent: Option<&str>) -> Value {
    match parent {
        Some(parent) => json!({"intent":"DR","docId":doc_id,"path":path,"parentDocId":parent}),
        None => json!({"intent":"DR","docId":doc_id,"path":path}),
    }
}

fn story(doc_id: &str, path: &str, parent: Option<&str>) -> Value {
    match parent {
        Some(parent) => {
            json!({"intent":"STORY","docId":doc_id,"path":path,"parentDocId":parent})
        }
        None => json!({"intent":"STORY","docId":doc_id,"path":path}),
    }
}

fn flow_snapshot(adapter: &NativeBusinessAdapter, workspace: &BusinessWorkspace, work_item: &str) -> Value {
    adapter
        .execute(
            RpcMethod::FlowSnapshot,
            &params(Some(work_item), json!({}), None),
            Some(workspace),
        )
        .expect("flow.snapshot succeeds")
}

#[test]
fn provided_documents_are_adopted_into_the_authoritative_tree() {
    let root = TempDir::new().expect("tempdir");
    provide(&root, "docs/PRD-001.md", "# custom PRD\n");
    provide(&root, "docs/DR-001.md", "# custom DR one\n");
    provide(&root, "docs/DR-002.md", "# custom DR two\n");
    provide(&root, "docs/STORY-001.md", "# nested story\n");
    provide(&root, "docs/STORY-002.md", "# root story\n");
    let workspace = workspace(&root);
    let adapter = adapter(&root);

    let response = create(
        &adapter,
        &workspace,
        "PRD-ADOPT-001",
        json!({
            "entryNode":"PRD",
            "providedDocuments":[
                prd("PRD-001", "docs/PRD-001.md"),
                dr("DR-001", "docs/DR-001.md", Some("PRD-001")),
                dr("DR-002", "docs/DR-002.md", Some("PRD-001")),
                story("STORY-001", "docs/STORY-001.md", Some("DR-001")),
                story("STORY-002", "docs/STORY-002.md", None),
            ]
        }),
    );
    let state = read_state(&root, &response);

    assert_eq!(state["phase"], "dr-generated");
    assert_eq!(state["currentPhase"], "dr-generated");
    assert_eq!(state["documentPaths"]["PRD"], "docs/PRD-001.md");
    assert_eq!(state["documentPaths"]["DR"], "docs/DR-001.md");
    assert_eq!(state["documentPaths"]["STORY"], "docs/STORY-001.md");
    assert_eq!(
        state["documentPaths"]["RA"],
        "ae-sdd-doc/RA/PRD-ADOPT-001.md",
        "unprovided intents keep their minted defaults"
    );
    assert_eq!(state["routeDocuments"]["PRD"], true);
    assert_eq!(state["routeDocuments"]["DR"], true);
    assert_eq!(state["routeDocuments"]["STORY"], true);

    assert_eq!(state["prdState"]["prdId"], "PRD-ADOPT-001");
    assert_eq!(state["prdState"]["docId"], "PRD-001");
    assert_eq!(state["prdState"]["docPath"], "docs/PRD-001.md");
    assert_eq!(state["prdState"]["phase"], "dr-generated");

    let dr_one = &state["drStates"]["DR-001"];
    assert_eq!(dr_one["drId"], "DR-001");
    assert_eq!(dr_one["phase"], "dr-generated");
    assert_eq!(dr_one["docPath"], "docs/DR-001.md");
    let nested = &dr_one["storyStates"]["STORY-001"];
    assert_eq!(nested["phase"], "story-generated");
    assert_eq!(nested["currentPhase"], "story-generated");
    assert_eq!(nested["docPath"], "docs/STORY-001.md");
    assert_eq!(state["drStates"]["DR-002"]["storyStates"], json!({}));

    let root_story = &state["storyStates"]["STORY-002"];
    assert_eq!(root_story["phase"], "story-generated");
    assert_eq!(root_story["currentPhase"], "story-generated");
    assert_eq!(root_story["docPath"], "docs/STORY-002.md");

    // Adoption is registration-only: provided files are never rewritten and
    // no minted default document is created for an adopted intent.
    assert_eq!(
        fs::read_to_string(root.path().join("docs/PRD-001.md")).expect("provided PRD"),
        "# custom PRD\n"
    );
    assert!(
        !root.path().join("ae-sdd-doc").exists(),
        "no minted document tree may be created for adopted intents"
    );

    let snapshot = flow_snapshot(&adapter, &workspace, "PRD-ADOPT-001");
    let tree = &snapshot["documentTree"];
    assert_eq!(tree["prd"]["docId"], "PRD-001");
    assert_eq!(tree["prd"]["docPath"], "docs/PRD-001.md");
    assert_eq!(tree["prd"]["phase"], "dr-generated");
    let drs = tree["drs"].as_array().expect("drs is an array");
    assert_eq!(drs.len(), 2);
    assert_eq!(drs[0]["drId"], "DR-001");
    assert_eq!(drs[0]["docPath"], "docs/DR-001.md");
    assert_eq!(drs[0]["stories"][0]["storyId"], "STORY-001");
    assert_eq!(drs[0]["stories"][0]["docPath"], "docs/STORY-001.md");
    assert_eq!(drs[1]["drId"], "DR-002");
    assert_eq!(drs[1]["stories"], json!([]));
    let stories = tree["stories"].as_array().expect("root stories");
    assert_eq!(stories.len(), 1);
    assert_eq!(stories[0]["storyId"], "STORY-002");
    assert_eq!(stories[0]["phase"], "story-generated");

    let context = adapter
        .project_context(
            &workspace,
            "PRD-ADOPT-001",
            "session-adopt",
            AgentRole::Root,
        )
        .expect("context projection succeeds");
    assert_eq!(
        context.projection["flow"]["documentTree"]["prd"]["docId"],
        "PRD-001",
        "the context projection exposes the same document tree"
    );
}

#[test]
fn a_traversal_path_is_rejected_before_any_state_is_written() {
    let root = TempDir::new().expect("tempdir");
    let workspace = workspace(&root);
    let adapter = adapter(&root);

    let error = adapter
        .execute(
            RpcMethod::OperationExecute,
            &params(
                Some("PRD-ADOPT-ESCAPE"),
                json!({
                    "operation":"workitem.create",
                    "payload":{
                        "entryNode":"PRD",
                        "providedDocuments":[prd("PRD-001", "../escape.md")]
                    }
                }),
                Some("create-escape"),
            ),
            Some(&workspace),
        )
        .expect_err("a traversal path must be rejected");

    assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
    assert!(
        !root.path().join(".auto-engineering").exists(),
        "a rejected adoption leaves no state directory behind"
    );
}

#[test]
fn a_missing_document_is_rejected_before_any_state_is_written() {
    let root = TempDir::new().expect("tempdir");
    let workspace = workspace(&root);
    let adapter = adapter(&root);

    let error = adapter
        .execute(
            RpcMethod::OperationExecute,
            &params(
                Some("PRD-ADOPT-MISSING"),
                json!({
                    "operation":"workitem.create",
                    "payload":{
                        "entryNode":"PRD",
                        "providedDocuments":[prd("PRD-001", "docs/NOPE-001.md")]
                    }
                }),
                Some("create-missing"),
            ),
            Some(&workspace),
        )
        .expect_err("a missing file must be rejected");

    assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
    assert!(!root.path().join(".auto-engineering").exists());
}

#[test]
fn dr_and_story_entries_fill_their_parent_links() {
    let root = TempDir::new().expect("tempdir");
    provide(&root, "docs/PRD-001.md", "# parent PRD\n");
    provide(&root, "docs/DR-ADOPT-004.md", "# own DR\n");
    provide(&root, "docs/STORY-001.md", "# child story\n");
    let workspace = workspace(&root);
    let adapter = adapter(&root);

    // DR entry adopting its own DR document plus the parent PRD.
    let response = create(
        &adapter,
        &workspace,
        "DR-ADOPT-004",
        json!({
            "entryNode":"DR",
            "providedDocuments":[
                prd("PRD-001", "docs/PRD-001.md"),
                dr("DR-ADOPT-004", "docs/DR-ADOPT-004.md", Some("PRD-001")),
                story("STORY-001", "docs/STORY-001.md", Some("DR-ADOPT-004")),
            ]
        }),
    );
    let state = read_state(&root, &response);
    assert_eq!(state["parentPrdId"], "PRD-001");
    assert_eq!(state["phase"], "dr-generated");
    assert_eq!(state["drState"]["docPath"], "docs/DR-ADOPT-004.md");
    assert_eq!(state["drState"]["phase"], "dr-generated");
    assert_eq!(
        state["drState"]["storyStates"]["STORY-001"]["docPath"],
        "docs/STORY-001.md",
        "a Story whose parent is the item's own DR nests under drState"
    );
    assert!(
        state.get("drStates").is_none(),
        "the item's own DR stays in the singular drState container"
    );

    // STORY entry adopting its parent DR plus its own Story document.
    let response = create(
        &adapter,
        &workspace,
        "STORY-ADOPT-004",
        json!({
            "entryNode":"STORY",
            "providedDocuments":[
                dr("DR-001", "docs/DR-ADOPT-004.md", None),
                story("STORY-009", "docs/STORY-001.md", Some("DR-001")),
            ]
        }),
    );
    let state = read_state(&root, &response);
    assert_eq!(state["parentDrId"], "DR-001");
    assert_eq!(state["phase"], "story-generated");
    assert_eq!(state["drStates"]["DR-001"]["phase"], "dr-generated");
    assert_eq!(
        state["drStates"]["DR-001"]["storyStates"]["STORY-009"]["phase"],
        "story-generated"
    );

    // STORY entry adopting only a DR: dr-generated is not a member of the
    // STORY route chain, so the deepest legal phase is requirement-analyzed.
    let response = create(
        &adapter,
        &workspace,
        "STORY-ADOPT-DRONLY",
        json!({
            "entryNode":"STORY",
            "providedDocuments":[dr("DR-001", "docs/DR-ADOPT-004.md", None)]
        }),
    );
    let state = read_state(&root, &response);
    assert_eq!(state["parentDrId"], "DR-001");
    assert_eq!(state["phase"], "requirement-analyzed");
    assert_eq!(state["currentPhase"], "requirement-analyzed");

    // PRD-only adoption keeps the item at initialized.
    let response = create(
        &adapter,
        &workspace,
        "PRD-ADOPT-ONLY",
        json!({
            "entryNode":"PRD",
            "providedDocuments":[prd("PRD-001", "docs/PRD-001.md")]
        }),
    );
    let state = read_state(&root, &response);
    assert_eq!(state["phase"], "initialized");
    assert_eq!(state["prdState"]["docPath"], "docs/PRD-001.md");
}

#[test]
fn a_route_intake_skips_adopted_series_in_its_handoff() {
    let root = TempDir::new().expect("tempdir");
    provide(&root, "docs/PRD-001.md", "# PRD\n");
    provide(&root, "docs/DR-001.md", "# DR\n");
    provide(&root, "docs/STORY-001.md", "# Story\n");
    let workspace = workspace(&root);
    let adapter = adapter(&root);

    let response = create(
        &adapter,
        &workspace,
        "ROUTE-ADOPT-001",
        json!({
            "entryNode":"ROUTE",
            "providedDocuments":[
                prd("PRD-001", "docs/PRD-001.md"),
                dr("DR-001", "docs/DR-001.md", Some("PRD-001")),
                story("STORY-001", "docs/STORY-001.md", Some("DR-001")),
            ]
        }),
    );
    let state = read_state(&root, &response);
    assert_eq!(state["phase"], "dr-generated");
    assert_eq!(state["routeDocuments"]["RA"], true);
    assert_eq!(state["routeDocuments"]["DR"], true);
    assert_eq!(state["routeDocuments"]["STORY"], true);

    // Before route.decide the projection is the route-analysis action, and it
    // still carries the adopted document tree.
    let snapshot = flow_snapshot(&adapter, &workspace, "ROUTE-ADOPT-001");
    assert_eq!(snapshot["nextAction"]["kind"], "analyze-route");
    assert_eq!(snapshot["documentTree"]["prd"]["docId"], "PRD-001");
    assert_eq!(snapshot["documentTree"]["drs"][0]["drId"], "DR-001");
    assert_eq!(
        snapshot["documentTree"]["drs"][0]["stories"][0]["storyId"],
        "STORY-001"
    );

    // Commit a route decision covering all three series; the handoff must not
    // delegate the adopted requirement-analysis or design-review series.
    let relative = response["data"]["statePath"]
        .as_str()
        .expect("statePath")
        .to_owned();
    let state_path = root.path().join(&relative);
    let mut state = read_state(&root, &response);
    state["scale"] = json!("large");
    state["selectedDesign"] = json!("DR");
    state["routeApproved"] = json!(true);
    state["routeDecision"] = json!({
        "designRoute":"dr",
        "requiredSeries":["requirement-analysis","design-review","story"]
    });
    fs::write(&state_path, serde_json::to_vec_pretty(&state).expect("state JSON"))
        .expect("commit route decision fixture");

    let snapshot = flow_snapshot(&adapter, &workspace, "ROUTE-ADOPT-001");
    assert_eq!(snapshot["nextAction"]["kind"], "delegate-series");
    assert_eq!(
        snapshot["nextAction"]["seriesKind"], "story",
        "adopted RA/DR series are skipped; the Story series is only delegated \
         for the un-adoptable CODING_PLAN artifact"
    );
    assert_eq!(snapshot["documentTree"]["drs"][0]["drId"], "DR-001");
}
