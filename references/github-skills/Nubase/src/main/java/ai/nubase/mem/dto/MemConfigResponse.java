package ai.nubase.mem.dto;

import com.fasterxml.jackson.annotation.JsonInclude;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Read-only snapshot of the mem-module configuration for the admin Settings page.
 *
 * <p>Deliberately excludes secrets (API keys, db credentials). The point is to let admins
 * see what model/dimensions/thresholds are active so they can diagnose surprising behavior
 * without shelling into the server.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class MemConfigResponse {

    /** Whether the mem feature is globally enabled. */
    private boolean enabled;

    private Chat chat;
    private Embedding embedding;
    private Search search;
    private Session session;
    private Entity entity;
    private boolean historyEnabled;

    private ProviderStatus providerStatus;

    /** Per-provider credentials (auth-token masked as {@code {set: bool}} only). */
    private Providers providers;

    @Data @Builder @NoArgsConstructor @AllArgsConstructor
    public static class Chat {
        private String provider;
        private String model;
        private double temperature;
    }

    @Data @Builder @NoArgsConstructor @AllArgsConstructor
    public static class Embedding {
        private String provider;
        private String model;
        private int dimensions;
        private boolean cacheEnabled;
        private long cacheMaximumSize;
        private long cacheTtlMinutes;
    }

    @Data @Builder @NoArgsConstructor @AllArgsConstructor
    public static class Search {
        private int defaultTopK;
        private double defaultThreshold;
        private boolean entityBoostEnabled;
        private double entityMatchSimilarity;
        private String ftsConfig;
    }

    @Data @Builder @NoArgsConstructor @AllArgsConstructor
    public static class Session {
        private boolean enabled;
        private int maxMessages;
        private boolean injectIntoExtraction;
    }

    @Data @Builder @NoArgsConstructor @AllArgsConstructor
    public static class Entity {
        private int maxLinkedMemoryIds;
    }

    @Data @Builder @NoArgsConstructor @AllArgsConstructor
    public static class ProviderStatus {
        /** {@code true} when the configured chat provider has a usable API key. */
        private boolean chatAvailable;
        private String chatProviderName;
        /** Same for embedding. */
        private boolean embeddingAvailable;
        private String embeddingProviderName;
    }

    @Data @Builder @NoArgsConstructor @AllArgsConstructor
    public static class Providers {
        private ProviderCreds openai;
        private ProviderCreds anthropic;
        private ProviderCreds generic;
    }

    /**
     * Non-sensitive view of one provider's credentials. {@code authToken} is never echoed
     * back over the wire — only {@code authTokenSet} signals whether one is configured.
     */
    @Data @Builder @NoArgsConstructor @AllArgsConstructor
    public static class ProviderCreds {
        private boolean authTokenSet;
        private String baseUrl;
    }
}
