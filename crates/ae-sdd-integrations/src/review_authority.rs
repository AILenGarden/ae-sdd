use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use ae_sdd_contracts::review::{
    AttestedReviewerV2, ReviewAttemptId, ReviewAttemptV2, ReviewAuthorityRef, ReviewBatchId,
    ReviewBatchReceiptV2, ReviewBatchStatusV2, ReviewBatchV2, ReviewContributionOutcomeV2,
    ReviewExitReceiptV2, ReviewFinalProofKind, ReviewFinalProofV2, ReviewFinding,
    ReviewFindingSeverity, ReviewMutationId, ReviewProjectAuthorityV2, ReviewRemediationV2,
    ReviewRepairClass, ReviewSessionStatusV2, ReviewSessionV2, ReviewTier, ReviewTimestamp,
    ReviewerContributionV2, ReviewerSpecialty,
};
use ae_sdd_contracts::{
    BoundedText, EvidenceLedgerEventKind, EvidenceLedgerEventV1, IdempotencyKey, ReasonCode,
    ReviewId,
};
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, DelegationId, GateOutcome, InputFingerprint, PolicyDigest,
    SessionId, StoryId,
};
use ae_sdd_operations::{OperationName, ValidatedOperationRequest};
use ae_sdd_protocol::StableErrorCode;
use ae_sdd_review::{ReviewSupervisor, ReviewSupervisorError, dedup_findings};
use ae_sdd_runtime::{
    BusinessWorkspace, CurrentBootSessionReceipt, PersistencePort,
    RuntimeDelegationAttestationRecord, RuntimeDelegationRecord, RuntimeError, RuntimeIdentityKind,
    RuntimeJobRecord, RuntimeJobStatus, RuntimeResult, RuntimeSessionRecord, ScopedGrantWire,
    WireAgentRole,
};
use ae_sdd_store::UtcTimestamp;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::gate_source::{AuthoritativeGateRuntime, gate_result_json};
use crate::persistence::{
    ReviewAuthorityProjectionV2, ReviewProjectionWrite, load_review_authority_projection,
};

const DETERMINISTIC_REVIEW_GATES: [&str; 3] = ["G-CODEPLAN-SRC", "G-14", "G-08"];
const REVIEW_GATE_DEADLINE: Duration = Duration::from_secs(10);
const REVIEW_INPUT_FILE_LIMIT: usize = 20_000;
const REVIEW_INPUT_CONTENT_LIMIT: u64 = 8 * 1024 * 1024;
const REVIEW_MANIFEST_BYTE_LIMIT: usize = 1_048_576;
const REVIEW_EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".hermes",
    ".venv",
    "__pycache__",
    "dist",
    "node_modules",
    "target",
    "vendor",
];
const SPECIALTY_CAPABILITIES: [&str; 4] = [
    "review.specialty.general",
    "review.specialty.be",
    "review.specialty.ar",
    "review.specialty.qa",
];

/// Daemon-derived identity used to bind a review to its physical caller.
#[derive(Clone, Debug)]
pub(crate) struct AuthenticatedCaller {
    agent_id: Box<str>,
    session_id: SessionId,
    role: AgentRole,
}

impl AuthenticatedCaller {
    pub(crate) fn new(
        agent_id: impl Into<Box<str>>,
        session_id: SessionId,
        role: AgentRole,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            session_id,
            role,
        }
    }

    fn agent_id(&self) -> &str {
        &self.agent_id
    }

    const fn session_id(&self) -> SessionId {
        self.session_id
    }

    const fn role(&self) -> AgentRole {
        self.role
    }
}

/// Pure review preparation result consumed by the transactional business adapter.
#[derive(Clone, Debug)]
pub(crate) struct PreparedReviewRecord {
    /// Latest review projection. It always contains a typed v2 batch.
    pub(crate) review: Option<Value>,
    /// Authoritative typed v2 review session committed with the batch.
    pub(crate) review_session: Value,
    /// Terminal v2 exit receipt, when the session reached an exit.
    pub(crate) receipt: Option<Value>,
    /// Input fingerprint computed from the locked non-review state.
    pub(crate) input_fingerprint: String,
    /// Ruleset fingerprint derived from daemon policy and inventory.
    pub(crate) ruleset_fingerprint: String,
    typed_session: ReviewSessionV2,
    typed_batch: ReviewBatchV2,
    typed_attempt: ReviewAttemptV2,
    typed_batch_receipt: ReviewBatchReceiptV2,
    typed_exit_receipt: Option<ReviewExitReceiptV2>,
}

impl PreparedReviewRecord {
    /// Binds the already validated typed Review records to the committed
    /// runtime event sequence for the SQLite projection transaction.
    pub(crate) fn projection_write(
        &self,
        workspace_id: &str,
        work_item_id: &str,
        event_sequence: u64,
    ) -> RuntimeResult<ReviewProjectionWrite> {
        ReviewProjectionWrite::new(
            workspace_id,
            work_item_id,
            event_sequence,
            self.typed_session.clone(),
            self.typed_batch.clone(),
            self.typed_attempt.clone(),
            self.typed_batch_receipt.clone(),
            self.typed_exit_receipt.clone(),
        )
    }
}

/// Prepared `review.contribute` mutation: the pending review object, the
/// session it binds to, and the fingerprint binding committed into state.
///
/// A contribution never produces a batch, attempt, receipt or SQLite Review
/// projection; those exist only after a `review.finalize` aggregates the
/// pending projection.
#[derive(Clone, Debug)]
pub(crate) struct PreparedReviewContribution {
    /// Pending review projection: status/findings/pendingContributions.
    pub(crate) review: Value,
    /// Authoritative typed v2 review session the contribution binds to.
    pub(crate) review_session: Value,
    /// Input fingerprint computed from the locked non-review state.
    pub(crate) input_fingerprint: String,
    /// Ruleset fingerprint derived from daemon policy and inventory.
    pub(crate) ruleset_fingerprint: String,
}

/// Rebuilds one typed Review Batch v2 projection write from authoritative
/// project state during restart replay.
///
/// States with no Review authority return `None`. Once any v2 Review field is
/// present, the complete session/batch/latest-attempt/optional-exit tuple is
/// required and all cross-record identities are checked by
/// [`ReviewProjectionWrite::new`].
pub(crate) fn review_projection_write_from_state(
    state: &Value,
    workspace_id: &str,
    work_item_id: &str,
    event_sequence: u64,
) -> RuntimeResult<Option<ReviewProjectionWrite>> {
    let session_value = state.get("reviewSession");
    let review_value = state.get("review");
    if session_value.is_none() && review_value.is_none() {
        return Ok(None);
    }
    let session: ReviewSessionV2 = serde_json::from_value(
        session_value
            .cloned()
            .ok_or_else(|| external_conflict("review projection replay lacks reviewSession"))?,
    )
    .map_err(|_| external_conflict("review projection replay has malformed reviewSession"))?;
    let review = review_value
        .and_then(Value::as_object)
        .ok_or_else(|| external_conflict("review projection replay lacks the review object"))?;
    let Some(batch_value) = review.get("batch") else {
        // A `review.contribute` commit leaves a session plus a pending
        // contribution projection but no batch: there is nothing to rebuild
        // into the SQLite Review projection until a finalize aggregates.
        if review.contains_key("pendingContributions") {
            return Ok(None);
        }
        return Err(external_conflict(
            "review projection replay lacks the v2 batch",
        ));
    };
    let batch: ReviewBatchV2 = serde_json::from_value(batch_value.clone())
        .map_err(|_| external_conflict("review projection replay has a malformed v2 batch"))?;
    let attempt: ReviewAttemptV2 =
        serde_json::from_value(review.get("attempt").cloned().ok_or_else(|| {
            external_conflict("review projection replay lacks the latest attempt")
        })?)
        .map_err(|_| {
            external_conflict("review projection replay has a malformed latest attempt")
        })?;
    let exit_receipt = review
        .get("receipt")
        .cloned()
        .map(serde_json::from_value::<ReviewExitReceiptV2>)
        .transpose()
        .map_err(|_| external_conflict("review projection replay has a malformed exit receipt"))?;
    ReviewProjectionWrite::new(
        workspace_id,
        work_item_id,
        event_sequence,
        session,
        batch.clone(),
        attempt,
        batch.latest_receipt().clone(),
        exit_receipt,
    )
    .map(Some)
}

#[derive(Clone, Debug)]
struct ReviewGateState {
    session: ReviewSessionV2,
    batch: ReviewBatchV2,
    attempt: ReviewAttemptV2,
    receipt: ReviewExitReceiptV2,
    session_value: Value,
    batch_value: Value,
    receipt_value: Value,
}

/// Fails closed unless the terminal Review authority in project state is
/// exactly backed by the durable SQLite projection and current daemon-owned
/// reviewer identity. Tier 3 additionally requires the durable toolset job,
/// project receipt, active evidence manifest, and COMMITTED mutation journal.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_review_gate_authority(
    database: &Path,
    workspace: &BusinessWorkspace,
    state_path: &Path,
    state: &Value,
    work_item_id: &str,
    persistence: &dyn PersistencePort,
    boot_id: &str,
    observed_at: &UtcTimestamp,
) -> RuntimeResult<()> {
    let state_bytes = fs::read(state_path)
        .map_err(|_| external_conflict("Review Gate state file is unreadable"))?;
    let persisted_state: Value = serde_json::from_slice(&state_bytes)
        .map_err(|_| external_conflict("Review Gate state file is malformed"))?;
    if &persisted_state != state {
        return Err(external_conflict(
            "Review Gate state snapshot differs from the locked project state",
        ));
    }

    let authority = parse_review_gate_state(state)?;
    let review_work_item_id = review_projection_work_item_id(state, work_item_id)?;
    let projection = load_review_authority_projection(
        database,
        &workspace.workspace_id,
        review_work_item_id,
        authority.session.review_id().as_str(),
    )?
    .ok_or_else(|| external_conflict("Review Gate SQLite projection is missing"))?;
    validate_review_projection_alignment(
        &authority,
        &projection,
        &workspace.workspace_id,
        review_work_item_id,
    )?;

    let current_input = authoritative_review_workspace_input_fingerprint(workspace, state)?;
    if current_input != authority.session.input_fingerprint()
        || state
            .get("inputFingerprint")
            .and_then(Value::as_str)
            .and_then(parse_manifest_input_fingerprint)
            != Some(authority.session.input_fingerprint())
        || state
            .get("rulesetFingerprint")
            .and_then(Value::as_str)
            .and_then(parse_manifest_input_fingerprint)
            != Some(authority.session.ruleset_fingerprint())
        || state
            .get("policyDigest")
            .and_then(Value::as_str)
            .and_then(|value| PolicyDigest::from_str(value).ok())
            != Some(authority.session.policy_digest())
        || state.get("inventoryGeneration").and_then(Value::as_u64)
            != Some(authority.session.inventory_generation())
        || state
            .get("revision")
            .and_then(Value::as_u64)
            .is_none_or(|revision| revision < authority.session.source_revision())
    {
        return Err(RuntimeError::new(
            StableErrorCode::StaleGateResult,
            "Review Gate authority is stale for the current workspace input",
        ));
    }

    let now_ms = u64::try_from(observed_at.as_timestamp().as_millisecond())
        .map_err(|_| schema_error("Review Gate timestamp predates the Unix epoch"))?;
    let committing_boot_id = committing_review_boot_id(
        persistence,
        &workspace.workspace_id,
        review_work_item_id,
        projection.last_event_sequence,
    )?;
    let boots = AttestationBoots::committed(boot_id, &committing_boot_id);
    for contribution in &projection.contributions {
        validate_projected_reviewer(
            contribution,
            &authority.session,
            workspace,
            review_work_item_id,
            persistence,
            boots,
            now_ms,
        )?;
    }

    if authority.session.tier() == ReviewTier::Tier3 {
        validate_tier3_review_authority(
            workspace,
            state_path,
            state,
            review_work_item_id,
            persistence,
            &authority,
            observed_at,
        )?;
    }
    Ok(())
}

/// Resolves Review persistence below a root Route/PRD state to the active Story
/// that owns the review session and projection. Direct Story Gate requests keep
/// their explicit identity.
fn review_projection_work_item_id<'a>(
    state: &'a Value,
    requested_work_item_id: &'a str,
) -> RuntimeResult<&'a str> {
    if requested_work_item_id.starts_with("STORY-") {
        StoryId::new(requested_work_item_id.to_owned())
            .map_err(|_| schema_error("Review Gate Story identity is invalid"))?;
        return Ok(requested_work_item_id);
    }
    let active_story = state
        .get("activeStory")
        .and_then(Value::as_str)
        .or_else(|| state.get("currentStory").and_then(Value::as_str))
        .ok_or_else(|| gate_blocked("Review Gate root state has no active Story anchor"))?;
    StoryId::new(active_story.to_owned())
        .map_err(|_| schema_error("Review Gate active Story identity is invalid"))?;
    if !state
        .get("storyStates")
        .and_then(Value::as_object)
        .is_some_and(|stories| stories.contains_key(active_story))
    {
        return Err(external_conflict(
            "Review Gate active Story is absent from authoritative storyStates",
        ));
    }
    Ok(active_story)
}

/// Resolves the daemon boot that durably committed the review aggregation
/// event the SQLite projection was built from.
///
/// The event must exist in this event store at exactly the projected cursor
/// and must carry the same workspace and Work Item. Both the legacy
/// `review.record` adapter and `review.finalize` produce the authoritative
/// aggregation event. A projection whose committing event cannot be produced
/// fails closed rather than being trusted.
fn committing_review_boot_id(
    persistence: &dyn PersistencePort,
    workspace_id: &str,
    work_item_id: &str,
    last_event_sequence: u64,
) -> RuntimeResult<String> {
    let cursor = last_event_sequence
        .checked_sub(1)
        .ok_or_else(|| external_conflict("Review projection has no committed event cursor"))?;
    let page = persistence.events_after(cursor, 1)?;
    let [event] = page.as_slice() else {
        return Err(external_conflict(
            "Review projection committing event is missing from the durable event store",
        ));
    };
    let aggregation_event = event.kind == OperationName::ReviewRecord.as_str()
        || event.kind == OperationName::ReviewFinalize.as_str();
    if event.event_seq != last_event_sequence
        || !aggregation_event
        || event.workspace_id.as_deref() != Some(workspace_id)
        || event.work_item_id.as_deref() != Some(work_item_id)
        || event.boot_id.is_empty()
    {
        return Err(external_conflict(
            "Review projection cursor does not resolve one committed review aggregation event",
        ));
    }
    Ok(event.boot_id.clone())
}

/// Tier 3 requires the terminal Review proof to join the durable toolset
/// verification job, the immutable project receipt locator, the active evidence
/// manifest, and one COMMITTED mutation journal entry for the same Work Item.
///
/// The typed `projectAuthority` written into the exit receipt is the only
/// accepted binding; nothing here is recomputed from the caller.
fn validate_tier3_review_authority(
    workspace: &BusinessWorkspace,
    state_path: &Path,
    state: &Value,
    work_item_id: &str,
    persistence: &dyn PersistencePort,
    authority: &ReviewGateState,
    observed_at: &UtcTimestamp,
) -> RuntimeResult<()> {
    let observed = ReviewTimestamp::new(observed_at.to_string())
        .map_err(|_| schema_error("Review Gate timestamp is not canonical UTC"))?;
    let material = latest_final_verification_authority(
        persistence,
        workspace,
        state,
        work_item_id,
        &authority.session,
        &observed,
    )?;
    let project_authority = authority
        .receipt_value
        .get("projectAuthority")
        .ok_or_else(|| gate_blocked("Tier 3 Review receipt lacks projectAuthority"))?;
    let expected_authority = serde_json::to_value(ReviewProjectAuthorityV2::new(
        ReviewAuthorityRef::new(material.project_receipt_ref.clone())
            .map_err(|_| external_conflict("Tier 3 project receipt reference is invalid"))?,
        material.active_manifest_digest,
        material.state_receipt_ref_digest,
        ReviewMutationId::new(material.journal_mutation_id.clone())
            .map_err(|_| external_conflict("Tier 3 journal mutation identity is invalid"))?,
    ))
    .map_err(|_| schema_error("Tier 3 project authority could not be canonicalized"))?;
    if project_authority != &expected_authority {
        return Err(external_conflict(
            "Tier 3 Review project authority differs from the durable verification receipt",
        ));
    }
    if authority.receipt.final_proof().kind() != ReviewFinalProofKind::FinalVerification
        || authority.receipt.final_proof().digest() != Some(material.state_receipt_ref_digest)
    {
        return Err(gate_blocked(
            "Tier 3 Review final proof is not bound to the final verification receipt",
        ));
    }
    require_committed_journal_mutation(
        workspace,
        state_path,
        work_item_id,
        &material.journal_mutation_id,
    )
}

/// Fails closed unless exactly one COMMITTED journal entry for this workspace
/// and Work Item carries the referenced mutation identity.
fn require_committed_journal_mutation(
    workspace: &BusinessWorkspace,
    state_path: &Path,
    work_item_id: &str,
    journal_mutation_id: &str,
) -> RuntimeResult<()> {
    let root = Path::new(&workspace.canonical_root);
    let relative = state_path
        .strip_prefix(root)
        .map_err(|_| external_conflict("Review Gate state path escaped the workspace"))?
        .to_string_lossy()
        .replace('\\', "/");
    let relative = ae_sdd_domain::ProjectRelativePath::new(relative)
        .map_err(|_| external_conflict("Review Gate state path is not project relative"))?;
    let paths = ae_sdd_store::ProjectStorePaths::new(root, relative)
        .map_err(|_| external_conflict("Review Gate journal paths cannot be resolved"))?;
    let directory = paths.journal_dir();
    let mut entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(gate_blocked(
                "Tier 3 Review requires a COMMITTED mutation journal",
            ));
        }
        Err(_) => {
            return Err(external_conflict("Review Gate journal is unreadable"));
        }
    };
    let mut committed = 0_usize;
    while let Some(entry) = entries
        .next()
        .transpose()
        .map_err(|_| external_conflict("Review Gate journal directory entry is unreadable"))?
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let bytes =
            fs::read(&path).map_err(|_| external_conflict("Review Gate journal is unreadable"))?;
        let Ok(record) = ae_sdd_store::MutationJournalEntry::from_json(&bytes) else {
            continue;
        };
        if record.mutation_id.to_string() != journal_mutation_id {
            continue;
        }
        if record.status != ae_sdd_store::JournalStatus::Committed
            || record.workspace_id.to_string() != workspace.workspace_id
            || record.work_item_id.as_str() != work_item_id
            || record.receipt.is_none()
        {
            return Err(external_conflict(
                "Tier 3 Review journal mutation is not COMMITTED for this Work Item",
            ));
        }
        committed += 1;
    }
    if committed != 1 {
        return Err(gate_blocked(
            "Tier 3 Review requires exactly one COMMITTED journal mutation",
        ));
    }
    Ok(())
}

