//! Append-only evidence ledger authority tests.
//!
//! The evidence truth is `.auto-engineering/{storyId}/evidence/ledger.jsonl`:
//! one canonical JSON event per line forming a hash chain. `manifest.json` is a
//! deterministic active projection sealed with `contentHash`, and project state
//! only stores the ledger/manifest locators and digests. These tests drive the
//! real `NativeBusinessAdapter` so every append passes the project mutation
//! journal; they cover append, supersede and finalize events, hash-chain
//! tamper detection, deterministic manifest rebuild, and legacy manifests that
//! predate the ledger.

use std::fs;
use std::sync::Arc;

use ae_sdd_contracts::{EvidenceLedgerEventKind, EvidenceLedgerEventV1};
use ae_sdd_domain::{AgentRole, ArtifactDigest, OperationId, ProjectPathScope, ScopedGrant};
use ae_sdd_integrations::{NativeBusinessAdapter, SqliteRuntimePersistence};
use ae_sdd_operations::OperationName;
use ae_sdd_protocol::{
    PROTOCOL_VERSION_V1, RequestParams, RpcMethod, StableErrorCode, WorkspaceMode,
};
use ae_sdd_runtime::{BusinessOperationPort, BusinessWorkspace, PersistencePort, RuntimeError};
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

const WORK_ITEM_ID: &str = "STORY-LEDGER-001";
const PROJECT_KEY: &str = "evidence-ledger";
const EVIDENCE_DIR: &str = ".auto-engineering/STORY-LEDGER-001/evidence";
const POLICY_DIGEST: &str = "4444444444444444444444444444444444444444444444444444444444444444";
const INPUT_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const INPUT_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

struct Fixture {
    root: TempDir,
    _runtime: TempDir,
    adapter: NativeBusinessAdapter,
}

struct Lease {
    lease_id: String,
    fencing_token: u64,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("workspace tempdir");
        let runtime = TempDir::new().expect("runtime tempdir");
        write_file(
            &root,
            ".auto-engineering/evidence-ledger/state.json",
            &state_bytes(7),
        );
        let database = runtime.path().join("runtime.sqlite3");
        let persistence =
            Arc::new(SqliteRuntimePersistence::open(&database).expect("runtime persistence"));
        let event_store_id = persistence.event_store_id().expect("event store id");
        let port: Arc<dyn PersistencePort> = persistence.clone();
        let adapter = NativeBusinessAdapter::new(
            database,
            event_store_id,
            ae_sdd_domain::BootId::from_uuid(Uuid::from_u128(15)),
            POLICY_DIGEST.to_owned(),
            port,
        );
        Self {
            root,
            _runtime: runtime,
            adapter,
        }
    }

    fn workspace(&self) -> BusinessWorkspace {
        self.workspace_with_role(AgentRole::Task)
    }

    fn workspace_with_role(&self, role: AgentRole) -> BusinessWorkspace {
        let grant = ScopedGrant::new(
            OperationName::ALL
                .into_iter()
                .filter(|operation| *operation != OperationName::LeaseBreak)
                .map(|operation| OperationId::new(operation.as_str()).expect("operation id")),
            [],
            [ProjectPathScope::ProjectRoot],
        );
        BusinessWorkspace {
            workspace_id: Uuid::from_u128(21).to_string(),
            canonical_root: self.root.path().to_string_lossy().into_owned(),
            project_key: PROJECT_KEY.to_owned(),
            mode: WorkspaceMode::RustCanary,
            agent_role: Some(role),
            agent_grant: Some(grant),
            caller_kind: None,
            inventory_generation: 5,
        }
    }

    fn execute(&self, operation: &str, payload: Value, key: &str) -> Result<Value, RuntimeError> {
        let request = self.request(operation, payload, key);
        self.adapter.execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&self.workspace()),
        )
    }

    fn acquire_lease(&self, key: &str) -> Lease {
        let response = self
            .execute(
                "lease.acquire",
                json!({"owner":{"role":"task"},"ttlSeconds":300}),
                key,
            )
            .expect("lease acquires");
        Lease {
            lease_id: response["data"]["leaseId"]
                .as_str()
                .expect("lease id")
                .to_owned(),
            fencing_token: response["data"]["fencingToken"]
                .as_u64()
                .expect("fencing token"),
        }
    }

    fn record(&self, lease: &Lease, key: &str, payload: Value) -> Result<Value, RuntimeError> {
        let mut request = self.request("evidence.record", payload, key);
        bind_write(&mut request, lease, self.current_revision());
        self.adapter.execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&self.workspace()),
        )
    }

    fn finalize(&self, lease: &Lease, key: &str) -> Result<Value, RuntimeError> {
        let mut request = self.request("evidence.finalize", json!({}), key);
        bind_write(&mut request, lease, self.current_revision());
        self.adapter.execute(
            RpcMethod::OperationExecute,
            &request,
            Some(&self.workspace()),
        )
    }

    fn request(&self, operation: &str, payload: Value, key: &str) -> RequestParams<Value> {
        RequestParams {
            protocol_version: PROTOCOL_VERSION_V1.to_owned(),
            workspace_id: Some(Uuid::from_u128(21).to_string()),
            agent_id: Some("agent-root".to_owned()),
            session_id: Some(Uuid::from_u128(22).to_string()),
            capability_token: None,
            turn_id: None,
            work_item_id: Some(WORK_ITEM_ID.to_owned()),
            lease_id: None,
            fencing_token: None,
            expected_revision: None,
            idempotency_key: Some(key.to_owned()),
            confirmation: None,
            deadline_ms: 10_000,
            payload: json!({"operation": operation, "payload": payload}),
        }
    }

    fn current_revision(&self) -> u64 {
        self.state()["revision"].as_u64().expect("state revision")
    }

    fn state(&self) -> Value {
        let bytes = fs::read(
            self.root
                .path()
                .join(".auto-engineering/evidence-ledger/state.json"),
        )
        .expect("state bytes");
        serde_json::from_slice(&bytes).expect("state JSON")
    }

    fn evidence_file(&self, name: &str) -> Vec<u8> {
        fs::read(self.root.path().join(EVIDENCE_DIR).join(name)).expect("evidence artifact bytes")
    }

    fn ledger_bytes(&self) -> Vec<u8> {
        self.evidence_file("ledger.jsonl")
    }

    fn manifest_bytes(&self) -> Vec<u8> {
        self.evidence_file("manifest.json")
    }

    fn manifest(&self) -> Value {
        serde_json::from_slice(&self.manifest_bytes()).expect("manifest JSON")
    }

    fn ledger_events(&self) -> Vec<EvidenceLedgerEventV1> {
        decode_ledger(&self.ledger_bytes())
    }
}

