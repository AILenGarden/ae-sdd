//! Offline Hook policy: what a Hook decides when the daemon is unreachable.
//!
//! The existing `handshake_and_hook_failure.rs` harness publishes a manifest
//! whose `capabilityPublicKey` is filler (`"e" * 64`), so the offline verifier
//! never constructs and every case there lands on the same `SessionExpired`
//! fail-closed branch. These tests publish a *real* Ed25519 key and mint
//! genuinely signed tokens, which is the only way to exercise the engaged /
//! unengaged / invalid outcomes that decide whether a missing daemon blocks the
//! user or silently lets the action through.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use ae_sdd_client::{ClientError, ClientTransport, DaemonClient, HookClient, HookInvocation};
use ae_sdd_protocol::{
    CapabilityTokenWire, ClientKind, EndpointManifest, HookDecision, PROTOCOL_RANGE_V1,
    PROTOCOL_VERSION_V1, RequestParams, RpcMethod, SecretString, StableErrorCode,
};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

const BOOT_ID: &str = "00000000-0000-0000-0000-000000000001";
const SESSION_ID: &str = "00000000-0000-0000-0000-0000000000bb";

fn signing_key() -> SigningKey {
    SigningKey::from_bytes(&[11_u8; 32])
}

fn key_id() -> String {
    hex::encode(Sha256::digest(signing_key().verifying_key().to_bytes()))
}

/// Publishes a manifest carrying the real verification key, so the offline
/// verifier constructs and token claims actually get checked.
fn manifest_with_real_key() -> NamedTempFile {
    let file = NamedTempFile::new().expect("manifest temp file");
    let manifest = EndpointManifest {
        schema_version: "ae-sdd-endpoint/v1".to_owned(),
        pid: 1,
        boot_id: BOOT_ID.to_owned(),
        event_store_id: "00000000-0000-0000-0000-000000000002".to_owned(),
        endpoint: "test-endpoint".to_owned(),
        endpoint_token: SecretString::new("endpoint-token"),
        protocol_range: PROTOCOL_RANGE_V1.to_owned(),
        daemon_version: "test".to_owned(),
        policy_digest: "a".repeat(64),
        capability_key_id: key_id(),
        capability_public_key: hex::encode(signing_key().verifying_key().to_bytes()),
        started_at: "2026-01-01T00:00:00Z".to_owned(),
    };
    std::fs::write(
        file.path(),
        serde_json::to_vec(&manifest).expect("manifest serializes"),
    )
    .expect("manifest writes");
    file
}

/// Mints a genuinely signed capability so each assertion is attributable to the
/// claim under test rather than to a bad signature.
fn signed_capability(capability_id: &str, session_id: &str) -> String {
    let key = signing_key();
    let unsigned = CapabilityTokenWire::new_v1(
        key_id(),
        BOOT_ID,
        capability_id,
        session_id,
        "root",
        None,
        "d".repeat(64),
        1_000,
        9_000,
        String::new(),
    );
    let canonical = unsigned.canonical_claims_bytes().expect("canonical claims");
    let signature = hex::encode(key.sign(&canonical).to_bytes());
    CapabilityTokenWire::new_v1(
        unsigned.key_id(),
        unsigned.boot_id(),
        unsigned.capability_id(),
        unsigned.session_id(),
        unsigned.role(),
        unsigned.delegation_id().map(ToOwned::to_owned),
        unsigned.grant_digest(),
        unsigned.issued_at_unix_ms(),
        unsigned.expires_at_unix_ms(),
        signature,
    )
    .encode_json()
    .expect("token encodes")
}

/// Always reports the daemon as unreachable, forcing the offline branch.
struct UnavailableTransport;

impl ClientTransport for UnavailableTransport {
    fn exchange<'a>(
        &'a self,
        _endpoint: &'a str,
        _payloads: &'a [Vec<u8>],
        _timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Vec<u8>>, ClientError>> + Send + 'a>> {
        Box::pin(async move { Err(ClientError::DaemonUnavailable) })
    }
}

