use super::*;

impl RuntimeService {
    pub(super) fn handshake(&self, request: &HandshakeRequest) -> RuntimeResult<HandshakeResponse> {
        if !constant_time_equal(
            request.endpoint_token.expose_secret().as_bytes(),
            self.endpoint_token.as_bytes(),
        ) {
            return Err(RuntimeError::new(
                StableErrorCode::EndpointAuthFailed,
                "endpoint authentication failed",
            ));
        }
        if !supports_v1(&request.protocol_range) {
            return Err(RuntimeError::new(
                StableErrorCode::ProtocolVersionUnsupported,
                "client protocol range does not include daemon protocol v1",
            ));
        }
        if request.expected_boot_id != self.boot_id.to_string()
            || request.expected_policy_digest != self.config.policy_digest
        {
            return Err(RuntimeError::new(
                StableErrorCode::EndpointStale,
                "endpoint manifest boot or policy identity is stale",
            )
            .with_remediation("atomically reread the endpoint manifest and reconnect"));
        }
        let public_key = self.capability_signer.public_key();
        let capabilities = [
            "event-cursor-v1",
            "hook-fail-closed-v1",
            "physical-delegation-v1",
            "context-projection-v1",
            "compact-ack-rehydrate-v1",
            "bounded-job-scheduler-v1",
            "execution-supervisor-v1",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        Ok(HandshakeResponse {
            protocol_version: PROTOCOL_VERSION_V1.to_owned(),
            boot_id: self.boot_id.to_string(),
            event_store_id: self.persistence.event_store_id()?.to_string(),
            daemon_build: crate::RUNTIME_BUILD.to_owned(),
            capabilities,
            policy_digest: self.config.policy_digest.clone(),
            operation_schema_digest: self.config.operation_schema_digest.clone(),
            limits: HandshakeLimits {
                max_frame_bytes: self.config.max_frame_bytes as u64,
                max_agent_depth: 2,
                max_string_bytes: 1_048_576,
                max_collection_items: 16_384,
                max_deadline_ms: self.config.max_deadline_ms,
                hook_deadline_ms: self.config.hook_deadline_ms,
                max_child_result_bytes: self.config.child_result_max_bytes(),
                max_child_summary_bytes: self.config.child_summary_max_bytes(),
                max_context_projection_bytes: self.config.max_context_projection_bytes as u64,
            },
            capability_key_id: public_key.key_id().to_owned(),
            capability_public_key: public_key.public_key_hex(),
        })
    }

    pub(super) fn validate_request(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
    ) -> RuntimeResult<()> {
        if params.protocol_version != PROTOCOL_VERSION_V1 {
            return Err(RuntimeError::new(
                StableErrorCode::ProtocolVersionUnsupported,
                "post-handshake protocolVersion differs from the negotiated version",
            ));
        }
        if params.deadline_ms == 0 || params.deadline_ms > self.config.max_deadline_ms {
            return Err(RuntimeError::new(
                StableErrorCode::OperationSchemaInvalid,
                "deadline budget is zero or exceeds the negotiated maximum",
            ));
        }
        let requirements = method.spec().requirements;
        if requirements.requires_workspace && params.workspace_id.is_none() {
            return Err(schema_error("workspaceId is required"));
        }
        if requirements.requires_work_item && params.work_item_id.is_none() {
            return Err(schema_error("workItemId is required"));
        }
        if requirements.requires_idempotency && params.idempotency_key.is_none() {
            return Err(schema_error("idempotencyKey is required"));
        }
        if requirements.requires_confirmation && params.confirmation.is_none() {
            return Err(RuntimeError::new(
                StableErrorCode::ConfirmationRequired,
                "explicit user confirmation is required",
            ));
        }
        if is_hook(method) && params.deadline_ms > self.config.hook_deadline_ms {
            return Err(schema_error(
                "Hook deadline exceeds the negotiated fast-path budget",
            ));
        }
        if self.lifecycle()? != DaemonLifecycle::Running
            && !matches!(
                method,
                RpcMethod::RuntimeStatus
                    | RpcMethod::RuntimeDrain
                    | RpcMethod::WorkspaceModeTransition
                    | RpcMethod::WorkspaceSnapshot
                    | RpcMethod::EventsSubscribe
            )
        {
            return Err(RuntimeError::new(
                StableErrorCode::DaemonDraining,
                "daemon is draining and does not admit session, host, job, Hook, or business work",
            ));
        }
        Ok(())
    }
}
