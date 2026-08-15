//! Process-level regressions for the installed Story-flow runtime boundaries.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ae_sdd_client::{ClientTransport, DaemonClient, LocalIpcTransport};
use ae_sdd_domain::InputFingerprint;
use ae_sdd_protocol::{
    ClientKind, ConfirmationRef, PROTOCOL_VERSION_V1, RequestParams, RpcMethod, WorkspaceMode,
};
use ae_sdd_runtime::{SessionResult, WorkspaceParityEvidence, WorkspaceResult};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use uuid::Uuid;

const WORK_ITEM_ID: &str = "ROUTE-STORY-FLOW-PROCESS";
const PROJECT_KEY: &str = "story-flow-runtime-process";
const AGENT_ID: &str = "story-flow-process-root";
const EXTERNAL_SESSION_KEY: &str = "story-flow-process-session";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn installed_story_friction_replay_closes_f005_f009_f033_f045_f050_f051_f052() {
    let fixture = Fixture::new();
    let installed_daemon = fixture.install_current_daemon();
    let mut daemon = DaemonProcess::start_binary(
        &installed_daemon,
        &fixture.runtime_dir,
        fixture.allowed_root(),
    )
    .await;
    let cli = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let admin = daemon_client(&fixture.runtime_dir, ClientKind::Admin);
    let status: Value = cli
        .call(RpcMethod::RuntimeStatus, params(json!({})))
        .await
        .expect("installed daemon status");
    let daemon_path = status["daemonPath"]
        .as_str()
        .map(Path::new)
        .expect("runtime status exposes daemonPath");
    assert_eq!(
        fs::canonicalize(daemon_path).expect("running daemon path canonicalizes"),
        fs::canonicalize(&installed_daemon).expect("installed daemon path canonicalizes"),
        "installed replay must execute the native-job installed binary: {status}"
    );

    let workspace = register_and_cut_over(&cli, &admin, &fixture).await;
    let session = open_root(&cli, &workspace).await;
    let root = Identity::new(&workspace, &session, AGENT_ID);

    let plan_schema: Value = cli
        .call(
            RpcMethod::OperationDescribe,
            root.params(json!({"operation":"execution.plan.set"})),
        )
        .await
        .expect("installed execution.plan.set schema");
    let verification = described_field(&plan_schema, "verification");
    assert_eq!(verification["kind"], "Array");
    assert_eq!(verification["itemKind"], "Object");
    assert_eq!(
        verification["items"],
        json!([
            {"name":"id","kind":"String","required":true},
            {"name":"acId","kind":"String","required":true},
            {"name":"boundary","kind":"String","required":true},
            {"name":"command","kind":"StringOrArray","required":true},
            {"name":"expected","kind":"String","required":true}
        ])
    );
    let document_schema: Value = cli
        .call(
            RpcMethod::OperationDescribe,
            root.params(json!({"operation":"document.save"})),
        )
        .await
        .expect("installed document.save schema");
    let keep_draft = described_field(&document_schema, "keepDraft");
    assert_eq!(keep_draft["kind"], "Boolean");
    assert_eq!(keep_draft["required"], false);

    let lease = acquire_lease(&cli, &root, "root", "installed-friction-lease").await;
    let state_before = fs::read(fixture.state_path()).expect("state before schema rejection");
    let mut incomplete = root.params(json!({
        "operation":"execution.plan.set",
        "payload":{
            "goal":"reject incomplete verification",
            "changedPaths":["bins/ae-sdd-daemon/tests/story_flow_runtime_process.rs"],
            "verification":[{
                "id":"V-INSTALLED",
                "acId":"AC-005",
                "command":"cargo test",
                "expected":"passes"
            }]
        }
    }));
    bind_write(
        &mut incomplete,
        &lease,
        read_revision(&fixture),
        "installed-incomplete-plan",
    );
    let rejected = cli
        .call::<Value>(RpcMethod::OperationExecute, incomplete)
        .await
        .expect_err("incomplete verification item is rejected");
    assert!(
        rejected.to_string().contains("OperationSchemaInvalid"),
        "{rejected:?}"
    );
    assert_eq!(
        fs::read(fixture.state_path()).expect("state after schema rejection"),
        state_before,
        "schema rejection must not mutate state"
    );
    let mut ra_gate = root.params(json!({"gateId":"G-RA-4"}));
    ra_gate.lease_id = Some(lease.id.clone());
    ra_gate.fencing_token = Some(lease.fencing);
    ra_gate.idempotency_key = Some("installed-friction-ra-grammar".to_owned());
    let evaluated: Value = cli
        .call(RpcMethod::GateEvaluate, ra_gate)
        .await
        .expect("installed RA scanner returns a typed non-PASS outcome");
    assert_ne!(evaluated["outcome"]["kind"], "PASS", "{evaluated}");
    let findings = evaluated["outcome"]["findings"]
        .as_array()
        .expect("RA findings");
    assert!(
        findings
            .iter()
            .any(|finding| finding["code"] == "closure-scale-rubric-malformed"),
        "{evaluated}"
    );
    release_lease(
        &cli,
        &root,
        &lease,
        read_revision(&fixture),
        "root",
        "installed-friction-lease-release",
    )
    .await;

    let flow: Value = cli
        .call(RpcMethod::FlowNext, root.params(json!({})))
        .await
        .expect("installed root flow decision");
    let host = HostAdapter::register(
        &fixture.runtime_dir,
        "codex",
        "installed-friction-host-register",
    )
    .await;
    let mut create = root.params(json!({
        "flowDecisionDigest":flow["decisionDigest"]
    }));
    create.idempotency_key = Some("installed-friction-series-create".to_owned());
    let created: Value = cli
        .call(RpcMethod::DelegationCreate, create)
        .await
        .expect("installed Root Series delegation");
    let delegation_id = created["delegationId"]
        .as_str()
        .expect("delegation id")
        .to_owned();
    assert_testcase_asset_refs(&fixture.project_root, &created["assetRefs"]);

    let child_session_id = Uuid::new_v4().to_string();
    let action = host.call(RpcMethod::HostActionNext, json!({})).await;
    host.call(
        RpcMethod::HostActionAck,
        json!({
            "ack":{
                "ackId":Uuid::new_v4().to_string(),
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"installed-friction-host-task",
                "sessionId":child_session_id,
            }
        }),
    )
    .await;
    let accept_payload = json!({
        "delegationId":delegation_id,
        "claimId":action["claimId"],
        "actionId":action["actionId"],
        "childSessionId":child_session_id,
        "expiresAtUnixMs":now_unix_ms().saturating_add(500_000),
    });
    let mut mismatched_accept = params(accept_payload.clone());
    mismatched_accept.workspace_id = Some(workspace.workspace_id.clone());
    mismatched_accept.work_item_id = Some("ROUTE-WRONG-WORK-ITEM".to_owned());
    mismatched_accept.idempotency_key = Some("installed-friction-series-accept".to_owned());
    let mismatch = cli
        .call::<Value>(RpcMethod::DelegationAccept, mismatched_accept)
        .await
        .expect_err("Work Item mismatch is checked before accept replay");
    assert_binding_mismatch_rejected(&mismatch.to_string());
    let mut accept = params(accept_payload);
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.idempotency_key = Some("installed-friction-series-accept".to_owned());
    cli.call::<Value>(RpcMethod::DelegationAccept, accept)
        .await
        .expect("rejected mismatch does not consume the corrected accept key");

    let child_agent = "installed-friction-series-agent";
    let child_open_payload = json!({
        "externalKey":"installed-friction-series-external",
        "role":"series",
        "engaged":true,
        "delegationId":delegation_id,
    });
    let mut mismatched_open = params(child_open_payload.clone());
    mismatched_open.workspace_id = Some(workspace.workspace_id.clone());
    mismatched_open.work_item_id = Some("ROUTE-WRONG-WORK-ITEM".to_owned());
    mismatched_open.agent_id = Some(child_agent.to_owned());
    mismatched_open.session_id = Some(child_session_id.clone());
    mismatched_open.idempotency_key = Some("installed-friction-series-open".to_owned());
    let mismatch = cli
        .call::<SessionResult>(RpcMethod::SessionOpen, mismatched_open)
        .await
        .expect_err("Work Item mismatch is checked before session replay");
    assert_binding_mismatch_rejected(&mismatch.to_string());
    let mut child_open = params(child_open_payload.clone());
    child_open.workspace_id = Some(workspace.workspace_id.clone());
    child_open.agent_id = Some(child_agent.to_owned());
    child_open.session_id = Some(child_session_id.clone());
    child_open.idempotency_key = Some("installed-friction-series-open".to_owned());
    let child: SessionResult = cli
        .call(RpcMethod::SessionOpen, child_open)
        .await
        .expect("rejected mismatch does not consume the corrected session key");
    let series = Identity::new(&workspace, &child, child_agent);

    let nested_deadline_unix_ms = now_unix_ms().saturating_add(500_000);
    let mut nested_create = series.params(json!({
        "childRole":"task",
        "parentDelegationId":delegation_id,
        "inputRevision":1,
        "inputFingerprint":flow["decisionDigest"],
        "deadlineUnixMs":nested_deadline_unix_ms,
        "grant":{"operations":[],"capabilities":[],"paths":[]}
    }));
    nested_create.idempotency_key = Some("installed-friction-nested-create".to_owned());
    let nested_created: Value = cli
        .call(RpcMethod::DelegationCreate, nested_create)
        .await
        .expect("installed nested Task delegation");
    let nested_delegation_id = nested_created["delegationId"]
        .as_str()
        .expect("nested delegation id")
        .to_owned();
    let nested_session_id = Uuid::new_v4().to_string();
    let nested_action = host.call(RpcMethod::HostActionNext, json!({})).await;
    host.call(
        RpcMethod::HostActionAck,
        json!({
            "ack":{
                "ackId":Uuid::new_v4().to_string(),
                "actionId":nested_action["actionId"],
                "commandSeq":nested_action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"installed-friction-nested-host-task",
                "sessionId":nested_session_id,
            }
        }),
    )
    .await;
    let mut nested_accept = params(json!({
        "delegationId":nested_delegation_id,
        "claimId":nested_action["claimId"],
        "actionId":nested_action["actionId"],
        "childSessionId":nested_session_id,
        "expiresAtUnixMs":nested_deadline_unix_ms,
    }));
    nested_accept.workspace_id = Some(workspace.workspace_id.clone());
    nested_accept.idempotency_key = Some("installed-friction-nested-accept".to_owned());
    cli.call::<Value>(RpcMethod::DelegationAccept, nested_accept)
        .await
        .expect("installed nested Task accepts durable authority");
    let nested_agent = "installed-friction-task-agent";
    let mut nested_open = params(json!({
        "externalKey":"installed-friction-task-external",
        "role":"task",
        "engaged":true,
        "delegationId":nested_delegation_id,
    }));
    nested_open.workspace_id = Some(workspace.workspace_id.clone());
    nested_open.agent_id = Some(nested_agent.to_owned());
    nested_open.session_id = Some(nested_session_id);
    nested_open.idempotency_key = Some("installed-friction-nested-open".to_owned());
    let nested_child: SessionResult = cli
        .call(RpcMethod::SessionOpen, nested_open)
        .await
        .expect("installed nested Task session derives the durable Work Item");
    let nested = Identity::new(&workspace, &nested_child, nested_agent);
    let mut nested_report = nested.params(json!({
        "delegationId":nested_delegation_id,
        "inputRevision":1,
        "inputFingerprint":flow["decisionDigest"],
        "summary":"installed nested Task result",
        "result":{
            "outcome":"succeeded",
            "findings":[],
            "deliverables":[],
            "requestedAction":null,
            "memorySnapshotDigest":"b".repeat(64)
        },
    }));
    nested_report.idempotency_key = Some("installed-friction-nested-report".to_owned());
    cli.call::<Value>(RpcMethod::DelegationReport, nested_report)
        .await
        .expect("installed nested Task reaches memory-cleaned");
    let mut nested_status = series.params(json!({"delegationId":nested_delegation_id}));
    nested_status.idempotency_key = Some("installed-friction-nested-status".to_owned());
    let nested_status: Value = cli
        .call(RpcMethod::DelegationStatus, nested_status)
        .await
        .expect("nested collect prerequisite is projected");
    assert_eq!(
        nested_status["collectPrerequisite"]["requiresRootProjectLease"], false,
        "{nested_status}"
    );
    assert!(
        nested_status["collectPrerequisite"]["rootProjectLeaseSubmit"].is_null(),
        "{nested_status}"
    );
    assert!(
        nested_status["collectPrerequisite"]["collectSubmit"]["requestContext"]
            .get("leaseIdFrom")
            .is_none(),
        "{nested_status}"
    );
    let mut nested_collect = series.params(json!({"delegationId":nested_delegation_id}));
    nested_collect.idempotency_key = Some("installed-friction-nested-collect".to_owned());
    let nested_projection: Value = cli
        .call(RpcMethod::DelegationCollect, nested_collect)
        .await
        .expect("nested collect completes without a Root project lease");
    assert_eq!(nested_projection["requiresRootProjectLease"], false);
    assert!(nested_projection["rootProjectLeaseSubmit"].is_null());
    assert!(nested_projection["collectSubmit"]["leaseBinding"].is_null());

    let frozen_fingerprint = flow["decisionDigest"]
        .as_str()
        .expect("flow decision digest")
        .to_owned();
    let mut stale_revision = series.params(json!({
        "delegationId":delegation_id,
        "inputRevision":2,
        "inputFingerprint":frozen_fingerprint,
        "summary":"installed stale revision",
        "result":{"memorySnapshotDigest":"a".repeat(64)},
    }));
    stale_revision.idempotency_key = Some("installed-friction-stale-revision".to_owned());
    let revision_error = cli
        .call::<Value>(RpcMethod::DelegationReport, stale_revision)
        .await
        .expect_err("stale inputRevision is rejected");
    let mut stale_fingerprint = series.params(json!({
        "delegationId":delegation_id,
        "inputRevision":1,
        "inputFingerprint":"0".repeat(64),
        "summary":"installed stale fingerprint",
        "result":{"memorySnapshotDigest":"a".repeat(64)},
    }));
    stale_fingerprint.idempotency_key = Some("installed-friction-stale-fingerprint".to_owned());
    let fingerprint_error = cli
        .call::<Value>(RpcMethod::DelegationReport, stale_fingerprint)
        .await
        .expect_err("stale inputFingerprint is rejected");
    let revision_error = revision_error.to_string().to_lowercase();
    let fingerprint_error = fingerprint_error.to_string().to_lowercase();
    assert!(revision_error.contains("revision"), "{revision_error}");
    assert!(!revision_error.contains("fingerprint"), "{revision_error}");
    assert!(
        fingerprint_error.contains("fingerprint"),
        "{fingerprint_error}"
    );
    assert!(
        !fingerprint_error.contains("revision"),
        "{fingerprint_error}"
    );

    daemon.crash();
    daemon = DaemonProcess::start_binary(
        &installed_daemon,
        &fixture.runtime_dir,
        fixture.allowed_root(),
    )
    .await;
    let cli = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let reopened_root = open_root(&cli, &workspace).await;
    let root = Identity::new(&workspace, &reopened_root, AGENT_ID);
    let mut reopen = params(child_open_payload);
    reopen.workspace_id = Some(workspace.workspace_id.clone());
    reopen.agent_id = Some(child_agent.to_owned());
    reopen.session_id = Some(child_session_id);
    reopen.idempotency_key = Some("installed-friction-series-reopen".to_owned());
    let reopened_child: SessionResult = cli
        .call(RpcMethod::SessionOpen, reopen)
        .await
        .expect("delegated session reopens after installed daemon restart without Work Item");
    let series = Identity::new(&workspace, &reopened_child, child_agent);

    let mut report = series.params(json!({
        "delegationId":delegation_id,
        "inputRevision":1,
        "inputFingerprint":frozen_fingerprint,
        "summary":"installed bounded Series result",
        "result":{
            "outcome":"succeeded",
            "findings":[],
            "deliverables":[],
            "requestedAction":null,
            "memorySnapshotDigest":"a".repeat(64)
        },
    }));
    report.idempotency_key = Some("installed-friction-series-report".to_owned());
    let reported: Value = cli
        .call(RpcMethod::DelegationReport, report)
        .await
        .expect("valid Series report reaches memory-cleaned");
    assert_eq!(reported["status"], "memory-cleaned", "{reported}");
    let mut status_request = root.params(json!({"delegationId":delegation_id}));
    status_request.idempotency_key = Some("installed-friction-series-status".to_owned());
    let delegation_status: Value = cli
        .call(RpcMethod::DelegationStatus, status_request)
        .await
        .expect("delegation status projects collect prerequisite");
    assert_eq!(
        delegation_status["collectPrerequisite"]["requiresRootProjectLease"], true,
        "{delegation_status}"
    );
    assert_eq!(
        delegation_status["collectPrerequisite"]["rootProjectLeaseSubmit"]["method"],
        "operation.execute"
    );
    let mut collect = root.params(json!({"delegationId":delegation_id}));
    collect.idempotency_key = Some("installed-friction-series-collect".to_owned());
    let collect_projection: Value = cli
        .call(RpcMethod::DelegationCollect, collect)
        .await
        .expect("Root collect without a lease returns executable remediation");
    assert_eq!(collect_projection["requiresRootProjectLease"], true);
    assert_eq!(
        collect_projection["collectSubmit"]["leaseBinding"]["leaseIdFrom"],
        "rootProjectLeaseSubmit.result.data.leaseId"
    );

    let document_lease =
        acquire_lease(&cli, &series, "series", "installed-friction-document-lease").await;
    let source = ".hermes/installed-default-ra.md";
    write_source(&fixture.project_root, source, "# installed default RA\n");
    let save_payload = json!({
        "operation":"document.save",
        "payload":{"intent":"RA","contentFile":source}
    });
    let save_revision = read_revision(&fixture);
    let mut save = series.params(save_payload.clone());
    bind_write(
        &mut save,
        &document_lease,
        save_revision,
        "installed-friction-document-save",
    );
    let first: Value = cli
        .call(RpcMethod::OperationExecute, save)
        .await
        .expect("default document save commits");
    assert!(!fixture.project_root.join(source).exists());
    assert_eq!(
        first["data"]["draftCleanup"],
        json!({"path":source,"status":"deleted"})
    );
    let mut save_replay = series.params(save_payload);
    bind_write(
        &mut save_replay,
        &document_lease,
        save_revision,
        "installed-friction-document-save",
    );
    let replay: Value = cli
        .call(RpcMethod::OperationExecute, save_replay)
        .await
        .expect("document save replays");
    assert_eq!(replay["receiptDigest"], first["receiptDigest"]);
    assert_eq!(replay["data"], first["data"]);

    let kept_source = ".hermes/installed-kept-ra.md";
    write_source(&fixture.project_root, kept_source, "# installed kept RA\n");
    let mut keep = series.params(json!({
        "operation":"document.save",
        "payload":{"intent":"RA","contentFile":kept_source,"keepDraft":true}
    }));
    bind_write(
        &mut keep,
        &document_lease,
        read_revision(&fixture),
        "installed-friction-document-keep",
    );
    let kept: Value = cli
        .call(RpcMethod::OperationExecute, keep)
        .await
        .expect("keepDraft document save commits");
    assert!(fixture.project_root.join(kept_source).is_file());
    assert_eq!(
        kept["data"]["draftCleanup"],
        json!({"path":kept_source,"status":"preserved"})
    );
    let owned_daemon_pid = daemon.id();
    assert_no_owned_processes(
        owned_daemon_pid,
        &[
            fixture.runtime_dir.as_path(),
            fixture.install_dir().as_path(),
        ],
    );
    daemon.crash();
    assert_no_owned_processes(
        owned_daemon_pid,
        &[
            fixture.runtime_dir.as_path(),
            fixture.install_dir().as_path(),
        ],
    );
    assert_tree_unlocked(&fixture.runtime_dir);
    assert_tree_unlocked(&fixture.install_dir());
    assert_no_native_transaction_residue(fixture.root.path());
    fs::remove_dir_all(&fixture.runtime_dir).expect("owned runtime directory is unlocked");
    fs::remove_dir_all(fixture.install_dir()).expect("owned install directory is unlocked");
    assert!(!fixture.runtime_dir.exists());
    assert!(!fixture.install_dir().exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn same_host_hook_event_replays_one_turn_and_one_receipt() {
    let fixture = Fixture::new();
    let mut daemon = DaemonProcess::start(&fixture.runtime_dir, fixture.allowed_root()).await;
    let cli = daemon_client(&fixture.runtime_dir, ClientKind::Cli);
    let admin = daemon_client(&fixture.runtime_dir, ClientKind::Admin);
    let hook_a = daemon_client(&fixture.runtime_dir, ClientKind::Hook);
    let hook_b = daemon_client(&fixture.runtime_dir, ClientKind::Hook);

    let workspace = register_and_cut_over(&cli, &admin, &fixture).await;
    let session = open_root(&cli, &workspace).await;

    let (left, right) = tokio::join!(
        hook_prompt(&hook_a, &workspace, &session, "host-event-1"),
        hook_prompt(&hook_b, &workspace, &session, "host-event-1")
    );
    let (first, replay) = if left["replayed"] == false {
        (left, right)
    } else {
        (right, left)
    };

    assert_eq!(
        first["context"]["nextAction"]["seriesKind"], "testcase",
        "story-generated state must hand off to the TestCase Series: {first}"
    );
    let methodology = first["context"]["assetRefs"]
        .as_array()
        .and_then(|refs| {
            refs.iter()
                .find(|reference| reference["kind"] == "methodology-skill")
        })
        .unwrap_or_else(|| panic!("TestCase handoff must bind its methodology asset: {first}"));
    assert_eq!(
        methodology["path"],
        "source/skills/phase1-design/testcase-generate-skill.md"
    );
    assert_eq!(
        methodology["sha256"].as_str().map(str::len),
        Some(64),
        "{methodology}"
    );
    assert_eq!(first["turnId"], replay["turnId"], "{first}\n{replay}");
    assert_eq!(first["turnSeq"], replay["turnSeq"], "{first}\n{replay}");
    assert_eq!(first["eventSeq"], replay["eventSeq"], "{first}\n{replay}");
    assert_eq!(first["replayed"], false, "{first}");
    assert_eq!(first["contextKind"], "full", "{first}");
    assert!(first["context"].is_object(), "{first}");
    assert_eq!(replay["context"], first["context"], "{first}\n{replay}");

    let replay_after_commit = hook_prompt(&hook_a, &workspace, &session, "host-event-1").await;
    assert_eq!(
        replay_after_commit["replayed"], true,
        "{replay_after_commit}"
    );
    assert_eq!(replay_after_commit["turnId"], first["turnId"]);
    assert_eq!(replay_after_commit["eventSeq"], first["eventSeq"]);

    let mut conflicting = hook_request(&workspace, &session, "host-event-1");
    conflicting.turn_id = Some("conflicting-explicit-turn".to_owned());
    conflicting.payload["turnSeq"] = json!(2);
    let conflict = hook_a
        .call::<Value>(RpcMethod::HookUserPrompt, conflicting)
        .await
        .expect_err("a reused event cannot change its explicit turn identity");
    assert!(
        format!("{conflict:?}").contains("IdempotencyKeyReused"),
        "{conflict:?}"
    );

    let later = hook_prompt(&hook_a, &workspace, &session, "host-event-2").await;
    assert_eq!(later["contextKind"], "no_change", "{later}");
    assert!(later["context"].is_null(), "{later}");
    assert_eq!(
        later["turnSeq"], 2,
        "a duplicate event must not consume a second turn: {later}"
    );

    daemon.crash();
}

async fn hook_prompt(
    hook: &DaemonClient,
    workspace: &WorkspaceResult,
    session: &SessionResult,
    event_id: &str,
) -> Value {
    let request = hook_request(workspace, session, event_id);
    hook.call(RpcMethod::HookUserPrompt, request)
        .await
        .unwrap_or_else(|error| panic!("Hook reaches the real daemon: {error:?}"))
}

fn hook_request(
    workspace: &WorkspaceResult,
    session: &SessionResult,
    event_id: &str,
) -> RequestParams<Value> {
    let mut request = params(json!({
        "hookEventId": event_id,
        "hostPayload": {"prompt":"continue the Story flow"},
    }));
    request.workspace_id = Some(workspace.workspace_id.clone());
    request.work_item_id = Some(WORK_ITEM_ID.to_owned());
    request.session_id = Some(session.session_id.clone());
    request.agent_id = Some(AGENT_ID.to_owned());
    request.capability_token = Some(session.capability_token.clone());
    request.idempotency_key = Some(format!("request-{event_id}"));
    request.deadline_ms = 250;
    request
}

struct Identity {
    workspace_id: String,
    work_item_id: String,
    agent_id: String,
    session_id: String,
    capability_token: String,
}

impl Identity {
    fn new(workspace: &WorkspaceResult, session: &SessionResult, agent_id: &str) -> Self {
        Self {
            workspace_id: workspace.workspace_id.clone(),
            work_item_id: WORK_ITEM_ID.to_owned(),
            agent_id: agent_id.to_owned(),
            session_id: session.session_id.clone(),
            capability_token: session.capability_token.clone(),
        }
    }

    fn params(&self, payload: Value) -> RequestParams<Value> {
        let mut request = params(payload);
        request.workspace_id = Some(self.workspace_id.clone());
        request.work_item_id = Some(self.work_item_id.clone());
        request.agent_id = Some(self.agent_id.clone());
        request.session_id = Some(self.session_id.clone());
        request.capability_token = Some(self.capability_token.clone());
        request
    }
}

struct Lease {
    id: String,
    fencing: u64,
}

async fn acquire_lease(
    client: &DaemonClient,
    identity: &Identity,
    owner_role: &str,
    key: &str,
) -> Lease {
    let mut request = identity.params(json!({
        "operation":"lease.acquire",
        "payload":{"owner":{"role":owner_role},"ttlSeconds":300}
    }));
    request.idempotency_key = Some(key.to_owned());
    let result: Value = client
        .call(RpcMethod::OperationExecute, request)
        .await
        .unwrap_or_else(|error| panic!("{key} lease: {error:?}"));
    Lease {
        id: result["data"]["leaseId"]
            .as_str()
            .expect("lease id")
            .to_owned(),
        fencing: result["data"]["fencingToken"]
            .as_u64()
            .expect("fencing token"),
    }
}

async fn release_lease(
    client: &DaemonClient,
    identity: &Identity,
    lease: &Lease,
    revision: u64,
    owner_role: &str,
    key: &str,
) {
    let mut request = identity.params(json!({
        "operation":"lease.release",
        "payload":{"owner":{"role":owner_role}}
    }));
    bind_write(&mut request, lease, revision, key);
    client
        .call::<Value>(RpcMethod::OperationExecute, request)
        .await
        .unwrap_or_else(|error| panic!("{key} release: {error:?}"));
}

fn bind_write(
    request: &mut RequestParams<Value>,
    lease: &Lease,
    revision: u64,
    idempotency_key: &str,
) {
    request.lease_id = Some(lease.id.clone());
    request.fencing_token = Some(lease.fencing);
    request.expected_revision = Some(revision);
    request.idempotency_key = Some(idempotency_key.to_owned());
}

fn described_field<'a>(description: &'a Value, name: &str) -> &'a Value {
    description[0]["fields"]
        .as_array()
        .expect("operation fields")
        .iter()
        .find(|field| field["name"] == name)
        .unwrap_or_else(|| panic!("operation field {name} is described: {description}"))
}