fn params_for_session(session_id: Option<&str>) -> RequestParams<Value> {
    RequestParams {
        protocol_version: PROTOCOL_VERSION_V1.to_owned(),
        workspace_id: None,
        agent_id: None,
        session_id: session_id.map(ToOwned::to_owned),
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

/// `HookClient` borrows its daemon, so each test owns the `DaemonClient` and
/// wraps it at the point of use.
fn daemon_client(manifest_path: &std::path::Path) -> DaemonClient {
    DaemonClient::new(
        manifest_path,
        ClientKind::Hook,
        Arc::new(UnavailableTransport),
        Duration::from_millis(250),
    )
}

fn invocation(
    method: RpcMethod,
    offline_capability: Option<String>,
    now_unix_ms: u64,
) -> HookInvocation {
    HookInvocation {
        method,
        params: params_for_session(Some(SESSION_ID)),
        engaged: false,
        offline_capability,
        now_unix_ms,
    }
}

#[tokio::test]
async fn an_engaged_offline_capability_fails_closed_with_the_originating_cause() {
    let manifest = manifest_with_real_key();
    let daemon = daemon_client(manifest.path());
    let client = HookClient::new(&daemon);
    let token = signed_capability("hook.engaged", SESSION_ID);

    let outcome = client
        .invoke(invocation(RpcMethod::HookPreTool, Some(token), 5_000))
        .await
        .expect("offline policy yields an outcome, not an error");

    assert!(
        outcome.engaged,
        "a valid engaged capability means daemon control is in force"
    );
    assert_eq!(
        outcome.decision,
        HookDecision::Deny,
        "an engaged PreTool must deny while the daemon is unreachable"
    );
    assert!(outcome.offline);
    assert_eq!(
        outcome.error_code,
        Some(StableErrorCode::DaemonUnavailable),
        "the originating cause must survive into the offline outcome"
    );
}

#[tokio::test]
async fn an_unengaged_offline_capability_allows_instead_of_blocking_the_user() {
    // This is the branch that decides a missing daemon does NOT block someone
    // who was never under daemon control.
    let manifest = manifest_with_real_key();
    let daemon = daemon_client(manifest.path());
    let client = HookClient::new(&daemon);
    let token = signed_capability("hook.unengaged", SESSION_ID);

    let outcome = client
        .invoke(invocation(RpcMethod::HookPreTool, Some(token), 5_000))
        .await
        .expect("offline policy yields an outcome");

    assert!(!outcome.engaged);
    assert_eq!(
        outcome.decision,
        HookDecision::Allow,
        "an unengaged session must not be blocked by an absent daemon"
    );
    assert!(outcome.offline);
    assert_eq!(outcome.error_code, Some(StableErrorCode::DaemonUnavailable));
    assert_eq!(outcome.event_seq, 0);
}

#[tokio::test]
async fn engaged_fail_closed_decision_follows_the_hook_method() {
    let manifest = manifest_with_real_key();
    let daemon = daemon_client(manifest.path());
    let client = HookClient::new(&daemon);

    for (method, expected) in [
        (RpcMethod::HookPreTool, HookDecision::Deny),
        (RpcMethod::HookStop, HookDecision::Block),
        (RpcMethod::HookUserPrompt, HookDecision::Context),
        (RpcMethod::HookPostTool, HookDecision::Allow),
    ] {
        let outcome = client
            .invoke(invocation(
                method,
                Some(signed_capability("hook.engaged", SESSION_ID)),
                5_000,
            ))
            .await
            .expect("offline policy yields an outcome");
        assert_eq!(
            outcome.decision, expected,
            "unexpected offline decision for {method:?}"
        );
        assert!(outcome.engaged);
    }
}

#[tokio::test]
async fn an_absent_or_expired_or_foreign_capability_fails_closed_as_session_expired() {
    let manifest = manifest_with_real_key();
    let daemon = daemon_client(manifest.path());
    let client = HookClient::new(&daemon);

    // No capability at all.
    let missing = client
        .invoke(invocation(RpcMethod::HookPreTool, None, 5_000))
        .await
        .expect("outcome");
    assert_eq!(missing.error_code, Some(StableErrorCode::SessionExpired));
    assert!(
        !missing.engaged,
        "an unverifiable claim cannot assert engagement"
    );
    assert_eq!(missing.decision, HookDecision::Deny);

    // Past its expiry (token expires at 9_000).
    let expired = client
        .invoke(invocation(
            RpcMethod::HookPreTool,
            Some(signed_capability("hook.engaged", SESSION_ID)),
            9_000,
        ))
        .await
        .expect("outcome");
    assert_eq!(expired.error_code, Some(StableErrorCode::SessionExpired));
    assert!(!expired.engaged);

    // Bound to a different session than the one being invoked.
    let foreign = client
        .invoke(invocation(
            RpcMethod::HookPreTool,
            Some(signed_capability(
                "hook.engaged",
                "00000000-0000-0000-0000-0000000000ff",
            )),
            5_000,
        ))
        .await
        .expect("outcome");
    assert_eq!(foreign.error_code, Some(StableErrorCode::SessionExpired));
    assert!(!foreign.engaged);
}

#[tokio::test]
async fn an_unreadable_manifest_fails_closed_as_daemon_unavailable() {
    // Without a manifest the client cannot even reconstruct a verifier, so the
    // cause is unavailability rather than an expired session.
    let manifest = manifest_with_real_key();
    let path = manifest.path().to_path_buf();
    drop(manifest);

    let daemon = daemon_client(&path);
    let client = HookClient::new(&daemon);
    let outcome = client
        .invoke(invocation(
            RpcMethod::HookPreTool,
            Some(signed_capability("hook.engaged", SESSION_ID)),
            5_000,
        ))
        .await
        .expect("outcome");

    assert_eq!(outcome.error_code, Some(StableErrorCode::DaemonUnavailable));
    assert!(!outcome.engaged);
    assert_eq!(outcome.decision, HookDecision::Deny);
    assert!(outcome.offline);
}

#[tokio::test]
async fn a_manifest_key_that_is_not_a_valid_point_fails_closed_as_session_expired() {
    // Filler/corrupt key material must not be silently tolerated.
    let file = NamedTempFile::new().expect("manifest temp file");
    let manifest = EndpointManifest {
        schema_version: "ae-sdd-endpoint/v1".to_owned(),
        pid: 1,
        boot_id: BOOT_ID.to_owned(),
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
        serde_json::to_vec(&manifest).expect("serializes"),
    )
    .expect("writes");

    let daemon = daemon_client(file.path());
    let client = HookClient::new(&daemon);
    let outcome = client
        .invoke(invocation(
            RpcMethod::HookPreTool,
            Some(signed_capability("hook.engaged", SESSION_ID)),
            5_000,
        ))
        .await
        .expect("outcome");

    assert_eq!(outcome.error_code, Some(StableErrorCode::SessionExpired));
    assert!(!outcome.engaged);
}

#[tokio::test]
async fn a_non_hook_method_is_rejected_before_any_transport_or_offline_work() {
    let manifest = manifest_with_real_key();
    let daemon = daemon_client(manifest.path());
    let client = HookClient::new(&daemon);

    for method in [RpcMethod::RuntimeStatus, RpcMethod::JobSubmit] {
        let error = client
            .invoke(invocation(method, None, 5_000))
            .await
            .expect_err("a non-hook method is a protocol misuse");
        assert!(
            matches!(error, ClientError::Protocol),
            "expected Protocol for {method:?}, got {error:?}"
        );
    }
}
