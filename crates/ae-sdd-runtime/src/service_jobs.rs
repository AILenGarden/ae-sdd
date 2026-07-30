use super::*;
use crate::ports::BoundJobIdentity;

#[derive(Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JobSubmitPayload {
    entrypoint: String,
    #[serde(default)]
    arguments: Value,
    deadline_unix_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct JobIdentityRequirements {
    work_item: bool,
    session: bool,
}

#[derive(Clone, Debug)]
struct JobIdentityBinding {
    session_id: String,
    root_session_id: String,
    delegation_id: Option<String>,
    context_generation: u64,
    role: WireAgentRole,
    grant: ScopedGrantWire,
    attestation_ref: String,
    attestation_digest: String,
    identity_digest: String,
}

/// A small set of legacy diagnostics deliberately has a stricter identity
/// contract than the protocol-wide workspace-scoped `job.submit` method.
/// Keeping this table in the daemon makes the admission decision authoritative
/// and prevents a CLI manifest from being the only enforcement layer.
fn job_identity_requirements(entrypoint: &str) -> JobIdentityRequirements {
    match entrypoint {
        "gate.doc-storage"
        | "iteration-check"
        | "update-check"
        | "memory.clean"
        | "memory.clean-all"
        | "memory.common"
        | "memory.create"
        | "memory.read"
        | "memory.search"
        | "memory.summarize"
        | "memory.update"
        | "toolset.required"
        | "toolset.receipt.record" => JobIdentityRequirements {
            work_item: true,
            session: true,
        },
        _ => JobIdentityRequirements {
            work_item: false,
            session: false,
        },
    }
}

