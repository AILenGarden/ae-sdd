//! Daemon-session execution supervision for the Hook fast path.
//!
//! After a successful `execution.resume`, the capsule digest and the
//! supervisor checkpoint facts are bound to the authenticated session
//! ([`ExecutionSessionBinding`]).  PreTool events reported through
//! `hostPayload.executionEvent` are then adjudicated by the pure
//! [`ExecutionHookGuard`], and PostTool appends one bounded
//! `execution.tool` durable event — classification, byte counts and digests
//! only, never the tool output body.
//!
//! The fast path stays bounded: it touches only the session checkpoint and
//! in-memory state — no project file reads, no Gate evaluation and no
//! business-authority calls.  Hosts that omit `executionEvent`, and sessions
//! without a binding, are recorded as `unclassified` shadow and are never
//! blocked (rollout stage: shadow records decisions; only stale authority is
//! enforced elsewhere, on the authoritative operation path).

use std::str::FromStr;

use ae_sdd_contracts::execution_runtime::ExecutionCapsuleV1;
use ae_sdd_policy::{
    ExecutionHookDenialReason, ExecutionHookGuard, ExecutionHookGuardInput, ExecutionHookToolClass,
    ExecutionHookVerdict,
};

use super::*;
use crate::{ExecutionHookDirective, ExecutionHookDirectiveDecision, ExecutionHookEvent};

/// Maximum accepted length of the `executionEvent.class` field.
const MAX_CLASS_BYTES: usize = 32;
/// Maximum accepted length of any digest-typed `executionEvent` field.
const MAX_DIGEST_FIELD_BYTES: usize = 128;
/// Maximum accepted length of the `executionEvent.path` field.
const MAX_PATH_FIELD_BYTES: usize = 512;

/// Execution authority snapshot bound to one authenticated session after a
/// successful `execution.resume`.
///
/// The binding is rebuildable runtime metadata: it never outranks the
/// project authority, it is dropped on daemon recovery (a fresh resume
/// rebinds), and it carries only what the Hook fast path may consult — the
/// capsule digest, the queue cursor, the frozen retained-output budget and
/// the authority-reported focused-GREEN fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionSessionBinding {
    work_item_id: String,
    capsule_digest: String,
    context_revision: u64,
    active_ordinal: u32,
    queue_digest: String,
    max_tool_output_bytes: u32,
    focused_green: bool,
}

impl ExecutionSessionBinding {
    fn new(
        work_item_id: &str,
        capsule_digest: &str,
        context_revision: u64,
        active_ordinal: u32,
        queue_digest: String,
        max_tool_output_bytes: u32,
        focused_green: bool,
    ) -> Self {
        Self {
            work_item_id: work_item_id.to_owned(),
            capsule_digest: capsule_digest.to_owned(),
            context_revision,
            active_ordinal,
            queue_digest,
            max_tool_output_bytes,
            focused_green,
        }
    }

    /// Returns the Work Item this binding supervises.
    pub fn work_item_id(&self) -> &str {
        &self.work_item_id
    }

    /// Returns the `sha256:`-prefixed capsule digest the session resumed.
    pub fn capsule_digest(&self) -> &str {
        &self.capsule_digest
    }

    /// Returns the context revision the capsule was resumed at.
    pub const fn context_revision(&self) -> u64 {
        self.context_revision
    }

    /// Returns the active slice ordinal in the approved queue.
    pub const fn active_ordinal(&self) -> u32 {
        self.active_ordinal
    }

    /// Returns the approved slice queue digest.
    pub fn queue_digest(&self) -> &str {
        &self.queue_digest
    }

    /// Returns the frozen single-call retained-output budget in bytes.
    pub const fn max_tool_output_bytes(&self) -> u32 {
        self.max_tool_output_bytes
    }

    /// Returns the authority-reported focused-GREEN fact for the active slice.
    pub const fn focused_green(&self) -> bool {
        self.focused_green
    }

    const fn set_focused_green(&mut self, green: bool) {
        self.focused_green = green;
    }
}

/// Bounded, typed execution tool event decoded from
/// `hostPayload.executionEvent`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClassifiedExecutionEvent {
    class: ExecutionHookToolClass,
    outcome: Option<bool>,
    path: Option<String>,
    content_digest: Option<String>,
    query_digest: Option<String>,
    result_digest: Option<String>,
    event_digest: Option<String>,
    output_bytes: Option<u32>,
    output_digest: Option<String>,
    start_line: Option<u32>,
    end_line: Option<u32>,
}

