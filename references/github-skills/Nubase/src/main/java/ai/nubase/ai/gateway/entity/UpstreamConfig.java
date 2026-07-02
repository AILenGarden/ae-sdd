package ai.nubase.ai.gateway.entity;

import ai.nubase.common.enums.ApiProvider;
import jakarta.persistence.*;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;
import org.hibernate.annotations.JdbcTypeCode;
import org.hibernate.type.SqlTypes;

import java.time.LocalDateTime;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * 上游 API 配置实体
 * 支持配置多个不同的上游 API 提供商（Claude, OpenAI-compatible 等）
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@Entity
@Table(name = "upstream_configs", schema = "ai_gateway")
public class UpstreamConfig {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    /**
     * 配置名称，用于标识不同的上游
     * 例如：anthropic-official, anthropic-proxy-1
     */
    @Column(nullable = false, unique = true, length = 100)
    private String name;

    /**
     * API 基础 URL
     * 例如：https://api.anthropic.com
     */
    @Column(name = "base_url", nullable = false, length = 500)
    private String baseUrl;

    /**
     * API 认证 Token（加密存储）
     */
    @Column(name = "auth_token", nullable = false, columnDefinition = "TEXT")
    private String authToken;

    /**
     * API 提供商类型：CLAUDE / OPENAI
     */
    @Enumerated(EnumType.STRING)
    @Column(name = "provider", nullable = false, length = 20)
    @Builder.Default
    private ApiProvider provider = ApiProvider.CLAUDE;

    /**
     * Routing channel for OpenAI-compatible native APIs.
     * Examples: openai, deepseek, openrouter.
     */
    @Column(name = "channel_code", length = 64)
    private String channelCode;

    /**
     * Supported model names for model-based routing.
     */
    @JdbcTypeCode(SqlTypes.JSON)
    @Column(name = "supported_models", nullable = false, columnDefinition = "jsonb")
    @Builder.Default
    private List<String> supportedModels = new ArrayList<>();

    /**
     * Chat completions endpoint path for this upstream.
     */
    @Column(name = "chat_completions_path", nullable = false, length = 128)
    @Builder.Default
    private String chatCompletionsPath = "/v1/chat/completions";

    /**
     * 配置描述
     */
    @Column(columnDefinition = "TEXT")
    private String description;

    /**
     * 是否为默认上游
     */
    @Column(name = "is_default", nullable = false)
    @Builder.Default
    private Boolean isDefault = false;

    /**
     * 是否激活
     */
    @Column(name = "is_active", nullable = false)
    @Builder.Default
    private Boolean isActive = true;

    /**
     * 请求超时时间（毫秒）
     */
    @Column(name = "timeout_ms", nullable = false)
    @Builder.Default
    private Integer timeoutMs = 60000;

    /**
     * 最大重试次数
     */
    @Column(name = "max_retries", nullable = false)
    @Builder.Default
    private Integer maxRetries = 3;

    /**
     * 优先级（数字越小优先级越高）
     * 可用于负载均衡或故障转移
     * 例如：priority=1 比 priority=10 优先级更高
     */
    @Column(nullable = false)
    @Builder.Default
    private Integer priority = 0;

    /**
     * 渠道侧支持的最大输入 token 数（按 chars/4 粗略估算）。
     * NULL 或 <=0 表示该渠道不需要网关裁剪（直连 Anthropic 200K/1M 等）。
     * sub2api → Codex 这类有产品级软墙的渠道建议设为 180000 左右。
     */
    @Column(name = "max_input_tokens")
    private Integer maxInputTokens;

    /**
     * 创建时间
     */
    @Column(name = "created_at", nullable = false, updatable = false)
    private LocalDateTime createdAt;

    /**
     * 更新时间
     */
    @Column(name = "updated_at", nullable = false)
    private LocalDateTime updatedAt;

    /**
     * 最后使用时间
     */
    @Column(name = "last_used_at")
    private LocalDateTime lastUsedAt;

    /**
     * 最后健康检查时间
     */
    @Column(name = "last_health_check")
    private LocalDateTime lastHealthCheck;

    /**
     * 健康状态：HEALTHY（健康）、UNHEALTHY（不健康）、UNKNOWN（未知）
     */
    @Column(name = "health_status", length = 20)
    private String healthStatus;

    /**
     * 健康检查消息（错误信息或成功信息）
     */
    @Column(name = "health_message", columnDefinition = "TEXT")
    private String healthMessage;

    @PrePersist
    protected void onCreate() {
        createdAt = LocalDateTime.now();
        updatedAt = LocalDateTime.now();
        if (healthStatus == null) {
            healthStatus = "UNKNOWN";
        }
        if ((channelCode == null || channelCode.isBlank()) && provider != null) {
            channelCode = provider.name().toLowerCase(Locale.ROOT);
        }
        if (supportedModels == null) {
            supportedModels = new ArrayList<>();
        }
        if (chatCompletionsPath == null || chatCompletionsPath.isBlank()) {
            chatCompletionsPath = "/v1/chat/completions";
        }
    }

    @PreUpdate
    protected void onUpdate() {
        updatedAt = LocalDateTime.now();
        if ((channelCode == null || channelCode.isBlank()) && provider != null) {
            channelCode = provider.name().toLowerCase(Locale.ROOT);
        }
        if (supportedModels == null) {
            supportedModels = new ArrayList<>();
        }
        if (chatCompletionsPath == null || chatCompletionsPath.isBlank()) {
            chatCompletionsPath = "/v1/chat/completions";
        }
    }

    /**
     * 更新最后使用时间
     */
    public void updateLastUsedAt() {
        this.lastUsedAt = LocalDateTime.now();
    }
}
