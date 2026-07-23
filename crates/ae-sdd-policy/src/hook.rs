use ae_sdd_domain::GateOutcome;

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

const fn permits(outcome: Option<&GateOutcome>) -> bool {
    match outcome {
        Some(outcome) => GateTruth::judge(outcome).transition_permitted(),
        None => false,
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
}
