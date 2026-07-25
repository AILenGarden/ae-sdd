//! Handshake drift: the client must refuse a daemon that does not match the
//! manifest it was authorized against.
//!
//! `DaemonClient::call` guards this with a single ten-clause condition, which
//! coverage scores as one region — entering it via any one clause marks the
//! whole thing covered. So these tests assert each clause separately: dropping
//! the `boot_id` or `capability_public_key` check would let a client complete a
//! handshake with a *different* daemon than the manifest authorizes, and no
//! coverage number would move.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use ae_sdd_client::{ClientError, ClientTransport, DaemonClient};
use ae_sdd_protocol::{
    ClientKind, EndpointManifest, PROTOCOL_RANGE_V1, PROTOCOL_VERSION_V1, RequestParams, RpcMethod,
    SecretString,
};
use serde_json::{Value, json};
use tempfile::NamedTempFile;

const BOOT_ID: &str = "00000000-0000-0000-0000-000000000001";
const EVENT_STORE_ID: &str = "00000000-0000-0000-0000-000000000002";

fn manifest_file() -> NamedTempFile {
    let file = NamedTempFile::new().expect("manifest temp file");
    let manifest = EndpointManifest {
        schema_version: "ae-sdd-endpoint/v1".to_owned(),
        pid: 1,
        boot_id: BOOT_ID.to_owned(),
        event_store_id: EVENT_STORE_ID.to_owned(),
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

/// The handshake result a well-behaved daemon matching the manifest returns.
fn agreeing_handshake_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION_V1,
        "bootId": BOOT_ID,
        "eventStoreId": EVENT_STORE_ID,
        "daemonBuild": "ae-sdd-daemon/test",
        "capabilities": ["bounded-job-scheduler-v1"],
        "policyDigest": "a".repeat(64),
        "operationSchemaDigest": "b".repeat(64),
        "limits": {
            "maxFrameBytes": 1_048_576,
            "maxAgentDepth": 2,
            "maxStringBytes": 65_536,
            "maxCollectionItems": 128,
            "maxDeadlineMs": 30_000,
            "hookDeadlineMs": 250,
            "maxChildResultBytes": 65_536,
            "maxChildSummaryBytes": 4_096,
            "maxContextProjectionBytes": 65_536
        },
        "capabilityKeyId": "d".repeat(64),
        "capabilityPublicKey": "e".repeat(64)
    })
}

/// Replies with a caller-supplied handshake result, so each test can perturb
/// exactly one negotiated field.
struct DriftTransport {
    handshake_result: Value,
    /// When false, only one frame comes back instead of the required two.
    two_responses: bool,
}

impl ClientTransport for DriftTransport {
    fn exchange<'a>(
        &'a self,
        _endpoint: &'a str,
        payloads: &'a [Vec<u8>],
        _timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Vec<u8>>, ClientError>> + Send + 'a>> {
        Box::pin(async move {
            let handshake_id = serde_json::from_slice::<Value>(&payloads[0])
                .map_err(|_| ClientError::Protocol)?["id"]
                .clone();
            let request_id = serde_json::from_slice::<Value>(&payloads[1])
                .map_err(|_| ClientError::Protocol)?["id"]
                .clone();
            let handshake = json!({
                "jsonrpc": "2.0",
                "id": handshake_id,
                "result": self.handshake_result.clone()
            });
            let handshake_bytes =
                serde_json::to_vec(&handshake).map_err(|_| ClientError::Protocol)?;
            if !self.two_responses {
                return Ok(vec![handshake_bytes]);
            }
            let response = json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": {"ok": true}
            });
            Ok(vec![
                handshake_bytes,
                serde_json::to_vec(&response).map_err(|_| ClientError::Protocol)?,
            ])
        })
    }
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

fn client_with(
    manifest: &NamedTempFile,
    handshake_result: Value,
    two_responses: bool,
) -> DaemonClient {
    DaemonClient::new(
        manifest.path(),
        ClientKind::Cli,
        Arc::new(DriftTransport {
            handshake_result,
            two_responses,
        }),
        Duration::from_millis(250),
    )
}

