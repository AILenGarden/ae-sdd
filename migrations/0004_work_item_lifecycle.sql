-- Future C1 read-model contract only. runtime_event and operation_receipt remain authoritative.
CREATE TABLE IF NOT EXISTS lifecycle_plan_projection (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    command_digest TEXT NOT NULL CHECK (length(command_digest) = 64),
    plan_digest TEXT NOT NULL CHECK (length(plan_digest) = 64),
    input_fingerprint TEXT NOT NULL CHECK (length(input_fingerprint) = 64),
    expected_revision INTEGER NOT NULL CHECK (expected_revision >= 0),
    disposition TEXT NOT NULL CHECK (
        disposition IN ('permitted','denied','awaiting_confirmation')
    ),
    confirmation_required INTEGER NOT NULL CHECK (confirmation_required IN (0,1)),
    confirmation_binding_digest TEXT NOT NULL CHECK (
        length(confirmation_binding_digest) = 64
    ),
    confirmation_id TEXT,
    policy_digest TEXT NOT NULL CHECK (length(policy_digest) = 64),
    applied_event_seq INTEGER REFERENCES runtime_event(event_seq),
    created_at TEXT NOT NULL,
    PRIMARY KEY(workspace_id, work_item_id, plan_digest),
    UNIQUE(
        workspace_id,
        work_item_id,
        expected_revision,
        command_digest,
        input_fingerprint
    )
);
CREATE INDEX IF NOT EXISTS lifecycle_plan_projection_disposition
    ON lifecycle_plan_projection(workspace_id, work_item_id, disposition, expected_revision);
CREATE INDEX IF NOT EXISTS lifecycle_plan_projection_event_cursor
    ON lifecycle_plan_projection(workspace_id, applied_event_seq);

CREATE TABLE IF NOT EXISTS lifecycle_evidence_ref (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    plan_digest TEXT NOT NULL,
    evidence_id TEXT NOT NULL,
    verification_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    digest TEXT NOT NULL CHECK (length(digest) = 64),
    byte_length INTEGER NOT NULL CHECK (byte_length >= 0),
    PRIMARY KEY(workspace_id, work_item_id, plan_digest, evidence_id),
    FOREIGN KEY(workspace_id, work_item_id, plan_digest)
        REFERENCES lifecycle_plan_projection(workspace_id, work_item_id, plan_digest)
);
CREATE INDEX IF NOT EXISTS lifecycle_evidence_ref_verification
    ON lifecycle_evidence_ref(workspace_id, work_item_id, verification_id);

CREATE TABLE IF NOT EXISTS lifecycle_mutation_intent (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    plan_digest TEXT NOT NULL,
    intent_ordinal INTEGER NOT NULL CHECK (intent_ordinal >= 0),
    intent_id TEXT NOT NULL,
    target_namespace TEXT NOT NULL,
    target_relative_path TEXT,
    target_logical_key TEXT,
    operation TEXT NOT NULL CHECK (
        operation IN ('create','replace','delete','append_event')
    ),
    expected_revision INTEGER NOT NULL CHECK (expected_revision >= 0),
    expected_digest TEXT CHECK (expected_digest IS NULL OR length(expected_digest) = 64),
    event_kind TEXT NOT NULL,
    event_payload_digest TEXT NOT NULL CHECK (length(event_payload_digest) = 64),
    application_status TEXT NOT NULL CHECK (
        application_status IN ('planned','applied','rejected','superseded')
    ),
    applied_operation_event_seq INTEGER REFERENCES runtime_event(event_seq),
    PRIMARY KEY(workspace_id, work_item_id, plan_digest, intent_ordinal),
    UNIQUE(workspace_id, intent_id),
    CHECK (
        (target_relative_path IS NOT NULL AND target_logical_key IS NULL) OR
        (target_relative_path IS NULL AND target_logical_key IS NOT NULL)
    ),
    FOREIGN KEY(workspace_id, work_item_id, plan_digest)
        REFERENCES lifecycle_plan_projection(workspace_id, work_item_id, plan_digest)
);
CREATE INDEX IF NOT EXISTS lifecycle_mutation_intent_application
    ON lifecycle_mutation_intent(workspace_id, application_status, applied_operation_event_seq);

