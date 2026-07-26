use super::*;
use ae_sdd_policy::ExecutionHookToolClass;

use super::execution_supervisor::{ExecutionHookDisposition, ExecutionHookGuardOutcome};
use crate::config::execution_cache::{SourceReadKey, SourceReadVisibility};
use crate::config::execution_resources::{CargoAcquireRequest, ResourceDecision, ResourceKind};
use crate::{ExecutionHookDirective, ExecutionHookDirectiveDecision, ExecutionHookEvent};

impl RuntimeService {
    pub(super) fn hook(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
    ) -> RuntimeResult<Value> {
        let started = Instant::now();
        let identity = self.session_identity(params, true)?;
        let turn_id = require(&params.turn_id, "turnId")?.to_owned();
        let work_item_id = require(&params.work_item_id, "workItemId")?.to_owned();
        let payload: HookPayload = decode_value(params.payload.clone())?;
        if serde_json::to_vec(&payload).map_err(canonical_error)?.len() > 65_536 {
            return Err(schema_error(
                "Hook payload exceeds the bounded event budget",
            ));
        }
        let execution_event = execution_supervisor::decode_execution_event(&payload)?;
        self.validate_turn(&identity.session_id, &turn_id, payload.turn_seq)?;
        let scope = format!(
            "hook\0{}\0{}\0{}\0{}",
            identity.workspace_id, identity.session_id, turn_id, payload.hook_event_id
        );
        let digest = canonical_digest(&(method.as_str(), &payload))?;
        if let Some((mut value, event_seq)) =
            self.replay_receipt(&scope, &payload.hook_event_id, &digest)?
        {
            if let Some(object) = value.as_object_mut() {
                object.insert("replayed".to_owned(), Value::Bool(true));
                object.insert("eventSeq".to_owned(), Value::from(event_seq));
            }
            return Ok(value);
        }

        let context = self.context.hook_projection(&identity.session_id)?;
        let inventory_generation = self
            .lock_state()?
            .workspaces
            .get(&identity.workspace_id)
            .ok_or_else(|| project_mismatch("workspace is not registered"))?
            .result
            .inventory_generation;
        let mut decision = hook_decision(
            method,
            identity.engaged,
            context.as_ref(),
            &self.config.policy_digest,
            inventory_generation,
        );
        let execution = self.execution_hook_guard(
            &identity.session_id,
            &work_item_id,
            method,
            execution_event.as_ref(),
        )?;
        let mut execution_directive = execution.directive().cloned();
        let cargo_deferred = self.apply_execution_resources(
            &identity,
            method,
            &payload,
            &execution,
            &mut execution_directive,
        )?;
        if method == RpcMethod::HookPreTool
            && identity.engaged
            && (cargo_deferred
                || matches!(
                    execution.disposition(),
                    ExecutionHookDisposition::RequireProgress | ExecutionHookDisposition::Deny
                ))
        {
            decision = HookDecision::Deny;
        }
        let base = HookResult {
            engaged: identity.engaged,
            decision,
            context: (decision == HookDecision::Context)
                .then_some(context)
                .flatten(),
            event_seq: 0,
            replayed: false,
            execution_directive,
        };
        let value = to_value(base)?;
        let (mut value, event_seq) = self.actors.execute(
            &identity.workspace_id,
            &work_item_id,
            params.deadline_ms,
            || {
                let committed = self.commit_receipt_event(
                    &scope,
                    &payload.hook_event_id,
                    digest,
                    value,
                    &format!("hook.{}", method.as_str().replace('.', "_")),
                    Some(identity.workspace_id.clone()),
                    Some(identity.session_id.clone()),
                    Some(work_item_id.clone()),
                )?;
                self.record_execution_hook_event(
                    &identity,
                    &work_item_id,
                    &execution,
                    execution_event.as_ref(),
                )?;
                Ok(committed)
            },
        )?;
        if let Some(object) = value.as_object_mut() {
            object.insert("eventSeq".to_owned(), Value::from(event_seq));
        }
        if started.elapsed().as_millis() > u128::from(params.deadline_ms) {
            return Err(RuntimeError::new(
                StableErrorCode::GateTimeout,
                "Hook fast path exceeded the caller deadline",
            ));
        }
        Ok(value)
    }

