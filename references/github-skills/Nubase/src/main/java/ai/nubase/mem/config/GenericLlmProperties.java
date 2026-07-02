package ai.nubase.mem.config;

import lombok.Data;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.stereotype.Component;

/**
 * Generic OpenAI-compatible provider configuration.
 *
 * <p>Used for DashScope / DeepSeek / Moonshot / vLLM / Ollama / etc. — anything that exposes
 * the OpenAI {@code /v1/chat/completions} and {@code /v1/embeddings} contracts.
 *
 * <p>Maps to {@code nubase.mem.generic.*}.
 */
@Data
@Component
@ConfigurationProperties(prefix = "nubase.mem.generic")
public class GenericLlmProperties {

    /** API key for the upstream provider. */
    private String authToken;

    /** Base URL (must include the version segment, e.g. https://dashscope.aliyuncs.com/compatible-mode/v1). */
    private String baseUrl;

    /** Request timeout in milliseconds. */
    private int timeout = 60000;
}
