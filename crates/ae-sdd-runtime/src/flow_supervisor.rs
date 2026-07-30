use std::str::FromStr;
use std::sync::Arc;

use ae_sdd_contracts::SeriesInput;
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, CancellationCode, ErrorCode, EventSequence, EventStoreId,
    FindingCode, FreshnessDimension, GateCancellation, GateError, GateFailure, GateFinding,
    GateOutcome, GateTimeout, InputFingerprint, ProcessPhase, StaleGate, StateRevision,
};
use ae_sdd_flow::{
    CompactAdviceReason, ControlAction, ControlDecision, ControlPlaneRuntime, EventCursor,
    EventProvenance, FlowDecision, FlowEvent, FlowEventKind, FlowInput, FlowRuntime, NextAction,
    SupervisorDegradation, SupervisorFault, SupervisorHealth,
};
use ae_sdd_policy::RequiredGate;
use ae_sdd_protocol::StableErrorCode;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{DurableEvent, PersistencePort, RuntimeError, RuntimeResult};

const MAX_FLOW_EVENTS: usize = 4_096;
const EVENT_PAGE: usize = 256;

/// Durable wrapper around the pure flow reducer.
pub struct FlowSupervisor {
    persistence: Arc<dyn PersistencePort>,
}

impl FlowSupervisor {
    /// Creates a supervisor backed by a durable checkpoint port.
    #[must_use]
    pub fn new(persistence: Arc<dyn PersistencePort>) -> Self {
        Self { persistence }
    }