fn parse_review_gate_state(state: &Value) -> RuntimeResult<ReviewGateState> {
    let session_value = state
        .get("reviewSession")
        .cloned()
        .ok_or_else(|| gate_blocked("Review Gate requires a v2 reviewSession"))?;
    let review = state
        .get("review")
        .and_then(Value::as_object)
        .ok_or_else(|| gate_blocked("Review Gate requires a v2 review projection"))?;
    let batch_value = review
        .get("batch")
        .cloned()
        .ok_or_else(|| gate_blocked("Review Gate requires a v2 batch"))?;
    let attempt_value = review
        .get("attempt")
        .cloned()
        .ok_or_else(|| gate_blocked("Review Gate requires the latest v2 attempt"))?;
    let receipt_value = review
        .get("receipt")
        .cloned()
        .ok_or_else(|| gate_blocked("Review Gate requires a terminal v2 receipt"))?;
    let session: ReviewSessionV2 = serde_json::from_value(session_value.clone())
        .map_err(|_| external_conflict("Review Gate reviewSession is malformed"))?;
    let batch: ReviewBatchV2 = serde_json::from_value(batch_value.clone())
        .map_err(|_| external_conflict("Review Gate batch is malformed"))?;
    let attempt: ReviewAttemptV2 = serde_json::from_value(attempt_value)
        .map_err(|_| external_conflict("Review Gate attempt is malformed"))?;
    let receipt: ReviewExitReceiptV2 = serde_json::from_value(receipt_value.clone())
        .map_err(|_| external_conflict("Review Gate receipt is malformed"))?;
    let specialties = batch
        .retained_contributions()
        .iter()
        .map(|item| item.reviewer().specialty())
        .collect::<BTreeSet<_>>();
    let sessions = batch
        .retained_contributions()
        .iter()
        .map(|item| item.reviewer().physical_session_id())
        .collect::<BTreeSet<_>>();
    let delegations = batch
        .retained_contributions()
        .iter()
        .map(|item| item.reviewer().delegation_id())
        .collect::<BTreeSet<_>>();
    if !receipt.valid_pass_for(&session)
        || batch.review_id() != session.review_id()
        || batch.batch_id() != receipt.last_batch_id()
        || batch.input_fingerprint() != session.input_fingerprint()
        || batch.ruleset_fingerprint() != session.ruleset_fingerprint()
        || batch.latest_status() != ReviewBatchStatusV2::ValidClean
        || !batch.is_closed()
        || batch.latest_receipt().attempt_id() != receipt.last_attempt_id()
        || batch.latest_receipt().receipt_digest()
            != receipt
                .zero_finding_batch_receipt_digest()
                .ok_or_else(|| gate_blocked("Review Gate receipt lacks zero-finding proof"))?
        || batch.exit_receipt() != Some(&receipt)
        || attempt.review_id() != session.review_id()
        || attempt.batch_id() != batch.batch_id()
        || attempt.attempt_id() != receipt.last_attempt_id()
        || !canonical_review_receipt_digest(&receipt_value, receipt.receipt_digest())
        || !canonical_review_receipt_digest(
            batch_value
                .get("latestReceipt")
                .ok_or_else(|| external_conflict("Review batch lacks latestReceipt"))?,
            batch.latest_receipt().receipt_digest(),
        )
        || batch.retained_contributions().len() != session.required_specialties().len()
        || specialties
            != session
                .required_specialties()
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
        || sessions.len() != batch.retained_contributions().len()
        || delegations.len() != batch.retained_contributions().len()
        || batch.retained_contributions().iter().any(|item| {
            item.outcome() != ReviewContributionOutcomeV2::Clean
                || !item.findings().is_empty()
                || item.input_fingerprint() != session.input_fingerprint()
                || item.ruleset_fingerprint() != session.ruleset_fingerprint()
                || item.reviewer().root_session_id() != session.root_session_id()
                || item.reviewer().physical_session_id() == session.author_session_id()
        })
    {
        return Err(gate_blocked(
            "Review Gate terminal v2 authority is internally inconsistent",
        ));
    }
    Ok(ReviewGateState {
        session,
        batch,
        attempt,
        receipt,
        session_value,
        batch_value,
        receipt_value,
    })
}

fn canonical_review_receipt_digest(value: &Value, expected: ArtifactDigest) -> bool {
    let mut body = value.clone();
    let Some(object) = body.as_object_mut() else {
        return false;
    };
    if object.remove("receiptDigest").is_none() {
        return false;
    }
    serde_json::to_vec(&body).ok().map(ArtifactDigest::digest) == Some(expected)
}

fn validate_review_projection_alignment(
    authority: &ReviewGateState,
    projection: &ReviewAuthorityProjectionV2,
    workspace_id: &str,
    work_item_id: &str,
) -> RuntimeResult<()> {
    if projection
        .session
        .get("schemaVersion")
        .and_then(Value::as_u64)
        != Some(2)
        || projection
            .session
            .get("workspaceId")
            .and_then(Value::as_str)
            != Some(workspace_id)
        || projection.session.get("workItemId").and_then(Value::as_str) != Some(work_item_id)
    {
        return Err(external_conflict(
            "Review session projection scope is inconsistent",
        ));
    }
    require_same_projection_fields(
        &authority.session_value,
        &projection.session,
        &[
            "reviewId",
            "parentReviewId",
            "tier",
            "status",
            "authorSessionId",
            "rootSessionId",
            "repairClass",
            "requiredSpecialties",
            "cleanPolicy",
            "budget",
            "counters",
            "inputFingerprint",
            "rulesetFingerprint",
            "sourceRevision",
            "inventoryGeneration",
            "startedAt",
            "deadlineAt",
            "terminalAt",
        ],
        "Review session projection",
    )?;

    let expected_batch = json!({
        "reviewId": authority.batch_value.get("reviewId"),
        "batchId": authority.batch_value.get("batchId"),
        "inputFingerprint": authority.batch_value.get("inputFingerprint"),
        "rulesetFingerprint": authority.batch_value.get("rulesetFingerprint"),
        "latestAttemptId": authority.batch_value.get("latestAttemptId"),
        "latestStatus": projected_status_column(&authority.batch_value, "latestStatus")?,
        "requiredSpecialtyCount": authority.session.required_specialties().len(),
        "effectiveContributionCount": authority.batch.retained_contributions().len(),
        "findingCount": authority.batch.finding_fingerprints().len(),
        "closed": authority.batch.is_closed(),
        "validBatchOrdinal": authority.batch_value.get("validBatchOrdinal"),
        "latestReceiptDigest": authority.batch.latest_receipt().receipt_digest().to_string(),
    });
    require_same_projection_fields(
        &expected_batch,
        &projection.batch,
        &[
            "reviewId",
            "batchId",
            "inputFingerprint",
            "rulesetFingerprint",
            "latestAttemptId",
            "latestStatus",
            "requiredSpecialtyCount",
            "effectiveContributionCount",
            "findingCount",
            "closed",
            "validBatchOrdinal",
            "latestReceiptDigest",
        ],
        "Review batch projection",
    )?;

    let counters = authority.session.counters();
    if projection.attempts.len() != counters.attempts() as usize
        || projection.findings.len() != authority.batch.finding_fingerprints().len()
        || projection.remediations.len() != counters.remediations() as usize
        || projection.first_event_sequence == 0
        || projection.last_event_sequence < projection.first_event_sequence
    {
        return Err(external_conflict(
            "Review projection counts or event cursor are inconsistent",
        ));
    }
    let mut ordinals = BTreeSet::new();
    let mut event_sequences = BTreeSet::new();
    let mut valid_batches = 0_u32;
    let mut infra_failures = 0_u32;
    let mut protocol_failures = 0_u32;
    for attempt in &projection.attempts {
        let ordinal = attempt
            .get("attemptOrdinal")
            .and_then(Value::as_u64)
            .ok_or_else(|| external_conflict("Review attempt projection lacks its ordinal"))?;
        let event_sequence = attempt
            .get("eventSequence")
            .and_then(Value::as_u64)
            .ok_or_else(|| external_conflict("Review attempt projection lacks its event"))?;
        if !ordinals.insert(ordinal)
            || !event_sequences.insert(event_sequence)
            || event_sequence < projection.first_event_sequence
            || event_sequence > projection.last_event_sequence
            || attempt.get("reviewId").and_then(Value::as_str)
                != Some(authority.session.review_id().as_str())
            || attempt.get("inputFingerprint").and_then(Value::as_str)
                != Some(authority.session.input_fingerprint().to_string().as_str())
            || attempt.get("rulesetFingerprint").and_then(Value::as_str)
                != Some(authority.session.ruleset_fingerprint().to_string().as_str())
            || attempt
                .get("requiredSpecialtyCount")
                .and_then(Value::as_u64)
                != Some(authority.session.required_specialties().len() as u64)
        {
            return Err(external_conflict(
                "Review attempt projection identity or event ordering is inconsistent",
            ));
        }
        match attempt.get("status").and_then(Value::as_str) {
            Some("valid_clean" | "valid_findings") => valid_batches += 1,
            Some("invalid_infra") => infra_failures += 1,
            Some("invalid_protocol") => protocol_failures += 1,
            Some(_) => {}
            None => {
                return Err(external_conflict(
                    "Review attempt projection lacks a status",
                ));
            }
        }
    }
    if ordinals != (1..=u64::from(counters.attempts())).collect::<BTreeSet<_>>()
        || valid_batches != counters.valid_batches()
        || infra_failures != counters.infra_failures()
        || protocol_failures != counters.protocol_failures()
    {
        return Err(external_conflict(
            "Review attempt projection does not reproduce session counters",
        ));
    }
    for receipt in authority.batch.attempt_receipts() {
        let rows = projection
            .attempts
            .iter()
            .filter(|row| {
                row.get("attemptId").and_then(Value::as_str) == Some(receipt.attempt_id().as_str())
            })
            .collect::<Vec<_>>();
        let [row] = rows.as_slice() else {
            return Err(external_conflict(
                "Review batch receipt lacks one durable attempt row",
            ));
        };
        let key_digest = hex::encode(Sha256::digest(
            receipt.idempotency_key().as_str().as_bytes(),
        ));
        if row.get("batchId").and_then(Value::as_str) != Some(receipt.batch_id().as_str())
            || row.get("attemptOrdinal").and_then(Value::as_u64)
                != Some(u64::from(receipt.counters().attempts()))
            || row.get("status").and_then(Value::as_str)
                != Some(projected_status_column(
                    &serde_json::to_value(receipt)
                        .map_err(|_| external_conflict("Review batch receipt is not canonical"))?,
                    "status",
                )?)
            || row.get("idempotencyKeyDigest").and_then(Value::as_str) != Some(key_digest.as_str())
            || row.get("payloadDigest").and_then(Value::as_str)
                != Some(receipt.attempt_digest().to_string().as_str())
            || row.get("receiptDigest").and_then(Value::as_str)
                != Some(receipt.receipt_digest().to_string().as_str())
            || row.get("findingCount").and_then(Value::as_u64)
                != Some(receipt.finding_fingerprints().len() as u64)
        {
            return Err(external_conflict(
                "Review attempt row differs from its authoritative batch receipt",
            ));
        }
    }

    validate_latest_attempt_projection(authority, projection)?;

    let expected_contribution_count = usize::try_from(counters.valid_batches())
        .ok()
        .and_then(|batches| batches.checked_mul(authority.session.required_specialties().len()))
        .ok_or_else(|| external_conflict("Review contribution count overflowed"))?;
    if projection.contributions.len() != expected_contribution_count {
        return Err(external_conflict(
            "Review effective-contribution projection count is inconsistent",
        ));
    }
    for contribution in authority.batch.retained_contributions() {
        let value = serde_json::to_value(contribution)
            .map_err(|_| external_conflict("Review contribution is not canonical"))?;
        let reviewer = value
            .get("reviewer")
            .ok_or_else(|| external_conflict("Review contribution lacks reviewer identity"))?;
        let specialty = reviewer
            .get("specialty")
            .and_then(Value::as_str)
            .ok_or_else(|| external_conflict("Review contribution lacks specialty"))?;
        let rows = projection
            .contributions
            .iter()
            .filter(|row| {
                row.get("batchId").and_then(Value::as_str)
                    == Some(authority.batch.batch_id().as_str())
                    && row.get("specialty").and_then(Value::as_str) == Some(specialty)
            })
            .collect::<Vec<_>>();
        let [row] = rows.as_slice() else {
            return Err(external_conflict(
                "Review latest contribution lacks one durable projection row",
            ));
        };
        let expected = json!({
            "sourceAttemptId": value.get("sourceAttemptId"),
            "agentRole": reviewer.get("agentRole"),
            "specialty": reviewer.get("specialty"),
            "outcome": value.get("outcome"),
            "physicalSessionId": reviewer.get("physicalSessionId"),
            "rootSessionId": reviewer.get("rootSessionId"),
            "delegationId": reviewer.get("delegationId"),
            "lineageDepth": reviewer.get("lineageDepth"),
            "attestationRef": reviewer.get("attestationRef"),
            "attestationDigest": reviewer.get("attestationDigest"),
            "specialtyGrantDigest": reviewer.get("specialtyGrantDigest"),
            "reportDigest": value.get("reportDigest"),
            "contributionDigest": value.get("contributionDigest"),
            "inputFingerprint": value.get("inputFingerprint"),
            "rulesetFingerprint": value.get("rulesetFingerprint"),
            "findingCount": contribution.findings().len(),
        });
        require_same_projection_fields(
            &expected,
            row,
            &[
                "sourceAttemptId",
                "agentRole",
                "specialty",
                "outcome",
                "physicalSessionId",
                "rootSessionId",
                "delegationId",
                "lineageDepth",
                "attestationRef",
                "attestationDigest",
                "specialtyGrantDigest",
                "reportDigest",
                "contributionDigest",
                "inputFingerprint",
                "rulesetFingerprint",
                "findingCount",
            ],
            "Review contribution projection",
        )?;
    }

    validate_exit_projection(authority, projection)
}

/// Binds the latest attempt carried by project state to exactly one durable
/// attempt row, and requires the attempt's committed remediation to be projected
/// against the parent review it repairs.
fn validate_latest_attempt_projection(
    authority: &ReviewGateState,
    projection: &ReviewAuthorityProjectionV2,
) -> RuntimeResult<()> {
    // The durable `payload_digest` column stores the frozen batch receipt's
    // `attemptDigest`, which the supervisor derives from the TYPED attempt in
    // contract declaration order. Digesting the untyped `Value` would hash the
    // same records in sorted key order and never match.
    let payload_digest = hex::encode(Sha256::digest(
        serde_json::to_vec(&authority.attempt)
            .map_err(|_| external_conflict("Review latest attempt is not canonical"))?,
    ));
    if authority
        .batch
        .latest_receipt()
        .attempt_digest()
        .to_string()
        != payload_digest
    {
        return Err(external_conflict(
            "Review latest attempt does not reproduce its batch receipt attempt digest",
        ));
    }
    let rows = projection
        .attempts
        .iter()
        .filter(|row| {
            row.get("attemptId").and_then(Value::as_str)
                == Some(authority.attempt.attempt_id().as_str())
        })
        .collect::<Vec<_>>();
    let [row] = rows.as_slice() else {
        return Err(external_conflict(
            "Review latest attempt lacks one durable projection row",
        ));
    };
    if row.get("batchId").and_then(Value::as_str) != Some(authority.attempt.batch_id().as_str()) {
        return Err(external_conflict(
            "Review latest attempt row batch differs from authoritative project state",
        ));
    }
    if row.get("attemptOrdinal").and_then(Value::as_u64)
        != Some(u64::from(authority.attempt.attempt_ordinal()))
    {
        return Err(external_conflict(
            "Review latest attempt row ordinal differs from authoritative project state",
        ));
    }
    if row.get("payloadDigest").and_then(Value::as_str) != Some(payload_digest.as_str()) {
        return Err(external_conflict(
            "Review latest attempt row payload digest differs from authoritative project state",
        ));
    }
    if row.get("status").and_then(Value::as_str) != Some("valid_clean") {
        return Err(external_conflict(
            "Review latest attempt row status is not a clean valid attempt",
        ));
    }

    let Some(remediation) = authority.attempt.remediation() else {
        return Ok(());
    };
    let parent_review_id = authority
        .session
        .parent_review_id()
        .ok_or_else(|| external_conflict("Review remediation has no parent review authority"))?;
    let matching = projection
        .remediations
        .iter()
        .filter(|item| {
            item.get("reviewId").and_then(Value::as_str) == Some(parent_review_id.as_str())
                && item.get("findingBatchId").and_then(Value::as_str)
                    == Some(remediation.finding_batch_id().as_str())
        })
        .collect::<Vec<_>>();
    let [projected] = matching.as_slice() else {
        return Err(external_conflict(
            "Review remediation lacks one durable parent projection row",
        ));
    };
    if projected.get("nextReviewId").and_then(Value::as_str)
        != Some(authority.session.review_id().as_str())
        || projected.get("nextReviewId").and_then(Value::as_str)
            != Some(remediation.next_review_id().as_str())
        || projected.get("newInputFingerprint").and_then(Value::as_str)
            != Some(remediation.new_input_fingerprint().to_string().as_str())
        || projected.get("planFingerprint").and_then(Value::as_str)
            != Some(remediation.plan_fingerprint().to_string().as_str())
        || projected.get("targetRevision").and_then(Value::as_u64)
            != Some(authority.session.source_revision())
    {
        return Err(external_conflict(
            "Review remediation projection differs from the committed parent link",
        ));
    }
    Ok(())
}

