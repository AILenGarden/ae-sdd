//! SPI-3 coverage for `DaemonClient::call_with_ensure`: recovery on
//! `EndpointManifest`/`DaemonUnavailable`, exactly-once `ensure`/replay,
//! verbatim `idempotency_key` reuse, and no-recovery on other error kinds.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use ae_sdd_client::{ClientError, ClientTransport, DaemonClient};
use ae_sdd_protocol::{
    ClientKind, EndpointManifest, PROTOCOL_RANGE_V1, PROTOCOL_VERSION_V1, RequestParams, RpcMethod,
    SecretString,
};
use serde_json::{Value, json};
use tempfile::NamedTempFile;

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
        idempotency_key: Some("idem-1".to_owned()),
        confirmation: None,
        deadline_ms: 1_000,
        payload: json!({}),
    }
}

fn handshake_ok(handshake_id: Value) -> Value {
    json!({
        "jsonrpc":"2.0",
        "id":handshake_id,
        "result": {
            "protocolVersion":PROTOCOL_VERSION_V1,
            "bootId":"00000000-0000-0000-0000-000000000001",
            "eventStoreId":"00000000-0000-0000-0000-000000000002",
            "daemonBuild":"ae-sdd-daemon/test",
            "capabilities":[],
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
    })
}

/// Fails the first `exchange` with `DaemonUnavailable`, succeeds afterwards.
/// Records every request's params (post-handshake JSON) it observes.
#[derive(Clone)]
struct RecoverOnceTransport {
    attempts: Arc<AtomicUsize>,
    seen_params: Arc<Mutex<Vec<Value>>>,
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
            self.seen_params
                .lock()
                .expect("seen params lock")
                .push(request["params"].clone());
            if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(ClientError::DaemonUnavailable);
            }
            let handshake_id = serde_json::from_slice::<Value>(&payloads[0])
                .map_err(|_| ClientError::Protocol)?["id"]
                .clone();
            let response = json!({
                "jsonrpc":"2.0",
                "id":request["id"].clone(),
                "result": {"ok": true}
            });
            Ok(vec![
                serde_json::to_vec(&handshake_ok(handshake_id))
                    .map_err(|_| ClientError::Protocol)?,
                serde_json::to_vec(&response).map_err(|_| ClientError::Protocol)?,
            ])
        })
    }
}

/// Always returns a remote (non-recoverable) error.
struct RemoteErrorTransport;

impl ClientTransport for RemoteErrorTransport {
    fn exchange<'a>(
        &'a self,
        _endpoint: &'a str,
        payloads: &'a [Vec<u8>],
        _timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Vec<u8>>, ClientError>> + Send + 'a>> {
        Box::pin(async move {
            let request =
                serde_json::from_slice::<Value>(&payloads[1]).map_err(|_| ClientError::Protocol)?;
            let handshake_id = serde_json::from_slice::<Value>(&payloads[0])
                .map_err(|_| ClientError::Protocol)?["id"]
                .clone();
            let error_response = json!({
                "jsonrpc":"2.0",
                "id":request["id"].clone(),
                "error": {
                    "code": -32000,
                    "message": "denied",
                    "data": {
                        "schemaVersion": "ae-sdd-error/v1",
                        "stableCode": "REVISION_CONFLICT",
                        "retryable": false
                    }
                }
            });
            Ok(vec![
                serde_json::to_vec(&handshake_ok(handshake_id))
                    .map_err(|_| ClientError::Protocol)?,
                serde_json::to_vec(&error_response).map_err(|_| ClientError::Protocol)?,
            ])
        })
    }
}

/// Always fails with `DaemonUnavailable`, regardless of attempt count.
struct AlwaysUnavailableTransport;

impl ClientTransport for AlwaysUnavailableTransport {
    fn exchange<'a>(
        &'a self,
        _endpoint: &'a str,
        _payloads: &'a [Vec<u8>],
        _timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Vec<u8>>, ClientError>> + Send + 'a>> {
        Box::pin(async move { Err(ClientError::DaemonUnavailable) })
    }
}

