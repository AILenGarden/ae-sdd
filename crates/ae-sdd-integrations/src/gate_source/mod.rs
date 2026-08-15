use std::{fmt, path::Path, sync::Arc, time::Duration};

use ae_sdd_domain::{
    ErrorCode, FencingToken, InventoryGeneration, PolicyDigest, WorkItemId, WorkspaceId,
};
use ae_sdd_gates::{
    CancellationToken, GateDag, GateInputError, GateInputSelector, GateRunRequest, GateScheduler,
    GateSchedulerStats, NativeGateExecutor,
};
use ae_sdd_protocol::StableErrorCode;
use ae_sdd_runtime::{BusinessWorkspace, RuntimeError, RuntimeResult};
use serde_json::Value;

mod contracts;
mod key;
mod outcome;
mod predicate;
pub(crate) mod ra_binding;
mod scanner;

use key::GateContext;
pub use key::ReviewGateAuthority;
pub use outcome::gate_result_json;
use scanner::{ProjectGateFreshness, ProjectGateSource};

/// Native scheduler wired to the project Gate inputs of one daemon-verified
/// workspace and Work Item.
type ProjectGateScheduler =
    GateScheduler<NativeGateExecutor<ProjectGateSource>, ProjectGateFreshness>;

/// Rust-native evaluator bound to one daemon-verified workspace and Work Item.
///
/// The scheduler is long-lived: it is created once per runtime instance so the
/// key cache and single-flight survive across `evaluate` calls. The instance
/// binds the workspace, Work Item, policy digest and inventory generation at
/// construction; a drift in any of them discards the runtime (and with it the
/// scheduler) instead of resetting it.
#[derive(Clone)]
pub struct AuthoritativeGateRuntime {
    context: Arc<GateContext>,
    scheduler: ProjectGateScheduler,
    dag: GateDag,
}

impl fmt::Debug for AuthoritativeGateRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthoritativeGateRuntime")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl AuthoritativeGateRuntime {
    /// Creates a lightweight Gate evaluator without Review authority
    /// dependencies. Every Review predicate fails closed under this
    /// constructor; use [`Self::with_review_authority`] on production paths.
    ///
    /// `expected_fencing_token` is checked when the snapshot is taken.
    pub fn new(
        workspace: &BusinessWorkspace,
        work_item_id: &str,
        policy_digest: &str,
        expected_fencing_token: Option<u64>,
    ) -> RuntimeResult<Self> {
        Self::build(
            workspace,
            work_item_id,
            policy_digest,
            expected_fencing_token,
            None,
        )
    }

    /// Creates the production Gate evaluator that can join durable Review
    /// authority: SQLite projection, reviewer lineage, and final proof.
    pub fn with_review_authority(
        workspace: &BusinessWorkspace,
        work_item_id: &str,
        policy_digest: &str,
        expected_fencing_token: Option<u64>,
        review: ReviewGateAuthority,
    ) -> RuntimeResult<Self> {
        if review.workspace.workspace_id != workspace.workspace_id
            || review.workspace.canonical_root != workspace.canonical_root
        {
            return Err(external(
                "Review Gate authority workspace differs from the evaluated workspace",
            ));
        }
        Self::build(
            workspace,
            work_item_id,
            policy_digest,
            expected_fencing_token,
            Some(review),
        )
    }

    fn build(
        workspace: &BusinessWorkspace,
        work_item_id: &str,
        policy_digest: &str,
        expected_fencing_token: Option<u64>,
        review: Option<ReviewGateAuthority>,
    ) -> RuntimeResult<Self> {
        let root = Path::new(&workspace.canonical_root)
            .canonicalize()
            .map_err(|_| external("workspace root cannot be canonicalized"))?;
        if !root.is_dir() {
            return Err(external("workspace root is not a directory"));
        }
        let context = Arc::new(GateContext {
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
            review,
        });
        // Fail closed at startup: Gates must never be evaluated from a
        // rejected dependency declaration.
        let dag = GateDag::from_registry()
            .map_err(|error| RuntimeError::new(StableErrorCode::GateError, error.to_string()))?;
        let scheduler = GateScheduler::new(
            NativeGateExecutor::new(Arc::new(ProjectGateSource {
                context: Arc::clone(&context),
            })),
            ProjectGateFreshness {
                context: Arc::clone(&context),
            },
        );
        Ok(Self {
            context,
            scheduler,
            dag,
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

    /// Evaluates any of the 36 registered Gates through the long-lived
    /// scheduler; an unchanged Gate key reuses the fresh cached outcome.
    pub fn evaluate(
        &self,
        gate_id: &str,
        deadline: Duration,
    ) -> RuntimeResult<ae_sdd_domain::GateResult> {
        let key = self.snapshot_key(gate_id)?;
        let request = GateRunRequest::new(key, deadline, CancellationToken::caller())
            .map_err(|_| schema("Gate deadline must be greater than zero"))?;
        Ok(self.scheduler.run(request))
    }

    /// Cumulative scheduler counters of this runtime.
    pub fn stats(&self) -> GateSchedulerStats {
        self.scheduler.stats()
    }

    /// Drops the cached outcomes of every Gate that depends on one of the
    /// `changed` selectors, directly or through prerequisite Gates, and
    /// returns the affected Gates in stable topological order so callers can
    /// re-evaluate them in dependency order. Gates without a selector
    /// declaration fail closed and are always invalidated.
    pub fn invalidate_selectors(&self, changed: &[GateInputSelector]) -> Vec<&'static str> {
        let affected = self.dag.affected(changed);
        self.scheduler.invalidate_gates(affected.iter().copied());
        affected
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
