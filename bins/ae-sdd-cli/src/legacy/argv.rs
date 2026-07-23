use std::collections::BTreeMap;
use std::path::PathBuf;

use ae_sdd_protocol::{ConfirmationRef, PROTOCOL_VERSION_V1, RequestParams, RpcMethod};
use serde_json::{Map, Value};

use super::tokens::{LegacyArgumentError, ParsedArguments, kebab_to_camel, validate_text};
use super::{LegacyCommandRoute, LegacyRpcAdapter, LegacyTarget};

/// One strict legacy RPC invocation after control-plane flags are separated
/// from command business flags.
#[derive(Debug)]
pub struct LegacyRpcInvocation {
    pub request: LegacyRequestSource,
    pub manifest: Option<PathBuf>,
}

/// The advanced full-JSON path is retained, while normal callers receive a
/// synthesized protocol request.
#[derive(Debug)]
pub enum LegacyRequestSource {
    ExplicitJson(String),
    Synthesized(Box<RequestParams<Value>>),
}

/// Parse legacy RPC argv. Environment access is injected so tests never need
/// to mutate process-global environment state.
pub fn parse_rpc_invocation<F>(
    route: &LegacyCommandRoute,
    method: RpcMethod,
    arguments: &[String],
    environment: F,
) -> Result<LegacyRpcInvocation, LegacyArgumentError>
where
    F: Fn(&str) -> Option<String>,
{
    let job_submission = matches!(
        &route.target,
        LegacyTarget::Rpc {
            adapter: LegacyRpcAdapter::JobSubmission { .. },
            ..
        }
    );
    let repeatable_flags = if job_submission && route.command_id == "git impact" {
        &["file"][..]
    } else {
        &[]
    };
    let mut parsed =
        ParsedArguments::parse_with_repeatable(arguments, &["json"], repeatable_flags)?;
    if !job_submission && !parsed.positionals.is_empty() {
        return Err(LegacyArgumentError::new(format!(
            "legacy daemon command {} accepts only named --kebab-case flags",
            route.command_id
        )));
    }
    parsed.take_boolean("json")?;

    let explicit_json = parsed.take_required_optional("request-json")?;
    let manifest =
        take_text(&mut parsed, &["manifest"], "AE_SDD_MANIFEST", &environment)?.map(PathBuf::from);
    if let Some(request_json) = explicit_json {
        if !parsed.options.is_empty() || !parsed.positionals.is_empty() {
            return Err(LegacyArgumentError::new(
                "--request-json cannot be combined with synthesized request flags",
            ));
        }
        return Ok(LegacyRpcInvocation {
            request: LegacyRequestSource::ExplicitJson(request_json),
            manifest,
        });
    }

    let protocol_version = take_text(
        &mut parsed,
        &["protocol-version"],
        "AE_SDD_PROTOCOL_VERSION",
        &environment,
    )?
    .unwrap_or_else(|| PROTOCOL_VERSION_V1.to_owned());
    let workspace_id = take_text(
        &mut parsed,
        &["workspace-id", "workspace"],
        "AE_SDD_WORKSPACE_ID",
        &environment,
    )?;
    let agent_id = take_text(
        &mut parsed,
        &["agent-id", "agent"],
        "AE_SDD_AGENT_ID",
        &environment,
    )?;
    let session_id = take_text(
        &mut parsed,
        &["session-id", "session"],
        "AE_SDD_SESSION_ID",
        &environment,
    )?;
    let capability_token = take_text(
        &mut parsed,
        &["capability-token"],
        "AE_SDD_CAPABILITY_TOKEN",
        &environment,
    )?;
    let turn_id = take_text(&mut parsed, &["turn-id"], "AE_SDD_TURN_ID", &environment)?;
    let work_item_id = take_text(
        &mut parsed,
        &["work-item-id", "work-item"],
        "AE_SDD_WORK_ITEM_ID",
        &environment,
    )?;
    let lease_id = take_text(&mut parsed, &["lease-id"], "AE_SDD_LEASE_ID", &environment)?;
    let fencing_token = take_u64(
        &mut parsed,
        &["fencing-token"],
        "AE_SDD_FENCING_TOKEN",
        &environment,
    )?;
    let expected_revision = take_u64(
        &mut parsed,
        &["expected-revision", "revision"],
        "AE_SDD_EXPECTED_REVISION",
        &environment,
    )?;
    let idempotency_key = take_text(
        &mut parsed,
        &["idempotency-key"],
        "AE_SDD_IDEMPOTENCY_KEY",
        &environment,
    )?;
    let confirmation = take_confirmation(&mut parsed, &environment)?;
    let deadline_ms = take_u64(
        &mut parsed,
        &["deadline-ms"],
        "AE_SDD_DEADLINE_MS",
        &environment,
    )?
    .unwrap_or(route.contract.deadline_ms);

    let payload = if let Some(raw) = parsed.take_required_optional("payload-json")? {
        if !parsed.options.is_empty() || !parsed.positionals.is_empty() {
            return Err(LegacyArgumentError::new(
                "--payload-json cannot be combined with individual business flags",
            ));
        }
        serde_json::from_str(&raw).map_err(|error| {
            LegacyArgumentError::new(format!("--payload-json is invalid JSON: {error}"))
        })?
    } else {
        business_payload(parsed.options, parsed.positionals)?
    };

    let params = RequestParams {
        protocol_version,
        workspace_id,
        agent_id,
        session_id,
        capability_token,
        turn_id,
        work_item_id,
        lease_id,
        fencing_token,
        expected_revision,
        idempotency_key,
        confirmation,
        deadline_ms,
        payload,
    };
    validate_request_params(route, method, &params)?;
    Ok(LegacyRpcInvocation {
        request: LegacyRequestSource::Synthesized(Box::new(params)),
        manifest,
    })
}

