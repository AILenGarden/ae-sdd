use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::OperationScope;

/// Number of RPC methods frozen by protocol v1.
pub const METHOD_COUNT: usize = 38;

/// Exact protocol-v1 RPC method identifier.
///
/// Unknown identifiers fail deserialization so an unsupported command cannot
/// silently acquire a default authorization profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[repr(u8)]
pub enum RpcMethod {
    /// Negotiate endpoint identity, version, capabilities, and limits.
    #[serde(rename = "runtime.handshake")]
    RuntimeHandshake,
    /// Read daemon health and drain state.
    #[serde(rename = "runtime.status")]
    RuntimeStatus,
    /// Enter the privileged daemon drain lifecycle.
    #[serde(rename = "runtime.drain")]
    RuntimeDrain,
    /// Register a canonical workspace root.
    #[serde(rename = "workspace.register")]
    WorkspaceRegister,
    /// Read a bounded workspace projection.
    #[serde(rename = "workspace.snapshot")]
    WorkspaceSnapshot,
    /// Perform a confirmed, drained workspace writer-mode transition.
    #[serde(rename = "workspace.mode_transition")]
    WorkspaceModeTransition,
    /// Open a trusted Agent session.
    #[serde(rename = "session.open")]
    SessionOpen,
    /// Renew the liveness of a trusted Agent session.
    #[serde(rename = "session.heartbeat")]
    SessionHeartbeat,
    /// Close a trusted Agent session.
    #[serde(rename = "session.close")]
    SessionClose,
    /// Process a host user-prompt event.
    #[serde(rename = "hook.user_prompt")]
    HookUserPrompt,
    /// Evaluate a host pre-tool event.
    #[serde(rename = "hook.pre_tool")]
    HookPreTool,
    /// Record a host post-tool event.
    #[serde(rename = "hook.post_tool")]
    HookPostTool,
    /// Evaluate a host stop event.
    #[serde(rename = "hook.stop")]
    HookStop,
    /// Read the current role-aware flow projection.
    #[serde(rename = "flow.snapshot")]
    FlowSnapshot,
    /// Compute the deterministic next flow action.
    #[serde(rename = "flow.next")]
    FlowNext,
    /// Create a scoped physical-session delegation intent.
    #[serde(rename = "delegation.create")]
    DelegationCreate,
    /// Read delegation lifecycle state.
    #[serde(rename = "delegation.status")]
    DelegationStatus,
    /// Accept a claimed delegation from a child session.
    #[serde(rename = "delegation.accept")]
    DelegationAccept,
    /// Child self-claim: the child itself presents the one-time claim (Plan §2).
    /// Semantically a sibling of `delegation.accept` routed to the same
    /// supervisor path; exists as its own wire method so the host-native (A2)
    /// path keeps a distinct entry point when an external host adapter (A1)
    /// reuses `delegation.accept`.
    #[serde(rename = "delegation.child_claim")]
    DelegationChildClaim,
    /// Submit a bounded child result.
    #[serde(rename = "delegation.report")]
    DelegationReport,
    /// Collect a validated child result projection.
    #[serde(rename = "delegation.collect")]
    DelegationCollect,
    /// Cancel a scoped delegation.
    #[serde(rename = "delegation.cancel")]
    DelegationCancel,
    /// Extend the deadline of a running delegation.
    #[serde(rename = "delegation.renew")]
    DelegationRenew,
    /// Register an authenticated host runtime adapter.
    #[serde(rename = "host.register")]
    HostRegister,
    /// Pull the next durable host action.
    #[serde(rename = "host.action_next")]
    HostActionNext,
    /// Acknowledge a correlated host action.
    #[serde(rename = "host.action_ack")]
    HostActionAck,
    /// Report authenticated session context pressure.
    #[serde(rename = "host.pressure_report")]
    HostPressureReport,
    /// Read a role-aware context projection.
    #[serde(rename = "context.get")]
    ContextGet,
    /// Materialize a full, delta, or unchanged context projection.
    #[serde(rename = "context.project")]
    ContextProject,
    /// Request a host-acknowledged compact cycle.
    #[serde(rename = "compact.request")]
    CompactRequest,
    /// Read a compact cycle without advancing it.
    #[serde(rename = "compact.status")]
    CompactStatus,
    /// Discover the typed operation registry.
    #[serde(rename = "operation.describe")]
    OperationDescribe,
    /// Execute one typed operation.
    #[serde(rename = "operation.execute")]
    OperationExecute,
    /// Evaluate a Gate against a freshness snapshot.
    #[serde(rename = "gate.evaluate")]
    GateEvaluate,
    /// Subscribe from a durable event cursor.
    #[serde(rename = "events.subscribe")]
    EventsSubscribe,
    /// Submit one authorized, bounded background job.
    #[serde(rename = "job.submit")]
    JobSubmit,
    /// Read an asynchronous job.
    #[serde(rename = "job.status")]
    JobStatus,
    /// Cancel an asynchronous job where legal.
    #[serde(rename = "job.cancel")]
    JobCancel,
}

