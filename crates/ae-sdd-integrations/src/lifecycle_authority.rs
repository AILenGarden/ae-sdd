use ae_sdd_contracts::lifecycle::CompletionMilestoneInput;
use std::{collections::BTreeMap, str::FromStr};

use ae_sdd_contracts::{
    FileLockSnapshot, LifecycleCommand, LifecycleDisposition, LifecycleInput, MutationIntent,
    PrdId, PrdSummary, ProcessSnapshot, ReasonCode, SchemaVersion, StorySummary,
};
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, DecisionDigest, DesignRoute, EvidenceDigest, EvidenceId,
    EvidenceRef, InputFingerprint, ProcessPhase, ProjectRelativePath, SessionId, StateRevision,
    StoryId, VerificationId, WorkItemId, WorkScale,
};
use ae_sdd_lifecycle::LifecycleEngine;
use ae_sdd_operations::{Confirmation, OperationName};
use ae_sdd_policy::RequiredGate;
use ae_sdd_protocol::{ConfirmationRef, StableErrorCode};
use ae_sdd_runtime::{RuntimeError, RuntimeResult};
use serde_json::{Map, Value, json};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LifecycleAuthorityDisposition {
    Permitted,
    Denied,
    AwaitingConfirmation,
}

#[derive(Clone, Debug)]
pub(crate) struct LifecycleOutcome {
    disposition: LifecycleAuthorityDisposition,
    intents: Vec<MutationIntent>,
    remediation: Vec<String>,
    confirmation_binding: Option<String>,
    target_phase: Option<ProcessPhase>,
    data: Value,
    input: LifecycleInput,
    plan_digest: DecisionDigest,
}

