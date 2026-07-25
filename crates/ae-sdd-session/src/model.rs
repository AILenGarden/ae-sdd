//! Part B session bootstrap value types.
//!
//! These types are owned by `ae-sdd-session` and are not part of the frozen
//! `ae-sdd-contracts` boundary: they exist to let `SessionBootstrapPort::bootstrap`
//! take an explicit, pre-resolved snapshot instead of reading daemon state itself.

use ae_sdd_domain::{SessionId, WorkspaceId};

/// Existing daemon session bound to a workspace, as resolved by the caller's
/// `session_by_external` lookup prior to calling `bootstrap`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExistingSessionInfo {
    session_id: SessionId,
    workspace_id: WorkspaceId,
}

impl ExistingSessionInfo {
    /// Builds an existing-session snapshot fragment.
    #[must_use]
    pub const fn new(session_id: SessionId, workspace_id: WorkspaceId) -> Self {
        Self {
            session_id,
            workspace_id,
        }
    }

    /// Returns the existing daemon session identity.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Returns the workspace the existing session is bound to.
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
}

/// Explicit, pre-resolved snapshot of the three read-only facts `bootstrap`
/// needs to decide a plan. The caller resolves these from workspace registry,
/// `session_by_external` index, and context projection cache before calling
/// `bootstrap`; the port performs no I/O of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootstrapSnapshot {
    workspace_registered: bool,
    existing_session: Option<ExistingSessionInfo>,
    context_ready: bool,
}

impl BootstrapSnapshot {
    /// Builds a bootstrap snapshot from already-resolved facts.
    #[must_use]
    pub const fn new(
        workspace_registered: bool,
        existing_session: Option<ExistingSessionInfo>,
        context_ready: bool,
    ) -> Self {
        Self {
            workspace_registered,
            existing_session,
            context_ready,
        }
    }

    /// Returns whether the target workspace is already registered.
    #[must_use]
    pub const fn workspace_registered(&self) -> bool {
        self.workspace_registered
    }

    /// Returns the existing session bound to this external conversation, if any.
    #[must_use]
    pub const fn existing_session(&self) -> Option<ExistingSessionInfo> {
        self.existing_session
    }

    /// Returns whether a context projection is already cached.
    #[must_use]
    pub const fn context_ready(&self) -> bool {
        self.context_ready
    }
}

/// One deterministic bootstrap orchestration step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootstrapStep {
    RegisterWorkspace,
    OpenSession,
    GrantScope,
    ProjectContext,
}

/// Ordered bootstrap intent produced by pure decision logic, plus a
/// content digest for idempotent-replay verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootstrapPlan {
    steps: Vec<BootstrapStep>,
    plan_digest: [u8; 32],
}

impl BootstrapPlan {
    pub(crate) const fn new(steps: Vec<BootstrapStep>, plan_digest: [u8; 32]) -> Self {
        Self { steps, plan_digest }
    }

    /// Returns the ordered bootstrap steps.
    #[must_use]
    pub fn steps(&self) -> &[BootstrapStep] {
        &self.steps
    }

    /// Returns the SHA-256 digest of the canonical (request, snapshot, steps) triple.
    #[must_use]
    pub const fn plan_digest(&self) -> [u8; 32] {
        self.plan_digest
    }
}
