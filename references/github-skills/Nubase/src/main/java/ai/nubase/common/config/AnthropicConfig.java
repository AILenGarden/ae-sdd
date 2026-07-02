package ai.nubase.common.config;

import lombok.Data;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.stereotype.Component;

/**
 * Anthropic Claude 默认上游配置（全局回退）。
 * <p>
 * 项目级路由优先使用各租户库 {@code ai_gateway.upstream_configs} 里的上游；当没有可用的项目级
 * 上游时，转发服务回退到这里的全局默认值（通常来自环境变量）。
 */
@Data
@Component
@ConfigurationProperties(prefix = "anthropic")
public class AnthropicConfig {

    /** 上游 API Key（x-api-key）。 */
    private String authToken = "";

    /** 上游基础地址。 */
    private String baseUrl = "https://api.anthropic.com";

    /** 请求超时（毫秒）。 */
    private int timeout = 60000;

    /** anthropic-version 头默认值。 */
    private String version = "2023-06-01";

    /** 请求日志开关。 */
    private Logging logging = new Logging();

    @Data
    public static class Logging {
        /** 是否记录请求/响应明细日志。 */
        private boolean enabled = false;
    }
}
