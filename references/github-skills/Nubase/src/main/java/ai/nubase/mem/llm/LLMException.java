package ai.nubase.mem.llm;

/**
 * Thrown when an LLM or embedding provider fails (transport, auth, parse, upstream error).
 */
public class LLMException extends RuntimeException {

    public LLMException(String message) {
        super(message);
    }

    public LLMException(String message, Throwable cause) {
        super(message, cause);
    }
}
