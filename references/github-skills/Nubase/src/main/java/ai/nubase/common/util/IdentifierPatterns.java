package ai.nubase.common.util;

/**
 * Single source for identifier-shape regexes shared across modules. The SQL CHECK
 * constraints mirror these and cannot reference them — keep in sync with:
 * V4__edge_functions.sql (chk_edge_function_slug, chk_edge_function_secret_name)
 * and V6__scheduled_jobs.sql (chk_scheduled_jobs_name).
 */
public final class IdentifierPatterns {

    /** Function slugs and scheduled-job names. */
    public static final String RESOURCE_NAME = "^[a-zA-Z0-9_-]{1,128}$";

    /** Function secret/env names. */
    public static final String SECRET_NAME = "^[A-Z_][A-Z0-9_]{0,127}$";

    /** Unquoted Postgres identifiers (db function names, roles). */
    public static final String SQL_IDENTIFIER = "^[a-zA-Z_][a-zA-Z0-9_]{0,127}$";

    private IdentifierPatterns() {
    }
}