impl LifecycleOutcome {
    #[allow(dead_code)]
    pub(crate) const fn disposition(&self) -> LifecycleAuthorityDisposition {
        self.disposition
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn intents(&self) -> &[MutationIntent] {
        &self.intents
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn remediation(&self) -> &[String] {
        &self.remediation
    }

    #[allow(dead_code)]
    pub(crate) fn confirmation_binding(&self) -> Option<&str> {
        self.confirmation_binding.as_deref()
    }

    pub(crate) fn into_permitted(self) -> RuntimeResult<PermittedLifecycleMutation> {
        match self.disposition {
            LifecycleAuthorityDisposition::Permitted => Ok(PermittedLifecycleMutation {
                intents: self.intents,
                target_phase: self.target_phase,
                data: self.data,
                input: self.input,
                plan_digest: self.plan_digest,
            }),
            LifecycleAuthorityDisposition::Denied => {
                let mut error = RuntimeError::new(
                    StableErrorCode::GateBlocked,
                    "lifecycle authority denied the requested mutation",
                );
                if !self.remediation.is_empty() {
                    error = error.with_remediation(self.remediation.join(", "));
                }
                Err(error)
            }
            LifecycleAuthorityDisposition::AwaitingConfirmation => {
                let remediation = self.confirmation_binding.map_or_else(
                    || "provide lifecycle confirmation bound to the current plan".to_owned(),
                    |binding| format!("provide lifecycle confirmation for binding {binding}"),
                );
                Err(RuntimeError::new(
                    StableErrorCode::ConfirmationRequired,
                    "lifecycle authority requires confirmation",
                )
                .with_remediation(remediation))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PermittedLifecycleMutation {
    intents: Vec<MutationIntent>,
    target_phase: Option<ProcessPhase>,
    data: Value,
    input: LifecycleInput,
    plan_digest: DecisionDigest,
}

impl PermittedLifecycleMutation {
    #[allow(dead_code)]
    pub(crate) fn intents(&self) -> &[MutationIntent] {
        &self.intents
    }

    #[allow(dead_code)]
    pub(crate) const fn target_phase(&self) -> Option<ProcessPhase> {
        self.target_phase
    }

    #[allow(dead_code)]
    pub(crate) const fn data(&self) -> &Value {
        &self.data
    }

    #[allow(dead_code)]
    pub(crate) const fn plan_digest(&self) -> DecisionDigest {
        self.plan_digest
    }
}

pub(crate) fn validate_exact_intents(
    prepared: &PermittedLifecycleMutation,
    supplied_plan_digest: &str,
    supplied_intents: &[MutationIntent],
) -> RuntimeResult<()> {
    let recomputed = LifecycleEngine::plan(&prepared.input)
        .map_err(|_| schema_error("lifecycle plan could not be recomputed"))?;
    if recomputed.disposition() != LifecycleDisposition::Permitted {
        return Err(schema_error("recomputed lifecycle plan is not permitted"));
    }
    let supplied_plan_digest = DecisionDigest::from_str(supplied_plan_digest)
        .map_err(|_| schema_error("lifecycle plan digest is invalid"))?;
    if recomputed.plan_digest() != prepared.plan_digest
        || recomputed.plan_digest() != supplied_plan_digest
    {
        return Err(schema_error("lifecycle plan digest does not match"));
    }
    if supplied_intents.len() != 2 {
        return Err(schema_error(
            "lifecycle plan must contain exactly two intents",
        ));
    }
    if supplied_intents != recomputed.intents() {
        return Err(schema_error(
            "lifecycle mutation intents do not exactly match the planned sequence",
        ));
    }
    Ok(())
}

pub(crate) fn apply_exact_after_image(
    state: &Value,
    work_item_id: &str,
    prepared: &PermittedLifecycleMutation,
) -> RuntimeResult<Value> {
    validate_exact_intents(
        prepared,
        &prepared.plan_digest.to_string(),
        &prepared.intents,
    )?;
    if prepared.input.snapshot().work_item_id.as_str() != work_item_id
        || prepared.input.snapshot().state_revision
            != StateRevision::new(required_u64(state, "revision")?)
    {
        return Err(schema_error(
            "lifecycle after-image input does not match the locked Work Item revision",
        ));
    }
    let state_bytes = serde_json::to_vec(state)
        .map_err(|_| schema_error("authoritative state could not be canonicalized"))?;
    if ArtifactDigest::digest(&state_bytes) != prepared.input.snapshot().state_digest {
        return Err(schema_error(
            "lifecycle after-image before digest does not match the locked state",
        ));
    }

    let mut after = state.clone();
    match prepared.input.command() {
        LifecycleCommand::Pause => {
            let target = work_item_object_mut(&mut after, work_item_id)?;
            apply_pause(target, prepared.input.snapshot().phase);
        }
        LifecycleCommand::Resume => {
            let source =
                prepared.input.snapshot().paused_from.ok_or_else(|| {
                    schema_error("resume source is missing from the locked snapshot")
                })?;
            let target = work_item_object_mut(&mut after, work_item_id)?;
            apply_resume(target, source);
        }
        LifecycleCommand::Transition { target_phase } => {
            let target = work_item_object_mut(&mut after, work_item_id)?;
            apply_transition(target, *target_phase)?;
        }
        LifecycleCommand::CompletePrd { prd_id } => {
            if prd_id.as_str() != work_item_id {
                return Err(schema_error(
                    "CompletePrd identity does not match the locked Work Item",
                ));
            }
            apply_complete_prd(&mut after, work_item_id)?;
        }
        _ => {
            return Err(schema_error(
                "lifecycle after-image reducer does not implement this command",
            ));
        }
    }
    Ok(after)
}

fn apply_complete_prd(state: &mut Value, work_item_id: &str) -> RuntimeResult<()> {
    let nested = nested_prd_view(state)?
        .is_some_and(|prd| prd.get("prdId").and_then(Value::as_str) == Some(work_item_id));
    if !nested && !is_flat_prd_root(state, work_item_id) {
        return Err(schema_error(
            "CompletePrd target is not the authoritative PRD projection",
        ));
    }

    apply_transition(
        work_item_object_mut(state, work_item_id)?,
        ProcessPhase::Completed,
    )?;

    let root = state
        .as_object_mut()
        .ok_or_else(|| schema_error("authoritative state must be an object"))?;
    if nested {
        let completed = Value::String("completed".to_owned());
        root.insert("phase".to_owned(), completed.clone());
        root.insert("currentPhase".to_owned(), completed.clone());
        root.insert("currentStep".to_owned(), completed);
    }
    root.insert(
        "prdStatus".to_owned(),
        Value::String("awaiting_compact".to_owned()),
    );
    Ok(())
}

fn apply_pause(target: &mut Map<String, Value>, source: ProcessPhase) {
    target.insert(
        "pausedFromPhase".to_owned(),
        Value::String(phase_wire(source).to_owned()),
    );
    target.insert("phase".to_owned(), Value::String("paused".to_owned()));
    target.insert(
        "currentPhase".to_owned(),
        Value::String("paused".to_owned()),
    );
    target.insert(
        "pauseReason".to_owned(),
        Value::String("user-manual".to_owned()),
    );
}

fn apply_resume(target: &mut Map<String, Value>, source: ProcessPhase) {
    let source = Value::String(phase_wire(source).to_owned());
    target.insert("phase".to_owned(), source.clone());
    target.insert("currentPhase".to_owned(), source);
    target.remove("pausedFromPhase");
    target.remove("pausedFrom");
    target.remove("pauseReason");
}

fn apply_transition(target: &mut Map<String, Value>, phase: ProcessPhase) -> RuntimeResult<()> {
    let previous_step = target
        .get("currentStep")
        .or_else(|| target.get("phase"))
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error("currentStep or phase must be a string"))?
        .to_owned();
    let mut completed_steps = Vec::new();
    if let Some(existing) = target.get("completedSteps") {
        let existing = existing
            .as_array()
            .ok_or_else(|| schema_error("completedSteps must be an array"))?;
        for step in existing {
            let step = step
                .as_str()
                .ok_or_else(|| schema_error("completedSteps entries must be strings"))?;
            if !completed_steps.iter().any(|item| item == step) {
                completed_steps.push(step.to_owned());
            }
        }
    }
    if !completed_steps.iter().any(|step| step == &previous_step) {
        completed_steps.push(previous_step);
    }
    let coding_round = is_coding_or_later(phase)
        .then(|| normalized_coding_round(target.get("codingRound")))
        .transpose()?;
    let pending_outputs = (phase == ProcessPhase::Completed)
        .then(|| cleared_pending_outputs(target.get("pendingOutputs")))
        .transpose()?;

    let phase = Value::String(phase_wire(phase).to_owned());
    target.insert("phase".to_owned(), phase.clone());
    target.insert("currentPhase".to_owned(), phase.clone());
    target.insert("currentStep".to_owned(), phase);
    target.insert(
        "completedSteps".to_owned(),
        Value::Array(completed_steps.into_iter().map(Value::String).collect()),
    );
    if let Some(coding_round) = coding_round {
        target.insert("codingRound".to_owned(), coding_round);
    }
    if let Some(pending_outputs) = pending_outputs {
        target.insert("pendingOutputs".to_owned(), pending_outputs);
        clear_pause_fields(target);
    }
    Ok(())
}

fn normalized_coding_round(value: Option<&Value>) -> RuntimeResult<Value> {
    let Some(value) = value else {
        return Ok(json!(1));
    };
    let round = if let Some(round) = value.as_u64() {
        round
    } else if let Some(round) = value.as_str() {
        let digits = round
            .strip_prefix('r')
            .filter(|digits| !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
            .ok_or_else(|| schema_error("codingRound must be a non-negative integer or rN"))?;
        digits
            .parse::<u64>()
            .map_err(|_| schema_error("codingRound exceeds its supported range"))?
    } else {
        return Err(schema_error(
            "codingRound must be a non-negative integer or rN",
        ));
    };
    u32::try_from(round).map_err(|_| schema_error("codingRound exceeds u32"))?;
    if round == 0 {
        Ok(json!(1))
    } else {
        Ok(value.clone())
    }
}

fn cleared_pending_outputs(value: Option<&Value>) -> RuntimeResult<Value> {
    match value {
        None => Ok(Value::Object(Map::new())),
        Some(Value::Object(_)) => Ok(Value::Object(Map::new())),
        Some(Value::Array(_)) => Ok(Value::Array(Vec::new())),
        Some(value) if value.as_u64().is_some() => Ok(json!(0)),
        Some(_) => Err(schema_error(
            "pendingOutputs must be an object, array, or non-negative integer",
        )),
    }
}

fn clear_pause_fields(target: &mut Map<String, Value>) {
    target.remove("pausedFromPhase");
    target.remove("pausedFrom");
    target.remove("pauseReason");
}

const fn is_coding_or_later(phase: ProcessPhase) -> bool {
    matches!(
        phase,
        ProcessPhase::Coding
            | ProcessPhase::TestRunning
            | ProcessPhase::CodeReviewed
            | ProcessPhase::Completed
    )
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn prepare_lifecycle_mutation(
    state: &Value,
    work_item_id: &str,
    operation: OperationName,
    payload: &Value,
    expected_revision: StateRevision,
    completion: Option<CompletionMilestoneInput>,
    confirmation: Option<&Confirmation>,
    actor_role: AgentRole,
    _authenticated_session_id: Option<SessionId>,
    evaluation_unix_ms: u64,
) -> RuntimeResult<LifecycleOutcome> {
    prepare_lifecycle_mutation_with_gate_passes(
        state,
        work_item_id,
        operation,
        payload,
        expected_revision,
        completion,
        confirmation,
        actor_role,
        _authenticated_session_id,
        evaluation_unix_ms,
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_lifecycle_mutation_with_gate_passes(
    state: &Value,
    work_item_id: &str,
    operation: OperationName,
    payload: &Value,
    expected_revision: StateRevision,
    completion: Option<CompletionMilestoneInput>,
    confirmation: Option<&Confirmation>,
    actor_role: AgentRole,
    _authenticated_session_id: Option<SessionId>,
    evaluation_unix_ms: u64,
    passed_gates: &[RequiredGate],
) -> RuntimeResult<LifecycleOutcome> {
    let state_revision = StateRevision::new(required_u64(state, "revision")?);
    if state_revision != expected_revision {
        return Err(RuntimeError::new(
            StableErrorCode::RevisionConflict,
            "authoritative state revision does not match expectedRevision",
        ));
    }

    let view = work_item_view(state, work_item_id)?;
    let phase = authoritative_phase(view)?;
    let paused_from = authoritative_paused_from(view, phase)?;

    let is_prd_completion = if operation == OperationName::WorkItemComplete {
        is_prd_root(state, work_item_id)?
    } else {
        false
    };
    if is_prd_completion {
        validate_prd_status(state)?;
    }
    let (command, target_phase) = match operation {
        OperationName::StateTransition => {
            let target = parse_phase(required_str(payload, "targetPhase")?)?;
            let command = if target == ProcessPhase::Paused {
                LifecycleCommand::Pause
            } else if phase == ProcessPhase::Paused {
                if paused_from != Some(target) {
                    return Err(schema_error(
                        "resume target must match authoritative pausedFromPhase",
                    ));
                }
                LifecycleCommand::Resume
            } else {
                LifecycleCommand::Transition {
                    target_phase: target,
                }
            };
            (command, Some(target))
        }
        OperationName::WorkItemComplete if is_prd_completion => (
            LifecycleCommand::CompletePrd {
                prd_id: PrdId::new(work_item_id)
                    .map_err(|_| schema_error("workItemId is not a valid PRD id"))?,
            },
            Some(ProcessPhase::Completed),
        ),
        OperationName::WorkItemComplete => (
            LifecycleCommand::Transition {
                target_phase: ProcessPhase::Completed,
            },
            Some(ProcessPhase::Completed),
        ),
        _ => {
            return Err(schema_error(
                "operation is not governed by lifecycle authority",
            ));
        }
    };

    let state_bytes = serde_json::to_vec(state)
        .map_err(|_| schema_error("authoritative state could not be canonicalized"))?;
    let fingerprint_bytes = serde_json::to_vec(&json!({
        "state": state,
        "workItemId": work_item_id,
        "operation": operation.as_str(),
        "payload": payload,
        "passedGateIds": passed_gates.iter().map(|gate| gate.as_str()).collect::<Vec<_>>(),
    }))
    .map_err(|_| schema_error("lifecycle input could not be canonicalized"))?;
    let story_summaries = project_story_summaries(state)?;
    let prd_summary = if is_prd_completion {
        Some(project_prd_summary(state, work_item_id, &story_summaries)?)
    } else {
        None
    };
    let snapshot = ProcessSnapshot::new(
        SchemaVersion::V1,
        WorkItemId::new(work_item_id).map_err(|_| schema_error("workItemId is invalid"))?,
        phase,
        paused_from,
        state_revision,
        ArtifactDigest::digest(&state_bytes),
    );
    let scale = parse_scale(
        state
            .get("scale")
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("authoritative scale is missing"))?,
    )?;
    let design_route = parse_design_route(
        state
            .pointer("/routeDecision/selectedDesign")
            .or_else(|| state.get("selectedDesign"))
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("authoritative design route is missing"))?,
    )?;
    let evidence_refs = project_evidence_refs(state)?;
    let passed_gate_ids = passed_gates
        .iter()
        .map(|gate| {
            VerificationId::new(gate.as_str())
                .map_err(|_| schema_error("daemon Gate id is invalid"))
        })
        .collect::<RuntimeResult<Vec<_>>>()?;
    let file_locks = project_file_locks(state)?;
    let build_input = |confirmation_refs| {
        LifecycleInput::new(
            SchemaVersion::V1,
            command.clone(),
            snapshot.clone(),
            expected_revision,
            actor_role,
            scale,
            design_route,
            story_summaries.clone(),
            prd_summary.clone(),
            confirmation_refs,
            evidence_refs.clone(),
            file_locks.clone(),
            evaluation_unix_ms,
            InputFingerprint::digest(&fingerprint_bytes),
        )
        .and_then(|input| input.with_passed_gate_ids(passed_gate_ids.clone()))
        .map(|input| match completion {
            Some(completion) => input.with_completion(completion),
            None => input,
        })
        .map_err(|_| schema_error("authoritative lifecycle projection is invalid"))
    };
    let unsigned_input = build_input(Vec::new())?;
    let unsigned_plan = LifecycleEngine::plan(&unsigned_input)
        .map_err(|_| schema_error("lifecycle engine rejected the projected contract"))?;
    let (input, plan) = if let Some(confirmation) = confirmation {
        let confirmation_refs = vec![ConfirmationRef {
            confirmation_id: confirmation.confirmation_id().to_owned(),
            approved_by: confirmation.approved_by().to_owned(),
            approved_at: confirmation.approved_at().to_owned(),
        }];
        let input = build_input(confirmation_refs)?;
        let plan = LifecycleEngine::plan(&input)
            .map_err(|_| schema_error("lifecycle engine rejected the confirmation binding"))?;
        (input, plan)
    } else {
        (unsigned_input, unsigned_plan)
    };
    let plan_value = serde_json::to_value(&plan)
        .map_err(|_| schema_error("lifecycle plan could not be projected"))?;

    let disposition = match plan.disposition() {
        LifecycleDisposition::Permitted => LifecycleAuthorityDisposition::Permitted,
        LifecycleDisposition::Denied => LifecycleAuthorityDisposition::Denied,
        LifecycleDisposition::AwaitingConfirmation => {
            LifecycleAuthorityDisposition::AwaitingConfirmation
        }
    };
    let intents = if disposition == LifecycleAuthorityDisposition::Permitted {
        plan.intents().to_vec()
    } else {
        Vec::new()
    };
    let remediation = plan_value
        .get("remediation")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("code").and_then(Value::as_str))
        .map(str::to_owned)
        .collect();
    let confirmation_binding = Some(
        plan_value
            .pointer("/confirmationRequirement/bindingDigest")
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("lifecycle plan is missing its binding digest"))?
            .to_owned(),
    );
    let data = json!({
        "phase": target_phase.map(phase_wire),
        "planDigest": plan.plan_digest().to_string(),
    });
    Ok(LifecycleOutcome {
        disposition,
        intents,
        remediation,
        confirmation_binding,
        target_phase,
        data,
        input,
        plan_digest: plan.plan_digest(),
    })
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn preflight_lifecycle_confirmation(
    state: &Value,
    work_item_id: &str,
    operation: OperationName,
    payload: &Value,
    expected_revision: StateRevision,
    completion: Option<CompletionMilestoneInput>,
    actor_role: AgentRole,
    authenticated_session_id: Option<SessionId>,
    evaluation_unix_ms: u64,
) -> RuntimeResult<LifecycleOutcome> {
    prepare_lifecycle_mutation(
        state,
        work_item_id,
        operation,
        payload,
        expected_revision,
        completion,
        None,
        actor_role,
        authenticated_session_id,
        evaluation_unix_ms,
    )
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub(crate) fn preflight_lifecycle_confirmation_with_gate_passes(
    state: &Value,
    work_item_id: &str,
    operation: OperationName,
    payload: &Value,
    expected_revision: StateRevision,
    completion: Option<CompletionMilestoneInput>,
    actor_role: AgentRole,
    authenticated_session_id: Option<SessionId>,
    evaluation_unix_ms: u64,
    passed_gates: &[RequiredGate],
) -> RuntimeResult<LifecycleOutcome> {
    prepare_lifecycle_mutation_with_gate_passes(
        state,
        work_item_id,
        operation,
        payload,
        expected_revision,
        completion,
        None,
        actor_role,
        authenticated_session_id,
        evaluation_unix_ms,
        passed_gates,
    )
}

fn work_item_view<'a>(state: &'a Value, work_item_id: &str) -> RuntimeResult<&'a Value> {
    let stories = merged_story_views(state)?;
    if let Some(prd) = nested_prd_view(state)? {
        if required_str(prd, "prdId")? == work_item_id {
            return Ok(prd);
        }
    } else if is_flat_prd_root(state, work_item_id) || is_flat_route_root(state, work_item_id) {
        return Ok(state);
    }
    if let Some(dr) = singular_dr_view(state)?
        && required_str(dr, "drId")? == work_item_id
    {
        return Ok(dr);
    }
    if let Some(dr) = dr_states(state)?.and_then(|states| states.get(work_item_id)) {
        return Ok(dr);
    }
    if let Some(story) = stories.get(work_item_id) {
        return Ok(story.value);
    }
    if let Some(tasks) = object_field(state, "taskStates")?
        && let Some(task) = tasks.get(work_item_id)
    {
        return Ok(task);
    }
    Err(schema_error(
        "workItemId is not present in the authoritative state projection",
    ))
}

fn work_item_object_mut<'a>(
    state: &'a mut Value,
    work_item_id: &str,
) -> RuntimeResult<&'a mut Map<String, Value>> {
    let stories = merged_story_views(state)?;
    let story_location = stories
        .get(work_item_id)
        .map(|story| story.location.clone());
    drop(stories);
    let nested_prd = nested_prd_view(state)?
        .is_some_and(|prd| prd.get("prdId").and_then(Value::as_str) == Some(work_item_id));
    let flat_root = !nested_prd
        && (is_flat_prd_root(state, work_item_id) || is_flat_route_root(state, work_item_id));
    let singular_dr = singular_dr_view(state)?
        .is_some_and(|dr| dr.get("drId").and_then(Value::as_str) == Some(work_item_id));
    let mapped_dr = dr_states(state)?.is_some_and(|states| states.contains_key(work_item_id));

    if nested_prd {
        return state
            .get_mut("prdState")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| schema_error("prdState must be an object"));
    }
    if flat_root {
        return state
            .as_object_mut()
            .ok_or_else(|| schema_error("authoritative state must be an object"));
    }
    if singular_dr {
        return state
            .get_mut("drState")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| schema_error("drState must be an object"));
    }
    if mapped_dr {
        return state
            .get_mut("drStates")
            .and_then(Value::as_object_mut)
            .and_then(|states| states.get_mut(work_item_id))
            .and_then(Value::as_object_mut)
            .ok_or_else(|| schema_error("drStates entry must be an object"));
    }
    if let Some(location) = story_location {
        return story_object_mut(state, work_item_id, location);
    }
    if object_field(state, "taskStates")?.is_some_and(|tasks| tasks.contains_key(work_item_id)) {
        return state
            .get_mut("taskStates")
            .and_then(Value::as_object_mut)
            .and_then(|tasks| tasks.get_mut(work_item_id))
            .and_then(Value::as_object_mut)
            .ok_or_else(|| schema_error("taskStates entry must be an object"));
    }
    Err(schema_error(
        "workItemId is not present in the authoritative state projection",
    ))
}

fn is_prd_root(state: &Value, work_item_id: &str) -> RuntimeResult<bool> {
    if let Some(prd) = nested_prd_view(state)? {
        return Ok(required_str(prd, "prdId")? == work_item_id);
    }
    Ok(is_flat_prd_root(state, work_item_id))
}

#[derive(Clone, Debug)]
enum StoryLocation {
    Root,
    DrState,
    DrStates(String),
}

struct MergedStoryView<'a> {
    value: &'a Value,
    location: StoryLocation,
    nested_owner: Option<String>,
}