#[tokio::test]
async fn an_agreeing_daemon_completes_the_call() {
    // Baseline: proves every rejection below is caused by the single field the
    // test perturbs, not by an unrelated defect in the fixture.
    let manifest = manifest_file();
    let client = client_with(&manifest, agreeing_handshake_result(), true);

    let result: Value = client
        .call(RpcMethod::JobSubmit, params())
        .await
        .expect("a matching daemon must be accepted");
    assert_eq!(result, json!({"ok": true}));
}

#[tokio::test]
async fn every_handshake_agreement_clause_is_independently_required() {
    let manifest = manifest_file();
    /// One labelled perturbation of the negotiated handshake result.
    type DriftCase = (&'static str, Box<dyn Fn(&mut Value)>);

    let cases: Vec<DriftCase> = vec![
        (
            "protocol version drift",
            Box::new(|r: &mut Value| r["protocolVersion"] = json!("99.0")),
        ),
        (
            "boot id drift: a different daemon generation answered",
            Box::new(|r: &mut Value| {
                r["bootId"] = json!("00000000-0000-0000-0000-0000000000ff");
            }),
        ),
        (
            "event store drift: a different durable epoch",
            Box::new(|r: &mut Value| {
                r["eventStoreId"] = json!("00000000-0000-0000-0000-0000000000ee");
            }),
        ),
        (
            "policy digest drift",
            Box::new(|r: &mut Value| r["policyDigest"] = json!("c".repeat(64))),
        ),
        (
            "capability key id drift",
            Box::new(|r: &mut Value| r["capabilityKeyId"] = json!("f".repeat(64))),
        ),
        (
            "capability public key drift: verification key substituted",
            Box::new(|r: &mut Value| r["capabilityPublicKey"] = json!("1".repeat(64))),
        ),
        (
            "empty daemon build",
            Box::new(|r: &mut Value| r["daemonBuild"] = json!("")),
        ),
        (
            "operation schema digest not lower-hex",
            Box::new(|r: &mut Value| r["operationSchemaDigest"] = json!("B".repeat(64))),
        ),
        (
            "unusable negotiated limits",
            Box::new(|r: &mut Value| r["limits"]["maxFrameBytes"] = json!(0)),
        ),
        (
            "required capability absent from the negotiated set",
            Box::new(|r: &mut Value| r["capabilities"] = json!([])),
        ),
    ];

    for (label, mutate) in cases {
        let mut handshake = agreeing_handshake_result();
        mutate(&mut handshake);
        let client = client_with(&manifest, handshake, true);
        let error = client
            .call::<Value>(RpcMethod::JobSubmit, params())
            .await
            .expect_err("drift must abort the call");
        assert!(
            matches!(error, ClientError::Protocol),
            "expected Protocol for {label}, got {error:?}"
        );
    }
}

#[tokio::test]
async fn a_missing_second_frame_is_a_protocol_violation() {
    // The transport must return exactly the handshake plus the call response;
    // one frame means the peer is not speaking the negotiated shape.
    let manifest = manifest_file();
    let client = client_with(&manifest, agreeing_handshake_result(), false);

    let error = client
        .call::<Value>(RpcMethod::JobSubmit, params())
        .await
        .expect_err("a single frame cannot satisfy a call");
    assert!(
        matches!(error, ClientError::Protocol),
        "expected Protocol, got {error:?}"
    );
}

#[tokio::test]
async fn a_method_without_a_required_capability_does_not_demand_one() {
    // `RuntimeStatus` has no capability requirement, so an empty negotiated set
    // must still be accepted — proving the capability clause is conditional.
    let manifest = manifest_file();
    let mut handshake = agreeing_handshake_result();
    handshake["capabilities"] = json!([]);
    let client = client_with(&manifest, handshake, true);

    let result: Value = client
        .call(RpcMethod::RuntimeStatus, params())
        .await
        .expect("a capability-free method needs no negotiated capability");
    assert_eq!(result, json!({"ok": true}));
}

#[test]
fn manifest_path_reports_the_snapshot_each_reconnect_reads() {
    let manifest = manifest_file();
    let client = client_with(&manifest, agreeing_handshake_result(), true);

    assert_eq!(client.manifest_path(), manifest.path());
}
