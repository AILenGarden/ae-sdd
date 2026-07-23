use std::{path::Path, sync::Arc, time::Duration};

use ae_sdd_domain::{
    ErrorCode, FencingToken, InventoryGeneration, PolicyDigest, WorkItemId, WorkspaceId,
};
use ae_sdd_gates::{
    CancellationToken, GateInputError, GateRunRequest, GateScheduler, NativeGateExecutor,
};
use ae_sdd_protocol::StableErrorCode;
use ae_sdd_runtime::{BusinessWorkspace, RuntimeError, RuntimeResult};
use serde_json::Value;

mod contracts;
mod key;
mod outcome;
mod predicate;
mod scanner;

use key::GateContext;
pub use outcome::gate_result_json;
use scanner::{ProjectGateFreshness, ProjectGateSource};

/// Rust-native evaluator bound to one daemon-verified workspace and Work Item.
#[derive(Clone, Debug)]
pub struct AuthoritativeGateRuntime {
    context: Arc<GateContext>,
}

impl AuthoritativeGateRuntime {
    /// Creates a Gate evaluator. `expected_fencing_token` is checked when the snapshot is taken.
    pub fn new(
        workspace: &BusinessWorkspace,
        work_item_id: &str,
        policy_digest: &str,
        expected_fencing_token: Option<u64>,
    ) -> RuntimeResult<Self> {
        let root = Path::new(&workspace.canonical_root)
            .canonicalize()
            .map_err(|_| external("workspace root cannot be canonicalized"))?;
        if !root.is_dir() {
            return Err(external("workspace root is not a directory"));
        }
        Ok(Self {
            context: Arc::new(GateContext {
                root,
                workspace_id: workspace
                    .workspace_id
                    .parse::<WorkspaceId>()
                    .map_err(|_| schema("workspaceId is invalid"))?,
                work_item_id: WorkItemId::new(work_item_id.to_owned())
                    .map_err(|_| schema("workItemId is invalid"))?,
                policy: policy_digest
                    .parse::<PolicyDigest>()
                    .map_err(|_| schema("policy digest is invalid"))?,
                inventory: InventoryGeneration::new(workspace.inventory_generation),
                expected_fencing_token: expected_fencing_token.map(FencingToken::new),
            }),
        })
    }

    /// Builds the exact Gate snapshot used for evaluation.
    pub fn snapshot_key(&self, gate_id: &str) -> RuntimeResult<ae_sdd_domain::GateKey> {
        self.context.build_key(gate_id, true)
    }

    /// Rebuilds a key from current authoritative files without enforcing the old fencing token.
    pub fn current_key(&self, gate_id: &str) -> RuntimeResult<ae_sdd_domain::GateKey> {
        self.context.build_key(gate_id, false)
    }

    /// Evaluates any of the 36 registered Gates through `NativeGateExecutor` and `GateScheduler`.
    pub fn evaluate(
        &self,
        gate_id: &str,
        deadline: Duration,
    ) -> RuntimeResult<ae_sdd_domain::GateResult> {
        let key = self.snapshot_key(gate_id)?;
        let source = Arc::new(ProjectGateSource {
            context: Arc::clone(&self.context),
        });
        let scheduler = GateScheduler::new(
            NativeGateExecutor::new(source),
            ProjectGateFreshness {
                context: Arc::clone(&self.context),
            },
        );
        let request = GateRunRequest::new(key, deadline, CancellationToken::caller())
            .map_err(|_| schema("Gate deadline must be greater than zero"))?;
        Ok(scheduler.run(request))
    }

    /// Evaluates a Gate and returns the stable structured wire projection.
    pub fn evaluate_json(&self, gate_id: &str, deadline: Duration) -> RuntimeResult<Value> {
        self.evaluate(gate_id, deadline)
            .map(|result| gate_result_json(&result))
    }
}

pub(super) fn code(value: &str) -> ErrorCode {
    ErrorCode::new(value.to_owned()).expect("constant Gate error code is valid")
}

pub(super) fn input_error() -> GateInputError {
    GateInputError::new(code("GATE_INPUT_UNAVAILABLE"), true)
}

pub(super) fn schema(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::OperationSchemaInvalid, message)
}

pub(super) fn external(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::ExternalStateConflict, message)
}

#[cfg(test)]
mod tests;
