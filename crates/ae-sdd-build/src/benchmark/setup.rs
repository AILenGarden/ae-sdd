use std::fs;
use std::path::{Path, PathBuf};

use ae_sdd_client::DaemonClient;
use ae_sdd_protocol::{ConfirmationRef, RequestParams, RpcMethod};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{
    BenchmarkError, CANARY_INVENTORY_GENERATION, HOOK_DEADLINE_MS, WORK_ITEM_ID, block_on,
    now_unix_ms,
};

pub(super) fn prepare_cached_hook(
    hook_client: &DaemonClient,
    admin_client: &DaemonClient,
    workspace_root: &Path,
    expected_input_fingerprint: &str,
    approved_at: String,
) -> Result<HookSession, BenchmarkError> {
    let confirmation = ConfirmationRef {
        confirmation_id: "benchmark-local-cutover".to_owned(),
        approved_by: "benchmark-invoker".to_owned(),
        approved_at,
    };
    let workspace: Value = call_sync(
        hook_client,
        RpcMethod::WorkspaceRegister,
        request_params(
            json!({
                "projectRoot": workspace_root.to_string_lossy(),
                "projectKey": "ae-sdd-hook-benchmark"
            }),
            None,
            None,
            None,
            Some("benchmark-workspace-register"),
        ),
    )?;
    let workspace_id = required_string(&workspace, "workspaceId")?;
    if required_string(&workspace, "mode")? != "shadow"
        || required_u64(&workspace, "inventoryGeneration")? != 1
    {
        return Err(BenchmarkError::WorkspaceModeMismatch);
    }
    let mut shadow_open = request_params(
        json!({
            "externalKey": "ae-sdd-hook-benchmark-session",
            "role": "root",
            "engaged": false
        }),
        Some(&workspace_id),
        None,
        None,
        Some("benchmark-session-open-shadow"),
    );
    shadow_open.work_item_id = Some(WORK_ITEM_ID.to_owned());
    let shadow_session: Value = call_sync(hook_client, RpcMethod::SessionOpen, shadow_open)?;
    if required_bool(&shadow_session, "engaged")? {
        return Err(BenchmarkError::WorkspaceModeMismatch);
    }
    let session_id = required_string(&shadow_session, "sessionId")?;

    let mut drain = request_params(
        json!({"stop": false}),
        None,
        None,
        None,
        Some("benchmark-runtime-drain"),
    );
    drain.confirmation = Some(confirmation.clone());
    let _: Value = call_sync(admin_client, RpcMethod::RuntimeDrain, drain)?;

    let mut transition = request_params(
        parity_transition_payload()?,
        Some(&workspace_id),
        None,
        None,
        Some("benchmark-workspace-canary"),
    );
    transition.confirmation = Some(confirmation);
    let canary_workspace: Value =
        call_sync(admin_client, RpcMethod::WorkspaceModeTransition, transition)?;
    if required_string(&canary_workspace, "mode")? != "rust-canary"
        || required_u64(&canary_workspace, "inventoryGeneration")? != CANARY_INVENTORY_GENERATION
    {
        return Err(BenchmarkError::WorkspaceModeMismatch);
    }

    let mut canary_open = request_params(
        json!({
            "externalKey": "ae-sdd-hook-benchmark-session",
            "role": "root",
            "engaged": true
        }),
        Some(&workspace_id),
        None,
        None,
        Some("benchmark-session-open-canary"),
    );
    canary_open.work_item_id = Some(WORK_ITEM_ID.to_owned());
    let canary_session: Value = call_sync(hook_client, RpcMethod::SessionOpen, canary_open)?;
    if !required_bool(&canary_session, "engaged")?
        || required_string(&canary_session, "sessionId")? != session_id
    {
        return Err(BenchmarkError::SessionCutoverMismatch);
    }
    let capability_token = required_string(&canary_session, "capabilityToken")?;
    let prepared = HookSession {
        workspace_id,
        session_id,
        capability_token,
        turn_id: "00000000-0000-0000-0000-000000000101".to_owned(),
        work_item_id: WORK_ITEM_ID.to_owned(),
    };
    let prompt: Value = call_sync(
        hook_client,
        RpcMethod::HookUserPrompt,
        prepared.params(
            "benchmark-user-prompt",
            json!({"hookEventId":"benchmark-user-prompt","turnSeq":1,"hostPayload":{"prompt":"benchmark"}}),
        ),
    )?;
    validate_authoritative_prompt(&prompt, expected_input_fingerprint)?;
    let first = cached_hook_call(hook_client, &prepared)?;
    validate_controlled_hook(&first, false)?;
    Ok(prepared)
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BenchmarkParityEvidence {
    comparison_count: u64,
    mismatch_count: u64,
    source_revision: u64,
    legacy_digest: String,
    rust_digest: String,
    observed_at_unix_ms: u64,
}

pub(super) fn parity_transition_payload() -> Result<Value, BenchmarkError> {
    let observation_digest = "a".repeat(64);
    let parity = BenchmarkParityEvidence {
        comparison_count: 10,
        mismatch_count: 0,
        source_revision: 1,
        legacy_digest: observation_digest.clone(),
        rust_digest: observation_digest,
        observed_at_unix_ms: now_unix_ms(),
    };
    let parity_digest = hex::encode(Sha256::digest(serde_json::to_vec(&parity)?));
    Ok(json!({
        "targetMode": "rust-canary",
        "reason": "live benchmark verified typed shadow parity",
        "parityDigest": parity_digest,
        "parity": parity,
    }))
}

fn validate_authoritative_prompt(
    result: &Value,
    expected_input_fingerprint: &str,
) -> Result<(), BenchmarkError> {
    let context = result
        .get("context")
        .and_then(Value::as_object)
        .ok_or(BenchmarkError::AuthoritativeContextMismatch)?;
    let guard = context
        .get("hookGuard")
        .and_then(Value::as_object)
        .ok_or(BenchmarkError::AuthoritativeContextMismatch)?;
    let valid = result.get("engaged").and_then(Value::as_bool) == Some(true)
        && result.get("decision").and_then(Value::as_str) == Some("context")
        && context.get("workItemId").and_then(Value::as_str) == Some(WORK_ITEM_ID)
        && context.get("stateRevision").and_then(Value::as_u64) == Some(1)
        && context.get("inventoryGeneration").and_then(Value::as_u64)
            == Some(CANARY_INVENTORY_GENERATION)
        && context.get("inputFingerprint").and_then(Value::as_str)
            == Some(expected_input_fingerprint)
        && guard.get("outcome").and_then(Value::as_str) == Some("PASS")
        && guard.get("stateRevision").and_then(Value::as_u64) == Some(1)
        && guard.get("inventoryGeneration").and_then(Value::as_u64)
            == Some(CANARY_INVENTORY_GENERATION)
        && guard.get("inputFingerprint").and_then(Value::as_str)
            == Some(expected_input_fingerprint);
    if valid {
        Ok(())
    } else {
        Err(BenchmarkError::AuthoritativeContextMismatch)
    }
}

pub(super) fn validate_controlled_hook(
    result: &Value,
    replayed: bool,
) -> Result<(), BenchmarkError> {
    if result.get("engaged").and_then(Value::as_bool) != Some(true) {
        return Err(BenchmarkError::HookControlMissing);
    }
    if result.get("decision").and_then(Value::as_str) != Some("allow") {
        return Err(BenchmarkError::HookDecisionMismatch);
    }
    if result.get("replayed").and_then(Value::as_bool) != Some(replayed) {
        return Err(if replayed {
            BenchmarkError::ReceiptReplayMissing
        } else {
            BenchmarkError::ReceiptSeedMissing
        });
    }
    Ok(())
}

pub(super) fn cached_hook_call(
    client: &DaemonClient,
    session: &HookSession,
) -> Result<Value, BenchmarkError> {
    call_sync(
        client,
        RpcMethod::HookPreTool,
        session.params(
            "benchmark-pre-tool-replay",
            json!({
                "hookEventId": "benchmark-pre-tool-replay",
                "turnSeq": 1,
                "hostPayload": {"tool": "apply_patch", "path": "benchmark"}
            }),
        ),
    )
}

pub(super) struct HookSession {
    workspace_id: String,
    session_id: String,
    capability_token: String,
    turn_id: String,
    work_item_id: String,
}

impl HookSession {
    fn params(&self, idempotency_key: &str, payload: Value) -> RequestParams<Value> {
        let mut params = request_params(
            payload,
            Some(&self.workspace_id),
            Some(&self.session_id),
            Some(&self.capability_token),
            Some(idempotency_key),
        );
        params.agent_id = Some("benchmark-agent".to_owned());
        params.turn_id = Some(self.turn_id.clone());
        params.work_item_id = Some(self.work_item_id.clone());
        params.deadline_ms = HOOK_DEADLINE_MS;
        params
    }
}

fn request_params(
    payload: Value,
    workspace_id: Option<&str>,
    session_id: Option<&str>,
    capability_token: Option<&str>,
    idempotency_key: Option<&str>,
) -> RequestParams<Value> {
    RequestParams {
        protocol_version: "1.0".to_owned(),
        workspace_id: workspace_id.map(str::to_owned),
        agent_id: Some("benchmark-agent".to_owned()),
        session_id: session_id.map(str::to_owned),
        capability_token: capability_token.map(str::to_owned),
        turn_id: None,
        work_item_id: None,
        lease_id: None,
        fencing_token: None,
        expected_revision: None,
        idempotency_key: idempotency_key.map(str::to_owned),
        confirmation: None,
        deadline_ms: 1_000,
        payload,
    }
}

fn required_string(value: &Value, field: &'static str) -> Result<String, BenchmarkError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(BenchmarkError::ResponseField(field))
}

fn required_u64(value: &Value, field: &'static str) -> Result<u64, BenchmarkError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(BenchmarkError::ResponseField(field))
}

fn required_bool(value: &Value, field: &'static str) -> Result<bool, BenchmarkError> {
    value
        .get(field)
        .and_then(Value::as_bool)
        .ok_or(BenchmarkError::ResponseField(field))
}

fn call_sync<R: serde::de::DeserializeOwned>(
    client: &DaemonClient,
    method: RpcMethod,
    params: RequestParams<Value>,
) -> Result<R, BenchmarkError> {
    block_on(client.call(method, params)).map_err(BenchmarkError::Client)
}

pub(super) struct BenchmarkWorkspace {
    allowed_root: PathBuf,
    path: PathBuf,
    state_path: PathBuf,
}

impl BenchmarkWorkspace {
    pub(super) fn create(allowed_root: &Path) -> Result<Self, BenchmarkError> {
        let path = allowed_root.join(format!(
            ".ae-sdd-hook-benchmark-{}-{}",
            std::process::id(),
            now_unix_ms()
        ));
        if path == allowed_root || !path.starts_with(allowed_root) {
            return Err(BenchmarkError::UnsafeFixturePath(path));
        }
        let work_item = path.join(".auto-engineering").join(WORK_ITEM_ID);
        fs::create_dir_all(&work_item).map_err(BenchmarkError::Io)?;
        Ok(Self {
            allowed_root: allowed_root.to_owned(),
            path,
            state_path: work_item.join("state.json"),
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn write_authoritative_state(
        &self,
        policy_digest: &str,
    ) -> Result<String, BenchmarkError> {
        let (state, input_fingerprint) = authoritative_state(policy_digest)?;
        fs::write(&self.state_path, serde_json::to_vec_pretty(&state)?)
            .map_err(BenchmarkError::Io)?;
        Ok(input_fingerprint)
    }
}

impl Drop for BenchmarkWorkspace {
    fn drop(&mut self) {
        if self.path != self.allowed_root && self.path.starts_with(&self.allowed_root) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub(super) fn authoritative_state(policy_digest: &str) -> Result<(Value, String), BenchmarkError> {
    let mut state = json!({
        "stateMachineName": WORK_ITEM_ID,
        "currentWorkItem": WORK_ITEM_ID,
        "revision": 1,
        "currentPhase": "coding",
    });
    let input_fingerprint = hex::encode(Sha256::digest(serde_json::to_vec(&state)?));
    state["hookGuard"] = json!({
        "outcome": "PASS",
        "stateRevision": 1,
        "policyDigest": policy_digest,
        "inventoryGeneration": CANARY_INVENTORY_GENERATION,
        "inputFingerprint": input_fingerprint,
    });
    Ok((state, input_fingerprint))
}