fn bind_write(request: &mut RequestParams<Value>, lease: &Lease, revision: u64) {
    request.lease_id = Some(lease.lease_id.clone());
    request.fencing_token = Some(lease.fencing_token);
    request.expected_revision = Some(revision);
}

fn write_file(root: &TempDir, relative: &str, bytes: &[u8]) {
    let path = root.path().join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("parent directories");
    fs::write(path, bytes).expect("fixture file");
}

fn state_bytes(revision: u64) -> Vec<u8> {
    let state = json!({
        "stateMachineName": "PRD-LEDGER-001",
        "activeStory": WORK_ITEM_ID,
        "revision": revision,
        "lastFencingToken": 0,
        "scale": "small",
        "phase": "coding",
        "currentPhase": "coding",
        "storyStates": {
            WORK_ITEM_ID: {"phase": "coding", "currentPhase": "coding"}
        },
    });
    let mut bytes = serde_json::to_vec_pretty(&state).expect("state serializes");
    bytes.push(b'\n');
    bytes
}

fn decode_ledger(bytes: &[u8]) -> Vec<EvidenceLedgerEventV1> {
    let text = std::str::from_utf8(bytes).expect("ledger is UTF-8");
    let events = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("ledger event decodes"))
        .collect::<Vec<EvidenceLedgerEventV1>>();
    EvidenceLedgerEventV1::verify_chain(&events).expect("ledger chain verifies");
    events
}

fn assert_canonical_lines(bytes: &[u8], events: &[EvidenceLedgerEventV1]) {
    let text = std::str::from_utf8(bytes).expect("ledger is UTF-8");
    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), events.len());
    for (line, event) in lines.iter().zip(events) {
        let canonical = String::from_utf8(event.canonical_json()).expect("canonical JSON");
        assert_eq!(*line, canonical, "every ledger line is canonical JSON");
    }
}

fn prefixed_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", ArtifactDigest::digest(bytes))
}

fn verify_manifest_seal(manifest: &Value) {
    let mut payload = manifest.clone();
    payload
        .as_object_mut()
        .expect("manifest object")
        .retain(|key, _| key != "contentHash" && !key.starts_with('_'));
    let expected = format!(
        "sha256:{}",
        ArtifactDigest::digest(serde_json::to_vec(&payload).expect("manifest canonical"))
    );
    assert_eq!(manifest["contentHash"], json!(expected));
}