fn validate_exit_projection(
    authority: &ReviewGateState,
    projection: &ReviewAuthorityProjectionV2,
) -> RuntimeResult<()> {
    let exit = projection
        .exit_receipt
        .as_ref()
        .ok_or_else(|| external_conflict("Review PASS projection lacks its exit receipt"))?;
    let expected = json!({
        "reviewId": authority.receipt_value.get("reviewId"),
        "tier": authority.receipt_value.get("tier"),
        "sessionStatus": authority.receipt_value.get("sessionStatus"),
        "disposition": authority.receipt_value.get("disposition"),
        "inputFingerprint": authority.receipt_value.get("inputFingerprint"),
        "observedInputFingerprint": authority.receipt_value.get("observedInputFingerprint"),
        "rulesetFingerprint": authority.receipt_value.get("rulesetFingerprint"),
        "sourceRevision": authority.receipt_value.get("sourceRevision"),
        "inventoryGeneration": authority.receipt_value.get("inventoryGeneration"),
        "policyDigest": authority.receipt_value.get("policyDigest"),
        "requiredSpecialtyCount": authority.receipt.required_specialties().len(),
        "completedSpecialtyCount": authority.receipt.completed_specialties().len(),
        "findingCount": 0,
        "cleanTarget": authority.receipt_value.get("cleanTarget"),
        "lastBatchId": authority.receipt_value.get("lastBatchId"),
        "lastAttemptId": authority.receipt_value.get("lastAttemptId"),
        "lastAttemptStatus": projected_status_column(&authority.receipt_value, "lastAttemptStatus")?,
        "zeroFindingBatchReceiptDigest": authority.receipt_value.get("zeroFindingBatchReceiptDigest"),
        "finalProof": authority.receipt_value.get("finalProof"),
        "projectAuthority": authority.receipt_value.get("projectAuthority"),
        "receiptDigest": authority.receipt_value.get("receiptDigest"),
    });
    require_same_projection_fields(
        &expected,
        exit,
        &[
            "reviewId",
            "tier",
            "sessionStatus",
            "disposition",
            "inputFingerprint",
            "observedInputFingerprint",
            "rulesetFingerprint",
            "sourceRevision",
            "inventoryGeneration",
            "policyDigest",
            "requiredSpecialtyCount",
            "completedSpecialtyCount",
            "findingCount",
            "cleanTarget",
            "lastBatchId",
            "lastAttemptId",
            "lastAttemptStatus",
            "zeroFindingBatchReceiptDigest",
            "finalProof",
            "projectAuthority",
            "receiptDigest",
        ],
        "Review exit projection",
    )?;
    let projected_counters = exit
        .get("counters")
        .and_then(Value::as_object)
        .ok_or_else(|| external_conflict("Review exit projection lacks counters"))?;
    let state_counters = authority
        .receipt_value
        .get("counters")
        .and_then(Value::as_object)
        .ok_or_else(|| external_conflict("Review exit receipt lacks counters"))?;
    for field in ["attempts", "validBatches", "cleanStreak", "remediations"] {
        if projected_counters.get(field) != state_counters.get(field) {
            return Err(external_conflict(
                "Review exit projection counters differ from project state",
            ));
        }
    }
    if exit.get("eventSequence").and_then(Value::as_u64) != Some(projection.last_event_sequence) {
        return Err(external_conflict(
            "Review exit projection is not bound to the latest event",
        ));
    }
    Ok(())
}

/// Reads one frozen upper-snake v2 status from project state and translates it
/// into the migration 0009 column domain the projection rows are stored in.
/// Unknown values fail closed through the shared mapping.
fn projected_status_column(value: &Value, key: &str) -> RuntimeResult<&'static str> {
    let wire = value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| external_conflict("Review authority status field is missing"))?;
    crate::persistence::projected_batch_status_column(wire)
}

fn require_same_projection_fields(
    expected: &Value,
    actual: &Value,
    fields: &[&str],
    label: &str,
) -> RuntimeResult<()> {
    if fields
        .iter()
        .any(|field| expected.get(*field) != actual.get(*field))
    {
        return Err(external_conflict(&format!(
            "{label} differs from authoritative project state"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_projected_reviewer(
    contribution: &Value,
    session: &ReviewSessionV2,
    workspace: &BusinessWorkspace,
    work_item_id: &str,
    persistence: &dyn PersistencePort,
    boots: AttestationBoots<'_>,
    now_ms: u64,
) -> RuntimeResult<()> {
    let physical_session_id = contribution
        .get("physicalSessionId")
        .and_then(Value::as_str)
        .ok_or_else(|| external_conflict("Review contribution lacks physicalSessionId"))?;
    let matching = persistence
        .list_identity_snapshots(RuntimeIdentityKind::Session)?
        .into_iter()
        .filter(|snapshot| snapshot.workspace.workspace_id == workspace.workspace_id)
        .filter_map(|snapshot| snapshot.session)
        .filter(|record| record.session_id == physical_session_id)
        .collect::<Vec<_>>();
    let [record] = matching.as_slice() else {
        return Err(identity_error(
            "Review contribution physical session is missing or ambiguous",
        ));
    };
    let caller = AuthenticatedCaller::new(
        record.agent_id.clone(),
        SessionId::from_str(physical_session_id)
            .map_err(|_| identity_error("Review physicalSessionId is invalid"))?,
        AgentRole::Reviewer,
    );
    let bound = bind_reviewer(
        workspace,
        work_item_id,
        &caller,
        persistence,
        boots,
        now_ms,
        ReviewAdmission::Revalidate,
    )?;
    let projected_reviewer: AttestedReviewerV2 = serde_json::from_value(json!({
        "agentRole": contribution.get("agentRole"),
        "specialty": contribution.get("specialty"),
        "grantedSpecialties": [contribution.get("specialty")],
        "physicalSessionId": contribution.get("physicalSessionId"),
        "rootSessionId": contribution.get("rootSessionId"),
        "delegationId": contribution.get("delegationId"),
        "lineageDepth": contribution.get("lineageDepth"),
        "attestationRef": contribution.get("attestationRef"),
        "attestationDigest": contribution.get("attestationDigest"),
        "specialtyGrantDigest": contribution.get("specialtyGrantDigest"),
    }))
    .map_err(|_| attestation_error("Review projection reviewer identity is malformed"))?;
    if bound.reviewer != projected_reviewer
        || bound.author_session_id != session.author_session_id()
        || bound.root_session_id != session.root_session_id()
        || bound.specialty != projected_reviewer.specialty()
    {
        return Err(attestation_error(
            "Review projection reviewer is stale or differs from current daemon authority",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct BoundReviewer {
    reviewer: AttestedReviewerV2,
    specialty: ReviewerSpecialty,
    author_session_id: SessionId,
    root_session_id: SessionId,
}

#[derive(Clone, Debug)]
struct ProjectAuthorityMaterial {
    project_receipt_ref: String,
    active_manifest_digest: ArtifactDigest,
    state_receipt_ref_digest: ArtifactDigest,
    journal_mutation_id: String,
}

/// Validates and prepares one legacy `review.record` mutation through the v2
/// supervisor.
///
/// Compat adapter only: the operation appends its own contribution to the
/// pending projection and immediately finalizes the aggregate through the
/// shared `review.finalize` pipeline. No second business implementation lives
/// here; every invocation is counted as deprecated telemetry.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_review_record(
    workspace: &BusinessWorkspace,
    state: &Value,
    work_item_id: &str,
    request: &ValidatedOperationRequest,
    caller: &AuthenticatedCaller,
    persistence: &dyn PersistencePort,
    boot_id: &str,
    policy_digest: &str,
    inventory_generation: u64,
    observed_at: &UtcTimestamp,
) -> RuntimeResult<PreparedReviewRecord> {
    if request.operation() != OperationName::ReviewRecord {
        return Err(schema_error("review authority requires review.record"));
    }
    tracing::warn!(
        operation = "review.record",
        deprecated = true,
        replacement = "review.contribute+review.finalize",
        "deprecated review operation served by the compat adapter"
    );
    let source_revision = validate_review_caller(
        workspace,
        state,
        work_item_id,
        request,
        caller,
        AgentRole::Reviewer,
    )?;
    let payload = request
        .request()
        .payload
        .as_object()
        .ok_or_else(|| schema_error("review payload must be an object"))?;
    let (outcome, findings) = decode_review_contribution_payload(payload)?;

    let input_fingerprint = authoritative_review_workspace_input_fingerprint(workspace, state)?;
    let policy_digest = PolicyDigest::from_str(policy_digest)
        .map_err(|_| schema_error("daemon policy digest is invalid"))?;
    let ruleset_fingerprint = review_ruleset_fingerprint(policy_digest, inventory_generation);
    let observed = review_observed_timestamp(observed_at)?;
    let now_ms = u64::try_from(observed_at.as_timestamp().as_millisecond())
        .map_err(|_| schema_error("daemon review timestamp predates the Unix epoch"))?;
    let boots = boots_with_receipt_fallback(
        boot_id,
        persistence,
        &workspace.workspace_id,
        caller.session_id(),
        now_ms,
    );
    let bound = bind_reviewer(
        workspace,
        work_item_id,
        caller,
        persistence,
        boots,
        now_ms,
        ReviewAdmission::Record,
    )?;
    if outcome == ReviewContributionOutcomeV2::Clean {
        validate_clean_contribution_depth(
            workspace,
            state,
            work_item_id,
            payload,
            input_fingerprint,
        )?;
    }

    let raw_findings = payload
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| schema_error("review findings must be an array"))?;
    let opened = open_review_session(
        state,
        work_item_id,
        &bound,
        raw_findings,
        input_fingerprint,
        ruleset_fingerprint,
        policy_digest,
        source_revision,
        inventory_generation,
        &observed,
    )?;

    // Adapter: contribute one, then finalize immediately. Any pending
    // projection from earlier `review.contribute` calls aggregates into the
    // same single attempt.
    let mut materials = load_pending_contributions(state)?
        .iter()
        .map(contribution_material)
        .collect::<RuntimeResult<Vec<_>>>()?;
    materials.push(ContributionMaterial {
        reviewer: bound.reviewer,
        outcome,
        findings,
        report_digest: review_report_digest(payload)?,
        input_fingerprint,
        ruleset_fingerprint,
    });
    finalize_review_attempt(
        workspace,
        state,
        work_item_id,
        request,
        opened.session,
        opened.remediation,
        opened.prior_batch,
        materials,
        input_fingerprint,
        ruleset_fingerprint,
        source_revision,
        persistence,
        &observed,
    )
}

/// Validates and prepares one `review.contribute` mutation: exactly one
/// reviewer contribution is appended to the pending projection without
/// closing, opening, or judging a batch.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_review_contribution(
    workspace: &BusinessWorkspace,
    state: &Value,
    work_item_id: &str,
    request: &ValidatedOperationRequest,
    caller: &AuthenticatedCaller,
    persistence: &dyn PersistencePort,
    boot_id: &str,
    policy_digest: &str,
    inventory_generation: u64,
    observed_at: &UtcTimestamp,
) -> RuntimeResult<PreparedReviewContribution> {
    if request.operation() != OperationName::ReviewContribute {
        return Err(schema_error("review authority requires review.contribute"));
    }
    let source_revision = validate_review_caller(
        workspace,
        state,
        work_item_id,
        request,
        caller,
        AgentRole::Reviewer,
    )?;
    let payload = request
        .request()
        .payload
        .as_object()
        .ok_or_else(|| schema_error("review payload must be an object"))?;
    let (outcome, findings) = decode_review_contribution_payload(payload)?;

    let input_fingerprint = authoritative_review_workspace_input_fingerprint(workspace, state)?;
    let policy_digest = PolicyDigest::from_str(policy_digest)
        .map_err(|_| schema_error("daemon policy digest is invalid"))?;
    let ruleset_fingerprint = review_ruleset_fingerprint(policy_digest, inventory_generation);
    let observed = review_observed_timestamp(observed_at)?;
    let now_ms = u64::try_from(observed_at.as_timestamp().as_millisecond())
        .map_err(|_| schema_error("daemon review timestamp predates the Unix epoch"))?;
    let boots = boots_with_receipt_fallback(
        boot_id,
        persistence,
        &workspace.workspace_id,
        caller.session_id(),
        now_ms,
    );
    let bound = bind_reviewer(
        workspace,
        work_item_id,
        caller,
        persistence,
        boots,
        now_ms,
        ReviewAdmission::Contribute,
    )?;
    if outcome == ReviewContributionOutcomeV2::Clean {
        validate_clean_contribution_depth(
            workspace,
            state,
            work_item_id,
            payload,
            input_fingerprint,
        )?;
    }

    let raw_findings = payload
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| schema_error("review findings must be an array"))?;
    let opened = open_review_session(
        state,
        work_item_id,
        &bound,
        raw_findings,
        input_fingerprint,
        ruleset_fingerprint,
        policy_digest,
        source_revision,
        inventory_generation,
        &observed,
    )?;
    let session = opened.session;
    if !session.required_specialties().contains(&bound.specialty) {
        return Err(gate_blocked(
            "reviewer specialty is not required by the active review session",
        ));
    }

    // A reused session keeps its pending projection; a freshly started
    // (child) session invalidates any projection recorded under the old
    // input, so the pending set restarts with this contribution.
    let mut pending = if opened.reused {
        load_pending_contributions(state)?
    } else {
        Vec::new()
    };
    let already_pending = |contribution: &ReviewerContributionV2| {
        contribution.reviewer().specialty() == bound.specialty
            || contribution.reviewer().physical_session_id() == bound.reviewer.physical_session_id()
    };
    if pending.iter().any(already_pending)
        || opened
            .prior_batch
            .iter()
            .flat_map(ReviewBatchV2::retained_contributions)
            .any(already_pending)
    {
        return Err(gate_blocked(
            "a pending contribution already exists for this reviewer or specialty",
        ));
    }

    let idempotency_key = request
        .request()
        .idempotency_key
        .as_deref()
        .ok_or_else(|| schema_error("review idempotencyKey is required"))?;
    let pending_attempt_id = derived_attempt_id(
        session.review_id(),
        &pending_batch_id(&session, opened.prior_batch.as_ref())?,
        idempotency_key,
        request.payload_digest(),
    )?;
    let contribution = build_contribution(
        pending_attempt_id,
        &ContributionMaterial {
            reviewer: bound.reviewer,
            outcome,
            findings,
            report_digest: review_report_digest(payload)?,
            input_fingerprint,
            ruleset_fingerprint,
        },
    )?;
    pending.push(contribution);

    let pending_values = pending
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| schema_error("pending review contribution could not be projected"))?;
    let mut review = Map::from_iter([
        ("status".to_owned(), Value::String("pending".to_owned())),
        (
            "findings".to_owned(),
            serde_json::to_value(Vec::<ReviewFinding>::new())
                .map_err(|_| schema_error("review findings could not be projected"))?,
        ),
        (
            "pendingContributions".to_owned(),
            Value::Array(pending_values),
        ),
    ]);
    if let Some(remediation) = opened.remediation.as_ref() {
        review.insert(
            "pendingRemediation".to_owned(),
            serde_json::to_value(remediation)
                .map_err(|_| schema_error("pending review remediation could not be projected"))?,
        );
    }
    Ok(PreparedReviewContribution {
        review: Value::Object(review),
        review_session: serde_json::to_value(&session)
            .map_err(|_| schema_error("review session could not be projected"))?,
        input_fingerprint: input_fingerprint.to_string(),
        ruleset_fingerprint: ruleset_fingerprint.to_string(),
    })
}

/// Validates and prepares one `review.finalize` mutation: the root/finalizer
/// aggregates the current pending contribution projection into exactly one
/// attempt, evaluates it through the existing supervisor, and atomically
/// writes batch/session/exit receipt.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_review_finalize(
    workspace: &BusinessWorkspace,
    state: &Value,
    work_item_id: &str,
    request: &ValidatedOperationRequest,
    caller: &AuthenticatedCaller,
    persistence: &dyn PersistencePort,
    policy_digest: &str,
    inventory_generation: u64,
    observed_at: &UtcTimestamp,
) -> RuntimeResult<PreparedReviewRecord> {
    if request.operation() != OperationName::ReviewFinalize {
        return Err(schema_error("review authority requires review.finalize"));
    }
    let source_revision = validate_review_caller(
        workspace,
        state,
        work_item_id,
        request,
        caller,
        AgentRole::Root,
    )?;
    validate_state_bound_review_evidence(workspace, state, work_item_id)?;
    let input_fingerprint = authoritative_review_workspace_input_fingerprint(workspace, state)?;
    let policy_digest = PolicyDigest::from_str(policy_digest)
        .map_err(|_| schema_error("daemon policy digest is invalid"))?;
    let ruleset_fingerprint = review_ruleset_fingerprint(policy_digest, inventory_generation);
    let observed = review_observed_timestamp(observed_at)?;

    let session = parse_existing_session(state)?
        .ok_or_else(|| gate_blocked("review.finalize requires an active review session"))?;
    if session.input_fingerprint() != input_fingerprint
        || session.ruleset_fingerprint() != ruleset_fingerprint
        || session.policy_digest() != policy_digest
        || session.inventory_generation() != inventory_generation
    {
        return Err(gate_blocked(
            "review.finalize input or ruleset drifted from the pending contributions",
        ));
    }
    let pending = load_pending_contributions(state)?;
    if pending.is_empty() {
        return Err(gate_blocked(
            "review.finalize requires at least one pending contribution",
        ));
    }
    if pending.iter().any(|contribution| {
        contribution.input_fingerprint() != input_fingerprint
            || contribution.ruleset_fingerprint() != ruleset_fingerprint
    }) {
        return Err(gate_blocked(
            "pending review contributions are stale for the current input",
        ));
    }
    let prior_batch = parse_existing_batch(state)?
        .filter(|batch| batch.review_id() == session.review_id())
        .filter(|batch| !batch.is_closed());
    let materials = pending
        .iter()
        .map(contribution_material)
        .collect::<RuntimeResult<Vec<_>>>()?;
    finalize_review_attempt(
        workspace,
        state,
        work_item_id,
        request,
        session,
        load_pending_remediation(state)?,
        prior_batch,
        materials,
        input_fingerprint,
        ruleset_fingerprint,
        source_revision,
        persistence,
        &observed,
    )
}

/// Shared finalize pipeline: binds the contribution materials to one derived
/// attempt, evaluates it through the existing supervisor, and projects the
/// next batch/session/exit receipt. Used by `review.finalize` and by the
/// legacy `review.record` compat adapter.
#[allow(clippy::too_many_arguments)]
fn finalize_review_attempt(
    workspace: &BusinessWorkspace,
    state: &Value,
    work_item_id: &str,
    request: &ValidatedOperationRequest,
    mut session: ReviewSessionV2,
    remediation: Option<ReviewRemediationV2>,
    prior_batch: Option<ReviewBatchV2>,
    materials: Vec<ContributionMaterial>,
    input_fingerprint: InputFingerprint,
    ruleset_fingerprint: InputFingerprint,
    source_revision: u64,
    persistence: &dyn PersistencePort,
    observed: &ReviewTimestamp,
) -> RuntimeResult<PreparedReviewRecord> {
    let batch_id = match prior_batch.as_ref() {
        Some(batch) => batch.batch_id().clone(),
        None => derived_batch_id(&session)?,
    };
    let attempt_ordinal = session
        .counters()
        .attempts()
        .checked_add(1)
        .ok_or_else(|| gate_blocked("review attempt budget is exhausted"))?;
    let idempotency_key = request
        .request()
        .idempotency_key
        .as_deref()
        .ok_or_else(|| schema_error("review idempotencyKey is required"))?;
    let attempt_id = derived_attempt_id(
        session.review_id(),
        &batch_id,
        idempotency_key,
        request.payload_digest(),
    )?;
    let mut contributions = Vec::with_capacity(materials.len());
    for material in &materials {
        contributions.push(build_contribution(attempt_id.clone(), material)?);
    }
    let completing_clean_attempt = materials
        .iter()
        .all(|material| material.outcome == ReviewContributionOutcomeV2::Clean)
        && input_fingerprint == session.input_fingerprint()
        && ruleset_fingerprint == session.ruleset_fingerprint()
        && completes_specialty_projection(&session, prior_batch.as_ref(), &materials);
    let (final_proof, proof_authority) = derive_final_proof(
        workspace,
        state,
        work_item_id,
        request,
        persistence,
        &session,
        completing_clean_attempt,
        observed,
    )?;
    let project_authority = build_project_authority(
        workspace,
        state,
        work_item_id,
        request,
        source_revision,
        proof_authority,
    )?;
    let attempt = ReviewAttemptV2::new(
        session.review_id().clone(),
        batch_id,
        attempt_id,
        attempt_ordinal,
        IdempotencyKey::new(idempotency_key.to_owned())
            .map_err(|_| schema_error("review idempotencyKey is invalid"))?,
        input_fingerprint,
        ruleset_fingerprint,
        contributions,
        observed.clone(),
        final_proof,
        project_authority,
        remediation,
    )
    .map_err(|_| schema_error("daemon-derived review attempt is invalid"))?;
    let attempt_value = serde_json::to_value(&attempt)
        .map_err(|_| schema_error("review attempt could not be projected"))?;
    let typed_attempt = attempt.clone();
    let evaluation = ReviewSupervisor::evaluate(&session, prior_batch.as_ref(), attempt)
        .map_err(supervisor_error)?;
    session = evaluation.next_session().clone();

    let batch = evaluation.next_batch().clone();
    let receipt = evaluation.exit_receipt().cloned();
    let findings = effective_batch_findings(&batch)?;
    let status = projected_status(&session, &batch, &findings);
    let mut review = Map::from_iter([
        ("status".to_owned(), Value::String(status.to_owned())),
        (
            "findings".to_owned(),
            serde_json::to_value(&findings)
                .map_err(|_| schema_error("review findings could not be projected"))?,
        ),
        (
            "batch".to_owned(),
            serde_json::to_value(&batch)
                .map_err(|_| schema_error("review batch could not be projected"))?,
        ),
        ("attempt".to_owned(), attempt_value),
        (
            "nextAction".to_owned(),
            serde_json::to_value(evaluation.next_action())
                .map_err(|_| schema_error("review next action could not be projected"))?,
        ),
    ]);
    let receipt_value = receipt
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|_| schema_error("review exit receipt could not be projected"))?;
    if let Some(value) = receipt_value.clone() {
        review.insert("receipt".to_owned(), value);
    }

    Ok(PreparedReviewRecord {
        review: Some(Value::Object(review)),
        review_session: serde_json::to_value(&session)
            .map_err(|_| schema_error("review session could not be projected"))?,
        receipt: receipt_value,
        input_fingerprint: input_fingerprint.to_string(),
        ruleset_fingerprint: ruleset_fingerprint.to_string(),
        typed_session: session,
        typed_batch: batch,
        typed_attempt,
        typed_batch_receipt: evaluation.batch_receipt().clone(),
        typed_exit_receipt: receipt,
    })
}

/// Which daemon boots may own a physical delegation attestation.
///
/// Admission of a live review contribution requires the running boot: a claim
/// accepted by an older boot must never mint new Review authority. Re-validating
/// already committed Review authority additionally accepts the boot that
/// durably committed the projected review aggregation event, because the durable
/// journal is what proves that boot really was this daemon. Without that, every
/// daemon restart would permanently destroy committed Review authority.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AttestationBoots<'a> {
    current: &'a str,
    policy: BootPermitPolicy<'a>,
}

#[derive(Clone, Copy, Debug)]
enum BootPermitPolicy<'a> {
    /// Only current boot is permitted (used when receipts missing/invalid)
    LiveCurrentOnly,
    /// Any historical boot is permitted (used when all four receipts valid)
    HistoricalReattachAuthorized,
    /// Specific committed boot is permitted (used for event revalidation)
    CommittedEventRevalidation(&'a str),
}

impl<'a> AttestationBoots<'a> {
    /// Live admission: only the running boot may own the attestation.
    const fn live(current: &'a str) -> Self {
        Self {
            current,
            policy: BootPermitPolicy::LiveCurrentOnly,
        }
    }

    /// Historical reattachment: any historical boot is allowed (when all
    /// four session receipts are valid in current boot).
    const fn historical_reattach(current: &'a str) -> Self {
        Self {
            current,
            policy: BootPermitPolicy::HistoricalReattachAuthorized,
        }
    }

    /// Committed re-validation: the running boot or the boot proven by the
    /// durable committing event.
    const fn committed(current: &'a str, committed: &'a str) -> Self {
        Self {
            current,
            policy: BootPermitPolicy::CommittedEventRevalidation(committed),
        }
    }

    pub(crate) fn permits(&self, accepted_boot_id: &str) -> bool {
        if accepted_boot_id.is_empty() {
            return false;
        }
        // Current boot always allowed
        if accepted_boot_id == self.current {
            return true;
        }
        // Historical boot handling depends on policy
        match self.policy {
            BootPermitPolicy::LiveCurrentOnly => false,
            BootPermitPolicy::HistoricalReattachAuthorized => true,
            BootPermitPolicy::CommittedEventRevalidation(committed_boot) => {
                accepted_boot_id == committed_boot
            }
        }
    }
}

/// Constructs AttestationBoots with receipt-based reattachment support.
///
/// Checks all four required lineage sessions (Root, Series, author Task, Reviewer)
/// for valid current-boot receipts. If all receipts exist, match current boot,
/// and have valid identity/lineage/grant/TTL, allows historical attestation.
/// Otherwise returns live boots requiring current-boot attestations only.
pub(crate) fn boots_with_receipt_fallback<'a>(
    boot_id: &'a str,
    persistence: &dyn PersistencePort,
    workspace_id: &str,
    reviewer_session_id: SessionId,
    now_ms: u64,
) -> AttestationBoots<'a> {
    // Load all session snapshots once
    let snapshots = match persistence.list_identity_snapshots(RuntimeIdentityKind::Session) {
        Ok(snapshots) => snapshots,
        Err(_) => return AttestationBoots::live(boot_id),
    };

    // Build receipt index by session ID
    let mut receipt_index = std::collections::HashMap::new();
    for snap in &snapshots {
        if let Some(session) = &snap.session
            && let Some(receipt) = &snap.current_boot_receipt
        {
            receipt_index.insert(session.session_id.as_str(), (session, receipt));
        }
    }

    // Find reviewer session and extract lineage
    let reviewer_session_id_str = reviewer_session_id.to_string();
    let Some((reviewer_session, reviewer_receipt)) =
        receipt_index.get(reviewer_session_id_str.as_str())
    else {
        return AttestationBoots::live(boot_id);
    };

    // Verify reviewer receipt matches current boot and basic identity
    if !verify_receipt_match(
        reviewer_receipt,
        boot_id,
        workspace_id,
        &reviewer_session_id_str,
        &reviewer_session.agent_id,
        WireAgentRole::Reviewer,
        &reviewer_session.root_session_id,
        reviewer_session.parent_session_id.as_deref(),
        reviewer_session.delegation_id.as_deref(),
        reviewer_session.current_work_item.as_deref(),
        &reviewer_session.grant,
        now_ms,
    ) {
        return AttestationBoots::live(boot_id);
    }

    // Extract required lineage sessions
    let root_session_id = &reviewer_session.root_session_id;
    let Some(series_session_id) = reviewer_session.parent_session_id.as_deref() else {
        return AttestationBoots::live(boot_id);
    };

    // Find and verify Root session receipt
    let Some((root_session, root_receipt)) = receipt_index.get(root_session_id.as_str()) else {
        return AttestationBoots::live(boot_id);
    };
    if !verify_receipt_match(
        root_receipt,
        boot_id,
        workspace_id,
        root_session_id,
        &root_session.agent_id,
        WireAgentRole::Root,
        root_session_id,
        None,
        None,
        root_session.current_work_item.as_deref(),
        &root_session.grant,
        now_ms,
    ) {
        return AttestationBoots::live(boot_id);
    }

    // Find and verify Series session receipt
    let Some((series_session, series_receipt)) = receipt_index.get(series_session_id) else {
        return AttestationBoots::live(boot_id);
    };
    if !verify_receipt_match(
        series_receipt,
        boot_id,
        workspace_id,
        series_session_id,
        &series_session.agent_id,
        WireAgentRole::Series,
        root_session_id,
        Some(root_session_id),
        series_session.delegation_id.as_deref(),
        series_session.current_work_item.as_deref(),
        &series_session.grant,
        now_ms,
    ) {
        return AttestationBoots::live(boot_id);
    }

    // Find author Task session from Series lineage (NOT from Reviewer delegation)
    // The Reviewer's delegation_id points to Series→Reviewer, but we need Series→Task
    let delegation_snapshots =
        match persistence.list_identity_snapshots(RuntimeIdentityKind::Delegation) {
            Ok(snapshots) => snapshots,
            Err(_) => return AttestationBoots::live(boot_id),
        };

    // Find the Task delegation: parent is Series, role is Task, work_item matches
    // current Review. Exactly one is required — zero or multiple candidates must
    // fail closed rather than guess which Task session the historical attestation
    // is allowed to bind.
    let review_work_item_id = reviewer_session.current_work_item.as_deref();
    let task_delegations = delegation_snapshots
        .iter()
        .filter_map(|snap| snap.delegation.as_ref())
        .filter(|d| {
            d.parent_session_id == series_session_id
                && d.workspace_id == workspace_id
                && d.role == WireAgentRole::Task
                && d.work_item_id.as_deref() == review_work_item_id
        })
        .collect::<Vec<_>>();
    let [task_delegation] = task_delegations.as_slice() else {
        return AttestationBoots::live(boot_id);
    };
    let Some(author_session_id) = task_delegation.child_session_id.as_deref() else {
        return AttestationBoots::live(boot_id);
    };

    // Find and verify author Task session receipt
    let Some((author_session, author_receipt)) = receipt_index.get(author_session_id) else {
        return AttestationBoots::live(boot_id);
    };
    if !verify_receipt_match(
        author_receipt,
        boot_id,
        workspace_id,
        author_session_id,
        &author_session.agent_id,
        WireAgentRole::Task,
        root_session_id,
        Some(series_session_id),
        author_session.delegation_id.as_deref(),
        author_session.current_work_item.as_deref(),
        &author_session.grant,
        now_ms,
    ) {
        return AttestationBoots::live(boot_id);
    }

    // All four receipts valid - allow any historical boot attestation.
    // When all sessions have valid receipts in the current boot, the lineage
    // is proven reconnected, so any historical accepted_boot_id is acceptable.
    AttestationBoots::historical_reattach(boot_id)
}

