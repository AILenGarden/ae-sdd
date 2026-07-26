use ae_sdd_domain::{
    ArtifactDigest, ContextDigest, GateOutcome, InventoryGeneration, PolicyDigest, StateRevision,
    WorkItemId,
};

use crate::GateTruth;

/// Host lifecycle point governed by the daemon Hook policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookPoint {
    /// A new user turn requests a bounded context projection.
    UserPrompt,
    /// A tool invocation is about to execute.
    PreTool,
    /// A completed tool invocation is recorded.
    PostTool,
    /// The host proposes ending the current turn.
    Stop,
}

/// Host-neutral action selected by the single Hook policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookAction {
    /// Permit the host event.
    Allow,
    /// Deny a tool invocation.
    Deny,
    /// Block turn completion.
    Block,
    /// Return the daemon's bounded context projection.
    Context,
}

/// Deterministic owner of engaged Hook fail-closed behavior.
pub struct HookPolicy;

impl HookPolicy {
    /// Selects a host action from trusted engagement and authoritative Gate state.
    ///
    /// `guard` must already be freshness-normalized by the Gate boundary. An
    /// engaged `PreTool` or `Stop` without a fresh passing guard fails closed;
    /// infrastructure outcomes never become a business correction or implicit
    /// pass. User-prompt context injection and post-tool recording do not
    /// execute a transition.
    #[must_use]
    pub const fn decide(
        point: HookPoint,
        engaged: bool,
        guard: Option<&GateOutcome>,
    ) -> HookAction {
        if !engaged {
            return HookAction::Allow;
        }
        match point {
            HookPoint::UserPrompt => HookAction::Context,
            HookPoint::PostTool => HookAction::Allow,
            HookPoint::PreTool if permits(guard) => HookAction::Allow,
            HookPoint::PreTool => HookAction::Deny,
            HookPoint::Stop if permits(guard) => HookAction::Allow,
            HookPoint::Stop => HookAction::Block,
        }
    }
}

/// Minimal content proof consumed by the pure Hook guard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookContextProof {
    work_item_id: WorkItemId,
    bundle_digest: ContextDigest,
    story_digest: ArtifactDigest,
    constraints_digest: ArtifactDigest,
    thinking_engine_digest: ArtifactDigest,
    verification_digest: ArtifactDigest,
    state_revision: StateRevision,
    inventory_generation: InventoryGeneration,
}

impl HookContextProof {
    /// Constructs a proof snapshot from the already-loaded mandatory context.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        work_item_id: WorkItemId,
        bundle_digest: ContextDigest,
        story_digest: ArtifactDigest,
        constraints_digest: ArtifactDigest,
        thinking_engine_digest: ArtifactDigest,
        verification_digest: ArtifactDigest,
        state_revision: StateRevision,
        inventory_generation: InventoryGeneration,
    ) -> Self {
        Self {
            work_item_id,
            bundle_digest,
            story_digest,
            constraints_digest,
            thinking_engine_digest,
            verification_digest,
            state_revision,
            inventory_generation,
        }
    }
}

/// Complete bounded input required for one Hook guard decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookGuardInput {
    point: HookPoint,
    engaged: bool,
    work_item_id: WorkItemId,
    methodology_digest: Option<ArtifactDigest>,
    context_proof: Option<HookContextProof>,
    current_state_revision: StateRevision,
    current_inventory_generation: InventoryGeneration,
    gate: Option<GateOutcome>,
}

impl HookGuardInput {
    /// Constructs one pure guard request from caller-supplied facts.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        point: HookPoint,
        engaged: bool,
        work_item_id: WorkItemId,
        methodology_digest: Option<ArtifactDigest>,
        context_proof: Option<HookContextProof>,
        current_state_revision: StateRevision,
        current_inventory_generation: InventoryGeneration,
        gate: Option<GateOutcome>,
    ) -> Self {
        Self {
            point,
            engaged,
            work_item_id,
            methodology_digest,
            context_proof,
            current_state_revision,
            current_inventory_generation,
            gate,
        }
    }
}

/// High-level result of an engaged Hook decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookGuardDisposition {
    /// The host action is authorized.
    Allow,
    /// The host action is denied or blocked.
    Deny,
    /// Context must be refreshed before a guarded action can proceed.
    RefreshRequired,
}