fn record_payload(logical_key: &str, input: &str) -> Value {
    json!({
        "artifactPath": "results/test.json",
        "inputFingerprint": input,
        "kind": "test",
        "command": ["cargo", "test"],
        "toolchainFingerprint": "rust-1",
        "exitCode": 0,
        "summary": {"gate": "G-TEST"},
        "logicalKey": logical_key,
    })
}

#[test]
fn root_role_is_denied_semantic_evidence_operations() {
    let fixture = Fixture::new();
    write_file(&fixture.root, "results/test.json", b"{\"pass\":true}\n");
    let lease = fixture.acquire_lease("ledger-root-denied-lease");
    let root_workspace = fixture.workspace_with_role(AgentRole::Root);

    for (operation, payload) in [
        ("evidence.record", record_payload("tests/core", INPUT_A)),
        ("evidence.finalize", json!({})),
    ] {
        let mut request = fixture.request(operation, payload, "ledger-root-denied");
        bind_write(&mut request, &lease, fixture.current_revision());
        let error = fixture
            .adapter
            .execute(RpcMethod::OperationExecute, &request, Some(&root_workspace))
            .expect_err("root orchestrator must not execute semantic work");
        assert_eq!(
            error.code(),
            StableErrorCode::RoleOperationForbidden,
            "{operation} must be delegated, not executed by root"
        );
    }
}

#[test]
fn record_appends_a_canonical_hash_chained_event_and_projects_the_manifest() {
    let fixture = Fixture::new();
    write_file(&fixture.root, "results/test.json", b"{\"pass\":true}\n");
    let lease = fixture.acquire_lease("ledger-lease-1");

    let response = fixture
        .record(
            &lease,
            "ledger-record-1",
            record_payload("tests/core", INPUT_A),
        )
        .expect("record succeeds");

    let data = &response["data"];
    let evidence_id = data["evidenceId"].as_str().expect("evidence id");
    assert_eq!(data["status"], "active");
    assert_eq!(data["logicalKey"], "tests/core");

    let ledger_bytes = fixture.ledger_bytes();
    let events = decode_ledger(&ledger_bytes);
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.sequence(), 1);
    assert_eq!(event.kind(), EvidenceLedgerEventKind::Recorded);
    assert_eq!(event.event_id().as_str(), evidence_id);
    assert_eq!(event.logical_key(), "tests/core");
    assert_eq!(event.input_fingerprint().to_string(), INPUT_A);
    assert_eq!(event.previous_event_digest(), None);
    assert_canonical_lines(&ledger_bytes, &events);
    let ref_kinds = event
        .artifact_refs()
        .iter()
        .map(|reference| reference.kind().as_str().to_owned())
        .collect::<Vec<_>>();
    assert!(ref_kinds.contains(&"evidence-entry".to_owned()));
    assert!(ref_kinds.contains(&"evidence-snapshot".to_owned()));

    let manifest = fixture.manifest();
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(manifest["storyId"], WORK_ITEM_ID);
    assert_eq!(manifest["entries"].as_array().expect("entries").len(), 1);
    assert_eq!(manifest["entries"][0]["evidenceId"], json!(evidence_id));
    assert_eq!(manifest["entries"][0]["status"], "active");
    verify_manifest_seal(&manifest);

    let state = fixture.state();
    let authority = &state["evidenceAuthority"];
    assert_eq!(
        authority["ledgerRef"],
        json!(format!("{EVIDENCE_DIR}/ledger.jsonl"))
    );
    assert_eq!(
        authority["ledgerDigest"],
        json!(prefixed_digest(&ledger_bytes))
    );
    assert_eq!(
        authority["manifestRef"],
        json!(format!("{EVIDENCE_DIR}/manifest.json"))
    );
    assert_eq!(
        authority["manifestDigest"],
        json!(prefixed_digest(&fixture.manifest_bytes()))
    );
}

