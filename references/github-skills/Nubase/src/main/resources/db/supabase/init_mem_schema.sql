-- Nubase Memory Schema Initialization
-- Creates AI memory tables in the 'mem' schema using pgvector
-- Three tables:
--   mem.memories         : fact-level memory items with embeddings (pgvector)
--   mem.memory_history   : audit trail of ADD/UPDATE/DELETE events
--   mem.session_messages : short-term conversation window (recent N messages)

-- pgvector extension is created by the super-user step in DatabaseInitService.initSuperExtensions().
-- This script assumes the 'vector' type is already available.

-- ==================================================
-- mem.memories : primary memory storage
-- ==================================================
CREATE TABLE IF NOT EXISTS mem.memories
(
    id          UUID PRIMARY KEY                  DEFAULT gen_random_uuid(),

    -- Owner triple (any one is required; enforced at app layer).
    -- user_id has FK + CASCADE so deleting the auth.users row also clears their
    -- memories. agent_id / run_id are app-layer identifiers (no FK target).
    user_id     UUID REFERENCES auth.users(id) ON DELETE CASCADE,
    agent_id    VARCHAR(128),
    run_id      VARCHAR(128),

    -- Memory content
    -- Embedding dimension is injected by DatabaseInitService from
    -- nubase.mem.embedding.dimensions (default 1536, validated against a whitelist).
    memory      TEXT             NOT NULL,
    embedding   vector(${embedding_dimensions}),
    metadata    JSONB                             DEFAULT CAST('{}' AS JSONB),

    -- Who said it (for actor tracking inside conversations)
    actor_id    VARCHAR(128),
    role        VARCHAR(32),

    -- Deduplication hash of the normalized fact text
    hash        VARCHAR(64),

    -- Timestamps + soft delete
    created_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at  TIMESTAMP WITH TIME ZONE
);

-- Vector similarity index (cosine distance)
CREATE INDEX IF NOT EXISTS memories_embedding_hnsw_idx
    ON mem.memories
    USING hnsw (embedding vector_cosine_ops);

-- Filter / pagination indexes
CREATE INDEX IF NOT EXISTS memories_user_id_idx
    ON mem.memories (user_id)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS memories_agent_id_idx
    ON mem.memories (agent_id)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS memories_run_id_idx
    ON mem.memories (run_id)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS memories_hash_idx
    ON mem.memories (hash)
    WHERE deleted_at IS NULL;

-- Full-text search (BM25-style keyword fallback / hybrid retrieval)
-- The text-search config name is injected by DatabaseInitService from
-- nubase.mem.search.ftsConfig (default 'simple'). For Chinese, install the
-- 'zhparser' extension and set the property to 'zhparser'.
CREATE INDEX IF NOT EXISTS memories_memory_fts_idx
    ON mem.memories
    USING gin (to_tsvector('${fts_config}', memory));

-- updated_at trigger
CREATE OR REPLACE FUNCTION mem.update_updated_at_column()
    RETURNS TRIGGER AS
$$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS update_memories_updated_at ON mem.memories;
CREATE TRIGGER update_memories_updated_at
    BEFORE UPDATE
    ON mem.memories
    FOR EACH ROW
EXECUTE FUNCTION mem.update_updated_at_column();

COMMENT ON TABLE mem.memories IS 'AI memory items: fact text + embedding + owner triple.';
COMMENT ON COLUMN mem.memories.embedding IS 'pgvector 1536-dim embedding (OpenAI text-embedding-3-small default).';
COMMENT ON COLUMN mem.memories.hash IS 'SHA-256 of normalized memory text, used for dedupe.';

-- Enable Row Level Security (policies defined in init_roles.sql)
ALTER TABLE mem.memories ENABLE ROW LEVEL SECURITY;


