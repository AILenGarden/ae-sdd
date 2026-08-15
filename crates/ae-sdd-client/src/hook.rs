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
    /// Turn the daemon allocated for this event; absent for offline outcomes.
    /// Optional so an older daemon response without the field still parses,
    /// and skipped when unset so the wire shape is unchanged for consumers
    /// that print the outcome verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    /// Monotonic sequence of `turnId`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_seq: Option<u64>,
    /// Work Item the daemon bound to the session. The daemon owns this
    /// binding (e.g. after a bootstrap `workitem.create`); the client must
    /// carry it through instead of dropping it, or later hooks lose the
    /// identity the daemon already settled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_item_id: Option<String>,
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
                turn_id: None,
                turn_seq: None,
                work_item_id: None,
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
        turn_id: None,
        turn_seq: None,
        work_item_id: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The fail-closed decision per hook is a safety contract: if `PreTool`
    /// stopped denying, an unreachable daemon would silently let tool calls
    /// through. Each mapping is pinned explicitly.
    #[test]
    fn fail_closed_decisions_are_pinned_per_hook_method() {
        let cases = [
            (RpcMethod::HookPreTool, HookDecision::Deny),
            (RpcMethod::HookStop, HookDecision::Block),
            (RpcMethod::HookUserPrompt, HookDecision::Context),
            // PostTool runs after the effect already happened, so denying it
            // would be theatre; it allows but still reports offline.
            (RpcMethod::HookPostTool, HookDecision::Allow),
        ];

        for (method, expected) in cases {
            let outcome = fail_closed(method, StableErrorCode::DaemonUnavailable, true);
            assert_eq!(
                outcome.decision, expected,
                "unexpected fail-closed decision for {method:?}"
            );
            assert!(outcome.offline, "a fail-closed outcome is always offline");
            assert_eq!(
                outcome.error_code,
                Some(StableErrorCode::DaemonUnavailable),
                "the originating code must stay visible to the caller"
            );
            assert_eq!(outcome.event_seq, 0);
            assert!(outcome.context.is_none());
        }

        // A non-hook method must never be treated as permissive.
        assert_eq!(
            fail_closed(
                RpcMethod::RuntimeStatus,
                StableErrorCode::DaemonUnavailable,
                false
            )
            .decision,
            HookDecision::Deny
        );
    }

    #[test]
    fn fail_closed_propagates_the_engaged_flag_verbatim() {
        for engaged in [true, false] {
            assert_eq!(
                fail_closed(
                    RpcMethod::HookPreTool,
                    StableErrorCode::SessionExpired,
                    engaged
                )
                .engaged,
                engaged
            );
        }
    }

    #[test]
    fn only_the_four_hook_methods_are_hooks() {
        for method in [
            RpcMethod::HookUserPrompt,
            RpcMethod::HookPreTool,
            RpcMethod::HookPostTool,
            RpcMethod::HookStop,
        ] {
            assert!(is_hook(method), "{method:?} must be a hook");
        }
        for method in [
            RpcMethod::RuntimeStatus,
            RpcMethod::RuntimeHandshake,
            RpcMethod::JobSubmit,
            RpcMethod::EventsSubscribe,
        ] {
            assert!(!is_hook(method), "{method:?} must not be a hook");
        }
    }

    #[test]
    fn only_local_reachability_failures_are_recoverable() {
        assert!(is_recoverable(&ClientError::DaemonUnavailable));
        assert!(is_recoverable(&ClientError::EndpointManifest));
        // A remote rejection or protocol breach is deterministic; retrying it
        // after starting a daemon would not change the answer.
        assert!(!is_recoverable(&ClientError::Protocol));
        assert!(!is_recoverable(&ClientError::OfflineCapabilityInvalid));
        assert!(!is_recoverable(&ClientError::Remote {
            code: StableErrorCode::SessionExpired,
            message: "redacted".to_owned(),
            remediation: None,
        }));
    }

    /// The daemon allocates the turn and binds the Work Item server-side and
    /// returns both on the hook response. A client type that lacks those
    /// fields lets serde silently discard them, forcing every later hook to
    /// re-derive identity the daemon already settled.
    #[test]
    fn hook_outcome_round_trips_the_daemon_allocated_identity() {
        let daemon_json = serde_json::json!({
            "engaged": true,
            "decision": "allow",
            "eventSeq": 41,
            "offline": false,
            "turnId": "turn-7",
            "turnSeq": 7,
            "workItemId": "WI-20260728-bootstrap",
        });

        let outcome: HookOutcome =
            serde_json::from_value(daemon_json).expect("daemon identity fields must parse");
        assert_eq!(outcome.turn_id.as_deref(), Some("turn-7"));
        assert_eq!(outcome.turn_seq, Some(7));
        assert_eq!(
            outcome.work_item_id.as_deref(),
            Some("WI-20260728-bootstrap")
        );

        let serialized = serde_json::to_value(&outcome).expect("outcome serializes");
        assert_eq!(serialized["turnId"], "turn-7");
        assert_eq!(serialized["turnSeq"], 7);
        assert_eq!(serialized["workItemId"], "WI-20260728-bootstrap");
    }

    /// Fields the daemon did not set must stay absent on the wire rather than
    /// serialize as explicit nulls: the typed CLI prints the outcome verbatim,
    /// and a changed response shape would break every consumer that pins it.
    #[test]
    fn hook_outcome_omits_unset_identity_fields_on_the_wire() {
        let outcome = fail_closed(
            RpcMethod::HookPreTool,
            StableErrorCode::DaemonUnavailable,
            true,
        );

        let serialized = serde_json::to_value(&outcome).expect("outcome serializes");
        for key in ["turnId", "turnSeq", "workItemId"] {
            assert!(
                serialized.get(key).is_none(),
                "{key} must stay absent when the daemon set no value"
            );
        }

        let parsed: HookOutcome =
            serde_json::from_value(serialized).expect("a response without identity re-parses");
        assert_eq!(parsed, outcome);
    }

    #[test]
    fn duplicate_params_copies_every_field_including_the_idempotency_key() {
        // Recovery replays the *same* request identity; a dropped field here
        // would turn one caller intent into a second distinct daemon request.
        let original = RequestParams {
            protocol_version: "1".to_owned(),
            workspace_id: Some("ws".to_owned()),
            agent_id: Some("agent".to_owned()),
            session_id: Some("session".to_owned()),
            capability_token: Some("token".to_owned()),
            turn_id: Some("turn".to_owned()),
            work_item_id: Some("work".to_owned()),
            lease_id: Some("lease".to_owned()),
            fencing_token: Some(9),
            expected_revision: Some(11),
            idempotency_key: Some("idem-1".to_owned()),
            confirmation: Some(ae_sdd_protocol::ConfirmationRef {
                confirmation_id: "confirm-1".to_owned(),
                approved_by: "operator".to_owned(),
                approved_at: "2024-01-01T00:00:00Z".to_owned(),
            }),
            deadline_ms: 250,
            payload: serde_json::json!({"k": "v"}),
        };

        let copy = duplicate_params(&original);

        assert_eq!(copy.protocol_version, original.protocol_version);
        assert_eq!(copy.workspace_id, original.workspace_id);
        assert_eq!(copy.agent_id, original.agent_id);
        assert_eq!(copy.session_id, original.session_id);
        assert_eq!(copy.capability_token, original.capability_token);
        assert_eq!(copy.turn_id, original.turn_id);
        assert_eq!(copy.work_item_id, original.work_item_id);
        assert_eq!(copy.lease_id, original.lease_id);
        assert_eq!(copy.fencing_token, original.fencing_token);
        assert_eq!(copy.expected_revision, original.expected_revision);
        assert_eq!(copy.idempotency_key, original.idempotency_key);
        assert_eq!(copy.confirmation, original.confirmation);
        assert_eq!(copy.deadline_ms, original.deadline_ms);
        assert_eq!(copy.payload, original.payload);
    }
}
