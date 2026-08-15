use super::*;
use ae_sdd_contracts::diagnostics::{DiagnosticRecord, HookInRecord, HookOutRecord};
use ae_sdd_policy::ExecutionHookToolClass;

use super::execution_supervisor::{
    ClassifiedExecutionEvent, ExecutionHookDisposition, ExecutionHookGuardOutcome,
};
use crate::config::execution_cache::{SourceReadKey, SourceReadVisibility};
use crate::config::execution_resources::{
    CargoAcquireRequest, ResourceAcquisition, ResourceDecision, ResourceKind,
};
use crate::diagnostics;
use crate::{
    ExecutionHookDirective, ExecutionHookDirectiveDecision, ExecutionHookEvent,
    ExecutionResourceLeaseOutcomeV1, ExecutionResourceLeaseRequestV1, PreparedExecutionHookV1,
};

struct HookTurnRollback<'a> {
    service: &'a RuntimeService,
    session_id: &'a str,
    snapshot: (Option<String>, u64),
    cargo_acquired: bool,
    durable_cargo_acquired: bool,
    receipt_committed: bool,
}

impl Drop for HookTurnRollback<'_> {
    fn drop(&mut self) {
        if !self.receipt_committed {
            let _ = self
                .service
                .restore_hook_turn(self.session_id, &self.snapshot);
            if self.cargo_acquired {
                self.service
                    .config
                    .execution_resources()
                    .cargo()
                    .release(ResourceKind::Cargo, self.session_id);
            }
            if self.durable_cargo_acquired {
                let _ = self.service.persistence.release_execution_resource_lease(
                    "cargo",
                    &self.service.boot_id.to_string(),
                    self.session_id,
                );
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ExecutionResourceEffects {
    cargo_deferred: bool,
    cargo_acquired: bool,
    durable_cargo_acquired: bool,
}

impl RuntimeService {
    /// Answers one Hook invocation and records both halves of the exchange.
    ///
    /// The diagnostic pair is split between this wrapper and the traced body on
    /// purpose.  The body owns the typed success detail; this wrapper guarantees
    /// that a failed invocation still records an answer.  So a `hook.in` with no
    /// `hook.out` means the daemon never returned at all — a crash or a lost
    /// thread — which no other cheap signal exposes.
    pub(super) fn hook(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
    ) -> RuntimeResult<Value> {
        let started = Instant::now();
        let result = self.hook_traced(method, params, started);
        if let Err(error) = &result {
            diagnostics::emit(DiagnosticRecord::HookOut(HookOutRecord {
                ts: diagnostics::now_ms(),
                hid: hook_event_id_of(params).unwrap_or_else(|| "unknown".to_owned()),
                tid: params
                    .turn_id
                    .clone()
                    .unwrap_or_else(|| "unallocated".to_owned()),
                dec: "error".to_owned(),
                dir: None,
                rc: None,
                ctx: None,
                cdg: None,
                es: 0,
                rp: false,
                ok: false,
                err: Some(format!("{:?}", error.code())),
                ms: diagnostics::elapsed_ms(started),
            }));
        }
        result
    }

    fn hook_traced(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
        started: Instant,
    ) -> RuntimeResult<Value> {
        // The turn is no longer demanded from the caller: a Hook subprocess
        // cannot know its session's monotonic sequence, so the daemon allocates
        // it when absent and validates it when the host does supply one.
        let identity = self.session_identity(params, false)?;
        // §9.4 attach point 5: every Hook event refreshes the binding's
        // last_interaction_unix_ms, which is the heartbeat signal for liveness
        // and the 12h hard-timeout sweep. Root sessions have no delegation and
        // short-circuit inside refresh_interaction; the lookup is one lock
        // acquire when a delegation is present.
        if identity.role != WireAgentRole::Root {
            let delegation_id = self
                .lock_state()?
                .sessions
                .get(&identity.session_id)
                .and_then(|session| session.delegation_id.clone());
            self.delegation
                .bindings()
                .refresh_interaction(delegation_id.as_deref(), self.clock.now_unix_ms())?;
        }
        let payload: HookPayload = decode_value(params.payload.clone())?;
        if serde_json::to_vec(&payload).map_err(canonical_error)?.len() > 65_536 {
            return Err(schema_error(
                "Hook payload exceeds the bounded event budget",
            ));
        }
        let execution_event = execution_supervisor::decode_execution_event(&payload)?;
        // Hook idempotency includes turn allocation and every secondary side
        // effect, not only the final receipt commit. Serializing by session
        // makes a concurrent duplicate wait until the first receipt is visible
        // before it can allocate a turn or touch execution resources.
        let actor_partition = format!("-hook-session\0{}", identity.session_id);
        self.actors.execute(
            &identity.workspace_id,
            &actor_partition,
            params.deadline_ms,
            || {
                self.hook_serialized(
                    method,
                    params,
                    started,
                    &identity,
                    &payload,
                    execution_event.as_ref(),
                )
            },
        )
    }

    fn hook_serialized(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
        started: Instant,
        identity: &TrustedSession,
        payload: &HookPayload,
        execution_event: Option<&ClassifiedExecutionEvent>,
    ) -> RuntimeResult<Value> {
        ensure_hook_deadline(started, params.deadline_ms)?;
        let mut work_item_id =
            self.resolve_hook_work_item(&identity.session_id, params.work_item_id.as_deref())?;
        // A host retry has no durable turn cursor. Replay by the host event
        // identity before allocating a fresh prompt turn, otherwise the retry
        // itself advances the session and can never find the original receipt.
        let scope = format!("hook\0{}\0{}", identity.workspace_id, identity.session_id);
        let digest = canonical_digest(&(method.as_str(), payload, params.turn_id.as_deref()))?;
        if let Some(value) =
            self.reconcile_prepared_execution_hook(identity, &payload.hook_event_id, &digest)?
        {
            let turn_id = value
                .get("turnId")
                .and_then(Value::as_str)
                .ok_or_else(|| schema_error("replayed Hook receipt lacks turnId"))?;
            emit_hook_out(&payload.hook_event_id, turn_id, &value, started);
            return Ok(value);
        }
        if let Some((mut value, event_seq)) =
            self.replay_receipt(&scope, &payload.hook_event_id, &digest)?
        {
            if let Some(object) = value.as_object_mut() {
                object.insert("replayed".to_owned(), Value::Bool(true));
                object.insert("eventSeq".to_owned(), Value::from(event_seq));
            }
            let turn_id = value
                .get("turnId")
                .and_then(Value::as_str)
                .ok_or_else(|| schema_error("replayed Hook receipt lacks turnId"))?;
            let replay_work_item = value.get("workItemId").and_then(Value::as_str);
            if self.execution_hook_requires_record(&identity.session_id, replay_work_item)?
                && !self.execution_hook_receipt_exists(identity, &payload.hook_event_id, &digest)?
            {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "durable Hook receipt lacks its prepared execution bundle",
                ));
            }
            if value.get("contextKind").and_then(Value::as_str) == Some("full")
                && let Some(delivered) = value.get("contextDigest").and_then(Value::as_str)
            {
                self.context
                    .mark_hook_delivered(&identity.session_id, delivered)?;
            }
            emit_hook_out(&payload.hook_event_id, turn_id, &value, started);
            return Ok(value);
        }
        let turn_snapshot = self.hook_turn_snapshot(&identity.session_id)?;
        let mut turn_rollback = HookTurnRollback {
            service: self,
            session_id: &identity.session_id,
            snapshot: turn_snapshot,
            cargo_acquired: false,
            durable_cargo_acquired: false,
            receipt_committed: false,
        };
        let explicit_turn = match params.turn_id.as_deref() {
            Some(turn_id) => {
                let turn_seq = payload
                    .turn_seq
                    .ok_or_else(|| schema_error("turnSeq is required with an explicit turnId"))?;
                self.validate_turn(&identity.session_id, turn_id, turn_seq)?;
                Some((turn_id.to_owned(), turn_seq))
            }
            None => None,
        };
        let (turn_id, turn_seq) = match explicit_turn {
            Some(turn) => turn,
            None => self.allocate_turn(&identity.session_id, method)?,
        };
        if work_item_id.is_none() && is_ae_sdd_activation(method, payload) {
            work_item_id = Some(self.bootstrap_route_work_item(params, &identity.session_id)?);
        }
        // Recorded before any work happens, so an invocation the daemon never
        // finishes still leaves the evidence that it arrived.
        diagnostics::emit(DiagnosticRecord::HookIn(HookInRecord {
            ts: diagnostics::now_ms(),
            hid: payload.hook_event_id.clone(),
            wsid: identity.workspace_id.clone(),
            sid: identity.session_id.clone(),
            tid: turn_id.clone(),
            wid: work_item_id.clone(),
            m: method.as_str().to_owned(),
            cls: execution_event.map(|event| event.class().wire_name().to_owned()),
            seq: turn_seq,
        }));
        // A Hook with no bound Work Item still needs one serialization domain
        // per session. A Work Item ID must start alphanumeric, so this sentinel
        // can never collide with a business name.
        let projection = self.context.hook_projection(&identity.session_id)?;
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
            projection.as_ref().map(|projection| &projection.value),
            &self.config.policy_digest,
            inventory_generation,
        );
        let execution = self.execution_hook_guard(
            &identity.session_id,
            work_item_id.as_deref(),
            method,
            execution_event,
        )?;
        let mut execution_directive = execution.directive().cloned();
        // Deadline admission is intentionally checked exactly once at actor
        // entry. From turn allocation onward this Hook is a tiny transaction:
        // returning TIMEOUT after bootstrap or resource acquisition would leave
        // an unreplayable turn, Work Item binding, or Cargo lease. Once admitted,
        // finish the durable receipt so a client-side timeout can retry safely.
        let resource_effects = self.apply_execution_resources(
            identity,
            method,
            payload,
            &execution,
            &mut execution_directive,
        )?;
        turn_rollback.cargo_acquired = resource_effects.cargo_acquired;
        turn_rollback.durable_cargo_acquired = resource_effects.durable_cargo_acquired;
        if method == RpcMethod::HookPreTool
            && identity.engaged
            && (resource_effects.cargo_deferred
                || matches!(
                    execution.disposition(),
                    ExecutionHookDisposition::RequireProgress | ExecutionHookDisposition::Deny
                ))
        {
            decision = HookDecision::Deny;
        }
        // Token-minimal delivery: a projection already delivered to this
        // session — or acknowledged by the client cursor — answers with a
        // digest-only no-change; the body crosses the wire once per digest.
        let (context, context_kind, context_digest, delivered_digest) =
            match (decision == HookDecision::Context, projection.as_ref()) {
                (true, Some(projection)) => {
                    let client_known = payload.known_revision == Some(projection.context_revision)
                        && payload.known_digest.as_deref() == Some(projection.digest.as_str());
                    let deliver = projection.deliver && !client_known;
                    (
                        deliver.then(|| projection.value.clone()),
                        Some(if deliver { "full" } else { "no_change" }.to_owned()),
                        Some(projection.digest.clone()),
                        deliver.then(|| projection.digest.clone()),
                    )
                }
                _ => (None, None, None, None),
            };
        let base = HookResult {
            engaged: identity.engaged,
            decision,
            context,
            event_seq: 0,
            replayed: false,
            execution_directive,
            context_kind,
            context_digest,
            // The allocated turn travels back so the host can correlate its
            // event with the turn the daemon actually recorded.
            turn_id: Some(turn_id.clone()),
            turn_seq: Some(turn_seq),
            work_item_id: work_item_id.clone(),
        };
        let value = to_value(base)?;
        let prepared = PreparedExecutionHookV1 {
            schema_version: "prepared-execution-hook/v1".to_owned(),
            workspace_id: identity.workspace_id.clone(),
            session_id: identity.session_id.clone(),
            hook_event_id: payload.hook_event_id.clone(),
            request_digest: digest.clone(),
            hook_scope: scope.clone(),
            hook_kind: format!("hook.{}", method.as_str().replace('.', "_")),
            response_json: serde_json::to_string(&value).map_err(canonical_error)?,
            work_item_id: work_item_id.clone(),
            delivered_context_digest: delivered_digest.clone(),
            turn_id: turn_id.clone(),
            turn_seq,
            checkpoint_after_image: None,
            cargo_lease_required: method == RpcMethod::HookPreTool
                && execution.disposition() == ExecutionHookDisposition::Allow
                && execution_event.is_some_and(|event| {
                    matches!(
                        event.class(),
                        ExecutionHookToolClass::FocusedTest | ExecutionHookToolClass::BroadTest
                    )
                })
                && !resource_effects.cargo_deferred,
            completed: false,
        };
        let prepared = self.prepare_execution_hook_event(
            identity,
            work_item_id.as_deref(),
            &execution,
            execution_event,
            prepared,
        )?;
        let (mut value, event_seq) = self.commit_receipt_event(
            &scope,
            &payload.hook_event_id,
            digest.clone(),
            value,
            &format!("hook.{}", method.as_str().replace('.', "_")),
            Some(identity.workspace_id.clone()),
            Some(identity.session_id.clone()),
            // Absent rather than fabricated: an unbound Hook must not
            // attribute its ledger entry to an invented Work Item.
            work_item_id.clone(),
        )?;
        turn_rollback.receipt_committed = true;
        if let Some(prepared) = prepared {
            self.finalize_prepared_execution_hook(prepared)?;
        } else if let Some(delivered) = delivered_digest.as_deref() {
            // Unsupervised Hooks have no prepared execution barrier, so their
            // context acknowledgement follows the main receipt directly.
            self.context
                .mark_hook_delivered(&identity.session_id, delivered)?;
        }
        if let Some(object) = value.as_object_mut() {
            object.insert("eventSeq".to_owned(), Value::from(event_seq));
        }
        emit_hook_out(&payload.hook_event_id, &turn_id, &value, started);
        Ok(value)
    }

    fn reconcile_prepared_execution_hook(
        &self,
        identity: &TrustedSession,
        hook_event_id: &str,
        request_digest: &str,
    ) -> RuntimeResult<Option<Value>> {
        let Some(prepared) = self.load_prepared_execution_hook(&identity.session_id)? else {
            return Ok(None);
        };
        if prepared.completed {
            self.persistence
                .delete_record("prepared-execution-hook/v1", &prepared.session_id)?;
            return Ok(None);
        }
        let expected_hook_scope =
            format!("hook\0{}\0{}", identity.workspace_id, identity.session_id);
        if prepared.schema_version != "prepared-execution-hook/v1"
            || prepared.workspace_id != identity.workspace_id
            || prepared.session_id != identity.session_id
            || prepared.hook_scope != expected_hook_scope
            || prepared.hook_event_id.is_empty()
            || prepared.request_digest.len() != 64
            || !prepared
                .request_digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "prepared execution Hook identity is inconsistent",
            ));
        }
        let main_receipt = self.replay_receipt(
            &prepared.hook_scope,
            &prepared.hook_event_id,
            &prepared.request_digest,
        )?;
        if main_receipt.is_none()
            && (prepared.hook_event_id != hook_event_id
                || prepared.request_digest != request_digest)
        {
            return Err(RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "another execution Hook transition is still pending for this session",
            ));
        }
        let cargo_acquisition = self.ensure_prepared_cargo_lease(&prepared)?;
        let committed = match main_receipt {
            Some(receipt) => Ok(receipt),
            None => {
                let response: Value =
                    serde_json::from_str(&prepared.response_json).map_err(|_| {
                        RuntimeError::new(
                            StableErrorCode::ExternalStateConflict,
                            "prepared Hook response is malformed",
                        )
                    })?;
                self.commit_receipt_event(
                    &prepared.hook_scope,
                    &prepared.hook_event_id,
                    prepared.request_digest.clone(),
                    response,
                    &prepared.hook_kind,
                    Some(prepared.workspace_id.clone()),
                    Some(prepared.session_id.clone()),
                    prepared.work_item_id.clone(),
                )
            }
        };
        let (mut value, event_seq) = match committed {
            Ok(value) => value,
            Err(error) => {
                if let Some((cargo_acquired, durable_cargo_acquired)) = cargo_acquisition {
                    self.compensate_cargo_acquisition(
                        &prepared.session_id,
                        cargo_acquired,
                        durable_cargo_acquired,
                    );
                }
                return Err(error);
            }
        };
        if let Err(error) = self.finalize_prepared_execution_hook(prepared.clone()) {
            if let Some((cargo_acquired, durable_cargo_acquired)) = cargo_acquisition {
                self.compensate_cargo_acquisition(
                    &prepared.session_id,
                    cargo_acquired,
                    durable_cargo_acquired,
                );
            }
            return Err(error);
        }
        if prepared.hook_event_id != hook_event_id || prepared.request_digest != request_digest {
            return Ok(None);
        }
        if let Some(object) = value.as_object_mut() {
            object.insert("replayed".to_owned(), Value::Bool(true));
            object.insert("eventSeq".to_owned(), Value::from(event_seq));
        }
        Ok(Some(value))
    }

    fn ensure_prepared_cargo_lease(
        &self,
        prepared: &PreparedExecutionHookV1,
    ) -> RuntimeResult<Option<(bool, bool)>> {
        if !prepared.cargo_lease_required {
            return Ok(None);
        }
        let (decision, cargo_acquired, durable_cargo_acquired) =
            self.acquire_cargo_resource(&prepared.session_id)?;
        match decision {
            ResourceDecision::Allow => Ok(Some((cargo_acquired, durable_cargo_acquired))),
            ResourceDecision::Defer { .. } => Err(RuntimeError::new(
                StableErrorCode::ExecutionResourceBusy,
                "prepared execution Hook is waiting for the Cargo lease",
            )),
        }
    }

    fn acquire_cargo_resource(
        &self,
        session_id: &str,
    ) -> RuntimeResult<(ResourceDecision, bool, bool)> {
        let durable = self.persistence.acquire_execution_resource_lease(
            &ExecutionResourceLeaseRequestV1 {
                resource: "cargo".to_owned(),
                boot_id: self.boot_id.to_string(),
                session_id: session_id.to_owned(),
                now_unix_ms: self.clock.now_unix_ms(),
                ttl_ms: self.config.cargo_lock_ttl_ms,
                retry_after_ms: self.config.cargo_lock_retry_after_ms,
            },
        )?;
        let durable_acquired = durable == ExecutionResourceLeaseOutcomeV1::Granted;
        if let ExecutionResourceLeaseOutcomeV1::Deferred { retry_after_ms } = durable {
            return Ok((ResourceDecision::Defer { retry_after_ms }, false, false));
        }
        let request = CargoAcquireRequest {
            session_id,
            lock_path: self.config.cargo_lock_path.as_deref(),
            now_unix_ms: self.clock.now_unix_ms(),
            ttl_ms: self.config.cargo_lock_ttl_ms,
            retry_after_ms: self.config.cargo_lock_retry_after_ms,
            queue_capacity: self.config.cargo_lock_queue_capacity,
        };
        let outcome = self
            .config
            .execution_resources()
            .cargo()
            .acquire_with_effect(ResourceKind::Cargo, &request);
        if matches!(outcome.decision, ResourceDecision::Defer { .. }) && durable_acquired {
            self.persistence.release_execution_resource_lease(
                "cargo",
                &self.boot_id.to_string(),
                session_id,
            )?;
        }
        Ok((
            outcome.decision,
            outcome.acquisition == ResourceAcquisition::Granted,
            durable_acquired && matches!(outcome.decision, ResourceDecision::Allow),
        ))
    }

    fn release_cargo_resource(&self, session_id: &str) -> RuntimeResult<()> {
        self.config
            .execution_resources()
            .cargo()
            .release(ResourceKind::Cargo, session_id);
        self.persistence.release_execution_resource_lease(
            "cargo",
            &self.boot_id.to_string(),
            session_id,
        )
    }

    fn compensate_cargo_acquisition(
        &self,
        session_id: &str,
        cargo_acquired: bool,
        durable_cargo_acquired: bool,
    ) {
        if cargo_acquired {
            self.config
                .execution_resources()
                .cargo()
                .release(ResourceKind::Cargo, session_id);
        }
        if durable_cargo_acquired {
            let _ = self.persistence.release_execution_resource_lease(
                "cargo",
                &self.boot_id.to_string(),
                session_id,
            );
        }
    }

    /// Performs the command-level bootstrap intake under the already trusted
    /// Hook session. The business authority mints the key and the runtime's
    /// regular create binding path persists it; the Hook never invents either
    /// a Work Item identity or a route decision.
    fn bootstrap_route_work_item(
        &self,
        hook_params: &RequestParams<Value>,
        session_id: &str,
    ) -> RuntimeResult<String> {
        let create = RequestParams {
            protocol_version: hook_params.protocol_version.clone(),
            workspace_id: hook_params.workspace_id.clone(),
            agent_id: hook_params.agent_id.clone(),
            session_id: hook_params.session_id.clone(),
            capability_token: hook_params.capability_token.clone(),
            turn_id: None,
            work_item_id: None,
            lease_id: None,
            fencing_token: None,
            expected_revision: None,
            idempotency_key: Some(format!("hook-bootstrap-{session_id}")),
            confirmation: None,
            deadline_ms: hook_params.deadline_ms,
            payload: json!({
                "operation":"workitem.create",
                "payload":{"entryNode":"ROUTE"},
            }),
        };
        let value = self.authoritative_business(
            RpcMethod::OperationExecute,
            &create,
            Some(ClientKind::Hook),
        )?;
        self.bind_created_work_item(&create, &value)?;
        value
            .get("data")
            .and_then(|data| data.get("workItemId"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| schema_error("bootstrap create did not return data.workItemId"))
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
    /// Gate evaluation. Returns the compensatable resource effects produced by
    /// this invocation so a failed receipt commit can release only a lease that
    /// this Hook actually acquired.
    fn apply_execution_resources(
        &self,
        identity: &TrustedSession,
        method: RpcMethod,
        payload: &HookPayload,
        execution: &ExecutionHookGuardOutcome,
        directive: &mut Option<ExecutionHookDirective>,
    ) -> RuntimeResult<ExecutionResourceEffects> {
        if !matches!(method, RpcMethod::HookPreTool | RpcMethod::HookPostTool) {
            return Ok(ExecutionResourceEffects::default());
        }
        let Some(wire) = payload.host_payload.get("executionEvent") else {
            return Ok(ExecutionResourceEffects::default());
        };
        // `decode_execution_event` already ran fail-closed on this payload, so
        // a decode failure here cannot occur for a classified event.
        let Ok(wire) = serde_json::from_value::<ExecutionHookEvent>(wire.clone()) else {
            return Ok(ExecutionResourceEffects::default());
        };
        let Some(class) = ExecutionHookToolClass::from_wire_name(wire.class.as_str()) else {
            return Ok(ExecutionResourceEffects::default());
        };
        // A directive exists exactly when the session is bound and the event
        // was classified; unbound shadow sessions are never arbitrated.
        if directive.is_none() {
            return Ok(ExecutionResourceEffects::default());
        }
        match (method, class) {
            (RpcMethod::HookPreTool, ExecutionHookToolClass::SourceRead) => {
                if execution.disposition() != ExecutionHookDisposition::Allow {
                    return Ok(ExecutionResourceEffects::default());
                }
                let Some(key) = source_read_key(identity, &wire) else {
                    return Ok(ExecutionResourceEffects::default());
                };
                let hit = self
                    .config
                    .execution_resources()
                    .source_reads()
                    .get(&source_read_visibility(identity), &key);
                if let (Some(directive), Some(reference)) = (directive.as_mut(), hit) {
                    directive.cached_read_ref = Some(reference.into_string());
                }
                Ok(ExecutionResourceEffects::default())
            }
            (RpcMethod::HookPostTool, ExecutionHookToolClass::SourceRead) => {
                let Some(key) = source_read_key(identity, &wire) else {
                    return Ok(ExecutionResourceEffects::default());
                };
                let Some(body) = payload
                    .host_payload
                    .get("toolOutput")
                    .and_then(Value::as_str)
                else {
                    return Ok(ExecutionResourceEffects::default());
                };
                self.config.execution_resources().source_reads().put(
                    &source_read_visibility(identity),
                    &key,
                    body,
                    self.config.source_read_cache_capacity,
                );
                Ok(ExecutionResourceEffects::default())
            }
            (
                RpcMethod::HookPreTool,
                ExecutionHookToolClass::FocusedTest | ExecutionHookToolClass::BroadTest,
            ) => {
                if execution.disposition() != ExecutionHookDisposition::Allow {
                    return Ok(ExecutionResourceEffects::default());
                }
                let (decision, cargo_acquired, durable_cargo_acquired) =
                    self.acquire_cargo_resource(&identity.session_id)?;
                match decision {
                    ResourceDecision::Allow => Ok(ExecutionResourceEffects {
                        cargo_deferred: false,
                        cargo_acquired,
                        durable_cargo_acquired,
                    }),
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
                        Ok(ExecutionResourceEffects {
                            cargo_deferred: true,
                            cargo_acquired: false,
                            durable_cargo_acquired: false,
                        })
                    }
                }
            }
            (
                RpcMethod::HookPostTool,
                ExecutionHookToolClass::FocusedTest | ExecutionHookToolClass::BroadTest,
            ) => {
                self.release_cargo_resource(identity.session_id.as_str())?;
                Ok(ExecutionResourceEffects::default())
            }
            _ => Ok(ExecutionResourceEffects::default()),
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
            // A completed compact rehydrate bumps the context generation and
            // replaces the host-side window: the next Hook must deliver the
            // full projection body again even though the digest never moved.
            let generation_before = self.context_generation(&identity.session_id)?;
            self.complete_compact_after_rehydrate(&identity.session_id, &result.digest)?;
            if self.context_generation(&identity.session_id)? != generation_before {
                self.context.mark_hook_redelivery(&identity.session_id)?;
            }
        }
        to_value(result)
    }

    fn context_generation(&self, session_id: &str) -> RuntimeResult<u64> {
        Ok(self
            .lock_state()?
            .sessions
            .get(session_id)
            .ok_or_else(session_expired)?
            .result
            .context_generation)
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

fn is_ae_sdd_activation(method: RpcMethod, payload: &HookPayload) -> bool {
    method == RpcMethod::HookUserPrompt
        && payload
            .host_payload
            .get("prompt")
            .and_then(Value::as_str)
            .is_some_and(|prompt| prompt.trim() == "/ae-sdd")
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

/// Reads `hookEventId` out of a raw Hook payload without a typed decode.
///
/// Used only on the failure path, where the typed decode may be exactly what
/// failed, so the identity has to be recovered from the raw value or not at all.
fn hook_event_id_of(params: &RequestParams<Value>) -> Option<String> {
    params
        .payload
        .get("hookEventId")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn ensure_hook_deadline(started: Instant, deadline_ms: u64) -> RuntimeResult<()> {
    if started.elapsed().as_millis() >= u128::from(deadline_ms) {
        return Err(RuntimeError::new(
            StableErrorCode::GateTimeout,
            "Hook fast path exceeded the caller deadline",
        ));
    }
    Ok(())
}

/// Records the answer the daemon returned for one Hook invocation.
///
/// The fields are read back out of the serialized response rather than rebuilt
/// from the values that produced it: this is meant to be a record of what the
/// caller actually received, and a parallel reconstruction could drift from the
/// wire without anything failing.
fn emit_hook_out(hook_event_id: &str, turn_id: &str, value: &Value, started: Instant) {
    let directive = value.get("executionDirective");
    diagnostics::emit(DiagnosticRecord::HookOut(HookOutRecord {
        ts: diagnostics::now_ms(),
        hid: hook_event_id.to_owned(),
        tid: turn_id.to_owned(),
        dec: value
            .get("decision")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned(),
        dir: directive
            .and_then(|directive| directive.get("decision"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        rc: directive
            .and_then(|directive| directive.get("reasonCode"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        ctx: value
            .get("contextKind")
            .and_then(Value::as_str)
            .map(str::to_owned),
        cdg: value
            .get("contextDigest")
            .and_then(Value::as_str)
            .map(str::to_owned),
        es: value.get("eventSeq").and_then(Value::as_u64).unwrap_or(0),
        rp: value
            .get("replayed")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        ok: true,
        err: None,
        ms: diagnostics::elapsed_ms(started),
    }));
}
