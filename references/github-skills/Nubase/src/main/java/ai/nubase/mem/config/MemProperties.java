package ai.nubase.mem.config;

import lombok.Data;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.stereotype.Component;

/**
 * Top-level configuration for the AI memory module.
 *
 * <p>Maps to {@code nubase.mem.*} in application.yml.
 */
@Data
@Component
@ConfigurationProperties(prefix = "nubase.mem")
public class MemProperties {

    /** Enable / disable the memory feature globally. */
    private boolean enabled = true;

    /** Which provider to use for chat completions: openai | anthropic | generic. */
    private String chatProvider = "openai";

    /** Which provider to use for embeddings: openai | generic. */
    private String embeddingProvider = "openai";

    /** Embedding-specific config (model, dimensions). */
    private Embedding embedding = new Embedding();

    /** Chat-specific config (model). */
    private Chat chat = new Chat();

    /** Search-time defaults. */
    private Search search = new Search();

    /** Recent message window. */
    private Session session = new Session();

    /** Whether to record audit events in mem.memory_history. */
    private boolean historyEnabled = true;

    /** Entity-store knobs. */
    private Entity entity = new Entity();

    @Data
    public static class Entity {
        /**
         * Hard cap on {@code mem.entities.linked_memory_ids} array length per row.
         *
         * <p>Without a cap, a heavily-referenced entity (e.g. the user's own name) accumulates
         * tens of thousands of memory ids — PG arrays of that size make every
         * {@code array_append}, {@code = ANY(...)}, and boost-spread computation slower and
         * give entity boost less value (the per-link share approaches zero anyway).
         *
         * <p>When the cap is hit, {@code appendLinkedMemory} is a silent no-op. The memory
         * is still searchable via vector/BM25 — it just stops contributing to entity boost.
         */
        private int maxLinkedMemoryIds = 1000;
    }

    @Data
    public static class Embedding {
        private String model = "text-embedding-3-small";
        private int dimensions = 1536;
        private Cache cache = new Cache();
    }

    /**
     * In-process cache for embeddings. Key = SHA-256(model + ":" + text).
     * Safe to share across tenants because the key is content-addressed and contains no
     * tenant data.
     */
    @Data
    public static class Cache {
        private boolean enabled = true;
        private long maximumSize = 10000;
        private long ttlMinutes = 60;
    }

    @Data
    public static class Chat {
        /** Model id passed to the provider (e.g. gpt-4o-mini, claude-3-5-haiku-latest, qwen-plus). */
        private String model = "gpt-4o-mini";
        /** Temperature for fact extraction and update decisions. Keep low for consistency. */
        private double temperature = 0.0;
    }

    @Data
    public static class Search {
        private int defaultTopK = 5;
        /** Cosine distance threshold (1 - similarity). Results with distance > threshold are filtered. */
        private double defaultThreshold = 0.7;
        /**
         * If true, search calls extract entities from the query and use them to boost
         * retrieval scores. Adds one LLM call per search; disable for latency-sensitive use.
         */
        private boolean entityBoostEnabled = true;
        /**
         * Minimum cosine similarity (1 - distance) for a query entity to match a stored entity
         * during boost computation. Mirrors mem0's 0.5 threshold.
         */
        private double entityMatchSimilarity = 0.5;
        /**
         * PostgreSQL text-search configuration used by {@code to_tsvector} / {@code plainto_tsquery}
         * in BM25 keyword search. Must match a config installed in the tenant DB.
         *
         * <p>Common values:
         * <ul>
         *   <li>{@code simple} — default, whitespace-split, no stemming (works for any language
         *       with explicit word boundaries, including English; useless for Chinese/Japanese)</li>
         *   <li>{@code english} — stemming + stopwords for English</li>
         *   <li>{@code zhparser} — requires the {@code zhparser} extension installed in PG;
         *       supports Chinese word segmentation</li>
         * </ul>
         *
         * <p><b>Changing this after deployment requires recreating the
         * {@code memories_memory_fts_idx} GIN index</b> — the index is built against a specific
         * config, and Postgres won't use it when the query uses a different config.
         */
        private String ftsConfig = "simple";
    }

    @Data
    public static class Session {
        private boolean enabled = true;
        private int maxMessages = 10;
        /**
         * If true, every fact-extraction call prepends the recent N messages from
         * {@code mem.session_messages} (for the same owner triple) to the current request's
         * messages, so the LLM can see continuing conversation context.
         *
         * <p>Default {@code false} — opt-in because (a) it adds token cost on every add,
         * (b) it can mix unrelated past conversations if the caller reuses owner ids loosely.
         * Recommended when {@code runId} is used to scope each conversation.
         */
        private boolean injectIntoExtraction = false;
    }
}
