use super::*;

impl RuntimeService {
    pub(super) fn workspace_register(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let payload: WorkspaceRegisterPayload = decode_value(params.payload.clone())?;
        if !matches!(payload.mode, None | Some(WorkspaceMode::Shadow)) {
            return Err(RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "initial workspace mode is daemon-controlled and must be shadow",
            ));
        }
        let resolved = self.resolver.resolve(&payload.project_root)?;
        if !resolved.inside_allowed_root {
            return Err(RuntimeError::new(
                StableErrorCode::WorkspaceOutsideAllowedRoot,
                "workspace root is outside configured allowed roots",
            ));
        }
        let key = require_idempotency(params)?;
        let scope = format!("workspace-register\0{}", resolved.canonical_root);
        let digest = canonical_digest(&payload)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }

        let result = {
            let mut state = self.lock_state()?;
            if let Some(workspace_id) = state.workspace_by_root.get(&resolved.canonical_root) {
                let existing = state
                    .workspaces
                    .get(workspace_id)
                    .expect("workspace root index is internally consistent");
                if existing.result.project_key != payload.project_key {
                    return Err(RuntimeError::new(
                        StableErrorCode::ProjectMismatch,
                        "canonical workspace root is registered to another project",
                    ));
                }
                existing.result.clone()
            } else {
                if state.workspaces.len() >= self.config.max_workspaces {
                    return Err(RuntimeError::new(
                        StableErrorCode::SubscriberBackpressure,
                        "workspace capacity is exhausted",
                    ));
                }
                let result = WorkspaceResult {
                    workspace_id: Uuid::new_v4().to_string(),
                    canonical_root: resolved.canonical_root.clone(),
                    project_key: payload.project_key,
                    mode: WorkspaceMode::Shadow,
                    inventory_generation: 1,
                };
                state
                    .workspace_by_root
                    .insert(result.canonical_root.clone(), result.workspace_id.clone());
                state.workspaces.insert(
                    result.workspace_id.clone(),
                    WorkspaceRecord {
                        result: result.clone(),
                    },
                );
                result
            }
        };
        let value = to_value(&result)?;
        self.persistence
            .store_record("workspace/v1", &result.workspace_id, &value)?;
        let (value, _) = self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "workspace.registered",
            Some(result.workspace_id.clone()),
            None,
            None,
        )?;
        Ok(value)
    }

    pub(super) fn workspace_snapshot(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let workspace_id = require(&params.workspace_id, "workspaceId")?;
        let state = self.lock_state()?;
        to_value(
            &state
                .workspaces
                .get(workspace_id)
                .ok_or_else(|| project_mismatch("workspace is not registered"))?
                .result,
        )
    }

    pub(super) fn workspace_mode_transition(
        &self,
        params: &RequestParams<Value>,
        client_kind: Option<ClientKind>,
    ) -> RuntimeResult<Value> {
        if client_kind != Some(ClientKind::Admin) {
            return Err(RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "workspace migration mode requires an admin client",
            ));
        }
        if self.lifecycle()? != DaemonLifecycle::Draining {
            return Err(RuntimeError::new(
                StableErrorCode::DaemonDraining,
                "workspace migration mode may change only while the daemon is draining",
            ));
        }
        if self.admitted.load(Ordering::Acquire) != 1 {
            return Err(RuntimeError::new(
                StableErrorCode::DaemonDraining,
                "workspace writer cutover requires all previously admitted requests to quiesce",
            ));
        }
        let workspace_id = require(&params.workspace_id, "workspaceId")?.to_owned();
        let payload: WorkspaceModeTransitionPayload = decode_value(params.payload.clone())?;
        if payload.reason.is_empty()
            || payload.reason.len() > 512
            || payload.parity_digest.len() != 64
            || payload
                .parity_digest
                .bytes()
                .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
        {
            return Err(schema_error(
                "mode transition requires a bounded reason and lowercase parityDigest",
            ));
        }
        validate_parity_evidence(self, &payload.parity, &payload.parity_digest)?;
        let key = require_idempotency(params)?;
        let digest = canonical_digest(&payload)?;
        let scope = format!("workspace-mode\0{workspace_id}");
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            *self.lifecycle.write().map_err(lock_error)? = DaemonLifecycle::Running;
            return Ok(value);
        }
        let (result, invalidated_sessions) = {
            let mut state = self.lock_state()?;
            let result = {
                let workspace = state
                    .workspaces
                    .get_mut(&workspace_id)
                    .ok_or_else(|| project_mismatch("workspace is not registered"))?;
                let legal = matches!(
                    (workspace.result.mode, payload.target_mode),
                    (WorkspaceMode::Shadow, WorkspaceMode::RustCanary)
                        | (WorkspaceMode::RustCanary, WorkspaceMode::RustSoleWriter)
                );
                if !legal {
                    return Err(RuntimeError::new(
                        StableErrorCode::RoleOperationForbidden,
                        "workspace migration mode transition is not a legal forward edge",
                    ));
                }
                workspace.result.mode = payload.target_mode;
                workspace.result.inventory_generation = workspace
                    .result
                    .inventory_generation
                    .checked_add(1)
                    .ok_or_else(|| schema_error("inventory generation overflow"))?;
                workspace.result.clone()
            };
            let invalidated = state
                .sessions
                .iter_mut()
                .filter_map(|(session_id, session)| {
                    if session.workspace_id == workspace_id && session.active {
                        session.active = false;
                        Some(session_id.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            (result, invalidated)
        };
        let value = to_value(&result)?;
        self.persistence
            .store_record("workspace/v1", &workspace_id, &value)?;
        let parity_value = to_value(&payload.parity)?;
        self.persistence
            .store_record("workspace-parity/v1", &workspace_id, &parity_value)?;
        let committed = self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "workspace.mode_transitioned",
            Some(workspace_id),
            None,
            None,
        )?;
        for session_id in invalidated_sessions {
            self.persist_session(&session_id)?;
        }
        *self.lifecycle.write().map_err(lock_error)? = DaemonLifecycle::Running;
        Ok(committed.0)
    }
}

fn validate_parity_evidence(
    runtime: &RuntimeService,
    evidence: &WorkspaceParityEvidence,
    claimed_digest: &str,
) -> RuntimeResult<()> {
    if evidence.comparison_count == 0
        || evidence.mismatch_count != 0
        || evidence.source_revision == 0
        || evidence.legacy_digest.len() != 64
        || evidence.rust_digest.len() != 64
        || evidence.legacy_digest != evidence.rust_digest
        || evidence
            .legacy_digest
            .bytes()
            .chain(evidence.rust_digest.bytes())
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(schema_error(
            "parity evidence must contain matching lowercase digests, a positive revision, and zero mismatches",
        ));
    }
    let now = runtime.clock.now_unix_ms();
    if evidence.observed_at_unix_ms > now
        || now.saturating_sub(evidence.observed_at_unix_ms) > runtime.config.parity_evidence_ttl_ms
    {
        return Err(RuntimeError::new(
            StableErrorCode::ContextRevisionStale,
            "parity evidence is outside the daemon freshness window",
        ));
    }
    if canonical_digest(evidence)? != claimed_digest {
        return Err(RuntimeError::new(
            StableErrorCode::ExternalStateConflict,
            "parityDigest does not match the typed parity evidence",
        ));
    }
    Ok(())
}