/// Verifies a single session receipt matches expected identity, lineage, and TTL.
#[allow(clippy::too_many_arguments)]
fn verify_receipt_match(
    receipt: &CurrentBootSessionReceipt,
    expected_boot_id: &str,
    expected_workspace_id: &str,
    expected_session_id: &str,
    expected_agent_id: &str,
    expected_role: WireAgentRole,
    expected_root_session_id: &str,
    expected_parent_session_id: Option<&str>,
    expected_delegation_id: Option<&str>,
    expected_work_item_id: Option<&str>,
    expected_grant: &ScopedGrantWire,
    now_ms: u64,
) -> bool {
    receipt.boot_id == expected_boot_id
        && receipt.workspace_id == expected_workspace_id
        && receipt.session_id == expected_session_id
        && receipt.agent_id == expected_agent_id
        && receipt.role == expected_role
        && receipt.root_session_id == expected_root_session_id
        && receipt.parent_session_id.as_deref() == expected_parent_session_id
        && receipt.delegation_id.as_deref() == expected_delegation_id
        && receipt.work_item_id.as_deref() == expected_work_item_id
        && receipt.grant == *expected_grant
        && receipt.expires_at_unix_ms > now_ms
}

/// Which review operation a physical reviewer admission must be granted.
///
/// Live admission is operation-exact: a contribution requires
/// `review.contribute`, the legacy adapter requires `review.record`.
/// Revalidation of already committed authority accepts either, because the
/// attested grant minted at admission time remains the authority regardless
/// of which operation name carried it.
#[derive(Clone, Copy, Debug)]
enum ReviewAdmission {
    Contribute,
    Record,
    Revalidate,
}

impl ReviewAdmission {
    fn permits(self, operation: &str) -> bool {
        match self {
            Self::Contribute => operation == OperationName::ReviewContribute.as_str(),
            Self::Record => operation == OperationName::ReviewRecord.as_str(),
            Self::Revalidate => {
                operation == OperationName::ReviewContribute.as_str()
                    || operation == OperationName::ReviewRecord.as_str()
            }
        }
    }

    const fn requires_live_ttl(self) -> bool {
        !matches!(self, Self::Revalidate)
    }
}

