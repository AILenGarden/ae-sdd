use serde::{Deserialize, Serialize};

/// Kind of client participating in handshake capability negotiation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    /// Interactive or scripted command-line client.
    Cli,
    /// Latency-sensitive Agent hook adapter.
    Hook,
    /// Privileged local administration client.
    Admin,
    /// Authenticated integration with a host Agent runtime.
    HostAdapter,
}

/// Authorization and identity scope of a registered RPC method.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationScope {
    /// Per-daemon process scope.
    Runtime,
    /// Registered workspace scope.
    Workspace,
    /// Explicit Work Item scope.
    WorkItem,
    /// Trusted Agent session scope.
    Session,
    /// Delegation lineage scope.
    Delegation,
    /// Authenticated host-adapter scope.
    Host,
}

/// Workspace migration and sole-writer mode exposed on the wire.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum WorkspaceMode {
    /// Pre-cutover mode, retained so existing workspaces still deserialize.
    /// The implementation it once deferred to no longer exists.
    #[serde(rename = "legacy")]
    Legacy,
    /// Rust evaluates and compares but does not mutate.
    #[serde(rename = "shadow")]
    Shadow,
    /// Rust is sole writer for an approved canary workspace.
    #[serde(rename = "rust-canary")]
    RustCanary,
    /// Rust is the permanent sole writer.
    #[serde(rename = "rust-sole-writer")]
    RustSoleWriter,
}

/// Typed business outcome of a Gate evaluation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GateOutcomeKind {
    /// Fresh evaluation passed.
    Pass,
    /// Fresh evaluation found a business-rule violation.
    Fail,
    /// Gate infrastructure or implementation failed.
    Error,
    /// Evaluation exceeded its deadline.
    Timeout,
    /// Evaluation was explicitly cancelled.
    Cancelled,
    /// Inputs changed before the result could be consumed.
    Stale,
}

/// Decision returned to a thin Agent hook adapter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookDecision {
    /// Permit the host action.
    Allow,
    /// Deny a pre-tool action.
    Deny,
    /// Block turn completion.
    Block,
    /// Inject a bounded context projection.
    Context,
}

/// State of an asynchronous daemon job.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// Accepted but not started.
    Queued,
    /// Currently executing.
    Running,
    /// Completed with a fresh pass.
    Pass,
    /// Completed with business findings.
    Fail,
    /// Failed because of infrastructure or implementation error.
    Error,
    /// Exceeded its deadline.
    Timeout,
    /// Explicitly cancelled.
    Cancelled,
    /// Completed against inputs that are no longer current.
    Stale,
}

/// Shape of a context projection returned to a client.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextPayloadKind {
    /// Complete bounded projection.
    Full,
    /// Delta from the caller's known revision and digest.
    Delta,
    /// Empty result because the caller already has the current digest.
    NoChange,
}

/// Exact command kinds supported by the host runtime adapter boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostActionKind {
    /// Create a physical child Agent session.
    Create,
    /// Send bounded input to a physical session.
    Send,
    /// Wait for a host-side lifecycle event.
    Wait,
    /// Cancel a host task or session.
    Cancel,
    /// Attest a physical child identity and claim.
    Attest,
    /// Ask the host to compact a specific session generation.
    Compact,
}

/// Authenticated acknowledgement outcome for a host action.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAckOutcome {
    /// Host accepted and correlated the command.
    Accepted,
    /// Host explicitly rejected the command.
    Rejected,
    /// Host attempted the command but it failed.
    Failed,
}

/// Lifecycle state of a compact cycle.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum CompactStatus {
    /// Trusted pressure crossed the high-watermark policy.
    #[serde(rename = "pressure-detected")]
    PressureDetected,
    /// A durable pre-compact snapshot exists.
    #[serde(rename = "snapshot-ready")]
    SnapshotReady,
    /// A host compact action was dispatched.
    #[serde(rename = "compact-requested")]
    CompactRequested,
    /// The host reports an in-progress compact action.
    #[serde(rename = "host-compacting")]
    HostCompacting,
    /// A correlated host acknowledgement was validated.
    #[serde(rename = "host-acknowledged")]
    HostAcknowledged,
    /// Projection rehydration succeeded and generation advanced.
    #[serde(rename = "context-restored")]
    ContextRestored,
    /// The host cannot provide required pressure or compact capability.
    #[serde(rename = "unsupported")]
    Unsupported,
    /// The acknowledgement deadline expired.
    #[serde(rename = "timed-out")]
    TimedOut,
    /// The cycle failed without advancing generation.
    #[serde(rename = "failed")]
    Failed,
}
