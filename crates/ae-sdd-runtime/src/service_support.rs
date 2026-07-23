use super::*;

impl RuntimeService {
    pub(super) fn business_workspace_for(
        &self,
        identity: &TrustedSession,
    ) -> RuntimeResult<BusinessWorkspace> {
        let state = self.lock_state()?;
        let workspace = state
            .workspaces
            .get(&identity.workspace_id)
            .ok_or_else(|| project_mismatch("workspace is not registered"))?;
        Ok(BusinessWorkspace {
            workspace_id: workspace.result.workspace_id.clone(),
            canonical_root: workspace.result.canonical_root.clone(),
            project_key: workspace.result.project_key.clone(),
            mode: workspace.result.mode,
            agent_role: Some(AgentRole::from(identity.role)),
            inventory_generation: workspace.result.inventory_generation,
        })
    }

    /// Reprojects every active session that is bound to a Work Item.
    ///
    /// This runs only off the Hook fast path. A failed projection is removed so
    /// an engaged PreTool/Stop cannot reuse stale authorization context.
    pub fn refresh_active_contexts(&self) -> RuntimeResult<usize> {
        self.refresh_contexts(None, None)
    }

    pub(super) fn refresh_work_item_contexts(
        &self,
        workspace_id: &str,
        work_item_id: &str,
    ) -> RuntimeResult<usize> {
        self.refresh_contexts(Some(workspace_id), Some(work_item_id))
    }

    fn refresh_contexts(
        &self,
        workspace_filter: Option<&str>,
        work_item_filter: Option<&str>,
    ) -> RuntimeResult<usize> {
        let targets = {
            let state = self.lock_state()?;
            state
                .sessions
                .values()
                .filter(|session| session.active)
                .filter_map(|session| {
                    let work_item_id = session.current_work_item.as_deref()?;
                    if workspace_filter.is_some_and(|value| value != session.workspace_id)
                        || work_item_filter.is_some_and(|value| value != work_item_id)
                    {
                        return None;
                    }
                    let workspace = state.workspaces.get(&session.workspace_id)?;
                    Some((
                        BusinessWorkspace {
                            workspace_id: workspace.result.workspace_id.clone(),
                            canonical_root: workspace.result.canonical_root.clone(),
                            project_key: workspace.result.project_key.clone(),
                            mode: workspace.result.mode,
                            agent_role: Some(AgentRole::from(session.result.role)),
                            inventory_generation: workspace.result.inventory_generation,
                        },
                        work_item_id.to_owned(),
                        session.result.session_id.clone(),
                        AgentRole::from(session.result.role),
                    ))
                })
                .collect::<Vec<_>>()
        };
        let mut refreshed = 0;
        for (workspace, work_item_id, session_id, role) in targets {
            match self
                .business
                .project_context(&workspace, &work_item_id, &session_id, role)
                .and_then(|projection| self.context.put(projection).map(|_| ()))
            {
                Ok(()) => refreshed += 1,
                Err(_) => self.context.invalidate(&session_id)?,
            }
        }
        Ok(refreshed)
    }

    pub(super) fn append_runtime_event(
        &self,
        kind: &str,
        payload: Value,
        workspace_id: Option<String>,
        session_id: Option<String>,
        work_item_id: Option<String>,
    ) -> RuntimeResult<u64> {
        let payload_bytes = serde_json::to_vec(&payload).map_err(canonical_error)?;
        self.persistence
            .append_event(DurableEvent {
                event_store_id: self.persistence.event_store_id()?.to_string(),
                event_seq: 0,
                boot_id: self.boot_id.to_string(),
                kind: kind.to_owned(),
                workspace_id,
                session_id,
                work_item_id,
                payload,
                payload_digest: hex::encode(Sha256::digest(payload_bytes)),
            })
            .map(|event| event.event_seq)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn commit_receipt_event(
        &self,
        scope: &str,
        key: &str,
        request_digest: String,
        response: Value,
        kind: &str,
        workspace_id: Option<String>,
        session_id: Option<String>,
        work_item_id: Option<String>,
    ) -> RuntimeResult<(Value, u64)> {
        let response_json = serde_json::to_string(&response).map_err(canonical_error)?;
        let event_payload = json!({"scope":scope,"key":key});
        let event_payload_bytes = serde_json::to_vec(&event_payload).map_err(canonical_error)?;
        let event = DurableEvent {
            event_store_id: self.persistence.event_store_id()?.to_string(),
            event_seq: 0,
            boot_id: self.boot_id.to_string(),
            kind: kind.to_owned(),
            workspace_id,
            session_id,
            work_item_id,
            payload: event_payload,
            payload_digest: hex::encode(Sha256::digest(event_payload_bytes)),
        };
        let receipt = IdempotencyReceipt {
            scope: scope.to_owned(),
            key: key.to_owned(),
            request_digest,
            response_json,
            event_seq: 0,
        };
        let (_, receipt) = self.persistence.commit_event_and_receipt(event, receipt)?;
        let value = serde_json::from_str(&receipt.response_json).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "durable receipt response is malformed",
            )
        })?;
        Ok((value, receipt.event_seq))
    }

    pub(super) fn replay_receipt(
        &self,
        scope: &str,
        key: &str,
        request_digest: &str,
    ) -> RuntimeResult<Option<(Value, u64)>> {
        let Some(receipt) = self.persistence.load_receipt(scope, key)? else {
            return Ok(None);
        };
        if receipt.request_digest != request_digest {
            return Err(RuntimeError::new(
                StableErrorCode::IdempotencyKeyReused,
                "idempotency key was replayed with a different canonical payload",
            ));
        }
        let value = serde_json::from_str(&receipt.response_json).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "durable idempotency receipt is malformed",
            )
        })?;
        Ok(Some((value, receipt.event_seq)))
    }

    pub(super) fn admit(&self) -> RuntimeResult<Admission<'_>> {
        let previous = self.admitted.fetch_add(1, Ordering::AcqRel);
        if previous >= self.config.connection_capacity {
            self.admitted.fetch_sub(1, Ordering::AcqRel);
            return Err(RuntimeError::new(
                StableErrorCode::SubscriberBackpressure,
                "runtime request admission capacity is exhausted",
            ));
        }
        Ok(Admission {
            count: &self.admitted,
        })
    }

    pub(super) fn lifecycle(&self) -> RuntimeResult<DaemonLifecycle> {
        self.lifecycle
            .read()
            .map(|value| *value)
            .map_err(lock_error)
    }

    pub(super) fn lock_state(&self) -> RuntimeResult<std::sync::MutexGuard<'_, RuntimeState>> {
        self.state.lock().map_err(lock_error)
    }
}
