use std::panic::{AssertUnwindSafe, catch_unwind};

use ae_sdd_contracts::{
    FileLockSnapshot, LifecycleCommand, LifecycleDisposition, LifecycleInput, PrdId, PrdSummary,
    ProcessSnapshot, ReasonCode, SchemaVersion, StorySummary,
};
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, DesignRoute, EvidenceDigest, EvidenceId, EvidenceRef,
    InputFingerprint, ProcessPhase, ProjectRelativePath, SessionId, StateRevision, StoryId,
    VerificationId, WorkItemId, WorkScale,
};
use ae_sdd_lifecycle::{
    LifecycleEngine, MAX_CONFIRMATION_APPROVED_AT_BYTES, MAX_CONFIRMATION_APPROVED_BY_BYTES,
    MAX_CONFIRMATION_ID_BYTES,
};
use ae_sdd_protocol::ConfirmationRef;
use proptest::prelude::*;
use serde_json::Value;
use uuid::Uuid;

const NOW: u64 = 1_785_000_000_000;

#[test]
fn confirmation_field_byte_contract_is_frozen() {
    assert_eq!(MAX_CONFIRMATION_ID_BYTES, 71);
    assert_eq!(MAX_CONFIRMATION_APPROVED_BY_BYTES, 256);
    assert_eq!(MAX_CONFIRMATION_APPROVED_AT_BYTES, 64);
}

#[test]
fn legal_transition_is_permitted_and_replay_is_byte_stable() {
    let input = input(
        LifecycleCommand::Transition {
            target_phase: ProcessPhase::TestcaseGenerated,
        },
        ProcessPhase::StoryGenerated,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );

    let first = LifecycleEngine::plan(&input).expect("legal transition plans");
    let replay = LifecycleEngine::plan(&input).expect("replay plans");

    assert_eq!(first.disposition(), LifecycleDisposition::Permitted);
    assert_eq!(first.intents().len(), 2);
    assert_eq!(
        first.intents()[0].target.namespace.as_str(),
        "work-item-state"
    );
    assert_eq!(
        first.intents()[1].target.namespace.as_str(),
        "runtime-event"
    );
    assert_eq!(first, replay);
    assert_eq!(
        serde_json::to_vec(&first).expect("plan serializes"),
        serde_json::to_vec(&replay).expect("replay serializes")
    );
}

#[test]
fn transition_policy_uses_real_role_scale_and_direct_route() {
    let child = input(
        LifecycleCommand::Transition {
            target_phase: ProcessPhase::TestcaseGenerated,
        },
        ProcessPhase::StoryGenerated,
        None,
        AgentRole::Series,
        WorkScale::Large,
        DesignRoute::Story,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let skipped = input(
        LifecycleCommand::Transition {
            target_phase: ProcessPhase::Coding,
        },
        ProcessPhase::StoryGenerated,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let unsupported = input(
        LifecycleCommand::Transition {
            target_phase: ProcessPhase::RouteSelected,
        },
        ProcessPhase::Initialized,
        None,
        AgentRole::Root,
        WorkScale::Small,
        DesignRoute::Story,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );

    for denied in [&child, &skipped, &unsupported] {
        let plan = LifecycleEngine::plan(denied).expect("semantic denial is a plan");
        assert_eq!(plan.disposition(), LifecycleDisposition::Denied);
        assert!(plan.intents().is_empty());
    }
}

#[test]
fn pause_and_resume_preserve_the_exact_source_phase() {
    let pause = input(
        LifecycleCommand::Pause,
        ProcessPhase::Coding,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&pause)
            .expect("pause plans")
            .disposition(),
        LifecycleDisposition::Permitted
    );

    let exact_resume = input(
        LifecycleCommand::Resume,
        ProcessPhase::Paused,
        Some(ProcessPhase::Coding),
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&exact_resume)
            .expect("resume plans")
            .disposition(),
        LifecycleDisposition::Permitted
    );

    let missing_source = input(
        LifecycleCommand::Resume,
        ProcessPhase::Paused,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&missing_source)
            .expect("invalid resume is a denied plan")
            .disposition(),
        LifecycleDisposition::Denied
    );
}