/// Validate both synthesized and advanced full-JSON requests against the
/// frozen route and protocol method descriptor before opening IPC.
pub fn validate_request_params(
    route: &LegacyCommandRoute,
    method: RpcMethod,
    params: &RequestParams<Value>,
) -> Result<(), LegacyArgumentError> {
    if params.protocol_version != PROTOCOL_VERSION_V1 {
        return Err(LegacyArgumentError::new(format!(
            "unsupported protocol version {}",
            params.protocol_version
        )));
    }
    if params.deadline_ms == 0 || params.deadline_ms > route.contract.deadline_ms {
        return Err(LegacyArgumentError::new(format!(
            "deadlineMs must be between 1 and the frozen {} ms route budget",
            route.contract.deadline_ms
        )));
    }
    let requirements = method.spec().requirements;
    require(
        route.identity_workspace || requirements.requires_workspace,
        params.workspace_id.as_deref(),
        "workspace identity (--workspace-id or AE_SDD_WORKSPACE_ID)",
    )?;
    require(
        route.identity_work_item || requirements.requires_work_item,
        params.work_item_id.as_deref(),
        "work-item identity (--work-item-id or AE_SDD_WORK_ITEM_ID)",
    )?;
    require(
        route.identity_session,
        params.session_id.as_deref(),
        "session identity (--session-id or AE_SDD_SESSION_ID)",
    )?;
    if route.identity_session {
        require(
            true,
            params.agent_id.as_deref(),
            "agent identity (--agent-id or AE_SDD_AGENT_ID)",
        )?;
        require(
            true,
            params.capability_token.as_deref(),
            "session capability (--capability-token or AE_SDD_CAPABILITY_TOKEN)",
        )?;
    }
    require(
        requirements.requires_idempotency,
        params.idempotency_key.as_deref(),
        "idempotency key (--idempotency-key or AE_SDD_IDEMPOTENCY_KEY)",
    )?;
    if requirements.requires_confirmation && params.confirmation.is_none() {
        return Err(LegacyArgumentError::new(
            "this command requires confirmation-id, approved-by, and approved-at",
        ));
    }
    if params.lease_id.is_some() != params.fencing_token.is_some() {
        return Err(LegacyArgumentError::new(
            "lease-id and fencing-token must be supplied together",
        ));
    }
    Ok(())
}

