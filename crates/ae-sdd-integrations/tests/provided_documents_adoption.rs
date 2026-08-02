use std::fs;
use std::sync::Arc;

use ae_sdd_domain::{AgentRole, BootId, EventStoreId, OperationId, ProjectPathScope, ScopedGrant};
use ae_sdd_integrations::NativeBusinessAdapter;
use ae_sdd_protocol::{
    PROTOCOL_VERSION_V1, RequestParams, RpcMethod, StableErrorCode, WorkspaceMode,
};
use ae_sdd_runtime::{
    BusinessOperationPort, BusinessWorkspace, MemoryPersistence, PersistencePort,
};
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

fn flow_snapshot(
    adapter: &NativeBusinessAdapter,
    workspace: &BusinessWorkspace,
    work_item: &str,
) -> Value {
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
        state["documentPaths"]["RA"], "ae-sdd-doc/RA/PRD-ADOPT-001.md",
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
        context.projection["flow"]["documentTree"]["prd"]["docId"], "PRD-001",
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
        state["drState"]["storyStates"]["STORY-001"]["docPath"], "docs/STORY-001.md",
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
    fs::write(
        &state_path,
        serde_json::to_vec_pretty(&state).expect("state JSON"),
    )
    .expect("commit route decision fixture");

    let snapshot = flow_snapshot(&adapter, &workspace, "ROUTE-ADOPT-001");
    assert_eq!(snapshot["nextAction"]["kind"], "delegate-series");
    assert_eq!(
        snapshot["nextAction"]["seriesKind"], "coding-plan",
        "adopted RA/DR/STORY series are skipped; only the un-adoptable \
         CODING_PLAN artifact is still delegated, and it owns its own series \
         now that Story/TestCase/CodingPlan are no longer bundled"
    );
    assert_eq!(snapshot["documentTree"]["drs"][0]["drId"], "DR-001");
}

/// `ae-sdd-daemon-design.md` §4.1 makes `FlowRunId` the identity of one main-flow
/// run: minted by the daemon as a time-ordered UUID, never reused by a retry,
/// preserved across recovery of the same run. §5.3 mints it right after the
/// bootstrap assessment is validated.
///
/// The created state carried no such field, so nothing could name "this run" as
/// distinct from the Work Item. Every run of one Work Item was indistinguishable,
/// which is exactly what the retry and recovery rules need to tell apart.
#[test]
fn workitem_create_mints_a_time_ordered_flow_run_id() {
    let root = TempDir::new().expect("temp dir");
    let workspace = workspace(&root);
    let adapter = adapter(&root);

    let response = create(
        &adapter,
        &workspace,
        "FLOWRUN-001",
        json!({"entryNode":"ROUTE"}),
    );
    let state = read_state(&root, &response);

    let flow_run_id = state["flowRunId"]
        .as_str()
        .expect("a created Work Item must name its first flow run");
    let parsed = Uuid::parse_str(flow_run_id).expect("flowRunId must be a UUID");
    assert_eq!(
        parsed.get_version_num(),
        7,
        "§4.1 requires a time-ordered UUID so runs sort by creation, not v4"
    );

    let second = create(
        &adapter,
        &workspace,
        "FLOWRUN-002",
        json!({"entryNode":"ROUTE"}),
    );
    let second_state = read_state(&root, &second);
    assert_ne!(
        second_state["flowRunId"], state["flowRunId"],
        "each run instance gets its own identity"
    );
}

