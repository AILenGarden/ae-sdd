use super::*;

impl RuntimeService {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn commit_receipt_event(
        &self,
        scope: &str,
        key: &str,
        request_digest: String,
        response: Value,
        kind: &str,
        workspace_id: Option<String>,
        session_id: Option<String>,
        work_item_id: Option<String>,
    ) -> RuntimeResult<(Value, u64)> {
        let response_json = serde_json::to_string(&response).map_err(canonical_error)?;
        let event_payload = json!({"scope":scope,"key":key});
        let event_payload_bytes = serde_json::to_vec(&event_payload).map_err(canonical_error)?;
        let event = DurableEvent {
            event_store_id: self.persistence.event_store_id()?.to_string(),
            event_seq: 0,
            boot_id: self.boot_id.to_string(),
            kind: kind.to_owned(),
            workspace_id,
            session_id,
            work_item_id,
            payload: event_payload,
            payload_digest: hex::encode(Sha256::digest(event_payload_bytes)),
        };
        let receipt = IdempotencyReceipt {
            scope: scope.to_owned(),
            key: key.to_owned(),
            request_digest,
            response_json,
            event_seq: 0,
        };
        let (_, receipt) = self.persistence.commit_event_and_receipt(event, receipt)?;
        let value = serde_json::from_str(&receipt.response_json).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "durable receipt response is malformed",
            )
        })?;
        Ok((value, receipt.event_seq))
    }

    pub(super) fn replay_receipt(
        &self,
        scope: &str,
        key: &str,
        request_digest: &str,
    ) -> RuntimeResult<Option<(Value, u64)>> {
        let Some(receipt) = self.persistence.load_receipt(scope, key)? else {
            return Ok(None);
        };
        if receipt.request_digest != request_digest {
            return Err(RuntimeError::new(
                StableErrorCode::IdempotencyKeyReused,
                "idempotency key was replayed with a different canonical payload",
            ));
        }
        let value = serde_json::from_str(&receipt.response_json).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "durable idempotency receipt is malformed",
            )
        })?;
        Ok(Some((value, receipt.event_seq)))
    }

    pub(super) fn admit(&self) -> RuntimeResult<Admission<'_>> {
        let previous = self.admitted.fetch_add(1, Ordering::AcqRel);
        if previous >= self.config.connection_capacity {
            self.admitted.fetch_sub(1, Ordering::AcqRel);
            return Err(RuntimeError::new(
                StableErrorCode::SubscriberBackpressure,
                "runtime request admission capacity is exhausted",
            ));
        }
        Ok(Admission {
            count: &self.admitted,
        })
    }

    pub(super) fn lifecycle(&self) -> RuntimeResult<DaemonLifecycle> {
        self.lifecycle
            .read()
            .map(|value| *value)
            .map_err(lock_error)
    }

    pub(super) fn lock_state(&self) -> RuntimeResult<std::sync::MutexGuard<'_, RuntimeState>> {
        self.state.lock().map_err(lock_error)
    }
}
