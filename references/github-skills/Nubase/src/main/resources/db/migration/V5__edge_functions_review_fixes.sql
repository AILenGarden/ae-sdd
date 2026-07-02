-- Edge functions review fixes: align indexes with the queries the admin plane
-- actually runs, and drop columns that no code path ever writes.

-- (project_ref, slug) lookups are already served by the index backing
-- uq_edge_functions_project_slug; the explicit index was a duplicate that only
-- taxed writes.
DROP INDEX IF EXISTS idx_edge_functions_project_slug;

-- Platform-user attribution is not available on the functions admin plane (it
-- authenticates with the service_role apikey only), so these were always NULL.
DROP INDEX IF EXISTS idx_edge_functions_project_created_by;
ALTER TABLE edge_function_invocations DROP COLUMN IF EXISTS caller_type;
ALTER TABLE edge_function_invocations DROP COLUMN IF EXISTS caller_platform_user_id;

-- No query filters by caller_user_id, and putting it before created_at prevented
-- Postgres from reading recent-invocations queries in index order. Replace the
-- caller-leading indexes with the shape listInvocations uses.
DROP INDEX IF EXISTS idx_edge_invocations_project_caller;
DROP INDEX IF EXISTS idx_edge_invocations_project_function_caller;
CREATE INDEX IF NOT EXISTS idx_edge_invocations_project_function_created
    ON edge_function_invocations (project_ref, function_slug, created_at DESC);

-- Retention pruning deletes by created_at alone.
CREATE INDEX IF NOT EXISTS idx_edge_invocations_created_at
    ON edge_function_invocations (created_at);