fn bind_reviewer(
    workspace: &BusinessWorkspace,
    work_item_id: &str,
    caller: &AuthenticatedCaller,
    persistence: &dyn PersistencePort,
    boots: AttestationBoots<'_>,
    now_ms: u64,
    admission: ReviewAdmission,
) -> RuntimeResult<BoundReviewer> {
    let sessions = persistence.list_identity_snapshots(RuntimeIdentityKind::Session)?;
    let mut by_id = BTreeMap::<String, RuntimeSessionRecord>::new();
    for snapshot in sessions {
        if snapshot.workspace.workspace_id != workspace.workspace_id {
            continue;
        }
        let Some(session) = snapshot.session else {
            return Err(identity_error(
                "typed session snapshot has no session record",
            ));
        };
        if by_id.insert(session.session_id.clone(), session).is_some() {
            return Err(identity_error("typed session identity is ambiguous"));
        }
    }
    let delegations = persistence.list_identity_snapshots(RuntimeIdentityKind::Delegation)?;
    let mut by_delegation = BTreeMap::new();
    for snapshot in delegations {
        if snapshot.workspace.workspace_id != workspace.workspace_id {
            continue;
        }
        let delegation = snapshot
            .delegation
            .ok_or_else(|| attestation_error("delegation snapshot lacks delegation authority"))?;
        let Some(attestation) = snapshot.attestation else {
            // A cancelled or otherwise incomplete delegation elsewhere in the
            // workspace is not part of this reviewer's physical lineage. The
            // target lineage still fails closed below when its delegation is
            // absent from this attested-authority map.
            continue;
        };
        if by_delegation
            .insert(delegation.delegation_id.clone(), (delegation, attestation))
            .is_some()
        {
            return Err(attestation_error("delegation identity is ambiguous"));
        }
    }

    let reviewer = admitted_session(
        by_id
            .get(&caller.session_id().to_string())
            .ok_or_else(|| identity_error("reviewer session has no typed authority"))?,
        now_ms,
        admission,
    )?;
    if reviewer.agent_id != caller.agent_id()
        || reviewer.workspace_id != workspace.workspace_id
        || reviewer.role != WireAgentRole::Reviewer
        || reviewer.current_work_item.as_deref() != Some(work_item_id)
    {
        return Err(identity_error(
            "authenticated reviewer differs from typed session authority",
        ));
    }
    let series_id = reviewer
        .parent_session_id
        .as_deref()
        .ok_or_else(|| identity_error("reviewer session lacks its Series parent"))?;
    let series = admitted_session(
        by_id
            .get(series_id)
            .ok_or_else(|| identity_error("reviewer Series parent has no typed authority"))?,
        now_ms,
        admission,
    )?;
    if series.role != WireAgentRole::Series
        || series.root_session_id != reviewer.root_session_id
        || series.parent_session_id.as_deref() != Some(reviewer.root_session_id.as_str())
    {
        return Err(identity_error(
            "reviewer lineage is not Root -> Series -> Reviewer",
        ));
    }
    let root = admitted_session(
        by_id
            .get(&reviewer.root_session_id)
            .ok_or_else(|| identity_error("review root has no typed authority"))?,
        now_ms,
        admission,
    )?;
    if root.role != WireAgentRole::Root
        || root.session_id != root.root_session_id
        || root.parent_session_id.is_some()
        || root.delegation_id.is_some()
    {
        return Err(identity_error("review root session authority is invalid"));
    }

    validate_child_authority(series, root, &by_delegation, boots, now_ms, admission)?;
    let reviewer_attestation =
        validate_child_authority(reviewer, series, &by_delegation, boots, now_ms, admission)?;

    let authors = by_id
        .values()
        .filter(|session| {
            session.role == WireAgentRole::Task
                && session.workspace_id == workspace.workspace_id
                && session.root_session_id == root.session_id
                && session.parent_session_id.as_deref() == Some(series.session_id.as_str())
                && session.current_work_item.as_deref() == Some(work_item_id)
                && session.engaged
                && session.status == "active"
                && (!admission.requires_live_ttl() || session.expires_at_unix_ms > now_ms)
        })
        .collect::<Vec<_>>();
    let [author] = authors.as_slice() else {
        return Err(identity_error(
            "review requires exactly one active typed Task author session",
        ));
    };
    validate_child_authority(author, series, &by_delegation, boots, now_ms, admission)?;

    let specialty = reviewer_specialty(&reviewer_attestation.grant)?;
    if reviewer.grant != reviewer_attestation.grant
        || !reviewer
            .grant
            .operations
            .iter()
            .any(|operation| admission.permits(operation.as_str()))
    {
        return Err(attestation_error(
            "reviewer grant differs from its physical attestation",
        ));
    }
    let grant_bytes = serde_json::to_vec(&reviewer_attestation.grant)
        .map_err(|_| attestation_error("reviewer grant could not be canonicalized"))?;
    let reviewer_value = AttestedReviewerV2::new(
        AgentRole::Reviewer,
        specialty,
        vec![specialty],
        SessionId::from_str(&reviewer.session_id)
            .map_err(|_| identity_error("reviewer sessionId is invalid"))?,
        SessionId::from_str(&root.session_id)
            .map_err(|_| identity_error("review root sessionId is invalid"))?,
        DelegationId::from_str(&reviewer_attestation.delegation_id)
            .map_err(|_| attestation_error("reviewer delegationId is invalid"))?,
        2,
        ReviewAuthorityRef::new(reviewer_attestation.attestation_ref.clone())
            .map_err(|_| attestation_error("reviewer attestation reference is invalid"))?,
        ArtifactDigest::from_str(&reviewer_attestation.attestation_digest)
            .map_err(|_| attestation_error("reviewer attestation digest is invalid"))?,
        ArtifactDigest::digest(grant_bytes),
    )
    .map_err(|_| attestation_error("reviewer attested identity is invalid"))?;
    Ok(BoundReviewer {
        reviewer: reviewer_value,
        specialty,
        author_session_id: SessionId::from_str(&author.session_id)
            .map_err(|_| identity_error("review author sessionId is invalid"))?,
        root_session_id: SessionId::from_str(&root.session_id)
            .map_err(|_| identity_error("review root sessionId is invalid"))?,
    })
}

fn admitted_session(
    session: &RuntimeSessionRecord,
    now_ms: u64,
    admission: ReviewAdmission,
) -> RuntimeResult<&RuntimeSessionRecord> {
    if !session.engaged
        || session.status != "active"
        || (admission.requires_live_ttl() && session.expires_at_unix_ms <= now_ms)
    {
        return Err(RuntimeError::new(
            StableErrorCode::SessionExpired,
            "review identity session is inactive or expired",
        ));
    }
    Ok(session)
}

fn validate_child_authority<'a>(
    child: &RuntimeSessionRecord,
    parent: &RuntimeSessionRecord,
    delegations: &'a BTreeMap<
        String,
        (RuntimeDelegationRecord, RuntimeDelegationAttestationRecord),
    >,
    boots: AttestationBoots<'_>,
    now_ms: u64,
    admission: ReviewAdmission,
) -> RuntimeResult<&'a RuntimeDelegationAttestationRecord> {
    let delegation_id = child
        .delegation_id
        .as_deref()
        .ok_or_else(|| attestation_error("child session lacks a delegation identity"))?;
    let (delegation, attestation) = delegations
        .get(delegation_id)
        .ok_or_else(|| attestation_error("child delegation has no typed attestation"))?;
    if delegation.workspace_id != child.workspace_id
        || delegation.root_session_id != child.root_session_id
        || delegation.parent_session_id != parent.session_id
        || delegation.child_session_id.as_deref() != Some(child.session_id.as_str())
        || delegation.delegation_id != delegation_id
        || delegation.role != child.role
        || delegation.status != "running"
        || (admission.requires_live_ttl() && delegation.deadline_unix_ms <= now_ms)
        || delegation.work_item_id.as_deref() != child.current_work_item.as_deref()
        || attestation.workspace_id != child.workspace_id
        || attestation.delegation_id != delegation_id
        || attestation.physical_session_id != child.session_id
        || attestation.grant != child.grant
        || !boots.permits(&attestation.accepted_boot_id)
    {
        return Err(attestation_error(
            "child session, delegation, and physical attestation do not strictly join",
        ));
    }
    Ok(attestation)
}

fn reviewer_specialty(grant: &ScopedGrantWire) -> RuntimeResult<ReviewerSpecialty> {
    let specialties = grant
        .capabilities
        .iter()
        .filter(|capability| SPECIALTY_CAPABILITIES.contains(&capability.as_str()))
        .map(String::as_str)
        .collect::<Vec<_>>();
    let [capability] = specialties.as_slice() else {
        return Err(attestation_error(
            "reviewer grant must carry exactly one specialty capability",
        ));
    };
    match *capability {
        "review.specialty.general" => Ok(ReviewerSpecialty::General),
        "review.specialty.be" => Ok(ReviewerSpecialty::Be),
        "review.specialty.ar" => Ok(ReviewerSpecialty::Ar),
        "review.specialty.qa" => Ok(ReviewerSpecialty::Qa),
        _ => unreachable!("filtered by the frozen specialty capability set"),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn select_or_start_session(
    existing: Option<ReviewSessionV2>,
    existing_batch: Option<&ReviewBatchV2>,
    tier: ReviewTier,
    repair_class: ReviewRepairClass,
    work_item_id: &str,
    author_session_id: SessionId,
    root_session_id: SessionId,
    input_fingerprint: InputFingerprint,
    ruleset_fingerprint: InputFingerprint,
    policy_digest: PolicyDigest,
    source_revision: u64,
    inventory_generation: u64,
    remediation_plan_fingerprint: Option<InputFingerprint>,
    observed: &ReviewTimestamp,
) -> RuntimeResult<(ReviewSessionV2, Option<ReviewRemediationV2>)> {
    if let Some(session) = existing {
        let drifted = session.input_fingerprint() != input_fingerprint
            || session.ruleset_fingerprint() != ruleset_fingerprint
            || session.policy_digest() != policy_digest
            || session.inventory_generation() != inventory_generation;
        if !drifted {
            if !session.status().is_terminal() {
                return Ok((session, None));
            }
            return Err(gate_blocked(
                "terminal review session rejects new attempts for the same input",
            ));
        }
        let parent_review_id = session.review_id().clone();
        let next_review_id = derived_review_id(
            work_item_id,
            input_fingerprint,
            ruleset_fingerprint,
            source_revision,
        )?;
        let mut remediations = session.counters().remediations();
        let remediation = if session.status() == ReviewSessionStatusV2::RemediationRequired {
            if session.input_fingerprint() == input_fingerprint
                || source_revision <= session.source_revision()
            {
                return Err(gate_blocked(
                    "review findings require a committed input-changing remediation",
                ));
            }
            let batch = existing_batch
                .filter(|batch| batch.review_id() == session.review_id())
                .filter(|batch| batch.latest_status() == ReviewBatchStatusV2::ValidFindings)
                .filter(|batch| batch.is_closed())
                .ok_or_else(|| {
                    external_conflict(
                        "remediation_required session lacks its closed findings batch",
                    )
                })?;
            let plan_fingerprint = remediation_plan_fingerprint.ok_or_else(|| {
                gate_blocked("review remediation requires a committed execution plan")
            })?;
            remediations = remediations
                .checked_add(1)
                .ok_or_else(|| gate_blocked("review remediation budget is exhausted"))?;
            Some(build_review_remediation(
                &parent_review_id,
                batch.batch_id().clone(),
                plan_fingerprint,
                input_fingerprint,
                next_review_id.clone(),
            )?)
        } else {
            None
        };
        let child = start_review_session(
            next_review_id,
            Some(parent_review_id),
            tier,
            repair_class,
            author_session_id,
            root_session_id,
            input_fingerprint,
            ruleset_fingerprint,
            policy_digest,
            source_revision,
            inventory_generation,
            remediations,
            observed,
        )?;
        return Ok((child, remediation));
    }
    start_review_session(
        derived_review_id(
            work_item_id,
            input_fingerprint,
            ruleset_fingerprint,
            source_revision,
        )?,
        None,
        tier,
        repair_class,
        author_session_id,
        root_session_id,
        input_fingerprint,
        ruleset_fingerprint,
        policy_digest,
        source_revision,
        inventory_generation,
        0,
        observed,
    )
    .map(|session| (session, None))
}

#[allow(clippy::too_many_arguments)]
fn start_review_session(
    review_id: ReviewId,
    parent_review_id: Option<ReviewId>,
    tier: ReviewTier,
    repair_class: ReviewRepairClass,
    author_session_id: SessionId,
    root_session_id: SessionId,
    input_fingerprint: InputFingerprint,
    ruleset_fingerprint: InputFingerprint,
    policy_digest: PolicyDigest,
    source_revision: u64,
    inventory_generation: u64,
    remediations: u32,
    observed: &ReviewTimestamp,
) -> RuntimeResult<ReviewSessionV2> {
    let budget = ae_sdd_contracts::review::ReviewBudgetV2::for_tier(tier);
    if remediations > budget.max_remediations() {
        return Err(gate_blocked("review remediation budget is exhausted"));
    }
    let deadline = add_minutes(observed, budget.max_wall_clock_minutes())?;
    let session = ReviewSessionV2::new(
        review_id,
        parent_review_id,
        tier,
        author_session_id,
        root_session_id,
        input_fingerprint,
        ruleset_fingerprint,
        policy_digest,
        source_revision,
        inventory_generation,
        repair_class,
        budget,
        observed.clone(),
        deadline,
    )
    .map_err(|_| schema_error("daemon-derived review session is invalid"))?;
    if remediations == 0 {
        return Ok(session);
    }
    session
        .transition(
            ae_sdd_contracts::review::ReviewCountersV2::new(0, 0, 0, remediations, 0, 0)
                .map_err(|_| schema_error("review remediation counters are invalid"))?,
            ReviewSessionStatusV2::Running,
            None,
        )
        .map_err(|_| schema_error("daemon-derived review child session is invalid"))
}

fn remediation_plan_fingerprint(state: &Value) -> RuntimeResult<Option<InputFingerprint>> {
    state
        .get("executionPlan")
        .map(|plan| {
            serde_json::to_vec(plan)
                .map(InputFingerprint::digest)
                .map_err(|_| schema_error("review remediation plan is not canonical"))
        })
        .transpose()
}

fn build_review_remediation(
    parent_review_id: &ReviewId,
    finding_batch_id: ReviewBatchId,
    plan_fingerprint: InputFingerprint,
    new_input_fingerprint: InputFingerprint,
    next_review_id: ReviewId,
) -> RuntimeResult<ReviewRemediationV2> {
    let digest = ArtifactDigest::digest(
        serde_json::to_vec(&json!({
            "domain":"review-remediation/v2",
            "reviewId":parent_review_id.as_str(),
            "findingBatchId":finding_batch_id.as_str(),
            "planFingerprint":plan_fingerprint.to_string(),
            "newInputFingerprint":new_input_fingerprint.to_string(),
            "nextReviewId":next_review_id.as_str(),
        }))
        .map_err(|_| schema_error("review remediation could not be canonicalized"))?,
    );
    Ok(ReviewRemediationV2::new(
        finding_batch_id,
        plan_fingerprint,
        new_input_fingerprint,
        next_review_id,
        digest,
    ))
}

fn validate_session_identity(
    session: &ReviewSessionV2,
    bound: &BoundReviewer,
) -> RuntimeResult<()> {
    if session.author_session_id() != bound.author_session_id
        || session.root_session_id() != bound.root_session_id
    {
        return Err(identity_error(
            "reviewer lineage differs from the active review session",
        ));
    }
    Ok(())
}

pub(crate) fn review_session_reuses_lineage(
    session: &ReviewSessionV2,
    input_fingerprint: InputFingerprint,
    ruleset_fingerprint: InputFingerprint,
    policy_digest: PolicyDigest,
    inventory_generation: u64,
) -> bool {
    session.input_fingerprint() == input_fingerprint
        && session.ruleset_fingerprint() == ruleset_fingerprint
        && session.policy_digest() == policy_digest
        && session.inventory_generation() == inventory_generation
}

fn parse_existing_session(state: &Value) -> RuntimeResult<Option<ReviewSessionV2>> {
    let Some(value) = state.get("reviewSession") else {
        return Ok(None);
    };
    if value.get("schemaVersion").and_then(Value::as_str) != Some("v2") {
        return Ok(None);
    }
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|_| schema_error("authoritative v2 reviewSession is malformed"))
}

fn parse_existing_batch(state: &Value) -> RuntimeResult<Option<ReviewBatchV2>> {
    let Some(value) = state.pointer("/review/batch") else {
        return Ok(None);
    };
    if value.get("schemaVersion").and_then(Value::as_str) != Some("v2") {
        return Ok(None);
    }
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|_| schema_error("authoritative v2 review batch is malformed"))
}

/// Contribution content that is independent of the attempt binding. The
/// attempt identity is only assigned when a finalize aggregates the pending
/// projection, so the same material can be re-bound without rewriting the
/// reviewer's attested content.
#[derive(Clone, Debug)]
struct ContributionMaterial {
    reviewer: AttestedReviewerV2,
    outcome: ReviewContributionOutcomeV2,
    findings: Vec<ReviewFinding>,
    report_digest: ArtifactDigest,
    input_fingerprint: InputFingerprint,
    ruleset_fingerprint: InputFingerprint,
}

fn review_report_digest(payload: &Map<String, Value>) -> RuntimeResult<ArtifactDigest> {
    let report_bytes = serde_json::to_vec(&json!({
        "status": payload.get("status"),
        "findings": payload.get("findings"),
        "reviewedPaths": payload.get("reviewedPaths"),
        "evidenceIds": payload.get("evidenceIds"),
    }))
    .map_err(|_| schema_error("review contribution report could not be canonicalized"))?;
    Ok(ArtifactDigest::digest(report_bytes))
}

fn contribution_digest_for(
    attempt_id: &ReviewAttemptId,
    reviewer: &AttestedReviewerV2,
    outcome: ReviewContributionOutcomeV2,
    report_digest: ArtifactDigest,
    input_fingerprint: InputFingerprint,
    ruleset_fingerprint: InputFingerprint,
) -> RuntimeResult<ArtifactDigest> {
    let reviewer_value = serde_json::to_value(reviewer)
        .map_err(|_| schema_error("reviewer identity could not be canonicalized"))?;
    Ok(ArtifactDigest::digest(
        serde_json::to_vec(&json!({
            "domain":"review-contribution/v2",
            "attemptId":attempt_id.as_str(),
            "specialty":specialty_name(reviewer.specialty()),
            "reviewer":reviewer_value,
            "outcome":outcome,
            "reportDigest":report_digest.to_string(),
            "inputFingerprint":input_fingerprint.to_string(),
            "rulesetFingerprint":ruleset_fingerprint.to_string(),
        }))
        .map_err(|_| schema_error("review contribution could not be canonicalized"))?,
    ))
}

fn build_contribution(
    attempt_id: ReviewAttemptId,
    material: &ContributionMaterial,
) -> RuntimeResult<ReviewerContributionV2> {
    let contribution_digest = contribution_digest_for(
        &attempt_id,
        &material.reviewer,
        material.outcome,
        material.report_digest,
        material.input_fingerprint,
        material.ruleset_fingerprint,
    )?;
    ReviewerContributionV2::new(
        attempt_id,
        material.reviewer.clone(),
        material.outcome,
        material.findings.clone(),
        material.report_digest,
        contribution_digest,
        material.input_fingerprint,
        material.ruleset_fingerprint,
    )
    .map_err(|_| schema_error("reviewer contribution is invalid"))
}

/// Re-derives the attempt-independent material of one stored pending
/// contribution so a finalize can bind it to the aggregating attempt. The
/// report digest travels inside the typed wire form because the frozen
/// contract exposes no accessor for it.
fn contribution_material(
    contribution: &ReviewerContributionV2,
) -> RuntimeResult<ContributionMaterial> {
    let value = serde_json::to_value(contribution)
        .map_err(|_| external_conflict("pending review contribution could not be canonicalized"))?;
    let report_digest = value
        .get("reportDigest")
        .and_then(Value::as_str)
        .and_then(|raw| ArtifactDigest::from_str(raw).ok())
        .ok_or_else(|| external_conflict("pending review contribution report digest is invalid"))?;
    Ok(ContributionMaterial {
        reviewer: contribution.reviewer().clone(),
        outcome: contribution.outcome(),
        findings: contribution.findings().to_vec(),
        report_digest,
        input_fingerprint: contribution.input_fingerprint(),
        ruleset_fingerprint: contribution.ruleset_fingerprint(),
    })
}

fn completes_specialty_projection(
    session: &ReviewSessionV2,
    prior_batch: Option<&ReviewBatchV2>,
    materials: &[ContributionMaterial],
) -> bool {
    let mut completed = prior_batch
        .into_iter()
        .flat_map(ReviewBatchV2::retained_contributions)
        .map(|contribution| contribution.reviewer().specialty())
        .collect::<BTreeSet<_>>();
    completed.extend(
        materials
            .iter()
            .map(|material| material.reviewer.specialty()),
    );
    completed
        == session
            .required_specialties()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
}

