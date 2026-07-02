package ai.nubase.ai.gateway.dto;

import lombok.Builder;
import lombok.Data;

import java.time.LocalDateTime;
import java.util.Map;
import java.util.UUID;

/**
 * API使用记录DTO
 */
@Data
@Builder
public class ApiUsageRecord {
    /**
     * API密钥
     */
    private String apiKey;

    /**
     * Authenticated auth.users id when the AI request is made with project apikey +
     * Bearer user JWT. Gateway-key calls leave this null unless the key itself is bound
     * to a user.
     */
    private UUID userId;

    /**
     * 请求ID
     */
    private String requestId;

    /**
     * 模型名称
     */
    private String model;

    /**
     * 请求端点
     */
    private String endpoint;

    /**
     * HTTP方法
     */
    private String method;

    /**
     * 响应状态码
     */
    private Integer statusCode;

    /**
     * Token使用量
     */
    private TokenUsage tokenUsage;

    /**
     * 请求耗时（毫秒）
     */
    private Long durationMs;

    /**
     * 流式请求中, 从发出到首个 token 抵达的耗时 (毫秒). 非流式 / 失败请求传 null.
     */
    private Long firstTokenLatencyMs;

    /**
     * 错误信息
     */
    private String errorMessage;

    /**
     * 请求元数据
     */
    private Map<String, Object> requestMetadata;

    /**
     * 创建时间
     */
    private LocalDateTime createdAt;
}
