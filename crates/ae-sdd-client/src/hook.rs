use std::future::Future;

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
            Err(error) if is_recoverable(&error) => {
                self.offline_outcome(
                    invocation.method,
                    session_id.as_deref(),
                    invocation.offline_capability.as_deref(),
                    invocation.now_unix_ms,
                    &error,
                )
                .await
            }
            Err(error) => Err(error),
        }
    }

    /// Invokes a Hook, recovering a missing daemon once before applying offline policy.
    ///
    /// The callback runs only after an endpoint-manifest or daemon-unavailable
    /// failure. A successful recovery replays the same method and request
    /// parameters, including its idempotency key, at most once.
    pub async fn invoke_with_recovery<F, Fut>(
        &self,
        invocation: HookInvocation,
        recover: F,
    ) -> ClientResult<HookOutcome>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ClientResult<()>>,
    {
        if !is_hook(invocation.method) {
            return Err(ClientError::Protocol);
        }
        let session_id = invocation.params.session_id.clone();
        let first_attempt_params = duplicate_params(&invocation.params);
        match self
            .daemon
            .call::<HookOutcome>(invocation.method, first_attempt_params)
            .await
        {
            Ok(mut outcome) => {
                outcome.offline = false;
                Ok(outcome)
            }
            Err(error) if is_recoverable(&error) => {
                if recover().await.is_ok() {
                    match self
                        .daemon
                        .call::<HookOutcome>(invocation.method, invocation.params)
                        .await
                    {
                        Ok(mut outcome) => {
                            outcome.offline = false;
                            return Ok(outcome);
                        }
                        Err(replay_error) if is_recoverable(&replay_error) => {
                            return self
                                .offline_outcome(
                                    invocation.method,
                                    session_id.as_deref(),
                                    invocation.offline_capability.as_deref(),
                                    invocation.now_unix_ms,
                                    &replay_error,
                                )
                                .await;
                        }
                        Err(replay_error) => return Err(replay_error),
                    }
                }
                self.offline_outcome(
                    invocation.method,
                    session_id.as_deref(),
                    invocation.offline_capability.as_deref(),
                    invocation.now_unix_ms,
                    &error,
                )
                .await
            }
            Err(error) => Err(error),
        }
    }

    async fn offline_outcome(
        &self,
        method: RpcMethod,
        session_id: Option<&str>,
        offline_capability: Option<&str>,
        now_unix_ms: u64,
        error: &ClientError,
    ) -> ClientResult<HookOutcome> {
        let manifest = match self.daemon.endpoint_manifest().await {
            Ok(manifest) => manifest,
            Err(_) => {
                return Ok(fail_closed(
                    method,
                    StableErrorCode::DaemonUnavailable,
                    false,
                ));
            }
        };
        let verifier = match OfflineCapabilityVerifier::from_manifest(
            manifest.boot_id,
            manifest.capability_key_id,
            &manifest.capability_public_key,
        ) {
            Ok(verifier) => verifier,
            Err(_) => {
                return Ok(fail_closed(method, StableErrorCode::SessionExpired, false));
            }
        };
        match verifier.verify(
            offline_capability.unwrap_or(""),
            session_id.unwrap_or(""),
            now_unix_ms,
        ) {
            Ok(claims) if claims.engaged => Ok(fail_closed(method, error.stable_code(), true)),
            Ok(_) => Ok(HookOutcome {
                engaged: false,
                decision: HookDecision::Allow,
                context: None,
                event_seq: 0,
                offline: true,
                error_code: Some(error.stable_code()),
            }),
            Err(_) => Ok(fail_closed(method, StableErrorCode::SessionExpired, false)),
        }
    }
}

fn duplicate_params(params: &RequestParams<Value>) -> RequestParams<Value> {
    RequestParams {
        protocol_version: params.protocol_version.clone(),
        workspace_id: params.workspace_id.clone(),
        agent_id: params.agent_id.clone(),
        session_id: params.session_id.clone(),
        capability_token: params.capability_token.clone(),
        turn_id: params.turn_id.clone(),
        work_item_id: params.work_item_id.clone(),
        lease_id: params.lease_id.clone(),
        fencing_token: params.fencing_token,
        expected_revision: params.expected_revision,
        idempotency_key: params.idempotency_key.clone(),
        confirmation: params.confirmation.clone(),
        deadline_ms: params.deadline_ms,
        payload: params.payload.clone(),
    }
}

fn is_recoverable(error: &ClientError) -> bool {
    matches!(
        error,
        ClientError::DaemonUnavailable | ClientError::EndpointManifest
    )
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
