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
}