impl ClassifiedExecutionEvent {
    const fn class(&self) -> ExecutionHookToolClass {
        self.class
    }

    const fn outcome(&self) -> Option<bool> {
        self.outcome
    }
}

/// Outcome of one execution Hook guard evaluation.
pub(super) struct ExecutionHookGuardOutcome {
    verdict: ExecutionHookVerdict,
    directive: Option<ExecutionHookDirective>,
    record: bool,
    focused_fact: Option<bool>,
}

impl ExecutionHookGuardOutcome {
    /// Verdict the pure guard produced for the event.
    pub(super) const fn verdict(&self) -> ExecutionHookVerdict {
        self.verdict
    }

    /// Directive attached to the Hook result, when the event is supervised.
    pub(super) const fn directive(&self) -> Option<&ExecutionHookDirective> {
        self.directive.as_ref()
    }

    /// Whether a bounded `execution.tool` event must be appended.
    pub(super) const fn record(&self) -> bool {
        self.record
    }

    /// Focused-GREEN fact reported by a PostTool event, when present.
    pub(super) const fn focused_fact(&self) -> Option<bool> {
        self.focused_fact
    }
}

/// Strict-decodes `hostPayload.executionEvent`; malformed input fails closed
/// with a schema error and a missing field yields `None` (shadow).
pub(super) fn decode_execution_event(
    payload: &HookPayload,
) -> RuntimeResult<Option<ClassifiedExecutionEvent>> {
    payload
        .host_payload
        .get("executionEvent")
        .map(|value| {
            let wire: ExecutionHookEvent = serde_json::from_value(value.clone())
                .map_err(|_| schema_error("hostPayload.executionEvent is malformed"))?;
            classify_execution_event(&wire)
        })
        .transpose()
}

fn classify_execution_event(wire: &ExecutionHookEvent) -> RuntimeResult<ClassifiedExecutionEvent> {
    let class = bounded(wire.class.as_str(), MAX_CLASS_BYTES, "class")?;
    let class = ExecutionHookToolClass::from_wire_name(class).ok_or_else(|| {
        schema_error("hostPayload.executionEvent.class is not a recognized execution tool class")
    })?;
    let outcome = match wire.outcome.as_deref() {
        None => None,
        Some("pass") => Some(true),
        Some("fail") => Some(false),
        Some(_) => {
            return Err(schema_error(
                "hostPayload.executionEvent.outcome must be `pass` or `fail`",
            ));
        }
    };
    Ok(ClassifiedExecutionEvent {
        class,
        outcome,
        path: bounded_opt(wire.path.as_deref(), MAX_PATH_FIELD_BYTES, "path")?,
        content_digest: bounded_opt(
            wire.content_digest.as_deref(),
            MAX_DIGEST_FIELD_BYTES,
            "contentDigest",
        )?,
        query_digest: bounded_opt(
            wire.query_digest.as_deref(),
            MAX_DIGEST_FIELD_BYTES,
            "queryDigest",
        )?,
        result_digest: bounded_opt(
            wire.result_digest.as_deref(),
            MAX_DIGEST_FIELD_BYTES,
            "resultDigest",
        )?,
        event_digest: bounded_opt(
            wire.event_digest.as_deref(),
            MAX_DIGEST_FIELD_BYTES,
            "eventDigest",
        )?,
        output_bytes: wire.output_bytes,
        output_digest: bounded_opt(
            wire.output_digest.as_deref(),
            MAX_DIGEST_FIELD_BYTES,
            "outputDigest",
        )?,
        start_line: wire.start_line,
        end_line: wire.end_line,
    })
}

fn bounded<'a>(value: &'a str, maximum: usize, field: &str) -> RuntimeResult<&'a str> {
    if value.len() > maximum {
        return Err(RuntimeError::new(
            StableErrorCode::OperationSchemaInvalid,
            format!("hostPayload.executionEvent.{field} exceeds the bounded length"),
        ));
    }
    Ok(value)
}

fn bounded_opt(value: Option<&str>, maximum: usize, field: &str) -> RuntimeResult<Option<String>> {
    value
        .map(|value| bounded(value, maximum, field).map(str::to_owned))
        .transpose()
}

