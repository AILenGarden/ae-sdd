package ai.nubase.mem.llm;

/**
 * Provider-neutral chat completion abstraction.
 *
 * <p>Each implementation talks to one upstream (OpenAI, Anthropic, OpenAI-compatible vendors).
 * Selection is driven by {@code nubase.mem.chat-provider}.
 */
public interface ChatLLMProvider {

    /** Unique identifier matching {@code nubase.mem.chat-provider}. */
    String name();

    /** Whether this provider is enabled and ready (auth token present, etc.). */
    boolean isAvailable();

    /**
     * Perform a chat completion and return the assistant's text response.
     *
     * @throws LLMException on transport, parsing, or upstream errors
     */
    String chat(ChatRequest request) throws LLMException;
}