fn object_field<'a>(
    state: &'a Value,
    field: &str,
) -> RuntimeResult<Option<&'a Map<String, Value>>> {
    state
        .get(field)
        .map(|value| {
            value
                .as_object()
                .ok_or_else(|| schema_error(&format!("{field} must be an object")))
        })
        .transpose()
}

fn nested_prd_view(state: &Value) -> RuntimeResult<Option<&Value>> {
    let Some(prd) = state.get("prdState") else {
        return Ok(None);
    };
    prd.as_object()
        .ok_or_else(|| schema_error("prdState must be an object"))?;
    let prd_id = required_str(prd, "prdId")?;
    PrdId::new(prd_id).map_err(|_| schema_error("prdState.prdId is invalid"))?;
    let phase = authoritative_phase(prd)?;
    if state.get("phase").is_some() && authoritative_phase(state)? != phase {
        return Err(schema_error(
            "top-level phase mirror must match authoritative prdState phase",
        ));
    }
    Ok(Some(prd))
}

fn is_flat_prd_root(state: &Value, work_item_id: &str) -> bool {
    state.get("prdState").is_none()
        && state.get("stateMachineName").and_then(Value::as_str) == Some(work_item_id)
        && state.get("prdCompletion").is_some_and(Value::is_object)
}

fn is_flat_route_root(state: &Value, work_item_id: &str) -> bool {
    state.get("prdState").is_none()
        && state.get("entryNode").and_then(Value::as_str) == Some("ROUTE")
        && state.get("stateMachineName").and_then(Value::as_str) == Some(work_item_id)
}

