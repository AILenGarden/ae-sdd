package ai.nubase.mem.service;

import ai.nubase.common.config.OpenAIConfig;
import ai.nubase.mem.config.AnthropicProperties;
import ai.nubase.mem.config.GenericLlmProperties;
import ai.nubase.mem.config.MemProperties;
import com.fasterxml.jackson.databind.JsonNode;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;

/**
 * Per-tenant accessor for memory configuration.
 *
 * <p>Every method returns the project's override (from {@code mem.config}) when present,
 * otherwise the platform-wide YAML default. Callers must use this resolver instead of
 * injecting {@link MemProperties} / {@link OpenAIConfig} / etc. directly so per-project
 * settings actually take effect at runtime.
 *
 * <p>Editable subset:
 * <ul>
 *   <li>{@code historyEnabled}, {@code search.*}, {@code session.*}, {@code entity.*}</li>
 *   <li>{@code chat.{provider,model,temperature}}</li>
 *   <li>{@code embedding.{provider,model}}</li>
 *   <li>{@code providers.{openai,anthropic,generic}.{authToken,baseUrl,timeout,...}}</li>
 * </ul>
 *
 * <p>Locked (re-init required): {@code embedding.dimensions} (pgvector column type),
 * {@code search.ftsConfig} (GIN index), {@code embedding.cache.*} (process-level Caffeine
 * bean), and the top-level {@code nubase.mem.enabled} switch (class-load gate).
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class MemConfigResolver {

    private final MemProperties yaml;
    private final OpenAIConfig openaiYaml;
    private final AnthropicProperties anthropicYaml;
    private final GenericLlmProperties genericYaml;
    private final MemConfigStoreService store;

    // ---------- top-level ----------

    public boolean isHistoryEnabled() {
        return getBool("historyEnabled", yaml.isHistoryEnabled());
    }

    // ---------- search ----------

    public int searchDefaultTopK() {
        return getInt("search.defaultTopK", yaml.getSearch().getDefaultTopK());
    }

    public double searchDefaultThreshold() {
        return getDouble("search.defaultThreshold", yaml.getSearch().getDefaultThreshold());
    }

    public boolean searchEntityBoostEnabled() {
        return getBool("search.entityBoostEnabled", yaml.getSearch().isEntityBoostEnabled());
    }

    public double searchEntityMatchSimilarity() {
        return getDouble("search.entityMatchSimilarity",
                yaml.getSearch().getEntityMatchSimilarity());
    }

    // ---------- session ----------

    public boolean sessionEnabled() {
        return getBool("session.enabled", yaml.getSession().isEnabled());
    }

    public int sessionMaxMessages() {
        return getInt("session.maxMessages", yaml.getSession().getMaxMessages());
    }

    public boolean sessionInjectIntoExtraction() {
        return getBool("session.injectIntoExtraction",
                yaml.getSession().isInjectIntoExtraction());
    }

    // ---------- entity ----------

    public int entityMaxLinkedMemoryIds() {
        return getInt("entity.maxLinkedMemoryIds", yaml.getEntity().getMaxLinkedMemoryIds());
    }

    // ---------- chat ----------

    public String chatProvider() {
        return getString("chat.provider", yaml.getChatProvider());
    }

    public String chatModel() {
        return getString("chat.model", yaml.getChat().getModel());
    }

    public double chatTemperature() {
        return getDouble("chat.temperature", yaml.getChat().getTemperature());
    }

    // ---------- embedding ----------

    public String embeddingProvider() {
        return getString("embedding.provider", yaml.getEmbeddingProvider());
    }

    public String embeddingModel() {
        return getString("embedding.model", yaml.getEmbedding().getModel());
    }

    // ---------- provider credentials: OpenAI ----------

    public String openaiAuthToken() {
        return getString("providers.openai.authToken", openaiYaml.getAuthToken());
    }

    public String openaiBaseUrl() {
        return getString("providers.openai.baseUrl", openaiYaml.getBaseUrl());
    }

    public int openaiTimeout() {
        return getInt("providers.openai.timeout",
                openaiYaml.getTimeout() != null ? openaiYaml.getTimeout() : 60000);
    }

    public int openaiMaxRetries() {
        return getInt("providers.openai.maxRetries",
                openaiYaml.getMaxRetries() != null ? openaiYaml.getMaxRetries() : 3);
    }

    // ---------- provider credentials: Anthropic ----------

    public String anthropicAuthToken() {
        return getString("providers.anthropic.authToken", anthropicYaml.getAuthToken());
    }

    public String anthropicBaseUrl() {
        return getString("providers.anthropic.baseUrl", anthropicYaml.getBaseUrl());
    }

    public int anthropicTimeout() {
        return getInt("providers.anthropic.timeout", anthropicYaml.getTimeout());
    }

    public String anthropicVersion() {
        return getString("providers.anthropic.version", anthropicYaml.getVersion());
    }

    // ---------- provider credentials: Generic OpenAI-compatible ----------

    public String genericAuthToken() {
        return getString("providers.generic.authToken", genericYaml.getAuthToken());
    }

    public String genericBaseUrl() {
        return getString("providers.generic.baseUrl", genericYaml.getBaseUrl());
    }

    public int genericTimeout() {
        return getInt("providers.generic.timeout", genericYaml.getTimeout());
    }

    // ---------- low-level ----------

    private JsonNode pathLookup(String dottedPath) {
        try {
            JsonNode node = store.read();
            for (String part : dottedPath.split("\\.")) {
                if (node == null || node.isNull()) return null;
                node = node.get(part);
            }
            return node;
        } catch (Exception e) {
            log.debug("mem config lookup '{}' failed, falling back to YAML: {}",
                    dottedPath, e.getMessage());
            return null;
        }
    }

    private boolean getBool(String path, boolean fallback) {
        JsonNode n = pathLookup(path);
        return (n != null && !n.isNull()) ? n.asBoolean(fallback) : fallback;
    }

    private int getInt(String path, int fallback) {
        JsonNode n = pathLookup(path);
        return (n != null && !n.isNull()) ? n.asInt(fallback) : fallback;
    }

    private double getDouble(String path, double fallback) {
        JsonNode n = pathLookup(path);
        return (n != null && !n.isNull()) ? n.asDouble(fallback) : fallback;
    }

    private String getString(String path, String fallback) {
        JsonNode n = pathLookup(path);
        if (n == null || n.isNull()) return fallback;
        String s = n.asText(null);
        // Treat explicit empty string as "use fallback" rather than "be empty" — empty
        // creds = misconfiguration, the YAML default should win.
        return (s == null || s.isEmpty()) ? fallback : s;
    }
}
