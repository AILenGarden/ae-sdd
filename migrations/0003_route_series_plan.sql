-- Future C1 read-model contract only. runtime_event remains the durable event truth.
CREATE TABLE IF NOT EXISTS route_decision_projection (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    route_revision INTEGER NOT NULL CHECK (route_revision >= 0),
    input_fingerprint TEXT NOT NULL CHECK (length(input_fingerprint) = 64),
    decision_digest TEXT NOT NULL CHECK (length(decision_digest) = 64),
    disposition TEXT NOT NULL CHECK (
        disposition IN ('await_user_approval','approved','denied','superseded')
    ),
    scale TEXT NOT NULL CHECK (scale IN ('large','medium','small','micro')),
    design_route TEXT NOT NULL CHECK (design_route IN ('dr','story','coding_plan')),
    approval_binding_digest TEXT CHECK (
        approval_binding_digest IS NULL OR length(approval_binding_digest) = 64
    ),
    approval_confirmation_id TEXT,
    superseded_by_digest TEXT CHECK (
        superseded_by_digest IS NULL OR length(superseded_by_digest) = 64
    ),
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(workspace_id, work_item_id, route_revision),
    UNIQUE(workspace_id, work_item_id, route_revision, input_fingerprint),
    UNIQUE(workspace_id, work_item_id, decision_digest)
);
CREATE UNIQUE INDEX IF NOT EXISTS route_decision_projection_one_active
    ON route_decision_projection(
        workspace_id,
        work_item_id,
        route_revision,
        input_fingerprint
    )
    WHERE disposition IN ('await_user_approval','approved');
CREATE INDEX IF NOT EXISTS route_decision_projection_event_cursor
    ON route_decision_projection(workspace_id, last_event_seq);

CREATE TABLE IF NOT EXISTS series_plan_projection (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    series_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    route_revision INTEGER NOT NULL CHECK (route_revision >= 0),
    input_fingerprint TEXT NOT NULL CHECK (length(input_fingerprint) = 64),
    series_kind TEXT NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN (
            'planned',
            'running',
            'result_staged',
            'collected',
            'cancelled',
            'failed'
        )
    ),
    plan_digest TEXT NOT NULL CHECK (length(plan_digest) = 64),
    methodology_digest TEXT NOT NULL CHECK (length(methodology_digest) = 64),
    result_ref TEXT,
    result_digest TEXT CHECK (result_digest IS NULL OR length(result_digest) = 64),
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(workspace_id, series_id),
    UNIQUE(workspace_id, work_item_id, plan_digest),
    FOREIGN KEY(workspace_id, work_item_id, route_revision, input_fingerprint)
        REFERENCES route_decision_projection(
            workspace_id,
            work_item_id,
            route_revision,
            input_fingerprint
        )
);
CREATE INDEX IF NOT EXISTS series_plan_projection_work_item_status
    ON series_plan_projection(workspace_id, work_item_id, status, series_id);
CREATE INDEX IF NOT EXISTS series_plan_projection_event_cursor
    ON series_plan_projection(workspace_id, last_event_seq);

PRAGMA user_version = 3;