impl RuntimeService {
    pub(super) fn job_submit(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let workspace_id = require(&params.workspace_id, "workspaceId")?.to_owned();
        let mut payload: JobSubmitPayload = decode_value(params.payload.clone())?;
        if payload.entrypoint.is_empty()
            || payload.entrypoint.len() > 128
            || payload.deadline_unix_ms <= self.clock.now_unix_ms()
            || !payload.arguments.is_object()
        {
            return Err(schema_error("job entrypoint or deadline is invalid"));
        }
        bind_finalized_receipt_lease(&payload.entrypoint, params, &mut payload.arguments)?;
        let identity = self.bind_job_identity(params, &payload.entrypoint)?;
        let session_id = identity.as_ref().map(|binding| binding.session_id.as_str());
        let key = require_idempotency(params)?;
        let work_item_id = params.work_item_id.as_deref();
        let (source_revision, input_fingerprint) =
            job_source_binding(&payload.entrypoint, params, &payload.arguments)?;
        let request_digest = canonical_digest(&json!({
            "workspaceId":workspace_id.as_str(),
            "payload":&payload,
            "workItemId":work_item_id,
            "sessionId":session_id,
            "sourceRevision":source_revision,
            "inputFingerprint":input_fingerprint,
            "identityDigest":identity.as_ref().map(|binding| binding.identity_digest.as_str()),
        }))?;
        let scope_digest = canonical_digest(&json!({
            "domain":"runtime-job-submit/v1",
            "workspaceId":workspace_id.as_str(),
            "workItemId":work_item_id,
            "sessionId":session_id,
        }))?;
        let workspace = {
            let state = self.lock_state()?;
            if let Some(existing) = state.jobs.values().find(|job| {
                job.submission_scope_digest == scope_digest && job.submission_idempotency_key == key
            }) {
                if existing.request_digest != request_digest {
                    return Err(RuntimeError::new(
                        StableErrorCode::IdempotencyKeyReused,
                        "job submission key was reused with a different trusted request",
                    ));
                }
                return to_value(existing);
            }
            if state.jobs.len() >= self.config.max_jobs
                || state.job_queue.len() >= self.config.max_jobs
            {
                return Err(RuntimeError::new(
                    StableErrorCode::SubscriberBackpressure,
                    "background job queue capacity is exhausted",
                ));
            }
            let workspace = state
                .workspaces
                .get(&workspace_id)
                .ok_or_else(|| project_mismatch("workspace is not registered"))?;
            workspace.result.clone()
        };
        let now = self.clock.now_unix_ms();
        let key_digest = hex::encode(Sha256::digest(key.as_bytes()));
        let record = RuntimeJobRecord {
            job_id: Uuid::new_v4().to_string(),
            workspace_id: workspace.workspace_id,
            work_item_id: params.work_item_id.clone(),
            session_id: identity.as_ref().map(|binding| binding.session_id.clone()),
            root_session_id: identity
                .as_ref()
                .map(|binding| binding.root_session_id.clone()),
            delegation_id: identity
                .as_ref()
                .and_then(|binding| binding.delegation_id.clone()),
            agent_role: identity.as_ref().map(|binding| binding.role),
            context_generation: identity.as_ref().map(|binding| binding.context_generation),
            submission_boot_id: identity.as_ref().map(|_| self.boot_id.to_string()),
            attestation_ref: identity
                .as_ref()
                .map(|binding| binding.attestation_ref.clone()),
            attestation_digest: identity
                .as_ref()
                .map(|binding| binding.attestation_digest.clone()),
            grant: identity.as_ref().map(|binding| binding.grant.clone()),
            identity_digest: identity
                .as_ref()
                .map(|binding| binding.identity_digest.clone()),
            workspace_mode: workspace.mode,
            inventory_generation: workspace.inventory_generation,
            entrypoint: payload.entrypoint,
            arguments: payload.arguments,
            submission_scope_digest: scope_digest,
            submission_idempotency_key: key.to_owned(),
            submission_idempotency_key_digest: key_digest,
            request_digest,
            source_revision,
            input_fingerprint,
            deadline_unix_ms: payload.deadline_unix_ms,
            status: RuntimeJobStatus::Queued,
            row_version: 0,
            result: None,
            error_code: None,
            mutation_id: None,
            receipt_locator: None,
            project_receipt_digest: None,
            submitted_event_seq: 0,
            last_event_seq: 0,
            created_at_unix_ms: now,
            started_at_unix_ms: None,
            finished_at_unix_ms: None,
            updated_at_unix_ms: now,
        };
        let event = self.job_event("job.submitted", &record, json!({}))?;
        let record = self
            .persistence
            .commit_job_transition(RuntimeJobTransition {
                record,
                expected_status: None,
                expected_row_version: None,
                event,
            })?;
        {
            let mut state = self.lock_state()?;
            if !state.jobs.contains_key(&record.job_id) {
                state.job_queue.push_back(record.job_id.clone());
            }
            state.jobs.insert(record.job_id.clone(), record.clone());
        }
        to_value(&record)
    }

