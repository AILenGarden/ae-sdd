//! Typed errors for `SessionBootstrapPort::bootstrap`.

use thiserror::Error;

/// Validation error for the Part B pure bootstrap decision.
///
/// Note: `bootstrap` never re-validates the frozen C0 `SessionBootstrapRequest`
/// invariants (root/delegation binding). Those are enforced at
/// `SessionBootstrapRequest::new` construction time and produce
/// `ae_sdd_contracts::session::SessionContractError` there, before a request
/// can even reach `bootstrap`. By the time `bootstrap` receives
/// `&SessionBootstrapRequest`, the type already guarantees that invariant
/// holds, so `SessionBootstrapError` carries only the decision this crate
/// actually makes.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SessionBootstrapError {
    /// `snapshot.existing_session` is bound to a different workspace than
    /// `request.workspace_id()`. The caller must not guess that the existing
    /// session belongs to the requested workspace; it must correct the
    /// upstream `session_by_external` lookup instead.
    #[error("existing session is bound to a different workspace than the bootstrap request")]
    CrossWorkspaceSession,
}
