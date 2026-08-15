CREATE TABLE IF NOT EXISTS series_review_profile_v1 (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    series_id TEXT NOT NULL,
    series_kind TEXT NOT NULL CHECK (series_kind IN ('requirement_analysis','design_review','story','coding','test')),
    profile_digest TEXT NOT NULL,
    methodology_ref TEXT NOT NULL,
    context_selectors_json TEXT NOT NULL CHECK (json_valid(context_selectors_json) AND json_type(context_selectors_json) = 'array'),
    skill_refs_json TEXT NOT NULL CHECK (json_valid(skill_refs_json) AND json_type(skill_refs_json) = 'array'),
    reviewer_specialties_json TEXT NOT NULL CHECK (json_valid(reviewer_specialties_json) AND json_type(reviewer_specialties_json) = 'array'),
    gate_refs_json TEXT NOT NULL CHECK (json_valid(gate_refs_json) AND json_type(gate_refs_json) = 'array'),
    artifact_refs_json TEXT NOT NULL CHECK (json_valid(artifact_refs_json) AND json_type(artifact_refs_json) = 'array'),
    created_at TEXT NOT NULL,
    PRIMARY KEY (workspace_id, work_item_id, series_id, profile_digest)
) STRICT;

CREATE TABLE IF NOT EXISTS series_review_dependency_v1 (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    series_id TEXT NOT NULL,
    profile_digest TEXT NOT NULL,
    upstream_series_id TEXT NOT NULL,
    dependency_ordinal INTEGER NOT NULL CHECK (dependency_ordinal >= 0),
    PRIMARY KEY (workspace_id, work_item_id, series_id, profile_digest, upstream_series_id),
    UNIQUE (workspace_id, work_item_id, series_id, profile_digest, dependency_ordinal),
    FOREIGN KEY (workspace_id, work_item_id, series_id, profile_digest)
        REFERENCES series_review_profile_v1(workspace_id, work_item_id, series_id, profile_digest)
        ON DELETE CASCADE
) STRICT;

CREATE TABLE IF NOT EXISTS series_review_run_v1 (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    series_id TEXT NOT NULL,
    series_run_id TEXT NOT NULL,
    review_run_id TEXT NOT NULL,
    series_kind TEXT NOT NULL CHECK (series_kind IN ('requirement_analysis','design_review','story','coding','test')),
    source_revision INTEGER NOT NULL CHECK (source_revision >= 0),
    input_fingerprint TEXT NOT NULL,
    profile_digest TEXT NOT NULL,
    dependency_digest TEXT NOT NULL,
    parent_review_run_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('queued','running','collecting','rework_required','closed','stale','blocked','aborted')),
    request_digest TEXT NOT NULL,
    result_digest TEXT,
    last_event_sequence INTEGER NOT NULL DEFAULT 0 CHECK (last_event_sequence >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (workspace_id, work_item_id, series_id, series_run_id, review_run_id),
    UNIQUE (review_run_id),
    UNIQUE (
        workspace_id, work_item_id, series_id, series_run_id, review_run_id,
        series_kind, source_revision, input_fingerprint, profile_digest, dependency_digest
    ),
    FOREIGN KEY (workspace_id, work_item_id, series_id, profile_digest)
        REFERENCES series_review_profile_v1(workspace_id, work_item_id, series_id, profile_digest)
) STRICT;

CREATE TABLE IF NOT EXISTS series_review_receipt_v1 (
    receipt_id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    series_id TEXT NOT NULL,
    series_run_id TEXT NOT NULL,
    review_run_id TEXT NOT NULL,
    series_kind TEXT NOT NULL CHECK (series_kind IN ('requirement_analysis','design_review','story','coding','test')),
    source_revision INTEGER NOT NULL CHECK (source_revision >= 0),
    input_fingerprint TEXT NOT NULL,
    profile_digest TEXT NOT NULL,
    dependency_digest TEXT NOT NULL,
    evidence_digest TEXT,
    gate_digest TEXT,
    status TEXT NOT NULL CHECK (status IN ('closed','rework_required','stale','blocked','aborted')),
    stale_reasons_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(stale_reasons_json) AND json_type(stale_reasons_json) = 'array'),
    event_sequence INTEGER NOT NULL CHECK (event_sequence >= 0),
    receipt_digest TEXT NOT NULL,
    closed_at TEXT NOT NULL,
    updated_event_sequence INTEGER NOT NULL DEFAULT 0 CHECK (updated_event_sequence >= 0),
    UNIQUE (
        workspace_id, work_item_id, series_id, series_run_id, review_run_id,
        series_kind, source_revision, input_fingerprint
    ),
    FOREIGN KEY (
        workspace_id, work_item_id, series_id, series_run_id, review_run_id,
        series_kind, source_revision, input_fingerprint, profile_digest, dependency_digest
    ) REFERENCES series_review_run_v1(
        workspace_id, work_item_id, series_id, series_run_id, review_run_id,
        series_kind, source_revision, input_fingerprint, profile_digest, dependency_digest
    )
) STRICT;

CREATE TABLE IF NOT EXISTS series_review_finding_v1 (
    receipt_id TEXT NOT NULL,
    finding_fingerprint TEXT NOT NULL,
    finding_ordinal INTEGER NOT NULL CHECK (finding_ordinal >= 0),
    code TEXT NOT NULL,
    severity TEXT NOT NULL,
    finding_scope TEXT NOT NULL,
    target_series_id TEXT NOT NULL,
    summary TEXT NOT NULL,
    PRIMARY KEY (receipt_id, finding_fingerprint),
    UNIQUE (receipt_id, finding_ordinal),
    FOREIGN KEY (receipt_id) REFERENCES series_review_receipt_v1(receipt_id) ON DELETE CASCADE
) STRICT;

CREATE INDEX IF NOT EXISTS idx_series_review_receipt_governance_v1
    ON series_review_receipt_v1(workspace_id, work_item_id, series_kind, status, source_revision);

CREATE INDEX IF NOT EXISTS idx_series_review_dependency_upstream_v1
    ON series_review_dependency_v1(workspace_id, work_item_id, upstream_series_id);

PRAGMA user_version = 15;