CREATE TABLE IF NOT EXISTS story_lifecycle_projection (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    story_id TEXT NOT NULL,
    registered INTEGER NOT NULL CHECK (registered IN (0,1)),
    document_path TEXT,
    phase TEXT NOT NULL,
    pending_outputs INTEGER NOT NULL CHECK (pending_outputs >= 0),
    coding_round INTEGER NOT NULL CHECK (coding_round >= 0),
    completed INTEGER NOT NULL CHECK (completed IN (0,1)),
    source_plan_digest TEXT NOT NULL CHECK (length(source_plan_digest) = 64),
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    PRIMARY KEY(workspace_id, work_item_id, story_id)
);
CREATE INDEX IF NOT EXISTS story_lifecycle_projection_completion
    ON story_lifecycle_projection(workspace_id, work_item_id, registered, completed);

CREATE TABLE IF NOT EXISTS prd_completion_projection (
    workspace_id TEXT NOT NULL,
    prd_id TEXT NOT NULL,
    registered_story_count INTEGER NOT NULL CHECK (registered_story_count > 0),
    completed_story_count INTEGER NOT NULL CHECK (
        completed_story_count >= 0 AND completed_story_count <= registered_story_count
    ),
    dependencies_satisfied INTEGER NOT NULL CHECK (dependencies_satisfied IN (0,1)),
    residual_risks_cleared INTEGER NOT NULL CHECK (residual_risks_cleared IN (0,1)),
    gates_passed INTEGER NOT NULL CHECK (gates_passed IN (0,1)),
    review_passed INTEGER NOT NULL CHECK (review_passed IN (0,1)),
    completed INTEGER NOT NULL CHECK (completed IN (0,1)),
    source_plan_digest TEXT NOT NULL CHECK (length(source_plan_digest) = 64),
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    PRIMARY KEY(workspace_id, prd_id),
    CHECK (
        completed = 0 OR (
            completed_story_count = registered_story_count AND
            dependencies_satisfied = 1 AND
            residual_risks_cleared = 1 AND
            gates_passed = 1 AND
            review_passed = 1
        )
    )
);
CREATE INDEX IF NOT EXISTS prd_completion_projection_pending
    ON prd_completion_projection(workspace_id, completed, prd_id);

CREATE TABLE IF NOT EXISTS prd_child_completion_projection (
    workspace_id TEXT NOT NULL,
    prd_id TEXT NOT NULL,
    story_id TEXT NOT NULL,
    completed INTEGER NOT NULL CHECK (completed IN (0,1)),
    completion_revision INTEGER NOT NULL CHECK (completion_revision >= 0),
    completion_plan_digest TEXT NOT NULL CHECK (length(completion_plan_digest) = 64),
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    PRIMARY KEY(workspace_id, prd_id, story_id),
    FOREIGN KEY(workspace_id, prd_id)
        REFERENCES prd_completion_projection(workspace_id, prd_id)
);
CREATE INDEX IF NOT EXISTS prd_child_completion_projection_pending
    ON prd_child_completion_projection(workspace_id, prd_id, completed, story_id);

CREATE TABLE IF NOT EXISTS work_item_file_lock_projection (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    owner_session_id TEXT NOT NULL,
    expires_at_unix_ms INTEGER NOT NULL CHECK (expires_at_unix_ms >= 0),
    metadata_valid INTEGER NOT NULL CHECK (metadata_valid IN (0,1)),
    source_plan_digest TEXT NOT NULL CHECK (length(source_plan_digest) = 64),
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    PRIMARY KEY(workspace_id, work_item_id, relative_path)
);
CREATE INDEX IF NOT EXISTS work_item_file_lock_projection_owner_expiry
    ON work_item_file_lock_projection(workspace_id, owner_session_id, expires_at_unix_ms);

PRAGMA user_version = 4;
