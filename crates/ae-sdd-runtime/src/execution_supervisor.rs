//! Daemon-session execution supervision for the Hook fast path.
//!
//! After a successful `execution.resume`, the capsule digest and a restartable
//! [`ExecutionSupervisorCheckpointV1`] are bound to the authenticated session
//! ([`ExecutionSessionBinding`]).  PreTool events reported through
//! `hostPayload.executionEvent` are adjudicated by the pure
//! [`ExecutionSupervisor`] reducer — the single owner of slice-progress
//! semantics (investigation batches, progress events, output budgets,
//! broad-test timing) — and PostTool commits the resulting checkpoint back to
//! the binding.  The `ae-sdd-policy` [`ExecutionHookGuard`] keeps its frozen
//! broad-before-green boundary as a last-resort net only; it never
//! re-implements batch counting.  Every supervised event appends one bounded
//! `execution.tool` durable event — classification, byte counts and digests
//! only, never the tool output body.
//!
//! The fast path stays bounded: it touches only the session checkpoint and
//! in-memory state — no project file reads, no Gate evaluation and no
//! business-authority calls.  Hosts that omit `executionEvent`, and sessions
//! without a binding, are recorded as `unclassified` shadow and are never
//! blocked (rollout stage: shadow records decisions; only stale authority is
//! enforced elsewhere, on the authoritative operation path).
//!
//! Commit discipline: a PreTool call previews the reducer decision without
//! mutating the binding (the preview checkpoint is discarded), because only
//! an executed tool call may consume the investigation budget; a denied
//! PreTool produces no PostTool and therefore never consumes budget.  The
//! PostTool call re-decides with the complete event (outcome, byte counts,
//! digests) and commits the new checkpoint, so every executed call is
//! accounted exactly once.

use std::str::FromStr;

use ae_sdd_contracts::execution_runtime::{ExecutionCapsuleV1, ExecutionSliceStatus};
use ae_sdd_domain::{ArtifactDigest, ProjectRelativePath};
use ae_sdd_execution::{
    ExecutionDecisionV1, ExecutionSupervisor, ExecutionSupervisorAfterImageV1,
    ExecutionSupervisorCheckpointV1, ExecutionToolEventV1, ExecutionToolOutputV1,
    FocusedTestOutcomeV1, FocusedTestStateV1,
};
use ae_sdd_policy::{
    ExecutionHookDenialReason, ExecutionHookGuard, ExecutionHookGuardInput, ExecutionHookToolClass,
    ExecutionHookVerdict,
};

use super::*;
use crate::{
    ExecutionHookDirective, ExecutionHookDirectiveDecision, ExecutionHookEvent,
    PreparedExecutionHookV1,
};

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
/// capsule digest, the queue cursor and the restartable supervisor
/// checkpoint (slice status, focused verification state, frozen budgets and
/// investigation accounting).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionSessionBinding {
    work_item_id: String,
    capsule_digest: String,
    context_revision: u64,
    active_ordinal: u32,
    queue_digest: String,
    checkpoint: ExecutionSupervisorCheckpointV1,
}

impl ExecutionSessionBinding {
    fn new(
        work_item_id: &str,
        capsule_digest: &str,
        context_revision: u64,
        active_ordinal: u32,
        queue_digest: String,
        checkpoint: ExecutionSupervisorCheckpointV1,
    ) -> Self {
        Self {
            work_item_id: work_item_id.to_owned(),
            capsule_digest: capsule_digest.to_owned(),
            context_revision,
            active_ordinal,
            queue_digest,
            checkpoint,
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
        self.checkpoint.budgets().max_tool_output_bytes()
    }

    /// Returns whether the focused verification is green for the active slice.
    pub fn focused_green(&self) -> bool {
        self.checkpoint.focused_test() == FocusedTestStateV1::Green
    }

    /// Returns the restartable supervisor checkpoint.
    pub const fn checkpoint(&self) -> &ExecutionSupervisorCheckpointV1 {
        &self.checkpoint
    }

    /// Commits the checkpoint produced by one PostTool supervisor decision.
    fn commit_checkpoint(&mut self, checkpoint: ExecutionSupervisorCheckpointV1) {
        self.checkpoint = checkpoint;
    }
}

/// Bounded, typed execution tool event decoded from
/// `hostPayload.executionEvent`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClassifiedExecutionEvent {
    class: ExecutionHookToolClass,
    outcome: Option<bool>,
    path: Option<ProjectRelativePath>,
    content_digest: Option<ArtifactDigest>,
    query_digest: Option<ArtifactDigest>,
    result_digest: Option<ArtifactDigest>,
    event_digest: Option<ArtifactDigest>,
    output_bytes: Option<u32>,
    output_digest: Option<ArtifactDigest>,
    start_line: Option<u32>,
    end_line: Option<u32>,
}