impl RpcMethod {
    /// All protocol-v1 methods in stable registry order.
    pub const ALL: [Self; METHOD_COUNT] = [
        Self::RuntimeHandshake,
        Self::RuntimeStatus,
        Self::RuntimeDrain,
        Self::WorkspaceRegister,
        Self::WorkspaceSnapshot,
        Self::WorkspaceModeTransition,
        Self::SessionOpen,
        Self::SessionHeartbeat,
        Self::SessionClose,
        Self::HookUserPrompt,
        Self::HookPreTool,
        Self::HookPostTool,
        Self::HookStop,
        Self::FlowSnapshot,
        Self::FlowNext,
        Self::DelegationCreate,
        Self::DelegationStatus,
        Self::DelegationAccept,
        Self::DelegationChildClaim,
        Self::DelegationReport,
        Self::DelegationCollect,
        Self::DelegationCancel,
        Self::DelegationRenew,
        Self::HostRegister,
        Self::HostActionNext,
        Self::HostActionAck,
        Self::HostPressureReport,
        Self::ContextGet,
        Self::ContextProject,
        Self::CompactRequest,
        Self::CompactStatus,
        Self::OperationDescribe,
        Self::OperationExecute,
        Self::GateEvaluate,
        Self::EventsSubscribe,
        Self::JobSubmit,
        Self::JobStatus,
        Self::JobCancel,
    ];

    /// Returns the exact lowercase `<domain>.<verb>` wire identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeHandshake => "runtime.handshake",
            Self::RuntimeStatus => "runtime.status",
            Self::RuntimeDrain => "runtime.drain",
            Self::WorkspaceRegister => "workspace.register",
            Self::WorkspaceSnapshot => "workspace.snapshot",
            Self::WorkspaceModeTransition => "workspace.mode_transition",
            Self::SessionOpen => "session.open",
            Self::SessionHeartbeat => "session.heartbeat",
            Self::SessionClose => "session.close",
            Self::HookUserPrompt => "hook.user_prompt",
            Self::HookPreTool => "hook.pre_tool",
            Self::HookPostTool => "hook.post_tool",
            Self::HookStop => "hook.stop",
            Self::FlowSnapshot => "flow.snapshot",
            Self::FlowNext => "flow.next",
            Self::DelegationCreate => "delegation.create",
            Self::DelegationStatus => "delegation.status",
            Self::DelegationAccept => "delegation.accept",
            Self::DelegationChildClaim => "delegation.child_claim",
            Self::DelegationReport => "delegation.report",
            Self::DelegationCollect => "delegation.collect",
            Self::DelegationCancel => "delegation.cancel",
            Self::DelegationRenew => "delegation.renew",
            Self::HostRegister => "host.register",
            Self::HostActionNext => "host.action_next",
            Self::HostActionAck => "host.action_ack",
            Self::HostPressureReport => "host.pressure_report",
            Self::ContextGet => "context.get",
            Self::ContextProject => "context.project",
            Self::CompactRequest => "compact.request",
            Self::CompactStatus => "compact.status",
            Self::OperationDescribe => "operation.describe",
            Self::OperationExecute => "operation.execute",
            Self::GateEvaluate => "gate.evaluate",
            Self::EventsSubscribe => "events.subscribe",
            Self::JobSubmit => "job.submit",
            Self::JobStatus => "job.status",
            Self::JobCancel => "job.cancel",
        }
    }

    /// Returns the immutable v1 authorization and precondition descriptor.
    #[must_use]
    pub const fn spec(self) -> &'static MethodSpec {
        &METHOD_REGISTRY[self as usize]
    }
}

impl fmt::Display for RpcMethod {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for RpcMethod {
    type Err = UnknownRpcMethod;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|method| method.as_str() == value)
            .ok_or_else(|| UnknownRpcMethod(value.to_owned()))
    }
}

