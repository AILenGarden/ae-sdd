package ai.nubase.mem.service;

import ai.nubase.mem.llm.ChatMessage;
import ai.nubase.mem.llm.ChatRequest;
import ai.nubase.mem.llm.LLMProviderRegistry;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Calls the configured chat LLM with {@link PromptLoader#factRetrievalPrompt()} to turn raw
 * user/assistant messages into a list of memorable facts and, for each fact, the named
 * entities it mentions.
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class FactExtractionService {

    private final LLMProviderRegistry providers;
    private final PromptLoader prompts;
    private final ObjectMapper objectMapper;

    /**
     * One named entity extracted from a fact (or a search query).
     */
    @Data
    @Builder
    @NoArgsConstructor
    @AllArgsConstructor
    public static class ExtractedEntity {
        /** Surface form as it appeared. */
        private String text;
        /** Coarse type tag — person / location / organization / product / event / food / date / other. */
        private String type;
    }

    /**
     * Result of one extraction call: {@code facts[i]} corresponds to {@code entities[i]}.
     * The two lists are always the same length.
     */
    @Data
    @Builder
    @NoArgsConstructor
    @AllArgsConstructor
    public static class FactExtractionResult {
        private List<String> facts;
        private List<List<ExtractedEntity>> entities;

        public static FactExtractionResult empty() {
            return FactExtractionResult.builder()
                    .facts(List.of())
                    .entities(List.of())
                    .build();
        }

        public boolean isEmpty() {
            return facts == null || facts.isEmpty();
        }
    }

    /**
     * Extract facts and per-fact entities from a transcript.
     *
     * @param messages role/content pairs (system messages are ignored by the prompt rules)
     */
    public FactExtractionResult extract(List<ChatMessage> messages) {
        if (messages == null || messages.isEmpty()) {
            return FactExtractionResult.empty();
        }
        // Cheap pre-flight: skip the upstream call entirely when no credentials are configured.
        // Without this, every add() with infer=true racks up a doomed HTTP attempt + 401 + log spam.
        if (!providers.chat().isAvailable()) {
            log.warn("Chat provider '{}' is not configured — skipping fact extraction",
                    providers.chat().name());
            return FactExtractionResult.empty();
        }

        String userBlock = formatMessages(messages);

        List<ChatMessage> llmMessages = List.of(
                ChatMessage.system(prompts.factRetrievalPrompt()),
                ChatMessage.user("Conversation:\n" + userBlock)
        );

        ChatRequest req = ChatRequest.builder()
                .messages(llmMessages)
                .jsonMode(true)
                .build();

        String raw = providers.chat().chat(req);
        return parseResult(raw);
    }

    /** Convenience: facts-only (legacy callers / simple cases). */
    public List<String> extractFacts(List<ChatMessage> messages) {
        return extract(messages).getFacts();
    }

    static String formatMessages(List<ChatMessage> messages) {
        StringBuilder sb = new StringBuilder();
        for (ChatMessage m : messages) {
            if ("system".equalsIgnoreCase(m.getRole())) {
                continue;
            }
            sb.append(m.getRole() == null ? "user" : m.getRole().toLowerCase())
                    .append(": ")
                    .append(m.getContent() == null ? "" : m.getContent())
                    .append('\n');
        }
        return sb.toString();
    }

    private FactExtractionResult parseResult(String raw) {
        if (raw == null || raw.isBlank()) {
            return FactExtractionResult.empty();
        }
        String cleaned = stripCodeFence(raw.trim());
        try {
            JsonNode root = objectMapper.readTree(cleaned);
            JsonNode factsNode = root.path("facts");
            if (!factsNode.isArray()) {
                log.warn("Fact extraction returned non-array 'facts' field, raw={}", raw);
                return FactExtractionResult.empty();
            }
            List<String> facts = new ArrayList<>(factsNode.size());
            for (JsonNode f : factsNode) {
                if (f.isTextual() && !f.asText().isBlank()) {
                    facts.add(f.asText().trim());
                }
            }

            // Entities are optional; if absent or shape-mismatched, fall back to empty per-fact lists.
            JsonNode entitiesNode = root.path("entities");
            List<List<ExtractedEntity>> entities = new ArrayList<>(facts.size());
            if (entitiesNode.isArray() && entitiesNode.size() == facts.size()) {
                for (JsonNode perFact : entitiesNode) {
                    entities.add(parseEntityArray(perFact));
                }
            } else {
                if (!entitiesNode.isMissingNode() && entitiesNode.size() != facts.size()) {
                    log.warn("Fact-extraction entities array length {} does not match facts length {}, ignoring entities",
                            entitiesNode.size(), facts.size());
                }
                for (int i = 0; i < facts.size(); i++) {
                    entities.add(Collections.emptyList());
                }
            }

            return FactExtractionResult.builder().facts(facts).entities(entities).build();
        } catch (Exception e) {
            log.warn("Failed to parse fact-extraction JSON ({}): {}", e.getMessage(), raw);
            return FactExtractionResult.empty();
        }
    }

    private List<ExtractedEntity> parseEntityArray(JsonNode arr) {
        if (!arr.isArray() || arr.isEmpty()) {
            return Collections.emptyList();
        }
        List<ExtractedEntity> out = new ArrayList<>(arr.size());
        for (JsonNode item : arr) {
            String text = item.path("text").asText(null);
            String type = item.path("type").asText(null);
            if (text != null && !text.isBlank()) {
                out.add(ExtractedEntity.builder().text(text.trim()).type(type).build());
            }
        }
        return out;
    }

    static String stripCodeFence(String s) {
        if (s.startsWith("```")) {
            int firstNl = s.indexOf('\n');
            if (firstNl > 0) {
                s = s.substring(firstNl + 1);
            }
            if (s.endsWith("```")) {
                s = s.substring(0, s.length() - 3);
            }
        }
        return s.trim();
    }
}
