use std::{collections::BTreeMap, sync::Mutex};

use ae_sdd_domain::{
    CompactId, ContextProjectionId, DelegationId, EventSequence, EventStoreId, HostActionId,
    SessionId, WorkItemId, WorkspaceId,
};

use crate::{
    ChildResultRecord, CompactCycleRecord, ContextPressureSampleRecord, ContextProjectionRecord,
    DelegationRecord, DelegationRequestReceipt, HookEventReceipt, HostAckReceipt, HostActionRecord,
    HostAdapterRecord, MemoryCleanupReceipt, OperationReceipt, RuntimeEventDraft,
    RuntimeEventRecord, StoreError, SupervisorCheckpointRecord,
};

pub trait RuntimeRepository: Send + Sync {
    fn event_store_id(&self) -> EventStoreId;
    fn operation_receipt(
        &self,
        workspace_id: WorkspaceId,
        idempotency_key: &str,
    ) -> Result<Option<(OperationReceipt, RuntimeEventRecord)>, StoreError>;
    fn index_committed_mutation(
        &self,
        receipt: &OperationReceipt,
        event: &RuntimeEventDraft,
    ) -> Result<(OperationReceipt, RuntimeEventRecord), StoreError>;
    fn persist_delegation(&self, record: &DelegationRecord) -> Result<(), StoreError>;
    fn delegation(&self, id: DelegationId) -> Result<Option<DelegationRecord>, StoreError>;
    fn put_delegation_request_receipt(
        &self,
        receipt: &DelegationRequestReceipt,
    ) -> Result<DelegationRequestReceipt, StoreError>;
    fn persist_child_result(&self, record: &ChildResultRecord) -> Result<(), StoreError>;
    fn persist_memory_cleanup(&self, receipt: &MemoryCleanupReceipt) -> Result<(), StoreError>;
    fn persist_host_adapter(&self, record: &HostAdapterRecord) -> Result<(), StoreError>;
    fn persist_host_action(&self, record: &HostActionRecord) -> Result<(), StoreError>;
    fn put_host_ack(&self, receipt: &HostAckReceipt) -> Result<HostAckReceipt, StoreError>;
    fn persist_pressure_sample(
        &self,
        record: &ContextPressureSampleRecord,
    ) -> Result<(), StoreError>;
    fn persist_context_projection(
        &self,
        record: &ContextProjectionRecord,
    ) -> Result<(), StoreError>;
    fn persist_compact_cycle(&self, record: &CompactCycleRecord) -> Result<(), StoreError>;
    fn persist_supervisor_checkpoint(
        &self,
        record: &SupervisorCheckpointRecord,
    ) -> Result<(), StoreError>;
    fn supervisor_checkpoint(
        &self,
        workspace_id: WorkspaceId,
        work_item_id: &WorkItemId,
    ) -> Result<Option<SupervisorCheckpointRecord>, StoreError>;
    fn put_hook_event_receipt(
        &self,
        receipt: &HookEventReceipt,
    ) -> Result<HookEventReceipt, StoreError>;
}

#[derive(Debug)]
pub struct InMemoryRuntimeRepository {
    event_store_id: EventStoreId,
    state: Mutex<MemoryState>,
}

#[derive(Debug, Default)]
struct MemoryState {
    next_event_sequence: u64,
    events: BTreeMap<u64, RuntimeEventRecord>,
    operation_receipts: BTreeMap<(WorkspaceId, Box<str>), (OperationReceipt, u64)>,
    delegations: BTreeMap<DelegationId, DelegationRecord>,
    delegation_receipts: BTreeMap<(WorkspaceId, SessionId, Box<str>), DelegationRequestReceipt>,
    child_results: BTreeMap<DelegationId, ChildResultRecord>,
    memory_cleanup: BTreeMap<DelegationId, MemoryCleanupReceipt>,
    host_adapters: BTreeMap<Box<str>, HostAdapterRecord>,
    host_actions: BTreeMap<HostActionId, HostActionRecord>,
    host_command_keys: BTreeMap<(Box<str>, u64), HostActionId>,
    host_acks: BTreeMap<(Box<str>, Box<str>), HostAckReceipt>,
    pressure_samples: BTreeMap<(Box<str>, SessionId, u64), ContextPressureSampleRecord>,
    projections: BTreeMap<ContextProjectionId, ContextProjectionRecord>,
    projection_keys: BTreeMap<(SessionId, u64), ContextProjectionId>,
    compact_cycles: BTreeMap<CompactId, CompactCycleRecord>,
    checkpoints: BTreeMap<(WorkspaceId, WorkItemId), SupervisorCheckpointRecord>,
    hook_receipts: BTreeMap<(SessionId, Box<str>), HookEventReceipt>,
}

impl InMemoryRuntimeRepository {
    pub fn new(event_store_id: EventStoreId) -> Self {
        Self {
            event_store_id,
            state: Mutex::new(MemoryState::default()),
        }
    }

