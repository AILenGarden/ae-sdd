use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;

use ae_sdd_domain::{ArtifactDigest, BootId, EventStoreId, InputFingerprint};
use ae_sdd_integrations::{FileWorkspaceResolver, NativeBusinessAdapter};
use ae_sdd_protocol::{
    ClientKind, ConfirmationRef, HandshakeRequest, JsonRpcRequest, PROTOCOL_RANGE_V1,
    PROTOCOL_VERSION_V1, RequestParams, RpcMethod, SecretString, StableErrorCode, WorkspaceMode,
};
use ae_sdd_runtime::{
    BusinessOperationPort, ClockPort, ConnectionState, MemoryPersistence, PersistencePort,
    RuntimeConfig, RuntimeService, SessionResult, WorkspaceParityEvidence, WorkspaceResult,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

#[allow(dead_code, unused_imports)]
#[path = "../../../bins/ae-sdd-cli/src/legacy/mod.rs"]
mod legacy;
#[path = "support/legacy_job_fixture.rs"]
mod legacy_job_fixture;

use legacy_job_fixture::prepare_workspace;

const NOW_MS: u64 = 1_000;
const ENDPOINT_TOKEN: &str = "legacy-job-e2e-token";
const MEMORY_WORK_ITEM: &str = "STORY-MEMORY-E2E";
const MEMORY_EMPTY_WORK_ITEM: &str = "STORY-MEMORY-EMPTY";
const MEMORY_COMMON_WORK_ITEM: &str = "STORY-MEMORY-COMMON";
const MEMORY_TRUNCATED_WORK_ITEM: &str = "STORY-MEMORY-TRUNCATED";

struct FixedClock;

impl ClockPort for FixedClock {
    fn now_unix_ms(&self) -> u64 {
        NOW_MS
    }
}

struct Harness {
    runtime: Arc<RuntimeService>,
    connection: ConnectionState,
    workspace: WorkspaceResult,
    agent_id: String,
    sessions: BTreeMap<String, SessionResult>,
}

impl Harness {
    fn new(root: &TempDir) -> Self {
        prepare_workspace(root);
        let event_store_id = EventStoreId::from_uuid(Uuid::from_u128(501));
        let persistence = Arc::new(MemoryPersistence::new(event_store_id));
        let persistence_port: Arc<dyn PersistencePort> = persistence.clone();
        let business: Arc<dyn BusinessOperationPort> = Arc::new(NativeBusinessAdapter::new(
            root.path().join("runtime.sqlite3"),
            event_store_id,
            BootId::from_uuid(Uuid::from_u128(502)),
            ae_sdd_policy::policy_digest().to_hex(),
            Arc::clone(&persistence_port),
        ));
        let resolver = Arc::new(
            FileWorkspaceResolver::new([root.path().to_path_buf()]).expect("workspace resolver"),
        );
        let runtime = Arc::new(RuntimeService::new(
            RuntimeConfig::default(),
            BootId::from_uuid(Uuid::from_u128(503)),
            ENDPOINT_TOKEN,
            persistence_port,
            Arc::new(FixedClock),
            resolver,
            business,
        ));
        let mut connection = connection(&runtime, ClientKind::Cli);
        let mut register = params(
            json!({
                "projectRoot":root.path(),
                "projectKey":"legacy-e2e",
                "mode":WorkspaceMode::Shadow,
            }),
            1_000,
        );
        register.idempotency_key = Some("workspace-register-legacy-e2e".to_owned());
        let workspace = serde_json::from_value(assert_result(raw_call(
            &runtime,
            &mut connection,
            RpcMethod::WorkspaceRegister,
            serde_json::to_value(register).expect("register params JSON"),
        )))
        .expect("workspace result");
        Self {
            runtime,
            connection,
            workspace,
            agent_id: "agent-legacy-job-e2e".to_owned(),
            sessions: BTreeMap::new(),
        }
    }

    fn cut_over_to_canary(&mut self) {
        let mut admin = connection(&self.runtime, ClientKind::Admin);
        let mut drain = params(json!({"stop":false}), 1_000);
        drain.idempotency_key = Some("legacy-job-e2e-drain".to_owned());
        drain.confirmation = Some(confirmation());
        assert_result(raw_call(
            &self.runtime,
            &mut admin,
            RpcMethod::RuntimeDrain,
            serde_json::to_value(drain).expect("drain JSON"),
        ));

        let parity = WorkspaceParityEvidence {
            comparison_count: 8,
            mismatch_count: 0,
            source_revision: 1,
            legacy_digest: "a".repeat(64),
            rust_digest: "a".repeat(64),
            observed_at_unix_ms: NOW_MS,
        };
        let parity_digest = InputFingerprint::digest(
            serde_json::to_vec(&parity).expect("parity evidence serializes"),
        )
        .to_string();
        let mut transition = params(
            json!({
                "targetMode":WorkspaceMode::RustCanary,
                "reason":"legacy memory job E2E parity fixture",
                "parityDigest":parity_digest,
                "parity":parity,
            }),
            1_000,
        );
        transition.workspace_id = Some(self.workspace.workspace_id.clone());
        transition.idempotency_key = Some("legacy-job-e2e-canary".to_owned());
        transition.confirmation = Some(confirmation());
        self.workspace = serde_json::from_value(assert_result(raw_call(
            &self.runtime,
            &mut admin,
            RpcMethod::WorkspaceModeTransition,
            serde_json::to_value(transition).expect("mode transition JSON"),
        )))
        .expect("canary workspace result");
        assert_eq!(self.workspace.mode, WorkspaceMode::RustCanary);
    }

    fn identity_for(&mut self, work_item: &str) -> SessionResult {
        if let Some(session) = self.sessions.get(work_item) {
            return session.clone();
        }
        let mut request = params(
            json!({
                "externalKey":format!("legacy-job-e2e-{work_item}"),
                "role":"root",
                "engaged":true,
            }),
            1_000,
        );
        request.workspace_id = Some(self.workspace.workspace_id.clone());
        request.work_item_id = Some(work_item.to_owned());
        request.agent_id = Some(self.agent_id.clone());
        request.idempotency_key = Some(format!("open-legacy-job-e2e-{work_item}"));
        let session: SessionResult = serde_json::from_value(assert_result(raw_call(
            &self.runtime,
            &mut self.connection,
            RpcMethod::SessionOpen,
            serde_json::to_value(request).expect("session JSON"),
        )))
        .expect("root session result");
        self.sessions.insert(work_item.to_owned(), session.clone());
        session
    }

    fn submit_legacy(&mut self, id: &str, business_argv: &[&str], sequence: usize) -> Value {
        let completed =
            self.submit_workspace_legacy(id, business_argv, &format!("legacy-job-{sequence}"));
        assert_eq!(completed["status"], "pass", "{id}: {completed}");
        assert_eq!(completed["result"]["outcome"], "PASS", "{id}");
        completed
    }

    fn submit_workspace_legacy(
        &mut self,
        id: &str,
        business_argv: &[&str],
        idempotency_key: &str,
    ) -> Value {
        let mut argv = id.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        argv.extend(business_argv.iter().map(|value| (*value).to_owned()));
        argv.extend([
            "--workspace-id".to_owned(),
            self.workspace.workspace_id.clone(),
            "--idempotency-key".to_owned(),
            idempotency_key.to_owned(),
        ]);
        let resolved = legacy::resolve_legacy_argv(&argv).expect("legacy route");
        let (method, entrypoint) = job_target(&resolved.route);
        let invocation = legacy::parse_rpc_invocation(
            &resolved.route,
            method,
            &resolved.trailing_arguments,
            |_| None,
        )
        .expect("legacy argv adapter");
        let mut request = match invocation.request {
            legacy::LegacyRequestSource::Synthesized(request) => *request,
            legacy::LegacyRequestSource::ExplicitJson(_) => panic!("test uses synthesized argv"),
        };
        legacy::adapt_job_submission(&resolved.route, &entrypoint, &mut request, NOW_MS)
            .expect("job adapter");
        assert_eq!(request.deadline_ms, 30_000);
        assert_eq!(request.payload["entrypoint"], entrypoint);
        assert!(request.payload["arguments"].is_object());
        assert_eq!(request.payload["deadlineUnixMs"], NOW_MS + 300_000);
        assert_eq!(request.payload.as_object().expect("job payload").len(), 3);

        let submitted = assert_result(raw_call(
            &self.runtime,
            &mut self.connection,
            RpcMethod::JobSubmit,
            serde_json::to_value(request).expect("job params JSON"),
        ));
        assert_eq!(submitted["status"], "queued");
        assert!(self.runtime.run_one_pending_job().expect("run queued job"));
        let mut status = params(json!({"jobId":submitted["jobId"]}), 1_000);
        status.workspace_id = Some(self.workspace.workspace_id.clone());
        assert_result(raw_call(
            &self.runtime,
            &mut self.connection,
            RpcMethod::JobStatus,
            serde_json::to_value(status).expect("status params JSON"),
        ))
    }

    fn submit_bound_legacy(
        &mut self,
        id: &str,
        business_argv: &[&str],
        work_item: &str,
        idempotency_key: &str,
    ) -> Value {
        let session = self.identity_for(work_item);
        let mut argv = id.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        argv.extend(business_argv.iter().map(|value| (*value).to_owned()));
        argv.extend([
            "--workspace-id".to_owned(),
            self.workspace.workspace_id.clone(),
            "--work-item-id".to_owned(),
            work_item.to_owned(),
            "--agent-id".to_owned(),
            self.agent_id.clone(),
            "--session-id".to_owned(),
            session.session_id.clone(),
            "--capability-token".to_owned(),
            session.capability_token.clone(),
            "--idempotency-key".to_owned(),
            idempotency_key.to_owned(),
        ]);
        let resolved = legacy::resolve_legacy_argv(&argv).expect("bound legacy route");
        let (method, entrypoint) = job_target(&resolved.route);
        let invocation = legacy::parse_rpc_invocation(
            &resolved.route,
            method,
            &resolved.trailing_arguments,
            |_| None,
        )
        .expect("bound legacy argv adapter");
        let mut request = match invocation.request {
            legacy::LegacyRequestSource::Synthesized(request) => *request,
            legacy::LegacyRequestSource::ExplicitJson(_) => panic!("synthesized request expected"),
        };
        legacy::adapt_job_submission(&resolved.route, &entrypoint, &mut request, NOW_MS)
            .expect("bound job adapter");
        let submitted = assert_result(raw_call(
            &self.runtime,
            &mut self.connection,
            RpcMethod::JobSubmit,
            serde_json::to_value(request).expect("bound job params JSON"),
        ));
        assert_eq!(submitted["status"], "queued", "{id}: {submitted}");
        assert_eq!(submitted["workItemId"], work_item);
        assert_eq!(submitted["sessionId"], session.session_id);
        assert!(self.runtime.run_one_pending_job().expect("run bound job"));

        let mut status = params(json!({"jobId":submitted["jobId"]}), 1_000);
        status.workspace_id = Some(self.workspace.workspace_id.clone());
        status.work_item_id = Some(work_item.to_owned());
        status.agent_id = Some(self.agent_id.clone());
        status.session_id = Some(session.session_id);
        status.capability_token = Some(session.capability_token);
        assert_result(raw_call(
            &self.runtime,
            &mut self.connection,
            RpcMethod::JobStatus,
            serde_json::to_value(status).expect("bound status params JSON"),
        ))
    }
}

#[test]
fn all_twenty_five_read_only_legacy_jobs_reach_native_daemon_execution() {
    let root = TempDir::new().expect("tempdir");
    let mut harness = Harness::new(&root);
    let cases: Vec<(&str, Vec<&str>)> = vec![
        ("assets check", vec![]),
        ("assets outline", vec![]),
        ("assets query", vec!["service", "--top", "5"]),
        ("assets read", vec!["coding", "--keys", "service"]),
        ("assets section", vec!["A"]),
        ("assets stats", vec![]),
        ("automation status", vec![]),
        (
            "baseline diff",
            vec![
                "--report",
                r#"{"findings":[{"findingKey":"finding-1","ruleId":"R1","path":"tracked.txt","severity":"WARNING"}]}"#,
            ],
        ),
        ("baseline inspect", vec![]),
        ("classify", vec!["--text", "large architecture migration"]),
        ("db audit", vec![]),
        (
            "db explain",
            vec!["--profile", "local", "--sql", "SELECT id,name FROM item"],
        ),
        ("db profiles", vec![]),
        (
            "db query",
            vec!["--profile", "local", "--sql", "SELECT id,name FROM item"],
        ),
        (
            "evidence lookup",
            vec![
                "--story",
                "STORY-EVIDENCE-001",
                "--command",
                "cargo test",
                "--input-fingerprint",
                "input-1",
                "--toolchain-fingerprint",
                "toolchain-1",
            ],
        ),
        ("git blame", vec!["--file", "tracked.txt"]),
        ("git diff", vec!["--stat"]),
        (
            "git impact",
            vec!["--file", "tracked.txt", "--file", "src/lib.rs"],
        ),
        ("git log", vec!["--limit", "5"]),
        ("git status", vec![]),
        ("perf doctor", vec!["--last", "10", "--limit", "5"]),
        ("perf report", vec!["--last", "10", "--limit", "5"]),
        ("plugin list", vec![]),
        ("plugin trace", vec!["fixture-skill"]),
        ("plugin validate", vec![]),
    ];
    assert_eq!(cases.len(), 25);
    let unique = cases
        .iter()
        .map(|(id, _)| *id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), 25);
    for (sequence, (id, argv)) in cases.iter().enumerate() {
        let completed = harness.submit_legacy(id, argv, sequence + 1);
        if *id == "git impact" {
            assert_eq!(completed["result"]["fileCount"], 2);
        }
    }
}

