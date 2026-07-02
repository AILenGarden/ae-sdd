package ai.nubase.ai.gateway.dto;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * 创建或更新上游配置的请求
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class UpstreamConfigRequest {

    /**
     * 配置名称
     */
    private String name;

    /**
     * API 基础 URL
     */
    private String baseUrl;

    /**
     * API 认证 Token
     */
    private String authToken;

    /**
     * 配置描述
     */
    private String description;

    /**
     * 是否为默认上游
     */
    private Boolean isDefault;

    /**
     * 是否激活
     */
    private Boolean isActive;

    /**
     * 请求超时时间（毫秒）
     */
    private Integer timeoutMs;

    /**
     * 最大重试次数
     */
    private Integer maxRetries;

    /**
     * 优先级（数字越小优先级越高，例如：1 > 10）
     */
    private Integer priority;

    /**
     * 渠道侧支持的最大输入 token 数。NULL/<=0 表示不裁剪。
     */
    private Integer maxInputTokens;
}