fn load_pending_contributions(state: &Value) -> RuntimeResult<Vec<ReviewerContributionV2>> {
    let Some(value) = state.pointer("/review/pendingContributions") else {
        return Ok(Vec::new());
    };
    value
        .as_array()
        .ok_or_else(|| external_conflict("pending review contributions are malformed"))?
        .iter()
        .map(|entry| {
            serde_json::from_value(entry.clone())
                .map_err(|_| external_conflict("pending review contribution is malformed"))
        })
        .collect()
}

fn load_pending_remediation(state: &Value) -> RuntimeResult<Option<ReviewRemediationV2>> {
    let Some(value) = state.pointer("/review/pendingRemediation") else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|_| external_conflict("pending review remediation is malformed"))
}

fn pending_batch_id(
    session: &ReviewSessionV2,
    prior_batch: Option<&ReviewBatchV2>,
) -> RuntimeResult<ReviewBatchId> {
    match prior_batch {
        Some(batch) => Ok(batch.batch_id().clone()),
        None => derived_batch_id(session),
    }
}

/// Shared caller/scope/revision validation for every review operation. The
/// daemon-derived role decides whether the caller may contribute (Reviewer)
/// or finalize (Root); returns the locked authoritative source revision.
fn validate_review_caller(
    workspace: &BusinessWorkspace,
    state: &Value,
    work_item_id: &str,
    request: &ValidatedOperationRequest,
    caller: &AuthenticatedCaller,
    expected_role: AgentRole,
) -> RuntimeResult<u64> {
    if caller.role() != expected_role {
        return Err(role_error(match expected_role {
            AgentRole::Reviewer => "only an authenticated reviewer may record a review",
            AgentRole::Root => "only the root finalizer may finalize a review batch",
            _ => "the authenticated role may not operate review authority",
        }));
    }
    if request.request().session_id.as_ref() != Some(&caller.session_id()) {
        return Err(identity_error(
            "review request session does not match the authenticated caller",
        ));
    }
    if request
        .request()
        .workspace_id
        .as_ref()
        .map(ToString::to_string)
        .as_deref()
        != Some(workspace.workspace_id.as_str())
    {
        return Err(identity_error(
            "review request workspace does not match the authenticated workspace",
        ));
    }
    if request
        .request()
        .work_item_id
        .as_ref()
        .map(|value| value.as_str())
        != Some(work_item_id)
    {
        return Err(identity_error(
            "review request Work Item does not match the locked authority",
        ));
    }
    let source_revision = state
        .get("revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| schema_error("authoritative state revision is missing"))?;
    if request.request().expected_revision.map(|value| value.get()) != Some(source_revision) {
        return Err(RuntimeError::new(
            StableErrorCode::RevisionConflict,
            "review expectedRevision does not match the locked authority",
        ));
    }
    Ok(source_revision)
}

fn decode_review_contribution_payload(
    payload: &Map<String, Value>,
) -> RuntimeResult<(ReviewContributionOutcomeV2, Vec<ReviewFinding>)> {
    let wire_status = required_string(payload.get("status"), "review status is required")?;
    let raw_findings = payload
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| schema_error("review findings must be an array"))?;
    let outcome = contribution_outcome(wire_status, raw_findings)?;
    let findings = dedup_findings(
        &raw_findings
            .iter()
            .map(project_finding)
            .collect::<RuntimeResult<Vec<_>>>()?,
    )
    .map_err(supervisor_error)?;
    Ok((outcome, findings))
}

struct OpenedReviewSession {
    session: ReviewSessionV2,
    remediation: Option<ReviewRemediationV2>,
    prior_batch: Option<ReviewBatchV2>,
    reused: bool,
}

/// Loads or starts the review session for one contribution admission and
/// returns the open batch projection the finalize pipeline aggregates into.
#[allow(clippy::too_many_arguments)]
fn open_review_session(
    state: &Value,
    work_item_id: &str,
    bound: &BoundReviewer,
    raw_findings: &[Value],
    input_fingerprint: InputFingerprint,
    ruleset_fingerprint: InputFingerprint,
    policy_digest: PolicyDigest,
    source_revision: u64,
    inventory_generation: u64,
    observed: &ReviewTimestamp,
) -> RuntimeResult<OpenedReviewSession> {
    let existing_session = parse_existing_session(state)?;
    let existing_batch = parse_existing_batch(state)?;
    if let Some(session) = existing_session.as_ref().filter(|session| {
        review_session_reuses_lineage(
            session,
            input_fingerprint,
            ruleset_fingerprint,
            policy_digest,
            inventory_generation,
        )
    }) {
        validate_session_identity(session, bound)?;
    }
    let existing_review_id = existing_session
        .as_ref()
        .map(|session| session.review_id().clone());
    let tier = existing_session
        .as_ref()
        .map_or_else(|| derive_tier(state), ReviewSessionV2::tier);
    let current_repair_class = derive_repair_class(state, raw_findings);
    let (session, remediation) = select_or_start_session(
        existing_session,
        existing_batch.as_ref(),
        tier,
        current_repair_class,
        work_item_id,
        bound.author_session_id,
        bound.root_session_id,
        input_fingerprint,
        ruleset_fingerprint,
        policy_digest,
        source_revision,
        inventory_generation,
        remediation_plan_fingerprint(state)?,
        observed,
    )?;
    validate_session_identity(&session, bound)?;

    if session.status() == ReviewSessionStatusV2::RemediationRequired
        && session.input_fingerprint() == input_fingerprint
        && session.ruleset_fingerprint() == ruleset_fingerprint
    {
        return Err(gate_blocked(
            "review findings require a committed input-changing remediation",
        ));
    }

    let prior_batch = existing_batch
        .as_ref()
        .filter(|batch| batch.review_id() == session.review_id())
        .filter(|batch| !batch.is_closed())
        .cloned();
    let reused = existing_review_id.as_ref() == Some(session.review_id());
    Ok(OpenedReviewSession {
        session,
        remediation,
        prior_batch,
        reused,
    })
}

fn review_observed_timestamp(observed_at: &UtcTimestamp) -> RuntimeResult<ReviewTimestamp> {
    ReviewTimestamp::new(observed_at.to_string())
        .map_err(|_| schema_error("daemon review timestamp is invalid"))
}

fn contribution_outcome(
    wire_status: &str,
    raw_findings: &[Value],
) -> RuntimeResult<ReviewContributionOutcomeV2> {
    match (wire_status, raw_findings.is_empty()) {
        ("passed", true) => Ok(ReviewContributionOutcomeV2::Clean),
        ("changes_required", false) => Ok(ReviewContributionOutcomeV2::Findings),
        ("pending", true) => Ok(ReviewContributionOutcomeV2::InfraFailure),
        ("passed", false) => Err(schema_error("review status=passed requires empty findings")),
        ("changes_required", true) => Err(schema_error(
            "review status=changes_required requires findings",
        )),
        ("pending", false) => Err(schema_error(
            "review status=pending requires empty findings",
        )),
        _ => Err(schema_error("review status is not registered")),
    }
}

#[allow(clippy::too_many_arguments)]
fn derive_final_proof(
    workspace: &BusinessWorkspace,
    state: &Value,
    work_item_id: &str,
    request: &ValidatedOperationRequest,
    persistence: &dyn PersistencePort,
    session: &ReviewSessionV2,
    completing_clean_attempt: bool,
    observed: &ReviewTimestamp,
) -> RuntimeResult<(ReviewFinalProofV2, Option<ProjectAuthorityMaterial>)> {
    if !completing_clean_attempt {
        return Ok((ReviewFinalProofV2::none(), None));
    }
    match session.clean_policy().final_proof_requirement() {
        ReviewFinalProofKind::None => Ok((ReviewFinalProofV2::none(), None)),
        ReviewFinalProofKind::DeterministicGates => {
            let runtime = AuthoritativeGateRuntime::new(
                workspace,
                work_item_id,
                &session.policy_digest().to_string(),
                request.request().fencing_token.map(|value| value.get()),
            )?;
            let mut results = Vec::with_capacity(DETERMINISTIC_REVIEW_GATES.len());
            for gate_id in DETERMINISTIC_REVIEW_GATES {
                let result = runtime.evaluate(gate_id, REVIEW_GATE_DEADLINE)?;
                if !matches!(result.outcome(), GateOutcome::Pass) {
                    return Err(gate_blocked(
                        "Tier 2 final deterministic Gate proof is not PASS",
                    ));
                }
                results.push(gate_result_json(&result));
            }
            let digest = ArtifactDigest::digest(
                serde_json::to_vec(&results)
                    .map_err(|_| schema_error("deterministic Gate proof is not canonical"))?,
            );
            let proof = ReviewFinalProofV2::bound(
                ReviewFinalProofKind::DeterministicGates,
                digest,
                session.source_revision(),
                session.input_fingerprint(),
                session.ruleset_fingerprint(),
                observed.clone(),
            )
            .map_err(|_| schema_error("deterministic Gate proof is invalid"))?;
            Ok((proof, None))
        }
        ReviewFinalProofKind::FinalVerification => {
            let material = latest_final_verification_authority(
                persistence,
                workspace,
                state,
                work_item_id,
                session,
                observed,
            )?;
            let proof = ReviewFinalProofV2::bound(
                ReviewFinalProofKind::FinalVerification,
                material.state_receipt_ref_digest,
                session.source_revision(),
                session.input_fingerprint(),
                session.ruleset_fingerprint(),
                observed.clone(),
            )
            .map_err(|_| schema_error("final verification proof is invalid"))?;
            Ok((proof, Some(material)))
        }
    }
}

/// Resolves the Tier 3 final verification authority: exactly one committed
/// PASS `toolset.receipt.record` job bound to the session fingerprints and
/// the active receipt/manifest/mutation locators. The verification scope is
/// deliberately not inspected here — under the incremental-testing strategy
/// it is incremental, with the full suite reserved for release/distribution
/// gates.
fn latest_final_verification_authority(
    persistence: &dyn PersistencePort,
    workspace: &BusinessWorkspace,
    state: &Value,
    work_item_id: &str,
    session: &ReviewSessionV2,
    observed: &ReviewTimestamp,
) -> RuntimeResult<ProjectAuthorityMaterial> {
    let state_ref = state.get("toolsetReceiptRef").and_then(Value::as_object);
    let state_job_id = state_ref
        .and_then(|value| value.get("toolsetJobId"))
        .and_then(Value::as_str)
        .ok_or_else(|| gate_blocked("Tier 3 requires an active toolset receipt authority"))?;
    let state_receipt_digest = state_ref
        .and_then(|value| value.get("projectReceiptDigest"))
        .and_then(Value::as_str)
        .ok_or_else(|| gate_blocked("Tier 3 toolset receipt has no project digest"))?;
    let state_manifest_digest = state_ref
        .and_then(|value| value.get("manifestDigest"))
        .and_then(Value::as_str)
        .ok_or_else(|| gate_blocked("Tier 3 toolset receipt has no active manifest digest"))?;
    let state_mutation_id = state_ref
        .and_then(|value| value.get("mutationId"))
        .and_then(Value::as_str)
        .ok_or_else(|| gate_blocked("Tier 3 toolset receipt has no mutation authority"))?;
    let state_locator = state_ref
        .and_then(|value| value.get("artifactRef"))
        .and_then(Value::as_str)
        .ok_or_else(|| gate_blocked("Tier 3 toolset receipt has no immutable locator"))?;
    let final_binding = state
        .get("finalVerificationBinding")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            gate_blocked("Tier 3 requires daemon-bound final verification provenance")
        })?;
    let binding_string = |field: &str| {
        final_binding
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| gate_blocked("final verification provenance is incomplete"))
    };
    let terminal_methodology =
        ArtifactDigest::digest(b"ae-sdd/finalized-evidence-receipt/v1").to_string();
    if binding_string("reviewId")? != session.review_id().as_str()
        || binding_string("toolsetJobId")? != state_job_id
        || binding_string("inputFingerprint")? != session.input_fingerprint().to_string()
        || binding_string("rulesetFingerprint")? != session.ruleset_fingerprint().to_string()
        || binding_string("policyDigest")? != session.policy_digest().to_string()
        || binding_string("methodologyDigest")? != terminal_methodology
        || final_binding.get("sourceRevision").and_then(Value::as_u64)
            != Some(session.source_revision())
        || final_binding
            .get("inventoryGeneration")
            .and_then(Value::as_u64)
            != Some(workspace.inventory_generation)
        || binding_string("receiptDigest")?
            != state_ref
                .and_then(|value| value.get("receiptDigest"))
                .and_then(Value::as_str)
                .ok_or_else(|| gate_blocked("Tier 3 toolset receipt has no receipt digest"))?
    {
        return Err(gate_blocked(
            "final verification provenance differs from the active Review binding",
        ));
    }

    let mut matches = persistence
        .list_jobs()?
        .into_iter()
        .filter(|job| {
            job.job_id == state_job_id
                && job.workspace_id == workspace.workspace_id
                && job.work_item_id.as_deref() == Some(work_item_id)
                && job.entrypoint == "toolset.receipt.record"
                && job.status == RuntimeJobStatus::Pass
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(gate_blocked(
            "Tier 3 requires exactly one active committed PASS verification job",
        ));
    }
    let job = matches.pop().expect("length checked");
    validate_final_verification_job(
        &job,
        session,
        workspace.inventory_generation,
        state_receipt_digest,
        state_manifest_digest,
        state_mutation_id,
        state_locator,
        final_binding,
        observed,
    )
}

/// Binds the single PASS verification job to the session: revision,
/// fingerprints, inventory generation, digests, and committed locators must
/// all match. Test scope (incremental vs full) is not part of this contract.
#[allow(clippy::too_many_arguments)]
fn validate_final_verification_job(
    job: &RuntimeJobRecord,
    session: &ReviewSessionV2,
    inventory_generation: u64,
    state_receipt_digest: &str,
    state_manifest_digest: &str,
    state_mutation_id: &str,
    state_locator: &str,
    final_binding: &Map<String, Value>,
    observed: &ReviewTimestamp,
) -> RuntimeResult<ProjectAuthorityMaterial> {
    let result = job
        .result
        .as_ref()
        .and_then(Value::as_object)
        .ok_or_else(|| gate_blocked("verification job has no typed PASS result"))?;
    let result_string = |field: &str| {
        result
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| gate_blocked("verification PASS result is incomplete"))
    };
    let project_digest = job
        .project_receipt_digest
        .as_deref()
        .ok_or_else(|| gate_blocked("verification job is not project-committed"))?;
    let locator = job
        .receipt_locator
        .as_deref()
        .ok_or_else(|| gate_blocked("verification job has no committed receipt locator"))?;
    let mutation_id = job
        .mutation_id
        .as_deref()
        .ok_or_else(|| gate_blocked("verification job has no committed mutation"))?;
    let finished = job
        .finished_at_unix_ms
        .ok_or_else(|| gate_blocked("verification job has no terminal timestamp"))?;
    let finished = review_timestamp_from_unix_ms(finished)?;
    if job.source_revision != Some(session.source_revision())
        || job.input_fingerprint.as_deref() != Some(&session.input_fingerprint().to_string())
        || job.inventory_generation != inventory_generation
        || result.get("outcome").and_then(Value::as_str) != Some("PASS")
        || result.get("validated").and_then(Value::as_bool) != Some(true)
        || result.get("sourceRevision").and_then(Value::as_u64) != Some(session.source_revision())
        || result.get("inventoryGeneration").and_then(Value::as_u64) != Some(inventory_generation)
        || result_string("policyDigest")? != session.policy_digest().to_string()
        || result_string("inputFingerprint")? != session.input_fingerprint().to_string()
        || result_string("projectReceiptDigest")? != project_digest
        || result_string("manifestDigest")? != state_manifest_digest
        || result
            .get("finalVerificationBinding")
            .and_then(Value::as_object)
            != Some(final_binding)
        || result_string("planDigest")?
            != final_binding
                .get("planDigest")
                .and_then(Value::as_str)
                .ok_or_else(|| gate_blocked("final verification provenance has no plan digest"))?
        || result_string("receiptDigest")?
            != final_binding
                .get("receiptDigest")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    gate_blocked("final verification provenance has no receipt digest")
                })?
        || project_digest != state_receipt_digest
        || locator != state_locator
        || mutation_id != state_mutation_id
        || finished > *observed
    {
        return Err(gate_blocked(
            "verification PASS authority is stale or differs from the active review input",
        ));
    }
    Ok(ProjectAuthorityMaterial {
        project_receipt_ref: locator.to_owned(),
        active_manifest_digest: ArtifactDigest::from_str(state_manifest_digest)
            .map_err(|_| gate_blocked("active manifest digest is invalid"))?,
        state_receipt_ref_digest: ArtifactDigest::from_str(project_digest)
            .map_err(|_| gate_blocked("project receipt digest is invalid"))?,
        journal_mutation_id: mutation_id.to_owned(),
    })
}

fn build_project_authority(
    workspace: &BusinessWorkspace,
    state: &Value,
    work_item_id: &str,
    request: &ValidatedOperationRequest,
    source_revision: u64,
    material: Option<ProjectAuthorityMaterial>,
) -> RuntimeResult<ReviewProjectAuthorityV2> {
    let material = material.unwrap_or_else(|| {
        let state_bytes = serde_json::to_vec(state).unwrap_or_default();
        let state_digest = ArtifactDigest::digest(&state_bytes);
        let manifest_path = Path::new(&workspace.canonical_root).join(format!(
            ".auto-engineering/{work_item_id}/evidence/manifest.json"
        ));
        let active_manifest_digest =
            fs::read(manifest_path).map_or(state_digest, ArtifactDigest::digest);
        let idempotency = request
            .request()
            .idempotency_key
            .as_deref()
            .unwrap_or("missing");
        ProjectAuthorityMaterial {
            project_receipt_ref: format!(
                "state:{}:{work_item_id}:{source_revision}",
                workspace.workspace_id
            ),
            active_manifest_digest,
            state_receipt_ref_digest: state_digest,
            journal_mutation_id: derived_identifier(
                "review-mutation",
                &[
                    work_item_id.as_bytes(),
                    idempotency.as_bytes(),
                    request.payload_digest(),
                ],
            ),
        }
    });
    Ok(ReviewProjectAuthorityV2::new(
        ReviewAuthorityRef::new(material.project_receipt_ref)
            .map_err(|_| schema_error("review project receipt reference is invalid"))?,
        material.active_manifest_digest,
        material.state_receipt_ref_digest,
        ReviewMutationId::new(material.journal_mutation_id)
            .map_err(|_| schema_error("review mutation identity is invalid"))?,
    ))
}

