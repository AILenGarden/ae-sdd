CREATE TABLE IF NOT EXISTS schema_migration (
    version INTEGER PRIMARY KEY CHECK (version > 0),
    name TEXT NOT NULL UNIQUE,
    checksum TEXT NOT NULL CHECK (length(checksum) = 64),
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS runtime_identity (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    event_store_id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS workspace (
    workspace_id TEXT PRIMARY KEY,
    canonical_root TEXT NOT NULL,
    project_key TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('legacy','shadow','rust_canary','rust_sole_writer')),
    inventory_generation INTEGER NOT NULL CHECK (inventory_generation >= 0),
    dirty INTEGER NOT NULL CHECK (dirty IN (0,1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(canonical_root, project_key)
);

CREATE TABLE IF NOT EXISTS agent_session (
    session_id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    external_key_hash TEXT NOT NULL CHECK (length(external_key_hash) = 64),
    workspace_id TEXT NOT NULL REFERENCES workspace(workspace_id),
    role TEXT NOT NULL CHECK (role IN ('root','series','task','reviewer')),
    root_session_id TEXT NOT NULL,
    parent_session_id TEXT,
    delegation_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('opening','active','closing','closed','expired','failed')),
    heartbeat_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS agent_session_status_heartbeat
    ON agent_session(status, heartbeat_at);
CREATE UNIQUE INDEX IF NOT EXISTS agent_session_active_external_key
    ON agent_session(workspace_id, external_key_hash)
    WHERE status IN ('opening','active');

CREATE TABLE IF NOT EXISTS turn (
    turn_id TEXT PRIMARY KEY,
    turn_seq INTEGER NOT NULL CHECK (turn_seq >= 0),
    session_id TEXT NOT NULL REFERENCES agent_session(session_id),
    work_item_id TEXT,
    engaged INTEGER NOT NULL CHECK (engaged IN (0,1)),
    capability_hash TEXT NOT NULL CHECK (length(capability_hash) = 64),
    deadline TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(session_id, turn_seq)
);

CREATE TABLE IF NOT EXISTS delegation (
    delegation_id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    root_session_id TEXT NOT NULL,
    parent_session_id TEXT NOT NULL,
    child_session_id TEXT,
    parent_delegation_id TEXT REFERENCES delegation(delegation_id),
    role TEXT NOT NULL CHECK (role IN ('series','task','reviewer')),
    input_revision INTEGER NOT NULL CHECK (input_revision >= 0),
    input_fingerprint TEXT NOT NULL CHECK (length(input_fingerprint) = 64),
    status TEXT NOT NULL,
    deadline TEXT NOT NULL,
    receipt_digest TEXT NOT NULL CHECK (length(receipt_digest) = 64),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS delegation_parent_status_deadline
    ON delegation(parent_delegation_id, status, deadline);

CREATE TABLE IF NOT EXISTS delegation_request_receipt (
    workspace_id TEXT NOT NULL,
    parent_session_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    request_digest TEXT NOT NULL CHECK (length(request_digest) = 64),
    delegation_id TEXT NOT NULL REFERENCES delegation(delegation_id),
    response_digest TEXT NOT NULL CHECK (length(response_digest) = 64),
    created_at TEXT NOT NULL,
    PRIMARY KEY(workspace_id, parent_session_id, idempotency_key)
);

CREATE TABLE IF NOT EXISTS delegation_grant (
    delegation_id TEXT NOT NULL REFERENCES delegation(delegation_id),
    resource_kind TEXT NOT NULL,
    selector TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(delegation_id, resource_kind, selector)
);

CREATE TABLE IF NOT EXISTS child_result (
    delegation_id TEXT PRIMARY KEY REFERENCES delegation(delegation_id),
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    result_digest TEXT NOT NULL CHECK (length(result_digest) = 64),
    byte_len INTEGER NOT NULL CHECK (byte_len >= 0 AND byte_len <= 65536),
    validation_status TEXT NOT NULL,
    artifact_ref TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS memory_cleanup_receipt (
    delegation_id TEXT PRIMARY KEY REFERENCES delegation(delegation_id),
    namespace TEXT NOT NULL,
    snapshot_digest TEXT NOT NULL CHECK (length(snapshot_digest) = 64),
    cleanup_digest TEXT NOT NULL CHECK (length(cleanup_digest) = 64),
    cleaned_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS host_adapter_instance (
    adapter_id TEXT PRIMARY KEY,
    capability_digest TEXT NOT NULL CHECK (length(capability_digest) = 64),
    status TEXT NOT NULL,
    last_command_seq INTEGER NOT NULL CHECK (last_command_seq >= 0),
    heartbeat_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS host_action (
    action_id TEXT PRIMARY KEY,
    adapter_id TEXT NOT NULL REFERENCES host_adapter_instance(adapter_id),
    kind TEXT NOT NULL,
    command_seq INTEGER NOT NULL CHECK (command_seq >= 0),
    request_digest TEXT NOT NULL CHECK (length(request_digest) = 64),
    session_id TEXT,
    context_generation INTEGER CHECK (context_generation >= 0),
    ack_status TEXT NOT NULL,
    ack_id TEXT,
    response_digest TEXT CHECK (response_digest IS NULL OR length(response_digest) = 64),
    deadline TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(adapter_id, command_seq),
    UNIQUE(adapter_id, ack_id)
);

CREATE TABLE IF NOT EXISTS context_pressure_sample (
    adapter_id TEXT NOT NULL REFERENCES host_adapter_instance(adapter_id),
    session_id TEXT NOT NULL,
    context_generation INTEGER NOT NULL CHECK (context_generation >= 0),
    sample_seq INTEGER NOT NULL CHECK (sample_seq >= 0),
    used_tokens INTEGER NOT NULL CHECK (used_tokens >= 0),
    context_window_tokens INTEGER NOT NULL CHECK (context_window_tokens > 0),
    source TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    PRIMARY KEY(adapter_id, session_id, sample_seq)
);
CREATE INDEX IF NOT EXISTS context_pressure_session_generation
    ON context_pressure_sample(session_id, context_generation, sample_seq);

CREATE TABLE IF NOT EXISTS context_projection (
    projection_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    context_revision INTEGER NOT NULL CHECK (context_revision >= 0),
    source_revision INTEGER NOT NULL CHECK (source_revision >= 0),
    policy_digest TEXT NOT NULL CHECK (length(policy_digest) = 64),
    inventory_generation INTEGER NOT NULL CHECK (inventory_generation >= 0),
    digest TEXT NOT NULL CHECK (length(digest) = 64),
    byte_budget INTEGER NOT NULL CHECK (byte_budget >= 0 AND byte_budget <= 65536),
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(session_id, context_revision)
);

CREATE TABLE IF NOT EXISTS compact_cycle (
    compact_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    snapshot_ref TEXT NOT NULL,
    previous_generation INTEGER NOT NULL CHECK (previous_generation >= 0),
    next_generation INTEGER NOT NULL CHECK (next_generation > previous_generation),
    host_action_id TEXT NOT NULL REFERENCES host_action(action_id),
    status TEXT NOT NULL CHECK (status IN ('snapshot_ready','requested','host_acknowledged','context_restored','unsupported','timed_out','failed')),
    deadline TEXT NOT NULL,
    restored_digest TEXT CHECK (restored_digest IS NULL OR length(restored_digest) = 64),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS supervisor_checkpoint (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    last_event_seq INTEGER NOT NULL CHECK (last_event_seq >= 0),
    last_event_digest TEXT NOT NULL CHECK (length(last_event_digest) = 64),
    state_revision INTEGER NOT NULL CHECK (state_revision >= 0),
    input_fingerprint TEXT NOT NULL CHECK (length(input_fingerprint) = 64),
    policy_digest TEXT NOT NULL CHECK (length(policy_digest) = 64),
    last_decision_digest TEXT NOT NULL CHECK (length(last_decision_digest) = 64),
    health TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(workspace_id, work_item_id)
);

CREATE TABLE IF NOT EXISTS runtime_event (
    event_seq INTEGER PRIMARY KEY AUTOINCREMENT,
    event_store_id TEXT NOT NULL,
    boot_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    session_id TEXT,
    work_item_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    payload_json TEXT,
    payload_ref TEXT,
    payload_digest TEXT NOT NULL CHECK (length(payload_digest) = 64),
    byte_len INTEGER NOT NULL CHECK (byte_len >= 0),
    committed_at TEXT NOT NULL,
    CHECK ((payload_json IS NOT NULL AND payload_ref IS NULL) OR
           (payload_json IS NULL AND payload_ref IS NOT NULL))
);
CREATE INDEX IF NOT EXISTS runtime_event_workspace_cursor
    ON runtime_event(workspace_id, event_seq);
CREATE INDEX IF NOT EXISTS runtime_event_work_item_cursor
    ON runtime_event(workspace_id, work_item_id, event_seq);

CREATE TABLE IF NOT EXISTS operation_receipt (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    payload_digest TEXT NOT NULL CHECK (length(payload_digest) = 64),
    operation TEXT NOT NULL,
    revision_before INTEGER NOT NULL CHECK (revision_before >= 0),
    revision_after INTEGER NOT NULL CHECK (revision_after > revision_before),
    fencing_token INTEGER NOT NULL CHECK (fencing_token >= 0),
    result_digest TEXT NOT NULL CHECK (length(result_digest) = 64),
    mutation_id TEXT NOT NULL,
    event_seq INTEGER NOT NULL REFERENCES runtime_event(event_seq),
    committed_at TEXT NOT NULL,
    PRIMARY KEY(workspace_id, idempotency_key)
);

CREATE TABLE IF NOT EXISTS hook_event_receipt (
    session_id TEXT NOT NULL,
    hook_event_id TEXT NOT NULL,
    request_digest TEXT NOT NULL CHECK (length(request_digest) = 64),
    decision_digest TEXT NOT NULL CHECK (length(decision_digest) = 64),
    event_seq INTEGER REFERENCES runtime_event(event_seq),
    created_at TEXT NOT NULL,
    PRIMARY KEY(session_id, hook_event_id)
);

CREATE TABLE IF NOT EXISTS gate_job (
    job_id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    gate_key TEXT NOT NULL,
    status TEXT NOT NULL,
    outcome_digest TEXT CHECK (outcome_digest IS NULL OR length(outcome_digest) = 64),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS gate_job_key_status ON gate_job(gate_key, status);

CREATE TABLE IF NOT EXISTS inventory_entry (
    workspace_id TEXT NOT NULL,
    path TEXT NOT NULL,
    generation INTEGER NOT NULL CHECK (generation >= 0),
    byte_len INTEGER NOT NULL CHECK (byte_len >= 0),
    modified_at TEXT NOT NULL,
    content_digest TEXT NOT NULL CHECK (length(content_digest) = 64),
    PRIMARY KEY(workspace_id, path)
);

PRAGMA user_version = 1;
