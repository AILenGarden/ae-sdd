use super::*;

#[derive(Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JobSubmitPayload {
    entrypoint: String,
    #[serde(default)]
    arguments: Value,
    deadline_unix_ms: u64,
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
        let key = require_idempotency(params)?;
        let digest = canonical_digest(&payload)?;
        let scope = format!("job-submit\0{workspace_id}");
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
            None,
            params.work_item_id.clone(),
        )
        .map(|(value, _)| value)
    }

    pub(super) fn job_status(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let workspace_id = require(&params.workspace_id, "workspaceId")?;
        let job_id = payload_string(&params.payload, "jobId")?;
        let state = self.lock_state()?;
        let job = state.jobs.get(job_id).ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::OperationSchemaInvalid,
                "job does not exist",
            )
        })?;
        if job.workspace.workspace_id != workspace_id {
            return Err(project_mismatch("job belongs to another workspace"));
        }
        to_value(job)
    }

    pub(super) fn job_cancel(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let workspace_id = require(&params.workspace_id, "workspaceId")?.to_owned();
        let job_id = payload_string(&params.payload, "jobId")?.to_owned();
        let key = require_idempotency(params)?;
        let digest = canonical_digest(&params.payload)?;
        let scope = format!("job-cancel\0{workspace_id}\0{job_id}");
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
            None,
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
            None,
            record.work_item_id.clone(),
        )?;
        let outcome = if self.clock.now_unix_ms() >= record.deadline_unix_ms {
            Err(StableErrorCode::GateTimeout)
        } else {
            let workspace = BusinessWorkspace {
                workspace_id: record.workspace.workspace_id.clone(),
                canonical_root: record.workspace.canonical_root.clone(),
                project_key: record.workspace.project_key.clone(),
                mode: record.workspace.mode,
                agent_role: None,
                inventory_generation: record.workspace.inventory_generation,
            };
            match self
                .business
                .execute_job(&workspace, &record.entrypoint, &record.arguments)
            {
                Ok(value) => Ok(value),
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
            None,
            record.work_item_id,
        )?;
        Ok(true)
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