#[tokio::test]
async fn call_with_ensure_recovers_once_and_replays_with_identical_params() {
    let manifest = manifest_file();
    let attempts = Arc::new(AtomicUsize::new(0));
    let seen_params = Arc::new(Mutex::new(Vec::new()));
    let client = DaemonClient::new(
        manifest.path(),
        ClientKind::Cli,
        Arc::new(RecoverOnceTransport {
            attempts: Arc::clone(&attempts),
            seen_params: Arc::clone(&seen_params),
        }),
        Duration::from_millis(250),
    );
    let recoveries = Arc::new(AtomicUsize::new(0));
    let recoveries_for_ensure = Arc::clone(&recoveries);

    let result: Value = client
        .call_with_ensure(RpcMethod::RuntimeStatus, params(), || async move {
            recoveries_for_ensure.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
        .await
        .expect("recovery then replay succeeds");

    assert_eq!(result, json!({"ok": true}));
    assert_eq!(attempts.load(Ordering::SeqCst), 2, "exactly one replay");
    assert_eq!(
        recoveries.load(Ordering::SeqCst),
        1,
        "ensure runs exactly once"
    );

    let captured = seen_params.lock().expect("seen params lock");
    assert_eq!(captured.len(), 2);
    assert_eq!(captured[0], captured[1], "replay reuses params verbatim");
    assert_eq!(captured[1]["idempotencyKey"], "idem-1");
}

#[tokio::test]
async fn call_with_ensure_does_not_invoke_ensure_for_remote_errors() {
    let manifest = manifest_file();
    let client = DaemonClient::new(
        manifest.path(),
        ClientKind::Cli,
        Arc::new(RemoteErrorTransport),
        Duration::from_millis(250),
    );
    let recoveries = Arc::new(AtomicUsize::new(0));
    let recoveries_for_ensure = Arc::clone(&recoveries);

    let error = client
        .call_with_ensure::<Value, _>(RpcMethod::RuntimeStatus, params(), || async move {
            recoveries_for_ensure.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
        .await
        .expect_err("remote errors must not trigger recovery");

    assert!(
        matches!(error, ClientError::Remote { .. }),
        "expected Remote, got {error:?}"
    );
    assert_eq!(recoveries.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn call_with_ensure_surfaces_daemon_unavailable_when_recovery_does_not_help() {
    let manifest = manifest_file();
    let client = DaemonClient::new(
        manifest.path(),
        ClientKind::Cli,
        Arc::new(AlwaysUnavailableTransport),
        Duration::from_millis(250),
    );
    let recoveries = Arc::new(AtomicUsize::new(0));
    let recoveries_for_ensure = Arc::clone(&recoveries);

    let error = client
        .call_with_ensure::<Value, _>(RpcMethod::RuntimeStatus, params(), || async move {
            recoveries_for_ensure.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
        .await
        .expect_err("replay still fails when the daemon stays unavailable");

    assert!(matches!(error, ClientError::DaemonUnavailable));
    assert_eq!(
        recoveries.load(Ordering::SeqCst),
        1,
        "ensure runs exactly once even though the replay also fails"
    );
}

#[tokio::test]
async fn call_with_ensure_surfaces_the_ensure_callback_error_without_replaying() {
    let manifest = manifest_file();
    let attempts = Arc::new(AtomicUsize::new(0));
    let client = DaemonClient::new(
        manifest.path(),
        ClientKind::Cli,
        Arc::new(RecoverOnceTransport {
            attempts: Arc::clone(&attempts),
            seen_params: Arc::new(Mutex::new(Vec::new())),
        }),
        Duration::from_millis(250),
    );

    let error = client
        .call_with_ensure::<Value, _>(RpcMethod::RuntimeStatus, params(), || async move {
            Err(ClientError::DaemonUnavailable)
        })
        .await
        .expect_err("a failing ensure callback must short-circuit before replay");

    assert!(matches!(error, ClientError::DaemonUnavailable));
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        1,
        "only the first attempt ran; ensure failure prevents the replay call"
    );
}

#[tokio::test]
async fn call_with_ensure_returns_the_first_attempt_without_running_ensure() {
    // The common production path: the daemon is already up, so the very first
    // `call` succeeds and the recovery callback must never run. Seeding
    // `attempts` at 1 makes `RecoverOnceTransport` succeed immediately
    // instead of failing once.
    let manifest = manifest_file();
    let attempts = Arc::new(AtomicUsize::new(1));
    let seen_params = Arc::new(Mutex::new(Vec::new()));
    let client = DaemonClient::new(
        manifest.path(),
        ClientKind::Cli,
        Arc::new(RecoverOnceTransport {
            attempts: Arc::clone(&attempts),
            seen_params: Arc::clone(&seen_params),
        }),
        Duration::from_millis(250),
    );
    let recoveries = Arc::new(AtomicUsize::new(0));
    let recoveries_for_ensure = Arc::clone(&recoveries);

    let result: Value = client
        .call_with_ensure(RpcMethod::RuntimeStatus, params(), || async move {
            recoveries_for_ensure.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
        .await
        .expect("a healthy daemon must satisfy the first attempt");

    assert_eq!(result, json!({"ok": true}));
    assert_eq!(
        recoveries.load(Ordering::SeqCst),
        0,
        "ensure must not run when the first attempt already succeeded"
    );
    assert_eq!(
        seen_params.lock().expect("seen params lock").len(),
        1,
        "exactly one round trip; no replay"
    );
}
