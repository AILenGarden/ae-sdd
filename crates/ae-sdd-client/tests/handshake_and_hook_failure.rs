use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use ae_sdd_client::{ClientError, ClientTransport, DaemonClient, HookClient, HookInvocation};
use ae_sdd_protocol::{
    ClientKind, EndpointManifest, PROTOCOL_RANGE_V1, PROTOCOL_VERSION_V1, RequestParams, RpcMethod,
    SecretString,
};
use serde_json::{Value, json};
use tempfile::NamedTempFile;

#[derive(Clone, Copy)]
enum FakeMode {
    WrongPolicy,
    Unavailable,
}

#[derive(Clone, Copy)]
struct FakeTransport {
    mode: FakeMode,
}

impl ClientTransport for FakeTransport {
    fn exchange<'a>(
        &'a self,
        _endpoint: &'a str,
        payloads: &'a [Vec<u8>],
        _timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Vec<u8>>, ClientError>> + Send + 'a>> {
        Box::pin(async move {
            if matches!(self.mode, FakeMode::Unavailable) {
                return Err(ClientError::DaemonUnavailable);
            }
            let handshake_id = serde_json::from_slice::<Value>(&payloads[0])
                .map_err(|_| ClientError::Protocol)?["id"]
                .clone();
            let request_id = serde_json::from_slice::<Value>(&payloads[1])
                .map_err(|_| ClientError::Protocol)?["id"]
                .clone();
            let policy = if matches!(self.mode, FakeMode::WrongPolicy) {
                "c".repeat(64)
            } else {
                "a".repeat(64)
            };
            let handshake = json!({
                "jsonrpc":"2.0",
                "id":handshake_id,
                "result": {
                    "protocolVersion":PROTOCOL_VERSION_V1,
                    "bootId":"00000000-0000-0000-0000-000000000001",
                    "eventStoreId":"00000000-0000-0000-0000-000000000002",
                    "daemonBuild":"ae-sdd-daemon/test",
                    "capabilities":["hook-fail-closed-v1"],
                    "policyDigest":policy,
                    "operationSchemaDigest":"b".repeat(64),
                    "limits": {
                        "maxFrameBytes":1048576,
                        "maxAgentDepth":2,
                        "maxStringBytes":65536,
                        "maxCollectionItems":128,
                        "maxDeadlineMs":30000,
                        "hookDeadlineMs":250,
                        "maxChildResultBytes":65536,
                        "maxChildSummaryBytes":4096,
                        "maxContextProjectionBytes":65536
                    },
                    "capabilityKeyId":"d".repeat(64),
                    "capabilityPublicKey":"e".repeat(64)
                }
            });
            let response = json!({"jsonrpc":"2.0","id":request_id,"result":{"ok":true}});
            Ok(vec![
                serde_json::to_vec(&handshake).map_err(|_| ClientError::Protocol)?,
                serde_json::to_vec(&response).map_err(|_| ClientError::Protocol)?,
            ])
        })
    }
}

#[derive(Clone)]
struct RecoverOnceTransport {
    attempts: Arc<AtomicUsize>,
    request_params: Arc<Mutex<Vec<Value>>>,
}

