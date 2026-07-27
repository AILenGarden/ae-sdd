use ae_sdd_domain::{
    ArtifactDigest, ContextDigest, FindingCode, GateFailure, GateFinding, GateOutcome,
    InventoryGeneration, StateRevision, WorkItemId,
};
use ae_sdd_policy::{
    HookAction, HookContextProof, HookGuard, HookGuardDisposition, HookGuardInput, HookGuardPort,
    HookGuardReason, HookPoint,
};

#[test]
fn engaged_hook_without_methodology_or_context_proof_fails_closed() {
    let guard = HookGuard;
    let work_item = WorkItemId::new("STORY-CONTEXT-001").unwrap();
    let pre_tool = HookGuardInput::new(
        HookPoint::PreTool,
        true,
        work_item.clone(),
        None,
        None,
        StateRevision::new(7),
        InventoryGeneration::new(3),
        None,
    );

    let decision = guard.decide(&pre_tool);
    assert_eq!(decision.disposition(), HookGuardDisposition::Deny);
    assert_eq!(decision.action(), HookAction::Deny);
    assert_eq!(decision.reason(), HookGuardReason::ContextRequired);
    assert!(decision.proof_digest().is_none());

    let prompt = HookGuardInput::new(
        HookPoint::UserPrompt,
        true,
        work_item,
        Some(ArtifactDigest::digest(b"methodology")),
        None,
        StateRevision::new(7),
        InventoryGeneration::new(3),
        None,
    );
    let decision = guard.decide(&prompt);
    assert_eq!(
        decision.disposition(),
        HookGuardDisposition::RefreshRequired
    );
    assert_eq!(decision.action(), HookAction::Context);
    assert_eq!(decision.reason(), HookGuardReason::ContextRequired);
}

fn proof(work_item: &str, revision: u64, generation: u64) -> HookContextProof {
    HookContextProof::new(
        WorkItemId::new(work_item).unwrap(),
        ContextDigest::digest(b"bundle"),
        ArtifactDigest::digest(b"story"),
        ArtifactDigest::digest(b"constraints"),
        ArtifactDigest::digest(b"thinking"),
        ArtifactDigest::digest(b"verification"),
        StateRevision::new(revision),
        InventoryGeneration::new(generation),
    )
}

#[test]
fn hook_guard_requires_matching_fresh_proof_before_gate_can_allow() {
    let guard = HookGuard;
    let work_item = WorkItemId::new("STORY-CONTEXT-001").unwrap();
    let fresh = HookGuardInput::new(
        HookPoint::PreTool,
        true,
        work_item.clone(),
        Some(ArtifactDigest::digest(b"methodology")),
        Some(proof("STORY-CONTEXT-001", 7, 3)),
        StateRevision::new(7),
        InventoryGeneration::new(3),
        Some(GateOutcome::Pass),
    );
    let decision = guard.decide(&fresh);
    assert_eq!(decision.disposition(), HookGuardDisposition::Allow);
    assert_eq!(decision.action(), HookAction::Allow);
    assert_eq!(decision.reason(), HookGuardReason::FreshContext);
    assert!(decision.proof_digest().is_some());

    let stale_revision = HookGuardInput::new(
        HookPoint::PreTool,
        true,
        work_item.clone(),
        Some(ArtifactDigest::digest(b"methodology")),
        Some(proof("STORY-CONTEXT-001", 6, 3)),
        StateRevision::new(7),
        InventoryGeneration::new(3),
        Some(GateOutcome::Pass),
    );
    let decision = guard.decide(&stale_revision);
    assert_eq!(
        decision.disposition(),
        HookGuardDisposition::RefreshRequired
    );
    assert_eq!(decision.action(), HookAction::Deny);
    assert_eq!(decision.reason(), HookGuardReason::StateRevisionStale);

    let stale_inventory = HookGuardInput::new(
        HookPoint::PreTool,
        true,
        work_item.clone(),
        Some(ArtifactDigest::digest(b"methodology")),
        Some(proof("STORY-CONTEXT-001", 7, 2)),
        StateRevision::new(7),
        InventoryGeneration::new(3),
        Some(GateOutcome::Pass),
    );
    let decision = guard.decide(&stale_inventory);
    assert_eq!(
        decision.disposition(),
        HookGuardDisposition::RefreshRequired
    );
    assert_eq!(decision.reason(), HookGuardReason::InventoryGenerationStale);

    let wrong_work_item = HookGuardInput::new(
        HookPoint::PreTool,
        true,
        work_item,
        Some(ArtifactDigest::digest(b"methodology")),
        Some(proof("STORY-OTHER-001", 7, 3)),
        StateRevision::new(7),
        InventoryGeneration::new(3),
        Some(GateOutcome::Pass),
    );
    let decision = guard.decide(&wrong_work_item);
    assert_eq!(decision.disposition(), HookGuardDisposition::Deny);
    assert_eq!(decision.reason(), HookGuardReason::WorkItemMismatch);
}

