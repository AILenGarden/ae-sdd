use super::*;

impl RuntimeService {
    /// Returns the current lifecycle projection.
    pub fn status(&self) -> RuntimeResult<RuntimeStatus> {
        let state = self.lock_state()?;
        Ok(RuntimeStatus {
            lifecycle: self.lifecycle()?,
            boot_id: self.boot_id.to_string(),
            event_store_id: self.persistence.event_store_id()?.to_string(),
            event_seq: self.persistence.latest_event_sequence()?,
            workspace_count: state.workspaces.len(),
            session_count: state.sessions.values().filter(|item| item.active).count(),
            policy_digest: self.config.policy_digest.clone(),
        })
    }

    /// Requests drain or stop without bypassing the authenticated RPC lifecycle.
    pub fn set_lifecycle(&self, next: DaemonLifecycle) -> RuntimeResult<()> {
        let mut lifecycle = self.lifecycle.write().map_err(lock_error)?;
        if *lifecycle == next {
            return Ok(());
        }
        match (*lifecycle, next) {
            (DaemonLifecycle::Running, DaemonLifecycle::Draining)
            | (DaemonLifecycle::Draining, DaemonLifecycle::Stopping)
            | (DaemonLifecycle::Running, DaemonLifecycle::Stopping) => {
                *lifecycle = next;
                Ok(())
            }
            _ => Err(RuntimeError::new(
                StableErrorCode::DaemonDraining,
                "daemon lifecycle transition is invalid",
            )),
        }
    }

    pub(super) fn runtime_drain(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let stop = params
            .payload
            .get("stop")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        self.set_lifecycle(if stop {
            DaemonLifecycle::Stopping
        } else {
            DaemonLifecycle::Draining
        })?;
        to_value(self.status()?)
    }

    pub(super) fn events_subscribe(&self, params: &RequestParams<Value>) -> RuntimeResult<Value> {
        let payload: EventSubscriptionPayload = decode_value(params.payload.clone())?;
        let store_id = self.persistence.event_store_id()?.to_string();
        if payload.event_store_id != store_id {
            return Err(RuntimeError::new(
                StableErrorCode::EventCursorGap,
                "event cursor belongs to another event-store epoch",
            ));
        }
        if payload.limit == 0 || payload.limit > self.config.max_event_batch {
            return Err(RuntimeError::new(
                StableErrorCode::SubscriberBackpressure,
                "event subscriber requested an invalid or unbounded batch",
            ));
        }
        let latest = self.persistence.latest_event_sequence()?;
        let oldest = self.persistence.oldest_event_sequence()?;
        if payload.after_event_seq > latest
            || (oldest > 1 && payload.after_event_seq.saturating_add(1) < oldest)
        {
            return Err(RuntimeError::new(
                StableErrorCode::EventCursorGap,
                "event cursor is outside the retained sequence window",
            )
            .with_remediation("fetch workspace.snapshot before resubscribing"));
        }
        to_value(EventBatch {
            event_store_id: store_id,
            events: self
                .persistence
                .events_after(payload.after_event_seq, payload.limit)?,
            snapshot_required: false,
        })
    }
}