/// D-03 requires a legacy-state compat read that "禁止缺失数据时隐式重建为空状态".
///
/// A Work Item created before `flowRunId` existed genuinely has no recorded run
/// identity. Two behaviours are both wrong: failing the read would strand every
/// existing workspace, and minting an id on read would *fabricate* one — it would
/// claim a run that was never minted, and would hand back a different answer on
/// every read, breaking §4.1's "preserved across recovery of the same run".
///
/// The compat read must therefore report absence as absence.
#[test]
fn legacy_state_without_a_flow_run_id_reads_as_absent_not_as_a_fresh_identity() {
    let root = TempDir::new().expect("temp dir");
    let workspace = workspace(&root);
    let adapter = adapter(&root);

    let response = create(
        &adapter,
        &workspace,
        "LEGACY-001",
        json!({"entryNode":"ROUTE"}),
    );
    let state_path = root.path().join(
        response["data"]["statePath"]
            .as_str()
            .expect("statePath is reported"),
    );

    // Roll the file back to its pre-`flowRunId` shape, which is exactly what an
    // existing workspace has on disk today.
    let mut state: Value =
        serde_json::from_slice(&fs::read(&state_path).expect("state")).expect("json");
    state
        .as_object_mut()
        .expect("state is an object")
        .remove("flowRunId")
        .expect("the created state had one to remove");
    fs::write(&state_path, serde_json::to_vec(&state).expect("encode")).expect("write");

    let first = ae_sdd_integrations::flow_run_identity(&state);
    let second = ae_sdd_integrations::flow_run_identity(&state);

    assert_eq!(
        first, None,
        "a legacy Work Item has no run identity, and the reader must not invent one"
    );
    assert_eq!(
        first, second,
        "repeated reads of one legacy state must agree; a minted-on-read id would not"
    );

    // And the read must still succeed end to end, not strand the workspace.
    let mut authorised = workspace.clone();
    authorised.agent_role = Some(AgentRole::Root);
    authorised.agent_grant = Some(ScopedGrant::new(
        [OperationId::new("workitem.get").expect("operation id")],
        [],
        [ProjectPathScope::ProjectRoot],
    ));
    let reread = adapter
        .execute(
            RpcMethod::OperationExecute,
            &params(
                Some("LEGACY-001"),
                json!({"operation":"workitem.get","payload":{}}),
                Some("legacy-get"),
            ),
            Some(&authorised),
        )
        .expect("a legacy state without flowRunId must still be readable");
    assert_eq!(
        reread["data"]["currentPhase"], "initialized",
        "the legacy state's own phase must survive the read: {reread}"
    );
    assert!(
        reread["data"]["flowRunId"].is_null(),
        "and the read must not backfill a run identity it never minted: {reread}"
    );
}

/// F-11 / D-01: `constraints/api.md` and the implementation disagreed on which
/// `entryNode` values exist. The constraint said "仅 PRD/DR/STORY" while the
/// `/ae-sdd` bootstrap created `entryNode=ROUTE`, so the declared API contract and
/// the shipped behaviour described two different intake models.
///
/// This pins the set the unified intake actually accepts. It is a behavioural
/// check rather than a doc parse: the constraint is prose, but a drift in either
/// direction — losing `ROUTE`, or re-admitting the `BUG`/`CONFIG` micro entries
/// that need a flat state this path does not build — now fails here with the
/// constraint named.
#[test]
fn unified_intake_accepts_exactly_the_declared_entry_nodes() {
    for accepted in ["ROUTE", "PRD", "DR", "STORY"] {
        let root = TempDir::new().expect("temp dir");
        let workspace = workspace(&root);
        let adapter = adapter(&root);
        let response = adapter.execute(
            RpcMethod::OperationExecute,
            &params(
                Some(&format!("{accepted}-INTAKE-001")),
                json!({"operation":"workitem.create","payload":{"entryNode":accepted}}),
                Some(&format!("intake-{accepted}")),
            ),
            Some(&workspace),
        );
        assert!(
            response.is_ok(),
            "`constraints/api.md` declares {accepted} a valid entry node: {response:?}"
        );
    }

    for rejected in ["BUG", "CONFIG"] {
        let root = TempDir::new().expect("temp dir");
        let workspace = workspace(&root);
        let adapter = adapter(&root);
        let response = adapter.execute(
            RpcMethod::OperationExecute,
            &params(
                Some(&format!("{rejected}-INTAKE-001")),
                json!({"operation":"workitem.create","payload":{"entryNode":rejected}}),
                Some(&format!("intake-{rejected}")),
            ),
            Some(&workspace),
        );
        assert!(
            response.is_err(),
            "{rejected} runs the micro chain on a flat state, which this path does \
             not build; accepting it would write a skeleton no reader expects"
        );
    }
}

