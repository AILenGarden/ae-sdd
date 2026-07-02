package ai.nubase.common.config;

import lombok.Data;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.stereotype.Component;

/**
 * OpenAI API Configuration
 * Configuration properties for OpenAI integration
 *
 * Used as fallback when database upstream configuration is unavailable
 */
@Data
@Component
@ConfigurationProperties(prefix = "openai")
public class OpenAIConfig {
    /**
     * OpenAI API Key (format: sk-...)
     * Will be prefixed with "Bearer " when making requests
     */
    private String authToken;

    /**
     * OpenAI API base URL
     * Default: https://api.openai.com
     */
    private String baseUrl = "https://api.openai.com";

    /**
     * Request timeout in milliseconds
     * Default: 60 seconds
     */
    private Integer timeout = 60000;

    /**
     * Maximum number of retries on failure
     * Default: 3
     */
    private Integer maxRetries = 3;
}