    /// Replays committed events and persists the resulting decision checkpoint.
    pub fn replay(
        &self,
        workspace_id: &str,
        work_item_id: &str,
        input: FlowInput,
        events: impl IntoIterator<Item = FlowEvent>,
    ) -> RuntimeResult<FlowDecision> {
        let decision = FlowRuntime::replay(input, events).map_err(|error| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                format!("deterministic flow replay rejected committed input: {error}"),
            )
        })?;
        self.persist_checkpoint(workspace_id, work_item_id, &decision)?;
        Ok(decision)
    }

    /// Rebuilds one deterministic decision from the current authoritative input
    /// and all matching typed events in the durable global event store.
    pub fn project(
        &self,
        workspace_id: &str,
        work_item_id: &str,
        input: FlowInput,
    ) -> RuntimeResult<FlowDecision> {
        let events = self.load_flow_events(workspace_id, work_item_id, input)?;
        self.replay(workspace_id, work_item_id, input, events)
    }

    /// Persists an idempotent root transition intent and returns the resulting
    /// reducer decision. Same-key/same-payload replay has no second side effect;
    /// same-key/different-payload fails closed.
    #[allow(clippy::too_many_arguments)]
    pub fn request_transition(
        &self,
        boot_id: &str,
        workspace_id: &str,
        session_id: Option<&str>,
        work_item_id: &str,
        idempotency_key: &str,
        input: FlowInput,
        actor_role: AgentRole,
        target: ProcessPhase,
    ) -> RuntimeResult<FlowDecision> {
        let payload = json!({
            "schemaVersion":"flow.transition-requested/v1",
            "idempotencyKey":idempotency_key,
            "actorRole":role_name(actor_role),
            "targetPhase":phase_name(target),
            "policyDigest":input.environment().policy_digest().to_string(),
            "inputFingerprint":input.environment().input_fingerprint().to_string(),
        });
        self.append_idempotent_flow_event(
            boot_id,
            workspace_id,
            session_id,
            work_item_id,
            "flow.transition_requested",
            idempotency_key,
            payload,
        )?;
        self.project(workspace_id, work_item_id, input)
    }

    /// Persists one six-state Gate result for an already pending transition and
    /// returns the next deterministic decision.
    #[allow(clippy::too_many_arguments)]
    pub fn record_gate(
        &self,
        boot_id: &str,
        workspace_id: &str,
        session_id: Option<&str>,
        work_item_id: &str,
        idempotency_key: &str,
        input: FlowInput,
        gate: RequiredGate,
        outcome: &GateOutcome,
    ) -> RuntimeResult<FlowDecision> {
        let payload = json!({
            "schemaVersion":"flow.gate-completed/v1",
            "idempotencyKey":idempotency_key,
            "gateId":gate.as_str(),
            "outcome":encode_gate_outcome(outcome),
            "policyDigest":input.environment().policy_digest().to_string(),
            "inputFingerprint":input.environment().input_fingerprint().to_string(),
        });
        self.append_idempotent_flow_event(
            boot_id,
            workspace_id,
            session_id,
            work_item_id,
            "flow.gate_completed",
            idempotency_key,
            payload,
        )?;
        self.project(workspace_id, work_item_id, input)
    }

    /// Persists the authoritative transition commit for audit/replay.
    #[allow(clippy::too_many_arguments)]
    pub fn record_transition_committed(
        &self,
        boot_id: &str,
        workspace_id: &str,
        session_id: Option<&str>,
        work_item_id: &str,
        idempotency_key: &str,
        input: FlowInput,
        phase: ProcessPhase,
        state_revision: StateRevision,
    ) -> RuntimeResult<()> {
        let payload = json!({
            "schemaVersion":"flow.transition-committed/v1",
            "idempotencyKey":idempotency_key,
            "phase":phase_name(phase),
            "stateRevision":state_revision.get(),
            "policyDigest":input.environment().policy_digest().to_string(),
            "inputFingerprint":input.environment().input_fingerprint().to_string(),
        });
        self.append_idempotent_flow_event(
            boot_id,
            workspace_id,
            session_id,
            work_item_id,
            "flow.transition_committed",
            idempotency_key,
            payload,
        )?;
        Ok(())
    }

    /// Persists a collected Root-to-Series delegation boundary and returns the
    /// resulting reducer decision, which carries an advisory compact
    /// suggestion whenever no real work owns the action.
    pub fn record_series_completed(
        &self,
        boot_id: &str,
        workspace_id: &str,
        session_id: Option<&str>,
        work_item_id: &str,
        idempotency_key: &str,
        input: FlowInput,
    ) -> RuntimeResult<FlowDecision> {
        let payload = json!({
            "schemaVersion":"flow.series-completed/v1",
            "idempotencyKey":idempotency_key,
            "policyDigest":input.environment().policy_digest().to_string(),
            "inputFingerprint":input.environment().input_fingerprint().to_string(),
        });
        self.append_idempotent_flow_event(
            boot_id,
            workspace_id,
            session_id,
            work_item_id,
            "flow.series_completed",
            idempotency_key,
            payload,
        )?;
        self.project(workspace_id, work_item_id, input)
    }

    /// Runs the pure control plane over a frozen Series input and persists the
    /// resulting delegation-oriented decision as a durable checkpoint.
    pub fn decide_control(
        &self,
        workspace_id: &str,
        work_item_id: &str,
        catalog_digest: ArtifactDigest,
        input: &SeriesInput,
    ) -> RuntimeResult<ControlDecision> {
        let decision = ControlPlaneRuntime::next(catalog_digest, input).map_err(|error| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                format!("deterministic control-plane decision rejected frozen input: {error}"),
            )
        })?;
        self.persist_control_checkpoint(workspace_id, work_item_id, &decision)?;
        Ok(decision)
    }

    /// Reads the last durable control-plane checkpoint projection.
    pub fn control_checkpoint(
        &self,
        workspace_id: &str,
        work_item_id: &str,
    ) -> RuntimeResult<Option<Value>> {
        self.persistence.load_record(
            "flow-control/v1",
            &format!("{workspace_id}\0{work_item_id}"),
        )
    }

    /// Converts a control decision into the bounded public delegation-oriented
    /// projection; a `run-series` action directs the Root to delegate to a Series.
    #[must_use]
    pub fn control_projection(decision: &ControlDecision) -> Value {
        let provenance = decision.provenance();
        json!({
            "schemaVersion":"flow-control/v1",
            "action":control_action_value(decision.action()),
            "provenance":{
                "catalogDigest":provenance.catalog_digest().to_string(),
                "routeDigest":provenance.route_digest().to_string(),
                "seriesDigest":provenance.series_digest().to_string(),
            },
            "decisionDigest":decision.decision_digest().to_string(),
        })
    }

    fn persist_control_checkpoint(
        &self,
        workspace_id: &str,
        work_item_id: &str,
        decision: &ControlDecision,
    ) -> RuntimeResult<()> {
        let checkpoint = json!({
            "schemaVersion":"flow-control-checkpoint/v1",
            "workspaceId":workspace_id,
            "workItemId":work_item_id,
            "decision":Self::control_projection(decision),
        });
        self.persistence.store_record(
            "flow-control/v1",
            &format!("{workspace_id}\0{work_item_id}"),
            &checkpoint,
        )
    }

    /// Reads the last durable checkpoint projection.
    pub fn checkpoint(
        &self,
        workspace_id: &str,
        work_item_id: &str,
    ) -> RuntimeResult<Option<Value>> {
        self.persistence.load_record(
            "flow-supervisor/v1",
            &format!("{workspace_id}\0{work_item_id}"),
        )
    }

    /// Converts a typed decision into the bounded public flow projection.
    #[must_use]
    pub fn projection(decision: &FlowDecision) -> Value {
        json!({
            "schemaVersion":"flow-decision/v1",
            "phase":phase_name(decision.snapshot().phase()),
            "stateRevision":decision.snapshot().state_revision().get(),
            "correctionCount":decision.snapshot().correction_count(),
            "pendingTransition":decision.pending_transition().map(phase_name),
            "requiredGates":decision.required_gates().iter().map(|gate| gate.as_str()).collect::<Vec<_>>(),
            "passedGates":decision.passed_gates().iter().map(|gate| gate.as_str()).collect::<Vec<_>>(),
            "health":health_value(decision.health()),
            "nextAction":next_action_value(decision.next_action()),
            "decisionDigest":decision.decision_digest().to_string(),
            "lastEventSeq":decision.last_cursor().map_or(0, |cursor| cursor.sequence().get()),
        })
    }

    fn persist_checkpoint(
        &self,
        workspace_id: &str,
        work_item_id: &str,
        decision: &FlowDecision,
    ) -> RuntimeResult<()> {
        let checkpoint = json!({
            "schemaVersion":"flow-supervisor-checkpoint/v2",
            "workspaceId":workspace_id,
            "workItemId":work_item_id,
            "decision":Self::projection(decision),
        });
        self.persistence.store_record(
            "flow-supervisor/v1",
            &format!("{workspace_id}\0{work_item_id}"),
            &checkpoint,
        )
    }

    fn append_idempotent_flow_event(
        &self,
        boot_id: &str,
        workspace_id: &str,
        session_id: Option<&str>,
        work_item_id: &str,
        kind: &str,
        idempotency_key: &str,
        payload: Value,
    ) -> RuntimeResult<DurableEvent> {
        if idempotency_key.is_empty() || idempotency_key.len() > 256 {
            return Err(RuntimeError::new(
                StableErrorCode::OperationSchemaInvalid,
                "flow event idempotencyKey is missing or oversized",
            ));
        }
        let payload_bytes = serde_json::to_vec(&payload).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::OperationSchemaInvalid,
                "flow event payload is not canonical JSON",
            )
        })?;
        let payload_digest = hex::encode(Sha256::digest(&payload_bytes));
        for event in self.events_for_work_item(workspace_id, work_item_id)? {
            if event.payload.get("idempotencyKey").and_then(Value::as_str) != Some(idempotency_key)
            {
                continue;
            }
            if event.kind == kind && event.payload_digest == payload_digest {
                return Ok(event);
            }
            return Err(RuntimeError::new(
                StableErrorCode::IdempotencyKeyReused,
                "flow event idempotencyKey was reused with a different payload",
            ));
        }
        self.persistence.append_event(DurableEvent {
            event_store_id: self.persistence.event_store_id()?.to_string(),
            event_seq: 0,
            boot_id: boot_id.to_owned(),
            kind: kind.to_owned(),
            workspace_id: Some(workspace_id.to_owned()),
            session_id: session_id.map(str::to_owned),
            work_item_id: Some(work_item_id.to_owned()),
            payload,
            payload_digest,
        })
    }

    fn load_flow_events(
        &self,
        workspace_id: &str,
        work_item_id: &str,
        input: FlowInput,
    ) -> RuntimeResult<Vec<FlowEvent>> {
        let mut output = Vec::new();
        for event in self.events_for_work_item(workspace_id, work_item_id)? {
            let Some(input_digest) = event
                .payload
                .get("inputFingerprint")
                .and_then(Value::as_str)
            else {
                continue;
            };
            if input_digest != input.environment().input_fingerprint().to_string() {
                continue;
            }
            output.push(flow_event_from_durable(&event, input)?);
        }
        Ok(output)
    }

    fn events_for_work_item(
        &self,
        workspace_id: &str,
        work_item_id: &str,
    ) -> RuntimeResult<Vec<DurableEvent>> {
        let mut after = 0_u64;
        let mut output = Vec::new();
        loop {
            let page = self.persistence.events_after(after, EVENT_PAGE)?;
            if page.is_empty() {
                break;
            }
            after = page.last().map_or(after, |event| event.event_seq);
            output.extend(page.into_iter().filter(|event| {
                event.workspace_id.as_deref() == Some(workspace_id)
                    && event.work_item_id.as_deref() == Some(work_item_id)
                    && event.kind.starts_with("flow.")
            }));
            if output.len() > MAX_FLOW_EVENTS {
                return Err(RuntimeError::new(
                    StableErrorCode::SubscriberBackpressure,
                    "flow event history exceeds the bounded replay window",
                ));
            }
        }
        Ok(output)
    }
}