impl ClassifiedExecutionEvent {
    pub(crate) const fn class(&self) -> ExecutionHookToolClass {
        self.class
    }

    const fn outcome(&self) -> Option<bool> {
        self.outcome
    }
}

/// Hook-level disposition derived from one supervisor decision.
///
/// `RequireProgress` and `Deny` both deny an engaged PreTool; they differ
/// only in the stable reason code attached to the directive and the bounded
/// event (`EXECUTION_PROGRESS_REQUIRED` vs the budget/slice codes).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExecutionHookDisposition {
    /// No classified event or no binding; shadow records only, never blocks.
    Unclassified,
    /// The event is admissible.
    Allow,
    /// The event is rejected until machine-verified progress is made.
    RequireProgress,
    /// The event is rejected because a budget is exhausted or the slice can
    /// no longer change.
    Deny,
}

impl ExecutionHookDisposition {
    /// Stable label recorded in the bounded `execution.tool` event.
    const fn event_label(self) -> &'static str {
        match self {
            Self::Unclassified => "unclassified",
            Self::Allow => "allow",
            Self::RequireProgress => "require-progress",
            Self::Deny => "deny",
        }
    }
}

/// Outcome of one execution Hook guard evaluation.
pub(super) struct ExecutionHookGuardOutcome {
    disposition: ExecutionHookDisposition,
    directive: Option<ExecutionHookDirective>,
    record: bool,
    reason_code: Option<StableErrorCode>,
    next_checkpoint: Option<ExecutionSupervisorCheckpointV1>,
}

impl ExecutionHookGuardOutcome {
    /// Disposition the supervisor produced for the event.
    pub(super) const fn disposition(&self) -> ExecutionHookDisposition {
        self.disposition
    }

    /// Directive attached to the Hook result, when the event is supervised.
    pub(super) const fn directive(&self) -> Option<&ExecutionHookDirective> {
        self.directive.as_ref()
    }

    /// Whether a bounded `execution.tool` event must be appended.
    pub(super) const fn record(&self) -> bool {
        self.record
    }

    /// Stable reason code for rejected events, when present.
    pub(super) const fn reason_code(&self) -> Option<StableErrorCode> {
        self.reason_code
    }