#[test]
fn all_eight_memory_commands_reach_the_trusted_daemon_namespace() {
    let root = TempDir::new().expect("tempdir");
    let mut harness = Harness::new(&root);
    prepare_memory_workspace(&root);
    harness.cut_over_to_canary();

    let context = serde_json::to_string(&json!({
        "series_chain":["route","story","coding"],
        "current_series":"coding",
        "next_step":"test",
        "deliverables":[{"name":"daemon","path":"src/daemon.rs","status":"ready"}],
        "dr_anchors":[{"section":"runtime","line":12,"summary":"must | remain\nbound"}],
        "story_acs":[{"id":"AC-1","description":"trusted memory","status":"covered"}],
        "constraints":["must preserve the trusted namespace"],
        "api_contracts":[{
            "name":"memory","method":"POST","path":"job.submit",
            "request":"arguments","response":"receipt"
        }],
        "data_models":[{"table":"runtime_memory","fields":"key,value","notes":"durable"}],
        "asset_refs":["constraints/testing.md"],
        "review_loop_status":"running",
        "pending_items":[{
            "id":"P-1","description":"finish coverage","owner":"root","status":"open"
        }],
        "failure_history":[{"round":1,"issue":"gap","action":"add E2E"}],
        "correction_counts":{"coding":2}
    }))
    .expect("structured context JSON");
    let project = root.path().to_string_lossy().into_owned();
    let created = harness.submit_bound_legacy(
        "memory create",
        &[
            "--entity-type",
            "story",
            "--entity-id",
            "STORY-MEMORY-1",
            "--project",
            &project,
            "--sources",
            "constraints=constraints/memory-source.md",
            "--context-json",
            &context,
        ],
        MEMORY_WORK_ITEM,
        "memory-create-story",
    );
    let created = passed_job(&created, "memory create");
    assert_eq!(created["created"], true);
    assert_eq!(created["entity_type"], "story");

    let read = harness.submit_bound_legacy(
        "memory read",
        &["--entity-type", "story", "--entity-id", "STORY-MEMORY-1"],
        MEMORY_WORK_ITEM,
        "memory-read-story",
    );
    let read = passed_job(&read, "memory read");
    assert_eq!(read["found"], true);
    assert!(
        read["boot"]
            .as_str()
            .is_some_and(|text| text.contains("Series Chain"))
    );
    assert!(
        read["context"]
            .as_str()
            .is_some_and(|text| text.contains("API Contracts"))
    );
    assert!(
        read["pending"]
            .as_str()
            .is_some_and(|text| text.contains("Failure History"))
    );

    for (index, (slice, content_args)) in [
        ("boot", vec!["--content", "replacement boot"]),
        (
            "context",
            vec!["--content-file", "constraints/memory-content.md"],
        ),
        ("pending", vec!["--content", "needle pending projection"]),
    ]
    .into_iter()
    .enumerate()
    {
        let mut arguments = vec![
            "--entity-type",
            "story",
            "--entity-id",
            "STORY-MEMORY-1",
            "--slice",
            slice,
        ];
        arguments.extend(content_args);
        let updated = harness.submit_bound_legacy(
            "memory update",
            &arguments,
            MEMORY_WORK_ITEM,
            &format!("memory-update-{index}"),
        );
        assert_eq!(passed_job(&updated, "memory update")["slice"], slice);
    }

    let searched = harness.submit_bound_legacy(
        "memory search",
        &["--query", "pending projection", "--limit", "1"],
        MEMORY_WORK_ITEM,
        "memory-search",
    );
    let searched = passed_job(&searched, "memory search");
    assert_eq!(searched["count"], 1);
    assert_eq!(searched["entries"][0]["slice"], "pending.compact");

    let summarized = harness.submit_bound_legacy(
        "memory summarize",
        &[],
        MEMORY_WORK_ITEM,
        "memory-summarize",
    );
    assert!(
        passed_job(&summarized, "memory summarize")["total_slices"]
            .as_u64()
            .is_some_and(|count| count >= 4)
    );

    let common = harness.submit_bound_legacy(
        "memory common",
        &["read"],
        MEMORY_WORK_ITEM,
        "memory-common-read",
    );
    let common = passed_job(&common, "memory common");
    assert_eq!(common["found"], true);
    assert!(
        common["context"]
            .as_str()
            .is_some_and(|text| text.contains("durable receipts"))
    );

    for (action, key) in [
        (
            &["update", "--content", "manually shared context"][..],
            "memory-common-update",
        ),
        (&["clean"][..], "memory-common-clean"),
    ] {
        let completed = harness.submit_bound_legacy("memory common", action, MEMORY_WORK_ITEM, key);
        assert_eq!(passed_job(&completed, "memory common")["outcome"], "PASS");
    }
    let common_after_clean = harness.submit_bound_legacy(
        "memory common",
        &["read"],
        MEMORY_WORK_ITEM,
        "memory-common-read-cleaned",
    );
    assert_eq!(
        passed_job(&common_after_clean, "memory common")["found"],
        false
    );

    let common_restored = harness.submit_bound_legacy(
        "memory common",
        &["update", "--content-file", "constraints/memory-content.md"],
        MEMORY_WORK_ITEM,
        "memory-common-restore",
    );
    assert_eq!(
        passed_job(&common_restored, "memory common")["updated"],
        true
    );

    let selector = ["--entity-type", "story", "--entity-id", "STORY-MEMORY-1"];
    let cleaned = harness.submit_bound_legacy(
        "memory clean",
        &selector,
        MEMORY_WORK_ITEM,
        "memory-clean-story",
    );
    assert_eq!(passed_job(&cleaned, "memory clean")["cleaned"], true);
    let cleaned_again = harness.submit_bound_legacy(
        "memory clean",
        &selector,
        MEMORY_WORK_ITEM,
        "memory-clean-story-again",
    );
    assert_eq!(passed_job(&cleaned_again, "memory clean")["cleaned"], false);
    let preserved = harness.submit_bound_legacy(
        "memory clean",
        &[],
        MEMORY_WORK_ITEM,
        "memory-clean-common-preserved",
    );
    assert_eq!(passed_job(&preserved, "memory clean")["cleaned"], false);

    let recreated = harness.submit_bound_legacy(
        "memory create",
        &selector,
        MEMORY_WORK_ITEM,
        "memory-recreate-story",
    );
    assert_eq!(passed_job(&recreated, "memory create")["created"], true);
    let coding = harness.submit_bound_legacy(
        "memory create",
        &["--phase", "coding", "--task", "TASK-MEMORY-2"],
        MEMORY_WORK_ITEM,
        "memory-create-coding",
    );
    assert_eq!(
        passed_job(&coding, "memory create")["entity_type"],
        "coding"
    );

    let clean_all = harness.submit_bound_legacy(
        "memory clean-all",
        &[],
        MEMORY_WORK_ITEM,
        "memory-clean-all",
    );
    let clean_all = passed_job(&clean_all, "memory clean-all");
    assert_eq!(clean_all["cleaned"], true);
    assert_eq!(clean_all["preserved"], json!(["common"]));
    assert!(
        clean_all["removed_types"]
            .as_array()
            .is_some_and(|values| values.len() == 2)
    );

    let missing = harness.submit_bound_legacy(
        "memory read",
        &selector,
        MEMORY_WORK_ITEM,
        "memory-read-cleaned",
    );
    assert_eq!(passed_job(&missing, "memory read")["found"], false);
}