fn flow_event_from_durable(event: &DurableEvent, input: FlowInput) -> RuntimeResult<FlowEvent> {
    let event_store_id = EventStoreId::from_str(&event.event_store_id).map_err(|_| {
        RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "durable flow event has an invalid eventStoreId",
        )
    })?;
    let policy_digest = event
        .payload
        .get("policyDigest")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed_event("flow event policyDigest is missing"))?;
    if policy_digest != input.environment().policy_digest().to_string() {
        return Err(malformed_event("flow event policy digest is stale"));
    }
    let event_fingerprint = InputFingerprint::from_str(&event.payload_digest).map_err(|_| {
        malformed_event("flow event payload digest is not a canonical input fingerprint")
    })?;
    let provenance = EventProvenance::new(
        EventCursor::new(event_store_id, EventSequence::new(event.event_seq)),
        input.environment().policy_digest(),
        input.environment().input_fingerprint(),
        event_fingerprint,
    );
    let kind = match event.kind.as_str() {
        "flow.transition_requested" => FlowEventKind::TransitionRequested {
            actor_role: parse_role(required_payload_string(&event.payload, "actorRole")?)?,
            target: parse_phase(required_payload_string(&event.payload, "targetPhase")?)?,
        },
        "flow.gate_completed" => FlowEventKind::GateCompleted {
            gate: parse_required_gate(required_payload_string(&event.payload, "gateId")?)?,
            outcome: decode_gate_outcome(
                event
                    .payload
                    .get("outcome")
                    .ok_or_else(|| malformed_event("flow Gate outcome is missing"))?,
            )?,
        },
        "flow.transition_committed" => FlowEventKind::TransitionCommitted {
            phase: parse_phase(required_payload_string(&event.payload, "phase")?)?,
            state_revision: StateRevision::new(
                event
                    .payload
                    .get("stateRevision")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| malformed_event("transition revision is missing"))?,
            ),
        },
        "flow.series_completed" => FlowEventKind::SeriesCompleted,
        "flow.prompt_accepted" => FlowEventKind::PromptAccepted,
        "flow.background_recovered" => FlowEventKind::BackgroundRecovered,
        "flow.background_fault" => FlowEventKind::BackgroundFault(parse_fault(
            required_payload_string(&event.payload, "fault")?,
        )?),
        _ => return Err(malformed_event("unknown typed flow event kind")),
    };
    Ok(FlowEvent::new(provenance, kind))
}