-- ==================================================
-- mem.memory_history : audit trail
-- ==================================================
CREATE TABLE IF NOT EXISTS mem.memory_history
(
    id         UUID PRIMARY KEY                  DEFAULT gen_random_uuid(),
    memory_id  UUID                     NOT NULL,
    old_value  TEXT,
    new_value  TEXT,
    event      VARCHAR(16)              NOT NULL,
    actor_id   VARCHAR(128),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS memory_history_memory_id_idx
    ON mem.memory_history (memory_id, created_at DESC);

COMMENT ON TABLE mem.memory_history IS 'Audit trail of memory ADD / UPDATE / DELETE events.';
COMMENT ON COLUMN mem.memory_history.event IS 'ADD | UPDATE | DELETE';

ALTER TABLE mem.memory_history ENABLE ROW LEVEL SECURITY;


-- ==================================================
-- mem.entities : named-entity index used for retrieval boosting
-- ==================================================
-- Each row stores a distinct entity (e.g. "John", "Tokyo") per owner triple, with
-- an array of memory ids that mention it. Search-time entity boost = look up the
-- query's entities, find matches here, and uplift the linked memories' scores.
CREATE TABLE IF NOT EXISTS mem.entities
(
    id                UUID PRIMARY KEY                  DEFAULT gen_random_uuid(),

    -- Owner triple (same isolation model as mem.memories).
    -- FK + CASCADE so deleting the auth.users row also clears their entities.
    user_id           UUID REFERENCES auth.users(id) ON DELETE CASCADE,
    agent_id          VARCHAR(128),
    run_id            VARCHAR(128),

    -- Entity content
    text              VARCHAR(512)             NOT NULL,
    entity_type       VARCHAR(64),
    embedding         vector(${embedding_dimensions}),

    -- Memories that reference this entity
    linked_memory_ids UUID[]                   NOT NULL DEFAULT '{}',

    created_at        TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Literal-uniqueness on (text, type, owner) — vector-similar duplicates are merged
-- in application code before insertion; this is the last-line-of-defense dedupe.
-- COALESCE collapses NULL owner fields to '' so they participate in the unique key.
CREATE UNIQUE INDEX IF NOT EXISTS entities_unique_idx
    ON mem.entities (
        lower(text),
        COALESCE(entity_type, ''),
        COALESCE(user_id::text, ''),
        COALESCE(agent_id, ''),
        COALESCE(run_id, '')
    );

CREATE INDEX IF NOT EXISTS entities_embedding_hnsw_idx
    ON mem.entities
    USING hnsw (embedding vector_cosine_ops);

CREATE INDEX IF NOT EXISTS entities_user_id_idx
    ON mem.entities (user_id);

-- updated_at trigger (reuses the function defined for memories above)
DROP TRIGGER IF EXISTS update_entities_updated_at ON mem.entities;
CREATE TRIGGER update_entities_updated_at
    BEFORE UPDATE
    ON mem.entities
    FOR EACH ROW
EXECUTE FUNCTION mem.update_updated_at_column();

COMMENT ON TABLE mem.entities IS 'Named entities extracted from facts; used for retrieval boosting.';
COMMENT ON COLUMN mem.entities.linked_memory_ids IS 'Array of mem.memories.id values that reference this entity.';

ALTER TABLE mem.entities ENABLE ROW LEVEL SECURITY;


-- ==================================================
-- mem.session_messages : short-term conversation window
-- ==================================================
CREATE TABLE IF NOT EXISTS mem.session_messages
(
    id            UUID PRIMARY KEY                  DEFAULT gen_random_uuid(),
    session_scope VARCHAR(256)             NOT NULL,
    -- Same FK + CASCADE story as mem.memories. NULL means the session is owned by
    -- an agent / run with no user attached, in which case RLS hides it from
    -- authenticated callers and only service_role can touch it.
    user_id       UUID REFERENCES auth.users(id) ON DELETE CASCADE,
    role          VARCHAR(32),
    content       TEXT,
    name          VARCHAR(128),
    created_at    TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS session_messages_scope_idx
    ON mem.session_messages (session_scope, created_at DESC);

CREATE INDEX IF NOT EXISTS session_messages_user_id_idx
    ON mem.session_messages (user_id, created_at DESC)
    WHERE user_id IS NOT NULL;

COMMENT ON TABLE mem.session_messages IS 'Recent N messages per session scope (rolling window for context).';
COMMENT ON COLUMN mem.session_messages.session_scope IS 'Composite scope key built from user_id/agent_id/run_id; groups messages of the same conversation.';
COMMENT ON COLUMN mem.session_messages.user_id IS 'Owning auth user when present. Drives RLS; agent/run-only sessions leave it NULL and are only reachable via service_role.';

ALTER TABLE mem.session_messages ENABLE ROW LEVEL SECURITY;


-- ==================================================
-- mem.config : per-project memory configuration overrides
-- ==================================================
-- Singleton row (id = 1) holding a JSONB blob of per-project settings that
-- override the platform-wide YAML defaults in nubase.mem.*.
--
-- Read path: MemConfigResolver overlays this JSON on MemProperties at every
-- accessor call. Missing keys fall back to the YAML default — the table is
-- always a thin override layer, never the source of truth for the full config.
--
-- Editable subset (rest stays YAML-pinned because they need re-init):
--   historyEnabled,
--   search.{defaultTopK, defaultThreshold, entityBoostEnabled, entityMatchSimilarity},
--   session.{enabled, maxMessages, injectIntoExtraction},
--   entity.maxLinkedMemoryIds.
CREATE TABLE IF NOT EXISTS mem.config
(
    id         INT PRIMARY KEY CHECK (id = 1),
    config     JSONB                    NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_by UUID
);

INSERT INTO mem.config (id, config) VALUES (1, '{}'::jsonb) ON CONFLICT DO NOTHING;

COMMENT ON TABLE mem.config IS 'Per-project memory configuration overrides (singleton; id=1 always).';
COMMENT ON COLUMN mem.config.config IS 'Partial JSON; missing keys fall back to the platform YAML default.';

ALTER TABLE mem.config ENABLE ROW LEVEL SECURITY;