#[test]
fn memory_cli_inputs_and_compiler_fail_closed_at_native_bounds() {
    let root = TempDir::new().expect("tempdir");
    let mut harness = Harness::new(&root);
    prepare_memory_workspace(&root);
    fs::create_dir_all(root.path().join("constraints")).expect("constraints dir");
    fs::write(
        root.path().join("constraints/large-memory-source.md"),
        (0..160)
            .map(|index| format!("must preserve unique constraint number {index:03}\n"))
            .collect::<String>(),
    )
    .expect("large common source");
    fs::write(
        root.path().join("constraints/oversized-memory-source.md"),
        vec![b'x'; 257 * 1024],
    )
    .expect("oversized source");
    fs::write(
        root.path().join("constraints/binary-memory-source.md"),
        [0xff, 0xfe, 0xfd],
    )
    .expect("binary source");
    harness.cut_over_to_canary();

    let empty = harness.submit_bound_legacy(
        "memory create",
        &["--entity-type", "coding", "--entity-id", "EMPTY"],
        MEMORY_EMPTY_WORK_ITEM,
        "memory-empty-create",
    );
    assert_eq!(passed_job(&empty, "memory create")["created"], true);
    let empty_common = harness.submit_bound_legacy(
        "memory common",
        &["read"],
        MEMORY_EMPTY_WORK_ITEM,
        "memory-empty-common",
    );
    assert!(
        passed_job(&empty_common, "memory common")["context"]
            .as_str()
            .is_some_and(|text| text.contains("no reusable constraints"))
    );

    let first_common = harness.submit_bound_legacy(
        "memory common",
        &["update", "--content", "first common value"],
        MEMORY_COMMON_WORK_ITEM,
        "memory-first-common",
    );
    assert_eq!(passed_job(&first_common, "memory common")["revision"], 2);

    let truncated = harness.submit_bound_legacy(
        "memory create",
        &[
            "--entity-type",
            "story",
            "--entity-id",
            "TRUNCATED",
            "--sources",
            "large=constraints/large-memory-source.md",
        ],
        MEMORY_TRUNCATED_WORK_ITEM,
        "memory-truncated-create",
    );
    assert_eq!(passed_job(&truncated, "memory create")["created"], true);
    let truncated_common = harness.submit_bound_legacy(
        "memory common",
        &["read"],
        MEMORY_TRUNCATED_WORK_ITEM,
        "memory-truncated-common",
    );
    let truncated_context = passed_job(&truncated_common, "memory common")["context"]
        .as_str()
        .expect("common context text");
    assert!(truncated_context.len() <= 2 * 1024);
    assert!(truncated_context.contains("truncated at the 2 KiB daemon bound"));

    let relative_project = harness.submit_bound_legacy(
        "memory create",
        &[
            "--entity-type",
            "coding",
            "--entity-id",
            "RELATIVE",
            "--project",
            ".",
            "--sources",
            "marker-only",
        ],
        MEMORY_WORK_ITEM,
        "memory-relative-project",
    );
    assert_eq!(
        passed_job(&relative_project, "memory create")["created"],
        true
    );

    let sources_payload = serde_json::to_string(&json!({
        "entityType":"coding",
        "entityId":"ARRAY-SOURCES",
        "sources":["one=constraints/memory-source.md","marker-only"]
    }))
    .expect("array sources payload");
    let array_sources = harness.submit_bound_legacy(
        "memory create",
        &["--payload-json", &sources_payload],
        MEMORY_WORK_ITEM,
        "memory-array-sources",
    );
    assert_eq!(passed_job(&array_sources, "memory create")["created"], true);

    let context_text =
        serde_json::to_string(&json!({"constraints":["string context"]})).expect("inner context");
    let quoted_context = serde_json::to_string(&context_text).expect("quoted context");
    let string_context = harness.submit_bound_legacy(
        "memory create",
        &[
            "--entity-type",
            "story",
            "--entity-id",
            "STRING-CONTEXT",
            "--context-json",
            &quoted_context,
        ],
        MEMORY_WORK_ITEM,
        "memory-string-context",
    );
    assert_eq!(
        passed_job(&string_context, "memory create")["created"],
        true
    );

    let long_cell = "x".repeat(2_049);
    let large_constraints = (0..10)
        .map(|index| format!("{index}-{}", "y".repeat(1_999)))
        .collect::<Vec<_>>();
    let compiler_cases = [
        json!({"unknownField":true}),
        json!({"series_chain":[],"seriesChain":[]}),
        json!({"deliverables":[{"unknown":"value"}]}),
        json!({"deliverables":[{"name":[]}]}),
        json!({"constraints":[long_cell]}),
        json!({"constraints":large_constraints}),
    ];
    for (index, context) in compiler_cases.into_iter().enumerate() {
        let context = serde_json::to_string(&context).expect("compiler rejection context");
        let completed = harness.submit_bound_legacy(
            "memory create",
            &[
                "--entity-type",
                "story",
                "--entity-id",
                &format!("COMPILER-{index}"),
                "--context-json",
                &context,
            ],
            MEMORY_WORK_ITEM,
            &format!("memory-compiler-error-{index}"),
        );
        assert_error_job(&completed, StableErrorCode::OperationSchemaInvalid);
    }

    let oversized_content = "z".repeat(16 * 1024 + 1);
    let error_cases: Vec<(&str, Vec<&str>, StableErrorCode)> = vec![
        (
            "memory create",
            vec!["--entity-type", "unsupported", "--entity-id", "INVALID"],
            StableErrorCode::OperationSchemaInvalid,
        ),
        (
            "memory create",
            vec![
                "--entity-type",
                "story",
                "--entity-id",
                "PROJECT",
                "--project",
                "..",
            ],
            StableErrorCode::ProjectMismatch,
        ),
        (
            "memory create",
            vec![
                "--entity-type",
                "story",
                "--entity-id",
                "BAD-JSON",
                "--context-json",
                "not-json",
            ],
            StableErrorCode::OperationSchemaInvalid,
        ),
        (
            "memory create",
            vec![
                "--entity-type",
                "story",
                "--entity-id",
                "NON-OBJECT",
                "--context-json",
                "\"[]\"",
            ],
            StableErrorCode::OperationSchemaInvalid,
        ),
        (
            "memory create",
            vec![
                "--entity-type",
                "story",
                "--entity-id",
                "BAD-NAME",
                "--sources",
                "bad/name=memory-source.md",
            ],
            StableErrorCode::OperationSchemaInvalid,
        ),
        (
            "memory create",
            vec![
                "--entity-type",
                "story",
                "--entity-id",
                "TRAVERSAL",
                "--sources",
                "source=../outside.md",
            ],
            StableErrorCode::OperationSchemaInvalid,
        ),
        (
            "memory create",
            vec![
                "--entity-type",
                "story",
                "--entity-id",
                "MISSING-SOURCE",
                "--sources",
                "source=constraints/missing.md",
            ],
            StableErrorCode::ExternalStateConflict,
        ),
        (
            "memory create",
            vec![
                "--entity-type",
                "story",
                "--entity-id",
                "LARGE-SOURCE",
                "--sources",
                "source=constraints/oversized-memory-source.md",
            ],
            StableErrorCode::OperationSchemaInvalid,
        ),
        (
            "memory create",
            vec![
                "--entity-type",
                "story",
                "--entity-id",
                "BINARY-SOURCE",
                "--sources",
                "source=constraints/binary-memory-source.md",
            ],
            StableErrorCode::OperationSchemaInvalid,
        ),
        (
            "memory update",
            vec![
                "--entity-type",
                "coding",
                "--entity-id",
                "RELATIVE",
                "--slice",
                "invalid",
            ],
            StableErrorCode::OperationSchemaInvalid,
        ),
        (
            "memory update",
            vec![
                "--entity-type",
                "coding",
                "--entity-id",
                "RELATIVE",
                "--slice",
                "context",
                "--content",
                &oversized_content,
            ],
            StableErrorCode::OperationSchemaInvalid,
        ),
    ];
    for (index, (id, arguments, code)) in error_cases.iter().enumerate() {
        let completed = harness.submit_bound_legacy(
            id,
            arguments,
            MEMORY_WORK_ITEM,
            &format!("memory-input-error-{index}"),
        );
        assert_error_job(&completed, *code);
    }

    let absent = harness.submit_bound_legacy(
        "memory clean",
        &["--entity-type", "story", "--entity-id", "ABSENT"],
        MEMORY_WORK_ITEM,
        "memory-clean-absent",
    );
    assert_eq!(passed_job(&absent, "memory clean")["cleaned"], false);
}