fn next_action_value(action: &NextAction) -> Value {
    match action {
        NextAction::AwaitAgentWork => json!({"kind":"await-agent-work"}),
        NextAction::EvaluateGates {
            target,
            required_gates,
        } => json!({
            "kind":"evaluate-gates",
            "targetPhase":phase_name(*target),
            "requiredGates":required_gates.iter().map(|gate| gate.as_str()).collect::<Vec<_>>(),
        }),
        NextAction::ApplyTransition { target } => {
            json!({"kind":"apply-transition","targetPhase":phase_name(*target)})
        }
        NextAction::ProvideCorrection => json!({"kind":"provide-correction"}),
        NextAction::RetryGate => json!({"kind":"retry-gate"}),
        NextAction::HaltForGateError => json!({"kind":"halt-for-gate-error"}),
        NextAction::AwaitCancellationResolution => {
            json!({"kind":"await-cancellation-resolution"})
        }
        NextAction::ReevaluateGate => json!({"kind":"reevaluate-gate"}),
        NextAction::TransitionDenied { target, reason } => json!({
            "kind":"transition-denied",
            "targetPhase":phase_name(*target),
            "reason":reason.to_string(),
        }),
        NextAction::ResumeApprovedExecution => json!({"kind":"resume-approved-execution"}),
        NextAction::ExecuteApprovedSlice {
            active_ordinal,
            queue_digest,
        } => json!({
            "kind":"execute-approved-slice",
            "activeOrdinal":active_ordinal,
            "queueDigest":queue_digest.to_string(),
        }),
        NextAction::FinalizeExecutionEvidence => json!({"kind":"finalize-execution-evidence"}),
        NextAction::CollectReviewContributions => json!({"kind":"collect-review-contributions"}),
        NextAction::FinalizeGovernance => json!({"kind":"finalize-governance"}),
        NextAction::SuggestCompact { reason } => json!({
            "kind":"suggest-compact",
            "reason":compact_advice_reason_name(*reason),
            "advisory":true,
        }),
    }
}

fn compact_advice_reason_name(reason: CompactAdviceReason) -> &'static str {
    match reason {
        CompactAdviceReason::SeriesBoundary => "series-boundary",
    }
}

fn control_action_value(action: &ControlAction) -> Value {
    match action {
        ControlAction::AwaitRouteApproval {
            idempotency_key,
            decision_id,
        } => json!({
            "kind":"await-route-approval",
            "idempotencyKey":idempotency_key.as_str(),
            "routeDecisionId":decision_id.as_str(),
        }),
        ControlAction::RunSeries {
            idempotency_key,
            plan,
        } => json!({
            "kind":"run-series",
            "idempotencyKey":idempotency_key.as_str(),
            "seriesId":plan.series_id().as_str(),
            "planDigest":plan.plan_digest().to_string(),
        }),
        ControlAction::AwaitSeries {
            idempotency_key,
            series_id,
        } => json!({
            "kind":"await-series",
            "idempotencyKey":idempotency_key.as_str(),
            "seriesId":series_id.as_str(),
        }),
        ControlAction::CollectSeries {
            idempotency_key,
            series_id,
        } => json!({
            "kind":"collect-series",
            "idempotencyKey":idempotency_key.as_str(),
            "seriesId":series_id.as_str(),
        }),
        ControlAction::Complete {
            idempotency_key,
            projection_digest,
        } => json!({
            "kind":"complete",
            "idempotencyKey":idempotency_key.as_str(),
            "projectionDigest":projection_digest.to_string(),
        }),
    }
}