/// Stable reason attached to every Hook guard decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookGuardReason {
    /// The daemon is not engaged for this event.
    NotEngaged,
    /// Methodology or loaded context proof was absent.
    ContextRequired,
    /// The proof belonged to a different Work Item.
    WorkItemMismatch,
    /// The project state revision moved after proof computation.
    StateRevisionStale,
    /// The resource inventory generation moved after proof computation.
    InventoryGenerationStale,
    /// The supplied context proof is current and complete.
    FreshContext,
    /// The authoritative Gate outcome did not permit the action.
    GateRejected,
}

/// Pure result returned to a host-specific Hook adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HookGuardDecision {
    disposition: HookGuardDisposition,
    action: HookAction,
    reason: HookGuardReason,
    proof_digest: Option<ContextDigest>,
    policy_digest: PolicyDigest,
}

impl HookGuardDecision {
    /// Returns the high-level guard disposition.
    pub const fn disposition(self) -> HookGuardDisposition {
        self.disposition
    }

    /// Returns the host-neutral action.
    pub const fn action(self) -> HookAction {
        self.action
    }

    /// Returns the stable reason code.
    pub const fn reason(self) -> HookGuardReason {
        self.reason
    }

    /// Returns the bound proof digest when a proof was consumed.
    pub const fn proof_digest(self) -> Option<ContextDigest> {
        self.proof_digest
    }

    /// Returns the digest of the policy table that produced this decision.
    pub const fn policy_digest(self) -> PolicyDigest {
        self.policy_digest
    }
}

/// Application port implemented by a bounded, I/O-free Hook guard.
pub trait HookGuardPort {
    /// Decides one Hook event using only supplied typed facts.
    fn decide(&self, input: &HookGuardInput) -> HookGuardDecision;
}

/// Stateless implementation of [`HookGuardPort`].
#[derive(Clone, Copy, Debug, Default)]
pub struct HookGuard;

impl HookGuardPort for HookGuard {
    fn decide(&self, input: &HookGuardInput) -> HookGuardDecision {
        let policy_digest = crate::policy_digest();
        if !input.engaged {
            return HookGuardDecision {
                disposition: HookGuardDisposition::Allow,
                action: HookAction::Allow,
                reason: HookGuardReason::NotEngaged,
                proof_digest: None,
                policy_digest,
            };
        }
        if input.methodology_digest.is_none() || input.context_proof.is_none() {
            let (disposition, action) = match input.point {
                HookPoint::UserPrompt | HookPoint::PostTool => {
                    (HookGuardDisposition::RefreshRequired, HookAction::Context)
                }
                HookPoint::PreTool => (HookGuardDisposition::Deny, HookAction::Deny),
                HookPoint::Stop => (HookGuardDisposition::Deny, HookAction::Block),
            };
            return HookGuardDecision {
                disposition,
                action,
                reason: HookGuardReason::ContextRequired,
                proof_digest: None,
                policy_digest,
            };
        }

        let Some(proof) = input.context_proof.as_ref() else {
            return HookGuardDecision {
                disposition: HookGuardDisposition::Deny,
                action: guarded_denial_action(input.point),
                reason: HookGuardReason::ContextRequired,
                proof_digest: None,
                policy_digest,
            };
        };
        let Some(methodology_digest) = input.methodology_digest else {
            return HookGuardDecision {
                disposition: HookGuardDisposition::Deny,
                action: guarded_denial_action(input.point),
                reason: HookGuardReason::ContextRequired,
                proof_digest: None,
                policy_digest,
            };
        };
        let proof_digest = bound_hook_proof_digest(proof, methodology_digest);
        if proof.work_item_id != input.work_item_id {
            return HookGuardDecision {
                disposition: HookGuardDisposition::Deny,
                action: guarded_denial_action(input.point),
                reason: HookGuardReason::WorkItemMismatch,
                proof_digest: Some(proof_digest),
                policy_digest,
            };
        }
        if proof.state_revision != input.current_state_revision {
            return HookGuardDecision {
                disposition: HookGuardDisposition::RefreshRequired,
                action: guarded_denial_action(input.point),
                reason: HookGuardReason::StateRevisionStale,
                proof_digest: Some(proof_digest),
                policy_digest,
            };
        }
        if proof.inventory_generation != input.current_inventory_generation {
            return HookGuardDecision {
                disposition: HookGuardDisposition::RefreshRequired,
                action: guarded_denial_action(input.point),
                reason: HookGuardReason::InventoryGenerationStale,
                proof_digest: Some(proof_digest),
                policy_digest,
            };
        }

        let action = HookPolicy::decide(input.point, input.engaged, input.gate.as_ref());
        HookGuardDecision {
            disposition: if matches!(action, HookAction::Allow | HookAction::Context) {
                HookGuardDisposition::Allow
            } else {
                HookGuardDisposition::Deny
            },
            action,
            reason: if matches!(action, HookAction::Allow | HookAction::Context) {
                HookGuardReason::FreshContext
            } else {
                HookGuardReason::GateRejected
            },
            proof_digest: Some(proof_digest),
            policy_digest,
        }
    }
}

