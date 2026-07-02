package ai.nubase.ai.gateway.entity;

import jakarta.persistence.*;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.math.BigDecimal;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.util.UUID;

/**
 * 每用户每日每模型 token 用量汇总（限流 + 报表共用）。
 */
@Data
@NoArgsConstructor
@AllArgsConstructor
@Builder
@Entity
@Table(name = "daily_token_usage", schema = "ai_gateway")
public class DailyTokenUsage {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    @Column(name = "user_id")
    private UUID userId;

    @Column(name = "api_key_id")
    private Long apiKeyId;

    @Column(name = "usage_date", nullable = false)
    private LocalDate usageDate;

    @Column(name = "model", nullable = false, length = 120)
    private String model;

    @Column(name = "request_count", nullable = false)
    private Integer requestCount;

    @Column(name = "error_count", nullable = false)
    private Integer errorCount;

    @Column(name = "input_tokens", nullable = false)
    private Long inputTokens;

    @Column(name = "output_tokens", nullable = false)
    private Long outputTokens;

    @Column(name = "cache_creation_input_tokens", nullable = false)
    private Long cacheCreationInputTokens;

    @Column(name = "cache_read_input_tokens", nullable = false)
    private Long cacheReadInputTokens;

    @Column(name = "total_tokens", nullable = false)
    private Long totalTokens;

    @Column(name = "cost_cny", nullable = false, precision = 14, scale = 6)
    private BigDecimal costCny;

    @Column(name = "cost_usd", nullable = false, precision = 14, scale = 6)
    private BigDecimal costUsd;

    @Column(name = "created_at", nullable = false)
    private LocalDateTime createdAt;

    @Column(name = "updated_at", nullable = false)
    private LocalDateTime updatedAt;

    @PrePersist
    protected void onCreate() {
        LocalDateTime now = LocalDateTime.now();
        if (createdAt == null) createdAt = now;
        if (updatedAt == null) updatedAt = now;
        if (requestCount == null) requestCount = 0;
        if (errorCount == null) errorCount = 0;
        if (inputTokens == null) inputTokens = 0L;
        if (outputTokens == null) outputTokens = 0L;
        if (cacheCreationInputTokens == null) cacheCreationInputTokens = 0L;
        if (cacheReadInputTokens == null) cacheReadInputTokens = 0L;
        if (totalTokens == null) totalTokens = 0L;
        if (costCny == null) costCny = BigDecimal.ZERO;
        if (costUsd == null) costUsd = BigDecimal.ZERO;
    }

    @PreUpdate
    protected void onUpdate() {
        updatedAt = LocalDateTime.now();
    }
}
