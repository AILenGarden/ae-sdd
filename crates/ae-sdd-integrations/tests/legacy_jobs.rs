use std::fs;
use std::process::Command;
use std::sync::Arc;

use ae_sdd_domain::{ArtifactDigest, BootId, EventStoreId};
use ae_sdd_integrations::NativeBusinessAdapter;
use ae_sdd_protocol::{StableErrorCode, WorkspaceMode};
use ae_sdd_runtime::{
    BusinessOperationPort, BusinessWorkspace, MemoryPersistence, PersistencePort,
};
use rusqlite::Connection;
use serde_json::json;
use tempfile::TempDir;
use uuid::Uuid;

fn adapter(root: &TempDir) -> NativeBusinessAdapter {
    let event_store_id = EventStoreId::from_uuid(Uuid::from_u128(41));
    let persistence: Arc<dyn PersistencePort> = Arc::new(MemoryPersistence::new(event_store_id));
    NativeBusinessAdapter::new(
        root.path().join("runtime.sqlite3"),
        event_store_id,
        BootId::from_uuid(Uuid::from_u128(42)),
        "0".repeat(64),
        persistence,
    )
}

fn workspace(root: &TempDir, mode: WorkspaceMode) -> BusinessWorkspace {
    BusinessWorkspace {
        workspace_id: Uuid::from_u128(43).to_string(),
        canonical_root: fs::canonicalize(root.path())
            .expect("workspace canonical path")
            .to_string_lossy()
            .into_owned(),
        project_key: "legacy-job-test".to_owned(),
        mode,
        agent_role: None,
        agent_grant: None,
        caller_kind: None,
        inventory_generation: 1,
    }
}

#[test]
fn native_asset_jobs_read_only_the_registered_project_asset() {
    let root = TempDir::new().expect("tempdir");
    let assets = root.path().join(".ae-sdd/assets");
    fs::create_dir_all(&assets).expect("assets directory");
    fs::write(
        assets.join("legacy-job-test.assets.md"),
        "# Project\n## §A Outline\nservice contract\n## §B Modules\nworker\n## §C Fields\nid\n## §D Components\nruntime\n## §E API\njob.submit\n## §F Keywords\ndaemon\n## §G Read API\nquery\n",
    )
    .expect("asset file");
    let adapter = adapter(&root);
    let workspace = workspace(&root, WorkspaceMode::Shadow);

    let check = adapter
        .execute_job(&workspace, "assets.check", &json!({}))
        .expect("asset check");
    assert_eq!(check["outcome"], "PASS");
    assert_eq!(check["missingSections"], json!([]));

    let query = adapter
        .execute_job(
            &workspace,
            "assets.query",
            &json!({"query":"service contract","top":5}),
        )
        .expect("asset query");
    assert_eq!(query["outcome"], "PASS");
    assert!(query["nHits"].as_u64().expect("hit count") > 0);
}

#[test]
fn asset_file_symlink_or_absolute_escape_is_denied() {
    let root = TempDir::new().expect("workspace tempdir");
    let outside = TempDir::new().expect("outside tempdir");
    let outside_file = outside.path().join("outside.assets.md");
    fs::write(&outside_file, "# outside").expect("outside file");
    let error = adapter(&root)
        .execute_job(
            &workspace(&root, WorkspaceMode::Shadow),
            "assets.stats",
            &json!({"assetFile":outside_file}),
        )
        .expect_err("outside asset must fail closed");
    assert_eq!(error.code(), StableErrorCode::WorkspaceOutsideAllowedRoot);
}

#[test]
fn sqlite_job_is_read_only_at_policy_and_connection_layers() {
    let root = TempDir::new().expect("tempdir");
    let secrets = root.path().join(".ae-sdd/secrets");
    fs::create_dir_all(&secrets).expect("secrets directory");
    let database = root.path().join("fixture.sqlite3");
    let connection = Connection::open(&database).expect("fixture database");
    connection
        .execute_batch("CREATE TABLE item(id INTEGER PRIMARY KEY,name TEXT); INSERT INTO item(name) VALUES('one');")
        .expect("fixture rows");
    drop(connection);
    fs::write(
        secrets.join("db-connections.local.json"),
        serde_json::to_vec_pretty(&json!({
            "profiles":[{
                "name":"local",
                "driver":"sqlite",
                "database":database,
                "readonly":true
            }]
        }))
        .expect("profiles JSON"),
    )
    .expect("profiles file");
    let adapter = adapter(&root);
    let workspace = workspace(&root, WorkspaceMode::RustSoleWriter);

    let result = adapter
        .execute_job(
            &workspace,
            "db.query",
            &json!({"profile":"local","sql":"SELECT id,name FROM item","limit":10}),
        )
        .expect("read-only query");
    assert_eq!(result["outcome"], "PASS");
    assert_eq!(result["rows"][0]["name"], "one");

    let error = adapter
        .execute_job(
            &workspace,
            "db.query",
            &json!({"profile":"local","sql":"DELETE FROM item"}),
        )
        .expect_err("write SQL must fail closed");
    assert_eq!(error.code(), StableErrorCode::OperationSchemaInvalid);
}

