package ai.nubase.mem.dto;

import ai.nubase.mem.llm.ChatMessage;
import lombok.Data;

import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Request body for {@code POST /mem/v1/memories}.
 *
 * <p>At least one of {@code userId} / {@code agentId} / {@code runId} must be provided.
 */
@Data
public class AddMemoryRequest {

    /** Conversation messages to extract memories from. */
    private List<ChatMessage> messages;

    /** Owner triple. */
    private UUID userId;
    private String agentId;
    private String runId;

    /** Caller-supplied metadata (merged into stored row). */
    private Map<String, Object> metadata;

    /**
     * If true (default), use the LLM to extract facts and decide ADD/UPDATE/DELETE.
     * If false, store every non-system message verbatim as its own memory.
     */
    private Boolean infer;
}