fn assert_testcase_asset_refs(project_root: &Path, value: &Value) {
    let refs = value
        .as_array()
        .unwrap_or_else(|| panic!("fresh delegation carries assetRefs: {value}"));
    let expected = [
        ("constraints-index", "constraints/README.md"),
        ("methodology-entry", "source/SKILL.md"),
        (
            "methodology-catalog",
            "source/standards/runtime/methodology-catalog.v1.json",
        ),
        (
            "methodology-skill",
            "source/skills/phase1-design/testcase-generate-skill.md",
        ),
        (
            "methodology-fallback",
            "source/skill-fallbacks/skills/phase1-design/testcase-generate-skill.full.md",
        ),
        (
            "methodology-template",
            "source/templates/testcase/be-testcase-template.md",
        ),
        (
            "testing-strategy",
            "source/standards/testing/be-testcase-strategy.md",
        ),
        ("testing-constraints", "constraints/testing.md"),
        (
            "document-storage-skill",
            "source/skills/cross-cutting/document-storage-skill.md",
        ),
        (
            "document-storage-fallback",
            "source/skill-fallbacks/skills/cross-cutting/document-storage-skill.full.md",
        ),
        (
            "requirement-analysis",
            "ae-sdd-doc/RA/ROUTE-STORY-FLOW-PROCESS.md",
        ),
        ("story", "ae-sdd-doc/Story/ROUTE-STORY-FLOW-PROCESS.md"),
    ];
    assert_eq!(refs.len(), expected.len(), "{refs:?}");
    assert!(
        serde_json::to_vec(refs).expect("assetRefs serialize").len() <= 64 * 1024,
        "reference-only projection exceeds 64 KiB"
    );
    assert!(
        refs.iter()
            .filter_map(|reference| reference["byteLength"].as_u64())
            .sum::<u64>()
            > 64 * 1024,
        "external referenced bodies must exceed 64 KiB"
    );
    for (reference, (kind, path)) in refs.iter().zip(expected) {
        let object = reference
            .as_object()
            .unwrap_or_else(|| panic!("asset ref object: {reference}"));
        assert_eq!(object.len(), 4, "fresh ref exposes exactly four fields");
        assert_eq!(reference["kind"], kind);
        assert_eq!(reference["path"], path);
        assert!(!Path::new(path).is_absolute());
        let bytes = fs::read(project_root.join(path)).expect("projected asset exists");
        assert_eq!(reference["sha256"], hex::encode(Sha256::digest(&bytes)));
        assert_eq!(reference["byteLength"], bytes.len() as u64);
    }
    assert!(
        refs.iter()
            .all(|reference| reference["path"] != "ae-sdd-doc/Story/ROUTE-FOREIGN.md"),
        "foreign Story must not enter TestCase refs"
    );
}

