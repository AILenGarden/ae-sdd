//! `HostSupervisorPort::{register,dispatch,cancel,compact}` — the Part B
//! adapter-facing supervisor that wraps a single injected
//! `ae_sdd_host::HostRuntimeAdapter` instance. See SPI-2 in
//! `ae-sdd-doc/Story/STORY-AE-SDD-SESSION-HOST-001.md` for the full contract
//! this module implements.

use std::collections::BTreeSet;
use std::sync::Mutex;

use ae_sdd_contracts::compact::CompactRequest;
use ae_sdd_domain::{HostActionId, SessionId};
use ae_sdd_host::{
    HostAck, HostAckOutcome, HostAction, HostActionError, HostActionKind, HostAdapterError,
    HostAdapterId, HostRuntimeAdapter, HostTaskId,
};
use thiserror::Error;
use uuid::Uuid;

/// Cancel target granularity accepted by [`HostSupervisorPort::cancel`].
///
/// Mirrors the shape of the frozen contract's `HostCancelTarget`, but with
/// local `ae-sdd-host` identity types. Scope is intentionally narrower than
/// the frozen contract for Part B: see [`HostSupervisorError::CancelTargetUnsupported`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalCancelTarget {
    /// Cancel by host-task identity. Not representable in Part B: local
    /// `HostAction` (`Cancel` kind) carries no `host_task_id` field and
    /// `HostRuntimeAdapter::dispatch` has no other channel to carry it.
    HostTask(HostTaskId),
    /// Cancel by session identity: the only granularity Part B can dispatch.
    Session(SessionId),
}

/// Minimal local summary of a `HostAck`, returned by `dispatch`/`cancel`/`compact`.
/// A field subset of `ae_sdd_host::HostAck` (it omits `ack_id`, which carries
/// no correlation meaning for the caller).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostAckSummary {
    outcome: HostAckOutcome,
    action_id: HostActionId,
    adapter_id: HostAdapterId,
    command_seq: u64,
    host_task_id: Option<HostTaskId>,
    session_id: Option<SessionId>,
}

impl HostAckSummary {
    fn from_ack(ack: &HostAck) -> Self {
        Self {
            outcome: ack.outcome().clone(),
            action_id: ack.action_id(),
            adapter_id: ack.adapter_id().clone(),
            command_seq: ack.command_seq(),
            host_task_id: ack.host_task_id().cloned(),
            session_id: ack.session_id(),
        }
    }

    /// Returns the real dispatch outcome (accepted, or host-rejected with a code).
    #[must_use]
    pub const fn outcome(&self) -> &HostAckOutcome {
        &self.outcome
    }

    /// Returns the dispatched action's identity.
    #[must_use]
    pub const fn action_id(&self) -> HostActionId {
        self.action_id
    }

    /// Returns the target adapter identity.
    #[must_use]
    pub fn adapter_id(&self) -> &HostAdapterId {
        &self.adapter_id
    }

    /// Returns the correlation command sequence.
    #[must_use]
    pub const fn command_seq(&self) -> u64 {
        self.command_seq
    }

    /// Returns the host-bound task identity, if the host reported one.
    #[must_use]
    pub const fn host_task_id(&self) -> Option<&HostTaskId> {
        self.host_task_id.as_ref()
    }

    /// Returns the bound child session identity, if applicable.
    #[must_use]
    pub const fn session_id(&self) -> Option<SessionId> {
        self.session_id
    }
}

/// Supervisor-level error for `register`/`dispatch`/`cancel`/`compact`.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum HostSupervisorError {
    /// Dispatch exceeded its internal deadline; delivery is unconfirmed.
    #[error("host action dispatch timed out")]
    Timeout,
    /// The host process failed to start.
    #[error("host adapter is unavailable")]
    Unavailable,
    /// `cancel` was called with `LocalCancelTarget::HostTask(..)`: Part B's
    /// local `HostAction` has no field to carry a host-task identity, so
    /// this granularity cannot be dispatched yet.
    #[error("host task-scoped cancellation is not supported in this build")]
    CancelTargetUnsupported,
    /// Wraps a `HostActionError` raised either by internal `HostAction`
    /// construction (e.g. `CompactBindingRequired`) or by ACK correlation
    /// validation (`AckCorrelationMismatch`). Never exposes a bare
    /// `HostActionError` to callers.
    #[error(transparent)]
    ActionRejected(#[from] HostActionError),
}

impl From<HostAdapterError> for HostSupervisorError {
    fn from(value: HostAdapterError) -> Self {
        match value {
            HostAdapterError::Timeout => Self::Timeout,
            HostAdapterError::Unavailable | HostAdapterError::Rejected(_) => Self::Unavailable,
        }
    }
}

