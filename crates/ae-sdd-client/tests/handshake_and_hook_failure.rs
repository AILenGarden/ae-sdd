use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
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
