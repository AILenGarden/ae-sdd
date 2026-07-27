use ae_sdd_contracts::{
    ConfirmationRequirement, LifecycleCommand, LifecycleDisposition, LifecycleInput,
    MutationIntent, MutationOperation, Remediation,
};
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, DecisionDigest, DesignRoute, ProcessPhase, WorkScale,
};

use crate::projection::{ConfirmationProjection, EvidenceProjection, LifecycleProjection};

pub(crate) fn action_binding(
    input: &LifecycleInput,
    projection: &LifecycleProjection,
) -> DecisionDigest {
    let mut encoder = Encoder::new(b"ae-sdd-lifecycle-action/v1");
    let snapshot = input.snapshot();
    encoder.string(snapshot.work_item_id.as_str());
    encoder.phase(snapshot.phase);
    encoder.optional_phase(snapshot.paused_from);
    encoder.u64(snapshot.state_revision.get());
    encoder.fixed(snapshot.state_digest.as_bytes());
    encoder.fixed(input.input_fingerprint().as_bytes());
    encoder.role(input.actor_role());
    encoder.scale(projection.scale);
    encoder.route(projection.design_route);
    encode_command(&mut encoder, input.command());

    let mut stories = input.story_summaries().to_vec();
    stories.sort_by(|left, right| left.story_id.cmp(&right.story_id));
    encoder.usize(stories.len());
    for story in stories {
        encoder.string(story.story_id.as_str());
        encoder.phase(story.phase);
        encoder.string(story.current_step.as_str());
        encoder.u64(u64::from(story.pending_outputs));
        encoder.u64(u64::from(story.coding_round));
        encoder.boolean(story.registered);
    }

    match input.prd_summary() {
        Some(prd) => {
            encoder.boolean(true);
            encoder.string(prd.prd_id.as_str());
            let mut registered = prd.registered_story_ids.clone();
            registered.sort();
            encoder.usize(registered.len());
            for story_id in registered {
                encoder.string(story_id.as_str());
            }
            let mut completed = prd.completed_story_ids.clone();
            completed.sort();
            encoder.usize(completed.len());
            for story_id in completed {
                encoder.string(story_id.as_str());
            }
            encoder.boolean(prd.dependencies_satisfied);
            encoder.boolean(prd.residual_risks_cleared);
            encoder.boolean(prd.gates_passed);
            encoder.boolean(prd.review_passed);
        }
        None => encoder.boolean(false),
    }

    encode_evidence(&mut encoder, &projection.evidence);
    if matches!(
        input.command(),
        LifecycleCommand::AcquireFileLock { .. } | LifecycleCommand::ReleaseFileLock { .. }
    ) {
        encoder.u64(input.evaluation_unix_ms());
        let mut locks = projection.file_locks.clone();
        locks.sort_by(|left, right| {
            (
                left.path.as_str(),
                left.owner_session_id,
                left.expires_at_unix_ms,
                left.metadata_valid,
            )
                .cmp(&(
                    right.path.as_str(),
                    right.owner_session_id,
                    right.expires_at_unix_ms,
                    right.metadata_valid,
                ))
        });
        encoder.usize(locks.len());
        for lock in locks {
            encoder.string(lock.path.as_str());
            encoder.string(&lock.owner_session_id.to_string());
            encoder.u64(lock.expires_at_unix_ms);
            encoder.boolean(lock.metadata_valid);
        }
    }
    DecisionDigest::digest(encoder.finish())
}

pub(crate) fn plan_digest(
    binding: DecisionDigest,
    disposition: LifecycleDisposition,
    intents: &[MutationIntent],
    confirmation: &ConfirmationRequirement,
    confirmations: &[ConfirmationProjection],
    remediation: &[Remediation],
) -> DecisionDigest {
    let mut encoder = Encoder::new(b"ae-sdd-lifecycle-plan/v1");
    encoder.fixed(binding.as_bytes());
    encoder.byte(match disposition {
        LifecycleDisposition::Permitted => 1,
        LifecycleDisposition::Denied => 2,
        LifecycleDisposition::AwaitingConfirmation => 3,
    });
    encoder.boolean(confirmation.required);
    match &confirmation.reason_code {
        Some(reason) => {
            encoder.boolean(true);
            encoder.string(reason.as_str());
        }
        None => encoder.boolean(false),
    }
    encoder.fixed(confirmation.binding_digest.as_bytes());

    encoder.usize(intents.len());
    for intent in intents {
        encoder.string(intent.intent_id.as_str());
        encoder.string(intent.target.namespace.as_str());
        match &intent.target.relative_path {
            Some(path) => {
                encoder.boolean(true);
                encoder.string(path.as_str());
            }
            None => encoder.boolean(false),
        }
        match &intent.target.logical_key {
            Some(key) => {
                encoder.boolean(true);
                encoder.string(key.as_str());
            }
            None => encoder.boolean(false),
        }
        encoder.byte(operation_tag(intent.operation));
        encoder.u64(intent.expected_revision.get());
        match intent.expected_digest {
            Some(digest) => {
                encoder.boolean(true);
                encoder.fixed(digest.as_bytes());
            }
            None => encoder.boolean(false),
        }
        encoder.string(intent.event.kind.as_str());
        encoder.fixed(intent.event.payload_digest.as_bytes());
    }

    let mut confirmations = confirmations.to_vec();
    confirmations.sort();
    encoder.usize(confirmations.len());
    for item in confirmations {
        encoder.string(&item.confirmation_id);
        encoder.string(&item.approved_by);
        encoder.string(&item.approved_at);
    }
    encoder.usize(remediation.len());
    for item in remediation {
        encoder.string(item.code.as_str());
        encoder.string(item.message_key.as_str());
    }
    DecisionDigest::digest(encoder.finish())
}

