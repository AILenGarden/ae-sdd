use std::fs;
use std::process::Command;

use ae_sdd_domain::ArtifactDigest;
use rusqlite::Connection;
use serde_json::{Value, json};
use tempfile::TempDir;

pub(crate) fn prepare_workspace(root: &TempDir) {
    let ae_sdd = root.path().join(".ae-sdd");
    fs::create_dir_all(ae_sdd.join("assets/legacy-e2e")).expect("assets directory");
    fs::write(
        ae_sdd.join("assets/legacy-e2e/legacy-e2e.assets.md"),
        "# Project\n## \u{00a7}A Outline\nservice contract\n## \u{00a7}B Modules\nworker\n## \u{00a7}C Fields\nid\n## \u{00a7}D Components\nruntime\n## \u{00a7}E API\njob.submit\n## \u{00a7}F Keywords\ndaemon\n## \u{00a7}G Read API\nquery\n",
    )
    .expect("asset file");
    fs::write(
        ae_sdd.join("config.yaml"),
        "version: 1\nprojectKey: legacy-e2e\nautomation:\n  enabled: false\n  reviewerTier: 3\n  preflightInfoCollection: true\n  onConsensusStall: pause\n  automatedReviewPoints: [1, 1.5, 2]\n  enabledAt: \"\"\n",
    )
    .expect("automation config");
    fs::write(
        root.path().join("scan-report.json"),
        serde_json::to_vec(&json!({
            "findings":[{
                "findingKey":"finding-1",
                "ruleId":"R1",
                "path":"tracked.txt",
                "severity":"WARNING"
            }]
        }))
        .expect("scan report JSON"),
    )
    .expect("scan report file");
    prepare_baseline(root);
    prepare_database(root);
    prepare_evidence(root);
    prepare_perf(root);
    prepare_plugin(root);
    prepare_git(root);
}

fn prepare_baseline(root: &TempDir) {
    let directory = root.path().join(".ae-sdd/baselines");
    fs::create_dir_all(&directory).expect("baseline directory");
    let mut baseline = json!({
        "schemaVersion":1,
        "gateId":"G-CODE-1",
        "findings":[{"findingKey":"finding-1","ruleId":"R1","path":"tracked.txt","symbol":null,"severity":"WARNING"}]
    });
    baseline["contentHash"] = json!(legacy_json_digest(&baseline));
    fs::write(
        directory.join("G-CODE-1.json"),
        serde_json::to_vec(&baseline).expect("baseline JSON"),
    )
    .expect("baseline file");
}

fn prepare_database(root: &TempDir) {
    let directory = root.path().join(".ae-sdd/secrets");
    fs::create_dir_all(&directory).expect("secrets directory");
    let database = root.path().join("fixture.sqlite3");
    Connection::open(&database)
        .expect("fixture database")
        .execute_batch(
            "CREATE TABLE item(id INTEGER PRIMARY KEY,name TEXT); \
             INSERT INTO item(name) VALUES('one');",
        )
        .expect("fixture schema");
    fs::write(
        directory.join("db-connections.local.json"),
        serde_json::to_vec(&json!({
            "profiles":[{
                "name":"local","driver":"sqlite","database":database,"readonly":true
            }]
        }))
        .expect("profiles JSON"),
    )
    .expect("profiles file");
}

fn prepare_evidence(root: &TempDir) {
    let directory = root
        .path()
        .join(".auto-engineering/STORY-EVIDENCE-001/evidence");
    fs::create_dir_all(&directory).expect("evidence directory");
    fs::write(root.path().join("artifact.txt"), "verified artifact").expect("artifact");
    let mut manifest = json!({
        "schemaVersion":1,
        "storyId":"STORY-EVIDENCE-001",
        "entries":[{
            "evidenceId":"evidence-1","status":"active","reusable":true,"exitCode":0,
            "inputFingerprint":"input-1","commandHash":legacy_json_digest(&json!("cargo test")),
            "toolchainFingerprint":"toolchain-1",
            "artifacts":[{
                "path":"artifact.txt",
                "sha256":format!("sha256:{}", ArtifactDigest::digest(b"verified artifact"))
            }]
        }]
    });
    manifest["contentHash"] = json!(legacy_json_digest(&manifest));
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
            "schema":"ae-sdd.runtimeStats.v1","command":"gates check","durationMs":12.0,
            "cpuMs":3.0,"bootstrapMs":10.0,
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
        "schema_version: 1\nplugins:\n  - name: fixture\n    type: skill-new\n    version: 1.0.0\n    description: isolated differential fixture\n    provides: fixture-skill\n    path: ./fixture/SKILL.md\n",
    )
    .expect("plugin registry");
}

fn prepare_git(root: &TempDir) {
    fs::write(
        root.path().join(".gitignore"),
        ".ae-sdd/\n.auto-engineering/\nartifact.txt\nfixture.sqlite3\nscan-report.json\n",
    )
    .expect("gitignore file");
    fs::write(root.path().join("tracked.txt"), "tracked\n").expect("tracked file");
    for arguments in [
        vec!["init", "--quiet"],
        vec!["config", "core.autocrlf", "false"],
        vec!["add", ".gitignore", "tracked.txt"],
        vec![
            "-c",
            "user.name=ae-sdd-test",
            "-c",
            "user.email=ae-sdd@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ],
    ] {
        let status = Command::new("git")
            .args(arguments)
            .current_dir(root.path())
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
            .status()
            .expect("git process");
        assert!(status.success());
    }
}

fn json_digest(value: &Value) -> String {
    ArtifactDigest::digest(serde_json::to_vec(value).expect("canonical JSON")).to_string()
}

fn legacy_json_digest(value: &Value) -> String {
    format!("sha256:{}", json_digest(value))
}