#[test]
fn story_binding_and_completion_fail_closed_on_registration_and_evidence() {
    let story_id = story_id("STORY-NESTED-001");
    let unregistered = story(story_id.clone(), ProcessPhase::Completed, 0, 1, false);
    let bind = input(
        LifecycleCommand::BindStory {
            story_id: story_id.clone(),
            document_path: path("ae-sdd-doc/Story/STORY-NESTED-001.md"),
        },
        ProcessPhase::StoryGenerated,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        vec![unregistered],
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&bind)
            .expect("unregistered story is denied")
            .disposition(),
        LifecycleDisposition::Denied
    );

    let registered = story(story_id.clone(), ProcessPhase::StoryGenerated, 1, 0, true);
    let valid_bind = input(
        LifecycleCommand::BindStory {
            story_id: story_id.clone(),
            document_path: path("ae-sdd-doc/Story/STORY-NESTED-001.md"),
        },
        ProcessPhase::StoryGenerated,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        vec![registered],
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&valid_bind)
            .expect("registered canonical Story binding plans")
            .disposition(),
        LifecycleDisposition::Permitted
    );

    let incomplete = story(story_id.clone(), ProcessPhase::CodeReviewed, 0, 1, true);
    let complete_story = input(
        LifecycleCommand::CompleteStory {
            story_id: story_id.clone(),
        },
        ProcessPhase::Coding,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        vec![incomplete],
        None,
        Vec::new(),
        vec![evidence("story-completion")],
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&complete_story)
            .expect("incomplete child is denied")
            .disposition(),
        LifecycleDisposition::Denied
    );

    let complete = story(story_id.clone(), ProcessPhase::Completed, 0, 1, true);
    let awaiting = input(
        LifecycleCommand::CompleteStory { story_id },
        ProcessPhase::Coding,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        vec![complete],
        None,
        Vec::new(),
        vec![evidence("story-completion")],
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&awaiting)
            .expect("protected completion awaits confirmation")
            .disposition(),
        LifecycleDisposition::AwaitingConfirmation
    );
}

#[test]
fn story_binding_requires_the_exact_authoritative_story_directory() {
    let story_id = story_id("STORY-PATH-001");
    let registered = story(story_id.clone(), ProcessPhase::StoryGenerated, 1, 0, true);

    for invalid_path in [
        "tmp/STORY-PATH-001.md",
        "ae-sdd-doc/Other/STORY-PATH-001.md",
        "nested/ae-sdd-doc/Story/STORY-PATH-001.md",
    ] {
        let bind = input(
            LifecycleCommand::BindStory {
                story_id: story_id.clone(),
                document_path: path(invalid_path),
            },
            ProcessPhase::StoryGenerated,
            None,
            AgentRole::Root,
            WorkScale::Large,
            DesignRoute::Story,
            vec![registered.clone()],
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(
            LifecycleEngine::plan(&bind)
                .expect("non-authoritative Story path is a semantic denial")
                .disposition(),
            LifecycleDisposition::Denied
        );
    }
}

#[test]
fn prd_completion_is_a_strict_children_dependencies_risks_gates_review_and() {
    let first_id = story_id("STORY-PRD-001");
    let second_id = story_id("STORY-PRD-002");
    let stories = vec![
        story(first_id.clone(), ProcessPhase::Completed, 0, 1, true),
        story(second_id.clone(), ProcessPhase::Completed, 0, 2, true),
    ];
    let mut summary = PrdSummary {
        prd_id: PrdId::new("PRD-001").expect("PRD id"),
        registered_story_ids: vec![first_id.clone(), second_id.clone()],
        completed_story_ids: vec![first_id.clone(), second_id.clone()],
        dependencies_satisfied: true,
        residual_risks_cleared: true,
        gates_passed: true,
        review_passed: false,
    };
    let incomplete = input(
        LifecycleCommand::CompletePrd {
            prd_id: summary.prd_id.clone(),
        },
        ProcessPhase::Completed,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        stories.clone(),
        Some(summary.clone()),
        Vec::new(),
        vec![evidence("prd-completion")],
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&incomplete)
            .expect("incomplete PRD is denied")
            .disposition(),
        LifecycleDisposition::Denied
    );

    summary.review_passed = true;
    let awaiting = input(
        LifecycleCommand::CompletePrd {
            prd_id: summary.prd_id.clone(),
        },
        ProcessPhase::Completed,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        stories,
        Some(summary),
        Vec::new(),
        vec![evidence("prd-completion")],
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&awaiting)
            .expect("complete PRD awaits confirmation")
            .disposition(),
        LifecycleDisposition::AwaitingConfirmation
    );
}