impl ClientTransport for RecoverOnceTransport {
    fn exchange<'a>(
        &'a self,
        _endpoint: &'a str,
        payloads: &'a [Vec<u8>],
        _timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Vec<u8>>, ClientError>> + Send + 'a>> {
        Box::pin(async move {
            let request =
                serde_json::from_slice::<Value>(&payloads[1]).map_err(|_| ClientError::Protocol)?;
            self.request_params
                .lock()
                .expect("request params lock")
                .push(request["params"].clone());
            if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(ClientError::DaemonUnavailable);
            }

            let handshake_id = serde_json::from_slice::<Value>(&payloads[0])
                .map_err(|_| ClientError::Protocol)?["id"]
                .clone();
            let request_id = request["id"].clone();
            let handshake = json!({
                "jsonrpc":"2.0",
                "id":handshake_id,
                "result": {
                    "protocolVersion":PROTOCOL_VERSION_V1,
                    "bootId":"00000000-0000-0000-0000-000000000001",
                    "eventStoreId":"00000000-0000-0000-0000-000000000002",
                    "daemonBuild":"ae-sdd-daemon/test",
                    "capabilities":["hook-fail-closed-v1"],
                    "policyDigest":"a".repeat(64),
                    "operationSchemaDigest":"b".repeat(64),
                    "limits": {
                        "maxFrameBytes":1048576,
                        "maxAgentDepth":2,
                        "maxStringBytes":65536,
                        "maxCollectionItems":128,
                        "maxDeadlineMs":30000,
                        "hookDeadlineMs":250,
                        "maxChildResultBytes":65536,
                        "maxChildSummaryBytes":4096,
                        "maxContextProjectionBytes":65536
                    },
                    "capabilityKeyId":"d".repeat(64),
                    "capabilityPublicKey":"e".repeat(64)
                }
            });
            let response = json!({
                "jsonrpc":"2.0",
                "id":request_id,
                "result": {
                    "engaged":true,
                    "decision":"allow",
                    "context":null,
                    "eventSeq":42,
                    "offline":false
                }
            });
            Ok(vec![
                serde_json::to_vec(&handshake).map_err(|_| ClientError::Protocol)?,
                serde_json::to_vec(&response).map_err(|_| ClientError::Protocol)?,
            ])
        })
    }
}

fn manifest_file() -> NamedTempFile {
    let file = NamedTempFile::new().expect("manifest temp file");
    let manifest = EndpointManifest {
        schema_version: "ae-sdd-endpoint/v1".to_owned(),
        pid: 1,
        boot_id: "00000000-0000-0000-0000-000000000001".to_owned(),
        event_store_id: "00000000-0000-0000-0000-000000000002".to_owned(),
        endpoint: "test-endpoint".to_owned(),
        endpoint_token: SecretString::new("endpoint-token"),
        protocol_range: PROTOCOL_RANGE_V1.to_owned(),
        daemon_version: "test".to_owned(),
        policy_digest: "a".repeat(64),
        capability_key_id: "d".repeat(64),
        capability_public_key: "e".repeat(64),
        started_at: "2026-01-01T00:00:00Z".to_owned(),
    };
    std::fs::write(
        file.path(),
        serde_json::to_vec(&manifest).expect("manifest serializes"),
    )
    .expect("manifest writes");
    file
}

fn params() -> RequestParams<Value> {
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
        deadline_ms: 1_000,
        payload: json!({}),
    }
}

#[tokio::test]
async fn handshake_rejects_manifest_policy_drift() {
    let manifest = manifest_file();
    let client = DaemonClient::new(
        manifest.path(),
        ClientKind::Cli,
        Arc::new(FakeTransport {
            mode: FakeMode::WrongPolicy,
        }),
        Duration::from_secs(1),
    );
    let error = client
        .call::<Value>(RpcMethod::RuntimeStatus, params())
        .await
        .expect_err("policy drift must fail the handshake");
    assert!(matches!(error, ClientError::Protocol));
}

#[tokio::test]
async fn hook_failure_is_fail_closed_when_daemon_is_unavailable() {
    let manifest = manifest_file();
    let client = DaemonClient::new(
        manifest.path(),
        ClientKind::Hook,
        Arc::new(FakeTransport {
            mode: FakeMode::Unavailable,
        }),
        Duration::from_millis(250),
    );
    let mut hook_params = params();
    hook_params.workspace_id = Some("workspace".to_owned());
    hook_params.agent_id = Some("agent".to_owned());
    hook_params.session_id = Some("00000000-0000-0000-0000-000000000003".to_owned());
    hook_params.turn_id = Some("turn".to_owned());
    hook_params.work_item_id = Some("WORK".to_owned());
    hook_params.payload = json!({
        "hookEventId":"event",
        "turnSeq":1,
        "hostPayload":{}
    });
    let outcome = HookClient::new(&client)
        .invoke(HookInvocation {
            method: RpcMethod::HookPreTool,
            params: hook_params,
            engaged: true,
            offline_capability: Some("not-a-valid-token".to_owned()),
            now_unix_ms: 1,
        })
        .await
        .expect("Hook failure is represented as a fail-closed outcome");
    assert!(!outcome.engaged);
    assert_eq!(outcome.decision, ae_sdd_protocol::HookDecision::Deny);
    assert!(outcome.offline);
}

