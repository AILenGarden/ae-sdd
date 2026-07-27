-- Future C1 read-model contract only. runtime_event and operation_receipt remain authoritative.
-- Part D (Assurance Plane): Verification execution plan, step and receipt projections.
CREATE TABLE IF NOT EXISTS verification_execution_plan_projection (
    workspace_id TEXT NOT NULL,
    execution_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    work_item_id TEXT NOT NULL,
    input_fingerprint TEXT NOT NULL CHECK (length(input_fingerprint) = 64),
    plan_digest TEXT NOT NULL CHECK (length(plan_digest) = 64),
    step_count INTEGER NOT NULL CHECK (step_count > 0),
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    created_at TEXT NOT NULL,
    PRIMARY KEY(workspace_id, execution_id),
    UNIQUE(workspace_id, work_item_id, input_fingerprint),
    UNIQUE(workspace_id, plan_digest)
);
CREATE INDEX IF NOT EXISTS verification_execution_plan_projection_work_item
    ON verification_execution_plan_projection(workspace_id, work_item_id, execution_id);
CREATE INDEX IF NOT EXISTS verification_execution_plan_projection_event_cursor
    ON verification_execution_plan_projection(workspace_id, last_event_seq);

CREATE TABLE IF NOT EXISTS verification_step_projection (
    workspace_id TEXT NOT NULL,
    execution_id TEXT NOT NULL,
    step_ordinal INTEGER NOT NULL CHECK (step_ordinal >= 0),
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    step_id TEXT NOT NULL,
    program_kind TEXT NOT NULL,
    program_path TEXT NOT NULL,
    program_digest TEXT NOT NULL CHECK (length(program_digest) = 64),
    arg_count INTEGER NOT NULL CHECK (arg_count >= 0),
    env_ref_count INTEGER NOT NULL CHECK (env_ref_count >= 0),
    timeout_ms INTEGER NOT NULL CHECK (timeout_ms > 0),
    max_stdout_bytes INTEGER NOT NULL CHECK (max_stdout_bytes > 0),
    max_stderr_bytes INTEGER NOT NULL CHECK (max_stderr_bytes > 0),
    PRIMARY KEY(workspace_id, execution_id, step_ordinal),
    FOREIGN KEY(workspace_id, execution_id)
        REFERENCES verification_execution_plan_projection(workspace_id, execution_id)
);
CREATE INDEX IF NOT EXISTS verification_step_projection_step_id
    ON verification_step_projection(workspace_id, execution_id, step_id);

CREATE TABLE IF NOT EXISTS verification_receipt_projection (
    workspace_id TEXT NOT NULL,
    execution_id TEXT NOT NULL,
    worker_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    work_item_id TEXT NOT NULL,
    input_fingerprint TEXT NOT NULL CHECK (length(input_fingerprint) = 64),
    status TEXT NOT NULL CHECK (
        status IN ('pass','fail','error','timeout','cancelled','stale')
    ),
    exit_code INTEGER,
    stdout_digest TEXT NOT NULL CHECK (length(stdout_digest) = 64),
    stderr_digest TEXT NOT NULL CHECK (length(stderr_digest) = 64),
    started_at_unix_ms INTEGER NOT NULL CHECK (started_at_unix_ms >= 0),
    finished_at_unix_ms INTEGER NOT NULL CHECK (finished_at_unix_ms >= started_at_unix_ms),
    timed_out INTEGER NOT NULL CHECK (timed_out IN (0,1)),
    cancelled INTEGER NOT NULL CHECK (cancelled IN (0,1)),
    receipt_digest TEXT NOT NULL CHECK (length(receipt_digest) = 64),
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    created_at TEXT NOT NULL,
    PRIMARY KEY(workspace_id, execution_id, worker_id),
    UNIQUE(workspace_id, receipt_digest),
    CHECK (
        (status = 'pass') = (
            exit_code = 0 AND timed_out = 0 AND cancelled = 0
        )
    ),
    CHECK (
        (status = 'timeout') = (timed_out = 1)
    ),
    CHECK (
        (status = 'cancelled') = (cancelled = 1)
    ),
    FOREIGN KEY(workspace_id, execution_id)
        REFERENCES verification_execution_plan_projection(workspace_id, execution_id)
);
CREATE INDEX IF NOT EXISTS verification_receipt_projection_status
    ON verification_receipt_projection(workspace_id, execution_id, status);
CREATE INDEX IF NOT EXISTS verification_receipt_projection_event_cursor
    ON verification_receipt_projection(workspace_id, last_event_seq);

PRAGMA user_version = 8;