#[test]
fn protected_command_confirmation_is_digest_and_revision_bound() {
    let story_id = story_id("STORY-CONFIRM-001");
    let complete = story(story_id.clone(), ProcessPhase::Completed, 0, 1, true);
    let pending = input(
        LifecycleCommand::CompleteStory {
            story_id: story_id.clone(),
        },
        ProcessPhase::Completed,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        vec![complete.clone()],
        None,
        Vec::new(),
        vec![evidence("story-completion")],
        Vec::new(),
    );
    let pending_plan = LifecycleEngine::plan(&pending).expect("pending plan");
    let binding = confirmation_binding(&pending_plan);

    let confirmed = input(
        LifecycleCommand::CompleteStory {
            story_id: story_id.clone(),
        },
        ProcessPhase::Completed,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        vec![complete.clone()],
        None,
        vec![confirmation(&binding)],
        vec![evidence("story-completion")],
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&confirmed)
            .expect("bound confirmation permits")
            .disposition(),
        LifecycleDisposition::Permitted
    );

    let mismatched = input(
        LifecycleCommand::CompleteStory { story_id },
        ProcessPhase::Completed,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        vec![complete],
        None,
        vec![confirmation(&"0".repeat(64))],
        vec![evidence("story-completion")],
        Vec::new(),
    );
    let error = LifecycleEngine::plan(&mismatched).expect_err("mismatch fails closed");
    assert_eq!(
        error.code,
        ae_sdd_contracts::ControlPlaneErrorCode::ConfirmationMismatch
    );
}

#[test]
fn confirmation_approver_is_byte_bounded_and_rejects_control_characters() {
    let (story_id, complete, binding) = pending_story_confirmation("STORY-CONFIRM-APPROVER");

    for invalid_approver in ["x".repeat(257), "user:\0owner".to_owned()] {
        let mut approval = confirmation(&binding);
        approval.approved_by = invalid_approver;
        let confirmed = input(
            LifecycleCommand::CompleteStory {
                story_id: story_id.clone(),
            },
            ProcessPhase::Completed,
            None,
            AgentRole::Root,
            WorkScale::Large,
            DesignRoute::Story,
            vec![complete.clone()],
            None,
            vec![approval],
            vec![evidence("story-completion")],
            Vec::new(),
        );

        let error = LifecycleEngine::plan(&confirmed).expect_err("invalid approver fails closed");
        assert_eq!(
            error.code,
            ae_sdd_contracts::ControlPlaneErrorCode::ConfirmationMismatch
        );
    }

    let mut boundary_approval = confirmation(&binding);
    boundary_approval.approved_by = "x".repeat(256);
    let boundary = input(
        LifecycleCommand::CompleteStory { story_id },
        ProcessPhase::Completed,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        vec![complete],
        None,
        vec![boundary_approval],
        vec![evidence("story-completion")],
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&boundary)
            .expect("maximum-size approver is valid")
            .disposition(),
        LifecycleDisposition::Permitted
    );
}

#[test]
fn confirmation_timestamp_is_bounded_canonical_utc_rfc3339() {
    let (story_id, complete, binding) = pending_story_confirmation("STORY-CONFIRM-TIME");

    for invalid_timestamp in [
        "2026-07-24T08:00:00+08:00".to_owned(),
        "2026-07-24T00:00:00Z\n".to_owned(),
        "x".repeat(65),
    ] {
        let mut approval = confirmation(&binding);
        approval.approved_at = invalid_timestamp;
        let confirmed = input(
            LifecycleCommand::CompleteStory {
                story_id: story_id.clone(),
            },
            ProcessPhase::Completed,
            None,
            AgentRole::Root,
            WorkScale::Large,
            DesignRoute::Story,
            vec![complete.clone()],
            None,
            vec![approval],
            vec![evidence("story-completion")],
            Vec::new(),
        );

        let error = LifecycleEngine::plan(&confirmed).expect_err("invalid timestamp fails closed");
        assert_eq!(
            error.code,
            ae_sdd_contracts::ControlPlaneErrorCode::ConfirmationMismatch
        );
    }

    let canonical = input(
        LifecycleCommand::CompleteStory { story_id },
        ProcessPhase::Completed,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        vec![complete],
        None,
        vec![confirmation(&binding)],
        vec![evidence("story-completion")],
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&canonical)
            .expect("canonical UTC timestamp permits")
            .disposition(),
        LifecycleDisposition::Permitted
    );
}

