#![allow(unused_imports)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use ae_sdd_context::{
    PressureDecision, PressurePolicy, PressureSample, PressureSource, PressureTracker,
};
use ae_sdd_domain::{
    AgentRole, ClaimId, CompactId, ContextGeneration, DelegationId, EventStoreId, HostAckId,
    HostActionId, InputFingerprint, SampleSequence, SessionId,
};
use ae_sdd_flow::{FlowDecision, FlowEvent, FlowInput, FlowRuntime};
use ae_sdd_host::{
    ChildClaim, HostAck, HostAckOutcome, HostAction, HostActionKind, HostAdapterId, HostTaskId,
    PhysicalSessionProof,
};
use ae_sdd_protocol::StableErrorCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    ContextProjectResult, ContextProjectionInput, DelegationCreatePayload, DelegationReportPayload,
    DelegationResult, HostAckPayload, HostActionPayload, HostPressurePayload, PersistencePort,
    RuntimeError, RuntimeResult, WireAgentRole,
};

/// Durable wrapper around the pure flow reducer.
pub struct FlowSupervisor {
    persistence: Arc<dyn PersistencePort>,
}

impl FlowSupervisor {
    /// Creates a supervisor backed by a durable checkpoint port.
    #[must_use]
    pub fn new(persistence: Arc<dyn PersistencePort>) -> Self {
        Self { persistence }
    }

    /// Replays committed events and persists the resulting decision checkpoint.
    pub fn replay(
        &self,
        workspace_id: &str,
        work_item_id: &str,
        input: FlowInput,
        events: impl IntoIterator<Item = FlowEvent>,
    ) -> RuntimeResult<FlowDecision> {
        let decision = FlowRuntime::replay(input, events).map_err(|error| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                format!("deterministic flow replay rejected committed input: {error}"),
            )
        })?;
        let checkpoint = json!({
            "schemaVersion": "flow-supervisor-checkpoint/v1",
            "workspaceId": workspace_id,
            "workItemId": work_item_id,
            "decisionDigest": decision.decision_digest().to_hex(),
            "lastEventSeq": decision.last_cursor().map_or(0, |cursor| cursor.sequence().get()),
            "nextAction": format!("{:?}", decision.next_action()),
        });
        self.persistence.store_record(
            "flow-supervisor/v1",
            &format!("{workspace_id}\0{work_item_id}"),
            &checkpoint,
        )?;
        Ok(decision)
    }

    /// Reads the last durable checkpoint projection.
    pub fn checkpoint(
        &self,
        workspace_id: &str,
        work_item_id: &str,
    ) -> RuntimeResult<Option<Value>> {
        self.persistence.load_record(
            "flow-supervisor/v1",
            &format!("{workspace_id}\0{work_item_id}"),
        )
    }
}
