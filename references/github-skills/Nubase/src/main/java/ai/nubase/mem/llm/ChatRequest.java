package ai.nubase.mem.llm;

import lombok.Builder;
import lombok.Data;

import java.util.List;

/**
 * Provider-neutral chat completion request.
 */
@Data
@Builder
public class ChatRequest {

    /** Conversation messages (system + user + ...). */
    private List<ChatMessage> messages;

    /** Model id (provider-specific). May be {@code null} to use the provider's default. */
    private String model;

    /** Sampling temperature. {@code null} means use provider default. */
    private Double temperature;

    /** If true, instruct the provider to return a strict JSON object response. */
    @Builder.Default
    private boolean jsonMode = false;

    /** Max tokens for the response. {@code null} means use provider default. */
    private Integer maxTokens;
}
