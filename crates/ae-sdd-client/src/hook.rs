use ae_sdd_protocol::{HookDecision, RequestParams, RpcMethod, StableErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ClientError, ClientResult, DaemonClient, OfflineCapabilityVerifier};

/// Host Hook invocation passed to the thin client.
pub struct HookInvocation {
    /// Exact Hook method.
    pub method: RpcMethod,
    /// Post-handshake request context and host payload.
    pub params: RequestParams<Value>,
    /// Legacy host hint retained for wire compatibility; never trusted.
    pub engaged: bool,
    /// Last boot-signed session capability used only if the daemon is unreachable.
    pub offline_capability: Option<String>,
    /// Host observation time for offline expiry checks.
    pub now_unix_ms: u64,
}

/// Host-neutral Hook result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutcome {
    /// Whether fail-closed daemon control applies.
    pub engaged: bool,
    /// Host action.
    pub decision: HookDecision,
    /// Optional bounded context injection.
    #[serde(default)]
    pub context: Option<Value>,
    /// Durable event sequence for online decisions.
    #[serde(default)]
    pub event_seq: u64,
    /// True when produced without contacting the daemon.
    #[serde(default)]
    pub offline: bool,
    /// Stable cause for an offline fail-closed outcome.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<StableErrorCode>,
}

/// Thin Hook client with explicit online/offline policy.
pub struct HookClient<'a> {
    daemon: &'a DaemonClient,
}

impl<'a> HookClient<'a> {
    /// Wraps an authenticated daemon client.
    #[must_use]
    pub const fn new(daemon: &'a DaemonClient) -> Self {
        Self { daemon }
    }

    /// Invokes a Hook or applies the exact fail-closed/offline-capability contract.
    pub async fn invoke(&self, invocation: HookInvocation) -> ClientResult<HookOutcome> {
        if !is_hook(invocation.method) {
            return Err(ClientError::Protocol);
        }
        let session_id = invocation.params.session_id.clone();
        match self
            .daemon
            .call::<HookOutcome>(invocation.method, invocation.params)
            .await
        {
            Ok(mut outcome) => {
                outcome.offline = false;
                Ok(outcome)
            }
            Err(error)
                if matches!(
                    error,
                    ClientError::DaemonUnavailable | ClientError::EndpointManifest
                ) =>
            {
                let manifest = match self.daemon.endpoint_manifest().await {
                    Ok(manifest) => manifest,
                    Err(_) => {
                        return Ok(fail_closed(
                            invocation.method,
                            StableErrorCode::DaemonUnavailable,
                            false,
                        ));
                    }
                };
                let token = invocation.offline_capability.as_deref().unwrap_or("");
                let session_id = session_id.as_deref().unwrap_or("");
                let verifier = match OfflineCapabilityVerifier::from_manifest(
                    manifest.boot_id,
                    manifest.capability_key_id,
                    &manifest.capability_public_key,
                ) {
                    Ok(verifier) => verifier,
                    Err(_) => {
                        return Ok(fail_closed(
                            invocation.method,
                            StableErrorCode::SessionExpired,
                            false,
                        ));
                    }
                };
                match verifier.verify(token, session_id, invocation.now_unix_ms) {
                    Ok(claims) if claims.engaged => Ok(fail_closed(
                        invocation.method,
                        error.stable_code(),
                        true,
                    )),
                    Ok(_) => Ok(HookOutcome {
                        engaged: false,
                        decision: HookDecision::Allow,
                        context: None,
                        event_seq: 0,
                        offline: true,
                        error_code: Some(error.stable_code()),
                    }),
                    Err(_) => Ok(fail_closed(
                        invocation.method,
                        StableErrorCode::SessionExpired,
                        false,
                    )),
                }
            }
            Err(error) => Err(error),
        }
    }
}

fn fail_closed(method: RpcMethod, code: StableErrorCode, engaged: bool) -> HookOutcome {
    let decision = match method {
        RpcMethod::HookPreTool => HookDecision::Deny,
        RpcMethod::HookStop => HookDecision::Block,
        RpcMethod::HookUserPrompt => HookDecision::Context,
        RpcMethod::HookPostTool => HookDecision::Allow,
        _ => HookDecision::Deny,
    };
    HookOutcome {
        engaged,
        decision,
        context: None,
        event_seq: 0,
        offline: true,
        error_code: Some(code),
    }
}

fn is_hook(method: RpcMethod) -> bool {
    matches!(
        method,
        RpcMethod::HookUserPrompt
            | RpcMethod::HookPreTool
            | RpcMethod::HookPostTool
            | RpcMethod::HookStop
    )
}