fn effective_batch_findings(batch: &ReviewBatchV2) -> RuntimeResult<Vec<ReviewFinding>> {
    dedup_findings(
        &batch
            .retained_contributions()
            .iter()
            .flat_map(|contribution| contribution.findings().iter().cloned())
            .collect::<Vec<_>>(),
    )
    .map_err(supervisor_error)
}

fn projected_status(
    session: &ReviewSessionV2,
    batch: &ReviewBatchV2,
    findings: &[ReviewFinding],
) -> &'static str {
    if session.status() == ReviewSessionStatusV2::Completed {
        "passed"
    } else if !findings.is_empty()
        || batch.latest_status() == ae_sdd_contracts::review::ReviewBatchStatusV2::ValidFindings
    {
        "changes_required"
    } else {
        "pending"
    }
}

fn project_finding(value: &Value) -> RuntimeResult<ReviewFinding> {
    let object = value
        .as_object()
        .ok_or_else(|| schema_error("review findings must be objects"))?;
    let severity = required_string(
        object.get("severity"),
        "review finding severity is required",
    )?;
    let severity = match severity.to_ascii_lowercase().as_str() {
        "p0" | "blocker" | "critical" => ReviewFindingSeverity::Blocker,
        "p1" | "p2" | "major" | "high" | "medium" => ReviewFindingSeverity::Major,
        "p3" | "p4" | "minor" | "low" | "info" => ReviewFindingSeverity::Minor,
        _ => return Err(schema_error("review finding severity is invalid")),
    };
    let code = object
        .get("code")
        .or_else(|| object.get("reasonCode"))
        .or_else(|| object.get("rule"))
        .or_else(|| object.get("sectionId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| ReasonCode::new(value.to_owned()))
        .transpose()
        .map_err(|_| schema_error("review finding code is invalid"))?
        .unwrap_or_else(|| generated_finding_code(value));
    let summary = object
        .get("summary")
        .or_else(|| object.get("problem"))
        .or_else(|| object.get("message"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| code.as_str());
    let summary = BoundedText::<1024>::new(summary.to_owned())
        .map_err(|_| schema_error("review finding summary exceeds its v2 bound"))?;
    Ok(ReviewFinding::new(code, severity, summary))
}

fn generated_finding_code(value: &Value) -> ReasonCode {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let digest = InputFingerprint::digest(bytes).to_string();
    ReasonCode::new(format!("review.{}", &digest[..24]))
        .expect("generated finding code is a bounded portable identifier")
}

fn derive_tier(state: &Value) -> ReviewTier {
    match state.get("scale").and_then(Value::as_str) {
        Some("large" | "\u{5927}") => ReviewTier::Tier3,
        Some("medium" | "\u{4e2d}") => ReviewTier::Tier2,
        // Small, micro, missing, and unknown values conservatively remain Tier 1.
        _ => ReviewTier::Tier1,
    }
}

fn derive_repair_class(state: &Value, findings: &[Value]) -> ReviewRepairClass {
    let critical_contract = state
        .pointer("/executionPlan/changedPaths")
        .and_then(Value::as_array)
        .is_some_and(|paths| {
            paths.iter().filter_map(Value::as_str).any(|path| {
                path.starts_with("migrations/")
                    || path.contains("contracts")
                    || path.ends_with(".sql")
                    || path.ends_with("schema.json")
            })
        });
    if critical_contract {
        return ReviewRepairClass::CriticalContract;
    }
    let high_risk = findings.iter().any(|finding| {
        finding
            .get("severity")
            .and_then(Value::as_str)
            .is_some_and(|severity| {
                matches!(
                    severity.to_ascii_lowercase().as_str(),
                    "p0" | "p1" | "p2" | "blocker" | "critical" | "major" | "high"
                )
            })
    });
    if high_risk {
        ReviewRepairClass::HighRisk
    } else {
        ReviewRepairClass::None
    }
}

/// Computes the Review input over the locked state plus the same relevant
/// source/configuration inventory used by native Gate snapshots. Runtime,
/// generated, monitor, and evidence directories remain excluded so review
/// projection writes cannot invalidate their own input.
pub(crate) fn authoritative_review_workspace_input_fingerprint(
    workspace: &BusinessWorkspace,
    state: &Value,
) -> RuntimeResult<InputFingerprint> {
    let root = Path::new(&workspace.canonical_root)
        .canonicalize()
        .map_err(|_| external_conflict("review workspace root cannot be canonicalized"))?;
    if !root.is_dir() {
        return Err(external_conflict(
            "review workspace root is not a directory",
        ));
    }
    let mut hasher = Sha256::new();
    hasher.update(b"ae-sdd-authoritative-review-input/v2\0");
    review_hash_part(&mut hasher, &review_state_authority_bytes(state)?);
    for (relative, path) in review_workspace_inputs(&root)? {
        review_hash_part(&mut hasher, relative.as_bytes());
        let metadata =
            fs::metadata(&path).map_err(|_| external_conflict("review input metadata changed"))?;
        if metadata.len() <= REVIEW_INPUT_CONTENT_LIMIT {
            review_hash_part(
                &mut hasher,
                &fs::read(path).map_err(|_| external_conflict("review input became unreadable"))?,
            );
        } else {
            review_hash_part(&mut hasher, &metadata.len().to_be_bytes());
        }
    }
    Ok(InputFingerprint::from_array(hasher.finalize().into()))
}

fn review_state_authority_bytes(state: &Value) -> RuntimeResult<Vec<u8>> {
    let mut authority = state.clone();
    strip_derived_review_fields(&mut authority);
    serde_json::to_vec(&authority)
        .map_err(|_| schema_error("authoritative review input could not be fingerprinted"))
}

fn review_hash_part(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn review_workspace_inputs(root: &Path) -> RuntimeResult<Vec<(String, std::path::PathBuf)>> {
    let mut output = Vec::new();
    collect_review_inputs(root, root, &mut output)?;
    output.sort_by(|left, right| left.0.cmp(&right.0));
    output.dedup_by(|left, right| left.0 == right.0);
    Ok(output)
}

fn collect_review_inputs(
    root: &Path,
    directory: &Path,
    output: &mut Vec<(String, std::path::PathBuf)>,
) -> RuntimeResult<()> {
    let mut entries = fs::read_dir(directory)
        .map_err(|_| external_conflict("review input directory is unreadable"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| external_conflict("review input directory entry is unreadable"))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| external_conflict("review input metadata is unreadable"))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let relative = review_relative_path(root, &path)?;
        if metadata.is_dir() {
            let name = relative
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            if REVIEW_EXCLUDED_DIRS.contains(&name.as_str())
                || name == ".auto-engineering"
                || relative.eq_ignore_ascii_case("apps/ae-sdd-monitor")
            {
                continue;
            }
            collect_review_inputs(root, &path, output)?;
        } else if metadata.is_file() && relevant_review_input(&relative) {
            if output.len() >= REVIEW_INPUT_FILE_LIMIT {
                return Err(external_conflict(
                    "review input inventory exceeds the file limit",
                ));
            }
            output.push((relative, path));
        }
    }
    Ok(())
}

fn relevant_review_input(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some(
            "c" | "cc"
                | "cpp"
                | "go"
                | "h"
                | "hpp"
                | "java"
                | "js"
                | "json"
                | "jsx"
                | "kt"
                | "kts"
                | "md"
                | "properties"
                | "ps1"
                | "py"
                | "rs"
                | "sh"
                | "toml"
                | "ts"
                | "tsx"
                | "xml"
                | "yaml"
                | "yml"
        )
    )
}

fn review_relative_path(root: &Path, path: &Path) -> RuntimeResult<String> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| external_conflict("review input path escaped the workspace"))
}

/// Enforces the minimum independent-review depth for a clean contribution.
pub(crate) fn validate_clean_contribution_depth(
    workspace: &BusinessWorkspace,
    state: &Value,
    work_item_id: &str,
    payload: &Map<String, Value>,
    input_fingerprint: InputFingerprint,
) -> RuntimeResult<()> {
    let reviewed_paths = nonempty_unique_strings(payload, "reviewedPaths")?;
    let evidence_ids = nonempty_unique_strings(payload, "evidenceIds")?;
    let root = Path::new(&workspace.canonical_root)
        .canonicalize()
        .map_err(|_| external_conflict("review workspace root cannot be canonicalized"))?;
    let approved = state
        .pointer("/executionPlan/changedPaths")
        .and_then(Value::as_array)
        .ok_or_else(|| gate_blocked("review depth requires approved executionPlan.changedPaths"))?;
    let approved = approved
        .iter()
        .filter_map(Value::as_str)
        .map(|path| canonical_workspace_path(&root, path))
        .collect::<RuntimeResult<Vec<_>>>()?;
    if approved.is_empty() {
        return Err(gate_blocked(
            "review depth requires a non-empty approved changed-path scope",
        ));
    }
    for reviewed in reviewed_paths {
        let reviewed = canonical_workspace_path(&root, reviewed)?;
        if !approved.iter().any(|approved| {
            reviewed == *approved || (approved.is_dir() && reviewed.starts_with(approved))
        }) {
            return Err(gate_blocked(
                "reviewed path is outside the approved execution-plan scope",
            ));
        }
    }
    let story_id = review_evidence_story_id(state, work_item_id)?;
    validate_review_evidence_manifest(&root, state, story_id, &evidence_ids, input_fingerprint)
}

fn nonempty_unique_strings<'a>(
    payload: &'a Map<String, Value>,
    field: &'static str,
) -> RuntimeResult<Vec<&'a str>> {
    let values = payload
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| schema_error(&format!("clean review {field} must be an array")))?;
    if values.is_empty() {
        return Err(schema_error(&format!(
            "clean review {field} must not be empty"
        )));
    }
    let mut unique = BTreeSet::new();
    let mut output = Vec::with_capacity(values.len());
    for value in values {
        let value = value
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                schema_error(&format!("clean review {field} entries must be strings"))
            })?;
        if !unique.insert(value) {
            return Err(schema_error(&format!(
                "clean review {field} entries must be unique"
            )));
        }
        output.push(value);
    }
    Ok(output)
}

fn canonical_workspace_path(root: &Path, raw: &str) -> RuntimeResult<std::path::PathBuf> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(schema_error("reviewed path must not be empty"));
    }
    let candidate = Path::new(raw);
    let candidate = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    let canonical = candidate
        .canonicalize()
        .map_err(|_| gate_blocked("reviewed or approved path does not exist"))?;
    if !canonical.starts_with(root) {
        return Err(gate_blocked("reviewed path escaped the workspace"));
    }
    Ok(canonical)
}

fn validate_review_evidence_manifest(
    root: &Path,
    state: &Value,
    story_id: &str,
    evidence_ids: &[&str],
    input_fingerprint: InputFingerprint,
) -> RuntimeResult<()> {
    let manifest_ref = format!(".auto-engineering/{story_id}/evidence/manifest.json");
    let manifest_path = root.join(&manifest_ref);
    let bytes = fs::read(&manifest_path)
        .map_err(|_| gate_blocked("clean review requires a finalized evidence manifest"))?;
    if bytes.is_empty() || bytes.len() > REVIEW_MANIFEST_BYTE_LIMIT {
        return Err(gate_blocked(
            "finalized evidence manifest exceeds its byte bound",
        ));
    }
    validate_review_evidence_authority(root, state, story_id, &manifest_ref, &bytes)?;
    let manifest: Value = serde_json::from_slice(&bytes)
        .map_err(|_| external_conflict("finalized evidence manifest is malformed"))?;
    if manifest.get("schemaVersion").and_then(Value::as_u64) != Some(1)
        || manifest.get("storyId").and_then(Value::as_str) != Some(story_id)
    {
        return Err(external_conflict(
            "finalized evidence manifest scope is invalid",
        ));
    }
    let expected = manifest
        .get("contentHash")
        .and_then(Value::as_str)
        .ok_or_else(|| gate_blocked("evidence manifest has not been finalized"))?;
    if expected != review_manifest_content_hash(&manifest)? {
        return Err(external_conflict(
            "finalized evidence manifest contentHash is invalid",
        ));
    }
    let entries = manifest
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| external_conflict("finalized evidence entries are missing"))?;
    for evidence_id in evidence_ids {
        let matching = entries
            .iter()
            .filter(|entry| {
                entry.get("evidenceId").and_then(Value::as_str) == Some(*evidence_id)
                    && entry.get("status").and_then(Value::as_str) == Some("active")
            })
            .collect::<Vec<_>>();
        let [entry] = matching.as_slice() else {
            return Err(gate_blocked(
                "clean review evidenceId is not one active manifest entry",
            ));
        };
        let observed = entry
            .get("inputFingerprint")
            .and_then(Value::as_str)
            .and_then(parse_manifest_input_fingerprint)
            .ok_or_else(|| gate_blocked("clean review evidence input fingerprint is invalid"))?;
        if observed != input_fingerprint {
            return Err(gate_blocked(
                "clean review evidence is stale for the current Review input",
            ));
        }
    }
    // When the story has an append-only ledger it is the evidence truth: the
    // hash chain must verify and every cited evidenceId must be backed by a
    // recorded event. A legacy manifest without a ledger keeps the checks
    // above unchanged.
    if let Some(events) = verify_review_evidence_ledger(root, story_id)? {
        let recorded = events
            .iter()
            .filter(|event| event.kind() == EvidenceLedgerEventKind::Recorded)
            .map(|event| event.event_id().as_str())
            .collect::<BTreeSet<_>>();
        for evidence_id in evidence_ids {
            if !recorded.contains(evidence_id) {
                return Err(gate_blocked(
                    "clean review evidenceId is not backed by the evidence ledger",
                ));
            }
        }
    }
    Ok(())
}

