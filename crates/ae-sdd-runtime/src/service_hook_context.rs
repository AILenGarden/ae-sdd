use super::*;

impl RuntimeService {
    pub(super) fn hook(
        &self,
        method: RpcMethod,
        params: &RequestParams<Value>,
    ) -> RuntimeResult<Value> {
        let started = Instant::now();
        let identity = self.session_identity(params, true)?;
        let turn_id = require(&params.turn_id, "turnId")?.to_owned();
        let work_item_id = require(&params.work_item_id, "workItemId")?.to_owned();
        let payload: HookPayload = decode_value(params.payload.clone())?;
        if serde_json::to_vec(&payload).map_err(canonical_error)?.len() > 65_536 {
            return Err(schema_error(
                "Hook payload exceeds the bounded event budget",
            ));
        }
        self.validate_turn(&identity.session_id, &turn_id, payload.turn_seq)?;
        let scope = format!(
            "hook\0{}\0{}\0{}\0{}",
            identity.workspace_id, identity.session_id, turn_id, payload.hook_event_id
        );
        let digest = canonical_digest(&(method.as_str(), &payload))?;
        if let Some((mut value, event_seq)) =
            self.replay_receipt(&scope, &payload.hook_event_id, &digest)?
        {
            if let Some(object) = value.as_object_mut() {
                object.insert("replayed".to_owned(), Value::Bool(true));
                object.insert("eventSeq".to_owned(), Value::from(event_seq));
            }
            return Ok(value);
        }

        let context = self.context.hook_projection(&identity.session_id)?;
        let inventory_generation = self
            .lock_state()?
            .workspaces
            .get(&identity.workspace_id)
            .ok_or_else(|| project_mismatch("workspace is not registered"))?
            .result
            .inventory_generation;
        let decision = hook_decision(
            method,
            identity.engaged,
            context.as_ref(),
            &self.config.policy_digest,
            inventory_generation,
        );
        let base = HookResult {
            engaged: identity.engaged,
            decision,
            context: (decision == HookDecision::Context)
                .then_some(context)
                .flatten(),
            event_seq: 0,
            replayed: false,
        };
        let value = to_value(base)?;
        let (mut value, event_seq) = self.actors.execute(
            &identity.workspace_id,
            &work_item_id,
            params.deadline_ms,
            || {
                self.commit_receipt_event(
                    &scope,
                    &payload.hook_event_id,
                    digest,
                    value,
                    &format!("hook.{}", method.as_str().replace('.', "_")),
                    Some(identity.workspace_id.clone()),
                    Some(identity.session_id.clone()),
                    Some(work_item_id.clone()),
                )
            },
        )?;
        if let Some(object) = value.as_object_mut() {
            object.insert("eventSeq".to_owned(), Value::from(event_seq));
        }
        if started.elapsed().as_millis() > u128::from(params.deadline_ms) {
            return Err(RuntimeError::new(
                StableErrorCode::GateTimeout,
                "Hook fast path exceeded the caller deadline",
            ));
        }
        Ok(value)
    }

    pub(super) fn context_get(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let result = self.context.project(&identity.session_id, 0, "")?;
        to_value(result)
    }

    pub(super) fn context_project(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let identity = self.session_identity(params, false)?;
        let request: ContextProjectPayload = decode_value(params.payload.clone())?;
        let result = self.context.project(
            &identity.session_id,
            request.known_revision,
            &request.known_digest,
        )?;
        if request.known_revision == result.context_revision
            && !request.known_digest.is_empty()
            && request.known_digest == result.digest
        {
            self.complete_compact_after_rehydrate(&identity.session_id, &result.digest)?;
        }
        to_value(result)
    }
}