fn require(
    required: bool,
    value: Option<&str>,
    description: &str,
) -> Result<(), LegacyArgumentError> {
    if required && value.is_none_or(str::is_empty) {
        Err(LegacyArgumentError::new(format!(
            "missing required {description}"
        )))
    } else {
        Ok(())
    }
}

fn take_confirmation<F>(
    parsed: &mut ParsedArguments,
    environment: &F,
) -> Result<Option<ConfirmationRef>, LegacyArgumentError>
where
    F: Fn(&str) -> Option<String>,
{
    let confirmation_id = take_text(
        parsed,
        &["confirmation-id"],
        "AE_SDD_CONFIRMATION_ID",
        environment,
    )?;
    let approved_by = take_text(
        parsed,
        &["approved-by", "confirmation-approved-by"],
        "AE_SDD_CONFIRMATION_APPROVED_BY",
        environment,
    )?;
    let approved_at = take_text(
        parsed,
        &["approved-at", "confirmation-approved-at"],
        "AE_SDD_CONFIRMATION_APPROVED_AT",
        environment,
    )?;
    if confirmation_id.is_none() && approved_by.is_none() && approved_at.is_none() {
        return Ok(None);
    }
    Ok(Some(ConfirmationRef {
        confirmation_id: confirmation_id.ok_or_else(|| {
            LegacyArgumentError::new("partial confirmation is missing confirmation-id")
        })?,
        approved_by: approved_by.ok_or_else(|| {
            LegacyArgumentError::new("partial confirmation is missing approved-by")
        })?,
        approved_at: approved_at.ok_or_else(|| {
            LegacyArgumentError::new("partial confirmation is missing approved-at")
        })?,
    }))
}

fn take_text<F>(
    parsed: &mut ParsedArguments,
    aliases: &[&str],
    environment_name: &str,
    environment: &F,
) -> Result<Option<String>, LegacyArgumentError>
where
    F: Fn(&str) -> Option<String>,
{
    let explicit = parsed.take_aliases(aliases)?;
    let value = explicit.or_else(|| environment(environment_name));
    value
        .map(|value| validate_text(environment_name, value))
        .transpose()
}

fn take_u64<F>(
    parsed: &mut ParsedArguments,
    aliases: &[&str],
    environment_name: &str,
    environment: &F,
) -> Result<Option<u64>, LegacyArgumentError>
where
    F: Fn(&str) -> Option<String>,
{
    take_text(parsed, aliases, environment_name, environment)?
        .map(|value| {
            value.parse::<u64>().map_err(|_| {
                LegacyArgumentError::new(format!("{environment_name} must be an unsigned integer"))
            })
        })
        .transpose()
}

fn business_payload(
    options: BTreeMap<String, Option<String>>,
    positionals: Vec<String>,
) -> Result<Value, LegacyArgumentError> {
    let mut payload = Map::new();
    for (name, raw) in options {
        let field = kebab_to_camel(&name)?;
        let value = raw.map_or(Value::Bool(true), |value| parse_business_value(&value));
        if payload.insert(field.clone(), value).is_some() {
            return Err(LegacyArgumentError::new(format!(
                "business field {field} was supplied more than once"
            )));
        }
    }
    if !positionals.is_empty()
        && payload
            .insert(
                "legacyPositionals".to_owned(),
                Value::Array(positionals.into_iter().map(Value::String).collect()),
            )
            .is_some()
    {
        return Err(LegacyArgumentError::new(
            "legacyPositionals is reserved for the job adapter",
        ));
    }
    Ok(Value::Object(payload))
}

fn parse_business_value(value: &str) -> Value {
    if matches!(value, "true" | "false" | "null")
        || value.starts_with(['{', '[', '"'])
        || value
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'-')
    {
        serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_owned()))
    } else {
        Value::String(value.to_owned())
    }
}
