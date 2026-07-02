package ai.nubase.ai.gateway.config;

import lombok.Data;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.context.annotation.Configuration;

/**
 * Feishu (Lark) webhook notification configuration.
 */
@Data
@Configuration
@ConfigurationProperties(prefix = "feishu")
public class FeishuConfig {

    /**
     * Feishu Bot webhook URL
     */
    private String webhookUrl;

    /**
     * Whether Feishu notifications are enabled
     */
    private boolean enabled = true;
}