fn singular_dr_view(state: &Value) -> RuntimeResult<Option<&Value>> {
    let Some(dr) = state.get("drState") else {
        return Ok(None);
    };
    dr.as_object()
        .ok_or_else(|| schema_error("drState must be an object"))?;
    required_str(dr, "drId")?;
    authoritative_phase(dr)?;
    Ok(Some(dr))
}

fn dr_states(state: &Value) -> RuntimeResult<Option<&Map<String, Value>>> {
    let Some(states) = object_field(state, "drStates")? else {
        return Ok(None);
    };
    let singular_id = singular_dr_view(state)?
        .map(|dr| required_str(dr, "drId"))
        .transpose()?;
    for (dr_id, dr) in states {
        dr.as_object()
            .ok_or_else(|| schema_error("drStates entries must be objects"))?;
        if required_str(dr, "drId")? != dr_id {
            return Err(schema_error("drStates key must match each drId"));
        }
        authoritative_phase(dr)?;
        if singular_id == Some(dr_id.as_str()) {
            return Err(schema_error(
                "the same DR cannot be owned by drState and drStates",
            ));
        }
    }
    Ok(Some(states))
}

fn merged_story_views(state: &Value) -> RuntimeResult<BTreeMap<String, MergedStoryView<'_>>> {
    let mut stories = BTreeMap::new();
    if let Some(root_stories) = object_field(state, "storyStates")? {
        for (story_id, story) in root_stories {
            validate_story_projection(story_id, story)?;
            stories.insert(
                story_id.clone(),
                MergedStoryView {
                    value: story,
                    location: StoryLocation::Root,
                    nested_owner: None,
                },
            );
        }
    }
    if let Some(dr) = singular_dr_view(state)? {
        let owner = format!("drState:{}", required_str(dr, "drId")?);
        if let Some(nested) = object_field(dr, "storyStates")? {
            for (story_id, story) in nested {
                merge_nested_story(
                    &mut stories,
                    story_id,
                    story,
                    &owner,
                    StoryLocation::DrState,
                )?;
            }
        }
    }
    if let Some(states) = dr_states(state)? {
        for (dr_id, dr) in states {
            if let Some(nested) = object_field(dr, "storyStates")? {
                let owner = format!("drStates:{dr_id}");
                for (story_id, story) in nested {
                    merge_nested_story(
                        &mut stories,
                        story_id,
                        story,
                        &owner,
                        StoryLocation::DrStates(dr_id.clone()),
                    )?;
                }
            }
        }
    }
    Ok(stories)
}

