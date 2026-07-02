package ai.nubase.mem.service;

import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.dto.MemConfigResponse;
import ai.nubase.mem.llm.ChatLLMProvider;
import ai.nubase.mem.llm.EmbeddingProvider;
import ai.nubase.mem.llm.LLMProviderRegistry;
import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Service;

/**
 * Assembles the read-only configuration snapshot exposed by {@code GET /mem/v1/config}.
 *
 * <p>Kept as a thin assembler instead of a Jackson view on {@link MemProperties} for two
 * reasons: (a) we enrich with provider availability, which lives in
 * {@link LLMProviderRegistry}, not the config object; (b) it gives us a stable API contract
 * decoupled from any future refactors of the internal property classes.
 */
@Service
@RequiredArgsConstructor
public class MemConfigService {

    private final MemProperties memProperties;
    private final LLMProviderRegistry providers;
    private final MemConfigResolver memConfig;

    public MemConfigResponse snapshot() {
        MemProperties.Embedding emb = memProperties.getEmbedding();
        MemProperties.Chat chat = memProperties.getChat();
        MemProperties.Search search = memProperties.getSearch();
        MemProperties.Session session = memProperties.getSession();
        MemProperties.Entity entity = memProperties.getEntity();

        // Resolve providers defensively — a misconfigured chat/embedding provider name
        // (typo in yml) would throw from registry.chat()/embedding(). The settings page
        // is the one place that needs to *show* that misconfiguration, not crash on it.
        String chatName = memProperties.getChatProvider();
        boolean chatAvailable = false;
        try {
            ChatLLMProvider c = providers.chat();
            chatName = c.name();
            chatAvailable = c.isAvailable();
        } catch (Exception ignore) {
            // misconfigured chat-provider — leave chatAvailable=false, name as configured
        }

        String embName = memProperties.getEmbeddingProvider();
        boolean embAvailable = false;
        try {
            EmbeddingProvider e = providers.embedding();
            embName = e.name();
            embAvailable = e.isAvailable();
        } catch (Exception ignore) {
            // misconfigured embedding-provider
        }

        // Snapshot reflects effective per-project values: resolver returns YAML default
        // when no override exists, project override when present. embedding.dimensions /
        // embedding.cache / search.ftsConfig stay YAML-only because they're baked into the
        // schema (vector column type, GIN index, process-level Caffeine bean).
        return MemConfigResponse.builder()
                .enabled(memProperties.isEnabled())
                .historyEnabled(memConfig.isHistoryEnabled())
                .chat(MemConfigResponse.Chat.builder()
                        .provider(memConfig.chatProvider())
                        .model(memConfig.chatModel())
                        .temperature(memConfig.chatTemperature())
                        .build())
                .embedding(MemConfigResponse.Embedding.builder()
                        .provider(memConfig.embeddingProvider())
                        .model(memConfig.embeddingModel())
                        .dimensions(emb.getDimensions())
                        .cacheEnabled(emb.getCache().isEnabled())
                        .cacheMaximumSize(emb.getCache().getMaximumSize())
                        .cacheTtlMinutes(emb.getCache().getTtlMinutes())
                        .build())
                .search(MemConfigResponse.Search.builder()
                        .defaultTopK(memConfig.searchDefaultTopK())
                        .defaultThreshold(memConfig.searchDefaultThreshold())
                        .entityBoostEnabled(memConfig.searchEntityBoostEnabled())
                        .entityMatchSimilarity(memConfig.searchEntityMatchSimilarity())
                        .ftsConfig(search.getFtsConfig())
                        .build())
                .session(MemConfigResponse.Session.builder()
                        .enabled(memConfig.sessionEnabled())
                        .maxMessages(memConfig.sessionMaxMessages())
                        .injectIntoExtraction(memConfig.sessionInjectIntoExtraction())
                        .build())
                .entity(MemConfigResponse.Entity.builder()
                        .maxLinkedMemoryIds(memConfig.entityMaxLinkedMemoryIds())
                        .build())
                .providerStatus(MemConfigResponse.ProviderStatus.builder()
                        .chatProviderName(chatName)
                        .chatAvailable(chatAvailable)
                        .embeddingProviderName(embName)
                        .embeddingAvailable(embAvailable)
                        .build())
                .providers(MemConfigResponse.Providers.builder()
                        .openai(creds(memConfig.openaiAuthToken(), memConfig.openaiBaseUrl()))
                        .anthropic(creds(memConfig.anthropicAuthToken(), memConfig.anthropicBaseUrl()))
                        .generic(creds(memConfig.genericAuthToken(), memConfig.genericBaseUrl()))
                        .build())
                .build();
    }

    private MemConfigResponse.ProviderCreds creds(String authToken, String baseUrl) {
        return MemConfigResponse.ProviderCreds.builder()
                .authTokenSet(authToken != null && !authToken.isBlank())
                .baseUrl(baseUrl)
                .build();
    }
}