/// Error returned when an identifier is not present in the v1 method registry.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("RPC method is not registered: {0}")]
pub struct UnknownRpcMethod(String);

/// Source of mutation preconditions for an RPC method.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementSource {
    /// The immutable flags on the method descriptor are authoritative.
    Method,
    /// `operation.execute` must consult the selected typed-operation descriptor.
    TypedOperation,
}

/// Frozen precondition flags for a protocol-v1 RPC method.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodRequirements {
    /// Whether a registered workspace identity must be supplied.
    pub requires_workspace: bool,
    /// Whether an explicit Work Item must be supplied.
    pub requires_work_item: bool,
    /// Whether the method mutates authoritative or durable runtime state.
    pub writes: bool,
    /// Whether a Work Item writer lease and fencing token are required.
    pub requires_lease: bool,
    /// Whether an expected project-state revision is required.
    pub requires_revision: bool,
    /// Whether a semantic idempotency key is required.
    pub requires_idempotency: bool,
    /// Whether an auditable user confirmation is required.
    pub requires_confirmation: bool,
    /// Whether these flags or the selected typed operation are authoritative.
    pub source: RequirementSource,
}

impl MethodRequirements {
    const fn direct(
        requires_workspace: bool,
        requires_work_item: bool,
        writes: bool,
        requires_idempotency: bool,
        requires_confirmation: bool,
    ) -> Self {
        Self {
            requires_workspace,
            requires_work_item,
            writes,
            requires_lease: false,
            requires_revision: false,
            requires_idempotency,
            requires_confirmation,
            source: RequirementSource::Method,
        }
    }

    const fn typed_operation() -> Self {
        Self {
            requires_workspace: true,
            requires_work_item: true,
            writes: false,
            requires_lease: false,
            requires_revision: false,
            requires_idempotency: false,
            requires_confirmation: false,
            source: RequirementSource::TypedOperation,
        }
    }
}

/// Immutable method ownership and precondition descriptor.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodSpec {
    /// Exact method identifier.
    pub method: RpcMethod,
    /// Trusted identity and authorization scope.
    pub scope: OperationScope,
    /// Frozen precondition behavior.
    #[serde(flatten)]
    pub requirements: MethodRequirements,
}

const fn method(
    method: RpcMethod,
    scope: OperationScope,
    requires_workspace: bool,
    requires_work_item: bool,
    writes: bool,
    requires_idempotency: bool,
    requires_confirmation: bool,
) -> MethodSpec {
    MethodSpec {
        method,
        scope,
        requirements: MethodRequirements::direct(
            requires_workspace,
            requires_work_item,
            writes,
            requires_idempotency,
            requires_confirmation,
        ),
    }
}