fn merge_nested_story<'a>(
    stories: &mut BTreeMap<String, MergedStoryView<'a>>,
    story_id: &str,
    story: &'a Value,
    owner: &str,
    location: StoryLocation,
) -> RuntimeResult<()> {
    validate_story_projection(story_id, story)?;
    if let Some(existing) = stories.get_mut(story_id) {
        if existing.nested_owner.is_some() {
            return Err(schema_error(
                "a Story cannot be owned by more than one DR projection",
            ));
        }
        if existing.value != story {
            return Err(schema_error(
                "top-level and nested Story mirrors must be byte-identical",
            ));
        }
        existing.nested_owner = Some(owner.to_owned());
    } else {
        stories.insert(
            story_id.to_owned(),
            MergedStoryView {
                value: story,
                location,
                nested_owner: Some(owner.to_owned()),
            },
        );
    }
    Ok(())
}

fn validate_story_projection(story_id: &str, story: &Value) -> RuntimeResult<()> {
    StoryId::new(story_id).map_err(|_| schema_error("storyStates contains an invalid Story id"))?;
    story
        .as_object()
        .ok_or_else(|| schema_error("storyStates entries must be objects"))?;
    authoritative_phase(story)?;
    Ok(())
}

fn story_object_mut<'a>(
    state: &'a mut Value,
    story_id: &str,
    location: StoryLocation,
) -> RuntimeResult<&'a mut Map<String, Value>> {
    let story = match location {
        StoryLocation::Root => state
            .get_mut("storyStates")
            .and_then(Value::as_object_mut)
            .and_then(|stories| stories.get_mut(story_id)),
        StoryLocation::DrState => state
            .get_mut("drState")
            .and_then(|dr| dr.get_mut("storyStates"))
            .and_then(Value::as_object_mut)
            .and_then(|stories| stories.get_mut(story_id)),
        StoryLocation::DrStates(dr_id) => state
            .get_mut("drStates")
            .and_then(Value::as_object_mut)
            .and_then(|states| states.get_mut(&dr_id))
            .and_then(|dr| dr.get_mut("storyStates"))
            .and_then(Value::as_object_mut)
            .and_then(|stories| stories.get_mut(story_id)),
    };
    story
        .and_then(Value::as_object_mut)
        .ok_or_else(|| schema_error("Story state must be an object"))
}

