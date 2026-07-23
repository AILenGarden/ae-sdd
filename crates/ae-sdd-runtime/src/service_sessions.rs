use super::*;

struct CapabilitySignInput<'a> {
    workspace_id: &'a str,
    session_id: &'a str,
    role: WireAgentRole,
    delegation_id: Option<&'a str>,
    grant: &'a ScopedGrantWire,
    engaged: bool,
    issued_at: u64,
    expires_at: u64,
}

impl RuntimeService {
    pub(super) fn session_open(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let workspace_id = require(&params.workspace_id, "workspaceId")?.to_owned();
        let agent_id = require(&params.agent_id, "agentId")?.to_owned();
        let payload: SessionOpenPayload = decode_value(params.payload.clone())?;
        if payload.context.is_some() {
            return Err(RuntimeError::new(
                StableErrorCode::RoleOperationForbidden,
                "caller-supplied session context is not authoritative",
            ));
        }
        let key = require_idempotency(params)?;
        let scope = format!(
            "session-open\0{}\0{workspace_id}\0{}",
            self.boot_id, payload.external_key
        );
        let digest = canonical_digest(&payload)?;
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let now = self.clock.now_unix_ms();
        let expires = now.saturating_add(self.config.session_ttl_ms);

        let (result, previous_record) = {
            let mut state = self.lock_state()?;
            let mode = state
                .workspaces
                .get(&workspace_id)
                .ok_or_else(|| project_mismatch("workspace is not registered"))?
                .result
                .mode;
            let engaged = matches!(
                mode,
                WorkspaceMode::RustCanary | WorkspaceMode::RustSoleWriter
            );
            if payload.engaged != engaged {
                return Err(RuntimeError::new(
                    StableErrorCode::RoleOperationForbidden,
                    "caller engaged mode differs from the daemon workspace policy",
                ));
            }
            if let Some(session_id) = state
                .session_by_external
                .get(&(workspace_id.clone(), payload.external_key.clone()))
                .cloned()
            {
                let previous = state
                    .sessions
                    .get(&session_id)
                    .cloned()
                    .expect("external session index is internally consistent");
                let authoritative_grant = self.validate_delegated_open(
                    previous.result.role,
                    previous.delegation_id.as_deref(),
                    &session_id,
                )?;
                if previous.result.role != WireAgentRole::Root
                    && previous.grant.normalized()? != authoritative_grant
                {
                    return Err(RuntimeError::new(
                        StableErrorCode::DelegationAttestationFailed,
                        "durable child session grant differs from its delegation",
                    ));
                }
                let session = state
                    .sessions
                    .get_mut(&session_id)
                    .expect("external session index is internally consistent");
                if session.agent_id != agent_id || session.result.role != payload.role {
                    return Err(turn_mismatch(
                        "external session identity is bound to another agent or role",
                    ));
                }
                session.active = true;
                session.grant = authoritative_grant;
                session.result.engaged = engaged;
                if params.work_item_id.is_some() {
                    session.current_work_item = params.work_item_id.clone();
                }
                session.result.expires_at_unix_ms = expires;
                session.result.capability_token = self.sign_capability(CapabilitySignInput {
                    workspace_id: &workspace_id,
                    session_id: &session_id,
                    role: session.result.role,
                    delegation_id: session.delegation_id.as_deref(),
                    grant: &session.grant,
                    engaged,
                    issued_at: now,
                    expires_at: expires,
                })?;
                (session.result.clone(), Some(previous))
            } else {
                if state.sessions.values().filter(|item| item.active).count()
                    >= self.config.max_sessions
                {
                    return Err(RuntimeError::new(
                        StableErrorCode::SubscriberBackpressure,
                        "active session capacity is exhausted",
                    ));
                }
                let session_id = match payload.role {
                    WireAgentRole::Root => {
                        if params.session_id.is_some() {
                            return Err(RuntimeError::new(
                                StableErrorCode::RoleOperationForbidden,
                                "root session identity is generated by the daemon",
                            ));
                        }
                        Uuid::new_v4().to_string()
                    }
                    _ => params.session_id.clone().ok_or_else(|| {
                        RuntimeError::new(
                            StableErrorCode::DelegationAttestationFailed,
                            "child session requires its attested sessionId",
                        )
                    })?,
                };
                SessionId::from_str(&session_id)
                    .map_err(|_| schema_error("sessionId is not a UUID"))?;
                if state.sessions.contains_key(&session_id) {
                    return Err(RuntimeError::new(
                        StableErrorCode::TurnIdentityMismatch,
                        "sessionId is already bound to another physical session",
                    ));
                }
                let grant = self.validate_delegated_open(
                    payload.role,
                    payload.delegation_id.as_deref(),
                    &session_id,
                )?;
                let token = self.sign_capability(CapabilitySignInput {
                    workspace_id: &workspace_id,
                    session_id: &session_id,
                    role: payload.role,
                    delegation_id: payload.delegation_id.as_deref(),
                    grant: &grant,
                    engaged,
                    issued_at: now,
                    expires_at: expires,
                })?;
                let result = SessionResult {
                    session_id: session_id.clone(),
                    role: payload.role,
                    engaged,
                    expires_at_unix_ms: expires,
                    context_generation: 0,
                    capability_token: token,
                };
                let record = SessionRecord {
                    workspace_id: workspace_id.clone(),
                    agent_id,
                    external_key: payload.external_key.clone(),
                    current_work_item: params.work_item_id.clone(),
                    result: result.clone(),
                    delegation_id: payload.delegation_id,
                    grant,
                    current_turn_id: None,
                    current_turn_seq: 0,
                    active: true,
                };
                state.session_by_external.insert(
                    (workspace_id.clone(), payload.external_key.clone()),
                    session_id.clone(),
                );
                state.sessions.insert(session_id, record);
                (result, None)
            }
        };
        let value = to_value(&result)?;
        if let Some(work_item_id) = params.work_item_id.as_deref() {
            let workspace = {
                let state = self.lock_state()?;
                let record = state
                    .workspaces
                    .get(&workspace_id)
                    .ok_or_else(|| project_mismatch("workspace is not registered"))?;
                let grant = state
                    .sessions
                    .get(&result.session_id)
                    .ok_or_else(session_expired)?
                    .grant
                    .to_domain()?;
                BusinessWorkspace {
                    workspace_id: record.result.workspace_id.clone(),
                    canonical_root: record.result.canonical_root.clone(),
                    project_key: record.result.project_key.clone(),
                    mode: record.result.mode,
                    agent_role: Some(AgentRole::from(result.role)),
                    agent_grant: Some(grant),
                    caller_kind: None,
                    inventory_generation: record.result.inventory_generation,
                }
            };
            let projection_result = self.business.project_context(
                &workspace,
                work_item_id,
                &result.session_id,
                AgentRole::from(result.role),
            );
            let projection = match projection_result {
                Ok(projection) => projection,
                Err(error) => {
                    self.rollback_open(
                        &workspace_id,
                        &payload.external_key,
                        &result.session_id,
                        previous_record.as_ref(),
                    )?;
                    return Err(error);
                }
            };
            if let Err(error) = self.context.put(projection) {
                self.rollback_open(
                    &workspace_id,
                    &payload.external_key,
                    &result.session_id,
                    previous_record.as_ref(),
                )?;
                return Err(error);
            }
        }
        self.persist_session(&result.session_id)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "session.opened",
            Some(workspace_id),
            Some(result.session_id),
            None,
        )
        .map(|(value, _)| value)
    }

    fn rollback_open(
        &self,
        workspace_id: &str,
        external_key: &str,
        session_id: &str,
        previous: Option<&SessionRecord>,
    ) -> RuntimeResult<()> {
        let mut state = self.lock_state()?;
        if let Some(previous) = previous {
            state
                .sessions
                .insert(session_id.to_owned(), previous.clone());
        } else {
            state.sessions.remove(session_id);
            state
                .session_by_external
                .remove(&(workspace_id.to_owned(), external_key.to_owned()));
        }
        Ok(())
    }

    pub(super) fn session_heartbeat(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let key = require_idempotency(params)?;
        let digest = canonical_digest(&params.payload)?;
        let scope = format!("session-heartbeat\0{}", identity.session_id);
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let now = self.clock.now_unix_ms();
        let expires = now.saturating_add(self.config.session_ttl_ms);
        let value = {
            let mut state = self.lock_state()?;
            let session = state
                .sessions
                .get_mut(&identity.session_id)
                .ok_or_else(session_expired)?;
            session.result.expires_at_unix_ms = expires;
            session.result.capability_token = self.sign_capability(CapabilitySignInput {
                workspace_id: &identity.workspace_id,
                session_id: &identity.session_id,
                role: session.result.role,
                delegation_id: session.delegation_id.as_deref(),
                grant: &session.grant,
                engaged: session.result.engaged,
                issued_at: now,
                expires_at: expires,
            })?;
            to_value(&session.result)?
        };
        self.persist_session(&identity.session_id)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "session.heartbeat",
            Some(identity.workspace_id),
            Some(identity.session_id),
            None,
        )
        .map(|(value, _)| value)
    }

    pub(super) fn session_close(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let key = require_idempotency(params)?;
        let digest = canonical_digest(&params.payload)?;
        let scope = format!("session-close\0{}", identity.session_id);
        if let Some((value, _)) = self.replay_receipt(&scope, key, &digest)? {
            return Ok(value);
        }
        let value = {
            let mut state = self.lock_state()?;
            let session = state
                .sessions
                .get_mut(&identity.session_id)
                .ok_or_else(session_expired)?;
            session.active = false;
            to_value(&session.result)?
        };
        self.persist_session(&identity.session_id)?;
        self.commit_receipt_event(
            &scope,
            key,
            digest,
            value,
            "session.closed",
            Some(identity.workspace_id),
            Some(identity.session_id),
            None,
        )
        .map(|(value, _)| value)
    }

    pub(super) fn persist_session(&self, session_id: &str) -> RuntimeResult<()> {
        let value = {
            let state = self.lock_state()?;
            to_value(state.sessions.get(session_id).ok_or_else(session_expired)?)?
        };
        self.persistence
            .store_record("session/v1", session_id, &value)
    }

    pub(super) fn validate_delegated_open(
        &self,
        role: WireAgentRole,
        delegation_id: Option<&str>,
        session_id: &str,
    ) -> RuntimeResult<ScopedGrantWire> {
        match role {
            WireAgentRole::Root if delegation_id.is_none() => Ok(crate::grant::root_grant()),
            WireAgentRole::Root => Err(RuntimeError::new(
                StableErrorCode::DelegationAttestationFailed,
                "root session cannot carry a delegation",
            )),
            _ => {
                let delegation_id = delegation_id.ok_or_else(|| {
                    RuntimeError::new(
                        StableErrorCode::DelegationAttestationFailed,
                        "child session requires a physical delegation",
                    )
                })?;
                let projection = self.delegation.status(session_id, delegation_id)?;
                if projection.status != "running"
                    || projection.child_session_id.as_deref() != Some(session_id)
                    || projection.child_role != role
                {
                    return Err(RuntimeError::new(
                        StableErrorCode::DelegationAttestationFailed,
                        "child session does not match the attested delegation",
                    ));
                }
                let grant = projection.grant.normalized()?;
                crate::grant::validate_session_grant(role, &grant)?;
                Ok(grant)
            }
        }
    }

    fn sign_capability(&self, input: CapabilitySignInput<'_>) -> RuntimeResult<String> {
        let capability_id = if input.engaged {
            "hook.engaged"
        } else {
            "hook.unengaged"
        };
        let claims = CapabilityClaims::new(
            self.capability_signer.key_id(),
            self.boot_id,
            CapabilityId::new(capability_id).expect("static capability ID is valid"),
            SessionId::from_str(input.session_id)
                .map_err(|_| schema_error("invalid session ID"))?,
            input.role.into(),
            input
                .delegation_id
                .map(DelegationId::from_str)
                .transpose()
                .map_err(|_| schema_error("invalid delegation ID"))?,
            capability_grant_digest(
                input.workspace_id,
                input.session_id,
                input.role,
                input.delegation_id,
                input.grant,
                &self.config.policy_digest,
                capability_id,
            )?,
            input.issued_at,
            input.expires_at,
        )
        .map_err(|_| schema_error("session capability claims are invalid"))?;
        self.capability_signer
            .sign(claims)
            .and_then(|token| token.encode())
            .map_err(|_| {
                RuntimeError::new(
                    StableErrorCode::ExternalStateConflict,
                    "session capability could not be signed",
                )
            })
    }

    pub(super) fn session_identity(
        &self,
        params: &RequestParams<Value>,
        require_turn: bool,
    ) -> RuntimeResult<TrustedSession> {
        let workspace_id = require(&params.workspace_id, "workspaceId")?;
        let session_id = require(&params.session_id, "sessionId")?;
        let agent_id = require(&params.agent_id, "agentId")?;
        if require_turn && params.turn_id.is_none() {
            return Err(schema_error("turnId is required for engaged Hook requests"));
        }
        let state = self.lock_state()?;

        let session = state.sessions.get(session_id).ok_or_else(session_expired)?;
        if !session.active || self.clock.now_unix_ms() >= session.result.expires_at_unix_ms {
            return Err(session_expired());
        }
        if session.workspace_id != workspace_id || session.agent_id != agent_id {
            return Err(turn_mismatch(
                "session does not belong to the supplied workspace/agent identity",
            ));
        }
        let encoded = params.capability_token.as_deref().ok_or_else(|| {
            RuntimeError::new(
                StableErrorCode::SessionExpired,
                "boot-scoped session capability proof is required",
            )
        })?;
        let token = CapabilityToken::decode(encoded).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::SessionExpired,
                "session capability proof is malformed",
            )
        })?;
        let claims = self
            .capability_signer
            .public_key()
            .verify(&token, self.clock.now_unix_ms())
            .map_err(|_| {
                RuntimeError::new(
                    StableErrorCode::SessionExpired,
                    "session capability proof is forged, stale, or expired",
                )
            })?;
        let grant = crate::grant::validate_session_grant(session.result.role, &session.grant)?;
        let expected_grant_digest = capability_grant_digest(
            workspace_id,
            session_id,
            session.result.role,
            session.delegation_id.as_deref(),
            &session.grant,
            &self.config.policy_digest,
            claims.capability_id().as_str(),
        )?;
        if claims.session_id().to_string() != session_id
            || AgentRole::from(session.result.role) != claims.role()
            || claims.delegation_id().map(|value| value.to_string()) != session.delegation_id
            || claims.capability_id().as_str()
                != if session.result.engaged {
                    "hook.engaged"
                } else {
                    "hook.unengaged"
                }
            || claims.grant_digest() != expected_grant_digest
        {
            return Err(RuntimeError::new(
                StableErrorCode::TurnIdentityMismatch,
                "session capability proof does not bind this session role/delegation",
            ));
        }
        Ok(TrustedSession {
            workspace_id: workspace_id.to_owned(),
            session_id: session_id.to_owned(),
            role: session.result.role,
            grant,
            engaged: session.result.engaged,
            capability_id: claims.capability_id().as_str().to_owned(),
        })
    }

    pub(super) fn validate_turn(
        &self,
        session_id: &str,
        turn_id: &str,
        turn_seq: u64,
    ) -> RuntimeResult<()> {
        if turn_seq == 0 {
            return Err(turn_mismatch("turn sequence must be greater than zero"));
        }
        let mut state = self.lock_state()?;
        let session = state
            .sessions
            .get_mut(session_id)
            .ok_or_else(session_expired)?;
        match session.current_turn_id.as_deref() {
            None if turn_seq == 1 => {
                session.current_turn_id = Some(turn_id.to_owned());
                session.current_turn_seq = 1;
                Ok(())
            }
            Some(current) if current == turn_id && turn_seq == session.current_turn_seq => Ok(()),
            Some(_) if turn_seq == session.current_turn_seq.saturating_add(1) => {
                session.current_turn_id = Some(turn_id.to_owned());
                session.current_turn_seq = turn_seq;
                Ok(())
            }
            _ => Err(turn_mismatch(
                "turn identity or sequence does not match the trusted session",
            )),
        }
    }
}