    pub(super) fn job_status(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let job_id = payload_string(&params.payload, "jobId")?;
        let job = {
            let state = self.lock_state()?;
            state.jobs.get(job_id).cloned().ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::OperationSchemaInvalid,
                    "job does not exist",
                )
            })?
        };
        self.assert_job_access(&job, params)?;
        to_value(&job)
    }

    pub(super) fn job_cancel(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let workspace_id = require(&params.workspace_id, "workspaceId")?.to_owned();
        let job_id = payload_string(&params.payload, "jobId")?.to_owned();
        let key = require_idempotency(params)?;
        let requested_work_item = params.work_item_id.as_deref();
        let job = {
            let state = self.lock_state()?;
            state.jobs.get(&job_id).cloned().ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::OperationSchemaInvalid,
                    "job does not exist",
                )
            })?
        };
        self.assert_job_access(&job, params)?;
        let digest = canonical_digest(&json!({
            "payload":&params.payload,
            "workItemId":requested_work_item,
            "sessionId":job.session_id,
        }))?;
        let binding_digest = canonical_digest(&json!({
            "workspaceId":workspace_id.as_str(),
            "workItemId":requested_work_item,
            "sessionId":job.session_id,
        }))?;
        let scope = format!("job-cancel\0{binding_digest}\0{job_id}");
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let mut next = {
            let state = self.lock_state()?;
            let job = state.jobs.get(&job_id).ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::OperationSchemaInvalid,
                    "job does not exist",
                )
            })?;
            if job.workspace_id != workspace_id {
                return Err(project_mismatch("job belongs to another workspace"));
            }
            assert_job_work_item(job, params.work_item_id.as_deref())?;
            if job.session_id.as_deref() != params.session_id.as_deref() {
                return Err(turn_mismatch("job belongs to another session"));
            }
            if job.status != RuntimeJobStatus::Queued {
                return Err(RuntimeError::new(
                    StableErrorCode::JobNotCancellable,
                    "job has crossed the cancellable queue point",
                ));
            }
            job.clone()
        };
        let expected_row_version = next.row_version;
        let now = self.clock.now_unix_ms();
        next.status = RuntimeJobStatus::Cancelled;
        next.row_version = next.row_version.saturating_add(1);
        next.error_code = Some("CANCELLED".to_owned());
        next.finished_at_unix_ms = Some(now);
        next.updated_at_unix_ms = now;
        let event = self.job_event("job.cancelled", &next, json!({}))?;
        let next = self
            .persistence
            .commit_job_transition(RuntimeJobTransition {
                record: next,
                expected_status: Some(RuntimeJobStatus::Queued),
                expected_row_version: Some(expected_row_version),
                event,
            })?;
        let value = to_value(&next)?;
        self.persistence.store_receipt(&IdempotencyReceipt {
            scope,
            key: key.to_owned(),
            request_digest: digest,
            response_json: serde_json::to_string(&value).map_err(canonical_error)?,
            event_seq: next.last_event_seq,
        })?;
        self.lock_state()?.jobs.insert(next.job_id.clone(), next);
        Ok(value)
    }

    /// Executes at most one queued job in the daemon's bounded blocking executor.
    pub fn run_one_pending_job(&self) -> RuntimeResult<bool> {
        let queued = {
            let mut state = self.lock_state()?;
            let Some(job_id) = state.job_queue.pop_front() else {
                return Ok(false);
            };
            let job = state.jobs.get(&job_id).ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "queued job is missing",
                )
            })?;
            if job.status != RuntimeJobStatus::Queued {
                return Ok(true);
            }
            job.clone()
        };
        let expected_row_version = queued.row_version;
        let mut running = queued;
        let started_at = self.clock.now_unix_ms();
        running.status = RuntimeJobStatus::Running;
        running.row_version = running.row_version.saturating_add(1);
        running.started_at_unix_ms = Some(started_at);
        running.updated_at_unix_ms = started_at;
        let event = self.job_event("job.started", &running, json!({}))?;
        let running = self
            .persistence
            .commit_job_transition(RuntimeJobTransition {
                record: running,
                expected_status: Some(RuntimeJobStatus::Queued),
                expected_row_version: Some(expected_row_version),
                event,
            })?;
        self.lock_state()?
            .jobs
            .insert(running.job_id.clone(), running.clone());

        let outcome = if self.clock.now_unix_ms() >= running.deadline_unix_ms {
            Err(StableErrorCode::GateTimeout)
        } else {
            match job_business_identity(&running) {
                Ok((agent_role, agent_grant)) => {
                    let workspace_record = {
                        let state = self.lock_state()?;
                        state
                            .workspaces
                            .get(&running.workspace_id)
                            .cloned()
                            .ok_or_else(|| project_mismatch("job workspace is not registered"))?
                    };
                    let workspace = BusinessWorkspace {
                        workspace_id: running.workspace_id.clone(),
                        canonical_root: workspace_record.result.canonical_root,
                        project_key: workspace_record.result.project_key,
                        mode: running.workspace_mode,
                        agent_role,
                        agent_grant,
                        caller_kind: None,
                        inventory_generation: running.inventory_generation,
                    };
                    let bound_identity = job_bound_identity(&running)?;
                    match self.business.execute_trusted_job(
                        &workspace,
                        running.work_item_id.as_deref(),
                        bound_identity.as_ref(),
                        &running.entrypoint,
                        &running.arguments,
                    ) {
                        Ok(value) => Ok(value),
                        Err(error) => Err(error.code()),
                    }
                }
                Err(error) => Err(error.code()),
            }
        };
        let expected_row_version = running.row_version;
        let mut terminal = running;
        if let Err(error) = apply_job_outcome(&mut terminal, outcome) {
            terminal.status = RuntimeJobStatus::Error;
            terminal.result = None;
            terminal.error_code = Some(error.code().as_str().to_owned());
            terminal.mutation_id = None;
            terminal.receipt_locator = None;
            terminal.project_receipt_digest = None;
        }
        let finished_at = self.clock.now_unix_ms();
        terminal.row_version = terminal.row_version.saturating_add(1);
        terminal.finished_at_unix_ms = Some(finished_at);
        terminal.updated_at_unix_ms = finished_at;
        let event = self.job_event("job.completed", &terminal, json!({}))?;
        let terminal = self
            .persistence
            .commit_job_transition(RuntimeJobTransition {
                record: terminal,
                expected_status: Some(RuntimeJobStatus::Running),
                expected_row_version: Some(expected_row_version),
                event,
            })?;
        self.lock_state()?
            .jobs
            .insert(terminal.job_id.clone(), terminal);
        Ok(true)
    }

    pub(super) fn job_event(
        &self,
        kind: &str,
        record: &RuntimeJobRecord,
        detail: Value,
    ) -> RuntimeResult<DurableEvent> {
        let payload = json!({
            "jobId":record.job_id,
            "entrypoint":record.entrypoint,
            "status":record.status,
            "detail":detail,
        });
        Ok(DurableEvent {
            event_store_id: self.persistence.event_store_id()?.to_string(),
            event_seq: 0,
            boot_id: self.boot_id.to_string(),
            kind: kind.to_owned(),
            workspace_id: Some(record.workspace_id.clone()),
            session_id: record.session_id.clone(),
            work_item_id: record.work_item_id.clone(),
            payload_digest: canonical_digest(&payload)?,
            payload,
        })
    }
}

