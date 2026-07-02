-- Supabase-Cron-style scheduled jobs, run by the Nubase control plane.
-- Two target types share one scheduler: edge_function (invoked through the
-- functions gateway path) and db_function (a named Postgres function in the
-- tenant schema, called via the PostgREST RPC engine).
--
-- Multi-instance coordination is row-level compare-and-set: a runner claims a
-- due job by advancing next_run_at and taking locked_until in one UPDATE, so
-- no external lock service is needed and overlapping occurrences of a slow job
-- coalesce into a single delayed run.

CREATE TABLE IF NOT EXISTS scheduled_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_ref VARCHAR(128) NOT NULL,
    name VARCHAR(128) NOT NULL,
    description TEXT,
    cron_expression VARCHAR(128) NOT NULL,
    target_type VARCHAR(32) NOT NULL,
    -- edge_function target
    function_slug VARCHAR(128),
    http_method VARCHAR(16),
    request_path TEXT,
    request_body TEXT,
    -- db_function target
    db_function_name VARCHAR(255),
    db_function_args JSONB,
    timeout_seconds INTEGER,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    next_run_at TIMESTAMPTZ,
    locked_until TIMESTAMPTZ,
    last_run_at TIMESTAMPTZ,
    last_status VARCHAR(32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_scheduled_jobs_project_name UNIQUE (project_ref, name),
    CONSTRAINT chk_scheduled_jobs_target CHECK (target_type IN ('edge_function', 'db_function')),
    CONSTRAINT chk_scheduled_jobs_name CHECK (name ~ '^[a-zA-Z0-9_-]{1,128}$')
);

CREATE TABLE IF NOT EXISTS scheduled_job_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id UUID NOT NULL REFERENCES scheduled_jobs(id) ON DELETE CASCADE,
    project_ref VARCHAR(128) NOT NULL,
    job_name VARCHAR(128) NOT NULL,
    target_type VARCHAR(32) NOT NULL,
    scheduled_for TIMESTAMPTZ,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    status VARCHAR(32) NOT NULL,
    result TEXT,
    error_message TEXT
);

-- The runner's due-scan: enabled jobs ordered by next_run_at.
CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_due
    ON scheduled_jobs (next_run_at) WHERE enabled = TRUE;

-- Run-history listing per job and per project, newest first; plus a bare
-- started_at index for retention pruning.
CREATE INDEX IF NOT EXISTS idx_scheduled_job_runs_job_started
    ON scheduled_job_runs (job_id, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_scheduled_job_runs_project_started
    ON scheduled_job_runs (project_ref, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_scheduled_job_runs_started_at
    ON scheduled_job_runs (started_at);
