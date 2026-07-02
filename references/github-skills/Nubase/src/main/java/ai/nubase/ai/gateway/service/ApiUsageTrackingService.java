package ai.nubase.ai.gateway.service;

import ai.nubase.ai.gateway.dto.ApiUsageRecord;
import ai.nubase.ai.gateway.dto.TokenUsage;
import ai.nubase.ai.gateway.entity.ApiKey;
import ai.nubase.ai.gateway.entity.ApiUsageLog;
import ai.nubase.ai.gateway.repository.ApiKeyRepository;
import ai.nubase.ai.gateway.repository.ApiUsageLogRepository;
import ai.nubase.ai.gateway.repository.DailyTokenUsageRepository;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.security.core.Authentication;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.LocalDate;
import java.time.LocalDateTime;
import java.util.HashMap;
import java.util.Map;
import java.util.UUID;

/**
 * API使用量跟踪服务
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class ApiUsageTrackingService {

    private final ApiKeyRepository apiKeyRepository;
    private final DailyTokenUsageRepository dailyTokenUsageRepository;
    private final ApiUsageLogRepository apiUsageLogRepository;
    private final PricingService pricingService;
    private final ObjectMapper objectMapper;

    /**
     * 从Claude API响应中提取token使用量
     */
    public TokenUsage extractTokenUsage(String responseBody) {
        try {
            JsonNode jsonNode = objectMapper.readTree(responseBody);
            JsonNode usage = jsonNode.get("usage");
            if (usage != null && usage.isObject()) {
                return buildClaudeTokenUsage(usage);
            }
        } catch (Exception e) {
            log.warn("Failed to extract token usage from response: {}", e.getMessage());
        }

        return TokenUsage.empty();
    }

    /**
     * 从Claude API流式响应事件中提取token使用量
     * 流式响应事件可能在 message_start.message.usage 或 message_delta.usage 中携带用量。
     */
    public TokenUsage extractTokenUsageFromStreamEvent(String eventData) {
        try {
            JsonNode jsonNode = objectMapper.readTree(eventData);
            JsonNode usage = findClaudeStreamUsageNode(jsonNode);
            if (usage != null && usage.isObject()) {
                return buildClaudeTokenUsage(usage);
            }
        } catch (Exception e) {
            log.debug("Not a usage event or failed to parse: {}", e.getMessage());
        }

        return null;
    }

    private JsonNode findClaudeStreamUsageNode(JsonNode eventNode) {
        JsonNode usage = eventNode.get("usage");
        if (usage != null && usage.isObject()) {
            return usage;
        }

        JsonNode message = eventNode.get("message");
        if (message != null) {
            usage = message.get("usage");
            if (usage != null && usage.isObject()) {
                return usage;
            }
        }

        return null;
    }

    private TokenUsage buildClaudeTokenUsage(JsonNode usage) {
        int inputTokens = usage.has("input_tokens") ? usage.get("input_tokens").asInt() : 0;
        int outputTokens = usage.has("output_tokens") ? usage.get("output_tokens").asInt() : 0;
        int cacheCreationInputTokens = usage.has("cache_creation_input_tokens")
                ? usage.get("cache_creation_input_tokens").asInt()
                : 0;
        int cacheReadInputTokens = usage.has("cache_read_input_tokens")
                ? usage.get("cache_read_input_tokens").asInt()
                : 0;

        return TokenUsage.builder()
                .inputTokens(inputTokens)
                .outputTokens(outputTokens)
                .totalTokens(inputTokens + outputTokens)
                .cacheCreationInputTokens(cacheCreationInputTokens)
                .cacheReadInputTokens(cacheReadInputTokens)
                .build();
    }

    /**
     * 从请求体中提取模型名称
     */
    public String extractModelFromRequest(String requestBody) {
        try {
            JsonNode jsonNode = objectMapper.readTree(requestBody);
            if (jsonNode.has("model")) {
                return jsonNode.get("model").asText();
            }
        } catch (Exception e) {
            log.debug("Failed to extract model from request: {}", e.getMessage());
        }
        return "unknown";
    }

    /**
     * 记录API使用情况。
     * <p>
     * 同步执行（不可用 {@code @Async}）：用量统计依赖 {@code MultiTenancyContext} 这个 ThreadLocal
     * 把 JPA 写入路由到当前租户库；异步线程会丢失该上下文，导致写到 GuardianDataSource 失败。
     */
    @Transactional
    public void trackUsage(ApiUsageRecord record) {
        try {
            String apiKey = record.getApiKey();
            UUID authenticatedUserId = resolveAuthenticatedUserId(record);

            // 1. 反查网关 key + 更新 last_used_at。Auth 用户 JWT 调用没有 nbk key,
            // 但仍应记录到 user_id 维度。
            ApiKey keyRow = upsertApiKeyLastUsed(apiKey);
            if (keyRow.getUserId() == null && authenticatedUserId != null) {
                keyRow.setUserId(authenticatedUserId);
            }

            // 2. 计算成本 (找不到模型定价时为 0)，仅用于统计，不做任何扣费
            PricingService.Cost cost = pricingService.computeCost(record.getModel(), record.getTokenUsage());
            TokenUsage usageForLog = record.getTokenUsage() == null ? TokenUsage.empty() : record.getTokenUsage();
            log.info("api_usage.persist requestId={} apiKeyId={} endpoint={} model={} status={} tokens={}/{}/{} costUsd={} costCny={}",
                    record.getRequestId(), keyRow.getId(), record.getEndpoint(),
                    record.getModel(), record.getStatusCode(),
                    usageForLog.getInputTokens(),
                    usageForLog.getOutputTokens(),
                    usageForLog.getTotalTokens(),
                    cost.getUsd(), cost.getCny());

            // 3. 记录详细日志（带 api_key_id / cost_usd）
            saveUsageLog(record, keyRow, cost.getUsd());

            // 4. 更新每日统计（按 api_key_id 归集，租户库本身即项目边界）
            updateDailyStats(record, keyRow, cost.getUsd(), cost.getCny());

            log.debug("Usage tracked successfully for API key: {}", maskApiKey(apiKey));

        } catch (Exception e) {
            log.error("Failed to track API usage: {}", e.getMessage(), e);
        }
    }

    /**
     * 按完整密钥的哈希反查 api_keys 并更新 last_used_at。
     * <p>数据面已由 {@code GatewayApiKeyAuthFilter} 校验过密钥，这里通常一定命中；万一未命中
     * （非自路由密钥等），返回一个仅含展示前缀的瞬态对象，使日志仍可落库（api_key_id 为空）。
     */
    private ApiKey upsertApiKeyLastUsed(String apiKey) {
        if (apiKey == null || apiKey.isBlank()) {
            return ApiKey.builder()
                    .keyPrefix("auth-user")
                    .isActive(true)
                    .build();
        }
        String hash = ai.nubase.ai.gateway.util.GatewayKeyUtil.sha256Hex(apiKey);
        return apiKeyRepository.findByKeyHash(hash)
                .map(key -> {
                    key.setLastUsedAt(LocalDateTime.now());
                    return apiKeyRepository.save(key);
                })
                .orElseGet(() -> ApiKey.builder()
                        .keyPrefix(ai.nubase.ai.gateway.util.GatewayKeyUtil.displayPrefix(apiKey))
                        .isActive(true)
                        .build());
    }

    /**
     * 保存使用日志
     */
    private void saveUsageLog(ApiUsageRecord record, ApiKey keyRow, java.math.BigDecimal costUsd) {
        TokenUsage usage = record.getTokenUsage();
        if (usage == null) {
            usage = TokenUsage.empty();
        }

        ApiUsageLog log = ApiUsageLog.builder()
                .userId(keyRow.getUserId())
                .apiKeyId(keyRow.getId())
                // 只存脱敏前缀，避免把明文密钥写进日志表
                .apiKey(keyRow.getKeyPrefix())
                .requestId(record.getRequestId())
                .model(record.getModel())
                .endpoint(record.getEndpoint())
                .method(record.getMethod())
                .statusCode(record.getStatusCode())
                .inputTokens(usage.getInputTokens())
                .outputTokens(usage.getOutputTokens())
                .totalTokens(usage.getTotalTokens())
                .cacheCreationInputTokens(usage.getCacheCreationInputTokens())
                .cacheReadInputTokens(usage.getCacheReadInputTokens())
                .costUsd(costUsd)
                .durationMs(record.getDurationMs())
                .firstTokenLatencyMs(record.getFirstTokenLatencyMs())
                .errorMessage(record.getErrorMessage())
                .requestMetadata(record.getRequestMetadata())
                .build();

        apiUsageLogRepository.save(log);
    }

    private UUID resolveAuthenticatedUserId(ApiUsageRecord record) {
        if (record.getUserId() != null) {
            return record.getUserId();
        }
        Authentication authentication = SecurityContextHolder.getContext().getAuthentication();
        if (authentication != null
                && authentication.isAuthenticated()
                && authentication.getPrincipal() instanceof ai.nubase.auth.entity.User user) {
            return user.getId();
        }
        return null;
    }

    /**
     * 更新每日统计数据 (写入 ai_gateway.daily_token_usage)。按 (api_key_id, usage_date, model) 归集。
     */
    @Transactional
    public void updateDailyStats(ApiUsageRecord record, ApiKey keyRow,
                                 java.math.BigDecimal costUsd, java.math.BigDecimal costCny) {
        LocalDate today = LocalDate.now();
        String model = record.getModel() != null ? record.getModel() : "unknown";

        java.util.UUID userId = keyRow.getUserId();
        Long apiKeyId = keyRow.getId();

        TokenUsage usage = record.getTokenUsage();
        if (usage == null) {
            usage = TokenUsage.empty();
        }
        java.math.BigDecimal normalizedUsd = costUsd == null ? java.math.BigDecimal.ZERO : costUsd;
        java.math.BigDecimal normalizedCny = costCny == null ? java.math.BigDecimal.ZERO : costCny;
        int errorCount = record.getStatusCode() != null && record.getStatusCode() >= 400 ? 1 : 0;

        dailyTokenUsageRepository.upsertDailyUsage(
                userId,
                apiKeyId,
                today,
                model,
                errorCount,
                usage.getInputTokens(),
                usage.getOutputTokens(),
                usage.getCacheCreationInputTokens(),
                usage.getCacheReadInputTokens(),
                usage.getTotalTokens(),
                normalizedUsd,
                normalizedCny);
    }

    /**
     * 掩码API Key用于日志输出
     */
    private String maskApiKey(String apiKey) {
        if (apiKey == null || apiKey.length() <= 8) {
            return "***";
        }
        return apiKey.substring(0, 4) + "..." + apiKey.substring(apiKey.length() - 4);
    }

    /**
     * 创建请求元数据
     */
    public Map<String, Object> createRequestMetadata(String userAgent, Map<String, String> customHeaders) {
        Map<String, Object> metadata = new HashMap<>();

        if (userAgent != null) {
            metadata.put("user_agent", userAgent);
        }

        if (customHeaders != null && !customHeaders.isEmpty()) {
            metadata.put("headers", customHeaders);
        }

        return metadata;
    }
}