    pub fn events(&self) -> Vec<RuntimeEventRecord> {
        self.state
            .lock()
            .expect("in-memory repository lock is not poisoned")
            .events
            .values()
            .cloned()
            .collect()
    }
}

impl RuntimeRepository for InMemoryRuntimeRepository {
    fn event_store_id(&self) -> EventStoreId {
        self.event_store_id
    }

    fn operation_receipt(
        &self,
        workspace_id: WorkspaceId,
        idempotency_key: &str,
    ) -> Result<Option<(OperationReceipt, RuntimeEventRecord)>, StoreError> {
        let state = self
            .state
            .lock()
            .expect("in-memory repository lock is not poisoned");
        Ok(state
            .operation_receipts
            .get(&(workspace_id, idempotency_key.into()))
            .and_then(|(receipt, sequence)| {
                state
                    .events
                    .get(sequence)
                    .map(|event| (receipt.clone(), event.clone()))
            }))
    }

    fn index_committed_mutation(
        &self,
        receipt: &OperationReceipt,
        event: &RuntimeEventDraft,
    ) -> Result<(OperationReceipt, RuntimeEventRecord), StoreError> {
        event.validate()?;
        let mut state = self
            .state
            .lock()
            .expect("in-memory repository lock is not poisoned");
        let key = (
            receipt.workspace_id,
            Box::<str>::from(receipt.idempotency_key.as_str()),
        );
        if let Some((existing, sequence)) = state.operation_receipts.get(&key) {
            if existing.payload_digest != receipt.payload_digest {
                return Err(StoreError::IdempotencyKeyReused {
                    expected: existing.payload_digest,
                    observed: receipt.payload_digest,
                });
            }
            let event = state
                .events
                .get(sequence)
                .expect("receipt event remains indexed")
                .clone();
            return Ok((existing.clone(), event));
        }
        let sequence = state.next_event_sequence.checked_add(1).ok_or_else(|| {
            StoreError::DatabaseIncompatible {
                reason: "global event sequence exhausted".into(),
            }
        })?;
        state.next_event_sequence = sequence;
        let event_record = RuntimeEventRecord {
            event_store_id: self.event_store_id,
            event_sequence: EventSequence::new(sequence),
            draft: event.clone(),
        };
        state.events.insert(sequence, event_record.clone());
        state
            .operation_receipts
            .insert(key, (receipt.clone(), sequence));
        Ok((receipt.clone(), event_record))
    }

    fn persist_delegation(&self, record: &DelegationRecord) -> Result<(), StoreError> {
        self.state
            .lock()
            .expect("in-memory repository lock is not poisoned")
            .delegations
            .insert(record.delegation_id, record.clone());
        Ok(())
    }

    fn delegation(&self, id: DelegationId) -> Result<Option<DelegationRecord>, StoreError> {
        Ok(self
            .state
            .lock()
            .expect("in-memory repository lock is not poisoned")
            .delegations
            .get(&id)
            .cloned())
    }

    fn put_delegation_request_receipt(
        &self,
        receipt: &DelegationRequestReceipt,
    ) -> Result<DelegationRequestReceipt, StoreError> {
        let mut state = self
            .state
            .lock()
            .expect("in-memory repository lock is not poisoned");
        let key = (
            receipt.workspace_id,
            receipt.parent_session_id,
            Box::<str>::from(receipt.idempotency_key.as_str()),
        );
        if let Some(existing) = state.delegation_receipts.get(&key) {
            if existing.request_digest != receipt.request_digest {
                return Err(StoreError::IdempotencyKeyReused {
                    expected: existing.request_digest,
                    observed: receipt.request_digest,
                });
            }
            return Ok(existing.clone());
        }
        state.delegation_receipts.insert(key, receipt.clone());
        Ok(receipt.clone())
    }

    fn persist_child_result(&self, record: &ChildResultRecord) -> Result<(), StoreError> {
        persist_once_or_same(
            &mut self
                .state
                .lock()
                .expect("in-memory repository lock is not poisoned")
                .child_results,
            record.delegation_id,
            record.clone(),
            "child_result",
        )
    }

    fn persist_memory_cleanup(&self, receipt: &MemoryCleanupReceipt) -> Result<(), StoreError> {
        persist_once_or_same(
            &mut self
                .state
                .lock()
                .expect("in-memory repository lock is not poisoned")
                .memory_cleanup,
            receipt.delegation_id,
            receipt.clone(),
            "memory_cleanup_receipt",
        )
    }

    fn persist_host_adapter(&self, record: &HostAdapterRecord) -> Result<(), StoreError> {
        self.state
            .lock()
            .expect("in-memory repository lock is not poisoned")
            .host_adapters
            .insert(record.adapter_id.clone(), record.clone());
        Ok(())
    }