const fn guarded_denial_action(point: HookPoint) -> HookAction {
    match point {
        HookPoint::UserPrompt | HookPoint::PostTool => HookAction::Context,
        HookPoint::PreTool => HookAction::Deny,
        HookPoint::Stop => HookAction::Block,
    }
}

fn bound_hook_proof_digest(
    proof: &HookContextProof,
    methodology_digest: ArtifactDigest,
) -> ContextDigest {
    fn push_field(target: &mut Vec<u8>, value: &[u8]) {
        target.extend_from_slice(&(value.len() as u64).to_be_bytes());
        target.extend_from_slice(value);
    }

    let mut canonical = Vec::new();
    push_field(&mut canonical, b"ae-sdd/hook-context-proof/v1");
    push_field(&mut canonical, proof.work_item_id.as_str().as_bytes());
    canonical.extend_from_slice(proof.bundle_digest.as_bytes());
    canonical.extend_from_slice(proof.story_digest.as_bytes());
    canonical.extend_from_slice(proof.constraints_digest.as_bytes());
    canonical.extend_from_slice(proof.thinking_engine_digest.as_bytes());
    canonical.extend_from_slice(proof.verification_digest.as_bytes());
    canonical.extend_from_slice(methodology_digest.as_bytes());
    canonical.extend_from_slice(&proof.state_revision.get().to_be_bytes());
    canonical.extend_from_slice(&proof.inventory_generation.get().to_be_bytes());
    ContextDigest::digest(canonical)
}

const fn permits(outcome: Option<&GateOutcome>) -> bool {
    match outcome {
        Some(outcome) => GateTruth::judge(outcome).transition_permitted(),
        None => false,
    }
}

/// Host-reported execution tool classification carried by
/// `hostPayload.executionEvent`.
///
/// The daemon never guesses a class from tool names: the host reports one of
/// these frozen classes and anything else fails closed during strict decode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionHookToolClass {
    /// Reading project source within the declared slice scope.
    SourceRead,
    /// Searching the workspace.
    Search,
    /// Applying a patch.
    Patch,
    /// Running the focused verification bound to the slice.
    FocusedTest,
    /// Running a broad verification.
    BroadTest,
    /// Appending one evidence ledger event.
    Evidence,
    /// Any tool call the host could not place.
    Other,
}

impl ExecutionHookToolClass {
    /// Decodes a wire class name; unknown names return `None` so the caller
    /// fails closed instead of guessing a classification.
    #[must_use]
    pub fn from_wire_name(name: &str) -> Option<Self> {
        match name {
            "source-read" => Some(Self::SourceRead),
            "search" => Some(Self::Search),
            "patch" => Some(Self::Patch),
            "focused-test" => Some(Self::FocusedTest),
            "broad-test" => Some(Self::BroadTest),
            "evidence" => Some(Self::Evidence),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

    /// Returns the stable wire name of this class.
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::SourceRead => "source-read",
            Self::Search => "search",
            Self::Patch => "patch",
            Self::FocusedTest => "focused-test",
            Self::BroadTest => "broad-test",
            Self::Evidence => "evidence",
            Self::Other => "other",
        }
    }
}

/// Specific reason the execution Hook guard rejects a tool event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionHookDenialReason {
    /// A broad verification was requested before the focused GREEN.
    BroadTestBeforeFocusedGreen,
}

