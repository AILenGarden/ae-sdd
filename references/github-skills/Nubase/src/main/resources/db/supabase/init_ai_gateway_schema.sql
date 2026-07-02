-- Nubase AI Gateway Schema Initialization
-- Creates the per-project AI gateway tables in the 'ai_gateway' schema.
-- One copy lives inside every tenant database, so all rows are naturally scoped to
-- the owning project. Accessed via JPA (entities are @Table(schema="ai_gateway")),
-- NOT exposed through PostgREST.
--
-- Tables:
--   ai_gateway.upstream_configs   : upstream AI providers this project forwards to
--   ai_gateway.api_keys           : self-routing gateway keys (nbk_<appCode>_<secret>)
--   ai_gateway.api_usage_logs     : per-request audit + token/cost log
--   ai_gateway.daily_token_usage  : per-(key, day, model) rolled-up token/cost stats
--   ai_gateway.model_pricing      : per-project editable pricing for cost calculation

-- ==================================================
-- ai_gateway.upstream_configs
-- ==================================================
CREATE TABLE IF NOT EXISTS ai_gateway.upstream_configs
(
    id                    BIGSERIAL PRIMARY KEY,
    name                  VARCHAR(100)  NOT NULL UNIQUE,
    base_url              VARCHAR(500)  NOT NULL,
    auth_token            TEXT          NOT NULL,
    provider              VARCHAR(20)   NOT NULL DEFAULT 'CLAUDE',
    channel_code          VARCHAR(64),
    supported_models      JSONB         NOT NULL DEFAULT '[]'::jsonb,
    chat_completions_path VARCHAR(128)  NOT NULL DEFAULT '/v1/chat/completions',
    description           TEXT,
    is_default            BOOLEAN       NOT NULL DEFAULT false,
    is_active             BOOLEAN       NOT NULL DEFAULT true,
    timeout_ms            INTEGER       NOT NULL DEFAULT 60000,
    max_retries           INTEGER       NOT NULL DEFAULT 3,
    priority              INTEGER       NOT NULL DEFAULT 0,
    max_input_tokens      INTEGER,
    created_at            TIMESTAMP     NOT NULL DEFAULT NOW(),
    updated_at            TIMESTAMP     NOT NULL DEFAULT NOW(),
    last_used_at          TIMESTAMP,
    last_health_check     TIMESTAMP,
    health_status         VARCHAR(20),
    health_message        TEXT
);

CREATE INDEX IF NOT EXISTS upstream_configs_active_priority_idx
    ON ai_gateway.upstream_configs (is_active, priority);
CREATE INDEX IF NOT EXISTS upstream_configs_provider_idx
    ON ai_gateway.upstream_configs (provider);
CREATE INDEX IF NOT EXISTS upstream_configs_channel_idx
    ON ai_gateway.upstream_configs (channel_code);

COMMENT ON TABLE ai_gateway.upstream_configs IS 'Upstream AI providers this project forwards to (priority-ordered, with failover).';