    /// Checkpoint committed back to the session binding after PostTool.
    pub(super) const fn next_checkpoint(&self) -> Option<&ExecutionSupervisorCheckpointV1> {
        self.next_checkpoint.as_ref()
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
        path: parse_path_opt(wire.path.as_deref())?,
        content_digest: parse_digest_opt(wire.content_digest.as_deref(), "contentDigest")?,
        query_digest: parse_digest_opt(wire.query_digest.as_deref(), "queryDigest")?,
        result_digest: parse_digest_opt(wire.result_digest.as_deref(), "resultDigest")?,
        event_digest: parse_digest_opt(wire.event_digest.as_deref(), "eventDigest")?,
        output_bytes: wire.output_bytes,
        output_digest: parse_digest_opt(wire.output_digest.as_deref(), "outputDigest")?,
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

fn parse_path_opt(value: Option<&str>) -> RuntimeResult<Option<ProjectRelativePath>> {
    value
        .map(|value| {
            bounded(value, MAX_PATH_FIELD_BYTES, "path")?;
            ProjectRelativePath::new(value).map_err(|_| {
                schema_error("hostPayload.executionEvent.path must be project-relative")
            })
        })
        .transpose()
}

fn parse_digest_opt(value: Option<&str>, field: &str) -> RuntimeResult<Option<ArtifactDigest>> {
    value
        .map(|value| {
            bounded(value, MAX_DIGEST_FIELD_BYTES, field)?;
            ArtifactDigest::from_str(value).map_err(|_| {
                RuntimeError::new(
                    StableErrorCode::OperationSchemaInvalid,
                    format!("hostPayload.executionEvent.{field} must be lowercase SHA-256 hex"),
                )
            })
        })
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

/// Builds the directive for one supervised event from its disposition.
///
/// An `Allow` directive echoes the frozen retained-output budget so the host
/// truncates before evidence is produced (the budget-truncation contract);
/// rejections carry the stable reason code the supervisor reduction produced,
/// and a resource deferral adds the bounded retry hint.
fn directive_for(
    disposition: ExecutionHookDisposition,
    reason_code: Option<StableErrorCode>,
    output_budget_bytes: u32,
    retry_after_ms: Option<u64>,
) -> Option<ExecutionHookDirective> {
    match disposition {
        ExecutionHookDisposition::Allow => Some(ExecutionHookDirective {
            decision: ExecutionHookDirectiveDecision::Allow,
            reason_code: None,
            output_budget_bytes: Some(output_budget_bytes),
            retry_after_ms: None,
            cached_read_ref: None,
        }),
        ExecutionHookDisposition::RequireProgress | ExecutionHookDisposition::Deny => {
            Some(ExecutionHookDirective {
                decision: ExecutionHookDirectiveDecision::RequireProgress,
                reason_code: reason_code.map(|code| code.as_str().to_owned()),
                output_budget_bytes: None,
                retry_after_ms,
                cached_read_ref: None,
            })
        }
        ExecutionHookDisposition::Unclassified => None,
    }
}

/// Maps one classified Hook event onto the bounded supervisor event.
///
/// Events whose class-required facts are absent or unparseable degrade to
/// `Other`: they stay admissible while investigation budget remains and are
/// denied once it is exhausted, but never consume batch accounting they
/// cannot be charged for.  A focused verification without a reported outcome
/// is probed as `fail` — admissibility of a focused run never depends on its
/// outcome, and the PreTool preview checkpoint is discarded; the PostTool
/// re-decision uses the reported outcome.
fn build_tool_event(event: &ClassifiedExecutionEvent) -> ExecutionToolEventV1 {
    let output = ExecutionToolOutputV1 {
        bytes: event.output_bytes.unwrap_or(0),
        digest: event
            .output_digest
            .unwrap_or_else(|| ArtifactDigest::digest(b"ae-sdd/execution-hook/no-output-digest")),
        locator: None,
    };
    match event.class() {
        ExecutionHookToolClass::SourceRead => match (&event.path, event.content_digest) {
            (Some(path), Some(content_digest)) => ExecutionToolEventV1::SourceRead {
                path: path.clone(),
                content_digest,
                start_line: event.start_line,
                end_line: event.end_line,
                output,
            },
            _ => ExecutionToolEventV1::Other { output },
        },
        ExecutionHookToolClass::Search => match event.query_digest {
            Some(query_digest) => ExecutionToolEventV1::Search {
                query_digest,
                output,
            },
            None => ExecutionToolEventV1::Other { output },
        },
        ExecutionHookToolClass::Patch => match event.result_digest {
            Some(result_digest) => ExecutionToolEventV1::Patch {
                result_digest,
                output,
            },
            None => ExecutionToolEventV1::Other { output },
        },
        ExecutionHookToolClass::FocusedTest => {
            let outcome = match event.outcome() {
                Some(true) => FocusedTestOutcomeV1::Pass,
                Some(false) | None => FocusedTestOutcomeV1::Fail,
            };
            ExecutionToolEventV1::FocusedTest { outcome, output }
        }
        ExecutionHookToolClass::BroadTest => ExecutionToolEventV1::BroadTest { output },
        ExecutionHookToolClass::Evidence => match event.event_digest {
            Some(event_digest) => ExecutionToolEventV1::Evidence {
                event_digest,
                output,
            },
            None => ExecutionToolEventV1::Other { output },
        },
        ExecutionHookToolClass::Other => ExecutionToolEventV1::Other { output },
    }
}

fn execution_event_payload(
    disposition: ExecutionHookDisposition,
    reason_code: Option<StableErrorCode>,
    event: Option<&ClassifiedExecutionEvent>,
) -> Value {
    let tool_event = event.map(build_tool_event);
    let mut payload = json!({
        "schemaVersion": "execution-tool/v1",
        "class": tool_event.as_ref().map_or("unclassified", execution_tool_event_class),
        "decision": disposition.event_label(),
    });
    if let Some(code) = reason_code {
        payload["reasonCode"] = Value::String(code.as_str().to_owned());
    }
    if let Some(event) = tool_event.as_ref() {
        append_typed_execution_event(&mut payload, event);
    }
    payload
}

fn execution_tool_event_class(event: &ExecutionToolEventV1) -> &'static str {
    match event {
        ExecutionToolEventV1::SourceRead { .. } => "source-read",
        ExecutionToolEventV1::Search { .. } => "search",
        ExecutionToolEventV1::Patch { .. } => "patch",
        ExecutionToolEventV1::FocusedTest { .. } => "focused-test",
        ExecutionToolEventV1::BroadTest { .. } => "broad-test",
        ExecutionToolEventV1::Evidence { .. } => "evidence",
        ExecutionToolEventV1::Blocker { .. } => "blocker",
        ExecutionToolEventV1::Slice(_) => "slice",
        ExecutionToolEventV1::Other { .. } => "other",
    }
}

fn append_typed_execution_event(payload: &mut Value, event: &ExecutionToolEventV1) {
    let output = match event {
        ExecutionToolEventV1::SourceRead {
            path,
            content_digest,
            start_line,
            end_line,
            output,
        } => {
            payload["path"] = Value::String(path.as_str().to_owned());
            payload["contentDigest"] = Value::String(content_digest.to_string());
            if let Some(start_line) = start_line {
                payload["startLine"] = Value::from(*start_line);
            }
            if let Some(end_line) = end_line {
                payload["endLine"] = Value::from(*end_line);
            }
            Some(output)
        }
        ExecutionToolEventV1::Search {
            query_digest,
            output,
        } => {
            payload["queryDigest"] = Value::String(query_digest.to_string());
            Some(output)
        }
        ExecutionToolEventV1::Patch {
            result_digest,
            output,
        } => {
            payload["resultDigest"] = Value::String(result_digest.to_string());
            Some(output)
        }
        ExecutionToolEventV1::FocusedTest { outcome, output } => {
            payload["outcome"] = Value::String(
                match outcome {
                    FocusedTestOutcomeV1::Pass => "pass",
                    FocusedTestOutcomeV1::Fail => "fail",
                }
                .to_owned(),
            );
            Some(output)
        }
        ExecutionToolEventV1::BroadTest { output } | ExecutionToolEventV1::Other { output } => {
            Some(output)
        }
        ExecutionToolEventV1::Evidence {
            event_digest,
            output,
        } => {
            payload["eventDigest"] = Value::String(event_digest.to_string());
            Some(output)
        }
        ExecutionToolEventV1::Blocker { code, locator } => {
            payload["blockerCode"] = Value::String(code.to_string());
            payload["locatorDigest"] = Value::String(locator.digest().to_string());
            None
        }
        ExecutionToolEventV1::Slice(_) => None,
    };
    if let Some(output) = output {
        payload["outputBytes"] = Value::from(output.bytes);
        payload["outputDigest"] = Value::String(output.digest.to_string());
    }
}

impl RuntimeService {
    /// Returns whether this session/Work Item currently has an execution
    /// binding and therefore requires a secondary execution receipt.
    pub(super) fn execution_hook_requires_record(
        &self,
        session_id: &str,
        work_item_id: Option<&str>,
    ) -> RuntimeResult<bool> {
        let state = self.lock_state()?;
        Ok(work_item_id.is_some_and(|work_item_id| {
            state
                .execution_bindings
                .get(session_id)
                .is_some_and(|binding| binding.work_item_id() == work_item_id)
        }))
    }

    /// Returns whether the atomic execution bundle receipt is durable for one
    /// Hook identity without recomputing a checkpoint transition.
    pub(super) fn execution_hook_receipt_exists(
        &self,
        identity: &TrustedSession,
        hook_event_id: &str,
        request_digest: &str,
    ) -> RuntimeResult<bool> {
        let scope = format!(
            "execution-hook\0{}\0{}",
            identity.workspace_id, identity.session_id
        );
        self.replay_receipt(&scope, hook_event_id, request_digest)
            .map(|receipt| receipt.is_some())
    }

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
        // A rebind to the same capsule keeps the supervised progress made so
        // far; a different capsule starts a fresh checkpoint for its slice.
        // The checkpoint begins at `running`: the resume binds the active
        // slice the FlowRuntime next action told the session to execute.
        let checkpoint = match state
            .execution_bindings
            .get(session_id)
            .filter(|existing| existing.capsule_digest() == capsule_digest)
        {
            Some(existing) => existing.checkpoint().clone(),
            None => ExecutionSupervisorCheckpointV1::new(
                ExecutionSliceStatus::Running,
                *capsule.budgets(),
            ),
        };
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
                checkpoint,
            ),
        );
        Ok(())
    }

    /// Adjudicates one classified tool event with the pure execution
    /// supervisor against the session checkpoint.
    ///
    /// Only PreTool and PostTool events are adjudicated; UserPrompt and Stop
    /// never consult the execution supervisor.  A PreTool call previews the
    /// decision and discards the preview checkpoint (only an executed call
    /// may consume budget); a PostTool call re-decides with the complete
    /// event and carries the new checkpoint for [`Self::record_execution_hook_event`]
    /// to commit.  The `ae-sdd-policy` guard re-checks only its frozen
    /// broad-before-green boundary as a last-resort net — the supervisor
    /// reducer owns every progress and batch rule.
    /// Arbitrates one Hook tool event against the session's execution binding.
    ///
    /// `work_item_id` is absent for a Hook whose session has no Work Item bound
    /// yet. Such an event can match no binding, so it stays unclassified — the
    /// same outcome an unbound shadow session already produced.
    pub(super) fn execution_hook_guard(
        &self,
        session_id: &str,
        work_item_id: Option<&str>,
        method: RpcMethod,
        event: Option<&ClassifiedExecutionEvent>,
    ) -> RuntimeResult<ExecutionHookGuardOutcome> {
        if !matches!(method, RpcMethod::HookPreTool | RpcMethod::HookPostTool) {
            return Ok(ExecutionHookGuardOutcome {
                disposition: ExecutionHookDisposition::Unclassified,
                directive: None,
                record: false,
                reason_code: None,
                next_checkpoint: None,
            });
        }
        let state = self.lock_state()?;
        let binding = work_item_id.and_then(|work_item_id| {
            state
                .execution_bindings
                .get(session_id)
                .filter(|binding| binding.work_item_id() == work_item_id)
        });
        let (Some(binding), Some(event)) = (binding, event) else {
            return Ok(ExecutionHookGuardOutcome {
                disposition: ExecutionHookDisposition::Unclassified,
                directive: None,
                record: binding.is_some(),
                reason_code: None,
                next_checkpoint: None,
            });
        };
        let tool_event = build_tool_event(event);
        let (decision, next_checkpoint) =
            ExecutionSupervisor::decide(binding.checkpoint(), &tool_event);
        let (disposition, reason_code, retry_after_ms) = match &decision {
            ExecutionDecisionV1::Allow(_) => {
                // Last-resort net: the policy guard may still veto a broad
                // verification before the focused GREEN.  The supervisor
                // already denies it, so this can only fire on a reducer
                // regression — the guard never re-implements batch rules.
                let guard_input = ExecutionHookGuardInput::new(
                    true,
                    binding.focused_green(),
                    Some(event.class()),
                    binding.max_tool_output_bytes(),
                );
                match ExecutionHookGuard::decide(&guard_input) {
                    ExecutionHookVerdict::RequireProgress { reason } => (
                        ExecutionHookDisposition::RequireProgress,
                        Some(stable_reason_code(reason)),
                        None,
                    ),
                    _ => (ExecutionHookDisposition::Allow, None, None),
                }
            }
            ExecutionDecisionV1::RequireProgress(error) => (
                ExecutionHookDisposition::RequireProgress,
                Some(error.error_code()),
                None,
            ),
            ExecutionDecisionV1::Deny(error) => (
                ExecutionHookDisposition::Deny,
                Some(error.error_code()),
                None,
            ),
            ExecutionDecisionV1::Defer(deferral) => (
                ExecutionHookDisposition::RequireProgress,
                Some(StableErrorCode::ExecutionResourceBusy),
                Some(deferral.retry_after_ms()),
            ),
        };
        let directive = directive_for(
            disposition,
            reason_code,
            binding.max_tool_output_bytes(),
            retry_after_ms,
        );
        Ok(ExecutionHookGuardOutcome {
            disposition,
            directive,
            record: true,
            reason_code,
            next_checkpoint: (method == RpcMethod::HookPostTool).then_some(next_checkpoint),
        })
    }

    /// Atomically prepares the bounded `execution.tool` event, its receipt,
    /// and the original checkpoint after-image before the main Hook receipt.
    pub(super) fn prepare_execution_hook_event(
        &self,
        identity: &TrustedSession,
        work_item_id: Option<&str>,
        outcome: &ExecutionHookGuardOutcome,
        event: Option<&ClassifiedExecutionEvent>,
        mut prepared: PreparedExecutionHookV1,
    ) -> RuntimeResult<Option<PreparedExecutionHookV1>> {
        if !outcome.record() {
            return Ok(None);
        }
        let scope = format!(
            "execution-hook\0{}\0{}",
            identity.workspace_id, identity.session_id
        );
        let payload = execution_event_payload(outcome.disposition(), outcome.reason_code(), event);
        let payload_bytes = serde_json::to_vec(&payload).map_err(canonical_error)?;
        let event_record = DurableEvent {
            event_store_id: self.persistence.event_store_id()?.to_string(),
            event_seq: 0,
            boot_id: self.boot_id.to_string(),
            kind: "execution.tool".to_owned(),
            workspace_id: Some(identity.workspace_id.clone()),
            session_id: Some(identity.session_id.clone()),
            work_item_id: work_item_id.map(str::to_owned),
            payload,
            payload_digest: hex::encode(Sha256::digest(payload_bytes)),
        };
        let receipt = IdempotencyReceipt {
            scope,
            key: prepared.hook_event_id.clone(),
            request_digest: prepared.request_digest.clone(),
            response_json: "{\"recorded\":true}".to_owned(),
            event_seq: 0,
        };
        prepared.checkpoint_after_image = outcome
            .next_checkpoint()
            .map(ExecutionSupervisorAfterImageV1::from);
        let (_, _, prepared) =
            self.persistence
                .commit_prepared_execution_hook(event_record, receipt, prepared)?;
        Ok(Some(prepared))
    }

    /// Loads the one per-session durable Hook barrier, if present.
    pub(super) fn load_prepared_execution_hook(
        &self,
        session_id: &str,
    ) -> RuntimeResult<Option<PreparedExecutionHookV1>> {
        self.persistence
            .load_record("prepared-execution-hook/v1", session_id)?
            .map(|value| {
                serde_json::from_value(value).map_err(|_| {
                    RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "prepared execution Hook record is malformed",
                    )
                })
            })
            .transpose()
    }

    /// Applies the exact prepared checkpoint/turn/context after-images and
    /// marks the session barrier complete. Every step is idempotent so a
    /// failed completion mark is reconciled on the next actor entry.
    pub(super) fn finalize_prepared_execution_hook(
        &self,
        prepared: PreparedExecutionHookV1,
    ) -> RuntimeResult<()> {
        if let Some(after_image) = prepared.checkpoint_after_image.clone() {
            let checkpoint =
                ExecutionSupervisorCheckpointV1::try_from(after_image).map_err(|_| {
                    RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "prepared execution checkpoint after-image is invalid",
                    )
                })?;
            let mut state = self.lock_state()?;
            let binding = state
                .execution_bindings
                .get_mut(prepared.session_id.as_str())
                .ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "prepared execution checkpoint has no active session binding",
                    )
                })?;
            if prepared.work_item_id.as_deref() != Some(binding.work_item_id()) {
                return Err(RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "prepared execution checkpoint Work Item no longer matches the session binding",
                ));
            }
            binding.commit_checkpoint(checkpoint);
        }
        self.restore_hook_turn(
            &prepared.session_id,
            &(Some(prepared.turn_id.clone()), prepared.turn_seq),
        )?;
        if let Some(digest) = prepared.delivered_context_digest.as_deref() {
            self.context
                .mark_hook_delivered(&prepared.session_id, digest)?;
        }
        self.persistence
            .delete_record("prepared-execution-hook/v1", &prepared.session_id)?;
        Ok(())
    }
}