#[tokio::test]
async fn hook_recovery_retries_once_with_identical_params() {
    let manifest = manifest_file();
    let attempts = Arc::new(AtomicUsize::new(0));
    let request_params = Arc::new(Mutex::new(Vec::new()));
    let client = DaemonClient::new(
        manifest.path(),
        ClientKind::Hook,
        Arc::new(RecoverOnceTransport {
            attempts: Arc::clone(&attempts),
            request_params: Arc::clone(&request_params),
        }),
        Duration::from_millis(250),
    );
    let recoveries = Arc::new(AtomicUsize::new(0));
    let recoveries_for_callback = Arc::clone(&recoveries);
    let mut hook_params = params();
    hook_params.session_id = Some("00000000-0000-0000-0000-000000000003".to_owned());
    hook_params.idempotency_key = Some("hook-event-1".to_owned());
    hook_params.payload = json!({"hookEventId":"event-1","hostPayload":{"tool":"read"}});

    let outcome = HookClient::new(&client)
        .invoke_with_recovery(
            HookInvocation {
                method: RpcMethod::HookPreTool,
                params: hook_params,
                engaged: true,
                offline_capability: None,
                now_unix_ms: 1,
            },
            move || async move {
                recoveries_for_callback.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        )
        .await
        .expect("successful recovery replays the Hook");

    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(recoveries.load(Ordering::SeqCst), 1);
    assert_eq!(outcome.event_seq, 42);
    assert!(!outcome.offline);
    let captured = request_params.lock().expect("request params lock");
    assert_eq!(captured.len(), 2);
    assert_eq!(captured[0], captured[1]);
    assert_eq!(captured[1]["idempotencyKey"], "hook-event-1");
}

#[tokio::test]
async fn hook_recovery_failure_applies_existing_offline_policy_without_replay() {
    let manifest = manifest_file();
    let attempts = Arc::new(AtomicUsize::new(0));
    let client = DaemonClient::new(
        manifest.path(),
        ClientKind::Hook,
        Arc::new(RecoverOnceTransport {
            attempts: Arc::clone(&attempts),
            request_params: Arc::new(Mutex::new(Vec::new())),
        }),
        Duration::from_millis(250),
    );
    let recoveries = Arc::new(AtomicUsize::new(0));
    let recoveries_for_callback = Arc::clone(&recoveries);

    let outcome = HookClient::new(&client)
        .invoke_with_recovery(
            HookInvocation {
                method: RpcMethod::HookPreTool,
                params: params(),
                engaged: true,
                offline_capability: None,
                now_unix_ms: 1,
            },
            move || async move {
                recoveries_for_callback.fetch_add(1, Ordering::SeqCst);
                Err(ClientError::DaemonUnavailable)
            },
        )
        .await
        .expect("failed recovery is represented by offline policy");

    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    assert_eq!(recoveries.load(Ordering::SeqCst), 1);
    assert_eq!(outcome.decision, ae_sdd_protocol::HookDecision::Deny);
    assert!(outcome.offline);
}

#[tokio::test]
async fn non_recoverable_hook_error_does_not_invoke_recovery() {
    let manifest = manifest_file();
    let client = DaemonClient::new(
        manifest.path(),
        ClientKind::Hook,
        Arc::new(FakeTransport {
            mode: FakeMode::WrongPolicy,
        }),
        Duration::from_millis(250),
    );
    let recoveries = Arc::new(AtomicUsize::new(0));
    let recoveries_for_callback = Arc::clone(&recoveries);

    let error = HookClient::new(&client)
        .invoke_with_recovery(
            HookInvocation {
                method: RpcMethod::HookPreTool,
                params: params(),
                engaged: true,
                offline_capability: None,
                now_unix_ms: 1,
            },
            move || async move {
                recoveries_for_callback.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        )
        .await
        .expect_err("protocol errors must remain visible");

    assert!(matches!(error, ClientError::Protocol));
    assert_eq!(recoveries.load(Ordering::SeqCst), 0);
}