#[test]
fn multiple_confirmations_fail_closed_even_when_each_is_individually_valid() {
    let (story_id, complete, binding) = pending_story_confirmation("STORY-CONFIRM-MULTIPLE");
    let confirmed = input(
        LifecycleCommand::CompleteStory { story_id },
        ProcessPhase::Completed,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        vec![complete],
        None,
        vec![confirmation(&binding), confirmation(&binding)],
        vec![evidence("story-completion")],
        Vec::new(),
    );

    let error = LifecycleEngine::plan(&confirmed).expect_err("multiple confirmations fail closed");
    assert_eq!(
        error.code,
        ae_sdd_contracts::ControlPlaneErrorCode::ConfirmationMismatch
    );
}

#[test]
fn gate_evidence_is_required_for_policy_gates() {
    let no_evidence = input(
        LifecycleCommand::Transition {
            target_phase: ProcessPhase::Coding,
        },
        ProcessPhase::CodingProcess,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&no_evidence)
            .expect("missing evidence is a denied plan")
            .disposition(),
        LifecycleDisposition::Denied
    );

    let evidence_refs = ["G-00", "G-07", "G-CODEPLAN-SRC", "G-14", "G-08", "G-HTTP-1"]
        .into_iter()
        .map(evidence)
        .collect();
    let pending = input(
        LifecycleCommand::Transition {
            target_phase: ProcessPhase::Coding,
        },
        ProcessPhase::CodingProcess,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        Vec::new(),
        None,
        Vec::new(),
        evidence_refs,
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&pending)
            .expect("coding transition awaits approval")
            .disposition(),
        LifecycleDisposition::AwaitingConfirmation
    );
}