#[test]
fn misc_cli_jobs_cover_classification_automation_and_evidence_boundaries() {
    let root = TempDir::new().expect("tempdir");
    let mut harness = Harness::new(&root);

    let text_cases = [
        ("# PRD small product requirement", "PRD", "small", "PRD"),
        ("# Story medium change", "DR", "medium", ""),
        ("# Bug fix trivial defect", "Issue", "micro", "BUG"),
        ("config one constant", "Conversation", "micro", "CONFIG"),
        (
            "code review this function",
            "Conversation",
            "micro",
            "CODE_REVIEW",
        ),
        (
            "product requirement medium coding plan",
            "PRD",
            "medium",
            "PRD",
        ),
    ];
    for (index, (text, source, scale, entry_node)) in text_cases.into_iter().enumerate() {
        let completed = harness.submit_workspace_legacy(
            "classify",
            &["--text", text],
            &format!("classify-text-{index}"),
        );
        let result = passed_job(&completed, "classify");
        assert_eq!(result["source"], source);
        assert_eq!(result["scale"], scale);
        if !entry_node.is_empty() {
            assert_eq!(result["entryNode"], entry_node);
        }
        if entry_node == "CODE_REVIEW" {
            assert_eq!(result["analysisRequired"], false);
            assert_eq!(result["nextAction"], "code-review");
            assert_eq!(result["specStrategy"]["needs"], false);
        }
    }

    for (index, (name, contents, source)) in [
        ("feature-prd-notes.md", "plain input", "PRD"),
        ("story-plan.md", "plain input", "DR"),
        ("issue-note.txt", "plain input", "Issue"),
    ]
    .into_iter()
    .enumerate()
    {
        fs::write(root.path().join(name), contents).expect("classification file");
        let completed = harness.submit_workspace_legacy(
            "classify",
            &["--file", name],
            &format!("classify-file-{index}"),
        );
        assert_eq!(passed_job(&completed, "classify")["source"], source);
    }

    for (index, (line_count, expected)) in [(20, "small"), (60, "medium"), (220, "large")]
        .into_iter()
        .enumerate()
    {
        let text = (0..line_count)
            .map(|line| format!("neutral conversation line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let filename = format!("neutral-{line_count}.txt");
        fs::write(root.path().join(&filename), text).expect("line classification file");
        let completed = harness.submit_workspace_legacy(
            "classify",
            &["--file", &filename],
            &format!("classify-lines-{index}"),
        );
        assert_eq!(passed_job(&completed, "classify")["scale"], expected);
    }

    let config = root.path().join(".ae-sdd/config.yaml");
    fs::write(&config, "version: 1\n").expect("default automation config");
    let defaults = harness.submit_workspace_legacy("automation status", &[], "automation-defaults");
    let defaults = passed_job(&defaults, "automation status");
    assert_eq!(defaults["enabled"], false);
    assert_eq!(defaults["reviewerTier"], 3);
    assert_eq!(defaults["automatedReviewPoints"], json!([]));

    fs::write(
        &config,
        "version: 1\nautomation:\n  # retained comment\n  enabled: true # inline\n  reviewerTier: 0\n  preflightInfoCollection: false\n  onConsensusStall: fail\n  automatedReviewPoints: []\nroot: done\n",
    )
    .expect("explicit automation config");
    let explicit = harness.submit_workspace_legacy("automation status", &[], "automation-explicit");
    let explicit = passed_job(&explicit, "automation status");
    assert_eq!(explicit["enabled"], true);
    assert_eq!(explicit["reviewerTier"], 0);
    assert_eq!(explicit["preflightInfoCollection"], false);
    assert_eq!(explicit["onConsensusStall"], "fail");

    for (index, contents) in [
        "automation:\n  enabled: maybe\n",
        "automation:\n  reviewerTier: 11\n",
        "automation:\n  onConsensusStall: continue\n",
        "automation:\n  automatedReviewPoints: [1, invalid]\n",
        "automation:\n  unsupported line\n",
    ]
    .into_iter()
    .enumerate()
    {
        fs::write(&config, contents).expect("invalid automation config");
        let completed = harness.submit_workspace_legacy(
            "automation status",
            &[],
            &format!("automation-error-{index}"),
        );
        assert_error_job(&completed, StableErrorCode::OperationSchemaInvalid);
    }

    let missing = harness.submit_workspace_legacy(
        "evidence lookup",
        &evidence_arguments("STORY-MISSING"),
        "evidence-missing",
    );
    let missing = passed_job(&missing, "evidence lookup");
    assert_eq!(missing["reusable"], false);
    assert_eq!(missing["integrity"], "missing");

    let tampered_directory = root
        .path()
        .join(".auto-engineering/STORY-TAMPERED/evidence");
    fs::create_dir_all(&tampered_directory).expect("tampered evidence directory");
    fs::write(
        tampered_directory.join("manifest.json"),
        serde_json::to_vec(&json!({"contentHash":"sha256:deadbeef","entries":[]}))
            .expect("tampered manifest"),
    )
    .expect("tampered evidence manifest");
    let tampered = harness.submit_workspace_legacy(
        "evidence lookup",
        &evidence_arguments("STORY-TAMPERED"),
        "evidence-tampered",
    );
    assert_failed_job(&tampered);
    assert_eq!(tampered["result"]["integrity"], "unverified-or-tampered");

    fs::write(root.path().join("alt-artifact.txt"), "alternate evidence")
        .expect("alternate artifact");
    let matching_entry = json!({
        "status":"active",
        "reusable":true,
        "exitCode":0,
        "inputFingerprint":"input-1",
        "commandHash":legacy_digest(&json!("cargo test")),
        "toolchainFingerprint":"toolchain-1",
        "freshnessWindowSeconds":3600,
        "startedAt":"2099-01-01T00:00:00Z",
        "artifacts":[{
            "snapshotPath":"alt-artifact.txt",
            "sha256":format!("sha256:{}", ArtifactDigest::digest(b"alternate evidence"))
        }]
    });
    write_evidence_manifest(
        &root,
        "prefix-STORY-ALTERNATE",
        json!({"schemaVersion":1,"entries":[matching_entry]}),
    );
    let alternate = harness.submit_workspace_legacy(
        "evidence lookup",
        &evidence_arguments("STORY-ALTERNATE"),
        "evidence-alternate",
    );
    let alternate = passed_job(&alternate, "evidence lookup");
    assert_eq!(alternate["reusable"], true);
    assert!(
        alternate["manifestPath"]
            .as_str()
            .is_some_and(|path| path.contains("prefix-STORY-ALTERNATE"))
    );

    for prefix in ["one", "two"] {
        write_evidence_manifest(
            &root,
            &format!("{prefix}-STORY-AMBIGUOUS"),
            json!({"schemaVersion":1,"entries":[]}),
        );
    }
    let ambiguous = harness.submit_workspace_legacy(
        "evidence lookup",
        &evidence_arguments("STORY-AMBIGUOUS"),
        "evidence-ambiguous",
    );
    assert_error_job(&ambiguous, StableErrorCode::ScopeAmbiguous);

    write_evidence_manifest(
        &root,
        "STORY-NONREUSABLE",
        json!({
            "schemaVersion":1,
            "entries":[
                {
                    "status":"superseded","reusable":true,"exitCode":0,
                    "inputFingerprint":"input-1",
                    "commandHash":legacy_digest(&json!("cargo test")),
                    "toolchainFingerprint":"toolchain-1","artifacts":[]
                },
                {
                    "status":"active","reusable":true,"exitCode":0,
                    "inputFingerprint":"input-1",
                    "commandHash":legacy_digest(&json!("cargo test")),
                    "toolchainFingerprint":"toolchain-1",
                    "freshnessWindowSeconds":0,"artifacts":[]
                },
                {
                    "status":"active","reusable":true,"exitCode":0,
                    "inputFingerprint":"input-1",
                    "commandHash":legacy_digest(&json!("cargo test")),
                    "toolchainFingerprint":"toolchain-1",
                    "freshnessWindowSeconds":3600,"startedAt":"2099-01-01T00:00:00Z",
                    "artifacts":[{"path":"alt-artifact.txt","sha256":"sha256:0000"}]
                }
            ]
        }),
    );
    let nonreusable = harness.submit_workspace_legacy(
        "evidence lookup",
        &evidence_arguments("STORY-NONREUSABLE"),
        "evidence-nonreusable",
    );
    assert_eq!(
        passed_job(&nonreusable, "evidence lookup")["reusable"],
        false
    );

    write_evidence_manifest(
        &root,
        "STORY-INVALID-FRESHNESS",
        json!({
            "schemaVersion":1,
            "entries":[{
                "status":"active","reusable":true,"exitCode":0,
                "inputFingerprint":"input-1",
                "commandHash":legacy_digest(&json!("cargo test")),
                "toolchainFingerprint":"toolchain-1",
                "freshnessWindowSeconds":60,"artifacts":[]
            }]
        }),
    );
    let invalid_freshness = harness.submit_workspace_legacy(
        "evidence lookup",
        &evidence_arguments("STORY-INVALID-FRESHNESS"),
        "evidence-invalid-freshness",
    );
    assert_error_job(&invalid_freshness, StableErrorCode::OperationSchemaInvalid);
}

#[test]
fn job_adapter_and_daemon_fail_closed_on_schema_bounds_and_identity() {
    let root = TempDir::new().expect("tempdir");
    let mut harness = Harness::new(&root);
    let route = legacy::resolve_command_id("assets query").expect("assets query route");

    let missing_workspace = parse_route(&route, &["service", "--idempotency-key", "missing-ws"])
        .expect_err("workspace identity is required");
    assert!(missing_workspace.to_string().contains("workspace identity"));
    let missing_idempotency = parse_route(
        &route,
        &["service", "--workspace-id", &harness.workspace.workspace_id],
    )
    .expect_err("idempotency identity is required");
    assert!(missing_idempotency.to_string().contains("idempotency key"));

    for invalid in [
        vec!["service", "--mystery", "value"],
        vec!["service", "--top", "101"],
        vec!["service", "--query", "duplicate"],
    ] {
        let mut request = parsed_job_request(&route, &harness.workspace.workspace_id, &invalid);
        legacy::adapt_job_submission(&route, "assets.query", &mut request, NOW_MS)
            .expect_err("invalid command schema must fail before IPC");
    }

    for (command_id, business) in [
        ("db profiles", vec!["--init"]),
        (
            "db query",
            vec!["--profile", "local", "--sql", "SELECT 1", "--write"],
        ),
    ] {
        let mutation_route = legacy::resolve_command_id(command_id).expect("mutation route");
        let (_, entrypoint) = job_target(&mutation_route);
        let mut request =
            parsed_job_request(&mutation_route, &harness.workspace.workspace_id, &business);
        let error =
            legacy::adapt_job_submission(&mutation_route, &entrypoint, &mut request, NOW_MS)
                .expect_err("mutating legacy option must fail before IPC");
        assert!(error.to_string().contains("mutating"));
    }

    let oversized = "x".repeat(65_537);
    let error = parse_route(
        &route,
        &[
            oversized.as_str(),
            "--workspace-id",
            &harness.workspace.workspace_id,
            "--idempotency-key",
            "oversized",
        ],
    )
    .expect_err("oversized argv value must fail");
    assert!(error.to_string().contains("value budget"));

    let mut unknown_wrapper = params(
        json!({
            "entrypoint":"assets.query",
            "arguments":{"query":"service"},
            "deadlineUnixMs":NOW_MS + 1_000,
            "unknown":true,
        }),
        1_000,
    );
    unknown_wrapper.workspace_id = Some(harness.workspace.workspace_id.clone());
    unknown_wrapper.idempotency_key = Some("unknown-wrapper".to_owned());
    let response = raw_call(
        &harness.runtime,
        &mut harness.connection,
        RpcMethod::JobSubmit,
        serde_json::to_value(unknown_wrapper).expect("unknown wrapper JSON"),
    );
    assert_eq!(
        stable_code(&response),
        StableErrorCode::OperationSchemaInvalid
    );

    let mut forged = parsed_job_request(&route, "forged-workspace", &["service"]);
    legacy::adapt_job_submission(&route, "assets.query", &mut forged, NOW_MS)
        .expect("valid job adapter");
    let response = raw_call(
        &harness.runtime,
        &mut harness.connection,
        RpcMethod::JobSubmit,
        serde_json::to_value(forged).expect("forged params JSON"),
    );
    assert_eq!(stable_code(&response), StableErrorCode::ProjectMismatch);
}

fn parse_route(
    route: &legacy::LegacyCommandRoute,
    trailing: &[&str],
) -> Result<legacy::LegacyRpcInvocation, legacy::LegacyArgumentError> {
    legacy::parse_rpc_invocation(
        route,
        RpcMethod::JobSubmit,
        &trailing
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>(),
        |_| None,
    )
}

fn parsed_job_request(
    route: &legacy::LegacyCommandRoute,
    workspace_id: &str,
    business: &[&str],
) -> RequestParams<Value> {
    let mut trailing = business
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    trailing.extend([
        "--workspace-id".to_owned(),
        workspace_id.to_owned(),
        "--idempotency-key".to_owned(),
        format!("negative-{}", trailing.len()),
    ]);
    match legacy::parse_rpc_invocation(route, RpcMethod::JobSubmit, &trailing, |_| None)
        .expect("generic legacy job params")
        .request
    {
        legacy::LegacyRequestSource::Synthesized(request) => *request,
        legacy::LegacyRequestSource::ExplicitJson(_) => panic!("synthesized request expected"),
    }
}

fn job_target(route: &legacy::LegacyCommandRoute) -> (RpcMethod, String) {
    match &route.target {
        legacy::LegacyTarget::Rpc {
            method,
            adapter: legacy::LegacyRpcAdapter::JobSubmission { entrypoint, .. },
        } => (*method, entrypoint.clone()),
        target => panic!("not a job route: {target:?}"),
    }
}

fn prepare_memory_workspace(root: &TempDir) {
    let state_directory = root.path().join(".auto-engineering/legacy-memory-e2e");
    fs::create_dir_all(&state_directory).expect("memory state directory");
    fs::write(
        state_directory.join("state.json"),
        serde_json::to_vec_pretty(&json!({
            "stateMachineName":"PRD-MEMORY-E2E",
            "activeStory":MEMORY_WORK_ITEM,
            "revision":1,
            "lastFencingToken":0,
            "scale":"small",
            "selectedDesign":"Story",
            "phase":"coding",
            "currentPhase":"coding",
            "storyStates":{
                MEMORY_WORK_ITEM:{
                    "phase":"coding",
                    "currentPhase":"coding",
                    "currentStep":"coding",
                    "completedSteps":[],
                    "pendingOutputs":[],
                    "codingRound":1
                },
                MEMORY_EMPTY_WORK_ITEM:{
                    "phase":"coding",
                    "currentPhase":"coding"
                },
                MEMORY_COMMON_WORK_ITEM:{
                    "phase":"coding",
                    "currentPhase":"coding"
                },
                MEMORY_TRUNCATED_WORK_ITEM:{
                    "phase":"coding",
                    "currentPhase":"coding"
                }
            },
            "documentPaths":{"STORY":"memory-story.md"}
        }))
        .expect("memory state JSON"),
    )
    .expect("memory state file");
    fs::write(root.path().join("memory-story.md"), "# Story\n").expect("memory Story");
    fs::create_dir_all(root.path().join("constraints")).expect("constraints dir");
    fs::write(
        root.path().join("constraints/memory-source.md"),
        "must use durable receipts\nordinary transcript content\n",
    )
    .expect("memory source");
    fs::write(
        root.path().join("constraints/memory-content.md"),
        "needle content loaded through a bounded granted file\n",
    )
    .expect("memory content");
}

fn evidence_arguments(story: &str) -> [&str; 8] {
    [
        "--story",
        story,
        "--command",
        "cargo test",
        "--input-fingerprint",
        "input-1",
        "--toolchain-fingerprint",
        "toolchain-1",
    ]
}

fn write_evidence_manifest(root: &TempDir, directory: &str, mut manifest: Value) {
    manifest["contentHash"] = json!(legacy_digest(&manifest));
    let evidence = root
        .path()
        .join(".auto-engineering")
        .join(directory)
        .join("evidence");
    fs::create_dir_all(&evidence).expect("evidence directory");
    fs::write(
        evidence.join("manifest.json"),
        serde_json::to_vec(&manifest).expect("evidence manifest JSON"),
    )
    .expect("evidence manifest");
}

fn legacy_digest(value: &Value) -> String {
    format!(
        "sha256:{}",
        ArtifactDigest::digest(serde_json::to_vec(value).expect("canonical JSON"))
    )
}

fn connection(runtime: &RuntimeService, client_kind: ClientKind) -> ConnectionState {
    let mut connection = ConnectionState::default();
    let handshake = HandshakeRequest {
        protocol_range: PROTOCOL_RANGE_V1.to_owned(),
        client_build: "legacy-job-e2e".to_owned(),
        client_kind,
        endpoint_token: SecretString::new(ENDPOINT_TOKEN.to_owned()),
        expected_boot_id: runtime.boot_id().to_string(),
        expected_policy_digest: runtime.policy_digest().to_owned(),
    };
    assert_result(raw_call(
        runtime,
        &mut connection,
        RpcMethod::RuntimeHandshake,
        serde_json::to_value(handshake).expect("handshake JSON"),
    ));
    connection
}

fn confirmation() -> ConfirmationRef {
    ConfirmationRef {
        confirmation_id: "legacy-job-e2e-confirmation".to_owned(),
        approved_by: "test-user".to_owned(),
        approved_at: "2026-01-01T00:00:00Z".to_owned(),
    }
}

fn passed_job<'a>(completed: &'a Value, id: &str) -> &'a Value {
    assert_eq!(completed["status"], "pass", "{id}: {completed}");
    assert_eq!(completed["result"]["outcome"], "PASS", "{id}");
    &completed["result"]
}

