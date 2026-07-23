use super::*;

impl RuntimeService {
    pub(super) fn host_register(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let credential = params.capability_token.as_deref().ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::EndpointAuthFailed,
                "boot-scoped host adapter credential is required",
            )
        })?;
        if !constant_time_equal(credential.as_bytes(), self.endpoint_token.as_bytes()) {
            return Err(RuntimeError::new(
                StableErrorCode::EndpointAuthFailed,
                "host adapter credential is invalid for this daemon boot",
            ));
        }
        let payload: HostRegisterPayload = decode_value(params.payload.clone())?;
        let key = require_idempotency(params)?;
        let scope = format!("host-register\0{}", payload.adapter_id);
        let digest = canonical_digest(&payload)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        self.host
            .register(&payload.adapter_id, &payload.capabilities)?;
        let value = to_value(payload)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "host.registered",
            None,
            None,
            None,
        )
        .map(|(value, _)| value)
    }

    pub(super) fn host_capabilities(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let adapter_id = params
            .payload
            .get("adapterId")
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("adapterId is required"))?;
        let value = self
            .persistence
            .load_record("host-adapter/v1", adapter_id)?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::HostCapabilityUnsupported,
                    "host adapter is not registered",
                )
            })?;
        Ok(value)
    }

    pub(super) fn host_action_next(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let adapter_id = params
            .payload
            .get("adapterId")
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("adapterId is required"))?;
        to_value(self.host.next(adapter_id)?)
    }

    pub(super) fn host_action_ack(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let adapter_id = params
            .payload
            .get("adapterId")
            .and_then(Value::as_str)
            .ok_or_else(|| schema_error("adapterId is required"))?
            .to_owned();
        let ack_value = params
            .payload
            .get("ack")
            .cloned()
            .ok_or_else(|| schema_error("ack is required"))?;
        let ack: HostAckPayload = decode_value(ack_value)?;
        let key = require_idempotency(params)?;
        let scope = format!("host-ack\0{}\0{}", adapter_id, ack.action_id);
        let digest = canonical_digest(&ack)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let action = self.host.acknowledge(&adapter_id, ack.clone())?;
        if action.kind == "compact"
            && ack.outcome == "accepted"
            && let Some(compact_id) = &action.compact_id
        {
            self.update_compact_status(compact_id, "host-acknowledged", None)?;
        }
        let value = to_value(action)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "host.action_acknowledged",
            None,
            None,
            None,
        )
        .map(|(value, _)| value)
    }

    pub(super) fn host_pressure(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let payload: HostPressurePayload = decode_value(params.payload.clone())?;
        self.host
            .require_capabilities(&payload.adapter_id, &["pressure"])?;
        let session_id = SessionId::from_str(&identity.session_id)
            .map_err(|_| schema_error("sessionId is not a UUID"))?;
        let decision = self.context.observe_pressure(session_id, &payload)?;
        let compact = if decision == PressureDecision::TriggerCompact {
            let projection = self.context.project(&identity.session_id, 0, "")?;
            Some(self.start_compact(
                &identity.session_id,
                payload.context_generation,
                &projection.digest,
                payload.observed_at_unix_ms.saturating_add(30_000),
                &payload.adapter_id,
            )?)
        } else {
            None
        };
        Ok(json!({"decision":format!("{decision:?}"),"compact":compact}))
    }

    pub(super) fn delegation_create(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let payload: DelegationCreatePayload = decode_value(params.payload.clone())?;
        let key = require_idempotency(params)?;
        let scope = format!(
            "delegation-create\0{}\0{}",
            identity.session_id,
            params.work_item_id.as_deref().unwrap_or("")
        );
        let digest = canonical_digest(&payload)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let projection = self
            .delegation
            .create(&identity.session_id, identity.role, payload)?;
        self.persistence.store_record(
            "delegation-memory/v1",
            &projection.delegation_id,
            &json!({
                "schemaVersion":"delegation-memory/v1",
                "delegationId":projection.delegation_id,
                "workspaceId":identity.workspace_id,
                "workItemId":params.work_item_id,
                "status":"active",
                "payloadPurged":false,
            }),
        )?;
        let value = to_value(projection)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "delegation.created",
            Some(identity.workspace_id),
            Some(identity.session_id),
            params.work_item_id.clone(),
        )
        .map(|(value, _)| value)
    }

    pub(super) fn delegation_status(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let id = payload_string(&params.payload, "delegationId")?;
        to_value(self.delegation.status(&identity.session_id, id)?)
    }

    pub(super) fn delegation_accept(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let payload: DelegationAcceptPayload = decode_value(params.payload.clone())?;
        let key = require_idempotency(params)?;
        let scope = format!("delegation-accept\0{}", payload.delegation_id);
        let digest = canonical_digest(&payload)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let projection = self.delegation.accept(
            &payload.delegation_id,
            &payload.claim_id,
            &payload.action_id,
            &payload.child_session_id,
            payload.expires_at_unix_ms,
            self.clock.now_unix_ms(),
        )?;
        let value = to_value(projection)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "delegation.accepted",
            params.workspace_id.clone(),
            Some(payload.child_session_id),
            params.work_item_id.clone(),
        )
        .map(|(value, _)| value)
    }

    pub(super) fn delegation_report(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let payload: DelegationReportPayload = decode_value(params.payload.clone())?;
        let key = require_idempotency(params)?;
        let scope = format!("delegation-report\0{}", payload.delegation_id);
        let digest = canonical_digest(&payload)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let memory = self
            .persistence
            .load_record("delegation-memory/v1", &payload.delegation_id)?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ChildResultInvalid,
                    "daemon-owned delegation memory namespace is missing",
                )
            })?;
        if memory.get("workspaceId").and_then(Value::as_str) != Some(identity.workspace_id.as_str())
            || !matches!(
                memory.get("status").and_then(Value::as_str),
                Some("active" | "cleaned")
            )
        {
            return Err(RuntimeError::new(
                StableErrorCode::ChildResultInvalid,
                "delegation memory namespace is outside the trusted workspace or lifecycle",
            ));
        }
        let delegation_id = payload.delegation_id.clone();
        self.delegation.report(&identity.session_id, payload)?;
        let workspace = self.business_workspace_for(&identity)?;
        let (mut status, result, mut artifact_receipt) =
            self.delegation.completion_material(&delegation_id)?;
        if status == "result-staged" {
            let receipt =
                self.business
                    .validate_delegation_artifacts(&workspace, &delegation_id, &result)?;
            self.delegation
                .artifacts_validated(&delegation_id, receipt.clone())?;
            artifact_receipt = Some(receipt);
            status = "artifacts-validated".to_owned();
        }
        if status == "artifacts-validated" {
            let artifact_receipt = artifact_receipt.ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ChildResultInvalid,
                    "artifact validation receipt is missing after validation",
                )
            })?;
            let receipt = self.business.cleanup_delegation_memory(
                &workspace,
                &delegation_id,
                &result,
                &artifact_receipt,
            )?;
            let cleaned_namespace = self
                .persistence
                .load_record("delegation-memory/v1", &delegation_id)?
                .ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::ChildResultInvalid,
                        "delegation memory namespace disappeared during cleanup",
                    )
                })?;
            if cleaned_namespace.get("workspaceId").and_then(Value::as_str)
                != Some(identity.workspace_id.as_str())
                || cleaned_namespace.get("status").and_then(Value::as_str) != Some("cleaned")
                || cleaned_namespace
                    .get("payloadPurged")
                    .and_then(Value::as_bool)
                    != Some(true)
                || cleaned_namespace
                    .get("memorySnapshotDigest")
                    .and_then(Value::as_str)
                    != receipt.get("memorySnapshotDigest").and_then(Value::as_str)
            {
                return Err(RuntimeError::new(
                    StableErrorCode::ChildResultInvalid,
                    "memory cleanup did not commit the daemon-owned namespace tombstone",
                ));
            }
            self.delegation.memory_cleaned(&delegation_id, receipt)?;
        }
        let value = to_value(
            self.delegation
                .status(&identity.session_id, &delegation_id)?,
        )?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "delegation.result_validated_and_memory_cleaned",
            Some(identity.workspace_id),
            Some(identity.session_id),
            params.work_item_id.clone(),
        )
        .map(|(value, _)| value)
    }

    pub(super) fn delegation_collect(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let id = payload_string(&params.payload, "delegationId")?;
        let key = require_idempotency(params)?;
        let scope = format!("delegation-collect\0{}", id);
        let digest = canonical_digest(&params.payload)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let value = self.delegation.collect(&identity.session_id, id)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "delegation.collected",
            Some(identity.workspace_id),
            Some(identity.session_id),
            params.work_item_id.clone(),
        )
        .map(|(value, _)| value)
    }

    pub(super) fn delegation_cancel(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let id = payload_string(&params.payload, "delegationId")?;
        let key = require_idempotency(params)?;
        let scope = format!("delegation-cancel\0{}", id);
        let digest = canonical_digest(&params.payload)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let value = to_value(self.delegation.cancel(&identity.session_id, id)?)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "delegation.cancelled",
            Some(identity.workspace_id),
            Some(identity.session_id),
            params.work_item_id.clone(),
        )
        .map(|(value, _)| value)
    }

    pub(super) fn compact_request(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let payload: CompactRequestPayload = decode_value(params.payload.clone())?;
        if payload.adapter_id.is_empty()
            || payload.deadline_unix_ms <= self.clock.now_unix_ms()
            || payload.snapshot_digest.len() != 64
            || payload
                .snapshot_digest
                .bytes()
                .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
        {
            return Err(schema_error(
                "compact request requires a registered adapter, a future deadline, and a lowercase snapshot digest",
            ));
        }
        let key = require_idempotency(params)?;
        let scope = format!("compact-request\0{}", identity.session_id);
        let digest = canonical_digest(&payload)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let result = self.start_compact(
            &identity.session_id,
            payload.previous_generation,
            &payload.snapshot_digest,
            payload.deadline_unix_ms,
            &payload.adapter_id,
        )?;
        let value = to_value(result)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "compact.requested",
            Some(identity.workspace_id),
            Some(identity.session_id),
            params.work_item_id.clone(),
        )
        .map(|(value, _)| value)
    }

    pub(super) fn compact_status(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let compact_id = payload_string(&params.payload, "compactId")?;
        self.persistence
            .load_record("compact/v1", compact_id)?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::CompactAckInvalid,
                    "compact cycle does not exist",
                )
            })
    }

    pub(super) fn authoritative_business(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
    ) -> RuntimeResult<Value> {
        let business_workspace = params
            .workspace_id
            .as_deref()
            .map(|workspace_id| {
                let state = self.lock_state()?;
                let record = state
                    .workspaces
                    .get(workspace_id)
                    .ok_or_else(|| project_mismatch("workspace is not registered"))?;
                let agent_role = params
                    .session_id
                    .as_deref()
                    .and_then(|session_id| state.sessions.get(session_id))
                    .map(|session| AgentRole::from(session.result.role));
                Ok(BusinessWorkspace {
                    workspace_id: record.result.workspace_id.clone(),
                    canonical_root: record.result.canonical_root.clone(),
                    project_key: record.result.project_key.clone(),
                    mode: record.result.mode,
                    agent_role,
                    inventory_generation: record.result.inventory_generation,
                })
            })
            .transpose()?;
        self.enforce_writer_mode(method, params, business_workspace.as_ref())?;
        let result = if let (Some(workspace), Some(work_item)) =
            (&params.workspace_id, &params.work_item_id)
        {
            self.actors
                .execute(workspace, work_item, params.deadline_ms, || {
                    self.business
                        .execute(method, params, business_workspace.as_ref())
                })
        } else {
            self.business
                .execute(method, params, business_workspace.as_ref())
        }?;
        if method == RpcMethod::OperationExecute
            && params
                .payload
                .get("operation")
                .and_then(Value::as_str)
                .and_then(|name| ae_sdd_operations::OperationName::from_str(name).ok())
                .is_some_and(|operation| operation.spec().writes)
            && let (Some(workspace_id), Some(work_item_id)) = (
                params.workspace_id.as_deref(),
                params.work_item_id.as_deref(),
            )
        {
            self.refresh_work_item_contexts(workspace_id, work_item_id)?;
        }
        Ok(result)
    }

    fn enforce_writer_mode(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
        workspace: Option<&BusinessWorkspace>,
    ) -> RuntimeResult<()> {
        if method != RpcMethod::OperationExecute {
            return Ok(());
        }
        let Some(operation_name) = params.payload.get("operation").and_then(Value::as_str) else {
            return Ok(());
        };
        let Ok(operation) = ae_sdd_operations::OperationName::from_str(operation_name) else {
            return Ok(());
        };
        if !operation.spec().writes {
            return Ok(());
        }
        let workspace = workspace.ok_or_else(|| project_mismatch("workspace is not registered"))?;
        if !matches!(
            workspace.mode,
            WorkspaceMode::RustCanary | WorkspaceMode::RustSoleWriter
        ) {
            return Err(RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "Rust mutation is forbidden until the daemon owns the workspace writer mode",
            ));
        }
        let identity = self.session_identity(params, false)?;
        if !identity.engaged || identity.capability_id != "hook.engaged" {
            return Err(RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "an unengaged Hook capability cannot authorize a Rust mutation",
            ));
        }
        Ok(())
    }

    pub(super) fn start_compact(
        &self,
        session_id: &str,
        previous_generation: u64,
        snapshot_digest: &str,
        deadline_unix_ms: u64,
        adapter_id: &str,
    ) -> RuntimeResult<CompactResult> {
        self.host.require_capabilities(adapter_id, &["compact"])?;
        {
            let state = self.lock_state()?;
            let session = state.sessions.get(session_id).ok_or_else(session_expired)?;
            if session.result.context_generation != previous_generation {
                return Err(RuntimeError::new(
                    StableErrorCode::CompactAckInvalid,
                    "compact previous generation does not match the trusted session",
                ));
            }
        }
        let compact_id = Uuid::new_v4().to_string();
        let action = self.host.enqueue(
            adapter_id,
            "compact",
            None,
            Some(compact_id.clone()),
            Some(session_id.to_owned()),
            Some(previous_generation),
            deadline_unix_ms,
        )?;
        let result = CompactResult {
            compact_id: compact_id.clone(),
            status: "compact-requested".to_owned(),
            previous_generation,
            next_generation: previous_generation.checked_add(1).ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::CompactAckInvalid,
                    "context generation overflow",
                )
            })?,
            action_id: action.action_id,
            restored_projection_digest: None,
        };
        let mut value = to_value(&result)?;
        value["snapshotDigest"] = Value::String(snapshot_digest.to_owned());
        value["sessionId"] = Value::String(session_id.to_owned());
        self.persistence
            .store_record("compact/v1", &compact_id, &value)?;
        self.persistence.store_record(
            "compact-active/v1",
            session_id,
            &json!({"schemaVersion":"compact-active/v1","compactId":compact_id}),
        )?;
        Ok(result)
    }

    pub(super) fn update_compact_status(
        &self,
        compact_id: &str,
        status: &str,
        restored_digest: Option<&str>,
    ) -> RuntimeResult<()> {
        let mut value = self
            .persistence
            .load_record("compact/v1", compact_id)?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::CompactAckInvalid,
                    "compact cycle does not exist",
                )
            })?;
        value["status"] = Value::String(status.to_owned());
        if let Some(digest) = restored_digest {
            value["restoredProjectionDigest"] = Value::String(digest.to_owned());
        }
        self.persistence
            .store_record("compact/v1", compact_id, &value)
    }

    pub(super) fn complete_compact_after_rehydrate(
        &self,
        session_id: &str,
        restored_digest: &str,
    ) -> RuntimeResult<()> {
        // The active cycle identity is persisted as a session-scoped pointer.
        let Some(pointer) = self
            .persistence
            .load_record("compact-active/v1", session_id)?
        else {
            return Ok(());
        };
        let compact_id = pointer
            .get("compactId")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "active compact pointer is malformed",
                )
            })?;
        let cycle = self
            .persistence
            .load_record("compact/v1", compact_id)?
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::CompactAckInvalid,
                    "active compact cycle is missing",
                )
            })?;
        if cycle.get("status").and_then(Value::as_str) == Some("context-restored") {
            return Ok(());
        }
        if cycle.get("status").and_then(Value::as_str) != Some("host-acknowledged") {
            return Err(RuntimeError::new(
                StableErrorCode::CompactAckInvalid,
                "context cannot be restored before correlated host ACK",
            ));
        }
        let previous = cycle
            .get("previousGeneration")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "compact previous generation is malformed",
                )
            })?;
        let next = cycle
            .get("nextGeneration")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "compact next generation is malformed",
                )
            })?;
        {
            let mut state = self.lock_state()?;
            let session = state
                .sessions
                .get_mut(session_id)
                .ok_or_else(session_expired)?;
            if session.result.context_generation != previous {
                return Err(RuntimeError::new(
                    StableErrorCode::CompactAckInvalid,
                    "context generation changed before compact rehydrate CAS",
                ));
            }
            session.result.context_generation = next;
        }
        self.persist_session(session_id)?;
        self.update_compact_status(compact_id, "context-restored", Some(restored_digest))?;
        self.append_runtime_event(
            "compact.context_restored",
            json!({
                "compactId":compact_id,
                "previousGeneration":previous,
                "nextGeneration":next,
                "restoredProjectionDigest":restored_digest,
            }),
            Some(
                self.lock_state()?
                    .sessions
                    .get(session_id)
                    .ok_or_else(session_expired)?
                    .workspace_id
                    .clone(),
            ),
            Some(session_id.to_owned()),
            None,
        )?;
        Ok(())
    }
}