#[test]
fn record_supersedes_the_same_logical_key_through_appended_events_only() {
    let fixture = Fixture::new();
    write_file(&fixture.root, "results/test.json", b"first\n");
    let lease = fixture.acquire_lease("ledger-lease-2");
    let first = fixture
        .record(
            &lease,
            "ledger-record-2a",
            record_payload("tests/core", INPUT_A),
        )
        .expect("first record succeeds");
    let first_id = first["data"]["evidenceId"]
        .as_str()
        .expect("first evidence id")
        .to_owned();
    let ledger_after_first = fixture.ledger_bytes();

    write_file(&fixture.root, "results/test.json", b"second\n");
    let second = fixture
        .record(
            &lease,
            "ledger-record-2b",
            record_payload("tests/core", INPUT_B),
        )
        .expect("second record succeeds");
    let second_id = second["data"]["evidenceId"]
        .as_str()
        .expect("second evidence id")
        .to_owned();

    let ledger_bytes = fixture.ledger_bytes();
    assert!(
        ledger_bytes.starts_with(&ledger_after_first),
        "ledger history is append-only: new bytes extend the previous prefix"
    );
    let events = decode_ledger(&ledger_bytes);
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].kind(), EvidenceLedgerEventKind::Recorded);
    assert_eq!(events[1].kind(), EvidenceLedgerEventKind::Superseded);
    assert_eq!(events[1].logical_key(), "tests/core");
    assert_eq!(events[2].kind(), EvidenceLedgerEventKind::Recorded);
    assert_eq!(events[2].sequence(), 3);
    assert_eq!(
        events[2].previous_event_digest(),
        Some(events[1].event_digest())
    );
    assert_canonical_lines(&ledger_bytes, &events);

    let manifest = fixture.manifest();
    let entries = manifest["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["evidenceId"], json!(first_id));
    assert_eq!(entries[0]["status"], "superseded");
    assert_eq!(entries[0]["inputFingerprint"], INPUT_A);
    assert_eq!(entries[1]["evidenceId"], json!(second_id));
    assert_eq!(entries[1]["status"], "active");
    verify_manifest_seal(&manifest);
}

#[test]
fn finalize_appends_a_finalized_event_and_seals_the_projection() {
    let fixture = Fixture::new();
    write_file(&fixture.root, "results/test.json", b"{\"pass\":true}\n");
    let lease = fixture.acquire_lease("ledger-lease-3");
    fixture
        .record(
            &lease,
            "ledger-record-3",
            record_payload("tests/core", INPUT_A),
        )
        .expect("record succeeds");

    let response = fixture
        .finalize(&lease, "ledger-finalize-3")
        .expect("finalize succeeds");

    let data = &response["data"];
    assert_eq!(
        data["manifest"],
        json!(format!("{EVIDENCE_DIR}/manifest.json"))
    );
    assert_eq!(data["entryCount"], 1);

    let ledger_bytes = fixture.ledger_bytes();
    let events = decode_ledger(&ledger_bytes);
    assert_eq!(events.len(), 2);
    let finalized = &events[1];
    assert_eq!(finalized.kind(), EvidenceLedgerEventKind::Finalized);
    assert_eq!(finalized.sequence(), 2);
    assert_canonical_lines(&ledger_bytes, &events);
    let manifest_ref = finalized
        .artifact_refs()
        .iter()
        .find(|reference| reference.kind().as_str() == "evidence-manifest")
        .expect("finalized event binds the manifest projection");
    assert_eq!(
        manifest_ref.path().as_str(),
        format!("{EVIDENCE_DIR}/manifest.json")
    );
    assert_eq!(
        manifest_ref.digest(),
        ArtifactDigest::digest(fixture.manifest_bytes())
    );

    let manifest = fixture.manifest();
    verify_manifest_seal(&manifest);
    assert_eq!(manifest["entries"][0]["status"], "active");

    let state = fixture.state();
    let authority = &state["evidenceAuthority"];
    assert_eq!(
        authority["ledgerDigest"],
        json!(prefixed_digest(&ledger_bytes))
    );
    assert_eq!(
        authority["manifestDigest"],
        json!(prefixed_digest(&fixture.manifest_bytes()))
    );
}

#[test]
fn tampered_ledger_history_fails_closed() {
    let fixture = Fixture::new();
    write_file(&fixture.root, "results/test.json", b"first\n");
    let lease = fixture.acquire_lease("ledger-lease-4");
    fixture
        .record(
            &lease,
            "ledger-record-4a",
            record_payload("tests/core", INPUT_A),
        )
        .expect("first record succeeds");
    write_file(&fixture.root, "results/test.json", b"second\n");
    fixture
        .record(
            &lease,
            "ledger-record-4b",
            record_payload("tests/core", INPUT_B),
        )
        .expect("second record succeeds");

    let tampered = fixture
        .ledger_bytes()
        .into_iter()
        .map(|byte| if byte == b'a' { b'b' } else { byte })
        .collect::<Vec<_>>();
    fs::write(
        fixture.root.path().join(EVIDENCE_DIR).join("ledger.jsonl"),
        tampered,
    )
    .expect("tampered ledger write");

    let record_error = fixture
        .record(
            &lease,
            "ledger-record-4c",
            record_payload("tests/other", INPUT_A),
        )
        .expect_err("a tampered hash chain must fail closed");
    assert_eq!(record_error.code(), StableErrorCode::ExternalStateConflict);
    let finalize_error = fixture
        .finalize(&lease, "ledger-finalize-4")
        .expect_err("finalize must fail closed on a tampered hash chain");
    assert_eq!(
        finalize_error.code(),
        StableErrorCode::ExternalStateConflict
    );
}

