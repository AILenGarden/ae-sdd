package ai.nubase.ai.gateway.service;

import ai.nubase.ai.gateway.entity.UpstreamConfig;
import ai.nubase.ai.gateway.repository.UpstreamConfigRepository;
import ai.nubase.common.enums.ApiProvider;
import lombok.extern.slf4j.Slf4j;
import okhttp3.*;
import org.springframework.beans.factory.annotation.Qualifier;
import org.springframework.stereotype.Service;
import org.springframework.transaction.PlatformTransactionManager;
import org.springframework.transaction.support.TransactionTemplate;

import java.time.LocalDateTime;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.TimeUnit;

/**
 * 上游配置定时健康检查服务
 * 每 20 分钟运行一次，检查所有活跃上游的可用性
 *
 * 健康检查策略：
 * - 向上游发送一个最小化的真实 API 请求（发送 "hi"，max_tokens=1）
 * - 收到 2xx 响应 = HEALTHY（API 可用）
 * - 收到 4xx/5xx 或网络异常，重试最多 3 次后仍失败 = UNHEALTHY
 * - 状态发生变化时发送飞书通知，持续不健康不重复告警
 */
@Slf4j
@Service
public class UpstreamHealthCheckService {

    private static final String STATUS_HEALTHY = "HEALTHY";
    private static final String STATUS_UNHEALTHY = "UNHEALTHY";
    private static final MediaType JSON = MediaType.get("application/json; charset=utf-8");


    /** 单次健康检查失败后的最大重试次数 */
    private static final int MAX_RETRY_ATTEMPTS = 3;

    /** 重试间隔（毫秒） */
    private static final long RETRY_DELAY_MS = 3_000;

    // --- Health check request payloads (minimal cost: max_tokens=1) ---

    private static final String CLAUDE_HEALTH_CHECK_BODY =
            "{\"model\":\"claude-sonnet-4-6\",\"max_tokens\":1,"
                    + "\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}]}";

    /**
     * 健康检查专用 HTTP 客户端，使用较短的超时时间
     * 与业务请求的 client 隔离，避免相互影响
     */
    private static final OkHttpClient HEALTH_CHECK_CLIENT = new OkHttpClient.Builder()
            .connectTimeout(15, TimeUnit.SECONDS)
            .readTimeout(30, TimeUnit.SECONDS)
            .writeTimeout(10, TimeUnit.SECONDS)
            .followRedirects(true)
            .build();

    private final UpstreamConfigRepository upstreamConfigRepository;
    private final FeishuNotificationService feishuNotificationService;
    private final TransactionTemplate metadataTxTemplate;

    /**
     * 记录已发送过告警的上游名称集合
     * 用于去重：如果上游持续 UNHEALTHY，不重复发送飞书消息
     * 当上游恢复 HEALTHY 后从集合中移除，下次再故障时才会重新告警
     */
    private final Set<String> alertedUpstreams = ConcurrentHashMap.newKeySet();

    public UpstreamHealthCheckService(
            UpstreamConfigRepository upstreamConfigRepository,
            FeishuNotificationService feishuNotificationService,
            @Qualifier("metadataTransactionManager") PlatformTransactionManager metadataTxManager) {
        this.upstreamConfigRepository = upstreamConfigRepository;
        this.feishuNotificationService = feishuNotificationService;
        this.metadataTxTemplate = new TransactionTemplate(metadataTxManager);
    }

    /**
     * 定时健康检查任务，每 10 分钟执行一次（600,000 毫秒）
     * 初始延迟 60 秒，等待应用启动完成
     * 检查所有活跃上游，失败时重试最多 3 次
     */
//    @Scheduled(fixedRate = 900_000, initialDelay = 60_000)
    public void performScheduledHealthChecks() {
        log.info("🏥 开始执行定时上游健康检查...");

        List<UpstreamConfig> upstreams = upstreamConfigRepository
                .findByIsActiveTrueOrderByPriorityAsc();

        if (upstreams.isEmpty()) {
            log.info("🏥 没有需要检查的活跃上游");
            return;
        }

        log.info("🏥 正在检查 {} 个上游...", upstreams.size());

        int healthyCount = 0;
        int unhealthyCount = 0;

        for (UpstreamConfig upstream : upstreams) {
            HealthCheckResult result = checkWithRetry(upstream);
            if (result.isHealthy()) {
                healthyCount++;
            } else {
                unhealthyCount++;
            }
        }

        log.info("🏥 健康检查完成：{} 个健康，{} 个不健康（共检查 {} 个）",
                healthyCount, unhealthyCount, upstreams.size());
    }