fn assert_binding_mismatch_rejected(error: &str) {
    assert!(
        error.contains("WorkItem") || error.contains("Work Item"),
        "durable Work Item mismatch must fail closed: {error}"
    );
}

fn read_state(fixture: &Fixture) -> Value {
    serde_json::from_slice(&fs::read(fixture.state_path()).expect("state bytes"))
        .expect("state JSON")
}

fn read_revision(fixture: &Fixture) -> u64 {
    read_state(fixture)["revision"]
        .as_u64()
        .expect("state revision")
}

fn write_source(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("source parent")).expect("source directory");
    fs::write(path, content).expect("source fixture");
}

struct HostAdapter {
    manifest_path: PathBuf,
    transport: LocalIpcTransport,
    adapter_id: String,
    register_key: String,
}

impl HostAdapter {
    async fn register(state_dir: &Path, adapter_id: &str, register_key: &str) -> Self {
        let adapter = Self {
            manifest_path: state_dir.join("endpoint.v1.json"),
            transport: LocalIpcTransport,
            adapter_id: adapter_id.to_owned(),
            register_key: register_key.to_owned(),
        };
        adapter.call(RpcMethod::HostActionNext, json!({})).await;
        adapter
    }

    async fn call(&self, method: RpcMethod, extra: Value) -> Value {
        let manifest: Value = serde_json::from_slice(
            &fs::read(&self.manifest_path).expect("endpoint manifest bytes"),
        )
        .expect("endpoint manifest JSON");
        let token = manifest["endpointToken"]
            .as_str()
            .expect("endpoint token")
            .to_owned();
        let handshake = json!({
            "jsonrpc":"2.0",
            "id":"host-handshake",
            "method":RpcMethod::RuntimeHandshake,
            "params":{
                "protocolRange":manifest["protocolRange"],
                "clientBuild":"story-friction-host-adapter",
                "clientKind":ClientKind::HostAdapter,
                "endpointToken":token,
                "expectedBootId":manifest["bootId"],
                "expectedPolicyDigest":manifest["policyDigest"],
            }
        });
        let mut register = params(json!({
            "adapterId":self.adapter_id,
            "capabilities":["create","attest"],
        }));
        register.capability_token = Some(token.clone());
        register.idempotency_key = Some(self.register_key.clone());
        let mut payload = json!({"adapterId":self.adapter_id});
        if let Some(fields) = extra.as_object() {
            for (name, value) in fields {
                payload[name] = value.clone();
            }
        }
        let mut request = params(payload);
        request.capability_token = Some(token);
        request.idempotency_key = Some(format!(
            "{}-{}-{}",
            self.register_key,
            method.as_str().replace('.', "-"),
            Uuid::new_v4()
        ));
        let frames = [
            serde_json::to_vec(&handshake).expect("handshake frame"),
            serde_json::to_vec(&json!({
                "jsonrpc":"2.0",
                "id":"host-register",
                "method":RpcMethod::HostRegister,
                "params":register,
            }))
            .expect("register frame"),
            serde_json::to_vec(&json!({
                "jsonrpc":"2.0",
                "id":"host-call",
                "method":method,
                "params":request,
            }))
            .expect("host call frame"),
        ];
        let responses = self
            .transport
            .exchange(
                manifest["endpoint"].as_str().expect("endpoint address"),
                &frames,
                Duration::from_secs(10),
            )
            .await
            .expect("host adapter connection exchanges frames");
        assert_eq!(responses.len(), 3, "one response per host frame");
        for (index, label) in ["handshake", "host.register"].into_iter().enumerate() {
            let response: Value =
                serde_json::from_slice(&responses[index]).expect("host response JSON");
            assert!(response.get("result").is_some(), "{label}: {response}");
        }
        let response: Value = serde_json::from_slice(&responses[2]).expect("host response JSON");
        response
            .get("result")
            .unwrap_or_else(|| panic!("{} failed: {response}", method.as_str()))
            .clone()
    }
}