/// Machine verdict produced by the pure execution Hook guard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionHookVerdict {
    /// The event has no classified `executionEvent` or the session is not
    /// bound to an execution capsule; the shadow rollout only records the
    /// event and never blocks it.
    Unclassified,
    /// The event is admissible; the guard echoes the frozen retained-output
    /// budget so the host can truncate before evidence is produced.
    Allow {
        /// Frozen single-call retained-output budget in bytes.
        output_budget_bytes: u32,
    },
    /// The event is rejected until machine-verified progress is made.
    RequireProgress {
        /// Specific rejection reason.
        reason: ExecutionHookDenialReason,
    },
}

/// Bounded fact snapshot consumed by the pure execution Hook guard.
///
/// Every fact is authority-reported and carried by the session binding; the
/// guard itself never reads files, Gates, clocks or randomness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionHookGuardInput {
    bound: bool,
    focused_green: bool,
    class: Option<ExecutionHookToolClass>,
    max_tool_output_bytes: u32,
}

impl ExecutionHookGuardInput {
    /// Constructs one guard request from caller-supplied facts.
    pub const fn new(
        bound: bool,
        focused_green: bool,
        class: Option<ExecutionHookToolClass>,
        max_tool_output_bytes: u32,
    ) -> Self {
        Self {
            bound,
            focused_green,
            class,
            max_tool_output_bytes,
        }
    }
}

/// Deterministic owner of the execution tool-event guard at the Hook
/// boundary.
///
/// The guard owns exactly one frozen boundary rule — a broad verification is
/// inadmissible before the focused GREEN — plus the shadow rollout semantics:
/// events without a classified `executionEvent`, or sessions not bound to an
/// execution capsule, are recorded as `unclassified` and never blocked.  The
/// full slice-progress policy (investigation batches, progress events, slice
/// lifecycle) stays with the authoritative `ExecutionSupervisor` reducer on
/// the operation path; this guard never duplicates it.
pub struct ExecutionHookGuard;