/// Returns true for a `sha256:`-prefixed lowercase hex digest.
fn is_prefixed_capsule_digest(value: &str) -> bool {
    let Some(hex_part) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex_part.len() == 64
        && hex_part
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

const fn stable_reason_code(reason: ExecutionHookDenialReason) -> StableErrorCode {
    match reason {
        ExecutionHookDenialReason::BroadTestBeforeFocusedGreen => {
            StableErrorCode::ExecutionProgressRequired
        }
    }
}

fn directive_for(verdict: ExecutionHookVerdict) -> Option<ExecutionHookDirective> {
    match verdict {
        ExecutionHookVerdict::Allow {
            output_budget_bytes,
        } => Some(ExecutionHookDirective {
            decision: ExecutionHookDirectiveDecision::Allow,
            reason_code: None,
            output_budget_bytes: Some(output_budget_bytes),
            retry_after_ms: None,
            cached_read_ref: None,
        }),
        ExecutionHookVerdict::RequireProgress { reason } => Some(ExecutionHookDirective {
            decision: ExecutionHookDirectiveDecision::RequireProgress,
            reason_code: Some(stable_reason_code(reason).as_str().to_owned()),
            output_budget_bytes: None,
            retry_after_ms: None,
            cached_read_ref: None,
        }),
        ExecutionHookVerdict::Unclassified => None,
    }
}

fn execution_event_payload(
    verdict: ExecutionHookVerdict,
    event: Option<&ClassifiedExecutionEvent>,
) -> Value {
    let decision = match verdict {
        ExecutionHookVerdict::Unclassified => "unclassified",
        ExecutionHookVerdict::Allow { .. } => "allow",
        ExecutionHookVerdict::RequireProgress { .. } => "require-progress",
    };
    let mut payload = json!({
        "schemaVersion": "execution-tool/v1",
        "class": event.map_or("unclassified", |event| event.class().wire_name()),
        "decision": decision,
    });
    if let ExecutionHookVerdict::RequireProgress { reason } = verdict {
        payload["reasonCode"] = Value::String(stable_reason_code(reason).as_str().to_owned());
    }
    if let Some(event) = event {
        if let Some(bytes) = event.output_bytes {
            payload["outputBytes"] = Value::from(bytes);
        }
        if let Some(path) = event.path.as_deref() {
            payload["path"] = Value::String(path.to_owned());
        }
        if let Some(digest) = event.content_digest.as_deref() {
            payload["contentDigest"] = Value::String(digest.to_owned());
        }
        if let Some(digest) = event.query_digest.as_deref() {
            payload["queryDigest"] = Value::String(digest.to_owned());
        }
        if let Some(digest) = event.result_digest.as_deref() {
            payload["resultDigest"] = Value::String(digest.to_owned());
        }
        if let Some(digest) = event.event_digest.as_deref() {
            payload["eventDigest"] = Value::String(digest.to_owned());
        }
        if let Some(digest) = event.output_digest.as_deref() {
            payload["outputDigest"] = Value::String(digest.to_owned());
        }
        if let Some(outcome) = event.outcome {
            payload["outcome"] = Value::String(if outcome { "pass" } else { "fail" }.to_owned());
        }
        if let Some(start_line) = event.start_line {
            payload["startLine"] = Value::from(start_line);
        }
        if let Some(end_line) = event.end_line {
            payload["endLine"] = Value::from(end_line);
        }
    }
    payload
}

impl RuntimeService {
    /// Binds the capsule digest and supervisor checkpoint facts to the
    /// authenticated session after a successful `execution.resume`.
    ///
    /// The binding is best-effort and additive: a response that is not a
    /// well-formed resume projection leaves the previous binding untouched
    /// (a `no-change` projection keeps the existing digest binding, since
    /// the business authority already proved it identical).  Binding never
    /// fails the underlying operation.
    pub(super) fn bind_execution_resume(
        &self,
        params: &RequestParams<Value>,
        value: &Value,
    ) -> RuntimeResult<()> {
        let is_resume = params
            .payload
            .get("operation")
            .and_then(Value::as_str)
            .and_then(|name| ae_sdd_operations::OperationName::from_str(name).ok())
            == Some(ae_sdd_operations::OperationName::ExecutionResume);
        if !is_resume {
            return Ok(());
        }
        let (Some(session_id), Some(work_item_id)) =
            (params.session_id.as_deref(), params.work_item_id.as_deref())
        else {
            return Ok(());
        };
        let data = value.get("data").cloned().unwrap_or(Value::Null);
        let Some(capsule_digest) = data.get("capsuleDigest").and_then(Value::as_str) else {
            return Ok(());
        };
        if !is_prefixed_capsule_digest(capsule_digest) {
            return Ok(());
        }
        let Some(context_revision) = data.get("contextRevision").and_then(Value::as_u64) else {
            return Ok(());
        };
        let capsule = match data.get("capsule") {
            None | Some(Value::Null) => None,
            Some(value) => serde_json::from_value::<ExecutionCapsuleV1>(value.clone()).ok(),
        };
        let Some(capsule) = capsule else {
            return Ok(());
        };
        let mut state = self.lock_state()?;
        if !state.sessions.contains_key(session_id) {
            return Ok(());
        }
        let focused_green = state
            .execution_bindings
            .get(session_id)
            .filter(|existing| existing.capsule_digest() == capsule_digest)
            .is_some_and(ExecutionSessionBinding::focused_green);
        if !state.execution_bindings.contains_key(session_id)
            && state.execution_bindings.len() >= self.config.max_sessions
        {
            return Ok(());
        }
        state.execution_bindings.insert(
            session_id.to_owned(),
            ExecutionSessionBinding::new(
                work_item_id,
                capsule_digest,
                context_revision,
                capsule.active_slice().ordinal(),
                capsule.queue().queue_digest().to_hex(),
                capsule.budgets().max_tool_output_bytes(),
                focused_green,
            ),
        );
        Ok(())
    }

    /// Evaluates the pure execution Hook guard for one tool event.
    ///
    /// Only PreTool and PostTool events are adjudicated; UserPrompt and Stop
    /// never consult the execution supervisor.  The evaluation is read-only
    /// against the session binding.
    pub(super) fn execution_hook_guard(
        &self,
        session_id: &str,
        work_item_id: &str,
        method: RpcMethod,
        event: Option<&ClassifiedExecutionEvent>,
    ) -> RuntimeResult<ExecutionHookGuardOutcome> {
        if !matches!(method, RpcMethod::HookPreTool | RpcMethod::HookPostTool) {
            return Ok(ExecutionHookGuardOutcome {
                verdict: ExecutionHookVerdict::Unclassified,
                directive: None,
                record: false,
                focused_fact: None,
            });
        }
        let state = self.lock_state()?;
        let binding = state
            .execution_bindings
            .get(session_id)
            .filter(|binding| binding.work_item_id() == work_item_id);
        let input = ExecutionHookGuardInput::new(
            binding.is_some(),
            binding.is_some_and(ExecutionSessionBinding::focused_green),
            event.map(ClassifiedExecutionEvent::class),
            binding.map_or(0, ExecutionSessionBinding::max_tool_output_bytes),
        );
        let verdict = ExecutionHookGuard::decide(&input);
        let directive = if binding.is_some() && event.is_some() {
            directive_for(verdict)
        } else {
            None
        };
        let focused_fact = if method == RpcMethod::HookPostTool
            && binding.is_some()
            && event.is_some_and(|event| event.class() == ExecutionHookToolClass::FocusedTest)
        {
            event.and_then(ClassifiedExecutionEvent::outcome)
        } else {
            None
        };
        Ok(ExecutionHookGuardOutcome {
            verdict,
            directive,
            record: binding.is_some(),
            focused_fact,
        })
    }

    /// Appends the bounded `execution.tool` event and applies PostTool facts
    /// to the session binding.  Runs after the Hook receipt committed so a
    /// replayed Hook event never double-records.
    pub(super) fn record_execution_hook_event(
        &self,
        identity: &TrustedSession,
        work_item_id: &str,
        outcome: &ExecutionHookGuardOutcome,
        event: Option<&ClassifiedExecutionEvent>,
    ) -> RuntimeResult<()> {
        if !outcome.record() {
            return Ok(());
        }
        if let Some(green) = outcome.focused_fact() {
            let mut state = self.lock_state()?;
            if let Some(binding) = state
                .execution_bindings
                .get_mut(identity.session_id.as_str())
            {
                binding.set_focused_green(green);
            }
            drop(state);
        }
        self.append_runtime_event(
            "execution.tool",
            execution_event_payload(outcome.verdict(), event),
            Some(identity.workspace_id.clone()),
            Some(identity.session_id.clone()),
            Some(work_item_id.to_owned()),
        )?;
        Ok(())
    }
}