impl RuntimeService {
    fn bind_job_identity(
        &self,
        params: &RequestParams<Value>,
        entrypoint: &str,
    ) -> RuntimeResult<Option<JobIdentityBinding>> {
        let requirements = job_identity_requirements(entrypoint);
        if requirements.work_item {
            require(&params.work_item_id, "workItemId")?;
        }
        if !requirements.session {
            return Ok(None);
        }
        let identity = self.session_identity(params, false)?;
        let work_item_id = require(&params.work_item_id, "workItemId")?;
        let (delegation_id, context_generation) = {
            let state = self.lock_state()?;
            let session = state
                .sessions
                .get(&identity.session_id)
                .ok_or_else(session_expired)?;
            if session.current_work_item.as_deref() != Some(work_item_id) {
                return Err(turn_mismatch(
                    "session is not bound to the supplied work item",
                ));
            }
            (
                session.delegation_id.clone(),
                session.result.context_generation,
            )
        };
        let root_session_id = match identity.role {
            WireAgentRole::Root => identity.session_id.clone(),
            WireAgentRole::Series | WireAgentRole::Task | WireAgentRole::Reviewer => self
                .delegation
                .root_session_id(delegation_id.as_deref().ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::ExternalStateConflict,
                        "child session lacks its durable delegation binding",
                    )
                })?)?,
        };
        let grant = ScopedGrantWire::from_domain(&identity.grant);
        let (attestation_ref, attestation_digest) = if identity.role == WireAgentRole::Root {
            let reference = format!("root-capability:{}", identity.capability_id);
            let digest = canonical_digest(&json!({
                "domain":"root-capability-attestation/v1",
                "bootId":self.boot_id.to_string(),
                "workspaceId":identity.workspace_id,
                "sessionId":identity.session_id,
                "capabilityId":identity.capability_id,
            }))?;
            (reference, digest)
        } else {
            let delegation_id = delegation_id.as_deref().ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::DelegationAttestationFailed,
                    "child job lacks a durable delegation",
                )
            })?;
            let attestation = self
                .persistence
                .list_identity_snapshots(RuntimeIdentityKind::Delegation)?
                .into_iter()
                .filter_map(|snapshot| snapshot.attestation)
                .find(|attestation| {
                    attestation.delegation_id == delegation_id
                        && attestation.physical_session_id == identity.session_id
                })
                .ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::DelegationAttestationFailed,
                        "child job lacks a durable physical attestation",
                    )
                })?;
            if attestation.accepted_boot_id != self.boot_id.to_string()
                || attestation.expires_at_unix_ms <= self.clock.now_unix_ms()
                || attestation.grant != grant
            {
                return Err(RuntimeError::new(
                    StableErrorCode::DelegationAttestationFailed,
                    "child job physical attestation is stale or mismatched",
                ));
            }
            (attestation.attestation_ref, attestation.attestation_digest)
        };
        let identity_digest = canonical_digest(&json!({
            "bootId":self.boot_id.to_string(),
            "sessionId":identity.session_id,
            "rootSessionId":root_session_id,
            "delegationId":delegation_id,
            "contextGeneration":context_generation,
            "role":identity.role,
            "grant":grant,
            "attestationRef":attestation_ref,
            "attestationDigest":attestation_digest,
        }))?;
        Ok(Some(JobIdentityBinding {
            session_id: identity.session_id,
            root_session_id,
            delegation_id,
            context_generation,
            role: identity.role,
            grant,
            attestation_ref,
            attestation_digest,
            identity_digest,
        }))
    }

    fn assert_job_access(
        &self,
        job: &RuntimeJobRecord,
        params: &RequestParams<Value>,
    ) -> RuntimeResult<()> {
        let workspace_id = require(&params.workspace_id, "workspaceId")?;
        if job.workspace_id != workspace_id {
            return Err(project_mismatch("job belongs to another workspace"));
        }
        assert_job_work_item(job, params.work_item_id.as_deref())?;
        if let Some(bound_session_id) = job.session_id.as_deref() {
            let identity = self.session_identity(params, false)?;
            if identity.session_id != bound_session_id {
                return Err(turn_mismatch("job belongs to another session"));
            }
        }
        Ok(())
    }
}

