-- Part C resource/context read models. The runtime event stream remains durable truth;
-- C1 owns persistence wiring and project-file transaction application.
CREATE TABLE IF NOT EXISTS resource_resolution (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    resource_kind TEXT NOT NULL CHECK (resource_kind <> ''),
    intent TEXT NOT NULL CHECK (intent <> ''),
    winner_path TEXT NOT NULL CHECK (
        winner_path <> '' AND
        substr(winner_path, 1, 1) <> '/' AND
        instr(winner_path, '\\') = 0 AND
        instr(winner_path, '..') = 0
    ),
    winner_digest TEXT NOT NULL CHECK (length(winner_digest) = 64),
    byte_length INTEGER NOT NULL CHECK (byte_length >= 0),
    inventory_generation INTEGER NOT NULL CHECK (inventory_generation >= 0),
    source_layer TEXT NOT NULL CHECK (
        source_layer IN ('declared-override','canonical','legacy')
    ),
    resolution_digest TEXT NOT NULL CHECK (length(resolution_digest) = 64),
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    resolved_at TEXT NOT NULL,
    PRIMARY KEY(workspace_id, work_item_id, resource_id, inventory_generation),
    UNIQUE(workspace_id, work_item_id, resolution_digest)
);
CREATE INDEX IF NOT EXISTS resource_resolution_lookup
    ON resource_resolution(
        workspace_id,
        work_item_id,
        resource_kind,
        intent,
        inventory_generation
    );
CREATE INDEX IF NOT EXISTS resource_resolution_event_cursor
    ON resource_resolution(workspace_id, last_event_seq);

CREATE TABLE IF NOT EXISTS loaded_context_proof (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    story_id TEXT NOT NULL,
    context_id TEXT NOT NULL,
    bundle_digest TEXT NOT NULL CHECK (length(bundle_digest) = 64),
    proof_digest TEXT NOT NULL CHECK (length(proof_digest) = 64),
    story_digest TEXT NOT NULL CHECK (length(story_digest) = 64),
    constraints_digest TEXT NOT NULL CHECK (length(constraints_digest) = 64),
    thinking_digest TEXT NOT NULL CHECK (length(thinking_digest) = 64),
    verification_digest TEXT NOT NULL CHECK (length(verification_digest) = 64),
    methodology_digest TEXT NOT NULL CHECK (length(methodology_digest) = 64),
    state_revision INTEGER NOT NULL CHECK (state_revision >= 0),
    inventory_generation INTEGER NOT NULL CHECK (inventory_generation >= 0),
    computed_at TEXT NOT NULL,
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    PRIMARY KEY(workspace_id, work_item_id, proof_digest),
    UNIQUE(workspace_id, work_item_id, story_id, state_revision, inventory_generation)
);
CREATE INDEX IF NOT EXISTS loaded_context_proof_freshness
    ON loaded_context_proof(
        workspace_id,
        work_item_id,
        story_id,
        state_revision,
        inventory_generation
    );
CREATE INDEX IF NOT EXISTS loaded_context_proof_event_cursor
    ON loaded_context_proof(workspace_id, last_event_seq);

CREATE TABLE IF NOT EXISTS document_transaction_plan (
    workspace_id TEXT NOT NULL,
    transaction_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    input_fingerprint TEXT NOT NULL CHECK (length(input_fingerprint) = 64),
    plan_digest TEXT NOT NULL CHECK (length(plan_digest) = 64),
    status TEXT NOT NULL CHECK (
        status IN ('planned','staged','applied','conflicted','failed')
    ),
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(workspace_id, transaction_id),
    UNIQUE(workspace_id, work_item_id, plan_digest)
);
CREATE INDEX IF NOT EXISTS document_transaction_plan_status
    ON document_transaction_plan(workspace_id, work_item_id, status, updated_at);
CREATE INDEX IF NOT EXISTS document_transaction_plan_event_cursor
    ON document_transaction_plan(workspace_id, last_event_seq);

CREATE TABLE IF NOT EXISTS document_transaction_operation (
    workspace_id TEXT NOT NULL,
    transaction_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('save','finalize')),
    target_path TEXT NOT NULL CHECK (
        target_path <> '' AND
        substr(target_path, 1, 1) <> '/' AND
        instr(target_path, '\\') = 0 AND
        instr(target_path, '..') = 0
    ),
    staged_digest TEXT NOT NULL CHECK (length(staged_digest) = 64),
    expected_before_digest TEXT CHECK (
        expected_before_digest IS NULL OR length(expected_before_digest) = 64
    ),
    byte_length INTEGER NOT NULL CHECK (byte_length > 0 AND byte_length <= 8388608),
    PRIMARY KEY(workspace_id, transaction_id, ordinal),
    UNIQUE(workspace_id, transaction_id, target_path),
    FOREIGN KEY(workspace_id, transaction_id)
        REFERENCES document_transaction_plan(workspace_id, transaction_id)
        ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS document_transaction_operation_target
    ON document_transaction_operation(workspace_id, target_path, transaction_id);

CREATE TABLE IF NOT EXISTS compact_checkpoint (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    compact_id TEXT NOT NULL,
    host_session_id TEXT NOT NULL,
    action_id TEXT NOT NULL,
    adapter_instance_id TEXT NOT NULL,
    previous_generation INTEGER NOT NULL CHECK (previous_generation >= 0),
    next_generation INTEGER NOT NULL CHECK (next_generation = previous_generation + 1),
    request_digest TEXT NOT NULL CHECK (length(request_digest) = 64),
    capsule_digest TEXT NOT NULL CHECK (length(capsule_digest) = 64),
    delta_digest TEXT NOT NULL CHECK (length(delta_digest) = 64),
    restored_digest TEXT CHECK (
        restored_digest IS NULL OR length(restored_digest) = 64
    ),
    status TEXT NOT NULL CHECK (
        status IN (
            'capsule_ready',
            'requested',
            'host_acknowledged',
            'rehydrated',
            'unsupported',
            'timed_out',
            'failed'
        )
    ),
    deadline_unix_ms INTEGER NOT NULL CHECK (deadline_unix_ms >= 0),
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(workspace_id, compact_id),
    UNIQUE(workspace_id, host_session_id, action_id),
    CHECK (status <> 'rehydrated' OR restored_digest IS NOT NULL)
);
CREATE INDEX IF NOT EXISTS compact_checkpoint_status_deadline
    ON compact_checkpoint(workspace_id, status, deadline_unix_ms);
CREATE INDEX IF NOT EXISTS compact_checkpoint_event_cursor
    ON compact_checkpoint(workspace_id, last_event_seq);

PRAGMA user_version = 5;