#[test]
fn hook_guard_proof_digest_binds_all_mandatory_context_and_methodology_inputs() {
    let guard = HookGuard;
    let work_item = WorkItemId::new("STORY-CONTEXT-001").unwrap();
    let build = |verification: &[u8], methodology: &[u8]| {
        HookGuardInput::new(
            HookPoint::PreTool,
            true,
            work_item.clone(),
            Some(ArtifactDigest::digest(methodology)),
            Some(HookContextProof::new(
                work_item.clone(),
                ContextDigest::digest(b"bundle"),
                ArtifactDigest::digest(b"story"),
                ArtifactDigest::digest(b"constraints"),
                ArtifactDigest::digest(b"thinking"),
                ArtifactDigest::digest(verification),
                StateRevision::new(7),
                InventoryGeneration::new(3),
            )),
            StateRevision::new(7),
            InventoryGeneration::new(3),
            Some(GateOutcome::Pass),
        )
    };

    let baseline = guard.decide(&build(b"verification", b"methodology"));
    let changed_verification = guard.decide(&build(b"changed verification", b"methodology"));
    let changed_methodology = guard.decide(&build(b"verification", b"changed methodology"));

    assert_ne!(baseline.proof_digest(), changed_verification.proof_digest());
    assert_ne!(baseline.proof_digest(), changed_methodology.proof_digest());
    assert_eq!(baseline.policy_digest(), ae_sdd_policy::policy_digest());
}

#[test]
fn hook_guard_covers_unengaged_missing_stop_and_rejected_gate_decisions() {
    let guard = HookGuard;
    let work_item = WorkItemId::new("STORY-CONTEXT-001").unwrap();
    let unengaged = HookGuardInput::new(
        HookPoint::PostTool,
        false,
        work_item.clone(),
        None,
        None,
        StateRevision::new(7),
        InventoryGeneration::new(3),
        None,
    );
    let decision = guard.decide(&unengaged);
    assert_eq!(decision.disposition(), HookGuardDisposition::Allow);
    assert_eq!(decision.action(), HookAction::Allow);
    assert_eq!(decision.reason(), HookGuardReason::NotEngaged);
    assert_eq!(decision.policy_digest(), ae_sdd_policy::policy_digest());

    let missing_stop = HookGuardInput::new(
        HookPoint::Stop,
        true,
        work_item.clone(),
        None,
        None,
        StateRevision::new(7),
        InventoryGeneration::new(3),
        None,
    );
    let decision = guard.decide(&missing_stop);
    assert_eq!(decision.disposition(), HookGuardDisposition::Deny);
    assert_eq!(decision.action(), HookAction::Block);
    assert_eq!(decision.reason(), HookGuardReason::ContextRequired);

    let failure = GateOutcome::Fail(
        GateFailure::new([GateFinding::new(FindingCode::new("BLOCKED").unwrap(), [])]).unwrap(),
    );
    let rejected = HookGuardInput::new(
        HookPoint::PreTool,
        true,
        work_item,
        Some(ArtifactDigest::digest(b"methodology")),
        Some(proof("STORY-CONTEXT-001", 7, 3)),
        StateRevision::new(7),
        InventoryGeneration::new(3),
        Some(failure),
    );
    let decision = guard.decide(&rejected);
    assert_eq!(decision.disposition(), HookGuardDisposition::Deny);
    assert_eq!(decision.action(), HookAction::Deny);
    assert_eq!(decision.reason(), HookGuardReason::GateRejected);
    assert!(decision.proof_digest().is_some());
}