fn validate_review_evidence_authority(
    root: &Path,
    state: &Value,
    story_id: &str,
    manifest_ref: &str,
    manifest_bytes: &[u8],
) -> RuntimeResult<()> {
    let Some(authority) = state.get("evidenceAuthority") else {
        // Legacy states predate the projection. Their manifests retain the
        // contentHash and optional ledger-chain checks below.
        return Ok(());
    };
    let authority = authority
        .as_object()
        .ok_or_else(|| external_conflict("state evidence authority is malformed"))?;
    let ledger_ref = format!(".auto-engineering/{story_id}/evidence/ledger.jsonl");
    for (field, expected) in [
        ("manifestRef", manifest_ref),
        ("ledgerRef", ledger_ref.as_str()),
    ] {
        if authority.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(external_conflict(
                "state evidence authority reference differs from the Review evidence path",
            ));
        }
    }
    let ledger_bytes = fs::read(root.join(&ledger_ref))
        .map_err(|_| external_conflict("state-bound evidence ledger is missing"))?;
    for (field, observed) in [
        ("manifestDigest", manifest_bytes),
        ("ledgerDigest", ledger_bytes.as_slice()),
    ] {
        let expected = authority
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| external_conflict("state evidence authority digest is missing"))?;
        let actual = format!("sha256:{}", ArtifactDigest::digest(observed));
        if expected != actual {
            return Err(external_conflict(
                "Review evidence digest differs from state evidence authority",
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_state_bound_review_evidence(
    workspace: &BusinessWorkspace,
    state: &Value,
    work_item_id: &str,
) -> RuntimeResult<()> {
    if state.get("evidenceAuthority").is_none() {
        return Ok(());
    }
    let root = Path::new(&workspace.canonical_root)
        .canonicalize()
        .map_err(|_| external_conflict("review workspace root cannot be canonicalized"))?;
    let story_id = review_evidence_story_id(state, work_item_id)?;
    let manifest_ref = format!(".auto-engineering/{story_id}/evidence/manifest.json");
    let manifest_bytes = fs::read(root.join(&manifest_ref))
        .map_err(|_| external_conflict("state-bound evidence manifest is missing"))?;
    validate_review_evidence_authority(&root, state, story_id, &manifest_ref, &manifest_bytes)
}

pub(crate) fn validate_finalized_review_evidence(
    workspace: &BusinessWorkspace,
    state: &Value,
    work_item_id: &str,
    input_fingerprint: InputFingerprint,
) -> RuntimeResult<Vec<u8>> {
    let root = Path::new(&workspace.canonical_root)
        .canonicalize()
        .map_err(|_| external_conflict("review workspace root cannot be canonicalized"))?;
    let story_id = review_evidence_story_id(state, work_item_id)?;
    let manifest_ref = format!(".auto-engineering/{story_id}/evidence/manifest.json");
    let manifest_bytes = fs::read(root.join(&manifest_ref)).map_err(|_| {
        gate_blocked("terminal verification requires a finalized evidence manifest")
    })?;
    if manifest_bytes.is_empty() || manifest_bytes.len() > REVIEW_MANIFEST_BYTE_LIMIT {
        return Err(gate_blocked(
            "finalized evidence manifest exceeds its byte bound",
        ));
    }
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| external_conflict("finalized evidence manifest is malformed"))?;
    let entries = manifest
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| external_conflict("finalized evidence entries are missing"))?;
    let active = entries
        .iter()
        .filter(|entry| {
            entry.get("status").and_then(Value::as_str) == Some("active")
                && entry.get("kind").and_then(Value::as_str) != Some("red-observed")
                && entry
                    .get("inputFingerprint")
                    .and_then(Value::as_str)
                    .and_then(parse_manifest_input_fingerprint)
                    == Some(input_fingerprint)
        })
        .collect::<Vec<_>>();
    if active.is_empty()
        || active
            .iter()
            .any(|entry| entry.get("exitCode").and_then(Value::as_i64) != Some(0))
    {
        return Err(gate_blocked(
            "finalized evidence does not establish a successful verification result",
        ));
    }
    let evidence_ids = active
        .iter()
        .map(|entry| {
            entry
                .get("evidenceId")
                .and_then(Value::as_str)
                .ok_or_else(|| external_conflict("finalized evidenceId is missing"))
        })
        .collect::<RuntimeResult<Vec<_>>>()?;
    validate_review_evidence_manifest(&root, state, story_id, &evidence_ids, input_fingerprint)?;
    let events = verify_review_evidence_ledger(&root, story_id)?
        .ok_or_else(|| gate_blocked("terminal verification requires an evidence ledger"))?;
    let finalized = events
        .last()
        .filter(|event| event.kind() == EvidenceLedgerEventKind::Finalized)
        .ok_or_else(|| gate_blocked("terminal verification requires a finalized ledger event"))?;
    let [manifest_artifact] = finalized.artifact_refs() else {
        return Err(external_conflict(
            "finalized ledger event must bind exactly one manifest artifact",
        ));
    };
    let manifest_digest = ArtifactDigest::digest(&manifest_bytes);
    if finalized.input_fingerprint() != InputFingerprint::digest(&manifest_bytes)
        || manifest_artifact.path().as_str() != manifest_ref
        || manifest_artifact.digest() != manifest_digest
        || manifest_artifact.byte_length()
            != u64::try_from(manifest_bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(external_conflict(
            "finalized ledger event does not bind the sealed manifest",
        ));
    }
    for entry in active {
        let artifacts = entry
            .get("artifacts")
            .and_then(Value::as_array)
            .filter(|artifacts| !artifacts.is_empty())
            .ok_or_else(|| gate_blocked("finalized evidence has no immutable artifact snapshot"))?;
        for artifact in artifacts {
            let snapshot_path = artifact
                .get("snapshotPath")
                .and_then(Value::as_str)
                .ok_or_else(|| external_conflict("evidence snapshotPath is missing"))?;
            let expected = artifact
                .get("sha256")
                .and_then(Value::as_str)
                .and_then(|value| value.strip_prefix("sha256:").or(Some(value)))
                .ok_or_else(|| external_conflict("evidence snapshot digest is missing"))?;
            let snapshot = root
                .join(snapshot_path)
                .canonicalize()
                .map_err(|_| external_conflict("evidence snapshot is missing"))?;
            if !snapshot.starts_with(&root) || !snapshot.is_file() {
                return Err(external_conflict(
                    "evidence snapshot escaped the registered workspace",
                ));
            }
            let bytes = fs::read(snapshot)
                .map_err(|_| external_conflict("evidence snapshot is unreadable"))?;
            if ArtifactDigest::digest(&bytes).to_string() != expected {
                return Err(external_conflict(
                    "evidence snapshot digest differs from the manifest",
                ));
            }
            if let Some(expected_bytes) = artifact.get("byteLength").and_then(Value::as_u64)
                && u64::try_from(bytes.len()).unwrap_or(u64::MAX) != expected_bytes
            {
                return Err(external_conflict(
                    "evidence snapshot byteLength differs from the manifest",
                ));
            }
        }
    }
    Ok(manifest_bytes)
}

fn review_evidence_story_id<'a>(state: &'a Value, work_item_id: &'a str) -> RuntimeResult<&'a str> {
    if state
        .get("storyStates")
        .and_then(Value::as_object)
        .is_some_and(|stories| stories.contains_key(work_item_id))
    {
        return Ok(work_item_id);
    }
    ["activeStory", "currentStory"]
        .into_iter()
        .find_map(|field| state.get(field).and_then(Value::as_str))
        .filter(|story| !story.is_empty())
        .ok_or_else(|| schema_error("Review evidence requires an authoritative Story scope"))
}

/// Loads and verifies the append-only evidence ledger for one story, failing
/// closed on any hash-chain, decode or canonical-form violation. Mirrors the
/// owner implementation in `operation_semantics::evidence`; duplicated here so
/// this module stays includable standalone by Review authority tests.
fn verify_review_evidence_ledger(
    root: &Path,
    story_id: &str,
) -> RuntimeResult<Option<Vec<EvidenceLedgerEventV1>>> {
    let path = root.join(format!(
        ".auto-engineering/{story_id}/evidence/ledger.jsonl"
    ));
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(&path)
        .map_err(|_| external_conflict("clean review evidence ledger is unreadable"))?;
    if bytes.is_empty() || bytes.len() > REVIEW_MANIFEST_BYTE_LIMIT {
        return Err(external_conflict(
            "clean review evidence ledger exceeds its byte bound",
        ));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| external_conflict("clean review evidence ledger is not UTF-8"))?;
    if !text.ends_with('\n') {
        return Err(external_conflict(
            "clean review evidence ledger is truncated",
        ));
    }
    let mut events = Vec::new();
    for line in text.lines() {
        let event: EvidenceLedgerEventV1 = serde_json::from_str(line)
            .map_err(|_| external_conflict("clean review evidence ledger event is invalid"))?;
        if line.as_bytes() != event.canonical_json() {
            return Err(external_conflict(
                "clean review evidence ledger event is not canonical",
            ));
        }
        events.push(event);
    }
    EvidenceLedgerEventV1::verify_chain(&events)
        .map_err(|_| external_conflict("clean review evidence ledger hash chain is broken"))?;
    Ok(Some(events))
}

fn parse_manifest_input_fingerprint(value: &str) -> Option<InputFingerprint> {
    InputFingerprint::from_str(value.strip_prefix("sha256:").unwrap_or(value)).ok()
}

fn review_manifest_content_hash(manifest: &Value) -> RuntimeResult<String> {
    let mut payload = manifest.clone();
    payload
        .as_object_mut()
        .ok_or_else(|| external_conflict("finalized evidence manifest root is invalid"))?
        .retain(|key, _| key != "contentHash" && !key.starts_with('_'));
    let bytes = serde_json::to_vec(&payload)
        .map_err(|_| external_conflict("finalized evidence manifest is not canonical"))?;
    Ok(format!("sha256:{}", ArtifactDigest::digest(bytes)))
}

#[derive(Clone, Copy)]
enum ReviewStateProjectionPath {
    Root,
    PrdState,
    DrState,
    DrStates,
    DrStateEntry,
    StoryStates,
    StoryStateEntry,
    Other,
}

impl ReviewStateProjectionPath {
    const fn is_lifecycle_projection(self) -> bool {
        matches!(
            self,
            Self::Root
                | Self::PrdState
                | Self::DrState
                | Self::DrStateEntry
                | Self::StoryStateEntry
        )
    }

    fn child(self, key: &str) -> Self {
        match (self, key) {
            (Self::Root, "prdState") => Self::PrdState,
            (Self::Root, "drState") => Self::DrState,
            (Self::Root, "drStates") => Self::DrStates,
            (Self::Root, "storyStates")
            | (Self::DrState, "storyStates")
            | (Self::DrStateEntry, "storyStates") => Self::StoryStates,
            (Self::DrStates, _) => Self::DrStateEntry,
            (Self::StoryStates, _) => Self::StoryStateEntry,
            _ => Self::Other,
        }
    }
}

fn strip_derived_review_fields(value: &mut Value) {
    strip_derived_review_fields_at(value, ReviewStateProjectionPath::Root);
}

fn strip_derived_review_fields_at(value: &mut Value, path: ReviewStateProjectionPath) {
    match value {
        Value::Object(object) => {
            // Lifecycle reducers advance these projections after Review gates
            // pass. They describe workflow position, not reviewed source or
            // design authority, so hashing them makes the permitted
            // test-running -> code-reviewed transition invalidate the Review
            // receipt required by the following completion gate. Only the
            // lifecycle authority's explicit projection paths are excluded;
            // similarly shaped semantic objects remain part of the input.
            if path.is_lifecycle_projection() {
                for field in [
                    "phase",
                    "currentPhase",
                    "currentStep",
                    "completedSteps",
                    "pausedFromPhase",
                    "pausedFrom",
                    "pauseReason",
                ] {
                    object.remove(field);
                }
            }
            for field in [
                "review",
                "reviewSession",
                "reviewLoop",
                "gateResults",
                "hookGuard",
                "nextActions",
                "inputFingerprint",
                "rulesetFingerprint",
                "policyDigest",
                "inventoryGeneration",
                "revision",
                "lastFencingToken",
                "lastMutation",
                // Evidence authority is validated independently by binding
                // the finalized ledger/manifest bytes to its refs and digests.
                // Including its projection here makes evidence.record and
                // evidence.finalize invalidate the evidence they just wrote.
                "evidenceAuthority",
                // Final verification writes these daemon-owned projections
                // after all reviewer contributions are bound. Their contents
                // are validated as terminal provenance, but must not make the
                // receipt invalidate the Review input it proves.
                "toolsetReceiptRef",
                "finalVerificationBinding",
                // The execution runtime section is daemon-derived control-plane
                // state, and `review.record` itself advances the completion
                // milestone inside it. Hashing it would let a review invalidate
                // the very Gate authority it just established, so a Work Item
                // could satisfy `G-12` or reach `GovernanceClosed` but never
                // both. Its `completionBound` digests stay authoritative through
                // `CompletionMilestone::invalidate`, not through this input.
                "executionRuntime",
            ] {
                object.remove(field);
            }
            for (key, child) in object.iter_mut() {
                strip_derived_review_fields_at(child, path.child(key));
            }
        }
        Value::Array(values) => {
            for child in values {
                strip_derived_review_fields_at(child, ReviewStateProjectionPath::Other);
            }
        }
        _ => {}
    }
}

fn review_ruleset_fingerprint(
    policy_digest: PolicyDigest,
    inventory_generation: u64,
) -> InputFingerprint {
    InputFingerprint::digest(
        format!(
            "ae-sdd-review-ruleset/v2\0{}\0{inventory_generation}",
            policy_digest
        )
        .as_bytes(),
    )
}

fn derived_review_id(
    work_item_id: &str,
    input: InputFingerprint,
    ruleset: InputFingerprint,
    revision: u64,
) -> RuntimeResult<ReviewId> {
    ReviewId::new(derived_identifier(
        "review",
        &[
            work_item_id.as_bytes(),
            input.as_bytes(),
            ruleset.as_bytes(),
            &revision.to_be_bytes(),
        ],
    ))
    .map_err(|_| schema_error("derived reviewId is invalid"))
}

fn derived_batch_id(session: &ReviewSessionV2) -> RuntimeResult<ReviewBatchId> {
    let ordinal = session
        .counters()
        .valid_batches()
        .checked_add(1)
        .ok_or_else(|| gate_blocked("review valid batch budget is exhausted"))?;
    ReviewBatchId::new(derived_identifier(
        "batch",
        &[
            session.review_id().as_str().as_bytes(),
            &ordinal.to_be_bytes(),
            &session.counters().attempts().to_be_bytes(),
        ],
    ))
    .map_err(|_| schema_error("derived batchId is invalid"))
}

fn derived_attempt_id(
    review_id: &ReviewId,
    batch_id: &ReviewBatchId,
    idempotency_key: &str,
    payload_digest: &[u8; 32],
) -> RuntimeResult<ReviewAttemptId> {
    ReviewAttemptId::new(derived_identifier(
        "attempt",
        &[
            review_id.as_str().as_bytes(),
            batch_id.as_str().as_bytes(),
            idempotency_key.as_bytes(),
            payload_digest,
        ],
    ))
    .map_err(|_| schema_error("derived attemptId is invalid"))
}

fn derived_identifier(prefix: &str, parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"ae-sdd-review-identifier/v2\0");
    hasher.update(prefix.as_bytes());
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    let digest = hex::encode(hasher.finalize());
    format!("{prefix}-{}", &digest[..24])
}

fn add_minutes(observed: &ReviewTimestamp, minutes: u32) -> RuntimeResult<ReviewTimestamp> {
    let timestamp = jiff::Timestamp::from_str(observed.as_str())
        .map_err(|_| schema_error("review start timestamp is invalid"))?
        .checked_add(jiff::SignedDuration::from_secs(i64::from(minutes) * 60))
        .map_err(|_| schema_error("review deadline overflow"))?;
    ReviewTimestamp::new(timestamp.to_string())
        .map_err(|_| schema_error("review deadline is invalid"))
}

fn review_timestamp_from_unix_ms(value: u64) -> RuntimeResult<ReviewTimestamp> {
    let value = i64::try_from(value)
        .map_err(|_| schema_error("verification timestamp exceeds its bound"))?;
    let timestamp = jiff::Timestamp::from_millisecond(value)
        .map_err(|_| schema_error("verification timestamp is invalid"))?;
    ReviewTimestamp::new(timestamp.to_string())
        .map_err(|_| schema_error("verification timestamp is not canonical UTC"))
}

fn specialty_name(value: ReviewerSpecialty) -> &'static str {
    match value {
        ReviewerSpecialty::General => "general",
        ReviewerSpecialty::Be => "be",
        ReviewerSpecialty::Ar => "ar",
        ReviewerSpecialty::Qa => "qa",
    }
}

fn required_string<'a>(value: Option<&'a Value>, message: &str) -> RuntimeResult<&'a str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| schema_error(message))
}

fn supervisor_error(error: ReviewSupervisorError) -> RuntimeError {
    RuntimeError::new(
        StableErrorCode::OperationSchemaInvalid,
        format!("review supervisor rejected the attempt: {error}"),
    )
}

fn schema_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::OperationSchemaInvalid, message)
}

fn role_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::RoleOperationForbidden, message)
}

fn identity_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::TurnIdentityMismatch, message)
}

fn gate_blocked(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::GateBlocked, message)
}

fn external_conflict(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::ExternalStateConflict, message)
}

fn attestation_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::DelegationAttestationFailed, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ae_sdd_runtime::GrantPathWire;
    use std::collections::BTreeMap;

    #[test]
    fn validate_child_authority_rejects_task_delegation_work_item_mismatch() {
        let workspace_id = "ws-test";
        let root_session_id = "root-123";
        let series_session_id = "series-456";
        let task_session_id = "task-789";
        let delegation_id = "delegation-abc";
        let boot_id = "boot-current";
        let now_ms = 1_700_000_000_000u64;
        let deadline_ms = now_ms + 3_600_000;

        let correct_work_item = "STORY-CORRECT";
        let wrong_work_item = "STORY-WRONG";

        // Parent and delegation both use STORY-WRONG to match each other.
        // Only child uses STORY-CORRECT, creating the single mismatch:
        // delegation.work_item_id != child.current_work_item
        let parent_session = RuntimeSessionRecord {
            session_id: series_session_id.to_owned(),
            agent_id: "series-agent".to_owned(),
            workspace_id: workspace_id.to_owned(),
            external_key_hash: "hash-series".to_owned(),
            role: WireAgentRole::Series,
            root_session_id: root_session_id.to_owned(),
            parent_session_id: Some(root_session_id.to_owned()),
            delegation_id: Some("delegation-series".to_owned()),
            engaged: true,
            current_work_item: Some(wrong_work_item.to_owned()),
            grant: ScopedGrantWire {
                operations: vec!["flow.next".to_owned()],
                capabilities: vec![],
                paths: vec![GrantPathWire::ProjectRoot],
            },
            context_generation: 0,
            expires_at_unix_ms: deadline_ms,
            status: "active".to_owned(),
            created_at_unix_ms: now_ms - 1000,
            updated_at_unix_ms: now_ms,
        };

        let child_session = RuntimeSessionRecord {
            session_id: task_session_id.to_owned(),
            agent_id: "task-agent".to_owned(),
            workspace_id: workspace_id.to_owned(),
            external_key_hash: "hash-task".to_owned(),
            role: WireAgentRole::Task,
            root_session_id: root_session_id.to_owned(),
            parent_session_id: Some(series_session_id.to_owned()),
            delegation_id: Some(delegation_id.to_owned()),
            engaged: true,
            current_work_item: Some(correct_work_item.to_owned()),
            grant: ScopedGrantWire {
                operations: vec!["workitem.read".to_owned()],
                capabilities: vec![],
                paths: vec![GrantPathWire::ProjectRoot],
            },
            context_generation: 0,
            expires_at_unix_ms: deadline_ms,
            status: "active".to_owned(),
            created_at_unix_ms: now_ms - 500,
            updated_at_unix_ms: now_ms,
        };

        let delegation = RuntimeDelegationRecord {
            delegation_id: delegation_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            work_item_id: Some(wrong_work_item.to_owned()),
            root_session_id: root_session_id.to_owned(),
            parent_session_id: series_session_id.to_owned(),
            child_session_id: Some(task_session_id.to_owned()),
            parent_delegation_id: Some("delegation-series".to_owned()),
            role: WireAgentRole::Task,
            input_revision: 7,
            input_fingerprint: "fingerprint".to_owned(),
            status: "running".to_owned(),
            deadline_unix_ms: deadline_ms,
            receipt_digest: "receipt".to_owned(),
            created_at_unix_ms: now_ms - 500,
            updated_at_unix_ms: now_ms,
        };

        let attestation = RuntimeDelegationAttestationRecord {
            workspace_id: workspace_id.to_owned(),
            delegation_id: delegation_id.to_owned(),
            physical_session_id: task_session_id.to_owned(),
            host_action_id: "action-001".to_owned(),
            host_ack_id: "ack-001".to_owned(),
            action_digest: "action".to_owned(),
            ack_digest: "ack".to_owned(),
            claim_digest: "claim".to_owned(),
            grant: child_session.grant.clone(),
            attestation_ref: "ref".to_owned(),
            attestation_digest: "digest".to_owned(),
            accepted_boot_id: boot_id.to_owned(),
            accepted_at_unix_ms: now_ms - 500,
            expires_at_unix_ms: deadline_ms,
        };

        let mut delegations = BTreeMap::new();
        delegations.insert(delegation_id.to_owned(), (delegation, attestation));

        let boots = AttestationBoots::live(boot_id);

        let result = validate_child_authority(
            &child_session,
            &parent_session,
            &delegations,
            boots,
            now_ms,
            ReviewAdmission::Contribute,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), StableErrorCode::DelegationAttestationFailed);
    }
}