-- ==================================================
-- ai_gateway.api_keys
-- ==================================================
-- Self-routing gateway keys. The full key is nbk_<appCode>_<secret>; only its
-- SHA-256 hash is stored (key_hash). key_prefix is a display-only masked head.
CREATE TABLE IF NOT EXISTS ai_gateway.api_keys
(
    id           BIGSERIAL PRIMARY KEY,
    user_id      UUID REFERENCES auth.users (id) ON DELETE SET NULL,
    api_key      VARCHAR(255),
    key_hash     VARCHAR(64) UNIQUE,
    key_prefix   VARCHAR(32),
    name         VARCHAR(255),
    description  TEXT,
    scope        VARCHAR(64) DEFAULT 'all',
    is_active    BOOLEAN     NOT NULL DEFAULT true,
    expires_at   TIMESTAMP,
    revoked_at   TIMESTAMP,
    last_used_at TIMESTAMP,
    created_at   TIMESTAMP   NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMP   NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS api_keys_active_idx ON ai_gateway.api_keys (is_active);

COMMENT ON TABLE ai_gateway.api_keys IS 'Self-routing gateway keys for this project; lookup by key_hash.';

-- ==================================================
-- ai_gateway.api_usage_logs
-- ==================================================
CREATE TABLE IF NOT EXISTS ai_gateway.api_usage_logs
(
    id                          BIGSERIAL PRIMARY KEY,
    user_id                     UUID REFERENCES auth.users (id) ON DELETE SET NULL,
    api_key_id                  BIGINT,
    api_key                     VARCHAR(255),
    request_id                  VARCHAR(100),
    model                       VARCHAR(100),
    endpoint                    VARCHAR(255),
    method                      VARCHAR(10),
    status_code                 INTEGER,
    input_tokens                INTEGER       DEFAULT 0,
    output_tokens               INTEGER       DEFAULT 0,
    total_tokens                INTEGER       DEFAULT 0,
    cache_creation_input_tokens INTEGER       DEFAULT 0,
    cache_read_input_tokens     INTEGER       DEFAULT 0,
    cost_usd                    NUMERIC(14, 6),
    duration_ms                 BIGINT,
    first_token_latency_ms      BIGINT,
    error_message               TEXT,
    request_metadata            JSONB,
    created_at                  TIMESTAMP     DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS api_usage_logs_api_key_id_created_idx
    ON ai_gateway.api_usage_logs (api_key_id, created_at DESC);
CREATE INDEX IF NOT EXISTS api_usage_logs_created_idx
    ON ai_gateway.api_usage_logs (created_at DESC);
CREATE INDEX IF NOT EXISTS api_usage_logs_request_id_idx
    ON ai_gateway.api_usage_logs (request_id);

COMMENT ON TABLE ai_gateway.api_usage_logs IS 'Per-request audit trail (tokens, cost, latency, errors).';

-- ==================================================
-- ai_gateway.daily_token_usage
-- ==================================================
-- Rolled up by (api_key_id, usage_date, model) via INSERT ... ON CONFLICT.
CREATE TABLE IF NOT EXISTS ai_gateway.daily_token_usage
(
    id                          BIGSERIAL PRIMARY KEY,
    user_id                     UUID REFERENCES auth.users (id) ON DELETE SET NULL,
    api_key_id                  BIGINT,
    usage_date                  DATE          NOT NULL,
    model                       VARCHAR(120)  NOT NULL,
    request_count               INTEGER       NOT NULL DEFAULT 0,
    error_count                 INTEGER       NOT NULL DEFAULT 0,
    input_tokens                BIGINT        NOT NULL DEFAULT 0,
    output_tokens               BIGINT        NOT NULL DEFAULT 0,
    cache_creation_input_tokens BIGINT        NOT NULL DEFAULT 0,
    cache_read_input_tokens     BIGINT        NOT NULL DEFAULT 0,
    total_tokens                BIGINT        NOT NULL DEFAULT 0,
    cost_cny                    NUMERIC(14, 6) NOT NULL DEFAULT 0,
    cost_usd                    NUMERIC(14, 6) NOT NULL DEFAULT 0,
    created_at                  TIMESTAMP     NOT NULL DEFAULT NOW(),
    updated_at                  TIMESTAMP     NOT NULL DEFAULT NOW(),
    CONSTRAINT daily_token_usage_key_date_model_uniq UNIQUE (api_key_id, usage_date, model)
);

CREATE INDEX IF NOT EXISTS daily_token_usage_date_idx
    ON ai_gateway.daily_token_usage (usage_date);

COMMENT ON TABLE ai_gateway.daily_token_usage IS 'Daily token + cost aggregates per (api_key, day, model).';

-- ==================================================
-- ai_gateway.model_pricing
-- ==================================================
CREATE TABLE IF NOT EXISTS ai_gateway.model_pricing
(
    id                              BIGSERIAL PRIMARY KEY,
    model                           VARCHAR(120) NOT NULL,
    provider                        VARCHAR(32)  NOT NULL,
    display_name                    VARCHAR(120),
    input_price_per_1m_cny          NUMERIC(12, 4) NOT NULL DEFAULT 0,
    output_price_per_1m_cny         NUMERIC(12, 4) NOT NULL DEFAULT 0,
    cache_creation_price_per_1m_cny NUMERIC(12, 4) NOT NULL DEFAULT 0,
    cache_read_price_per_1m_cny     NUMERIC(12, 4) NOT NULL DEFAULT 0,
    input_price_per_1m_usd          NUMERIC(12, 4) NOT NULL DEFAULT 0,
    output_price_per_1m_usd         NUMERIC(12, 4) NOT NULL DEFAULT 0,
    cache_creation_price_per_1m_usd NUMERIC(12, 4) NOT NULL DEFAULT 0,
    cache_read_price_per_1m_usd     NUMERIC(12, 4) NOT NULL DEFAULT 0,
    currency                        VARCHAR(8)   NOT NULL DEFAULT 'USD',
    effective_from                  DATE         NOT NULL DEFAULT CURRENT_DATE,
    effective_to                    DATE,
    is_active                       BOOLEAN      NOT NULL DEFAULT true,
    notes                           TEXT,
    quickstart_example              TEXT,
    sort_order                      INTEGER      NOT NULL DEFAULT 0,
    created_at                      TIMESTAMP    NOT NULL DEFAULT NOW(),
    updated_at                      TIMESTAMP    NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS model_pricing_model_active_idx
    ON ai_gateway.model_pricing (model, is_active);

COMMENT ON TABLE ai_gateway.model_pricing IS 'Per-project editable model pricing used to compute token cost (stats only, no billing).';