fn health_value(health: SupervisorHealth) -> Value {
    match health {
        SupervisorHealth::Healthy => json!({"status":"healthy"}),
        SupervisorHealth::Degraded(reason) => json!({
            "status":"degraded",
            "reason":match reason {
                SupervisorDegradation::GateError => "gate-error",
                SupervisorDegradation::GateTimeout => "gate-timeout",
                SupervisorDegradation::Background(SupervisorFault::GateWorker) => "gate-worker",
                SupervisorDegradation::Background(SupervisorFault::EventStore) => "event-store",
                SupervisorDegradation::Background(SupervisorFault::ArtifactProjection) => "artifact-projection",
                SupervisorDegradation::Background(SupervisorFault::HostAdapter) => "host-adapter",
            },
        }),
    }
}

fn encode_gate_outcome(outcome: &GateOutcome) -> Value {
    match outcome {
        GateOutcome::Pass => json!({"kind":"PASS"}),
        GateOutcome::Fail(failure) => json!({
            "kind":"FAIL",
            "findings":failure.findings().iter().map(|finding| finding.code().as_str()).collect::<Vec<_>>(),
        }),
        GateOutcome::Error(error) => json!({
            "kind":"ERROR",
            "code":error.code().as_str(),
            "retryable":error.retryable(),
        }),
        GateOutcome::Timeout(timeout) => {
            json!({"kind":"TIMEOUT","deadlineMs":timeout.deadline_ms()})
        }
        GateOutcome::Cancelled(cancelled) => {
            json!({"kind":"CANCELLED","reason":cancelled.reason().as_str()})
        }
        GateOutcome::Stale(stale) => json!({
            "kind":"STALE",
            "changed":stale.changed().iter().map(|dimension| freshness_name(*dimension)).collect::<Vec<_>>(),
        }),
    }
}

fn decode_gate_outcome(value: &Value) -> RuntimeResult<GateOutcome> {
    match required_payload_string(value, "kind")? {
        "PASS" => Ok(GateOutcome::Pass),
        "FAIL" => {
            let findings = value
                .get("findings")
                .and_then(Value::as_array)
                .ok_or_else(|| malformed_event("FAIL findings are missing"))?
                .iter()
                .filter_map(Value::as_str)
                .map(|code| {
                    FindingCode::new(code.to_owned())
                        .map(|code| GateFinding::new(code, []))
                        .map_err(|_| malformed_event("FAIL finding code is invalid"))
                })
                .collect::<RuntimeResult<Vec<_>>>()?;
            GateFailure::new(findings)
                .map(GateOutcome::Fail)
                .map_err(|_| malformed_event("FAIL requires findings"))
        }
        "ERROR" => Ok(GateOutcome::Error(GateError::new(
            ErrorCode::new(required_payload_string(value, "code")?.to_owned())
                .map_err(|_| malformed_event("Gate error code is invalid"))?,
            value
                .get("retryable")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ))),
        "TIMEOUT" => GateTimeout::new(
            value
                .get("deadlineMs")
                .and_then(Value::as_u64)
                .ok_or_else(|| malformed_event("Gate timeout deadline is missing"))?,
        )
        .map(GateOutcome::Timeout)
        .map_err(|_| malformed_event("Gate timeout deadline is invalid")),
        "CANCELLED" => Ok(GateOutcome::Cancelled(GateCancellation::new(
            CancellationCode::new(required_payload_string(value, "reason")?.to_owned())
                .map_err(|_| malformed_event("Gate cancellation reason is invalid"))?,
        ))),
        "STALE" => {
            let changed = value
                .get("changed")
                .and_then(Value::as_array)
                .ok_or_else(|| malformed_event("STALE dimensions are missing"))?
                .iter()
                .filter_map(Value::as_str)
                .map(parse_freshness)
                .collect::<RuntimeResult<Vec<_>>>()?;
            StaleGate::new(changed)
                .map(GateOutcome::Stale)
                .map_err(|_| malformed_event("STALE requires changed dimensions"))
        }
        _ => Err(malformed_event("unknown Gate outcome kind")),
    }
}

fn parse_required_gate(value: &str) -> RuntimeResult<RequiredGate> {
    let gate = match value {
        "G-00" => RequiredGate::G00,
        "G-01" => RequiredGate::G01,
        "G-02" => RequiredGate::G02,
        "G-03" => RequiredGate::G03,
        "G-04" => RequiredGate::G04,
        "G-07" => RequiredGate::G07,
        "G-08" => RequiredGate::G08,
        "G-09" => RequiredGate::G09,
        "G-10" => RequiredGate::G10,
        "G-11" => RequiredGate::G11,
        "G-12" => RequiredGate::G12,
        "G-13" => RequiredGate::G13,
        "G-14" => RequiredGate::G14,
        "G-CODE-1" => RequiredGate::GCode1,
        "G-CODEPLAN-SRC" => RequiredGate::GCodePlanSource,
        "G-DR-CTX" => RequiredGate::GDrContext,
        "G-HTTP-1" => RequiredGate::GHttp1,
        "G-RA-1" => RequiredGate::GRa1,
        "G-RA-2" => RequiredGate::GRa2,
        "G-RA-3" => RequiredGate::GRa3,
        "G-RA-4" => RequiredGate::GRa4,
        "G-RA-5" => RequiredGate::GRa5,
        "G-RA-6" => RequiredGate::GRa6,
        "G-RA-FLOW-VIOLATION" => RequiredGate::GRaFlowViolation,
        "G-REVIEW-DEPTH" => RequiredGate::GReviewDepth,
        "G-STORY-CTX" => RequiredGate::GStoryContext,
        _ => return Err(malformed_event("Gate is not part of transition policy")),
    };
    Ok(gate)
}