/// SPI-2 supervisor wrapping a single injected `HostRuntimeAdapter`.
///
/// `register` only maintains a local declaration set (`registered_adapters`)
/// for future C1 multi-adapter routing. Nothing prechecks what the injected
/// adapter can do: whether an errand succeeded is reported by the host as an
/// ACK outcome, which is the only account of it that can be verified.
pub struct HostSupervisor<A: HostRuntimeAdapter> {
    adapter: A,
    registered_adapters: Mutex<BTreeSet<HostAdapterId>>,
}

impl<A: HostRuntimeAdapter> HostSupervisor<A> {
    /// Wraps `adapter` as the single Part B `HostRuntimeAdapter` instance.
    #[must_use]
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            registered_adapters: Mutex::new(BTreeSet::new()),
        }
    }

    /// Records `adapter_id` in the Port-local declaration set.
    pub fn register(&self, adapter_id: &HostAdapterId) -> Result<(), HostSupervisorError> {
        self.registered_adapters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(adapter_id.clone());
        Ok(())
    }

    /// Dispatches a caller-constructed `HostAction`.
    pub fn dispatch(&self, action: &HostAction) -> Result<HostAckSummary, HostSupervisorError> {
        self.dispatch_checked(action)
    }

    /// Cancels a host-bound action. `HostTask` targets are rejected without
    /// constructing a `HostAction` or calling the adapter — Part B cannot
    /// represent that granularity (see [`LocalCancelTarget`]).
    pub fn cancel(
        &self,
        adapter_id: &HostAdapterId,
        target: LocalCancelTarget,
        command_seq: u64,
        deadline_unix_ms: u64,
    ) -> Result<HostAckSummary, HostSupervisorError> {
        let session_id = match target {
            LocalCancelTarget::HostTask(_) => {
                return Err(HostSupervisorError::CancelTargetUnsupported);
            }
            LocalCancelTarget::Session(id) => id,
        };

        let action = HostAction::new(
            HostActionId::from_uuid(Uuid::new_v4()),
            adapter_id.clone(),
            command_seq,
            HostActionKind::Cancel,
            None,
            None,
            Some(session_id),
            None,
            deadline_unix_ms,
            [0_u8; 32],
        )?;

        self.dispatch_checked(&action)
    }

    /// Compacts a session's context, generation-CAS already validated by C0.
    pub fn compact(&self, request: &CompactRequest) -> Result<HostAckSummary, HostSupervisorError> {
        // `AdapterId` (contracts, 128 bytes, alphanumeric+`._:-`) validates a
        // strict subset of `HostAdapterId` (host, 256 bytes, only excludes
        // control characters), so this conversion cannot fail for any value
        // that already passed `AdapterId::new`.
        let adapter_id = HostAdapterId::new(request.adapter_id().as_str())
            .expect("AdapterId's validation rules are a strict subset of HostAdapterId's");

        let action = HostAction::new(
            HostActionId::from_uuid(Uuid::new_v4()),
            adapter_id,
            request.next_generation().get(),
            HostActionKind::Compact,
            None,
            Some(request.compact_id()),
            Some(request.session_id()),
            Some(request.previous_generation()),
            request.deadline_unix_ms(),
            [0_u8; 32],
        )?;

        self.dispatch_checked(&action)
    }

    fn dispatch_checked(&self, action: &HostAction) -> Result<HostAckSummary, HostSupervisorError> {
        let ack = self.adapter.dispatch(action)?;
        ack.validate_for(action)?;
        Ok(HostAckSummary::from_ack(&ack))
    }
}

#[cfg(test)]
mod tests {
    use ae_sdd_contracts::compact::CompactRequest;
    use ae_sdd_contracts::{AdapterId, IdempotencyKey, SchemaVersion};
    use ae_sdd_domain::{
        ArtifactDigest, ArtifactKind, ArtifactRef, CompactId, ContextGeneration, HostAckId,
    };
    use std::sync::Mutex as StdMutex;

    use super::*;

    type StubResponse = Box<dyn FnMut(&HostAction) -> Result<HostAck, HostAdapterError> + Send>;

    struct StubAdapter {
        adapter_id: HostAdapterId,
        response: StdMutex<StubResponse>,
    }

    impl HostRuntimeAdapter for StubAdapter {
        fn adapter_id(&self) -> &HostAdapterId {
            &self.adapter_id
        }

        fn dispatch(&self, action: &HostAction) -> Result<HostAck, HostAdapterError> {
            (self.response.lock().unwrap())(action)
        }
    }

    fn adapter_id(value: &str) -> HostAdapterId {
        HostAdapterId::new(value).expect("adapter id")
    }

    fn accept(action: &HostAction) -> Result<HostAck, HostAdapterError> {
        Ok(HostAck::new(
            HostAckId::from_uuid(Uuid::new_v4()),
            action.action_id(),
            action.adapter_id().clone(),
            action.command_seq(),
            HostAckOutcome::Accepted,
            None,
            action.session_id(),
        )
        .expect("valid ack"))
    }