/// Builds an adapter while keeping the persistence handle, so the test can query
/// the runtime record namespaces the adapter writes.
fn adapter_with_persistence(root: &TempDir) -> (NativeBusinessAdapter, Arc<MemoryPersistence>) {
    let event_store_id = EventStoreId::from_uuid(Uuid::from_u128(22));
    let persistence = Arc::new(MemoryPersistence::new(event_store_id));
    let port: Arc<dyn PersistencePort> = Arc::clone(&persistence) as Arc<dyn PersistencePort>;
    let adapter = NativeBusinessAdapter::new(
        root.path().join("runtime.sqlite3"),
        event_store_id,
        BootId::from_uuid(Uuid::from_u128(23)),
        "0".repeat(64),
        port,
    );
    (adapter, persistence)
}

/// D-03 item 5 and §4.2: the execution flow tree must be separately queryable.
///
/// Before the `flow_run/v1` projection a Flow Run existed only as a string inside
/// `state.json`, so "list the runs of this Work Item" had no answer — the run
/// identity was minted and then became unreachable. §1.2 goal 4 requires each of
/// the three relation families be independently queryable, and line 767 adds that
/// they must not pollute one another.
#[test]
fn a_created_work_item_publishes_a_queryable_flow_run_projection() {
    let root = TempDir::new().expect("temp dir");
    let workspace = workspace(&root);
    let (adapter, persistence) = adapter_with_persistence(&root);

    let response = create(
        &adapter,
        &workspace,
        "FLOWPROJ-001",
        json!({"entryNode":"ROUTE"}),
    );
    let flow_run_id = response["data"]["flowRunId"]
        .as_str()
        .expect("the create response must report the run it minted");

    let records = persistence
        .list_records("flow_run/v1")
        .expect("flow_run namespace is listable");
    assert_eq!(
        records.len(),
        1,
        "one create yields exactly one Flow Run projection"
    );

    let (key, projection) = &records[0];
    assert!(
        key.contains(flow_run_id),
        "the projection is keyed by FlowRunId, not by Work Item: §4.1 lets one \
         Work Item run many times, so a Work Item key would hold only the newest"
    );
    assert_eq!(projection["flowRunId"], json!(flow_run_id));
    assert_eq!(projection["workItemId"], json!("FLOWPROJ-001"));
    assert_eq!(projection["schemaVersion"], json!("flow_run/v1"));
    assert_eq!(
        projection["workspaceId"], workspace.workspace_id,
        "the projection is workspace-scoped so two workspaces cannot collide"
    );
}

/// §7 rule 9: 同一 Hook event 重放不会重复创建 Work Item、Flow Run。
///
/// Exercised through the *anonymous* create (`workItemId` omitted), which is the
/// `/ae-sdd` bootstrap path and the only one that replays: a named duplicate is
/// rejected as `ScopeAmbiguous` instead. Replay returns a different function's
/// response (`created_work_item_response`), so it is a genuinely separate chance
/// to mint a second run — without reading the stored identity back it would report
/// a fresh one and the namespace would grow a phantom run per replayed Hook.
#[test]
fn replaying_a_create_does_not_publish_a_second_flow_run() {
    let root = TempDir::new().expect("temp dir");
    let workspace = workspace(&root);
    let (adapter, persistence) = adapter_with_persistence(&root);

    let anonymous = |key: &str| {
        adapter
            .execute(
                RpcMethod::OperationExecute,
                &params(
                    None,
                    json!({"operation":"workitem.create","payload":{"entryNode":"ROUTE"}}),
                    Some(key),
                ),
                Some(&workspace),
            )
            .expect("anonymous workitem.create succeeds")
    };
    let first = anonymous("bootstrap-replay");
    let second = anonymous("bootstrap-replay");

    assert_eq!(
        first["data"]["flowRunId"], second["data"]["flowRunId"],
        "a replay must report the run the first create minted, not a new one"
    );
    assert_eq!(
        persistence
            .list_records("flow_run/v1")
            .expect("listable")
            .len(),
        1,
        "a replayed create must not add a phantom Flow Run"
    );
}