fn job_business_identity(
    job: &RuntimeJobRecord,
) -> RuntimeResult<(Option<AgentRole>, Option<ScopedGrant>)> {
    match (&job.session_id, job.agent_role, &job.grant) {
        (None, None, None) => Ok((None, None)),
        (Some(_), Some(role), Some(grant)) => Ok((
            Some(AgentRole::from(role)),
            Some(crate::grant::validate_session_grant(role, grant)?),
        )),
        _ => Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "durable job identity binding is incomplete",
        )),
    }
}

fn job_bound_identity(job: &RuntimeJobRecord) -> RuntimeResult<Option<BoundJobIdentity>> {
    match (
        &job.session_id,
        &job.root_session_id,
        job.context_generation,
        &job.submission_boot_id,
        &job.attestation_ref,
        &job.attestation_digest,
        &job.identity_digest,
    ) {
        (None, None, None, None, None, None, None) => Ok(None),
        (
            Some(session_id),
            Some(root_session_id),
            Some(context_generation),
            Some(boot_id),
            Some(attestation_ref),
            Some(attestation_digest),
            Some(identity_digest),
        ) => Ok(Some(BoundJobIdentity {
            job_id: job.job_id.clone(),
            boot_id: boot_id.clone(),
            session_id: session_id.clone(),
            root_session_id: root_session_id.clone(),
            delegation_id: job.delegation_id.clone(),
            context_generation,
            attestation_ref: attestation_ref.clone(),
            attestation_digest: attestation_digest.clone(),
            identity_digest: identity_digest.clone(),
            idempotency_key: job.submission_idempotency_key.clone(),
        })),
        _ => Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "durable job lineage binding is incomplete",
        )),
    }
}