pub(crate) fn event_payload_digest(binding: DecisionDigest, event_kind: &str) -> ArtifactDigest {
    let mut encoder = Encoder::new(b"ae-sdd-lifecycle-event/v1");
    encoder.fixed(binding.as_bytes());
    encoder.string(event_kind);
    ArtifactDigest::digest(encoder.finish())
}

fn encode_command(encoder: &mut Encoder, command: &LifecycleCommand) {
    match command {
        LifecycleCommand::Transition { target_phase } => {
            encoder.byte(1);
            encoder.phase(*target_phase);
        }
        LifecycleCommand::Pause => encoder.byte(2),
        LifecycleCommand::Resume => encoder.byte(3),
        LifecycleCommand::BindStory {
            story_id,
            document_path,
        } => {
            encoder.byte(4);
            encoder.string(story_id.as_str());
            encoder.string(document_path.as_str());
        }
        LifecycleCommand::CompleteStory { story_id } => {
            encoder.byte(5);
            encoder.string(story_id.as_str());
        }
        LifecycleCommand::CompletePrd { prd_id } => {
            encoder.byte(6);
            encoder.string(prd_id.as_str());
        }
        LifecycleCommand::AcquireFileLock {
            path,
            owner_session_id,
            expires_at_unix_ms,
        } => {
            encoder.byte(7);
            encoder.string(path.as_str());
            encoder.string(&owner_session_id.to_string());
            encoder.u64(*expires_at_unix_ms);
        }
        LifecycleCommand::ReleaseFileLock {
            path,
            owner_session_id,
        } => {
            encoder.byte(8);
            encoder.string(path.as_str());
            encoder.string(&owner_session_id.to_string());
        }
        LifecycleCommand::ArchiveWorkItem => encoder.byte(9),
    }
}

fn encode_evidence(encoder: &mut Encoder, evidence: &[EvidenceProjection]) {
    let mut evidence = evidence.to_vec();
    evidence.sort();
    encoder.usize(evidence.len());
    for item in evidence {
        encoder.string(&item.evidence_id);
        encoder.string(&item.verification_id);
        encoder.string(&item.path);
        encoder.string(&item.digest);
        encoder.u64(item.byte_length);
    }
}

const fn operation_tag(operation: MutationOperation) -> u8 {
    match operation {
        MutationOperation::Create => 1,
        MutationOperation::Replace => 2,
        MutationOperation::Delete => 3,
        MutationOperation::AppendEvent => 4,
    }
}

struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    fn new(tag: &[u8]) -> Self {
        let mut bytes = Vec::with_capacity(1024);
        bytes.extend_from_slice(tag);
        bytes.push(0);
        Self { bytes }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn byte(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn boolean(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn usize(&mut self, value: usize) {
        let encoded = u64::try_from(value).unwrap_or(u64::MAX);
        self.u64(encoded);
    }

    fn fixed(&mut self, value: &[u8; 32]) {
        self.bytes.extend_from_slice(value);
    }

    fn string(&mut self, value: &str) {
        self.usize(value.len());
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn optional_phase(&mut self, value: Option<ProcessPhase>) {
        match value {
            Some(phase) => {
                self.boolean(true);
                self.phase(phase);
            }
            None => self.boolean(false),
        }
    }

    fn phase(&mut self, value: ProcessPhase) {
        self.byte(match value {
            ProcessPhase::Initialized => 1,
            ProcessPhase::RouteSelected => 2,
            ProcessPhase::RequirementAnalyzed => 3,
            ProcessPhase::DrGenerated => 4,
            ProcessPhase::StoryGenerated => 5,
            ProcessPhase::TestcaseGenerated => 6,
            ProcessPhase::CodingProcess => 7,
            ProcessPhase::Coding => 8,
            ProcessPhase::TestRunning => 9,
            ProcessPhase::CodeReviewed => 10,
            ProcessPhase::Completed => 11,
            ProcessPhase::Paused => 12,
        });
    }

    fn role(&mut self, value: AgentRole) {
        self.byte(match value {
            AgentRole::Root => 1,
            AgentRole::Series => 2,
            AgentRole::Task => 3,
            AgentRole::Reviewer => 4,
        });
    }

    fn scale(&mut self, value: WorkScale) {
        self.byte(match value {
            WorkScale::Large => 1,
            WorkScale::Medium => 2,
            WorkScale::Small => 3,
            WorkScale::Micro => 4,
        });
    }

    fn route(&mut self, value: DesignRoute) {
        self.byte(match value {
            DesignRoute::Dr => 1,
            DesignRoute::Story => 2,
            DesignRoute::CodingPlan => 3,
        });
    }
}
