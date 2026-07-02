package ai.nubase.mem.config;

import lombok.Data;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.stereotype.Component;

/**
 * Anthropic API configuration (Claude chat completions).
 *
 * <p>Maps to {@code anthropic.*} in application.yml.
 */
@Data
@Component
@ConfigurationProperties(prefix = "anthropic")
public class AnthropicProperties {

    /** Anthropic API key. */
    private String authToken;

    /** Base URL — defaults to public api.anthropic.com. */
    private String baseUrl = "https://api.anthropic.com";

    /** Request timeout in milliseconds. */
    private int timeout = 60000;

    /** API version header value. */
    private String version = "2023-06-01";
}