fn capability_grant_digest(
    workspace_id: &str,
    session_id: &str,
    role: WireAgentRole,
    delegation_id: Option<&str>,
    grant: &ScopedGrantWire,
    policy_digest: &str,
    capability_id: &str,
) -> RuntimeResult<GrantDigest> {
    let role = match role {
        WireAgentRole::Root => "root",
        WireAgentRole::Series => "series",
        WireAgentRole::Task => "task",
        WireAgentRole::Reviewer => "reviewer",
    };
    let mut material = format!(
        "{workspace_id}\0{session_id}\0{role}\0{}\0{policy_digest}\0{capability_id}",
        delegation_id.unwrap_or("")
    )
    .into_bytes();
    material.push(0);
    let normalized = grant.normalized()?;
    material.extend(serde_json::to_vec(&normalized).map_err(canonical_error)?);
    Ok(GrantDigest::digest(material))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrantPathWire;

    #[test]
    fn capability_digest_binds_the_actual_operation_and_path_grant() {
        let base = ScopedGrantWire {
            operations: vec!["document.resolve".to_owned()],
            capabilities: Vec::new(),
            paths: vec![GrantPathWire::Subtree {
                path: "ae-sdd-doc/Story".to_owned(),
            }],
        };
        let wider_operation = ScopedGrantWire {
            operations: vec!["document.resolve".to_owned(), "document.save".to_owned()],
            ..base.clone()
        };
        let wider_path = ScopedGrantWire {
            paths: vec![GrantPathWire::ProjectRoot],
            ..base.clone()
        };
        let digest = capability_grant_digest(
            "workspace",
            "session",
            WireAgentRole::Task,
            Some("delegation"),
            &base,
            "policy",
            "hook.engaged",
        )
        .expect("digest");
        assert_ne!(
            digest,
            capability_grant_digest(
                "workspace",
                "session",
                WireAgentRole::Task,
                Some("delegation"),
                &wider_operation,
                "policy",
                "hook.engaged",
            )
            .expect("digest")
        );
        assert_ne!(
            digest,
            capability_grant_digest(
                "workspace",
                "session",
                WireAgentRole::Task,
                Some("delegation"),
                &wider_path,
                "policy",
                "hook.engaged",
            )
            .expect("digest")
        );
    }
}
