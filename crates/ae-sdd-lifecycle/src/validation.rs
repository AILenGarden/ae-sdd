use std::collections::BTreeSet;

use ae_sdd_contracts::{
    ControlPlaneError, LifecycleCommand, LifecycleInput, MessageKey, MutationOperation, PrdId,
    ReasonCode, Remediation,
};
use ae_sdd_domain::{ArtifactDigest, ProcessPhase, ProjectRelativePath, StoryId};
use ae_sdd_policy::{RoleOperation, RolePolicy, TransitionContext, TransitionPolicy};

use crate::{
    engine::{MAX_FILE_LOCK_TTL_MS, invariant_error},
    projection::LifecycleProjection,
};

#[derive(Clone, Debug)]
pub(crate) enum TargetSpec {
    WorkItem,
    Story(StoryId),
    Prd(PrdId),
    File(ProjectRelativePath),
}

#[derive(Clone, Debug)]
pub(crate) enum EvidenceRequirement {
    None,
    Any,
    Gates(Vec<&'static str>),
}

#[derive(Clone, Debug)]
pub(crate) struct AuthorizedCommand {
    pub(crate) target: TargetSpec,
    pub(crate) operation: MutationOperation,
    pub(crate) expected_digest: Option<ArtifactDigest>,
    pub(crate) event_kind: &'static str,
    pub(crate) confirmation_required: bool,
    pub(crate) evidence_requirement: EvidenceRequirement,
}

pub(crate) enum ValidationFailure {
    Denied(Vec<Remediation>),
    Contract(ControlPlaneError),
}

impl From<ControlPlaneError> for ValidationFailure {
    fn from(error: ControlPlaneError) -> Self {
        Self::Contract(error)
    }
}

pub(crate) fn authorize(
    input: &LifecycleInput,
    projection: &LifecycleProjection,
) -> Result<AuthorizedCommand, ValidationFailure> {
    match input.command() {
        LifecycleCommand::Transition { target_phase } => authorize_transition(
            input,
            projection,
            *target_phase,
            "lifecycle.phase-transitioned",
        ),
        LifecycleCommand::Pause => {
            authorize_transition(input, projection, ProcessPhase::Paused, "lifecycle.paused")
        }
        LifecycleCommand::Resume => {
            let Some(target) = input.snapshot().paused_from else {
                return Err(one_remediation(
                    "lifecycle.resume-source-missing",
                    "lifecycle.resume-source-missing",
                ));
            };
            authorize_transition(input, projection, target, "lifecycle.resumed")
        }
        LifecycleCommand::BindStory {
            story_id,
            document_path,
        } => {
            authorize_root_mutation(input)?;
            let Some(summary) = story_summary(input, story_id) else {
                return Err(one_remediation(
                    "lifecycle.story-not-registered",
                    "lifecycle.story-not-registered",
                ));
            };
            if !summary.registered || !story_path_matches(story_id, document_path) {
                return Err(one_remediation(
                    "lifecycle.story-binding-invalid",
                    "lifecycle.story-binding-invalid",
                ));
            }
            Ok(AuthorizedCommand {
                target: TargetSpec::Story(story_id.clone()),
                operation: MutationOperation::Replace,
                expected_digest: Some(input.snapshot().state_digest),
                event_kind: "lifecycle.story-bound",
                confirmation_required: false,
                evidence_requirement: EvidenceRequirement::None,
            })
        }
        LifecycleCommand::CompleteStory { story_id } => {
            authorize_root_mutation(input)?;
            let Some(summary) = story_summary(input, story_id) else {
                return Err(one_remediation(
                    "lifecycle.story-not-registered",
                    "lifecycle.story-not-registered",
                ));
            };
            if !summary.is_complete() {
                return Err(one_remediation(
                    "lifecycle.story-incomplete",
                    "lifecycle.story-incomplete",
                ));
            }
            Ok(AuthorizedCommand {
                target: TargetSpec::Story(story_id.clone()),
                operation: MutationOperation::Replace,
                expected_digest: Some(input.snapshot().state_digest),
                event_kind: "lifecycle.story-completed",
                confirmation_required: true,
                evidence_requirement: EvidenceRequirement::Any,
            })
        }
        LifecycleCommand::CompletePrd { prd_id } => {
            authorize_root_mutation(input)?;
            validate_prd_completion(input, prd_id)?;
            Ok(AuthorizedCommand {
                target: TargetSpec::Prd(prd_id.clone()),
                operation: MutationOperation::Replace,
                expected_digest: Some(input.snapshot().state_digest),
                event_kind: "lifecycle.prd-completed",
                confirmation_required: true,
                evidence_requirement: EvidenceRequirement::Any,
            })
        }
        LifecycleCommand::AcquireFileLock {
            path,
            owner_session_id,
            expires_at_unix_ms,
        } => {
            authorize_lock_mutation(input)?;
            let operation = validate_lock_acquire(
                input,
                projection,
                path,
                *owner_session_id,
                *expires_at_unix_ms,
            )?;
            Ok(AuthorizedCommand {
                target: TargetSpec::File(path.clone()),
                operation,
                expected_digest: None,
                event_kind: "lifecycle.file-lock-acquired",
                confirmation_required: false,
                evidence_requirement: EvidenceRequirement::None,
            })
        }
        LifecycleCommand::ReleaseFileLock {
            path,
            owner_session_id,
        } => {
            authorize_lock_mutation(input)?;
            validate_lock_release(projection, path, *owner_session_id)?;
            Ok(AuthorizedCommand {
                target: TargetSpec::File(path.clone()),
                operation: MutationOperation::Delete,
                expected_digest: None,
                event_kind: "lifecycle.file-lock-released",
                confirmation_required: false,
                evidence_requirement: EvidenceRequirement::None,
            })
        }
        LifecycleCommand::ArchiveWorkItem => {
            authorize_root_mutation(input)?;
            if input.snapshot().phase != ProcessPhase::Completed {
                return Err(one_remediation(
                    "lifecycle.complete-before-archive",
                    "lifecycle.complete-before-archive",
                ));
            }
            Ok(AuthorizedCommand {
                target: TargetSpec::WorkItem,
                operation: MutationOperation::Replace,
                expected_digest: Some(input.snapshot().state_digest),
                event_kind: "lifecycle.work-item-archived",
                confirmation_required: true,
                evidence_requirement: EvidenceRequirement::None,
            })
        }
    }
}

pub(crate) fn validate_evidence(
    authorized: &AuthorizedCommand,
    projection: &LifecycleProjection,
) -> Result<(), ValidationFailure> {
    let mut evidence_ids = BTreeSet::new();
    for item in &projection.evidence {
        if item.evidence_id.is_empty()
            || item.verification_id.is_empty()
            || item.path.is_empty()
            || item.digest.len() != 64
            || !evidence_ids.insert(item.evidence_id.as_str())
        {
            return Err(one_remediation(
                "lifecycle.evidence-invalid",
                "lifecycle.evidence-invalid",
            ));
        }
    }

    match &authorized.evidence_requirement {
        EvidenceRequirement::None => Ok(()),
        EvidenceRequirement::Any if projection.evidence.is_empty() => Err(one_remediation(
            "lifecycle.evidence-required",
            "lifecycle.evidence-required",
        )),
        EvidenceRequirement::Any => Ok(()),
        EvidenceRequirement::Gates(gates) => {
            let present: BTreeSet<&str> = projection
                .evidence
                .iter()
                .map(|item| item.verification_id.as_str())
                .collect();
            let missing: Vec<_> = gates
                .iter()
                .copied()
                .filter(|gate| !present.contains(*gate))
                .collect();
            if missing.is_empty() {
                Ok(())
            } else {
                let remediation = missing
                    .into_iter()
                    .map(|gate| {
                        remediation(
                            &format!("lifecycle.evidence.{gate}"),
                            "lifecycle.gate-evidence-required",
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Err(ValidationFailure::Denied(remediation))
            }
        }
    }
}

fn authorize_transition(
    input: &LifecycleInput,
    projection: &LifecycleProjection,
    target: ProcessPhase,
    event_kind: &'static str,
) -> Result<AuthorizedCommand, ValidationFailure> {
    let permit = TransitionPolicy::authorize(TransitionContext {
        actor_role: input.actor_role(),
        current: input.snapshot().phase,
        target,
        scale: projection.scale,
        design_route: projection.design_route,
        paused_from: input.snapshot().paused_from,
    })
    .map_err(|_| one_remediation("lifecycle.transition-denied", "lifecycle.transition-denied"))?;
    if matches!(target, ProcessPhase::Completed) {
        let Some(completion) = projection.completion else {
            return Err(one_remediation(
                "lifecycle.completion-milestone-required",
                "lifecycle.completion-milestone-required",
            ));
        };
        if TransitionPolicy::authorize_completion(completion.effective_milestone()).is_err() {
            return Err(one_remediation(
                "lifecycle.completion-milestone-open",
                "lifecycle.completion-milestone-open",
            ));
        }
    }
    let required_gates = permit
        .required_gates()
        .iter()
        .map(|gate| gate.as_str())
        .collect();
    Ok(AuthorizedCommand {
        target: TargetSpec::WorkItem,
        operation: MutationOperation::Replace,
        expected_digest: Some(input.snapshot().state_digest),
        event_kind,
        confirmation_required: event_kind == "lifecycle.phase-transitioned"
            && matches!(target, ProcessPhase::Coding | ProcessPhase::Completed),
        evidence_requirement: EvidenceRequirement::Gates(required_gates),
    })
}

fn authorize_root_mutation(input: &LifecycleInput) -> Result<(), ValidationFailure> {
    RolePolicy::authorize(input.actor_role(), RoleOperation::RequestGlobalTransition).map_err(
        |_| {
            one_remediation(
                "lifecycle.root-role-required",
                "lifecycle.root-role-required",
            )
        },
    )
}

fn authorize_lock_mutation(input: &LifecycleInput) -> Result<(), ValidationFailure> {
    RolePolicy::authorize(input.actor_role(), RoleOperation::ManageOwnLease)
        .map_err(|_| one_remediation("lifecycle.lock-role-denied", "lifecycle.lock-role-denied"))
}

fn story_summary<'a>(
    input: &'a LifecycleInput,
    story_id: &StoryId,
) -> Option<&'a ae_sdd_contracts::StorySummary> {
    input
        .story_summaries()
        .iter()
        .find(|summary| &summary.story_id == story_id)
}

fn story_path_matches(story_id: &StoryId, document_path: &ProjectRelativePath) -> bool {
    document_path.as_str() == format!("ae-sdd-doc/Story/{story_id}.md")
}

fn validate_prd_completion(
    input: &LifecycleInput,
    requested_prd_id: &PrdId,
) -> Result<(), ValidationFailure> {
    let Some(prd) = input.prd_summary() else {
        return Err(one_remediation(
            "lifecycle.prd-summary-required",
            "lifecycle.prd-summary-required",
        ));
    };
    if &prd.prd_id != requested_prd_id {
        return Err(one_remediation(
            "lifecycle.prd-identity-mismatch",
            "lifecycle.prd-identity-mismatch",
        ));
    }

    let registered: BTreeSet<_> = prd.registered_story_ids.iter().collect();
    let completed: BTreeSet<_> = prd.completed_story_ids.iter().collect();
    let child_sets_valid = !registered.is_empty()
        && registered.len() == prd.registered_story_ids.len()
        && completed.len() == prd.completed_story_ids.len()
        && registered == completed;
    if !child_sets_valid {
        return Err(one_remediation(
            "lifecycle.prd-children-incomplete",
            "lifecycle.prd-children-incomplete",
        ));
    }
    if registered
        .iter()
        .any(|story_id| story_summary(input, story_id).is_none_or(|summary| !summary.is_complete()))
    {
        return Err(one_remediation(
            "lifecycle.prd-child-state-incomplete",
            "lifecycle.prd-child-state-incomplete",
        ));
    }

    let mut remediation_items = Vec::new();
    if !prd.dependencies_satisfied {
        remediation_items.push(remediation(
            "lifecycle.prd-dependencies-incomplete",
            "lifecycle.prd-dependencies-incomplete",
        )?);
    }
    if !prd.residual_risks_cleared {
        remediation_items.push(remediation(
            "lifecycle.prd-risks-incomplete",
            "lifecycle.prd-risks-incomplete",
        )?);
    }
    if !prd.gates_passed {
        remediation_items.push(remediation(
            "lifecycle.prd-gates-incomplete",
            "lifecycle.prd-gates-incomplete",
        )?);
    }
    if !prd.review_passed {
        remediation_items.push(remediation(
            "lifecycle.prd-review-incomplete",
            "lifecycle.prd-review-incomplete",
        )?);
    }
    if remediation_items.is_empty() {
        Ok(())
    } else {
        Err(ValidationFailure::Denied(remediation_items))
    }
}

fn validate_lock_snapshot(projection: &LifecycleProjection) -> Result<(), ValidationFailure> {
    let mut paths = BTreeSet::new();
    if projection
        .file_locks
        .iter()
        .any(|lock| !lock.metadata_valid || !paths.insert(lock.path.as_str()))
    {
        return Err(one_remediation(
            "lifecycle.file-lock-snapshot-invalid",
            "lifecycle.file-lock-snapshot-invalid",
        ));
    }
    Ok(())
}

fn validate_lock_acquire(
    input: &LifecycleInput,
    projection: &LifecycleProjection,
    path: &ProjectRelativePath,
    owner: ae_sdd_domain::SessionId,
    expires_at_unix_ms: u64,
) -> Result<MutationOperation, ValidationFailure> {
    validate_lock_snapshot(projection)?;
    let now = input.evaluation_unix_ms();
    let Some(ttl_ms) = expires_at_unix_ms.checked_sub(now) else {
        return Err(one_remediation(
            "lifecycle.file-lock-ttl-invalid",
            "lifecycle.file-lock-ttl-invalid",
        ));
    };
    if ttl_ms == 0 || ttl_ms > MAX_FILE_LOCK_TTL_MS {
        return Err(one_remediation(
            "lifecycle.file-lock-ttl-invalid",
            "lifecycle.file-lock-ttl-invalid",
        ));
    }

    let Some(existing) = projection.file_locks.iter().find(|lock| &lock.path == path) else {
        return Ok(MutationOperation::Create);
    };
    if existing.expires_at_unix_ms > now && existing.owner_session_id != owner {
        return Err(one_remediation(
            "lifecycle.file-lock-owned",
            "lifecycle.file-lock-owned",
        ));
    }
    Ok(MutationOperation::Replace)
}

fn validate_lock_release(
    projection: &LifecycleProjection,
    path: &ProjectRelativePath,
    owner: ae_sdd_domain::SessionId,
) -> Result<(), ValidationFailure> {
    validate_lock_snapshot(projection)?;
    let Some(existing) = projection.file_locks.iter().find(|lock| &lock.path == path) else {
        return Err(one_remediation(
            "lifecycle.file-lock-missing",
            "lifecycle.file-lock-missing",
        ));
    };
    if existing.owner_session_id != owner {
        return Err(one_remediation(
            "lifecycle.file-lock-owner-mismatch",
            "lifecycle.file-lock-owner-mismatch",
        ));
    }
    Ok(())
}

fn one_remediation(code: &str, message_key: &str) -> ValidationFailure {
    match remediation(code, message_key) {
        Ok(remediation) => ValidationFailure::Denied(vec![remediation]),
        Err(error) => ValidationFailure::Contract(error),
    }
}

fn remediation(code: &str, message_key: &str) -> Result<Remediation, ControlPlaneError> {
    Ok(Remediation {
        code: ReasonCode::new(code).map_err(|_| invariant_error(None))?,
        message_key: MessageKey::new(message_key).map_err(|_| invariant_error(None))?,
    })
}