#[test]
fn daemon_gate_pass_satisfies_policy_without_file_evidence() {
    // RA-first: RouteSelected is now the second step (after RequirementAnalyzed)
    // and requires only G-RA-FLOW-VIOLATION. The transition starts from
    // RequirementAnalyzed, not Initialized.
    let input = input(
        LifecycleCommand::Transition {
            target_phase: ProcessPhase::RouteSelected,
        },
        ProcessPhase::RequirementAnalyzed,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Dr,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .with_passed_gate_ids(vec![
        VerificationId::new("G-RA-FLOW-VIOLATION").expect("Gate id"),
    ])
    .expect("bounded daemon Gate pass");

    assert_eq!(
        LifecycleEngine::plan(&input)
            .expect("daemon Gate pass is valid lifecycle authority")
            .disposition(),
        LifecycleDisposition::Permitted
    );
}

#[test]
fn file_lock_owner_ttl_expiry_and_malformed_metadata_fail_closed() {
    let path = path("crates/ae-sdd-lifecycle/src/lib.rs");
    let owner = session(1);
    let other = session(2);
    let active_other = FileLockSnapshot {
        path: path.clone(),
        owner_session_id: other,
        expires_at_unix_ms: NOW + 30_000,
        metadata_valid: true,
    };
    let blocked = lock_input(path.clone(), owner, NOW + 60_000, vec![active_other]);
    assert_eq!(
        LifecycleEngine::plan(&blocked)
            .expect("lock conflict is denied")
            .disposition(),
        LifecycleDisposition::Denied
    );

    let expired_other = FileLockSnapshot {
        path: path.clone(),
        owner_session_id: other,
        expires_at_unix_ms: NOW,
        metadata_valid: true,
    };
    let takeover = lock_input(path.clone(), owner, NOW + 60_000, vec![expired_other]);
    assert_eq!(
        LifecycleEngine::plan(&takeover)
            .expect("expired lock can be replaced")
            .disposition(),
        LifecycleDisposition::Permitted
    );

    let active_owner = FileLockSnapshot {
        path: path.clone(),
        owner_session_id: owner,
        expires_at_unix_ms: NOW + 30_000,
        metadata_valid: true,
    };
    let renewal = lock_input(path.clone(), owner, NOW + 60_000, vec![active_owner]);
    assert_eq!(
        LifecycleEngine::plan(&renewal)
            .expect("same owner can renew")
            .disposition(),
        LifecycleDisposition::Permitted
    );

    let malformed = FileLockSnapshot {
        path: path.clone(),
        owner_session_id: other,
        expires_at_unix_ms: NOW + 30_000,
        metadata_valid: false,
    };
    let corrupt = lock_input(path.clone(), owner, NOW + 60_000, vec![malformed]);
    assert_eq!(
        LifecycleEngine::plan(&corrupt)
            .expect("malformed metadata is denied")
            .disposition(),
        LifecycleDisposition::Denied
    );

    let invalid_ttl = lock_input(path, owner, NOW, Vec::new());
    assert_eq!(
        LifecycleEngine::plan(&invalid_ttl)
            .expect("non-positive TTL is denied")
            .disposition(),
        LifecycleDisposition::Denied
    );
}

#[test]
fn file_lock_release_is_owner_only() {
    let lock_path = path("crates/ae-sdd-lifecycle/src/engine.rs");
    let owner = session(11);
    let other = session(12);
    let snapshot = FileLockSnapshot {
        path: lock_path.clone(),
        owner_session_id: owner,
        expires_at_unix_ms: NOW + 30_000,
        metadata_valid: true,
    };
    let wrong_owner = input(
        LifecycleCommand::ReleaseFileLock {
            path: lock_path.clone(),
            owner_session_id: other,
        },
        ProcessPhase::Coding,
        None,
        AgentRole::Task,
        WorkScale::Large,
        DesignRoute::Story,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        vec![snapshot.clone()],
    );
    assert_eq!(
        LifecycleEngine::plan(&wrong_owner)
            .expect("wrong owner is denied")
            .disposition(),
        LifecycleDisposition::Denied
    );

    let release = input(
        LifecycleCommand::ReleaseFileLock {
            path: lock_path,
            owner_session_id: owner,
        },
        ProcessPhase::Coding,
        None,
        AgentRole::Task,
        WorkScale::Large,
        DesignRoute::Story,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        vec![snapshot],
    );
    let plan = LifecycleEngine::plan(&release).expect("owner release plans");
    assert_eq!(plan.disposition(), LifecycleDisposition::Permitted);
    assert_eq!(
        plan.intents()[0].operation,
        ae_sdd_contracts::MutationOperation::Delete
    );
}

#[test]
fn archive_requires_terminal_state_and_bound_confirmation() {
    let active = input(
        LifecycleCommand::ArchiveWorkItem,
        ProcessPhase::CodeReviewed,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&active)
            .expect("nonterminal archive is denied")
            .disposition(),
        LifecycleDisposition::Denied
    );

    let terminal = input(
        LifecycleCommand::ArchiveWorkItem,
        ProcessPhase::Completed,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(
        LifecycleEngine::plan(&terminal)
            .expect("terminal archive awaits confirmation")
            .disposition(),
        LifecycleDisposition::AwaitingConfirmation
    );
}

#[test]
fn unordered_child_and_evidence_inputs_have_one_canonical_plan_digest() {
    let first_id = story_id("STORY-ORDER-001");
    let second_id = story_id("STORY-ORDER-002");
    let first = story(first_id.clone(), ProcessPhase::Completed, 0, 1, true);
    let second = story(second_id.clone(), ProcessPhase::Completed, 0, 2, true);
    let prd = PrdSummary {
        prd_id: PrdId::new("PRD-ORDER-001").expect("PRD id"),
        registered_story_ids: vec![first_id.clone(), second_id.clone()],
        completed_story_ids: vec![second_id, first_id],
        dependencies_satisfied: true,
        residual_risks_cleared: true,
        gates_passed: true,
        review_passed: true,
    };
    let command = LifecycleCommand::CompletePrd {
        prd_id: prd.prd_id.clone(),
    };
    let left = input(
        command.clone(),
        ProcessPhase::Completed,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        vec![first.clone(), second.clone()],
        Some(prd.clone()),
        Vec::new(),
        vec![evidence("proof-a"), evidence("proof-b")],
        Vec::new(),
    );
    let right = input(
        command,
        ProcessPhase::Completed,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        vec![second, first],
        Some(PrdSummary {
            registered_story_ids: prd.registered_story_ids.into_iter().rev().collect(),
            completed_story_ids: prd.completed_story_ids.into_iter().rev().collect(),
            ..prd
        }),
        Vec::new(),
        vec![evidence("proof-b"), evidence("proof-a")],
        Vec::new(),
    );

    let left_plan = LifecycleEngine::plan(&left).expect("left plan");
    let right_plan = LifecycleEngine::plan(&right).expect("right plan");
    assert_eq!(left_plan.plan_digest(), right_plan.plan_digest());
    assert_eq!(
        confirmation_binding(&left_plan),
        confirmation_binding(&right_plan)
    );
}

#[allow(clippy::too_many_arguments)]
fn input(
    command: LifecycleCommand,
    phase: ProcessPhase,
    paused_from: Option<ProcessPhase>,
    actor_role: AgentRole,
    scale: WorkScale,
    design_route: DesignRoute,
    stories: Vec<StorySummary>,
    prd: Option<PrdSummary>,
    confirmations: Vec<ConfirmationRef>,
    evidence_refs: Vec<EvidenceRef>,
    file_locks: Vec<FileLockSnapshot>,
) -> LifecycleInput {
    LifecycleInput::new(
        SchemaVersion::V1,
        command,
        ProcessSnapshot::new(
            SchemaVersion::V1,
            WorkItemId::new("WORK-ITEM-001").expect("work item"),
            phase,
            paused_from,
            StateRevision::new(7),
            ArtifactDigest::digest(b"authoritative state"),
        ),
        StateRevision::new(7),
        actor_role,
        scale,
        design_route,
        stories,
        prd,
        confirmations,
        evidence_refs,
        file_locks,
        NOW,
        InputFingerprint::digest(b"stable lifecycle input"),
    )
    .expect("valid lifecycle input")
}

fn story(
    story_id: StoryId,
    phase: ProcessPhase,
    pending_outputs: u16,
    coding_round: u32,
    registered: bool,
) -> StorySummary {
    StorySummary {
        story_id,
        phase,
        current_step: ReasonCode::new("story.step").expect("reason"),
        pending_outputs,
        coding_round,
        registered,
    }
}

fn story_id(value: &str) -> StoryId {
    StoryId::new(value).expect("story id")
}

fn path(value: &str) -> ProjectRelativePath {
    ProjectRelativePath::new(value).expect("project-relative path")
}

fn session(seed: u128) -> SessionId {
    SessionId::from_uuid(Uuid::from_u128(seed))
}

fn evidence(verification_id: &str) -> EvidenceRef {
    EvidenceRef::new(
        EvidenceId::new(format!("evidence-{verification_id}")).expect("evidence id"),
        VerificationId::new(verification_id).expect("verification id"),
        path(&format!(".ae-sdd/evidence/{verification_id}.json")),
        EvidenceDigest::digest(verification_id.as_bytes()),
        32,
    )
}

fn confirmation(binding: &str) -> ConfirmationRef {
    ConfirmationRef {
        confirmation_id: binding.to_owned(),
        approved_by: "user:owner".to_owned(),
        approved_at: "2026-07-24T00:00:00Z".to_owned(),
    }
}

fn pending_story_confirmation(story: &str) -> (StoryId, StorySummary, String) {
    let story_id = story_id(story);
    let complete = self::story(story_id.clone(), ProcessPhase::Completed, 0, 1, true);
    let pending = input(
        LifecycleCommand::CompleteStory {
            story_id: story_id.clone(),
        },
        ProcessPhase::Completed,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        vec![complete.clone()],
        None,
        Vec::new(),
        vec![evidence("story-completion")],
        Vec::new(),
    );
    let plan = LifecycleEngine::plan(&pending).expect("pending confirmation plan");
    (story_id, complete, confirmation_binding(&plan))
}

fn confirmation_binding(plan: &ae_sdd_contracts::LifecyclePlan) -> String {
    let value = serde_json::to_value(plan).expect("plan wire");
    value["confirmationRequirement"]["bindingDigest"]
        .as_str()
        .expect("binding digest")
        .to_owned()
}

fn lock_input(
    path: ProjectRelativePath,
    owner: SessionId,
    expires_at_unix_ms: u64,
    locks: Vec<FileLockSnapshot>,
) -> LifecycleInput {
    input(
        LifecycleCommand::AcquireFileLock {
            path,
            owner_session_id: owner,
            expires_at_unix_ms,
        },
        ProcessPhase::Coding,
        None,
        AgentRole::Task,
        WorkScale::Large,
        DesignRoute::Story,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        locks,
    )
}

#[test]
fn plan_wire_has_no_unbounded_or_adapter_specific_payload() {
    let plan = LifecycleEngine::plan(&input(
        LifecycleCommand::Pause,
        ProcessPhase::Coding,
        None,
        AgentRole::Root,
        WorkScale::Large,
        DesignRoute::Story,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ))
    .expect("pause plan");
    let wire: Value = serde_json::to_value(plan).expect("plan wire");
    let encoded = wire.to_string();
    for forbidden in ["sqlite", "adapter", "sqlHandle", "daemon", "absolutePath"] {
        assert!(!encoded.contains(forbidden), "wire leaked {forbidden}");
    }
}

#[test]
fn exhaustive_phase_role_scale_route_matrix_is_total_and_replay_stable() {
    let phases = [
        ProcessPhase::Initialized,
        ProcessPhase::RouteSelected,
        ProcessPhase::RequirementAnalyzed,
        ProcessPhase::DrGenerated,
        ProcessPhase::StoryGenerated,
        ProcessPhase::TestcaseGenerated,
        ProcessPhase::CodingProcess,
        ProcessPhase::Coding,
        ProcessPhase::TestRunning,
        ProcessPhase::CodeReviewed,
        ProcessPhase::Completed,
        ProcessPhase::Paused,
    ];
    let roles = [
        AgentRole::Root,
        AgentRole::Series,
        AgentRole::Task,
        AgentRole::Reviewer,
    ];
    let scales = [
        WorkScale::Large,
        WorkScale::Medium,
        WorkScale::Small,
        WorkScale::Micro,
    ];
    let routes = [DesignRoute::Dr, DesignRoute::Story, DesignRoute::CodingPlan];

    let mut cases = 0_usize;
    for phase in phases {
        for role in roles {
            for scale in scales {
                for route in routes {
                    let input = matrix_input(phase, role, scale, route);
                    let first = catch_unwind(AssertUnwindSafe(|| LifecycleEngine::plan(&input)))
                        .expect("lifecycle planner must not panic for a typed matrix input");
                    let replay = catch_unwind(AssertUnwindSafe(|| LifecycleEngine::plan(&input)))
                        .expect("lifecycle replay must not panic");
                    assert_eq!(first, replay);
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 12 * 4 * 4 * 3);
}

#[test]
fn file_lock_ttl_boundary_matrix_is_total_and_replay_stable() {
    for (ttl_ms, expected) in [
        (0, LifecycleDisposition::Denied),
        (1, LifecycleDisposition::Permitted),
        (
            ae_sdd_lifecycle::MAX_FILE_LOCK_TTL_MS,
            LifecycleDisposition::Permitted,
        ),
        (
            ae_sdd_lifecycle::MAX_FILE_LOCK_TTL_MS + 1,
            LifecycleDisposition::Denied,
        ),
    ] {
        let input = lock_input(
            path("crates/ae-sdd-lifecycle/src/lib.rs"),
            session(100 + u128::from(ttl_ms)),
            NOW + ttl_ms,
            Vec::new(),
        );
        let first = catch_unwind(AssertUnwindSafe(|| LifecycleEngine::plan(&input)))
            .expect("TTL boundary planning must not panic")
            .expect("TTL boundary is represented as a lifecycle plan");
        let replay = LifecycleEngine::plan(&input).expect("TTL replay plans");
        assert_eq!(first, replay);
        assert_eq!(first.disposition(), expected);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn randomized_phase_role_scale_route_replay_is_total(
        phase_index in 0_usize..12,
        role_index in 0_usize..4,
        scale_index in 0_usize..4,
        route_index in 0_usize..3,
    ) {
        let phase = phases()[phase_index];
        let role = roles()[role_index];
        let scale = scales()[scale_index];
        let route = routes()[route_index];
        let input = matrix_input(phase, role, scale, route);

        let first = catch_unwind(AssertUnwindSafe(|| LifecycleEngine::plan(&input)));
        prop_assert!(first.is_ok());
        let replay = catch_unwind(AssertUnwindSafe(|| LifecycleEngine::plan(&input)));
        prop_assert!(replay.is_ok());
        prop_assert_eq!(first.ok(), replay.ok());
    }

    #[test]
    fn randomized_ttl_boundary_replay_is_total(ttl_index in 0_usize..4) {
        let ttl_values = [
            0,
            1,
            ae_sdd_lifecycle::MAX_FILE_LOCK_TTL_MS,
            ae_sdd_lifecycle::MAX_FILE_LOCK_TTL_MS + 1,
        ];
        let expected = [
            LifecycleDisposition::Denied,
            LifecycleDisposition::Permitted,
            LifecycleDisposition::Permitted,
            LifecycleDisposition::Denied,
        ];
        let ttl_ms = ttl_values[ttl_index];
        let input = lock_input(
            path("crates/ae-sdd-lifecycle/src/lib.rs"),
            session(500 + u128::from(ttl_ms)),
            NOW + ttl_ms,
            Vec::new(),
        );

        let first = catch_unwind(AssertUnwindSafe(|| LifecycleEngine::plan(&input)));
        prop_assert!(first.is_ok());
        let replay = catch_unwind(AssertUnwindSafe(|| LifecycleEngine::plan(&input)));
        prop_assert!(replay.is_ok());
        prop_assert_eq!(&first.as_ref().ok(), &replay.as_ref().ok());
        prop_assert_eq!(
            first
                .expect("checked above")
                .expect("TTL boundary is represented as a plan")
                .disposition(),
            expected[ttl_index]
        );
    }

    #[test]
    fn confirmation_count_and_field_boundaries_are_total_and_replay_stable(
        count in 0_usize..=3,
        id_case in 0_usize..5,
        approver_case in 0_usize..5,
        timestamp_case in 0_usize..4,
    ) {
        let (story_id, complete, binding) = pending_story_confirmation("STORY-PROP-CONFIRM");
        let confirmation_id = match id_case {
            0 => binding.clone(),
            1 => format!("sha256:{binding}"),
            2 => "0".repeat(64),
            3 => format!("{binding}\0"),
            _ => "x".repeat(MAX_CONFIRMATION_ID_BYTES + 1),
        };
        let approved_by = match approver_case {
            0 => String::new(),
            1 => "x".to_owned(),
            2 => "x".repeat(MAX_CONFIRMATION_APPROVED_BY_BYTES),
            3 => "x".repeat(MAX_CONFIRMATION_APPROVED_BY_BYTES + 1),
            _ => "user:\0owner".to_owned(),
        };
        let approved_at = match timestamp_case {
            0 => "2026-07-24T00:00:00Z".to_owned(),
            1 => "2026-07-24T08:00:00+08:00".to_owned(),
            2 => "2026-07-24T00:00:00Z\n".to_owned(),
            _ => "x".repeat(MAX_CONFIRMATION_APPROVED_AT_BYTES + 1),
        };
        let approval = ConfirmationRef {
            confirmation_id,
            approved_by,
            approved_at,
        };
        let confirmations = (0..count).map(|_| approval.clone()).collect();
        let input = input(
            LifecycleCommand::CompleteStory { story_id },
            ProcessPhase::Completed,
            None,
            AgentRole::Root,
            WorkScale::Large,
            DesignRoute::Story,
            vec![complete],
            None,
            confirmations,
            vec![evidence("story-completion")],
            Vec::new(),
        );

        let first = catch_unwind(AssertUnwindSafe(|| LifecycleEngine::plan(&input)));
        prop_assert!(first.is_ok());
        let replay = catch_unwind(AssertUnwindSafe(|| LifecycleEngine::plan(&input)));
        prop_assert!(replay.is_ok());
        prop_assert_eq!(&first.as_ref().ok(), &replay.as_ref().ok());

        let result = first.expect("checked above");
        if count == 0 {
            prop_assert_eq!(
                result.expect("missing confirmation is a plan").disposition(),
                LifecycleDisposition::AwaitingConfirmation
            );
        } else if count == 1
            && id_case <= 1
            && matches!(approver_case, 1 | 2)
            && timestamp_case == 0
        {
            prop_assert_eq!(
                result.expect("valid confirmation permits").disposition(),
                LifecycleDisposition::Permitted
            );
        } else {
            prop_assert_eq!(
                result.expect_err("invalid or multiple confirmation fails closed").code,
                ae_sdd_contracts::ControlPlaneErrorCode::ConfirmationMismatch
            );
        }
    }
}

fn matrix_input(
    phase: ProcessPhase,
    role: AgentRole,
    scale: WorkScale,
    route: DesignRoute,
) -> LifecycleInput {
    input(
        LifecycleCommand::Transition {
            target_phase: ProcessPhase::Completed,
        },
        phase,
        (phase == ProcessPhase::Paused).then_some(ProcessPhase::Coding),
        role,
        scale,
        route,
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

const fn phases() -> [ProcessPhase; 12] {
    [
        ProcessPhase::Initialized,
        ProcessPhase::RouteSelected,
        ProcessPhase::RequirementAnalyzed,
        ProcessPhase::DrGenerated,
        ProcessPhase::StoryGenerated,
        ProcessPhase::TestcaseGenerated,
        ProcessPhase::CodingProcess,
        ProcessPhase::Coding,
        ProcessPhase::TestRunning,
        ProcessPhase::CodeReviewed,
        ProcessPhase::Completed,
        ProcessPhase::Paused,
    ]
}

const fn roles() -> [AgentRole; 4] {
    [
        AgentRole::Root,
        AgentRole::Series,
        AgentRole::Task,
        AgentRole::Reviewer,
    ]
}

const fn scales() -> [WorkScale; 4] {
    [
        WorkScale::Large,
        WorkScale::Medium,
        WorkScale::Small,
        WorkScale::Micro,
    ]
}

const fn routes() -> [DesignRoute; 3] {
    [DesignRoute::Dr, DesignRoute::Story, DesignRoute::CodingPlan]
}
