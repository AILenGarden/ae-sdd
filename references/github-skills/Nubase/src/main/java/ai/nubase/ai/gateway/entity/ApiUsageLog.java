package ai.nubase.ai.gateway.entity;

import jakarta.persistence.*;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;
import org.hibernate.annotations.JdbcTypeCode;
import org.hibernate.type.SqlTypes;

import java.time.LocalDateTime;
import java.util.Map;
import java.util.UUID;

/**
 * API使用详细日志实体
 */
@Data
@NoArgsConstructor
@AllArgsConstructor
@Builder
@Entity
@Table(name = "api_usage_logs", schema = "ai_gateway")
public class ApiUsageLog {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    @Column(name = "user_id")
    private UUID userId;

    @Column(name = "api_key_id")
    private Long apiKeyId;

    /**
     * Display-only gateway key prefix or auth mode marker. Never store plaintext keys here.
     */
    @Column(name = "api_key", length = 255)
    private String apiKey;

    /**
     * 请求ID
     */
    @Column(name = "request_id", length = 100)
    private String requestId;

    /**
     * 模型名称
     */
    @Column(name = "model", length = 100)
    private String model;

    /**
     * 请求端点
     */
    @Column(name = "endpoint", length = 255)
    private String endpoint;

    /**
     * HTTP方法
     */
    @Column(name = "method", length = 10)
    private String method;

    /**
     * 响应状态码
     */
    @Column(name = "status_code")
    private Integer statusCode;

    /**
     * 输入token数
     */
    @Column(name = "input_tokens")
    private Integer inputTokens = 0;

    /**
     * 输出token数
     */
    @Column(name = "output_tokens")
    private Integer outputTokens = 0;

    /**
     * 总token数
     */
    @Column(name = "total_tokens")
    private Integer totalTokens = 0;

    /**
     * 缓存创建输入token数
     */
    @Column(name = "cache_creation_input_tokens")
    private Integer cacheCreationInputTokens = 0;

    /**
     * 缓存读取输入token数
     */
    @Column(name = "cache_read_input_tokens")
    private Integer cacheReadInputTokens = 0;

    /**
     * 本次请求美元成本 (按 model_pricing 折算)
     */
    @Column(name = "cost_usd", precision = 14, scale = 6)
    private java.math.BigDecimal costUsd;

    /**
     * 请求耗时（毫秒）
     */
    @Column(name = "duration_ms")
    private Long durationMs;

    /**
     * 流式请求首个 token 返回耗时（毫秒）. 非流式 / 失败请求保持 NULL.
     */
    @Column(name = "first_token_latency_ms")
    private Long firstTokenLatencyMs;

    /**
     * 错误信息
     */
    @Column(name = "error_message", columnDefinition = "TEXT")
    private String errorMessage;

    /**
     * 请求元数据（JSON格式）
     */
    @JdbcTypeCode(SqlTypes.JSON)
    @Column(name = "request_metadata", columnDefinition = "jsonb")
    private Map<String, Object> requestMetadata;

    /**
     * 创建时间
     */
    @Column(name = "created_at")
    private LocalDateTime createdAt;

    @PrePersist
    protected void onCreate() {
        createdAt = LocalDateTime.now();
    }
}