fn parse_role(value: &str) -> RuntimeResult<AgentRole> {
    match value {
        "root" => Ok(AgentRole::Root),
        "series" => Ok(AgentRole::Series),
        "task" => Ok(AgentRole::Task),
        "reviewer" => Ok(AgentRole::Reviewer),
        _ => Err(malformed_event("flow actor role is invalid")),
    }
}

fn role_name(role: AgentRole) -> &'static str {
    match role {
        AgentRole::Root => "root",
        AgentRole::Series => "series",
        AgentRole::Task => "task",
        AgentRole::Reviewer => "reviewer",
    }
}

fn parse_phase(value: &str) -> RuntimeResult<ProcessPhase> {
    match value {
        "initialized" => Ok(ProcessPhase::Initialized),
        "route-selected" => Ok(ProcessPhase::RouteSelected),
        "requirement-analyzed" => Ok(ProcessPhase::RequirementAnalyzed),
        "dr-generated" => Ok(ProcessPhase::DrGenerated),
        "story-generated" => Ok(ProcessPhase::StoryGenerated),
        "testcase-generated" => Ok(ProcessPhase::TestcaseGenerated),
        "coding-process" => Ok(ProcessPhase::CodingProcess),
        "coding" => Ok(ProcessPhase::Coding),
        "test-running" => Ok(ProcessPhase::TestRunning),
        "code-reviewed" => Ok(ProcessPhase::CodeReviewed),
        "completed" => Ok(ProcessPhase::Completed),
        "paused" => Ok(ProcessPhase::Paused),
        _ => Err(malformed_event("flow phase is invalid")),
    }
}

fn phase_name(phase: ProcessPhase) -> &'static str {
    match phase {
        ProcessPhase::Initialized => "initialized",
        ProcessPhase::RouteSelected => "route-selected",
        ProcessPhase::RequirementAnalyzed => "requirement-analyzed",
        ProcessPhase::DrGenerated => "dr-generated",
        ProcessPhase::StoryGenerated => "story-generated",
        ProcessPhase::TestcaseGenerated => "testcase-generated",
        ProcessPhase::CodingProcess => "coding-process",
        ProcessPhase::Coding => "coding",
        ProcessPhase::TestRunning => "test-running",
        ProcessPhase::CodeReviewed => "code-reviewed",
        ProcessPhase::Completed => "completed",
        ProcessPhase::Paused => "paused",
    }
}

fn parse_fault(value: &str) -> RuntimeResult<SupervisorFault> {
    match value {
        "gate-worker" => Ok(SupervisorFault::GateWorker),
        "event-store" => Ok(SupervisorFault::EventStore),
        "artifact-projection" => Ok(SupervisorFault::ArtifactProjection),
        "host-adapter" => Ok(SupervisorFault::HostAdapter),
        _ => Err(malformed_event("supervisor fault is invalid")),
    }
}

fn freshness_name(value: FreshnessDimension) -> &'static str {
    match value {
        FreshnessDimension::GateId => "gate-id",
        FreshnessDimension::GateImplementation => "gate-implementation",
        FreshnessDimension::Policy => "policy",
        FreshnessDimension::Workspace => "workspace",
        FreshnessDimension::WorkItem => "work-item",
        FreshnessDimension::Story => "story",
        FreshnessDimension::StateRevision => "state-revision",
        FreshnessDimension::FencingToken => "fencing-token",
        FreshnessDimension::InventoryGeneration => "inventory-generation",
        FreshnessDimension::Toolchain => "toolchain",
        FreshnessDimension::Configuration => "configuration",
        FreshnessDimension::Input => "input",
    }
}

fn parse_freshness(value: &str) -> RuntimeResult<FreshnessDimension> {
    match value {
        "gate-id" => Ok(FreshnessDimension::GateId),
        "gate-implementation" => Ok(FreshnessDimension::GateImplementation),
        "policy" => Ok(FreshnessDimension::Policy),
        "workspace" => Ok(FreshnessDimension::Workspace),
        "work-item" => Ok(FreshnessDimension::WorkItem),
        "story" => Ok(FreshnessDimension::Story),
        "state-revision" => Ok(FreshnessDimension::StateRevision),
        "fencing-token" => Ok(FreshnessDimension::FencingToken),
        "inventory-generation" => Ok(FreshnessDimension::InventoryGeneration),
        "toolchain" => Ok(FreshnessDimension::Toolchain),
        "configuration" => Ok(FreshnessDimension::Configuration),
        "input" => Ok(FreshnessDimension::Input),
        _ => Err(malformed_event("unknown freshness dimension")),
    }
}

