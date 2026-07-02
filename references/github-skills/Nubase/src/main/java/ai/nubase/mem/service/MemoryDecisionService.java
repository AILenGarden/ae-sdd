package ai.nubase.mem.service;

import ai.nubase.mem.llm.ChatMessage;
import ai.nubase.mem.llm.ChatRequest;
import ai.nubase.mem.llm.LLMProviderRegistry;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;

import java.util.ArrayList;
import java.util.List;

/**
 * Asks the LLM to decide ADD / UPDATE / DELETE / NONE for each new fact
 * given the current top-K similar memories.
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class MemoryDecisionService {

    private final LLMProviderRegistry providers;
    private final PromptLoader prompts;
    private final ObjectMapper objectMapper;

    @Data
    @Builder
    @NoArgsConstructor
    @AllArgsConstructor
    public static class ExistingMemory {
        private String id;
        private String text;
    }

    @Data
    @Builder
    @NoArgsConstructor
    @AllArgsConstructor
    public static class Decision {
        /** ADD | UPDATE | DELETE | NONE */
        private String event;
        /** Existing memory id for UPDATE/DELETE/NONE, or a temp id like "new_0" for ADD. */
        private String id;
        /** New memory text for ADD / UPDATE. */
        private String text;
        /** Previous memory text for UPDATE (informational). */
        private String oldMemory;
    }

    /**
     * Decide actions for a batch of new facts against existing memories.
     *
     * @param existing the top-K nearest-neighbor memories already stored
     * @param newFacts the freshly extracted facts
     * @return one decision per fact (or per existing memory for DELETE/UPDATE/NONE)
     */
    public List<Decision> decide(List<ExistingMemory> existing, List<String> newFacts) {
        if (newFacts == null || newFacts.isEmpty()) {
            return List.of();
        }
        // Pre-flight: when the provider isn't configured, skip the LLM hop and use the
        // ADD-only fallback directly (matches the catch path below — same outcome, no HTTP).
        if (!providers.chat().isAvailable()) {
            log.info("Chat provider not configured — using ADD-only fallback for {} facts",
                    newFacts.size());
            return fallbackAllAdd(newFacts);
        }

        ObjectNode userPayload = objectMapper.createObjectNode();
        ArrayNode existingArr = userPayload.putArray("existing_memories");
        if (existing != null) {
            for (ExistingMemory em : existing) {
                ObjectNode n = existingArr.addObject();
                n.put("id", em.getId());
                n.put("text", em.getText());
            }
        }
        ArrayNode factsArr = userPayload.putArray("new_facts");
        for (String f : newFacts) {
            factsArr.add(f);
        }

        String userMessage;
        try {
            userMessage = objectMapper.writerWithDefaultPrettyPrinter()
                    .writeValueAsString(userPayload);
        } catch (Exception e) {
            log.warn("Failed to serialize decision payload, falling back to ADD-only", e);
            return fallbackAllAdd(newFacts);
        }

        List<ChatMessage> llmMessages = List.of(
                ChatMessage.system(prompts.updateMemoryPrompt()),
                ChatMessage.user(userMessage)
        );

        ChatRequest req = ChatRequest.builder()
                .messages(llmMessages)
                .jsonMode(true)
                .build();

        String raw;
        try {
            raw = providers.chat().chat(req);
        } catch (Exception e) {
            log.warn("Memory-decision LLM call failed, falling back to ADD-only: {}", e.getMessage());
            return fallbackAllAdd(newFacts);
        }

        return parseDecisions(raw, newFacts);
    }

    private List<Decision> parseDecisions(String raw, List<String> newFacts) {
        if (raw == null || raw.isBlank()) {
            return fallbackAllAdd(newFacts);
        }
        String cleaned = FactExtractionService.stripCodeFence(raw.trim());
        try {
            JsonNode root = objectMapper.readTree(cleaned);
            JsonNode memory = root.path("memory");
            if (!memory.isArray()) {
                log.warn("Decision response missing 'memory' array, falling back to ADD-only. raw={}", raw);
                return fallbackAllAdd(newFacts);
            }
            List<Decision> out = new ArrayList<>(memory.size());
            for (JsonNode d : memory) {
                Decision dec = Decision.builder()
                        .event(d.path("event").asText("ADD").toUpperCase())
                        .id(d.path("id").asText(null))
                        .text(d.path("text").asText(null))
                        .oldMemory(d.path("old_memory").asText(null))
                        .build();
                out.add(dec);
            }
            return out;
        } catch (Exception e) {
            log.warn("Failed to parse decision JSON, falling back to ADD-only: {}", e.getMessage());
            return fallbackAllAdd(newFacts);
        }
    }

    private static List<Decision> fallbackAllAdd(List<String> newFacts) {
        List<Decision> out = new ArrayList<>(newFacts.size());
        for (int i = 0; i < newFacts.size(); i++) {
            out.add(Decision.builder()
                    .event("ADD")
                    .id("new_" + i)
                    .text(newFacts.get(i))
                    .build());
        }
        return out;
    }
}