    fn supervisor_with(
        response: impl FnMut(&HostAction) -> Result<HostAck, HostAdapterError> + Send + 'static,
    ) -> HostSupervisor<StubAdapter> {
        HostSupervisor::new(StubAdapter {
            adapter_id: adapter_id("stub-adapter"),
            response: StdMutex::new(Box::new(response)),
        })
    }

    fn wait_action(adapter: &HostAdapterId) -> HostAction {
        HostAction::new(
            HostActionId::from_uuid(Uuid::new_v4()),
            adapter.clone(),
            1,
            HostActionKind::Wait,
            None,
            None,
            None,
            None,
            1,
            [0_u8; 32],
        )
        .expect("valid action")
    }

    #[test]
    fn register_records_an_addressable_adapter_and_is_idempotent() {
        let supervisor = supervisor_with(accept);
        assert_eq!(supervisor.register(&adapter_id("new-adapter")), Ok(()));
        assert_eq!(supervisor.register(&adapter_id("new-adapter")), Ok(()));
    }

    #[test]
    fn dispatch_maps_timeout() {
        let supervisor = supervisor_with(|_| Err(HostAdapterError::Timeout));
        let action = wait_action(&adapter_id("stub-adapter"));

        assert_eq!(
            supervisor.dispatch(&action),
            Err(HostSupervisorError::Timeout)
        );
    }

    #[test]
    fn dispatch_maps_host_rejection_to_ok_summary_not_err() {
        let supervisor = supervisor_with(|action| {
            Ok(HostAck::new(
                HostAckId::from_uuid(Uuid::new_v4()),
                action.action_id(),
                action.adapter_id().clone(),
                action.command_seq(),
                HostAckOutcome::Rejected {
                    error_code: "denied".into(),
                },
                None,
                action.session_id(),
            )
            .expect("valid ack"))
        });
        let action = wait_action(&adapter_id("stub-adapter"));

        let summary = supervisor.dispatch(&action).expect("host rejection is Ok");
        assert_eq!(
            summary.outcome(),
            &HostAckOutcome::Rejected {
                error_code: "denied".into()
            }
        );
    }

    #[test]
    fn dispatch_rejects_ack_correlation_mismatch() {
        let supervisor = supervisor_with(|_action| {
            Ok(HostAck::new(
                HostAckId::from_uuid(Uuid::new_v4()),
                HostActionId::from_uuid(Uuid::new_v4()), // mismatched action_id
                adapter_id("stub-adapter"),
                1,
                HostAckOutcome::Accepted,
                None,
                None,
            )
            .expect("valid ack"))
        });
        let action = wait_action(&adapter_id("stub-adapter"));

        assert_eq!(
            supervisor.dispatch(&action),
            Err(HostSupervisorError::ActionRejected(
                HostActionError::AckCorrelationMismatch
            ))
        );
    }

    #[test]
    fn cancel_host_task_target_is_rejected_without_constructing_an_action() {
        let supervisor = supervisor_with(|_| {
            panic!("adapter.dispatch must not be called for an unsupported cancel target")
        });
        let target = LocalCancelTarget::HostTask(HostTaskId::new("task-1").expect("host task id"));

        let outcome = supervisor.cancel(&adapter_id("stub-adapter"), target, 1, 1);
        assert_eq!(outcome, Err(HostSupervisorError::CancelTargetUnsupported));
    }

    #[test]
    fn cancel_session_target_dispatches_and_returns_summary() {
        let supervisor = supervisor_with(accept);
        let session_id = SessionId::from_uuid(Uuid::new_v4());
        let target = LocalCancelTarget::Session(session_id);

        let summary = supervisor
            .cancel(&adapter_id("stub-adapter"), target, 1, 1)
            .expect("cancel dispatches");
        assert_eq!(summary.session_id(), Some(session_id));
        assert_eq!(summary.outcome(), &HostAckOutcome::Accepted);
    }

    fn compact_request(adapter: &str) -> CompactRequest {
        CompactRequest::new(
            SchemaVersion::V1,
            CompactId::from_uuid(Uuid::new_v4()),
            SessionId::from_uuid(Uuid::new_v4()),
            AdapterId::new(adapter).expect("adapter id"),
            ArtifactRef::new(
                ArtifactKind::new("context-snapshot").expect("artifact kind"),
                ae_sdd_domain::ProjectRelativePath::new(".ae-sdd/snapshots/compact-1.json")
                    .expect("relative path"),
                ArtifactDigest::digest(b"snapshot"),
                8,
            ),
            ContextGeneration::new(1),
            ContextGeneration::new(2),
            1,
            IdempotencyKey::new("compact-1").expect("idempotency key"),
        )
        .expect("valid compact request")
    }

    #[test]
    fn compact_converts_adapter_id_and_dispatches() {
        let supervisor = supervisor_with(accept);
        let request = compact_request("stub-adapter");

        let summary = supervisor.compact(&request).expect("compact dispatches");
        assert_eq!(summary.session_id(), Some(request.session_id()));
        assert_eq!(summary.command_seq(), request.next_generation().get());
    }
}