    /**
     * 对单个上游执行带重试的健康检查
     * 第一次检查失败后，最多重试 MAX_RETRY_ATTEMPTS 次
     * 全部失败后才判定为 UNHEALTHY 并更新状态
     */
    private HealthCheckResult checkWithRetry(UpstreamConfig config) {
        HealthCheckResult result = performHealthCheck(config);

        if (result.isHealthy()) {
            // 首次检查就通过，直接更新状态
            return checkAndUpdateHealth(config, result);
        }

        // 首次失败，开始重试
        log.warn("🏥 上游 '{}' 首次检查失败，开始重试（最多 {} 次）...",
                config.getName(), MAX_RETRY_ATTEMPTS);

        for (int attempt = 1; attempt <= MAX_RETRY_ATTEMPTS; attempt++) {
            try {
                Thread.sleep(RETRY_DELAY_MS);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                log.warn("🏥 上游 '{}' 重试被中断", config.getName());
                break;
            }

            result = performHealthCheck(config);
            if (result.isHealthy()) {
                log.info("🏥 上游 '{}' 第 {} 次重试成功", config.getName(), attempt);
                return checkAndUpdateHealth(config, result);
            }

            log.warn("🏥 上游 '{}' 第 {}/{} 次重试仍失败: {}",
                    config.getName(), attempt, MAX_RETRY_ATTEMPTS, result.getMessage());
        }

        // 所有重试均失败
        log.error("🏥 上游 '{}' 全部 {} 次重试后仍不健康", config.getName(), MAX_RETRY_ATTEMPTS);
        return checkAndUpdateHealth(config, result);
    }

    /**
     * 对单个上游执行健康检查并更新数据库状态
     * 同时检测状态变化，在状态转换时发送飞书通知
     *
     * @param config 上游配置
     * @return 健康检查结果
     */
    public HealthCheckResult checkAndUpdateHealth(UpstreamConfig config) {
        HealthCheckResult result = performHealthCheck(config);
        return checkAndUpdateHealth(config, result);
    }

    /**
     * 根据已有的检查结果更新数据库状态并触发通知
     */
    private HealthCheckResult checkAndUpdateHealth(UpstreamConfig config, HealthCheckResult result) {
        // Read previous status directly from DB to avoid JPA L1 cache stale data
        String previousStatus = metadataTxTemplate.execute(status ->
                upstreamConfigRepository.findHealthStatusById(config.getId()));

        updateHealthStatus(config, result);

        // Detect state transitions and send Feishu notifications
        String newStatus = result.isHealthy() ? STATUS_HEALTHY : STATUS_UNHEALTHY;
        log.debug("🔔 上游 '{}' 状态判定: DB中的 previousStatus={}, 本次 newStatus={}",
                config.getName(), previousStatus, newStatus);
        notifyOnStatusChange(config, previousStatus, newStatus, result);

        return result;
    }

    /**
     * 检测健康状态变化并触发飞书通知
     * 使用 alertedUpstreams 集合去重，确保：
     * - HEALTHY → UNHEALTHY: 发送告警，标记为已告警
     * - 持续 UNHEALTHY: 不重复发送
     * - UNHEALTHY → HEALTHY: 发送恢复通知，移除已告警标记
     */
    private void notifyOnStatusChange(UpstreamConfig config, String previousStatus,
                                      String newStatus, HealthCheckResult result) {
        String upstreamName = config.getName();

        if (STATUS_HEALTHY.equals(newStatus)) {
            // 恢复健康：如果之前已告警过，发送恢复通知并清除标记
            if (alertedUpstreams.remove(upstreamName)) {
                log.info("🔔 上游 '{}' 已恢复，发送飞书恢复通知", upstreamName);
                feishuNotificationService.notifyUpstreamRecovered(config);
            } else if (!STATUS_HEALTHY.equals(previousStatus) && previousStatus != null) {
                // DB 中之前不是 HEALTHY（比如手动修改后的场景），也发恢复通知
                log.info("🔔 上游 '{}' 状态变化: {} → HEALTHY，发送飞书恢复通知",
                        upstreamName, previousStatus);
                feishuNotificationService.notifyUpstreamRecovered(config);
            }
        } else if (STATUS_UNHEALTHY.equals(newStatus)) {
            // 不健康：仅在尚未告警时发送
            if (alertedUpstreams.add(upstreamName)) {
                log.warn("🔔 上游 '{}' 不健康（{} → UNHEALTHY），发送飞书告警",
                        upstreamName, previousStatus != null ? previousStatus : "UNKNOWN");
                feishuNotificationService.notifyUpstreamDown(config, result.getMessage());
            } else {
                log.info("🔔 上游 '{}' 持续不健康，已告警过，跳过重复通知", upstreamName);
            }
        }
    }

