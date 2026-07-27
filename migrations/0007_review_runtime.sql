-- Future C1 read-model contract only. runtime_event and operation_receipt remain authoritative.
-- Part D (Assurance Plane): ReviewSupervisor session, round, finding and exit-receipt projections.
CREATE TABLE IF NOT EXISTS review_session_projection (
    workspace_id TEXT NOT NULL,
    review_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    tier TEXT NOT NULL CHECK (tier IN ('tier1','tier2','tier3')),
    status TEXT NOT NULL CHECK (
        status IN ('queued','running','collecting','completed','stalled','invalid_infra','aborted')
    ),
    input_fingerprint TEXT NOT NULL CHECK (length(input_fingerprint) = 64),
    ruleset_fingerprint TEXT NOT NULL CHECK (length(ruleset_fingerprint) = 64),
    round INTEGER NOT NULL CHECK (round > 0),
    clean_streak INTEGER NOT NULL CHECK (clean_streak >= 0),
    max_rounds INTEGER NOT NULL CHECK (max_rounds > 0),
    max_findings INTEGER NOT NULL CHECK (max_findings > 0),
    max_duration_ms INTEGER NOT NULL CHECK (max_duration_ms > 0),
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(workspace_id, review_id),
    UNIQUE(workspace_id, review_id, input_fingerprint)
);
CREATE INDEX IF NOT EXISTS review_session_projection_status
    ON review_session_projection(workspace_id, status, review_id);
CREATE INDEX IF NOT EXISTS review_session_projection_event_cursor
    ON review_session_projection(workspace_id, last_event_seq);

CREATE TABLE IF NOT EXISTS review_round_projection (
    workspace_id TEXT NOT NULL,
    review_id TEXT NOT NULL,
    round INTEGER NOT NULL CHECK (round > 0),
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    started_at_unix_ms INTEGER NOT NULL CHECK (started_at_unix_ms >= 0),
    finished_at_unix_ms INTEGER CHECK (finished_at_unix_ms IS NULL OR finished_at_unix_ms >= started_at_unix_ms),
    finding_count INTEGER NOT NULL CHECK (finding_count >= 0),
    disposition TEXT NOT NULL CHECK (
        disposition IN ('pass','findings','retry','blocked','aborted')
    ),
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    PRIMARY KEY(workspace_id, review_id, round),
    FOREIGN KEY(workspace_id, review_id)
        REFERENCES review_session_projection(workspace_id, review_id)
);
CREATE INDEX IF NOT EXISTS review_round_projection_disposition
    ON review_round_projection(workspace_id, review_id, disposition, round);

CREATE TABLE IF NOT EXISTS review_finding_projection (
    workspace_id TEXT NOT NULL,
    review_id TEXT NOT NULL,
    finding_fingerprint TEXT NOT NULL CHECK (length(finding_fingerprint) = 64),
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    code TEXT NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN ('blocker','major','minor')),
    summary TEXT NOT NULL,
    first_seen_round INTEGER NOT NULL CHECK (first_seen_round > 0),
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    PRIMARY KEY(workspace_id, review_id, finding_fingerprint),
    UNIQUE(workspace_id, finding_fingerprint),
    FOREIGN KEY(workspace_id, review_id)
        REFERENCES review_session_projection(workspace_id, review_id)
);
CREATE INDEX IF NOT EXISTS review_finding_projection_severity
    ON review_finding_projection(workspace_id, review_id, severity);

CREATE TABLE IF NOT EXISTS review_exit_receipt_projection (
    workspace_id TEXT NOT NULL,
    review_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    disposition TEXT NOT NULL CHECK (disposition IN ('pass','findings','retry','blocked','aborted')),
    session_status TEXT NOT NULL CHECK (
        session_status IN ('completed','stalled','invalid_infra','aborted')
    ),
    session_input_fingerprint TEXT NOT NULL CHECK (length(session_input_fingerprint) = 64),
    observed_input_fingerprint TEXT NOT NULL CHECK (length(observed_input_fingerprint) = 64),
    ruleset_fingerprint TEXT NOT NULL CHECK (length(ruleset_fingerprint) = 64),
    round INTEGER NOT NULL CHECK (round > 0),
    finding_count INTEGER NOT NULL CHECK (finding_count >= 0),
    receipt_digest TEXT NOT NULL CHECK (length(receipt_digest) = 64),
    last_event_seq INTEGER REFERENCES runtime_event(event_seq),
    created_at TEXT NOT NULL,
    PRIMARY KEY(workspace_id, review_id),
    UNIQUE(workspace_id, review_id, receipt_digest),
    FOREIGN KEY(workspace_id, review_id)
        REFERENCES review_session_projection(workspace_id, review_id)
);
CREATE INDEX IF NOT EXISTS review_exit_receipt_projection_pass
    ON review_exit_receipt_projection(workspace_id, disposition, review_id);

PRAGMA user_version = 7;