fn assert_job_work_item(job: &RuntimeJobRecord, requested: Option<&str>) -> RuntimeResult<()> {
    if job.work_item_id.as_deref() == requested {
        Ok(())
    } else {
        Err(project_mismatch("job belongs to another work item"))
    }
}

fn bind_finalized_receipt_lease(
    entrypoint: &str,
    params: &RequestParams<Value>,
    arguments: &mut Value,
) -> RuntimeResult<()> {
    if entrypoint != "toolset.receipt.record" || arguments.get("finalizedEvidence").is_none() {
        return Ok(());
    }
    let lease_id = params
        .lease_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| schema_error("finalized receipt job requires leaseId"))?;
    let fencing_token = params
        .fencing_token
        .filter(|value| *value > 0)
        .ok_or_else(|| schema_error("finalized receipt job requires a positive fencingToken"))?;
    let object = arguments
        .as_object_mut()
        .ok_or_else(|| schema_error("finalized receipt arguments must be an object"))?;
    object.insert("leaseId".to_owned(), Value::String(lease_id.to_owned()));
    object.insert("fencingToken".to_owned(), json!(fencing_token));
    Ok(())
}

fn job_source_binding(
    entrypoint: &str,
    params: &RequestParams<Value>,
    arguments: &Value,
) -> RuntimeResult<(Option<u64>, Option<String>)> {
    if !matches!(entrypoint, "toolset.required" | "toolset.receipt.record") {
        return Ok((None, None));
    }
    let expected_revision = params
        .expected_revision
        .ok_or_else(|| schema_error("toolset job requires expectedRevision"))?;
    let receipt_binding = arguments.get("finalizedEvidence").unwrap_or(arguments);
    let source_revision = if entrypoint == "toolset.receipt.record" {
        let declared = receipt_binding
            .get("sourceRevision")
            .and_then(Value::as_u64)
            .ok_or_else(|| schema_error("toolset receipt sourceRevision is required"))?;
        if arguments.get("finalizedEvidence").is_none() && declared != expected_revision {
            return Err(schema_error(
                "toolset receipt sourceRevision does not match expectedRevision",
            ));
        }
        declared
    } else {
        expected_revision
    };
    let input_fingerprint = if entrypoint == "toolset.required" {
        arguments.get("inputFingerprint")
    } else {
        receipt_binding.get("inputFingerprint").or_else(|| {
            arguments
                .get("plan")
                .and_then(|plan| plan.get("inputFingerprint"))
        })
    }
    .and_then(Value::as_str)
    .filter(|value| is_lower_hex_digest(value))
    .ok_or_else(|| schema_error("toolset job requires a canonical inputFingerprint"))?;
    Ok((Some(source_revision), Some(input_fingerprint.to_owned())))
}