fn validate_prd_status(state: &Value) -> RuntimeResult<()> {
    let Some(status) = state.get("prdStatus") else {
        return Ok(());
    };
    let status = status
        .as_str()
        .ok_or_else(|| schema_error("prdStatus must be a string"))?;
    match status {
        "in_progress" | "prd_complete_pending_user" => Ok(()),
        "awaiting_compact" | "compacted" | "prd_aborted" => Err(RuntimeError::new(
            StableErrorCode::GateBlocked,
            "PRD status cannot transition to awaiting_compact",
        )
        .with_remediation("start from in_progress or prd_complete_pending_user")),
        _ => Err(schema_error("prdStatus is invalid")),
    }
}

fn project_story_summaries(state: &Value) -> RuntimeResult<Vec<StorySummary>> {
    merged_story_views(state)?
        .into_iter()
        .map(|(id, story)| project_story_summary(&id, story.value))
        .collect()
}

fn project_story_summary(story_id: &str, value: &Value) -> RuntimeResult<StorySummary> {
    let phase = authoritative_phase(value)?;
    let current_phase_present = value.get("currentPhase").is_some();
    let current_step = value
        .get("currentStep")
        .map(|step| {
            step.as_str()
                .ok_or_else(|| schema_error("Story currentStep must be a string"))
        })
        .transpose()?;
    let (pending_outputs, pending_outputs_present) = story_pending_outputs(value)?;
    let coding_round = value
        .get("codingRound")
        .map(coding_round_number)
        .transpose()?;
    let terminal_projection_valid = phase != ProcessPhase::Completed
        || (current_phase_present
            && current_step == Some("completed")
            && pending_outputs_present
            && pending_outputs == 0
            && coding_round.is_some_and(|round| round >= 1));
    Ok(StorySummary {
        story_id: StoryId::new(story_id)
            .map_err(|_| schema_error("storyStates contains an invalid Story id"))?,
        phase,
        current_step: ReasonCode::new(current_step.unwrap_or("story.step"))
            .map_err(|_| schema_error("Story currentStep is invalid"))?,
        pending_outputs,
        coding_round: if terminal_projection_valid {
            coding_round.unwrap_or(0)
        } else {
            0
        },
        registered: true,
    })
}