impl ExecutionHookGuard {
    /// Adjudicates one execution tool event against the session facts.
    #[must_use]
    pub const fn decide(input: &ExecutionHookGuardInput) -> ExecutionHookVerdict {
        let Some(class) = input.class else {
            return ExecutionHookVerdict::Unclassified;
        };
        if !input.bound {
            return ExecutionHookVerdict::Unclassified;
        }
        if matches!(class, ExecutionHookToolClass::BroadTest) && !input.focused_green {
            return ExecutionHookVerdict::RequireProgress {
                reason: ExecutionHookDenialReason::BroadTestBeforeFocusedGreen,
            };
        }
        ExecutionHookVerdict::Allow {
            output_budget_bytes: input.max_tool_output_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use ae_sdd_domain::{
        CancellationCode, ErrorCode, FindingCode, FreshnessDimension, GateCancellation, GateError,
        GateFailure, GateFinding, GateOutcome, GateTimeout, StaleGate,
    };

    use super::*;

    fn non_pass_outcomes() -> [GateOutcome; 5] {
        [
            GateOutcome::Fail(
                GateFailure::new([GateFinding::new(
                    FindingCode::new("HOOK_BLOCKED").expect("valid code"),
                    [],
                )])
                .expect("non-empty failure"),
            ),
            GateOutcome::Error(GateError::new(
                ErrorCode::new("HOOK_ERROR").expect("valid code"),
                false,
            )),
            GateOutcome::Timeout(GateTimeout::new(250).expect("positive timeout")),
            GateOutcome::Cancelled(GateCancellation::new(
                CancellationCode::new("CALLER_CANCELLED").expect("valid code"),
            )),
            GateOutcome::Stale(
                StaleGate::new([FreshnessDimension::StateRevision]).expect("changed dimension"),
            ),
        ]
    }

    #[test]
    fn engaged_pre_tool_and_stop_only_accept_fresh_pass() {
        assert_eq!(
            HookPolicy::decide(HookPoint::PreTool, true, Some(&GateOutcome::Pass)),
            HookAction::Allow
        );
        assert_eq!(
            HookPolicy::decide(HookPoint::Stop, true, Some(&GateOutcome::Pass)),
            HookAction::Allow
        );
        assert_eq!(
            HookPolicy::decide(HookPoint::PreTool, true, None),
            HookAction::Deny
        );
        assert_eq!(
            HookPolicy::decide(HookPoint::Stop, true, None),
            HookAction::Block
        );
        for outcome in non_pass_outcomes() {
            assert_eq!(
                HookPolicy::decide(HookPoint::PreTool, true, Some(&outcome)),
                HookAction::Deny
            );
            assert_eq!(
                HookPolicy::decide(HookPoint::Stop, true, Some(&outcome)),
                HookAction::Block
            );
        }
    }

    #[test]
    fn unengaged_hooks_do_not_claim_daemon_control() {
        for point in [
            HookPoint::UserPrompt,
            HookPoint::PreTool,
            HookPoint::PostTool,
            HookPoint::Stop,
        ] {
            assert_eq!(HookPolicy::decide(point, false, None), HookAction::Allow);
        }
    }

    #[test]
    fn engaged_prompt_projects_context_and_post_tool_only_records() {
        assert_eq!(
            HookPolicy::decide(HookPoint::UserPrompt, true, None),
            HookAction::Context
        );
        assert_eq!(
            HookPolicy::decide(HookPoint::PostTool, true, None),
            HookAction::Allow
        );
    }

    #[test]
    fn execution_event_wire_names_round_trip_and_fail_closed() {
        for class in [
            ExecutionHookToolClass::SourceRead,
            ExecutionHookToolClass::Search,
            ExecutionHookToolClass::Patch,
            ExecutionHookToolClass::FocusedTest,
            ExecutionHookToolClass::BroadTest,
            ExecutionHookToolClass::Evidence,
            ExecutionHookToolClass::Other,
        ] {
            assert_eq!(
                ExecutionHookToolClass::from_wire_name(class.wire_name()),
                Some(class)
            );
        }
        assert_eq!(ExecutionHookToolClass::from_wire_name("warp-drive"), None);
        assert_eq!(ExecutionHookToolClass::from_wire_name("BroadTest"), None);
        assert_eq!(ExecutionHookToolClass::from_wire_name(""), None);
    }

    #[test]
    fn execution_guard_only_denies_broad_before_the_focused_green() {
        const BUDGET: u32 = 65_536;
        let unbound = ExecutionHookGuardInput::new(false, false, None, BUDGET);
        assert_eq!(
            ExecutionHookGuard::decide(&unbound),
            ExecutionHookVerdict::Unclassified
        );
        let bound_without_event = ExecutionHookGuardInput::new(true, false, None, BUDGET);
        assert_eq!(
            ExecutionHookGuard::decide(&bound_without_event),
            ExecutionHookVerdict::Unclassified
        );
        let unbound_broad = ExecutionHookGuardInput::new(
            false,
            false,
            Some(ExecutionHookToolClass::BroadTest),
            BUDGET,
        );
        assert_eq!(
            ExecutionHookGuard::decide(&unbound_broad),
            ExecutionHookVerdict::Unclassified
        );
        let broad_before_green = ExecutionHookGuardInput::new(
            true,
            false,
            Some(ExecutionHookToolClass::BroadTest),
            BUDGET,
        );
        assert_eq!(
            ExecutionHookGuard::decide(&broad_before_green),
            ExecutionHookVerdict::RequireProgress {
                reason: ExecutionHookDenialReason::BroadTestBeforeFocusedGreen,
            }
        );
        let broad_after_green = ExecutionHookGuardInput::new(
            true,
            true,
            Some(ExecutionHookToolClass::BroadTest),
            BUDGET,
        );
        assert_eq!(
            ExecutionHookGuard::decide(&broad_after_green),
            ExecutionHookVerdict::Allow {
                output_budget_bytes: BUDGET,
            }
        );
        for class in [
            ExecutionHookToolClass::SourceRead,
            ExecutionHookToolClass::Search,
            ExecutionHookToolClass::Patch,
            ExecutionHookToolClass::FocusedTest,
            ExecutionHookToolClass::Evidence,
            ExecutionHookToolClass::Other,
        ] {
            let input = ExecutionHookGuardInput::new(true, false, Some(class), BUDGET);
            assert_eq!(
                ExecutionHookGuard::decide(&input),
                ExecutionHookVerdict::Allow {
                    output_budget_bytes: BUDGET,
                },
                "{} stays admissible before the focused GREEN",
                class.wire_name()
            );
        }
    }
}