struct Fixture {
    root: TempDir,
    project_root: PathBuf,
    runtime_dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("process fixture root");
        let project_root = root.path().join("project");
        let runtime_dir = root.path().join("runtime");
        fs::create_dir_all(project_root.join(".auto-engineering/story-flow-runtime-process"))
            .expect("state directory");
        fs::create_dir_all(project_root.join("constraints")).expect("constraints directory");
        fs::create_dir_all(project_root.join("source/skills/phase1-design"))
            .expect("methodology directory");
        fs::create_dir_all(project_root.join("source/skill-fallbacks/skills/phase1-design"))
            .expect("methodology fallback directory");
        fs::create_dir_all(project_root.join("source/standards/runtime"))
            .expect("methodology catalog directory");
        fs::create_dir_all(project_root.join("ae-sdd-doc/RA")).expect("RA directory");
        for directory in [
            "source/templates/testcase",
            "source/standards/testing",
            "source/skills/cross-cutting",
            "source/skill-fallbacks/skills/cross-cutting",
            "ae-sdd-doc/Story",
        ] {
            fs::create_dir_all(project_root.join(directory)).expect("TestCase asset directory");
        }
        fs::create_dir_all(&runtime_dir).expect("runtime directory");
        fs::write(
            project_root.join("constraints/README.md"),
            "# process fixture constraints\n",
        )
        .expect("constraints fixture");
        fs::write(
            project_root.join("source/skills/phase1-design/requirement-analysis-skill.md"),
            "# Requirement Analysis\n\nProduce the bounded RA artifact.\n",
        )
        .expect("RA methodology fixture");
        fs::write(
            project_root.join(
                "source/skill-fallbacks/skills/phase1-design/requirement-analysis-skill.full.md",
            ),
            "# Requirement Analysis fallback\n\nProduce the bounded RA artifact.\n",
        )
        .expect("RA methodology fallback fixture");
        fs::write(
            project_root.join("source/SKILL.md"),
            "# ae-sdd methodology entry\n",
        )
        .expect("methodology entry fixture");
        fs::write(
            project_root.join("source/standards/runtime/methodology-catalog.v1.json"),
            serde_json::to_vec(&json!({
                "schemaVersion":"ae-sdd-methodology-catalog/v1",
                "catalogVersion":"1.0.0",
                "entries":[
                    {
                        "skillId":"phase1-design.story-generate",
                        "seriesKind":"story",
                        "activity":"generate",
                        "variant":"test-v1",
                        "version":"1.0.0",
                        "activation":"workflow",
                        "spawnPolicy":"physical_series",
                        "compactRef":"skills/phase1-design/story-generate-skill.md",
                        "fallbackRef":"skill-fallbacks/skills/phase1-design/story-generate-skill.full.md",
                        "routePredicates":[{
                            "fact":"required-series",
                            "operator":"contains",
                            "value":"story"
                        }],
                        "requiredInputs":[],
                        "deliverableKinds":["story"],
                        "requiredGates":[],
                        "toolDependencies":[]
                    },
                    {
                        "skillId":"phase1-design.testcase-generate",
                        "seriesKind":"testcase",
                        "activity":"generate",
                        "variant":"test-v1",
                        "version":"1.0.0",
                        "activation":"workflow",
                        "spawnPolicy":"physical_series",
                        "compactRef":"skills/phase1-design/testcase-generate-skill.md",
                        "fallbackRef":"skill-fallbacks/skills/phase1-design/testcase-generate-skill.full.md",
                        "routePredicates":[{
                            "fact":"required-series",
                            "operator":"contains",
                            "value":"testcase"
                        }],
                        "requiredInputs":[],
                        "deliverableKinds":["test-case"],
                        "requiredGates":[],
                        "toolDependencies":[]
                    },
                    {
                        "skillId":"cross-cutting.document-storage",
                        "seriesKind":"document-storage",
                        "variant":"test-v1",
                        "version":"1.0.0",
                        "activation":"capability",
                        "spawnPolicy":"inline",
                        "compactRef":"skills/cross-cutting/document-storage-skill.md",
                        "fallbackRef":"skill-fallbacks/skills/cross-cutting/document-storage-skill.full.md",
                        "routePredicates":[],
                        "requiredInputs":["document-intent"],
                        "deliverableKinds":[],
                        "requiredGates":[],
                        "toolDependencies":["document-store"]
                    }
                ]
            }))
            .expect("methodology catalog serializes"),
        )
        .expect("methodology catalog fixture");
        for (path, body) in [
            (
                "constraints/testing.md",
                b"# testing constraints\n".as_slice(),
            ),
            (
                "source/skills/phase1-design/testcase-generate-skill.md",
                b"# testcase skill\n".as_slice(),
            ),
            (
                "source/skill-fallbacks/skills/phase1-design/testcase-generate-skill.full.md",
                b"# testcase fallback\n".as_slice(),
            ),
            (
                "source/skills/phase1-design/story-generate-skill.md",
                b"# story skill\n".as_slice(),
            ),
            (
                "source/skill-fallbacks/skills/phase1-design/story-generate-skill.full.md",
                b"# story fallback\n".as_slice(),
            ),
            (
                "source/templates/testcase/be-testcase-template.md",
                b"# testcase template\n".as_slice(),
            ),
            (
                "source/standards/testing/be-testcase-strategy.md",
                b"# testcase strategy\n".as_slice(),
            ),
            (
                "source/skills/cross-cutting/document-storage-skill.md",
                b"# document storage\n".as_slice(),
            ),
            (
                "ae-sdd-doc/Story/ROUTE-STORY-FLOW-PROCESS.md",
                b"# active Story\n".as_slice(),
            ),
            (
                "ae-sdd-doc/Story/ROUTE-FOREIGN.md",
                b"# foreign Story\n".as_slice(),
            ),
        ] {
            fs::write(project_root.join(path), body).expect("TestCase asset fixture");
        }
        fs::write(
            project_root
                .join("source/skill-fallbacks/skills/cross-cutting/document-storage-skill.full.md"),
            vec![b'x'; 70 * 1024],
        )
        .expect("large document storage fallback");
        fs::write(
            project_root.join("ae-sdd-doc/RA/ROUTE-STORY-FLOW-PROCESS.md"),
            include_str!("../../../tests/fixtures/gates/ra-v2/micro-doc-config.md").replacen(
                "最高分 = 1 -> Scale = micro。",
                "最高分 = 1 => Scale = micro。",
                1,
            ),
        )
        .expect("malformed RA fixture");
        let state = json!({
            "stateMachineName": WORK_ITEM_ID,
            "entryNode": "ROUTE",
            "revision": 1,
            "lastFencingToken": 0,
            "scale": "medium",
            "selectedDesign": "DR",
            "phase": "story-generated",
            "currentPhase": "story-generated",
            "documentPaths": {
                "RA": "ae-sdd-doc/RA/ROUTE-STORY-FLOW-PROCESS.md"
            },
            "routeApproved":true,
            "routeDecision":{
                "designRoute":"dr",
                "requiredSeries":["requirement-analysis","design-review","story","testcase"]
            },
            "engineeringRoute":{
                "decision":{
                    "scale":"medium",
                    "designRoute":"dr",
                    "requiredSeries":["requirement-analysis","design-review","story","testcase"]
                }
            },
            "routeDocuments":{"RA":true,"DR":true,"STORY":true},
            "activeStory":"STORY-STORY-FLOW-ACTIVE",
            "storyStates":{
                "STORY-STORY-FLOW-ACTIVE":{
                    "docPath":"ae-sdd-doc/Story/ROUTE-STORY-FLOW-PROCESS.md"
                },
                "STORY-STORY-FLOW-FOREIGN":{
                    "docPath":"ae-sdd-doc/Story/ROUTE-FOREIGN.md"
                }
            },
            "executionPlan": {
                "goal": "prove one Hook event creates one turn and receipt",
                "changedPaths": ["bins/ae-sdd-daemon/tests/story_flow_runtime_process.rs"],
                "verification": [],
                "risks": [],
                "sourceReads": ["constraints/README.md"],
                "approved": true,
                "approvedAt": "2026-08-11T00:00:00Z",
                "approvedBy": "user:test"
            }
        });
        fs::write(
            project_root.join(".auto-engineering/story-flow-runtime-process/state.json"),
            serde_json::to_vec_pretty(&state).expect("state serializes"),
        )
        .expect("state fixture");
        Self {
            root,
            project_root,
            runtime_dir,
        }
    }

    fn allowed_root(&self) -> &Path {
        self.root.path()
    }

    fn state_path(&self) -> PathBuf {
        self.project_root
            .join(".auto-engineering/story-flow-runtime-process/state.json")
    }

    fn install_dir(&self) -> PathBuf {
        self.root.path().join("installed")
    }

    fn install_current_daemon(&self) -> PathBuf {
        let package_dir = self.root.path().join("package");
        let install_dir = self.install_dir();
        let build_target = self.root.path().join("native-build-target");
        fs::create_dir_all(&package_dir).expect("package directory");
        let package_daemon = package_dir.join("ae-sddd.exe");
        fs::copy(env!("CARGO_BIN_EXE_ae-sddd"), &package_daemon)
            .expect("current daemon enters the package");

        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root");
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let built = Command::new(cargo)
            .current_dir(&workspace_root)
            .args(["build", "-p", "ae-sdd-build", "--bin", "ae-sdd-build"])
            .arg("--target-dir")
            .arg(&build_target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .expect("native installer build runs");
        assert!(
            built.status.success(),
            "native installer build failed: {}",
            String::from_utf8_lossy(&built.stderr)
        );

        let request_path = self.root.path().join("native-install.json");
        fs::write(
            &request_path,
            serde_json::to_vec_pretty(&json!({
                "schemaVersion":"ae-sdd-native-job/v1",
                "entrypoint":"install",
                "actor":"story-friction-process-test",
                "reason":"prove package-to-install parity for STORY-008-BE",
                "idempotencyKey":"story-friction-native-install",
                "mode":"apply",
                "allowedRoots":[self.root.path()],
                "job":{
                    "kind":"install",
                    "input":{
                        "packageDirectory":package_dir,
                        "targetDirectory":install_dir,
                    }
                }
            }))
            .expect("native install request serializes"),
        )
        .expect("native install request");
        let installer = build_target.join("debug/ae-sdd-build.exe");
        let installed = Command::new(&installer)
            .args(["native-job", "--request"])
            .arg(&request_path)
            .arg("--json")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("native install runs");
        assert!(
            installed.status.success(),
            "native install failed: {}",
            String::from_utf8_lossy(&installed.stderr)
        );
        let installed_daemon = self.install_dir().join("ae-sddd.exe");
        assert!(installed_daemon.is_file(), "installed daemon exists");
        installed_daemon
    }
}