#[test]
fn mutation_jobs_never_bypass_mode_or_lease_authority() {
    let root = TempDir::new().expect("tempdir");
    let adapter = adapter(&root);
    let mutations = [
        "automation.disable",
        "automation.enable",
        "baseline.create",
        "doc.finalize",
        "perf.clear",
        "preflight.collect",
        "state.bind-story-doc",
        "state.new",
        "state.prd-archive",
        "state.prd-check-complete",
        "state.prd-complete",
        "state.prd-init",
        "state.register-review-consensus",
        "state.relocate",
        "state.write",
    ];
    for entrypoint in mutations {
        let shadow = adapter
            .execute_job(
                &workspace(&root, WorkspaceMode::Shadow),
                entrypoint,
                &json!({}),
            )
            .expect_err("shadow mutation must be forbidden");
        assert_eq!(shadow.code(), StableErrorCode::RoleOperationForbidden);

        let sole_writer = adapter
            .execute_job(
                &workspace(&root, WorkspaceMode::RustSoleWriter),
                entrypoint,
                &json!({}),
            )
            .expect_err("writer mutation still needs the typed lease envelope");
        assert_eq!(sole_writer.code(), StableErrorCode::LeaseRequired);
    }
}

#[test]
fn metadata_analysis_and_evidence_jobs_execute_native_read_paths() {
    let root = TempDir::new().expect("tempdir");
    prepare_automation(&root);
    prepare_baseline(&root);
    prepare_evidence(&root);
    prepare_perf(&root);
    prepare_plugin(&root);
    let adapter = adapter(&root);
    let workspace = workspace(&root, WorkspaceMode::Shadow);

    let automation = adapter
        .execute_job(&workspace, "automation.status", &json!({}))
        .expect("automation status");
    assert_eq!(automation["outcome"], "PASS");
    assert_eq!(automation["reviewerTier"], 3);

    let classification = adapter
        .execute_job(
            &workspace,
            "classify",
            &json!({"text":"large cross-module architecture migration"}),
        )
        .expect("classification");
    assert_eq!(classification["scale"], "large");
    assert_eq!(classification["multiAgent"], true);

    let baseline = adapter
        .execute_job(
            &workspace,
            "baseline.diff",
            &json!({
                "gate":"G-CODE-1",
                "report":{"findings":[{
                    "findingKey":"finding-1",
                    "ruleId":"R1",
                    "path":"src/lib.rs",
                    "severity":"WARNING"
                }]}
            }),
        )
        .expect("baseline diff");
    assert_eq!(baseline["outcome"], "PASS");
    assert_eq!(baseline["status"], "PASS_WITH_BASELINE_DEBT");

    let evidence = adapter
        .execute_job(
            &workspace,
            "evidence.lookup",
            &json!({
                "story":"STORY-EVIDENCE-001",
                "command":"cargo test",
                "inputFingerprint":"input-1",
                "toolchainFingerprint":"toolchain-1"
            }),
        )
        .expect("evidence lookup");
    assert_eq!(evidence["outcome"], "PASS");
    assert_eq!(evidence["reusable"], true);

    let perf = adapter
        .execute_job(&workspace, "perf.doctor", &json!({"last":10,"limit":5}))
        .expect("perf doctor");
    assert_eq!(perf["outcome"], "PASS");
    assert_eq!(perf["summary"]["count"], 1);

    let plugins = adapter
        .execute_job(&workspace, "plugin.validate", &json!({}))
        .expect("plugin validate");
    assert_eq!(plugins["outcome"], "PASS");
    assert_eq!(plugins["totalPlugins"], 1);
    let trace = adapter
        .execute_job(
            &workspace,
            "plugin.trace",
            &json!({"target":"fixture-skill"}),
        )
        .expect("plugin trace");
    assert_eq!(trace["hit"], true);
    assert_eq!(trace["layer"], "project");
}