    fn persist_host_action(&self, record: &HostActionRecord) -> Result<(), StoreError> {
        let mut state = self
            .state
            .lock()
            .expect("in-memory repository lock is not poisoned");
        let command_key = (record.adapter_id.clone(), record.command_sequence);
        if let Some(existing_action_id) = state.host_command_keys.get(&command_key)
            && *existing_action_id != record.action_id
        {
            return Err(StoreError::PersistenceConflict {
                entity: "host_action.command_sequence",
            });
        }
        state
            .host_command_keys
            .insert(command_key, record.action_id);
        state.host_actions.insert(record.action_id, record.clone());
        Ok(())
    }

    fn put_host_ack(&self, receipt: &HostAckReceipt) -> Result<HostAckReceipt, StoreError> {
        let mut state = self
            .state
            .lock()
            .expect("in-memory repository lock is not poisoned");
        let key = (
            receipt.adapter_id.clone(),
            receipt.ack_id.to_string().into_boxed_str(),
        );
        if let Some(existing) = state.host_acks.get(&key) {
            if existing != receipt {
                return Err(StoreError::PersistenceConflict { entity: "host_ack" });
            }
            return Ok(existing.clone());
        }
        state.host_acks.insert(key, receipt.clone());
        Ok(receipt.clone())
    }

    fn persist_pressure_sample(
        &self,
        record: &ContextPressureSampleRecord,
    ) -> Result<(), StoreError> {
        let key = (
            record.adapter_id.clone(),
            record.session_id,
            record.sample_sequence,
        );
        persist_once_or_same(
            &mut self
                .state
                .lock()
                .expect("in-memory repository lock is not poisoned")
                .pressure_samples,
            key,
            record.clone(),
            "context_pressure_sample",
        )
    }

    fn persist_context_projection(
        &self,
        record: &ContextProjectionRecord,
    ) -> Result<(), StoreError> {
        let mut state = self
            .state
            .lock()
            .expect("in-memory repository lock is not poisoned");
        let key = (record.session_id, record.context_revision.get());
        if let Some(existing_id) = state.projection_keys.get(&key)
            && *existing_id != record.projection_id
        {
            return Err(StoreError::PersistenceConflict {
                entity: "context_projection.revision",
            });
        }
        state.projection_keys.insert(key, record.projection_id);
        persist_once_or_same(
            &mut state.projections,
            record.projection_id,
            record.clone(),
            "context_projection",
        )
    }

    fn persist_compact_cycle(&self, record: &CompactCycleRecord) -> Result<(), StoreError> {
        self.state
            .lock()
            .expect("in-memory repository lock is not poisoned")
            .compact_cycles
            .insert(record.compact_id, record.clone());
        Ok(())
    }

    fn persist_supervisor_checkpoint(
        &self,
        record: &SupervisorCheckpointRecord,
    ) -> Result<(), StoreError> {
        let mut state = self
            .state
            .lock()
            .expect("in-memory repository lock is not poisoned");
        let key = (record.workspace_id, record.work_item_id.clone());
        if let Some(existing) = state.checkpoints.get(&key) {
            if record.last_event_sequence < existing.last_event_sequence
                || (record.last_event_sequence == existing.last_event_sequence
                    && record != existing)
            {
                return Err(StoreError::PersistenceConflict {
                    entity: "supervisor_checkpoint.cursor",
                });
            }
            if record == existing {
                return Ok(());
            }
        }
        state.checkpoints.insert(key, record.clone());
        Ok(())
    }

    fn supervisor_checkpoint(
        &self,
        workspace_id: WorkspaceId,
        work_item_id: &WorkItemId,
    ) -> Result<Option<SupervisorCheckpointRecord>, StoreError> {
        Ok(self
            .state
            .lock()
            .expect("in-memory repository lock is not poisoned")
            .checkpoints
            .get(&(workspace_id, work_item_id.clone()))
            .cloned())
    }

    fn put_hook_event_receipt(
        &self,
        receipt: &HookEventReceipt,
    ) -> Result<HookEventReceipt, StoreError> {
        let mut state = self
            .state
            .lock()
            .expect("in-memory repository lock is not poisoned");
        let key = (receipt.session_id, receipt.hook_event_id.clone());
        if let Some(existing) = state.hook_receipts.get(&key) {
            if existing.request_digest != receipt.request_digest {
                return Err(StoreError::IdempotencyKeyReused {
                    expected: existing.request_digest,
                    observed: receipt.request_digest,
                });
            }
            return Ok(existing.clone());
        }
        state.hook_receipts.insert(key, receipt.clone());
        Ok(receipt.clone())
    }
}

fn persist_once_or_same<K, V>(
    map: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    entity: &'static str,
) -> Result<(), StoreError>
where
    K: Ord,
    V: PartialEq,
{
    if let Some(existing) = map.get(&key) {
        if existing == &value {
            return Ok(());
        }
        return Err(StoreError::PersistenceConflict { entity });
    }
    map.insert(key, value);
    Ok(())
}
