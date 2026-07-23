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
}

/// A small set of legacy diagnostics deliberately has a stricter identity
/// contract than the protocol-wide workspace-scoped `job.submit` method.
/// Keeping this table in the daemon makes the admission decision authoritative
/// and prevents a CLI manifest from being the only enforcement layer.
fn job_identity_requirements(entrypoint: &str) -> JobIdentityRequirements {
    match entrypoint {
        "gate.doc-storage" | "iteration-check" | "update-check" | "memory.clean"
        | "memory.clean-all" | "memory.common" | "memory.create" | "memory.read"
        | "memory.search" | "memory.summarize" | "memory.update" => JobIdentityRequirements {
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
        let payload: JobSubmitPayload = decode_value(params.payload.clone())?;
        if payload.entrypoint.is_empty()
            || payload.entrypoint.len() > 128
            || payload.deadline_unix_ms <= self.clock.now_unix_ms()
        {
            return Err(schema_error("job entrypoint or deadline is invalid"));
        }
        let identity = self.bind_job_identity(params, &payload.entrypoint)?;
        let session_id = identity.as_ref().map(|binding| binding.session_id.as_str());
        let key = require_idempotency(params)?;
        let work_item_id = params.work_item_id.as_deref();
        let digest = canonical_digest(&json!({
            "payload":&payload,
            "workItemId":work_item_id,
            "sessionId":session_id,
        }))?;
        let binding_digest = canonical_digest(&json!({
            "workspaceId":workspace_id.as_str(),
            "workItemId":work_item_id,
            "sessionId":session_id,
        }))?;
        let scope = format!("job-submit\0{binding_digest}");
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let record = {
            let mut state = self.lock_state()?;
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
            let job_id = Uuid::new_v4().to_string();
            let record = JobRecord {
                job_id: job_id.clone(),
                workspace: BusinessWorkspaceWire {
                    workspace_id: workspace.result.workspace_id.clone(),
                    canonical_root: workspace.result.canonical_root.clone(),
                    project_key: workspace.result.project_key.clone(),
                    mode: workspace.result.mode,
                    inventory_generation: workspace.result.inventory_generation,
                },
                work_item_id: params.work_item_id.clone(),
                session_id: identity.as_ref().map(|binding| binding.session_id.clone()),
                agent_role: identity.as_ref().map(|binding| binding.role),
                agent_grant: identity.as_ref().map(|binding| binding.grant.clone()),
                root_session_id: identity
                    .as_ref()
                    .map(|binding| binding.root_session_id.clone()),
                delegation_id: identity
                    .as_ref()
                    .and_then(|binding| binding.delegation_id.clone()),
                context_generation: identity.as_ref().map(|binding| binding.context_generation),
                submission_idempotency_key: Some(key.to_owned()),
                entrypoint: payload.entrypoint,
                arguments: payload.arguments,
                deadline_unix_ms: payload.deadline_unix_ms,
                status: JobStatus::Queued,
                result: None,
                error_code: None,
            };
            state.job_queue.push_back(job_id.clone());
            state.jobs.insert(job_id, record.clone());
            record
        };
        let value = to_value(&record)?;
        self.persistence
            .store_record("job/v1", &record.job_id, &value)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "job.submitted",
            Some(workspace_id),
            identity.map(|binding| binding.session_id),
            params.work_item_id.clone(),
        )
        .map(|(value, _)| value)
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
        let value = {
            let mut state = self.lock_state()?;
            let job = state.jobs.get_mut(&job_id).ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::OperationSchemaInvalid,
                    "job does not exist",
                )
            })?;
            if job.workspace.workspace_id != workspace_id {
                return Err(project_mismatch("job belongs to another workspace"));
            }
            assert_job_work_item(job, params.work_item_id.as_deref())?;
            if job.session_id.as_deref() != params.session_id.as_deref() {
                return Err(turn_mismatch("job belongs to another session"));
            }
            if job.status != JobStatus::Queued {
                return Err(RuntimeError::new(
                    StableErrorCode::JobNotCancellable,
                    "job has crossed the cancellable queue point",
                ));
            }
            job.status = JobStatus::Cancelled;
            to_value(job)?
        };
        self.persistence.store_record("job/v1", &job_id, &value)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "job.cancelled",
            Some(workspace_id),
            job.session_id,
            None,
        )
        .map(|(value, _)| value)
    }

    /// Executes at most one queued job in the daemon's bounded blocking executor.
    pub fn run_one_pending_job(&self) -> RuntimeResult<bool> {
        let record = {
            let mut state = self.lock_state()?;
            let Some(job_id) = state.job_queue.pop_front() else {
                return Ok(false);
            };
            let job = state.jobs.get_mut(&job_id).ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "queued job is missing",
                )
            })?;
            if job.status != JobStatus::Queued {
                return Ok(true);
            }
            job.status = JobStatus::Running;
            job.clone()
        };
        self.persistence
            .store_record("job/v1", &record.job_id, &to_value(&record)?)?;
        self.append_runtime_event(
            "job.started",
            json!({"jobId":record.job_id,"entrypoint":record.entrypoint}),
            Some(record.workspace.workspace_id.clone()),
            record.session_id.clone(),
            record.work_item_id.clone(),
        )?;
        let outcome = if self.clock.now_unix_ms() >= record.deadline_unix_ms {
            Err(StableErrorCode::GateTimeout)
        } else {
            match job_business_identity(&record) {
                Ok((agent_role, agent_grant)) => {
                    let workspace = BusinessWorkspace {
                        workspace_id: record.workspace.workspace_id.clone(),
                        canonical_root: record.workspace.canonical_root.clone(),
                        project_key: record.workspace.project_key.clone(),
                        mode: record.workspace.mode,
                        agent_role,
                        agent_grant,
                        caller_kind: None,
                        inventory_generation: record.workspace.inventory_generation,
                    };
                    let bound_identity = job_bound_identity(&record, self.boot_id)?;
                    match self.business.execute_trusted_job(
                        &workspace,
                        record.work_item_id.as_deref(),
                        bound_identity.as_ref(),
                        &record.entrypoint,
                        &record.arguments,
                    ) {
                        Ok(value) => Ok(value),
                        Err(error) => Err(error.code()),
                    }
                }
                Err(error) => Err(error.code()),
            }
        };
        let value = {
            let mut state = self.lock_state()?;
            let job = state.jobs.get_mut(&record.job_id).ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "running job is missing",
                )
            })?;
            match outcome {
                Ok(value) => {
                    job.status = job_status_from_result(&value);
                    job.result = Some(value);
                }
                Err(code) => {
                    job.status = if code == StableErrorCode::GateTimeout {
                        JobStatus::Timeout
                    } else {
                        JobStatus::Error
                    };
                    job.error_code = Some(code);
                }
            }
            to_value(job)?
        };
        self.persistence
            .store_record("job/v1", &record.job_id, &value)?;
        self.append_runtime_event(
            "job.completed",
            json!({
                "jobId":record.job_id,
                "status":value.get("status").cloned().unwrap_or(Value::Null),
            }),
            Some(record.workspace.workspace_id),
            record.session_id,
            record.work_item_id,
        )?;
        Ok(true)
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
        Ok(Some(JobIdentityBinding {
            session_id: identity.session_id,
            root_session_id,
            delegation_id,
            context_generation,
            role: identity.role,
            grant: ScopedGrantWire::from_domain(&identity.grant),
        }))
    }

    fn assert_job_access(
        &self,
        job: &JobRecord,
        params: &RequestParams<Value>,
    ) -> RuntimeResult<()> {
        let workspace_id = require(&params.workspace_id, "workspaceId")?;
        if job.workspace.workspace_id != workspace_id {
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
    job: &JobRecord,
) -> RuntimeResult<(Option<AgentRole>, Option<ScopedGrant>)> {
    match (&job.session_id, job.agent_role, &job.agent_grant) {
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

fn job_bound_identity(job: &JobRecord, boot_id: BootId) -> RuntimeResult<Option<BoundJobIdentity>> {
    match (
        &job.session_id,
        &job.root_session_id,
        job.context_generation,
        &job.submission_idempotency_key,
    ) {
        (None, None, None, Some(_)) | (None, None, None, None) => Ok(None),
        (Some(session_id), Some(root_session_id), Some(context_generation), Some(key)) => {
            Ok(Some(BoundJobIdentity {
                boot_id: boot_id.to_string(),
                session_id: session_id.clone(),
                root_session_id: root_session_id.clone(),
                delegation_id: job.delegation_id.clone(),
                context_generation,
                idempotency_key: key.clone(),
            }))
        }
        _ => Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "durable job lineage binding is incomplete",
        )),
    }
}

fn assert_job_work_item(job: &JobRecord, requested: Option<&str>) -> RuntimeResult<()> {
    if job.work_item_id.as_deref() == requested {
        Ok(())
    } else {
        Err(project_mismatch("job belongs to another work item"))
    }
}

fn job_status_from_result(value: &Value) -> JobStatus {
    match value.get("outcome").and_then(Value::as_str) {
        Some("PASS") => JobStatus::Pass,
        Some("FAIL") => JobStatus::Fail,
        Some("STALE") => JobStatus::Stale,
        Some("TIMEOUT") => JobStatus::Timeout,
        Some("CANCELLED") => JobStatus::Cancelled,
        _ => JobStatus::Error,
    }
}
