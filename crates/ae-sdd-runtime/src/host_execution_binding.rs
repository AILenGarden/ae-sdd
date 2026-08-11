//! Host-execution binding ledger (`ae-sdd-daemon-design.md` §9.4).
//!
//! This is the **durable, delegation-keyed** liveness ledger. It is distinct
//! from [`crate::ExecutionSessionBinding`], which is the in-memory slice-supervisor
//! binding for the Hook fast path. This ledger answers one question only: "is the
//! host execution a delegation opened still alive, or has it been released /
//! preempted / expired?" It never authenticates a child — that stays with the
//! `claim_id` + `PhysicalSessionProof` chain.
//!
//! The shape mirrors [`ae_sdd_store::LeaseLedger`]: an in-memory struct whose
//! every mutation is followed by a `store_record("host-execution-binding/v1",
//! …)` upsert, with lazy expiry (`expire_if_needed`) called at the top of each
//! mutator. There is no background sweeper and no dedicated heartbeat RPC — the
//! four Hook methods refresh `last_interaction_unix_ms` in band.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::{PersistencePort, RuntimeError, RuntimeResult};
use ae_sdd_protocol::StableErrorCode;

/// Durable namespace for host-execution binding rows. Stored as a JSON blob in
/// `runtime_record_v1`; the typed `host_execution_binding_v1` table is a mirror
/// that stays optional for this ROUTE.
pub const HOST_EXECUTION_BINDING_V1: &str = "host-execution-binding/v1";

/// A binding whose `last_interaction_unix_ms` is older than this is considered
/// stale and a new applicant may preempt it (§1.4 branch 2).
pub const STALE_WINDOW_MS: u64 = 30 * 60 * 1000;

/// A binding that has not been touched for this long is reclaimed unconditionally
/// even if no close was signalled (§1.6 hard timeout).
pub const HARD_TIMEOUT_MS: u64 = 12 * 60 * 60 * 1000;

/// Reason recorded when a binding leaves the live state for a terminal one.
///
/// Kept as a plain string (not an enum) so future reasons can be added without
/// a migration of the durable shape; the CHECK constraint on the typed table is
/// the authority over the allowed set.
pub mod released_reason {
    pub const SESSION_CLOSED: &str = "session-closed";
    pub const COLLECTED: &str = "collected";
    pub const CANCELLED: &str = "cancelled";
    pub const EXPIRED: &str = "expired";
    pub const PREEMPTED: &str = "preempted";
}

/// Outcome of a [`HostExecutionBindingLedger::claim_or_preempt`] attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClaimOutcome {
    /// The applicant took the binding (it was already terminal, or it was stale
    /// enough to preempt). Carries the `binding_id` the applicant now owns.
    Claimed { binding_id: String },
    /// The live binding is within the stale window. The applicant must ask the
    /// root to release before retrying (§1.4 branch 3).
    RefusedWithinWindow,
    /// No binding row exists for the given key.
    NotFound,
}

/// Canonical durable row. `#[serde(default)]` on every added-later field keeps
/// older rows readable (D-03 forbids treating missing data as a fabricated
/// default — an absent `active_at_unix_ms` legitimately means "never activated").
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DurableHostExecutionBinding {
    pub schema_version: String,
    pub binding_id: String,
    pub workspace_id: String,
    pub root_session_id: String,
    pub delegation_id: String,
    pub status: String,
    pub created_at_unix_ms: u64,
    pub last_interaction_unix_ms: u64,
    #[serde(default)]
    pub active_at_unix_ms: Option<u64>,
    #[serde(default)]
    pub released_at_unix_ms: Option<u64>,
    #[serde(default)]
    pub released_reason: Option<String>,
}

impl DurableHostExecutionBinding {
    fn new(
        binding_id: String,
        workspace_id: String,
        root_session_id: String,
        delegation_id: String,
        now_unix_ms: u64,
    ) -> Self {
        Self {
            schema_version: "host-execution-binding/v1".to_owned(),
            binding_id,
            workspace_id,
            root_session_id,
            delegation_id,
            status: "spawning".to_owned(),
            created_at_unix_ms: now_unix_ms,
            last_interaction_unix_ms: now_unix_ms,
            active_at_unix_ms: None,
            released_at_unix_ms: None,
            released_reason: None,
        }
    }

