use ae_sdd_protocol::RequestParams;
use serde_json::{Value, json};

use super::LegacyArgumentError;

const JOB_STATUS_RPC_DEADLINE_MS: u64 = 1_000;

/// Trusted identity and absolute deadline retained after `job.submit` consumes
/// its request. Status polls never recover scope from caller-controlled job
/// arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyJobPollContext {
    protocol_version: String,
    workspace_id: String,
    agent_id: Option<String>,
    session_id: Option<String>,
    capability_token: Option<String>,
    work_item_id: Option<String>,
    deadline_unix_ms: u64,
}

impl LegacyJobPollContext {
    pub fn from_submission(params: &RequestParams<Value>) -> Result<Self, LegacyArgumentError> {
        let workspace_id = params
            .workspace_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| error("job polling requires trusted workspace identity"))?;
        let deadline_unix_ms = params
            .payload
            .get("deadlineUnixMs")
            .and_then(Value::as_u64)
            .ok_or_else(|| error("job submission is missing its absolute deadline"))?;
        Ok(Self {
            protocol_version: params.protocol_version.clone(),
            workspace_id: workspace_id.to_owned(),
            agent_id: params.agent_id.clone(),
            session_id: params.session_id.clone(),
            capability_token: params.capability_token.clone(),
            work_item_id: params.work_item_id.clone(),
            deadline_unix_ms,
        })
    }

    pub fn status_request(
        &self,
        job_id: &str,
        now_unix_ms: u64,
    ) -> Result<RequestParams<Value>, LegacyArgumentError> {
        if job_id.trim().is_empty() || job_id.len() > 128 {
            return Err(error("job.submit returned an invalid jobId"));
        }
        let remaining = self
            .deadline_unix_ms
            .checked_sub(now_unix_ms)
            .filter(|remaining| *remaining > 0)
            .ok_or_else(|| error("LEGACY_JOB_POLL_TIMEOUT: background job deadline elapsed"))?;
        Ok(RequestParams {
            protocol_version: self.protocol_version.clone(),
            workspace_id: Some(self.workspace_id.clone()),
            agent_id: self.agent_id.clone(),
            session_id: self.session_id.clone(),
            capability_token: self.capability_token.clone(),
            turn_id: None,
            work_item_id: self.work_item_id.clone(),
            lease_id: None,
            fencing_token: None,
            expected_revision: None,
            idempotency_key: None,
            confirmation: None,
            deadline_ms: remaining.min(JOB_STATUS_RPC_DEADLINE_MS),
            payload: json!({"jobId":job_id}),
        })
    }
}

/// Returns `false` while a job is pending and `true` only for a verified PASS.
/// Every other terminal state is a command failure even though `job.status`
/// itself was a valid JSON-RPC response.
pub fn validate_job_terminal_status(
    command_id: &str,
    status: &Value,
) -> Result<bool, LegacyArgumentError> {
    match status.get("status").and_then(Value::as_str) {
        Some("queued" | "running") => Ok(false),
        Some("pass")
            if status.pointer("/result/outcome").and_then(Value::as_str) == Some("PASS") =>
        {
            Ok(true)
        }
        Some("pass") => Err(error(format!(
            "LEGACY_JOB_INVALID_PASS: {command_id} completed without a PASS result"
        ))),
        Some(state @ ("fail" | "error" | "timeout" | "cancelled" | "stale")) => {
            let error_code = status
                .get("errorCode")
                .and_then(Value::as_str)
                .unwrap_or("none");
            Err(error(format!(
                "LEGACY_JOB_NON_PASS: {command_id} completed with status {state} (errorCode={error_code})"
            )))
        }
        _ => Err(error(format!(
            "LEGACY_JOB_STATUS_INVALID: {command_id} returned an unknown job status"
        ))),
    }
}

fn error(message: impl Into<String>) -> LegacyArgumentError {
    LegacyArgumentError::new(message)
}