    /**
     * 对单个上游执行真实 API 请求检查
     * 按 provider 类型构造最小化请求（发送 "hi"，max_tokens=1）
     * 只有 2xx 响应 = HEALTHY，4xx/5xx/网络异常 = UNHEALTHY
     * 对于 OPENAI，会按顺序尝试多个低成本模型，任一成功即视为健康
     *
     * @param config 上游配置
     * @return 健康检查结果
     */
    public HealthCheckResult performHealthCheck(UpstreamConfig config) {
        log.debug("🏥 正在检查上游 '{}' (provider={}, baseUrl={})",
                config.getName(), config.getProvider(), config.getBaseUrl());

        long startTime = System.currentTimeMillis();

        try {
            List<HealthCheckProbe> probes = buildHealthCheckProbes(config);
            List<String> failureMessages = new ArrayList<>();

            for (HealthCheckProbe probe : probes) {
                try (Response response = HEALTH_CHECK_CLIENT.newCall(probe.request()).execute()) {
                    long responseTime = System.currentTimeMillis() - startTime;
                    String responseBody = response.body() != null ? response.body().string() : "";

                    if (response.isSuccessful()) {
                        String message = String.format("%s: HTTP %d - API 可用（响应时间: %dms）",
                                probe.name(), response.code(), responseTime);
//                        log.info("🏥 ✅ 上游 '{}' 健康 [{}] (HTTP {}, {}ms)",
//                                config.getName(), probe.name(), response.code(), responseTime);
                        return new HealthCheckResult(true, message, responseTime);
                    }

                    String message = String.format("%s: HTTP %d - API 不可用（响应时间: %dms，响应: %s）",
                            probe.name(), response.code(), responseTime, truncate(responseBody, 200));
                    log.warn("🏥 ❌ 上游 '{}' 探活失败 [{}] (HTTP {}, {}ms): {}",
                            config.getName(), probe.name(), response.code(), responseTime, truncate(responseBody, 200));
                    failureMessages.add(message);
                } catch (Exception e) {
                    long responseTime = System.currentTimeMillis() - startTime;
                    String message = String.format("%s: %s: %s",
                            probe.name(), e.getClass().getSimpleName(), e.getMessage());
                    log.warn("🏥 ❌ 上游 '{}' 探活异常 [{}] ({}ms): {}",
                            config.getName(), probe.name(), responseTime, message);
                    failureMessages.add(message);
                }
            }

            long responseTime = System.currentTimeMillis() - startTime;
            String message = String.join("; ", failureMessages);
            log.warn("🏥 ❌ 上游 '{}' 所有探活模型均失败 ({}ms): {}",
                    config.getName(), responseTime, message);
            return new HealthCheckResult(false, message, responseTime);
        } catch (Exception e) {
            long responseTime = System.currentTimeMillis() - startTime;
            String errorMessage = e.getClass().getSimpleName() + ": " + e.getMessage();

            log.warn("🏥 ❌ 上游 '{}' 不健康: {} ({}ms)",
                    config.getName(), errorMessage, responseTime);
            return new HealthCheckResult(false, errorMessage, responseTime);
        }
    }

    /**
     * 根据 provider 类型构建健康检查 HTTP 请求
     * 每种 provider 使用其最廉价的模型和最小 token 消耗。
     * OPENAI 返回按优先级排列的探活模型列表，任一成功即可视为健康。
     */
    List<HealthCheckProbe> buildHealthCheckProbes(UpstreamConfig config) {
        ApiProvider provider = config.getProvider();
        String baseUrl = config.getBaseUrl();

        String authToken = config.getAuthToken();
        return switch (provider) {
            case CLAUDE -> List.of(new HealthCheckProbe(
                    "claude-sonnet-4-6",
                    new Request.Builder()
                            .url(baseUrl + "/v1/messages")
                            .post(RequestBody.create(CLAUDE_HEALTH_CHECK_BODY, JSON))
                            .addHeader("x-api-key", authToken)
                            .addHeader("anthropic-version", "2023-06-01")
                            .addHeader("Content-Type", "application/json")
                            .build()
            ));

            case OPENAI -> buildOpenAICompatibleProbes(config, baseUrl, authToken);

        };
    }