#[test]
fn git_status_uses_the_bounded_direct_process_adapter() {
    let root = TempDir::new().expect("tempdir");
    let status = Command::new("git")
        .arg("init")
        .arg("--quiet")
        .arg(root.path())
        .status()
        .expect("git executable");
    assert!(status.success());
    fs::write(root.path().join("untracked.txt"), "content").expect("untracked file");

    let result = adapter(&root)
        .execute_job(
            &workspace(&root, WorkspaceMode::Shadow),
            "git.status",
            &json!({}),
        )
        .expect("git status");
    assert_eq!(result["outcome"], "PASS");
    assert_eq!(result["clean"], false);
    assert!(
        result["entries"]
            .as_array()
            .expect("status entries")
            .iter()
            .any(|entry| entry["path"] == "untracked.txt")
    );
}

fn prepare_automation(root: &TempDir) {
    fs::create_dir_all(root.path().join(".ae-sdd")).expect("ae-sdd directory");
    fs::write(
        root.path().join(".ae-sdd/config.yaml"),
        "version: 1\nautomation:\n  enabled: false\n  reviewerTier: 3\n  preflightInfoCollection: true\n  onConsensusStall: pause\n  automatedReviewPoints: [1, 1.5, 2]\n  enabledAt: \"\"\n",
    )
    .expect("automation config");
}

fn prepare_baseline(root: &TempDir) {
    let directory = root.path().join(".ae-sdd/baselines");
    fs::create_dir_all(&directory).expect("baseline directory");
    let mut baseline = json!({
        "schemaVersion":1,
        "gateId":"G-CODE-1",
        "rulesetFingerprint":"ruleset-1",
        "findings":[{
            "findingKey":"finding-1",
            "ruleId":"R1",
            "path":"src/lib.rs",
            "symbol":null,
            "severity":"WARNING"
        }]
    });
    baseline["contentHash"] = json!(json_digest(&baseline));
    fs::write(
        directory.join("G-CODE-1.json"),
        serde_json::to_vec(&baseline).expect("baseline JSON"),
    )
    .expect("baseline file");
}

fn prepare_evidence(root: &TempDir) {
    let directory = root
        .path()
        .join(".auto-engineering/STORY-EVIDENCE-001/evidence");
    fs::create_dir_all(&directory).expect("evidence directory");
    fs::write(root.path().join("artifact.txt"), "verified artifact").expect("artifact");
    let command_hash = json_digest(&json!("cargo test"));
    let artifact_hash = ArtifactDigest::digest(b"verified artifact").to_string();
    let mut manifest = json!({
        "schemaVersion":1,
        "storyId":"STORY-EVIDENCE-001",
        "entries":[{
            "evidenceId":"evidence-1",
            "status":"active",
            "reusable":true,
            "exitCode":0,
            "inputFingerprint":"input-1",
            "commandHash":command_hash,
            "toolchainFingerprint":"toolchain-1",
            "artifacts":[{"path":"artifact.txt","sha256":artifact_hash}]
        }]
    });
    manifest["contentHash"] = json!(json_digest(&manifest));
    fs::write(
        directory.join("manifest.json"),
        serde_json::to_vec(&manifest).expect("manifest JSON"),
    )
    .expect("manifest file");
}

fn prepare_perf(root: &TempDir) {
    let directory = root.path().join(".ae-sdd/runtime-stats");
    fs::create_dir_all(&directory).expect("stats directory");
    fs::write(
        directory.join("2026-07-23.jsonl"),
        serde_json::to_string(&json!({
            "schema":"ae-sdd.runtimeStats.v1",
            "command":"gates check",
            "durationMs":12.0,
            "cpuMs":3.0,
            "bootstrapMs":10.0,
            "spans":[{"name":"gate","durationMs":5.0,"cpuMs":1.0,"attrs":{}}]
        }))
        .expect("stats JSON")
            + "\n",
    )
    .expect("stats file");
}

fn prepare_plugin(root: &TempDir) {
    let directory = root.path().join(".ae-sdd/plugins/fixture");
    fs::create_dir_all(&directory).expect("plugin directory");
    fs::write(directory.join("SKILL.md"), "# fixture").expect("plugin skill");
    fs::write(
        root.path().join(".ae-sdd/plugins/registry.yaml"),
        "schema_version: 1\nplugins:\n  - name: fixture\n    type: skill-new\n    version: 1.0.0\n    description: native job fixture\n    provides: fixture-skill\n    path: ./fixture/SKILL.md\n",
    )
    .expect("plugin registry");
}

fn json_digest(value: &serde_json::Value) -> String {
    ArtifactDigest::digest(serde_json::to_vec(value).expect("canonical JSON")).to_string()
}
