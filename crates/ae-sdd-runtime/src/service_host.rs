use super::*;

impl RuntimeService {
    pub(super) fn host_register(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let payload: HostRegisterPayload = decode_value(params.payload.clone())?;
        self.host
            .register(&payload.adapter_id, &payload.capabilities)?;
        to_value(payload)
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
        let action = self.host.acknowledge(&adapter_id, ack.clone())?;
        if action.kind == "compact" && ack.outcome == "accepted" {
            if let Some(compact_id) = &action.compact_id {
                self.update_compact_status(compact_id, "host-acknowledged", None)?;
            }
            if let Some(session_id) = &action.session_id {
                let projection = self.context.project(session_id, 0, "")?;
                self.complete_compact_after_rehydrate(session_id, &projection.digest)?;
            }
        }
        to_value(action)
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
        to_value(
            self.delegation
                .create(&identity.session_id, identity.role, payload)?,
        )
    }

    pub(super) fn delegation_status(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let id = payload_string(&params.payload, "delegationId")?;
        to_value(self.delegation.status(id)?)
    }

    pub(super) fn delegation_accept(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let payload: DelegationAcceptPayload = decode_value(params.payload.clone())?;
        to_value(self.delegation.accept(
            &payload.delegation_id,
            &payload.claim_id,
            &payload.action_id,
            &payload.child_session_id,
            payload.expires_at_unix_ms,
            self.clock.now_unix_ms(),
        )?)
    }

    pub(super) fn delegation_report(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let payload: DelegationReportPayload = decode_value(params.payload.clone())?;
        to_value(self.delegation.report(&identity.session_id, payload)?)
    }

    pub(super) fn delegation_collect(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let id = payload_string(&params.payload, "delegationId")?;
        self.delegation.collect(id)
    }

    pub(super) fn delegation_cancel(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let id = payload_string(&params.payload, "delegationId")?;
        to_value(self.delegation.cancel(id)?)
    }

    pub(super) fn compact_request(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let payload: CompactRequestPayload = decode_value(params.payload.clone())?;
        to_value(self.start_compact(
            &identity.session_id,
            payload.previous_generation,
            &payload.snapshot_digest,
            payload.deadline_unix_ms,
            &payload.adapter_id,
        )?)
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
                Ok(BusinessWorkspace {
                    workspace_id: record.result.workspace_id.clone(),
                    canonical_root: record.result.canonical_root.clone(),
                    project_key: record.result.project_key.clone(),
                })
            })
            .transpose()?;
        if let (Some(workspace), Some(work_item)) = (&params.workspace_id, &params.work_item_id) {
            self.actors
                .execute(workspace, work_item, params.deadline_ms, || {
                    self.business
                        .execute(method, params, business_workspace.as_ref())
                })
        } else {
            self.business
                .execute(method, params, business_workspace.as_ref())
        }
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
        self.update_compact_status(compact_id, "context-restored", Some(restored_digest))
    }
}