fn story_pending_outputs(value: &Value) -> RuntimeResult<(u16, bool)> {
    let Some(pending) = value.get("pendingOutputs") else {
        return Ok((0, false));
    };
    let count = match pending {
        Value::Object(values) => values.len() as u64,
        Value::Array(values) => values.len() as u64,
        value if value.as_u64().is_some() => value.as_u64().unwrap_or(0),
        _ => {
            return Err(schema_error(
                "Story pendingOutputs must be an object, array, or non-negative integer",
            ));
        }
    };
    Ok((
        u16::try_from(count).map_err(|_| schema_error("Story pendingOutputs exceeds u16"))?,
        true,
    ))
}

fn coding_round_number(value: &Value) -> RuntimeResult<u32> {
    let round = if let Some(round) = value.as_u64() {
        round
    } else if let Some(round) = value.as_str() {
        let digits = round
            .strip_prefix('r')
            .filter(|digits| !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
            .ok_or_else(|| schema_error("codingRound must be a non-negative integer or rN"))?;
        digits
            .parse::<u64>()
            .map_err(|_| schema_error("codingRound exceeds its supported range"))?
    } else {
        return Err(schema_error(
            "codingRound must be a non-negative integer or rN",
        ));
    };
    u32::try_from(round).map_err(|_| schema_error("codingRound exceeds u32"))
}

fn project_prd_summary(
    state: &Value,
    work_item_id: &str,
    stories: &[StorySummary],
) -> RuntimeResult<PrdSummary> {
    let completion = state
        .get("prdCompletion")
        .and_then(Value::as_object)
        .ok_or_else(|| schema_error("PRD completion projection is missing"))?;
    let registered_story_ids = stories
        .iter()
        .map(|story| story.story_id.clone())
        .collect::<Vec<_>>();
    let completed_story_ids = stories
        .iter()
        .filter(|story| story.is_complete())
        .map(|story| story.story_id.clone())
        .collect();
    Ok(PrdSummary {
        prd_id: PrdId::new(work_item_id)
            .map_err(|_| schema_error("workItemId is not a valid PRD id"))?,
        registered_story_ids,
        completed_story_ids,
        dependencies_satisfied: required_bool(completion, "dependenciesSatisfied")?,
        residual_risks_cleared: required_bool(completion, "residualRisksCleared")?,
        gates_passed: required_bool(completion, "gatesPassed")?,
        review_passed: required_bool(completion, "reviewPassed")?,
    })
}

fn project_evidence_refs(state: &Value) -> RuntimeResult<Vec<EvidenceRef>> {
    let Some(evidence) = state.get("evidenceRefs") else {
        return Ok(Vec::new());
    };
    evidence
        .as_array()
        .ok_or_else(|| schema_error("evidenceRefs must be an array"))?
        .iter()
        .map(|item| {
            Ok(EvidenceRef::new(
                EvidenceId::new(required_str(item, "evidenceId")?)
                    .map_err(|_| schema_error("evidenceId is invalid"))?,
                VerificationId::new(required_str(item, "verificationId")?)
                    .map_err(|_| schema_error("verificationId is invalid"))?,
                ProjectRelativePath::new(required_str(item, "path")?)
                    .map_err(|_| schema_error("evidence path is invalid"))?,
                EvidenceDigest::from_str(required_str(item, "digest")?)
                    .map_err(|_| schema_error("evidence digest is invalid"))?,
                required_u64(item, "byteLength")?,
            ))
        })
        .collect()
}

fn project_file_locks(state: &Value) -> RuntimeResult<Vec<FileLockSnapshot>> {
    let Some(locks) = state.get("fileLocks") else {
        return Ok(Vec::new());
    };
    let locks = locks
        .as_array()
        .ok_or_else(|| schema_error("fileLocks must be an array"))?;
    locks
        .iter()
        .map(|lock| {
            Ok(FileLockSnapshot {
                path: ProjectRelativePath::new(required_str(lock, "path")?)
                    .map_err(|_| schema_error("file lock path is invalid"))?,
                owner_session_id: SessionId::from_str(required_str(lock, "ownerSessionId")?)
                    .map_err(|_| schema_error("file lock ownerSessionId is invalid"))?,
                expires_at_unix_ms: required_u64(lock, "expiresAtUnixMs")?,
                metadata_valid: lock
                    .get("metadataValid")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            })
        })
        .collect()
}

fn parse_phase(value: &str) -> RuntimeResult<ProcessPhase> {
    match normalize(value).as_str() {
        "initialized" => Ok(ProcessPhase::Initialized),
        "routeselected" => Ok(ProcessPhase::RouteSelected),
        "requirementanalyzed" => Ok(ProcessPhase::RequirementAnalyzed),
        "drgenerated" => Ok(ProcessPhase::DrGenerated),
        "storygenerated" => Ok(ProcessPhase::StoryGenerated),
        "testcasegenerated" => Ok(ProcessPhase::TestcaseGenerated),
        "codingprocess" => Ok(ProcessPhase::CodingProcess),
        "coding" => Ok(ProcessPhase::Coding),
        "testrunning" => Ok(ProcessPhase::TestRunning),
        "codereviewed" => Ok(ProcessPhase::CodeReviewed),
        "completed" => Ok(ProcessPhase::Completed),
        "paused" => Ok(ProcessPhase::Paused),
        _ => Err(schema_error("lifecycle phase is invalid")),
    }
}

fn authoritative_phase(value: &Value) -> RuntimeResult<ProcessPhase> {
    let phase = parse_phase(required_str(value, "phase")?)?;
    if let Some(current_phase) = value.get("currentPhase") {
        let current_phase = current_phase
            .as_str()
            .ok_or_else(|| schema_error("currentPhase must be a string"))?;
        if parse_phase(current_phase)? != phase {
            return Err(schema_error("currentPhase must match authoritative phase"));
        }
    }
    Ok(phase)
}

fn authoritative_paused_from(
    value: &Value,
    phase: ProcessPhase,
) -> RuntimeResult<Option<ProcessPhase>> {
    let exact = optional_phase(value, "pausedFromPhase")?;
    let legacy = optional_phase(value, "pausedFrom")?;
    if exact.is_some() && legacy.is_some() && exact != legacy {
        return Err(schema_error(
            "pausedFrom must match authoritative pausedFromPhase",
        ));
    }
    if phase == ProcessPhase::Paused {
        return exact
            .map(Some)
            .ok_or_else(|| schema_error("paused lifecycle state is missing pausedFromPhase"));
    }
    Ok(None)
}

fn optional_phase(value: &Value, field: &str) -> RuntimeResult<Option<ProcessPhase>> {
    value
        .get(field)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| schema_error(&format!("{field} must be a string")))
                .and_then(parse_phase)
        })
        .transpose()
}

