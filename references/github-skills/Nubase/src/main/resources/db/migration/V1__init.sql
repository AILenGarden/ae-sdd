-- nubase metadata DB — consolidated initial schema.
--
-- This single migration captures every table the open-source release needs in the
-- metadata DB. It is intentionally consolidated: prior to this commit the codebase
-- had a dozen fragmented migrations plus an un-versioned unified_multitenancy.sql
-- that Flyway never picked up, leading to "relation does not exist" errors at boot.
--
-- All statements use IF NOT EXISTS / ADD COLUMN IF NOT EXISTS where possible so the
-- file is safe to re-run during local development. Future schema changes should be
-- added as V2__..., V3__... — never modify this file in place.

-- ============================================================================
-- 1. Tenant routing — the heart of nubase's database-per-tenant model.
--    Each row maps an app_code/db_key to a real Postgres connection with its
--    own JWT secret and OAuth config.
-- ============================================================================
CREATE TABLE IF NOT EXISTS database_configs (
    db_key                          VARCHAR(50)  NOT NULL,
    db_name                         VARCHAR(100) NOT NULL,
    description                     TEXT,
    jdbc_url                        TEXT         NOT NULL,
    db_user                         VARCHAR(100) NOT NULL,
    db_password_encrypted           TEXT         NOT NULL,
    db_schemas                      TEXT[]       DEFAULT ARRAY['public'::text],
    db_anon_role                    VARCHAR(100) DEFAULT 'anonymous',
    db_max_rows                     INTEGER,
    db_extra_search_path            TEXT[],
    jwt_secret_encrypted            TEXT,
    jwt_secret_is_base64            BOOLEAN      DEFAULT FALSE,
    jwt_audience                    VARCHAR(255),
    jwt_role_claim_key              VARCHAR(100) DEFAULT '.role',
    pool_size                       INTEGER      DEFAULT 10,
    pool_timeout_ms                 INTEGER      DEFAULT 10000,
    pool_max_lifetime_ms            INTEGER      DEFAULT 1800000,
    pool_connection_timeout_ms      INTEGER      DEFAULT 30000,
    pool_idle_timeout_ms            INTEGER      DEFAULT 600000,
    pool_minimum_idle               INTEGER      DEFAULT 2,
    enabled                         BOOLEAN      DEFAULT TRUE,
    created_at                      TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    updated_at                      TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    created_by                      VARCHAR(100),
    updated_by                      VARCHAR(100),
    last_health_check               TIMESTAMP,
    health_status                   VARCHAR(20),
    health_message                  TEXT,
    app_code                        VARCHAR(100),
    app_name                        VARCHAR(255),
    schema_name                     VARCHAR(100),
    jwt_secret                      TEXT,
    service_role_token              TEXT,
    authenticated_token             TEXT,
    oauth_config                    TEXT,
    -- Physical database initialization tracking: pending_init / initializing /
    -- initialized / init_failed. The row exists as soon as the project is created,
    -- but the underlying database might not be ready yet.
    init_status                     VARCHAR(32),
    init_message                    TEXT,
    init_started_at                 TIMESTAMP,
    init_completed_at               TIMESTAMP,
    CONSTRAINT database_configs_pkey PRIMARY KEY (db_key)
);
CREATE INDEX IF NOT EXISTS idx_database_configs_db_key   ON database_configs (db_key);
CREATE INDEX IF NOT EXISTS idx_database_configs_app_code ON database_configs (app_code);

-- ============================================================================
-- 2. SQL execution log — every ad-hoc SQL run through the Studio.
-- ============================================================================
CREATE TABLE IF NOT EXISTS sql_execution_records (
    id                  BIGSERIAL PRIMARY KEY,
    app_code            VARCHAR(255),
    database_key        VARCHAR(255),
    schema_name         VARCHAR(255),
    sql_query           TEXT      NOT NULL,
    success             BOOLEAN   NOT NULL,
    execution_time_ms   BIGINT,
    execution_result    TEXT,
    error_message       TEXT,
    error_stack_trace   TEXT,
    created_at          TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_sql_execution_records_app_code  ON sql_execution_records (app_code);
CREATE INDEX IF NOT EXISTS idx_sql_execution_records_created_at ON sql_execution_records (created_at DESC);

-- ============================================================================
-- 3. Platform users — Studio login accounts; distinct from per-tenant auth.users.
--    super_admin role sees all projects; user role only sees projects they own.
-- ============================================================================
CREATE TABLE IF NOT EXISTS platform_users (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email                VARCHAR(255) NOT NULL UNIQUE,
    encrypted_password   VARCHAR(255) NOT NULL,
    full_name            VARCHAR(255),
    role                 VARCHAR(32)  NOT NULL DEFAULT 'user',
    is_active            BOOLEAN      NOT NULL DEFAULT TRUE,
    last_signed_in_at    TIMESTAMPTZ,
    created_at           TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_platform_users_email ON platform_users (email);

CREATE TABLE IF NOT EXISTS platform_user_projects (
    id          BIGSERIAL PRIMARY KEY,
    user_id     UUID         NOT NULL REFERENCES platform_users (id) ON DELETE CASCADE,
    db_key      VARCHAR(255) NOT NULL,
    role        VARCHAR(32)  NOT NULL DEFAULT 'owner',
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_platform_user_projects UNIQUE (user_id, db_key)
);
CREATE INDEX IF NOT EXISTS idx_pup_user  ON platform_user_projects (user_id);
CREATE INDEX IF NOT EXISTS idx_pup_dbkey ON platform_user_projects (db_key);

-- ============================================================================
-- 4. SQL snippets — saved per platform user, scoped to a project (db_key).
-- ============================================================================
CREATE TABLE IF NOT EXISTS sql_snippets (
    id                BIGSERIAL PRIMARY KEY,
    platform_user_id  UUID         NOT NULL REFERENCES platform_users (id) ON DELETE CASCADE,
    db_key            VARCHAR(255) NOT NULL,
    name              VARCHAR(255) NOT NULL,
    query             TEXT         NOT NULL,
    created_at        TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_sql_snippets_user_db     ON sql_snippets (platform_user_id, db_key);
CREATE INDEX IF NOT EXISTS idx_sql_snippets_updated_at  ON sql_snippets (updated_at DESC);

-- ============================================================================
-- 5. Platform settings — runtime-editable configuration (SMTP, R2, OAuth,
--    LLM provider keys, ...). Sensitive values are AES-256-GCM encrypted via
--    EncryptionService using the master key. Read-through cache in
--    PlatformSettingsService; SettingsChangedEvent invalidates on PUT.
--
--    This table is empty on a fresh install. Consumers must fall back to
--    YAML / env defaults so the server boots before any setting exists.
-- ============================================================================
CREATE TABLE IF NOT EXISTS platform_settings (
    category    VARCHAR(64)  NOT NULL,
    key         VARCHAR(128) NOT NULL,
    value       TEXT,
    encrypted   BOOLEAN      NOT NULL DEFAULT FALSE,
    updated_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_by  UUID,
    PRIMARY KEY (category, key)
);
CREATE INDEX IF NOT EXISTS idx_platform_settings_category ON platform_settings (category);