fn required_payload_string<'a>(value: &'a Value, field: &str) -> RuntimeResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| malformed_event("typed flow event field is missing"))
}

fn malformed_event(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::ExternalStateConflict, message)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ae_sdd_contracts::{
        ContextBundleId, IdempotencyKey, MethodologyRef, MethodologyResolution, MethodologyVariant,
        OverrideDisposition, OverrideLayer, OverrideTrace, ProcessSnapshot, ReasonCode,
        RetryPolicy, RouteDecision, RouteDecisionId, RouteDisposition, SchemaVersion, SeriesId,
        SeriesInput, SeriesKind, SeriesPlan, SkillId, resource::ContextBundleRef,
    };
    use ae_sdd_domain::{
        ArtifactDigest, ArtifactKind, ArtifactRef, ContextDigest, DecisionDigest,
        DeliverableContract, DeliverableId, DeliverableRequirement, DesignRoute, OperationId,
        ProjectPathScope, ProjectRelativePath, WorkItemId, WorkScale,
    };
    use ae_sdd_flow::{
        CompactAdviceReason, ControlAction, EventCursor, EventProvenance, FlowEnvironment,
        FlowRuntime, FlowSnapshot, RouteSelection,
    };
    use ae_sdd_policy::TransitionPolicy;

    use crate::MemoryPersistence;

    use super::*;

    fn event_store() -> EventStoreId {
        EventStoreId::from_str("00000000-0000-0000-0000-000000000111")
            .expect("fixed event store ID is valid")
    }

    fn flow_input() -> FlowInput {
        let snapshot = FlowSnapshot::new(ProcessPhase::Initialized, StateRevision::new(7), 0);
        let environment = FlowEnvironment::new(
            event_store(),
            InputFingerprint::digest(b"work-item-input-v1"),
            RouteSelection::new(WorkScale::Large, DesignRoute::Story),
        );
        FlowInput::new(snapshot, environment)
    }

    fn series_completed_event(sequence: u64) -> FlowEvent {
        FlowEvent::new(
            EventProvenance::new(
                EventCursor::new(event_store(), EventSequence::new(sequence)),
                TransitionPolicy::digest(),
                InputFingerprint::digest(b"work-item-input-v1"),
                InputFingerprint::digest(b"series-collected"),
            ),
            FlowEventKind::SeriesCompleted,
        )
    }

    #[test]
    fn suggest_compact_projection_marks_the_advice_advisory() {
        let decision = FlowRuntime::replay(flow_input(), [series_completed_event(1)])
            .expect("series boundary reduces");

        let projection = FlowSupervisor::projection(&decision);

        assert_eq!(
            projection["nextAction"],
            json!({"kind":"suggest-compact","reason":"series-boundary","advisory":true})
        );
    }

    #[test]
    fn recorded_series_completion_projects_the_advisory_action() {
        let supervisor = FlowSupervisor::new(Arc::new(MemoryPersistence::new(event_store())));
        let input = flow_input();

        let decision = supervisor
            .record_series_completed(
                "boot-1",
                "workspace-1",
                None,
                "STORY-1",
                "collect-series-1",
                input,
            )
            .expect("series completion records");

        assert_eq!(
            decision.next_action(),
            &NextAction::SuggestCompact {
                reason: CompactAdviceReason::SeriesBoundary,
            }
        );

        let replayed = supervisor
            .record_series_completed(
                "boot-1",
                "workspace-1",
                None,
                "STORY-1",
                "collect-series-1",
                input,
            )
            .expect("same-key replay is idempotent");
        assert_eq!(replayed, decision);
    }

    #[test]
    fn control_decision_persists_a_delegation_oriented_checkpoint() {
        let supervisor = FlowSupervisor::new(Arc::new(MemoryPersistence::new(event_store())));
        let input = series_input();
        let catalog_digest = ArtifactDigest::digest(b"catalog-v1");

        let decision = supervisor
            .decide_control("workspace-1", "STORY-FLOW-001", catalog_digest, &input)
            .expect("control decision");

        assert!(matches!(decision.action(), ControlAction::RunSeries { .. }));

        let checkpoint = supervisor
            .control_checkpoint("workspace-1", "STORY-FLOW-001")
            .expect("checkpoint reads")
            .expect("checkpoint persists");
        assert_eq!(
            checkpoint["decision"]["action"]["kind"],
            json!("run-series")
        );
        assert_eq!(
            checkpoint["decision"]["decisionDigest"],
            json!(decision.decision_digest().to_string())
        );
    }

    #[test]
    fn control_decision_rejects_a_foreign_catalog_snapshot() {
        let supervisor = FlowSupervisor::new(Arc::new(MemoryPersistence::new(event_store())));
        let input = series_input();

        let error = supervisor
            .decide_control(
                "workspace-1",
                "STORY-FLOW-001",
                ArtifactDigest::digest(b"catalog-v2"),
                &input,
            )
            .expect_err("foreign catalog is rejected");

        assert!(error.to_string().contains("Catalog digest mismatch"));
    }

    fn artifact(path: &str, contents: &[u8]) -> ArtifactRef {
        ArtifactRef::new(
            ArtifactKind::new("series-fixture").expect("artifact kind"),
            ProjectRelativePath::new(path).expect("project-relative path"),
            ArtifactDigest::digest(contents),
            u64::try_from(contents.len()).expect("fixture length"),
        )
    }

    fn candidate(
        work_item_id: &WorkItemId,
        kind: &str,
        series_id: &str,
        state_revision: StateRevision,
        input_fingerprint: InputFingerprint,
    ) -> (SeriesPlan, MethodologyResolution) {
        let series_kind = SeriesKind::new(kind).expect("series kind");
        let skill_id = SkillId::new(format!("phase.{kind}")).expect("skill id");
        let methodology = MethodologyRef::new(
            SchemaVersion::V1,
            skill_id.clone(),
            series_kind.clone(),
            MethodologyVariant::new("builtin-v1").expect("variant"),
            artifact(&format!("runtime/skills/{kind}/compact.md"), b"compact"),
            None,
            ArtifactDigest::digest(format!("entry:{kind}")),
            ArtifactDigest::digest(b"catalog-v1"),
        )
        .expect("methodology ref");
        let plan = SeriesPlan::new(
            SchemaVersion::V1,
            SeriesId::new(series_id).expect("series id"),
            work_item_id.clone(),
            None,
            series_kind,
            AgentRole::Series,
            methodology.clone(),
            ContextBundleRef::new(
                SchemaVersion::V1,
                ContextBundleId::new(format!("context-{series_id}")).expect("context id"),
                work_item_id.clone(),
                vec![artifact("ae-sdd-doc/Story/STORY-FLOW-001.md", b"story")],
                ContextDigest::digest(format!("context:{kind}")),
                5,
            )
            .expect("context ref"),
            DeliverableContract::bounded_default([DeliverableRequirement::new(
                DeliverableId::new(format!("deliverable-{kind}")).expect("deliverable id"),
                ArtifactKind::new("document").expect("artifact kind"),
                ProjectRelativePath::new(format!("ae-sdd-doc/{kind}.md"))
                    .expect("deliverable path"),
            )])
            .expect("deliverable contract"),
            vec![OperationId::new("document.save").expect("operation")],
            vec![ProjectPathScope::Subtree(
                ProjectRelativePath::new("ae-sdd-doc").expect("path scope"),
            )],
            Vec::new(),
            state_revision,
            input_fingerprint,
            1_900_000_000_000,
            RetryPolicy::new(2, 250, 2_000).expect("retry policy"),
            DecisionDigest::digest(format!("plan:{kind}")),
        )
        .expect("series plan");
        let resolution = MethodologyResolution::new(
            SchemaVersion::V1,
            methodology,
            OverrideLayer::BuiltIn,
            vec![OverrideTrace::new(
                SchemaVersion::V1,
                OverrideLayer::BuiltIn,
                skill_id,
                OverrideDisposition::Selected,
                ReasonCode::new("methodology.builtin_selected").expect("reason"),
                ArtifactDigest::digest(format!("entry:{kind}")),
            )],
            DecisionDigest::digest(format!("resolution:{kind}")),
        )
        .expect("resolution");
        (plan, resolution)
    }

    fn series_input() -> SeriesInput {
        let work_item_id = WorkItemId::new("STORY-FLOW-001").expect("work item");
        let state_revision = StateRevision::new(7);
        let input_fingerprint = InputFingerprint::digest(b"series input r7");
        let (requirement_plan, requirement_resolution) = candidate(
            &work_item_id,
            "requirement-analysis",
            "series-flow-ra-r7",
            state_revision,
            input_fingerprint,
        );
        let (story_plan, story_resolution) = candidate(
            &work_item_id,
            "story",
            "series-flow-story-r7",
            state_revision,
            input_fingerprint,
        );
        let route = RouteDecision::new(
            SchemaVersion::V1,
            RouteDecisionId::new("route-flow-r7").expect("route id"),
            work_item_id.clone(),
            WorkScale::Medium,
            DesignRoute::Story,
            RouteDisposition::Approved,
            vec![ReasonCode::new("route.medium_story").expect("reason")],
            vec![
                SeriesKind::new("requirement-analysis").expect("series kind"),
                SeriesKind::new("story").expect("series kind"),
            ],
            InputFingerprint::digest(b"route input"),
            None,
            DecisionDigest::digest(b"route decision"),
        )
        .expect("route decision");
        SeriesInput::new(
            SchemaVersion::V1,
            route,
            ProcessSnapshot::new(
                SchemaVersion::V1,
                work_item_id,
                ProcessPhase::RequirementAnalyzed,
                None,
                state_revision,
                ArtifactDigest::digest(b"state r7"),
            ),
            Vec::new(),
            vec![requirement_resolution, story_resolution],
            vec![requirement_plan, story_plan],
            AgentRole::Root,
            state_revision,
            input_fingerprint,
            IdempotencyKey::new("series-next-flow-r7").expect("idempotency key"),
        )
        .expect("series input")
    }
}