struct DaemonProcess {
    child: Child,
}

impl DaemonProcess {
    async fn start(state_dir: &Path, allowed_root: &Path) -> Self {
        Self::start_binary(
            Path::new(env!("CARGO_BIN_EXE_ae-sddd")),
            state_dir,
            allowed_root,
        )
        .await
    }

    async fn start_binary(binary: &Path, state_dir: &Path, allowed_root: &Path) -> Self {
        let mut child = Command::new(binary)
            .arg("serve")
            .arg("--state-dir")
            .arg(state_dir)
            .arg("--allowed-root")
            .arg(allowed_root)
            .arg("--policy-digest")
            .arg("c".repeat(64))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("real ae-sddd process starts");
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(status) = child.try_wait().expect("daemon status") {
                panic!("ae-sddd exited before ready ({status})");
            }
            if state_dir.join("endpoint.v1.json").is_file()
                && daemon_client(state_dir, ClientKind::Cli)
                    .call::<Value>(RpcMethod::RuntimeStatus, params(json!({})))
                    .await
                    .is_ok()
            {
                return Self { child };
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("ae-sddd did not become ready");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    fn crash(&mut self) {
        if self.child.try_wait().expect("daemon status").is_none() {
            #[cfg(windows)]
            {
                let output = Command::new("taskkill.exe")
                    .args(["/PID", &self.child.id().to_string(), "/T", "/F"])
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .expect("daemon process tree termination runs");
                assert!(
                    output.status.success(),
                    "daemon process tree termination failed: stdout={} stderr={}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            #[cfg(not(windows))]
            self.child.kill().expect("daemon kill");
            self.child.wait().expect("daemon exit");
        }
    }

    fn id(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for DaemonProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn descendant_files(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory).unwrap_or_else(|error| {
            panic!("read owned directory {}: {error}", directory.display())
        });
        for entry in entries {
            let entry = entry.expect("owned directory entry");
            let path = entry.path();
            let file_type = entry.file_type().expect("owned entry type");
            if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file() {
                files.push(path);
            }
        }
    }
    files
}

#[cfg(windows)]
fn assert_tree_unlocked(root: &Path) {
    use std::os::windows::fs::OpenOptionsExt;

    for path in descendant_files(root) {
        fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .unwrap_or_else(|error| {
                panic!("owned file remains locked {}: {error}", path.display())
            });
    }
}

#[cfg(not(windows))]
fn assert_tree_unlocked(root: &Path) {
    for path in descendant_files(root) {
        fs::OpenOptions::new()
            .read(true)
            .open(&path)
            .unwrap_or_else(|error| {
                panic!("owned file is not readable {}: {error}", path.display())
            });
    }
}

#[cfg(windows)]
fn assert_no_owned_processes(parent_pid: u32, roots: &[&Path]) {
    let script = r#"
$parentPid = [uint32]$env:AE_SDD_TEST_PARENT_PID
$roots = @($env:AE_SDD_TEST_RUNTIME_ROOT, $env:AE_SDD_TEST_INSTALL_ROOT)
$processes = @(Get-CimInstance Win32_Process)
$ownedIds = [System.Collections.Generic.HashSet[uint32]]::new()
[void]$ownedIds.Add($parentPid)
$owned = @()
$added = $true
while ($added) {
  $added = $false
  foreach ($process in $processes) {
    $processId = [uint32]$process.ProcessId
    if ($processId -ne $parentPid -and $ownedIds.Contains([uint32]$process.ParentProcessId) -and $ownedIds.Add($processId)) {
      $owned += $process
      $added = $true
    }
  }
}
foreach ($process in $processes) {
  $processId = [uint32]$process.ProcessId
  if ($processId -ne $parentPid -and -not $ownedIds.Contains($processId) -and $process.ExecutablePath) {
    foreach ($root in $roots) {
      if ($process.ExecutablePath.StartsWith($root, [StringComparison]::OrdinalIgnoreCase)) {
        [void]$ownedIds.Add($processId)
        $owned += $process
        break
      }
    }
  }
}
if ($owned.Count -ne 0) {
  $owned | Select-Object ProcessId, ParentProcessId, ExecutablePath | ConvertTo-Json -Compress
  exit 1
}
"#;
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("AE_SDD_TEST_PARENT_PID", parent_pid.to_string())
        .env("AE_SDD_TEST_RUNTIME_ROOT", roots[0])
        .env("AE_SDD_TEST_INSTALL_ROOT", roots[1])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("owned process inspection runs");
    assert!(
        output.status.success(),
        "owned daemon descendant remains: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(not(windows))]
fn assert_no_owned_processes(_parent_pid: u32, _roots: &[&Path]) {}

fn assert_no_native_transaction_residue(root: &Path) {
    let mut pending = vec![root.to_path_buf()];
    let mut residues = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("scan native transaction residue") {
            let entry = entry.expect("native transaction entry");
            let path = entry.path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.contains(".ae-sdd-stage-")
                        || name.contains(".ae-sdd-backup-")
                        || name.contains(".ae-sdd-write-")
                })
            {
                residues.push(path.clone());
            }
            if entry
                .file_type()
                .expect("native transaction entry type")
                .is_dir()
            {
                pending.push(path);
            }
        }
    }
    assert!(
        residues.is_empty(),
        "native install transaction residue remains: {residues:?}"
    );
}

async fn register_and_cut_over(
    cli: &DaemonClient,
    admin: &DaemonClient,
    fixture: &Fixture,
) -> WorkspaceResult {
    let mut register = params(json!({
        "projectRoot": fixture.project_root.to_string_lossy(),
        "projectKey": PROJECT_KEY,
    }));
    register.idempotency_key = Some("workspace-register-story-flow".to_owned());
    let registered = cli
        .call::<WorkspaceResult>(RpcMethod::WorkspaceRegister, register)
        .await
        .expect("workspace register");

    let mut drain = params(json!({"stop": false}));
    drain.idempotency_key = Some("runtime-drain-story-flow".to_owned());
    drain.confirmation = Some(confirmation("runtime-drain-story-flow"));
    admin
        .call::<Value>(RpcMethod::RuntimeDrain, drain)
        .await
        .expect("runtime drain");

    let parity = WorkspaceParityEvidence {
        comparison_count: 1,
        mismatch_count: 0,
        source_revision: 1,
        legacy_digest: "a".repeat(64),
        rust_digest: "a".repeat(64),
        observed_at_unix_ms: now_unix_ms(),
    };
    let parity_digest =
        InputFingerprint::digest(serde_json::to_vec(&parity).expect("parity serializes"))
            .to_string();
    let mut transition = params(json!({
        "targetMode": WorkspaceMode::RustCanary,
        "reason": "Story-flow process regression fixture",
        "parityDigest": parity_digest,
        "parity": parity,
    }));
    transition.workspace_id = Some(registered.workspace_id);
    transition.idempotency_key = Some("workspace-cutover-story-flow".to_owned());
    transition.confirmation = Some(confirmation("workspace-cutover-story-flow"));
    admin
        .call(RpcMethod::WorkspaceModeTransition, transition)
        .await
        .expect("workspace cutover")
}

async fn open_root(client: &DaemonClient, workspace: &WorkspaceResult) -> SessionResult {
    let mut request = params(json!({
        "externalKey": EXTERNAL_SESSION_KEY,
        "role": "root",
        "engaged": true,
    }));
    request.workspace_id = Some(workspace.workspace_id.clone());
    request.work_item_id = Some(WORK_ITEM_ID.to_owned());
    request.agent_id = Some(AGENT_ID.to_owned());
    request.idempotency_key = Some("session-open-story-flow".to_owned());
    client
        .call(RpcMethod::SessionOpen, request)
        .await
        .expect("root session open")
}

fn daemon_client(state_dir: &Path, kind: ClientKind) -> DaemonClient {
    DaemonClient::new(
        state_dir.join("endpoint.v1.json"),
        kind,
        Arc::new(LocalIpcTransport),
        Duration::from_secs(5),
    )
}

fn params(payload: Value) -> RequestParams<Value> {
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
        deadline_ms: 10_000,
        payload,
    }
}

fn confirmation(id: &str) -> ConfirmationRef {
    ConfirmationRef {
        confirmation_id: id.to_owned(),
        approved_by: "user:test".to_owned(),
        approved_at: "2026-08-11T00:00:00Z".to_owned(),
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Unix epoch")
        .as_millis()
        .try_into()
        .expect("timestamp fits u64")
}