fn parse_scale(value: &str) -> RuntimeResult<WorkScale> {
    match normalize(value).as_str() {
        "large" | "大" => Ok(WorkScale::Large),
        "medium" | "中" => Ok(WorkScale::Medium),
        "small" | "小" => Ok(WorkScale::Small),
        "micro" | "微" => Ok(WorkScale::Micro),
        _ => Err(schema_error("work scale is invalid")),
    }
}

fn parse_design_route(value: &str) -> RuntimeResult<DesignRoute> {
    match normalize(value).as_str() {
        "dr" => Ok(DesignRoute::Dr),
        "story" => Ok(DesignRoute::Story),
        "codingplan" => Ok(DesignRoute::CodingPlan),
        _ => Err(schema_error("design route is invalid")),
    }
}

fn normalize(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|character| !matches!(character, '-' | '_' | ' '))
        .flat_map(char::to_lowercase)
        .collect()
}

const fn phase_wire(phase: ProcessPhase) -> &'static str {
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

fn required_str<'a>(value: &'a Value, field: &str) -> RuntimeResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| schema_error(&format!("{field} must be a string")))
}

fn required_u64(value: &Value, field: &str) -> RuntimeResult<u64> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| schema_error(&format!("{field} must be an unsigned integer")))
}

fn required_bool(value: &serde_json::Map<String, Value>, field: &str) -> RuntimeResult<bool> {
    value
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| schema_error(&format!("{field} must be a boolean")))
}

fn schema_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::OperationSchemaInvalid, message)
}