    /// Applies the bounded source-read cache and the daemon-wide Cargo lease
    /// to one classified execution event.
    ///
    /// A PreTool source read that hits the cache carries `cachedReadRef`; a
    /// PostTool source read stores the bounded excerpt carried by the Hook
    /// payload.  A PreTool Cargo-bearing event (focused or broad
    /// verification) acquires the daemon-wide lease; the matching PostTool
    /// releases it.  Only sessions bound by a successful `execution.resume`
    /// are arbitrated — unbound shadow sessions stay unblocked during the
    /// rollout shadow stage.  The fast path stays bounded: one in-memory LRU
    /// lookup, one bounded lock-file attempt, no project file reads and no
    /// Gate evaluation.  Returns true when the event deferred on the lease.
    fn apply_execution_resources(
        &self,
        identity: &TrustedSession,
        method: RpcMethod,
        payload: &HookPayload,
        execution: &ExecutionHookGuardOutcome,
        directive: &mut Option<ExecutionHookDirective>,
    ) -> RuntimeResult<bool> {
        if !matches!(method, RpcMethod::HookPreTool | RpcMethod::HookPostTool) {
            return Ok(false);
        }
        let Some(wire) = payload.host_payload.get("executionEvent") else {
            return Ok(false);
        };
        // `decode_execution_event` already ran fail-closed on this payload, so
        // a decode failure here cannot occur for a classified event.
        let Ok(wire) = serde_json::from_value::<ExecutionHookEvent>(wire.clone()) else {
            return Ok(false);
        };
        let Some(class) = ExecutionHookToolClass::from_wire_name(wire.class.as_str()) else {
            return Ok(false);
        };
        // A directive exists exactly when the session is bound and the event
        // was classified; unbound shadow sessions are never arbitrated.
        if directive.is_none() {
            return Ok(false);
        }
        match (method, class) {
            (RpcMethod::HookPreTool, ExecutionHookToolClass::SourceRead) => {
                if execution.disposition() != ExecutionHookDisposition::Allow {
                    return Ok(false);
                }
                let Some(key) = source_read_key(identity, &wire) else {
                    return Ok(false);
                };
                let hit = self
                    .config
                    .execution_resources()
                    .source_reads()
                    .get(&source_read_visibility(identity), &key);
                if let (Some(directive), Some(reference)) = (directive.as_mut(), hit) {
                    directive.cached_read_ref = Some(reference.into_string());
                }
                Ok(false)
            }
            (RpcMethod::HookPostTool, ExecutionHookToolClass::SourceRead) => {
                let Some(key) = source_read_key(identity, &wire) else {
                    return Ok(false);
                };
                let Some(body) = payload
                    .host_payload
                    .get("toolOutput")
                    .and_then(Value::as_str)
                else {
                    return Ok(false);
                };
                self.config.execution_resources().source_reads().put(
                    &source_read_visibility(identity),
                    &key,
                    body,
                    self.config.source_read_cache_capacity,
                );
                Ok(false)
            }
            (
                RpcMethod::HookPreTool,
                ExecutionHookToolClass::FocusedTest | ExecutionHookToolClass::BroadTest,
            ) => {
                if execution.disposition() != ExecutionHookDisposition::Allow {
                    return Ok(false);
                }
                let request = CargoAcquireRequest {
                    session_id: identity.session_id.as_str(),
                    lock_path: self.config.cargo_lock_path.as_deref(),
                    now_unix_ms: self.clock.now_unix_ms(),
                    ttl_ms: self.config.cargo_lock_ttl_ms,
                    retry_after_ms: self.config.cargo_lock_retry_after_ms,
                    queue_capacity: self.config.cargo_lock_queue_capacity,
                };
                match self
                    .config
                    .execution_resources()
                    .cargo()
                    .acquire(ResourceKind::Cargo, &request)
                {
                    ResourceDecision::Allow => Ok(false),
                    ResourceDecision::Defer { retry_after_ms } => {
                        *directive = Some(ExecutionHookDirective {
                            decision: ExecutionHookDirectiveDecision::RequireProgress,
                            reason_code: Some(
                                StableErrorCode::ExecutionResourceBusy.as_str().to_owned(),
                            ),
                            output_budget_bytes: None,
                            retry_after_ms: Some(retry_after_ms),
                            cached_read_ref: None,
                        });
                        Ok(true)
                    }
                }
            }
            (
                RpcMethod::HookPostTool,
                ExecutionHookToolClass::FocusedTest | ExecutionHookToolClass::BroadTest,
            ) => {
                self.config
                    .execution_resources()
                    .cargo()
                    .release(ResourceKind::Cargo, identity.session_id.as_str());
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    pub(super) fn context_get(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let _: EmptyPayload = decode_value(params.payload.clone())?;
        let identity = self.session_identity(params, false)?;
        self.assert_context_work_item(&identity.session_id, params.work_item_id.as_deref())?;
        let result = self.context.project(&identity.session_id, 0, "")?;
        to_value(result)
    }

    pub(super) fn context_project(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        self.assert_context_work_item(&identity.session_id, params.work_item_id.as_deref())?;
        let request: ContextProjectPayload = decode_value(params.payload.clone())?;
        let result = self.context.project(
            &identity.session_id,
            request.known_revision,
            &request.known_digest,
        )?;
        if request.known_revision == result.context_revision
            && !request.known_digest.is_empty()
            && request.known_digest == result.digest
        {
            self.complete_compact_after_rehydrate(&identity.session_id, &result.digest)?;
        }
        to_value(result)
    }

    fn assert_context_work_item(
        &self,
        session_id: &str,
        requested_work_item: Option<&str>,
    ) -> RuntimeResult<()> {
        let requested = requested_work_item
            .filter(|value| !value.is_empty())
            .ok_or_else(|| schema_error("workItemId is required for context projection"))?;
        let state = self.lock_state()?;
        let session = state.sessions.get(session_id).ok_or_else(session_expired)?;
        if session.current_work_item.as_deref() == Some(requested) {
            Ok(())
        } else {
            Err(turn_mismatch(
                "context projection Work Item differs from the session binding",
            ))
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyPayload {}

/// Session visibility scope for the source-read cache: one authenticated
/// session inside one workspace.
fn source_read_visibility(identity: &TrustedSession) -> SourceReadVisibility<'_> {
    SourceReadVisibility::new(identity.workspace_id.as_str(), identity.session_id.as_str())
}

/// Cache key for one source-read event; absent path or content digest means
/// the read cannot be keyed and skips the cache.
fn source_read_key(identity: &TrustedSession, wire: &ExecutionHookEvent) -> Option<SourceReadKey> {
    let path = wire.path.as_deref()?;
    let digest = wire.content_digest.as_deref()?;
    Some(SourceReadKey::new(
        identity.workspace_id.as_str(),
        path,
        digest,
        wire.start_line,
        wire.end_line,
    ))
}
