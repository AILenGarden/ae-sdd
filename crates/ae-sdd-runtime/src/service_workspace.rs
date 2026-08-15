use super::*;

impl RuntimeService {
    pub(super) fn workspace_register(
        &self,
        params: &RequestParams<Value>,
        _client_kind: Option<ClientKind>,
    ) -> RuntimeResult<Value> {
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
        let scope_digest = canonical_digest(&json!({
            "domain":"workspace.register/v1",
            "canonicalRoot":resolved.canonical_root,
        }))?;
        let initial_mode = WorkspaceMode::Shadow;
        let request_digest = canonical_digest(&json!({
            "canonicalRoot":resolved.canonical_root,
            "projectKey":payload.project_key,
            "mode":initial_mode,
        }))?;
        let (result, expected_mode, expected_generation) = {
            let state = self.lock_state()?;
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
                (
                    existing.result.clone(),
                    Some(existing.result.mode),
                    Some(existing.result.inventory_generation),
                )
            } else {
                if state.workspaces.len() >= self.config.max_workspaces {
                    return Err(RuntimeError::new(
                        StableErrorCode::SubscriberBackpressure,
                        "workspace capacity is exhausted",
                    ));
                }
                (
                    WorkspaceResult {
                        workspace_id: Uuid::new_v4().to_string(),
                        canonical_root: resolved.canonical_root.clone(),
                        project_key: payload.project_key.clone(),
                        mode: initial_mode,
                        inventory_generation: 1,
                    },
                    None,
                    None,
                )
            }
        };
        let value = to_value(&result)?;
        let now = self.clock.now_unix_ms();
        let snapshot = self
            .persistence
            .commit_identity_bundle(RuntimeIdentityTransition {
                operation: "workspace.register".to_owned(),
                scope_digest,
                idempotency_key: key.to_owned(),
                request_digest,
                expected_workspace_mode: expected_mode,
                expected_inventory_generation: expected_generation,
                expected_session_status: None,
                expected_delegation_status: None,
                expected_context_generation: None,
                snapshot: RuntimeIdentitySnapshot {
                    identity_kind: RuntimeIdentityKind::Workspace,
                    workspace: RuntimeWorkspaceRecord {
                        workspace_id: result.workspace_id.clone(),
                        canonical_root: result.canonical_root.clone(),
                        project_key: result.project_key.clone(),
                        mode: result.mode,
                        inventory_generation: result.inventory_generation,
                        dirty: false,
                        created_at_unix_ms: now,
                        updated_at_unix_ms: now,
                    },
                    session: None,
                    delegation: None,
                    host_action: None,
                    attestation: None,
                    response: value,
                    replayed: false,
                },
                committed_at_unix_ms: now,
            })?;
        let committed: WorkspaceResult = decode_value(snapshot.response.clone())?;
        {
            let mut state = self.lock_state()?;
            state.workspace_by_root.insert(
                committed.canonical_root.clone(),
                committed.workspace_id.clone(),
            );
            state.workspaces.insert(
                committed.workspace_id.clone(),
                WorkspaceRecord {
                    result: committed.clone(),
                },
            );
        }
        if !snapshot.replayed {
            self.append_runtime_event(
                "workspace.registered",
                json!({"workspaceId":committed.workspace_id}),
                Some(committed.workspace_id),
                None,
                None,
            )?;
        }
        Ok(snapshot.response)
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
        if client_kind == Some(ClientKind::Hook) {
            return self.workspace_bootstrap_activate(params);
        }
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
        let (result, expected_mode, expected_generation, invalidated_sessions) = {
            let mut state = self.lock_state()?;
            let (result, expected_mode, expected_generation) = {
                let workspace = state
                    .workspaces
                    .get_mut(&workspace_id)
                    .ok_or_else(|| project_mismatch("workspace is not registered"))?;
                let expected_mode = workspace.result.mode;
                let expected_generation = workspace.result.inventory_generation;
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
                (workspace.result.clone(), expected_mode, expected_generation)
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
            (result, expected_mode, expected_generation, invalidated)
        };
        let value = to_value(&result)?;
        let now = self.clock.now_unix_ms();
        let created_at = self
            .persistence
            .list_identity_snapshots(RuntimeIdentityKind::Workspace)?
            .into_iter()
            .map(|snapshot| snapshot.workspace)
            .find(|workspace| workspace.workspace_id == workspace_id)
            .map_or(now, |workspace| workspace.created_at_unix_ms);
        self.persistence
            .commit_identity_bundle(RuntimeIdentityTransition {
                operation: "workspace.mode.transition".to_owned(),
                scope_digest: canonical_digest(&json!({
                    "domain":"workspace.mode.transition/v1",
                    "workspaceId":workspace_id,
                }))?,
                idempotency_key: key.to_owned(),
                request_digest: digest.clone(),
                expected_workspace_mode: Some(expected_mode),
                expected_inventory_generation: Some(expected_generation),
                expected_session_status: None,
                expected_delegation_status: None,
                expected_context_generation: None,
                snapshot: RuntimeIdentitySnapshot {
                    identity_kind: RuntimeIdentityKind::Workspace,
                    workspace: RuntimeWorkspaceRecord {
                        workspace_id: result.workspace_id.clone(),
                        canonical_root: result.canonical_root.clone(),
                        project_key: result.project_key.clone(),
                        mode: result.mode,
                        inventory_generation: result.inventory_generation,
                        dirty: false,
                        created_at_unix_ms: created_at,
                        updated_at_unix_ms: now,
                    },
                    session: None,
                    delegation: None,
                    host_action: None,
                    attestation: None,
                    response: value.clone(),
                    replayed: false,
                },
                committed_at_unix_ms: now,
            })?;
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

    /// Enrolls an exact `/ae-sdd` Hook bootstrap without exposing the admin
    /// migration surface. This branch has one payload and one legal edge; it
    /// cannot select a target, waive parity for a later cutover, reverse mode,
    /// or invalidate already-bound sessions.
    fn workspace_bootstrap_activate(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        #[derive(Deserialize, serde::Serialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct BootstrapActivationPayload {
            bootstrap_activation: bool,
        }

        let payload: BootstrapActivationPayload = decode_value(params.payload.clone())?;
        if !payload.bootstrap_activation {
            return Err(schema_error("bootstrapActivation must be true"));
        }
        let workspace_id = require(&params.workspace_id, "workspaceId")?.to_owned();
        let key = require_idempotency(params)?;
        let request_digest = canonical_digest(&payload)?;
        let scope_digest = canonical_digest(&json!({
            "domain":"workspace.bootstrap.activate/v1",
            "workspaceId":workspace_id,
        }))?;
        let (result, expected_mode, expected_generation) = {
            let state = self.lock_state()?;
            let workspace = state
                .workspaces
                .get(&workspace_id)
                .ok_or_else(|| project_mismatch("workspace is not registered"))?;
            if workspace.result.mode != WorkspaceMode::Shadow {
                return Err(RuntimeError::new(
                    StableErrorCode::RoleOperationForbidden,
                    "bootstrap activation permits only the shadow to rust-canary edge",
                ));
            }
            let mut result = workspace.result.clone();
            result.mode = WorkspaceMode::RustCanary;
            result.inventory_generation = result
                .inventory_generation
                .checked_add(1)
                .ok_or_else(|| schema_error("inventory generation overflow"))?;
            (
                result,
                workspace.result.mode,
                workspace.result.inventory_generation,
            )
        };
        let now = self.clock.now_unix_ms();
        let created_at = self
            .persistence
            .list_identity_snapshots(RuntimeIdentityKind::Workspace)?
            .into_iter()
            .map(|snapshot| snapshot.workspace)
            .find(|workspace| workspace.workspace_id == workspace_id)
            .map_or(now, |workspace| workspace.created_at_unix_ms);
        let value = to_value(&result)?;
        let snapshot = self
            .persistence
            .commit_identity_bundle(RuntimeIdentityTransition {
                operation: "workspace.bootstrap.activate".to_owned(),
                scope_digest,
                idempotency_key: key.to_owned(),
                request_digest,
                expected_workspace_mode: Some(expected_mode),
                expected_inventory_generation: Some(expected_generation),
                expected_session_status: None,
                expected_delegation_status: None,
                expected_context_generation: None,
                snapshot: RuntimeIdentitySnapshot {
                    identity_kind: RuntimeIdentityKind::Workspace,
                    workspace: RuntimeWorkspaceRecord {
                        workspace_id: result.workspace_id.clone(),
                        canonical_root: result.canonical_root.clone(),
                        project_key: result.project_key.clone(),
                        mode: result.mode,
                        inventory_generation: result.inventory_generation,
                        dirty: false,
                        created_at_unix_ms: created_at,
                        updated_at_unix_ms: now,
                    },
                    session: None,
                    delegation: None,
                    host_action: None,
                    attestation: None,
                    response: value,
                    replayed: false,
                },
                committed_at_unix_ms: now,
            })?;
        let committed: WorkspaceResult = decode_value(snapshot.response.clone())?;
        {
            let mut state = self.lock_state()?;
            let workspace = state
                .workspaces
                .get_mut(&workspace_id)
                .ok_or_else(|| project_mismatch("workspace is not registered"))?;
            workspace.result = committed.clone();
        }
        if !snapshot.replayed {
            self.append_runtime_event(
                "workspace.bootstrap_activated",
                json!({"workspaceId":workspace_id}),
                Some(workspace_id),
                None,
                None,
            )?;
        }
        Ok(snapshot.response)
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
