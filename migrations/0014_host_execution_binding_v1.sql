-- 0014 host_execution_binding_v1: UUID host-execution binding ledger.
--
-- ae-sdd-daemon-design.md §9.4: daemon tracks the liveness of each host-bound
-- delegation by a daemon-minted UUID (HostExecutionBindingId), not by PID.
-- One row per delegation; the binding transitions spawning -> active -> one of
-- released/preempted/expired. Preemption (branch 2 of §1.4) flips the row to
-- 'preempted' rather than deleting it, so the audit trail survives. The unique
-- constraint is on delegation_id (one delegation = one binding); there is NO
-- root_session_id unique index, because the same root session may hold several
-- active bindings at once (Plan §2.4 correction to the earlier draft).
CREATE TABLE IF NOT EXISTS host_execution_binding_v1 (
    binding_id               TEXT PRIMARY KEY,
    workspace_id             TEXT NOT NULL REFERENCES workspace(workspace_id),
    root_session_id          TEXT NOT NULL,
    delegation_id            TEXT NOT NULL UNIQUE,
    status                   TEXT NOT NULL CHECK (status IN
                                 ('spawning','active','released','preempted','expired')),
    created_at_unix_ms       INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    last_interaction_unix_ms INTEGER NOT NULL CHECK (last_interaction_unix_ms >= 0),
    active_at_unix_ms        INTEGER CHECK (active_at_unix_ms IS NULL OR active_at_unix_ms >= 0),
    released_at_unix_ms      INTEGER CHECK (released_at_unix_ms IS NULL OR released_at_unix_ms >= 0),
    released_reason          TEXT CHECK (released_reason IS NULL OR released_reason IN
                                 ('session-closed','collected','cancelled','expired','preempted')),
    CHECK ((status IN ('spawning','active')) = (released_at_unix_ms IS NULL)),
    CHECK ((status = 'active') = (active_at_unix_ms IS NOT NULL)),
    CHECK ((released_at_unix_ms IS NULL) = (released_reason IS NULL))
);
PRAGMA user_version=14;