#[test]
fn manifest_rebuilds_byte_identically_from_the_ledger() {
    let fixture = Fixture::new();
    write_file(&fixture.root, "results/test.json", b"first\n");
    let lease = fixture.acquire_lease("ledger-lease-5");
    fixture
        .record(
            &lease,
            "ledger-record-5a",
            record_payload("tests/core", INPUT_A),
        )
        .expect("first record succeeds");
    write_file(&fixture.root, "results/test.json", b"second\n");
    fixture
        .record(
            &lease,
            "ledger-record-5b",
            record_payload("tests/core", INPUT_B),
        )
        .expect("second record succeeds");
    fixture
        .finalize(&lease, "ledger-finalize-5a")
        .expect("first finalize succeeds");
    let manifest_before = fixture.manifest_bytes();

    fs::remove_file(fixture.root.path().join(EVIDENCE_DIR).join("manifest.json"))
        .expect("manifest projection deleted");
    fixture
        .finalize(&lease, "ledger-finalize-5b")
        .expect("second finalize rebuilds the projection");

    assert_eq!(
        fixture.manifest_bytes(),
        manifest_before,
        "the same ledger must produce a byte-identical manifest projection"
    );
}

#[test]
fn legacy_manifest_without_a_ledger_stays_read_compatible() {
    let fixture = Fixture::new();
    let snapshot = b"legacy snapshot\n";
    let snapshot_digest = ArtifactDigest::digest(snapshot);
    write_file(
        &fixture.root,
        &format!("{EVIDENCE_DIR}/artifacts/legacy.txt"),
        snapshot,
    );
    let legacy_entry = json!({
        "evidenceId": "ev-legacy",
        "kind": "test",
        "commandHash": "sha256:legacy",
        "inputFingerprint": "i1",
        "toolchainFingerprint": "java8",
        "exitCode": 0,
        "reusable": true,
        "artifacts": [{
            "path": "results/legacy.txt",
            "sha256": format!("sha256:{snapshot_digest}"),
            "snapshotPath": format!("{EVIDENCE_DIR}/artifacts/legacy.txt"),
        }],
    });
    let mut legacy = json!({
        "schemaVersion": 1,
        "storyId": WORK_ITEM_ID,
        "entries": [legacy_entry.clone()],
    });
    let digest = format!(
        "sha256:{}",
        ArtifactDigest::digest(serde_json::to_vec(&legacy).expect("legacy canonical"))
    );
    legacy["contentHash"] = json!(digest);
    let mut legacy_bytes = serde_json::to_vec_pretty(&legacy).expect("legacy serializes");
    legacy_bytes.push(b'\n');
    write_file(
        &fixture.root,
        &format!("{EVIDENCE_DIR}/manifest.json"),
        &legacy_bytes,
    );
    let lease = fixture.acquire_lease("ledger-lease-6");

    fixture
        .finalize(&lease, "ledger-finalize-6")
        .expect("legacy finalize succeeds");
    assert!(
        !fixture
            .root
            .path()
            .join(EVIDENCE_DIR)
            .join("ledger.jsonl")
            .exists(),
        "a legacy manifest without a ledger must not gain one from finalize alone"
    );
    let finalized = fixture.manifest();
    assert_eq!(finalized["entries"][0], legacy_entry);
    assert!(
        finalized["entries"][0].get("status").is_none(),
        "legacy entries are never rewritten in place"
    );
    let state = fixture.state();
    let authority = &state["evidenceAuthority"];
    assert_eq!(authority["ledgerRef"], Value::Null);
    assert_eq!(authority["ledgerDigest"], Value::Null);
    assert_eq!(
        authority["manifestDigest"],
        json!(prefixed_digest(&fixture.manifest_bytes()))
    );

    write_file(&fixture.root, "results/test.json", b"fresh\n");
    fixture
        .record(
            &lease,
            "ledger-record-6",
            record_payload("tests/core", INPUT_A),
        )
        .expect("record after a legacy manifest succeeds");
    let manifest = fixture.manifest();
    let entries = manifest["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0], legacy_entry,
        "legacy entries survive the first ledger append verbatim"
    );
    assert_eq!(entries[1]["status"], "active");
    let events = fixture.ledger_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind(), EvidenceLedgerEventKind::Recorded);
}