fn apply_job_outcome(
    record: &mut RuntimeJobRecord,
    outcome: Result<Value, StableErrorCode>,
) -> RuntimeResult<()> {
    record.result = None;
    record.error_code = None;
    record.mutation_id = None;
    record.receipt_locator = None;
    record.project_receipt_digest = None;
    match outcome {
        Err(code) => {
            record.status = if code == StableErrorCode::GateTimeout {
                RuntimeJobStatus::Timeout
            } else {
                RuntimeJobStatus::Error
            };
            record.error_code = Some(code.as_str().to_owned());
        }
        Ok(value) => match value.get("outcome").and_then(Value::as_str) {
            Some("PASS") => {
                record.status = RuntimeJobStatus::Pass;
                if record.entrypoint == "toolset.receipt.record" {
                    apply_project_receipt(record, &value)?;
                }
                record.result = Some(value);
            }
            Some("FAIL") => {
                record.status = RuntimeJobStatus::Fail;
                record.result = Some(value);
            }
            Some("STALE") => {
                record.status = RuntimeJobStatus::Stale;
                record.result = Some(value);
            }
            Some("TIMEOUT") => {
                record.status = RuntimeJobStatus::Timeout;
                record.error_code = Some(
                    value
                        .get("errorCode")
                        .and_then(Value::as_str)
                        .unwrap_or(StableErrorCode::GateTimeout.as_str())
                        .to_owned(),
                );
            }
            Some("CANCELLED") => {
                record.status = RuntimeJobStatus::Cancelled;
                record.error_code = Some(
                    value
                        .get("errorCode")
                        .and_then(Value::as_str)
                        .unwrap_or("CANCELLED")
                        .to_owned(),
                );
            }
            _ => {
                record.status = RuntimeJobStatus::Error;
                record.error_code =
                    Some(StableErrorCode::OperationSchemaInvalid.as_str().to_owned());
            }
        },
    }
    Ok(())
}

fn apply_project_receipt(record: &mut RuntimeJobRecord, value: &Value) -> RuntimeResult<()> {
    let mutation_id = bounded_result_string(value, "mutationId", 128)?;
    let receipt_locator = bounded_result_string(value, "receiptLocator", 1_024)?;
    let project_receipt_digest = bounded_result_string(value, "projectReceiptDigest", 64)?;
    if !is_lower_hex_digest(project_receipt_digest) {
        return Err(schema_error(
            "projectReceiptDigest must be canonical sha256 hex",
        ));
    }
    let source_revision = value
        .get("sourceRevision")
        .or_else(|| value.get("revisionAfter"))
        .and_then(Value::as_u64)
        .ok_or_else(|| schema_error("toolset PASS result lacks committed sourceRevision"))?;
    record.source_revision = Some(source_revision);
    record.mutation_id = Some(mutation_id.to_owned());
    record.receipt_locator = Some(receipt_locator.to_owned());
    record.project_receipt_digest = Some(project_receipt_digest.to_owned());
    Ok(())
}

fn bounded_result_string<'a>(value: &'a Value, field: &str, max: usize) -> RuntimeResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= max)
        .ok_or_else(|| schema_error(&format!("toolset PASS result lacks bounded {field}")))
}

fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finalized_receipt_job_binds_the_authenticated_outer_lease() {
        let mut params = RequestParams {
            protocol_version: PROTOCOL_VERSION_V1.to_owned(),
            workspace_id: None,
            agent_id: None,
            session_id: None,
            capability_token: None,
            turn_id: None,
            work_item_id: None,
            lease_id: Some("trusted-lease".to_owned()),
            fencing_token: Some(17),
            expected_revision: None,
            idempotency_key: None,
            confirmation: None,
            deadline_ms: 1_000,
            payload: Value::Null,
        };
        let mut arguments = json!({
            "finalizedEvidence": {"sourceRevision": 1},
            "leaseId": "forged-lease",
            "fencingToken": 99,
        });

        bind_finalized_receipt_lease("toolset.receipt.record", &params, &mut arguments)
            .expect("outer lease binds");

        assert_eq!(arguments["leaseId"], "trusted-lease");
        assert_eq!(arguments["fencingToken"], 17);

        params.lease_id = None;
        assert!(
            bind_finalized_receipt_lease("toolset.receipt.record", &params, &mut arguments)
                .is_err(),
            "the daemon-owned final receipt route requires an outer lease"
        );
    }
}