    private List<HealthCheckProbe> buildOpenAICompatibleProbes(
            UpstreamConfig config, String baseUrl, String authToken) {
        if ("deepseek".equals(resolveChannelCode(config))) {
            return List.of(buildOpenAIProbe(baseUrl, authToken, "deepseek-v4-pro", "/chat/completions"));
        }

        return List.of(
                buildOpenAIProbe(baseUrl, authToken, "gpt-5.4-mini", "/v1/chat/completions"),
                buildOpenAIProbe(baseUrl, authToken, "glm-5", "/v1/chat/completions")
        );
    }

    private String resolveChannelCode(UpstreamConfig config) {
        String channelCode = config.getChannelCode();
        if (channelCode != null && !channelCode.isBlank()) {
            return channelCode.trim().toLowerCase(Locale.ROOT);
        }
        ApiProvider provider = config.getProvider();
        return provider == null ? "" : provider.name().toLowerCase(Locale.ROOT);
    }

    private HealthCheckProbe buildOpenAIProbe(String baseUrl, String authToken, String model, String path) {
        String requestBody = String.format(
                "{\"model\":\"%s\",\"max_tokens\":1,\"temperature\":0,\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}]}",
                model
        );
        String url = baseUrl.endsWith("/")
                ? baseUrl.substring(0, baseUrl.length() - 1) + path
                : baseUrl + path;

        return new HealthCheckProbe(
                model,
                new Request.Builder()
                        .url(url)
                        .post(RequestBody.create(requestBody, null))
                        .addHeader("Authorization", "Bearer " + authToken)
                        .addHeader("Content-Type", "application/json")
                        .build()
        );
    }

    /**
     * Truncate a string to the given max length, appending "..." if truncated.
     */
    private String truncate(String text, int maxLength) {
        if (text == null) {
            return "";
        }
        return text.length() <= maxLength ? text : text.substring(0, maxLength) + "...";
    }

    /**
     * 检查给定的上游配置当前是否标记为健康
     *
     * @param config 上游配置
     * @return 健康状态为 HEALTHY 时返回 true
     */
    public boolean isHealthy(UpstreamConfig config) {
        return STATUS_HEALTHY.equals(config.getHealthStatus());
    }

    /**
     * 更新上游配置的健康状态字段并持久化到数据库
     * 使用 JPQL UPDATE 直接写入，绕过 JPA 实体状态管理
     */
    private void updateHealthStatus(UpstreamConfig config, HealthCheckResult result) {
        try {
            String newStatus = result.isHealthy() ? STATUS_HEALTHY : STATUS_UNHEALTHY;
            String message = result.getMessage();
            LocalDateTime now = LocalDateTime.now();

            int updatedRows = metadataTxTemplate.execute(status ->
                    upstreamConfigRepository.updateHealthStatusById(
                            config.getId(), newStatus, message, now));

            if (updatedRows > 0) {
                // Also update the in-memory entity for subsequent notification logic
                config.setHealthStatus(newStatus);
                config.setHealthMessage(message);
                config.setLastHealthCheck(now);
//                log.info("🏥 上游 '{}' (id={}) 健康状态已更新为: {}，影响 {} 行",
//                        config.getName(), config.getId(), newStatus, updatedRows);
            } else {
                log.error("🏥 上游 '{}' (id={}) 健康状态更新失败：JPQL UPDATE 影响 0 行",
                        config.getName(), config.getId());
            }
        } catch (Exception e) {
            log.error("🏥 更新上游 '{}' 健康状态失败: {}",
                    config.getName(), e.getMessage(), e);
        }
    }

    /**
     * 健康检查结果
     */
    public static class HealthCheckResult {
        private final boolean healthy;
        private final String message;
        private final long responseTimeMs;

        public HealthCheckResult(boolean healthy, String message, long responseTimeMs) {
            this.healthy = healthy;
            this.message = message;
            this.responseTimeMs = responseTimeMs;
        }

        public boolean isHealthy() {
            return healthy;
        }

        public String getMessage() {
            return message;
        }

        public long getResponseTimeMs() {
            return responseTimeMs;
        }
    }

    static final class HealthCheckProbe {
        private final String name;
        private final Request request;

        HealthCheckProbe(String name, Request request) {
            this.name = name;
            this.request = request;
        }

        String name() {
            return name;
        }

        Request request() {
            return request;
        }
    }
}
