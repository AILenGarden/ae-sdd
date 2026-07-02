package ai.nubase.mem.llm;

import java.util.List;

/**
 * Provider-neutral text embedding abstraction.
 *
 * <p>Selection is driven by {@code nubase.mem.embedding-provider}.
 */
public interface EmbeddingProvider {

    /** Unique identifier matching {@code nubase.mem.embedding-provider}. */
    String name();

    /** Whether this provider is enabled and ready. */
    boolean isAvailable();

    /** Embedding dimensionality (must match the {@code vector(N)} column in mem.memories). */
    int dimensions();

    /**
     * Embed a single text.
     *
     * @throws LLMException on transport, parsing, or upstream errors
     */
    float[] embed(String text) throws LLMException;

    /**
     * Embed multiple texts in one call when possible.
     *
     * <p>Default implementation falls back to per-text calls.
     *
     * @throws LLMException on transport, parsing, or upstream errors
     */
    default List<float[]> embedBatch(List<String> texts) throws LLMException {
        return texts.stream().map(t -> {
            try {
                return embed(t);
            } catch (LLMException e) {
                throw new RuntimeException(e);
            }
        }).toList();
    }
}