    /// True once the delegation has reached a state it never leaves.
    fn is_terminal(&self) -> bool {
        matches!(self.status.as_str(), "released" | "preempted" | "expired")
    }

    /// True while the binding can still be claimed/preempted by an applicant.
    fn is_live(&self) -> bool {
        matches!(self.status.as_str(), "spawning" | "active")
    }
}

/// In-memory mirror of the binding ledger. Every mutation that changes a row
/// durably persists it; reads do not (they only sweep lazy expiry when it is
/// cheap to do so from the same lock hold).
///
/// Keyed by `delegation_id`, matching the `UNIQUE` constraint on the table:
/// one delegation carries at most one binding, and preemption is resolved by
/// `claim_id → delegation_id →` this single row (Plan §3.2 / revision 7).
pub struct HostExecutionBindingLedger {
    persistence: Mutex<Option<std::sync::Arc<dyn PersistencePort>>>,
    bindings: Mutex<BTreeMap<String, DurableHostExecutionBinding>>,
}

impl HostExecutionBindingLedger {
    /// Creates an empty ledger. The supervisor wires persistence via
    /// [`Self::attach_persistence`] at construction, mirroring how
    /// `DelegationSupervisor` holds its own `Arc<dyn PersistencePort>`.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            persistence: Mutex::new(None),
            bindings: Mutex::new(BTreeMap::new()),
        }
    }

    /// Binds the durable port. Required before any mutation that should persist.
    pub fn attach_persistence(&self, persistence: std::sync::Arc<dyn PersistencePort>) {
        *self
            .persistence
            .lock()
            .expect("binding ledger persistence lock poisoned") = Some(persistence);
    }

    /// Mints and persists a `spawning` binding for a fresh delegation.
    ///
    /// Idempotent at the caller contract level: the daemon derives the same
    /// `binding_id` for the same `(scope_digest, idempotency_key)`, so a replay
    /// of `delegation.create` re-enters here with the same key and this method
    /// simply no-ops if a row already exists.
    pub fn spawn(
        &self,
        binding_id: &str,
        workspace_id: &str,
        root_session_id: &str,
        delegation_id: &str,
        now_unix_ms: u64,
    ) -> RuntimeResult<()> {
        let mut bindings = self
            .bindings
            .lock()
            .map_err(|_| lock_error("binding ledger lock poisoned"))?;
        if bindings.contains_key(delegation_id) {
            return Ok(());
        }
        let record = DurableHostExecutionBinding::new(
            binding_id.to_owned(),
            workspace_id.to_owned(),
            root_session_id.to_owned(),
            delegation_id.to_owned(),
            now_unix_ms,
        );
        self.persist(&record)?;
        bindings.insert(delegation_id.to_owned(), record);
        Ok(())
    }

    /// Promotes a `spawning` binding to `active` (called from `delegation.accept`).
    ///
    /// Idempotent: an already-`active` binding is a no-op, which keeps an
    /// idempotent replay of accept from double-stamping `active_at_unix_ms`.
    pub fn activate(&self, delegation_id: &str, now_unix_ms: u64) -> RuntimeResult<()> {
        let mut bindings = self
            .bindings
            .lock()
            .map_err(|_| lock_error("binding ledger lock poisoned"))?;
        let Some(binding) = bindings.get_mut(delegation_id) else {
            // No binding row: the delegation predates this ledger or was recovered
            // from a malformed store. Activating nothing is the correct no-op.
            return Ok(());
        };
        expire_if_needed(binding, now_unix_ms);
        if binding.status != "spawning" {
            return Ok(());
        }
        binding.status = "active".to_owned();
        binding.active_at_unix_ms = Some(now_unix_ms);
        binding.last_interaction_unix_ms = now_unix_ms;
        let clone = binding.clone();
        drop(bindings);
        self.persist(&clone)
    }

    /// Marks a binding `released` with the given reason. Idempotent and no-op on
    /// missing rows, mirroring [`ae_sdd_store::LeaseLedger::release_by_owner`].
    pub fn release_by_delegation(
        &self,
        delegation_id: &str,
        reason: &str,
        now_unix_ms: u64,
    ) -> RuntimeResult<()> {
        let mut bindings = self
            .bindings
            .lock()
            .map_err(|_| lock_error("binding ledger lock poisoned"))?;
        let Some(binding) = bindings.get_mut(delegation_id) else {
            return Ok(());
        };
        expire_if_needed(binding, now_unix_ms);
        if binding.is_terminal() {
            return Ok(());
        }
        binding.status = "released".to_owned();
        binding.released_at_unix_ms = Some(now_unix_ms);
        binding.released_reason = Some(reason.to_owned());
        let clone = binding.clone();
        drop(bindings);
        self.persist(&clone)
    }

    /// Refreshes `last_interaction_unix_ms` from any Hook event on the child
    /// session. Cheap and safe to call on every hook: a root session has no
    /// delegation and resolves to an immediate no-op.
    pub fn refresh_interaction(
        &self,
        delegation_id: Option<&str>,
        now_unix_ms: u64,
    ) -> RuntimeResult<()> {
        let Some(delegation_id) = delegation_id else {
            return Ok(());
        };
        let mut bindings = self
            .bindings
            .lock()
            .map_err(|_| lock_error("binding ledger lock poisoned"))?;
        let Some(binding) = bindings.get_mut(delegation_id) else {
            return Ok(());
        };
        // Lazy hard-timeout sweep: if 12h have passed since the last interaction,
        // this very refresh becomes the expiry event. The caller still gets a
        // success (the session is interacting now), but the row transitions to
        // `expired` before the refresh lands so the audit trail is correct.
        expire_if_needed(binding, now_unix_ms);
        if binding.is_terminal() {
            return Ok(());
        }
        binding.last_interaction_unix_ms = now_unix_ms;
        let clone = binding.clone();
        drop(bindings);
        self.persist(&clone)
    }

    /// Resolves a claim under §1.4's three branches.
    ///
    /// Exposed for ROUTE-C (Child Self-Claim) to wire; this ROUTE has no
    /// production caller, so the body is exercised only by unit tests.
    pub fn claim_or_preempt(
        &self,
        delegation_id: &str,
        now_unix_ms: u64,
    ) -> RuntimeResult<ClaimOutcome> {
        let mut bindings = self
            .bindings
            .lock()
            .map_err(|_| lock_error("binding ledger lock poisoned"))?;
        let Some(binding) = bindings.get_mut(delegation_id) else {
            return Ok(ClaimOutcome::NotFound);
        };
        expire_if_needed(binding, now_unix_ms);
        if binding.is_terminal() {
            // Branch 1: released / preempted / expired → direct claim.
            return Ok(ClaimOutcome::Claimed {
                binding_id: binding.binding_id.clone(),
            });
        }
        let stale = now_unix_ms.saturating_sub(binding.last_interaction_unix_ms) > STALE_WINDOW_MS;
        if stale {
            // Branch 2: live but past the stale window → preempt.
            binding.status = "preempted".to_owned();
            binding.released_at_unix_ms = Some(now_unix_ms);
            binding.released_reason = Some(released_reason::PREEMPTED.to_owned());
            let clone = binding.clone();
            let binding_id = binding.binding_id.clone();
            drop(bindings);
            self.persist(&clone)?;
            return Ok(ClaimOutcome::Claimed { binding_id });
        }
        // Branch 3: live and within the window → refuse.
        Ok(ClaimOutcome::RefusedWithinWindow)
    }

    /// Rebuilds the in-memory map from durable rows. Called once at boot from
    /// [`crate::RuntimeService::recover`]. Malformed rows fail closed.
    pub fn recover(&self, persistence: &dyn PersistencePort) -> RuntimeResult<()> {
        let mut rebuilt = BTreeMap::new();
        for (key, value) in persistence.list_records(HOST_EXECUTION_BINDING_V1)? {
            let binding: DurableHostExecutionBinding = serde_json::from_value(value)
                .map_err(|_| malformed("durable host-execution binding is malformed"))?;
            if binding.binding_id != key && binding.delegation_id != key {
                return Err(malformed(
                    "durable host-execution binding identity does not match its key",
                ));
            }
            rebuilt.insert(binding.delegation_id.clone(), binding);
        }
        *self
            .bindings
            .lock()
            .map_err(|_| lock_error("binding ledger lock poisoned"))? = rebuilt;
        Ok(())
    }

    fn persist(&self, binding: &DurableHostExecutionBinding) -> RuntimeResult<()> {
        let guard = self
            .persistence
            .lock()
            .map_err(|_| lock_error("binding ledger persistence lock poisoned"))?;
        let Some(persistence) = guard.as_ref() else {
            // Persistence not wired (unit tests of the ledger struct in
            // isolation). The mutation stays in memory only.
            return Ok(());
        };
        let value = serde_json::to_value(binding).map_err(|_| {
            RuntimeError::new(
                StableErrorCode::ExternalStateConflict,
                "host-execution binding could not be canonicalized",
            )
        })?;
        persistence.store_record(HOST_EXECUTION_BINDING_V1, &binding.binding_id, &value)
    }
}

