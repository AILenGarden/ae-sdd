-- Part B host/session bootstrap persistence contract. This migration freezes
-- the schema C1 will use to persist `session.bootstrap` composition plans; it
-- is not applied by any store today (no `include_str!` reference exists yet).
-- C1 owns wiring this into `ae-sdd-store` and actually applying it.
CREATE TABLE IF NOT EXISTS host_session_bootstrap (
    workspace_id TEXT NOT NULL REFERENCES workspace(workspace_id),
    external_session_key_hash TEXT NOT NULL CHECK (length(external_session_key_hash) = 64),
    plan_digest TEXT NOT NULL CHECK (length(plan_digest) = 64),
    steps_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (workspace_id, external_session_key_hash)
);

PRAGMA user_version = 6;
