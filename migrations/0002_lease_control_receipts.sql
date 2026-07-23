ALTER TABLE operation_receipt RENAME TO operation_receipt_v1;

CREATE TABLE operation_receipt (
    workspace_id TEXT NOT NULL,
    work_item_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    payload_digest TEXT NOT NULL CHECK (length(payload_digest) = 64),
    operation TEXT NOT NULL,
    revision_before INTEGER NOT NULL CHECK (revision_before >= 0),
    revision_after INTEGER NOT NULL CHECK (revision_after >= revision_before),
    fencing_token INTEGER NOT NULL CHECK (fencing_token >= 0),
    result_digest TEXT NOT NULL CHECK (length(result_digest) = 64),
    mutation_id TEXT NOT NULL,
    event_seq INTEGER NOT NULL REFERENCES runtime_event(event_seq),
    committed_at TEXT NOT NULL,
    PRIMARY KEY(workspace_id, idempotency_key)
);

INSERT INTO operation_receipt (
    workspace_id,
    work_item_id,
    idempotency_key,
    payload_digest,
    operation,
    revision_before,
    revision_after,
    fencing_token,
    result_digest,
    mutation_id,
    event_seq,
    committed_at
)
SELECT
    workspace_id,
    work_item_id,
    idempotency_key,
    payload_digest,
    operation,
    revision_before,
    revision_after,
    fencing_token,
    result_digest,
    mutation_id,
    event_seq,
    committed_at
FROM operation_receipt_v1;

DROP TABLE operation_receipt_v1;

PRAGMA user_version = 2;