impl Default for HostExecutionBindingLedger {
    fn default() -> Self {
        Self::empty()
    }
}

/// Applies the 12h hard-timeout sweep in place. Called at the top of every
/// mutator and on the claim path, mirroring `LeaseLedger::expire_if_needed`.
fn expire_if_needed(binding: &mut DurableHostExecutionBinding, now_unix_ms: u64) {
    if binding.is_terminal() {
        return;
    }
    let elapsed = now_unix_ms.saturating_sub(binding.last_interaction_unix_ms);
    if elapsed > HARD_TIMEOUT_MS {
        binding.status = "expired".to_owned();
        binding.released_at_unix_ms = Some(now_unix_ms);
        binding.released_reason = Some(released_reason::EXPIRED.to_owned());
    }
}

fn lock_error(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::ExternalStateConflict, message)
}

fn malformed(message: &str) -> RuntimeError {
    RuntimeError::new(StableErrorCode::ExternalStateConflict, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryPersistence;
    use ae_sdd_domain::EventStoreId;
    use ae_sdd_protocol::StableErrorCode;
    use uuid::Uuid;

    fn ledger_with_memory() -> (
        HostExecutionBindingLedger,
        std::sync::Arc<MemoryPersistence>,
    ) {
        let persistence =
            std::sync::Arc::new(MemoryPersistence::new(EventStoreId::from_uuid(Uuid::nil())));
        let ledger = HostExecutionBindingLedger::empty();
        ledger.attach_persistence(std::sync::Arc::clone(&persistence) as _);
        (ledger, persistence)
    }

    #[test]
    fn spawn_writes_a_spawning_row_keyed_by_delegation() {
        let (ledger, persistence) = ledger_with_memory();
        ledger
            .spawn("bid-1", "ws", "root-1", "del-1", 1_000)
            .expect("spawn persists");
        let row = persistence
            .load_record(HOST_EXECUTION_BINDING_V1, "bid-1")
            .expect("load ok")
            .expect("row exists");
        let binding: DurableHostExecutionBinding = serde_json::from_value(row).expect("decodes");
        assert_eq!(binding.status, "spawning");
        assert_eq!(binding.delegation_id, "del-1");
        assert_eq!(binding.root_session_id, "root-1");
        assert!(binding.active_at_unix_ms.is_none());
    }

    #[test]
    fn spawn_is_idempotent_under_replay_with_the_same_binding_id() {
        let (ledger, _persistence) = ledger_with_memory();
        ledger.spawn("bid", "ws", "root", "del", 1_000).unwrap();
        // Replay: same key — must not overwrite or error.
        ledger.spawn("bid", "ws", "root", "del", 2_000).unwrap();
        let bindings = ledger.bindings.lock().unwrap();
        let binding = bindings.get("del").unwrap();
        assert_eq!(binding.created_at_unix_ms, 1_000);
    }

    #[test]
    fn activate_promotes_spawning_to_active_and_is_idempotent() {
        let (ledger, _persistence) = ledger_with_memory();
        ledger.spawn("bid", "ws", "root", "del", 1_000).unwrap();
        ledger.activate("del", 2_000).unwrap();
        {
            let bindings = ledger.bindings.lock().unwrap();
            let binding = bindings.get("del").unwrap();
            assert_eq!(binding.status, "active");
            assert_eq!(binding.active_at_unix_ms, Some(2_000));
        }
        // Replay activate: no double-stamp.
        ledger.activate("del", 9_999).unwrap();
        let bindings = ledger.bindings.lock().unwrap();
        let binding = bindings.get("del").unwrap();
        assert_eq!(binding.active_at_unix_ms, Some(2_000));
    }

    #[test]
    fn release_by_delegation_is_idempotent_and_noop_on_missing() {
        let (ledger, _persistence) = ledger_with_memory();
        // Missing row: no-op, no error.
        ledger
            .release_by_delegation("ghost", released_reason::COLLECTED, 5_000)
            .unwrap();
        ledger.spawn("bid", "ws", "root", "del", 1_000).unwrap();
        ledger.activate("del", 2_000).unwrap();
        ledger
            .release_by_delegation("del", released_reason::COLLECTED, 3_000)
            .unwrap();
        {
            let bindings = ledger.bindings.lock().unwrap();
            let binding = bindings.get("del").unwrap();
            assert_eq!(binding.status, "released");
            assert_eq!(binding.released_reason.as_deref(), Some("collected"));
        }
        // Release twice: stays released, reason unchanged.
        ledger
            .release_by_delegation("del", released_reason::CANCELLED, 4_000)
            .unwrap();
        let bindings = ledger.bindings.lock().unwrap();
        let binding = bindings.get("del").unwrap();
        assert_eq!(binding.released_reason.as_deref(), Some("collected"));
    }

    #[test]
    fn refresh_interaction_advances_timestamp_and_skips_root_sessions() {
        let (ledger, _persistence) = ledger_with_memory();
        // Root session: no delegation id → no-op.
        ledger.refresh_interaction(None, 5_000).unwrap();
        ledger.spawn("bid", "ws", "root", "del", 1_000).unwrap();
        ledger.activate("del", 2_000).unwrap();
        ledger.refresh_interaction(Some("del"), 7_000).unwrap();
        let bindings = ledger.bindings.lock().unwrap();
        let binding = bindings.get("del").unwrap();
        assert_eq!(binding.last_interaction_unix_ms, 7_000);
    }

    #[test]
    fn hard_timeout_sweeps_to_expired_on_the_next_touch() {
        let (ledger, _persistence) = ledger_with_memory();
        ledger.spawn("bid", "ws", "root", "del", 1_000).unwrap();
        ledger.activate("del", 2_000).unwrap();
        // 12h + 1ms later, the next interaction triggers expiry.
        let later = 2_000 + HARD_TIMEOUT_MS + 1;
        ledger.refresh_interaction(Some("del"), later).unwrap();
        let bindings = ledger.bindings.lock().unwrap();
        let binding = bindings.get("del").unwrap();
        assert_eq!(binding.status, "expired");
        assert_eq!(binding.released_reason.as_deref(), Some("expired"));
    }

    #[test]
    fn claim_or_preempt_branch_one_accepts_a_terminal_binding() {
        let (ledger, _persistence) = ledger_with_memory();
        ledger.spawn("bid", "ws", "root", "del", 1_000).unwrap();
        ledger.activate("del", 2_000).unwrap();
        ledger
            .release_by_delegation("del", released_reason::COLLECTED, 3_000)
            .unwrap();
        let outcome = ledger.claim_or_preempt("del", 4_000).unwrap();
        assert_eq!(
            outcome,
            ClaimOutcome::Claimed {
                binding_id: "bid".to_owned()
            }
        );
    }

    #[test]
    fn claim_or_preempt_branch_two_preempts_a_stale_live_binding() {
        let (ledger, _persistence) = ledger_with_memory();
        ledger.spawn("bid", "ws", "root", "del", 1_000).unwrap();
        ledger.activate("del", 2_000).unwrap();
        let later = 2_000 + STALE_WINDOW_MS + 1;
        let outcome = ledger.claim_or_preempt("del", later).unwrap();
        assert_eq!(
            outcome,
            ClaimOutcome::Claimed {
                binding_id: "bid".to_owned()
            }
        );
        let bindings = ledger.bindings.lock().unwrap();
        let binding = bindings.get("del").unwrap();
        assert_eq!(binding.status, "preempted");
    }

    #[test]
    fn claim_or_preempt_branch_three_refuses_within_the_window() {
        let (ledger, _persistence) = ledger_with_memory();
        ledger.spawn("bid", "ws", "root", "del", 1_000).unwrap();
        ledger.activate("del", 2_000).unwrap();
        let within = 2_000 + STALE_WINDOW_MS - 1;
        let outcome = ledger.claim_or_preempt("del", within).unwrap();
        assert_eq!(outcome, ClaimOutcome::RefusedWithinWindow);
    }

    #[test]
    fn claim_or_preempt_returns_not_found_for_an_unknown_delegation() {
        let (ledger, _persistence) = ledger_with_memory();
        let outcome = ledger.claim_or_preempt("ghost", 1_000).unwrap();
        assert_eq!(outcome, ClaimOutcome::NotFound);
    }

    #[test]
    fn recover_rebuilds_from_durable_rows_and_rejects_malformed() {
        let persistence = MemoryPersistence::new(EventStoreId::from_uuid(Uuid::nil()));
        // Seed a good row directly through persistence.
        let good = DurableHostExecutionBinding::new(
            "bid".to_owned(),
            "ws".to_owned(),
            "root".to_owned(),
            "del".to_owned(),
            1_000,
        );
        let value = serde_json::to_value(&good).unwrap();
        persistence
            .store_record(HOST_EXECUTION_BINDING_V1, "bid", &value)
            .unwrap();
        let ledger = HostExecutionBindingLedger::empty();
        ledger
            .recover(&persistence as &dyn PersistencePort)
            .unwrap();
        let bindings = ledger.bindings.lock().unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings.get("del").unwrap().status, "spawning");
        drop(bindings);
        // Malformed row fails closed.
        let bad =
            serde_json::json!({"schemaVersion":"host-execution-binding/v1","not":"a binding"});
        persistence
            .store_record(HOST_EXECUTION_BINDING_V1, "bad", &bad)
            .unwrap();
        let err = ledger
            .recover(&persistence as &dyn PersistencePort)
            .unwrap_err();
        assert_eq!(err.code(), StableErrorCode::ExternalStateConflict);
    }

    #[test]
    fn multiple_active_bindings_coexist_under_one_root_session() {
        // Regression for Plan §2.4 / revision 1: the same root session may hold
        // several active bindings at once. No unique index on root_session_id
        // exists in the ledger, and the in-memory map is keyed by delegation_id.
        let (ledger, _persistence) = ledger_with_memory();
        ledger.spawn("bid-a", "ws", "root", "del-a", 1_000).unwrap();
        ledger.spawn("bid-b", "ws", "root", "del-b", 1_000).unwrap();
        ledger.activate("del-a", 2_000).unwrap();
        ledger.activate("del-b", 2_000).unwrap();
        let bindings = ledger.bindings.lock().unwrap();
        assert_eq!(
            bindings.values().filter(|b| b.status == "active").count(),
            2
        );
    }
}