fn assert_error_job(completed: &Value, code: StableErrorCode) {
    assert_eq!(completed["status"], "error", "{completed}");
    assert_eq!(
        completed["errorCode"],
        serde_json::to_value(code).expect("stable error code serializes"),
        "{completed}"
    );
}

fn assert_failed_job(completed: &Value) {
    assert_eq!(completed["status"], "fail", "{completed}");
    assert_eq!(completed["result"]["outcome"], "FAIL", "{completed}");
}

fn params(payload: Value, deadline_ms: u64) -> RequestParams<Value> {
    RequestParams {
        protocol_version: PROTOCOL_VERSION_V1.to_owned(),
        workspace_id: None,
        agent_id: None,
        session_id: None,
        capability_token: None,
        turn_id: None,
        work_item_id: None,
        lease_id: None,
        fencing_token: None,
        expected_revision: None,
        idempotency_key: None,
        confirmation: None,
        deadline_ms,
        payload,
    }
}

fn raw_call(
    runtime: &RuntimeService,
    connection: &mut ConnectionState,
    method: RpcMethod,
    params: Value,
) -> Value {
    let request = JsonRpcRequest::new(format!("{}-e2e", method.as_str()), method, params);
    serde_json::from_slice(&runtime.handle_payload(
        connection,
        &serde_json::to_vec(&request).expect("request JSON"),
    ))
    .expect("response JSON")
}

fn assert_result(response: Value) -> Value {
    response
        .get("result")
        .cloned()
        .unwrap_or_else(|| panic!("RPC failed: {response}"))
}

fn stable_code(response: &Value) -> StableErrorCode {
    serde_json::from_value(response["error"]["data"]["stableCode"].clone())
        .unwrap_or_else(|_| panic!("missing stable code: {response}"))
}