/// Protocol-v1 method registry in the same stable order as [`RpcMethod::ALL`].
///
/// `operation.execute` is intentionally marked `TypedOperation`: its selected
/// operation descriptor is the single source for lease, revision,
/// idempotency, confirmation, and write flags.
pub const METHOD_REGISTRY: [MethodSpec; METHOD_COUNT] = [
    method(
        RpcMethod::RuntimeHandshake,
        OperationScope::Runtime,
        false,
        false,
        false,
        false,
        false,
    ),
    method(
        RpcMethod::RuntimeStatus,
        OperationScope::Runtime,
        false,
        false,
        false,
        false,
        false,
    ),
    method(
        RpcMethod::RuntimeDrain,
        OperationScope::Runtime,
        false,
        false,
        true,
        true,
        true,
    ),
    method(
        RpcMethod::WorkspaceRegister,
        OperationScope::Workspace,
        false,
        false,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::WorkspaceSnapshot,
        OperationScope::Workspace,
        true,
        false,
        false,
        false,
        false,
    ),
    method(
        RpcMethod::WorkspaceModeTransition,
        OperationScope::Workspace,
        true,
        false,
        true,
        true,
        true,
    ),
    method(
        RpcMethod::SessionOpen,
        OperationScope::Session,
        true,
        false,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::SessionHeartbeat,
        OperationScope::Session,
        true,
        false,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::SessionClose,
        OperationScope::Session,
        true,
        false,
        true,
        true,
        false,
    ),
    // The four Hooks are Session-scoped: their trusted identity is the session
    // capability, not a Work Item. Requiring `workItemId` locked the very
    // bootstrap turn that is supposed to establish routing, so the first host
    // event could never reach the daemon. The Work Item stays optional
    // attribution resolved from the session binding when one exists.
    method(
        RpcMethod::HookUserPrompt,
        OperationScope::Session,
        true,
        false,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::HookPreTool,
        OperationScope::Session,
        true,
        false,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::HookPostTool,
        OperationScope::Session,
        true,
        false,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::HookStop,
        OperationScope::Session,
        true,
        false,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::FlowSnapshot,
        OperationScope::WorkItem,
        true,
        true,
        false,
        false,
        false,
    ),
    method(
        RpcMethod::FlowNext,
        OperationScope::WorkItem,
        true,
        true,
        false,
        false,
        false,
    ),
    method(
        RpcMethod::DelegationCreate,
        OperationScope::Delegation,
        true,
        true,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::DelegationStatus,
        OperationScope::Delegation,
        true,
        true,
        false,
        false,
        false,
    ),
    method(
        RpcMethod::DelegationAccept,
        OperationScope::Delegation,
        true,
        true,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::DelegationChildClaim,
        OperationScope::Delegation,
        true,
        true,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::DelegationReport,
        OperationScope::Delegation,
        true,
        true,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::DelegationCollect,
        OperationScope::Delegation,
        true,
        true,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::DelegationCancel,
        OperationScope::Delegation,
        true,
        true,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::DelegationRenew,
        OperationScope::Delegation,
        true,
        true,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::HostRegister,
        OperationScope::Host,
        false,
        false,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::HostActionNext,
        OperationScope::Host,
        false,
        false,
        false,
        false,
        false,
    ),
    method(
        RpcMethod::HostActionAck,
        OperationScope::Host,
        false,
        false,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::HostPressureReport,
        OperationScope::Host,
        true,
        false,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::ContextGet,
        OperationScope::Session,
        true,
        true,
        false,
        false,
        false,
    ),
    method(
        RpcMethod::ContextProject,
        OperationScope::Session,
        true,
        true,
        false,
        false,
        false,
    ),
    method(
        RpcMethod::CompactRequest,
        OperationScope::Session,
        true,
        true,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::CompactStatus,
        OperationScope::Session,
        true,
        true,
        false,
        false,
        false,
    ),
    method(
        RpcMethod::OperationDescribe,
        OperationScope::Runtime,
        false,
        false,
        false,
        false,
        false,
    ),
    MethodSpec {
        method: RpcMethod::OperationExecute,
        scope: OperationScope::WorkItem,
        requirements: MethodRequirements::typed_operation(),
    },
    method(
        RpcMethod::GateEvaluate,
        OperationScope::WorkItem,
        true,
        true,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::EventsSubscribe,
        OperationScope::Workspace,
        true,
        false,
        false,
        false,
        false,
    ),
    method(
        RpcMethod::JobSubmit,
        OperationScope::Workspace,
        true,
        false,
        true,
        true,
        false,
    ),
    method(
        RpcMethod::JobStatus,
        OperationScope::Workspace,
        true,
        false,
        false,
        false,
        false,
    ),
    method(
        RpcMethod::JobCancel,
        OperationScope::Workspace,
        true,
        false,
        true,
        true,
        false,
    ),
];

#[cfg(test)]
mod tests {
    use std::hint::black_box;

    use super::*;

    #[test]
    fn private_registry_builders_execute_at_runtime() {
        let direct: fn(bool, bool, bool, bool, bool) -> MethodRequirements =
            black_box(MethodRequirements::direct);
        let direct_requirements = direct(
            black_box(true),
            black_box(false),
            black_box(true),
            black_box(true),
            black_box(false),
        );
        assert_eq!(
            direct_requirements,
            MethodRequirements {
                requires_workspace: true,
                requires_work_item: false,
                writes: true,
                requires_lease: false,
                requires_revision: false,
                requires_idempotency: true,
                requires_confirmation: false,
                source: RequirementSource::Method,
            }
        );

        let typed_operation: fn() -> MethodRequirements =
            black_box(MethodRequirements::typed_operation);
        assert_eq!(
            typed_operation(),
            RpcMethod::OperationExecute.spec().requirements
        );

        let build_method: fn(
            RpcMethod,
            OperationScope,
            bool,
            bool,
            bool,
            bool,
            bool,
        ) -> MethodSpec = black_box(method);
        assert_eq!(
            build_method(
                black_box(RpcMethod::RuntimeDrain),
                black_box(OperationScope::Runtime),
                black_box(false),
                black_box(false),
                black_box(true),
                black_box(true),
                black_box(true),
            ),
            *RpcMethod::RuntimeDrain.spec()
        );
    }
}
