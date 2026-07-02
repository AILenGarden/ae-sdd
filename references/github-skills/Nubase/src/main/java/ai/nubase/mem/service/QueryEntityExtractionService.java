package ai.nubase.mem.service;

import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.llm.ChatMessage;
import ai.nubase.mem.llm.ChatRequest;
import ai.nubase.mem.llm.LLMProviderRegistry;
import ai.nubase.mem.service.FactExtractionService.ExtractedEntity;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;

import java.util.ArrayList;
import java.util.List;

/**
 * Extracts named entities from a search query for entity-boost scoring at retrieval time.
 *
 * <p>This is a separate, lightweight prompt (shorter than fact extraction). It can be
 * disabled via {@code nubase.mem.search.entity-boost-enabled=false} to save one LLM call
 * per search when entity boost is not needed.
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class QueryEntityExtractionService {

    private final LLMProviderRegistry providers;
    private final PromptLoader prompts;
    private final ObjectMapper objectMapper;
    private final MemProperties memProperties;
    private final MemConfigResolver memConfig;

    /**
     * Return entities found in {@code query}; empty when extraction is disabled, the query
     * is empty, or the LLM call fails (degraded gracefully).
     */
    public List<ExtractedEntity> extract(String query) {
        if (!memConfig.searchEntityBoostEnabled()) {
            return List.of();
        }
        if (query == null || query.isBlank()) {
            return List.of();
        }
        // Pre-flight: don't burn a per-search HTTP round trip when the provider isn't configured.
        if (!providers.chat().isAvailable()) {
            return List.of();
        }

        ChatRequest req = ChatRequest.builder()
                .messages(List.of(
                        ChatMessage.system(prompts.queryEntitiesPrompt()),
                        ChatMessage.user("Query: " + query)
                ))
                .jsonMode(true)
                .build();

        String raw;
        try {
            raw = providers.chat().chat(req);
        } catch (Exception e) {
            log.warn("Query-entity extraction failed, falling back to no boost: {}", e.getMessage());
            return List.of();
        }
        return parse(raw);
    }

    private List<ExtractedEntity> parse(String raw) {
        if (raw == null || raw.isBlank()) {
            return List.of();
        }
        String cleaned = FactExtractionService.stripCodeFence(raw.trim());
        try {
            JsonNode root = objectMapper.readTree(cleaned);
            JsonNode arr = root.path("entities");
            if (!arr.isArray()) {
                return List.of();
            }
            List<ExtractedEntity> out = new ArrayList<>(arr.size());
            for (JsonNode n : arr) {
                String text = n.path("text").asText(null);
                String type = n.path("type").asText(null);
                if (text != null && !text.isBlank()) {
                    out.add(ExtractedEntity.builder().text(text.trim()).type(type).build());
                }
            }
            return out;
        } catch (Exception e) {
            log.warn("Failed to parse query-entity JSON ({}): {}", e.getMessage(), raw);
            return List.of();
        }
    }
}
