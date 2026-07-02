package ai.nubase.ai.gateway.service;

import ai.nubase.ai.gateway.dto.ApiUsageRecord;
import ai.nubase.ai.gateway.dto.TokenUsage;
import ai.nubase.ai.gateway.entity.UpstreamConfig;
import ai.nubase.common.config.OpenAIConfig;
import ai.nubase.common.enums.ApiProvider;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ObjectNode;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import okhttp3.*;
import okhttp3.sse.EventSource;
import okhttp3.sse.EventSourceListener;
import okhttp3.sse.EventSources;
import org.springframework.stereotype.Service;
import org.springframework.web.servlet.mvc.method.annotation.SseEmitter;

import java.io.IOException;
import java.util.*;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;

/**
 * OpenAI 原生 API 服务
 * 直接透传 OpenAI 原生格式的请求和响应，不经过 Claude 格式转换
 * 保留 credit 消耗、上游故障转移、usage 追踪等逻辑
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class OpenAINativeApiService {

    private static final String DEFAULT_CHANNEL_CODE = "openai";
    private static final ApiProvider DEFAULT_PROVIDER = ApiProvider.OPENAI;
    private static final String CHAT_COMPLETIONS_PATH = "/v1/chat/completions";
    private static final String RESPONSES_PATH = "/v1/responses";
    private static final String RESPONSES_COMPACT_PATH = "/v1/responses/compact";
    private static final String MEMORIES_TRACE_SUMMARIZE_PATH = "/v1/memories/trace_summarize";
    /** Compact / 摘要类: 上游可能要 90~180s, 默认 60s 不够; 用 per-call callTimeout 单独放宽 */
    private static final long SLOW_CALL_TIMEOUT_MS = 300_000L;
    private static final NativeOpenAIRoute DEFAULT_ROUTE =
            NativeOpenAIRoute.defaultProvider(DEFAULT_PROVIDER);
    private static final NativeOpenAIRoute RESPONSES_ROUTE =
            NativeOpenAIRoute.responsesChannel(DEFAULT_CHANNEL_CODE, RESPONSES_PATH, 0L);
    private static final NativeOpenAIRoute RESPONSES_COMPACT_ROUTE =
            NativeOpenAIRoute.responsesChannel(DEFAULT_CHANNEL_CODE, RESPONSES_COMPACT_PATH, SLOW_CALL_TIMEOUT_MS);
    private static final NativeOpenAIRoute MEMORIES_TRACE_SUMMARIZE_ROUTE =
            NativeOpenAIRoute.responsesChannel(DEFAULT_CHANNEL_CODE, MEMORIES_TRACE_SUMMARIZE_PATH, SLOW_CALL_TIMEOUT_MS);

    private final OpenAIConfig openAIConfig;
    private final ObjectMapper objectMapper;
    private final ApiUsageTrackingService usageTrackingService;
    private final ApiRequestLogService requestLogService;
    private final UpstreamConfigService upstreamConfigService;

    private OkHttpClient httpClient;

    /**
     * 初始化 HTTP 客户端
     */
    private OkHttpClient getHttpClient() {
        if (httpClient == null) {
            httpClient = new OkHttpClient.Builder()
                    .connectTimeout(openAIConfig.getTimeout(), TimeUnit.MILLISECONDS)
                    .readTimeout(openAIConfig.getTimeout(), TimeUnit.MILLISECONDS)
                    .writeTimeout(openAIConfig.getTimeout(), TimeUnit.MILLISECONDS)
                    .build();
        }
        return httpClient;
    }

    /**
     * 获取上游配置信息
     *
     * @param upstreamName 上游名称（可选）
     * @return 上游配置信息
     */
    private UpstreamInfo getUpstreamInfo(String upstreamName, NativeOpenAIRoute route) throws IOException {
        if (upstreamName != null && !upstreamName.isBlank()) {
            UpstreamConfig config;
            try {
                config = upstreamConfigService.getByName(upstreamName);
            } catch (Exception e) {
                throw new IOException("Unable to get upstream config: " + upstreamName, e);
            }

            if (route.usesChannelCode()) {
                try {
                    validateChannel(config, route);
                } catch (IllegalArgumentException e) {
                    throw new IOException(e.getMessage(), e);
                }
            }
            if (route.hasModelConstraint()) {
                try {
                    validateAllowedModel(config, routeModelForSelection(route.supportedModel(), route));
                } catch (IllegalArgumentException e) {
                    throw new IOException(e.getMessage(), e);
                }
            }

            log.info("Using specified OpenAI-compatible upstream: {} ({})",
                    upstreamName, route.routingLabel(routeModelForSelection(route.supportedModel(), route)));
            upstreamConfigService.updateLastUsedAt(config.getName());
            return toUpstreamInfo(config);
        }

        try {
            UpstreamConfig config = getDefaultUpstream(route);
            log.info("Using default OpenAI-compatible upstream: {} ({})",
                    config.getName(), route.routingLabel(routeModelForSelection(route.supportedModel(), route)));

            upstreamConfigService.updateLastUsedAt(config.getName());

            return toUpstreamInfo(config);
        } catch (Exception e) {
            if (route.usesSupportedModel() && route.provider() == null && !route.usesChannelCode()) {
                throw new IOException("Unable to get upstream config for model: " + route.supportedModel(), e);
            }
            if (route.usesChannelCode() && !DEFAULT_CHANNEL_CODE.equals(route.channelCode())) {
                throw new IOException("Unable to get upstream config for channel: " + route.channelCode(), e);
            }

            log.warn("Unable to get OpenAI upstream config from database, using config file: {}", e.getMessage());
            return new UpstreamInfo(
                    "config-file",
                    openAIConfig.getBaseUrl(),
                    openAIConfig.getAuthToken(),
                    openAIConfig.getTimeout(),
                    CHAT_COMPLETIONS_PATH);
        }
    }

    private UpstreamConfig getDefaultUpstream(NativeOpenAIRoute route) {
        String selectionModel = routeModelForSelection(route.supportedModel(), route);
        if (route.usesChannelCode()) {
            return upstreamConfigService.selectForChannelAndModel(route.channelCode(), selectionModel);
        }
        if (route.provider() != null) {
            return upstreamConfigService.selectForProviderAndModel(route.provider(), selectionModel);
        }
        if (route.usesSupportedModel()) {
            return upstreamConfigService.getDefaultBySupportedModel(selectionModel);
        }
        throw new IllegalStateException("Unsupported OpenAI route: " + route);
    }

    private List<UpstreamConfig> getFailoverUpstreams(NativeOpenAIRoute route, List<String> triedUpstreams) {
        String selectionModel = routeModelForSelection(route.supportedModel(), route);
        if (route.usesChannelCode()) {
            return upstreamConfigService.getFailoverUpstreamsByChannelAndModel(
                    route.channelCode(), selectionModel, triedUpstreams);
        }
        if (route.provider() != null) {
            return upstreamConfigService.getFailoverUpstreamsByProviderAndModel(
                    route.provider(), selectionModel, triedUpstreams);
        }
        if (route.usesSupportedModel()) {
            return upstreamConfigService.getFailoverUpstreamsBySupportedModel(
                    selectionModel, triedUpstreams);
        }
        return List.of();
    }

    private UpstreamInfo toUpstreamInfo(UpstreamConfig config) {
        return new UpstreamInfo(
                config.getName(),
                config.getBaseUrl(),
                config.getAuthToken(),
                config.getTimeoutMs(),
                normalizeChatCompletionsPath(config.getChatCompletionsPath()));
    }

    private void validateChannel(UpstreamConfig config, NativeOpenAIRoute route) {
        String actualChannelCode = resolveChannelCode(config);
        if (!route.channelCode().equals(actualChannelCode)) {
            throw new IllegalArgumentException("upstream channel mismatch: upstream=" + config.getName()
                    + ", expected=" + route.channelCode() + ", actual=" + actualChannelCode);
        }
    }

    private String resolveChannelCode(UpstreamConfig config) {
        String channelCode = config.getChannelCode();
        if (channelCode != null && !channelCode.isBlank()) {
            return channelCode.trim().toLowerCase(Locale.ROOT);
        }
        ApiProvider provider = config.getProvider();
        return provider == null ? DEFAULT_CHANNEL_CODE : provider.name().toLowerCase(Locale.ROOT);
    }

    private void validateAllowedModel(UpstreamConfig config, String model) {
        if (!upstreamConfigService.allowsModel(config, model)) {
            throw new IllegalArgumentException("upstream model mismatch: upstream=" + config.getName()
                    + ", model=" + model);
        }
    }

    private NativeOpenAIRoute resolveRoute(String model) {
        if (model == null || model.isBlank()) {
            return DEFAULT_ROUTE;
        }
        String normalizedModel = model.trim().toLowerCase(Locale.ROOT);
        NativeOpenAIRoute prefixRoute = resolvePrefixedChannelRoute(normalizedModel);
        if (prefixRoute != null) {
            return prefixRoute;
        }
        return DEFAULT_ROUTE.withSupportedModel(normalizedModel);
    }

    private NativeOpenAIRoute resolvePrefixedChannelRoute(String normalizedModel) {
        int separatorIndex = normalizedModel.indexOf('/');
        if (separatorIndex <= 0 || separatorIndex == normalizedModel.length() - 1) {
            return null;
        }

        String channelCode = normalizedModel.substring(0, separatorIndex);
        try {
            if (upstreamConfigService.hasActiveUpstreamForChannelCode(channelCode)) {
                return NativeOpenAIRoute.chatPrefix(channelCode + "/", channelCode, normalizedModel);
            }
        } catch (Exception e) {
            log.warn("Unable to resolve channel route from model prefix '{}', falling back: {}",
                    channelCode, e.getMessage());
        }
        return null;
    }

    private String buildEndpointUrl(String baseUrl, String path) {
        String normalizedBaseUrl = baseUrl.endsWith("/") ? baseUrl.substring(0, baseUrl.length() - 1) : baseUrl;
        String normalizedPath = path.startsWith("/") ? path : "/" + path;
        return normalizedBaseUrl + normalizedPath;
    }

    private String normalizeChatCompletionsPath(String path) {
        if (path == null || path.isBlank()) {
            return CHAT_COMPLETIONS_PATH;
        }
        String normalizedPath = path.trim();
        return normalizedPath.startsWith("/") ? normalizedPath : "/" + normalizedPath;
    }

    private String rewriteModelForUpstream(String model, NativeOpenAIRoute route) {
        if (model == null || model.isBlank() || !route.stripsModelPrefix()) {
            return model;
        }
        String normalizedModel = model.trim();
        String modelPrefix = route.modelPrefix();
        if (modelPrefix == null || modelPrefix.isBlank()) {
            return normalizedModel;
        }
        return normalizedModel.regionMatches(true, 0, modelPrefix, 0, modelPrefix.length())
                ? normalizedModel.substring(modelPrefix.length())
                : normalizedModel;
    }

    private String rewriteRequestModelForUpstream(
            String requestBody, String model, NativeOpenAIRoute route) throws IOException {
        String outboundModel = rewriteModelForUpstream(model, route);
        if (outboundModel == null || outboundModel.equals(model)) {
            return requestBody;
        }

        JsonNode root = objectMapper.readTree(requestBody);
        if (!(root instanceof ObjectNode objectNode) || !objectNode.has("model")) {
            return requestBody;
        }
        objectNode.put("model", outboundModel);
        return objectMapper.writeValueAsString(objectNode);
    }

    private String routeModelForSelection(String model, NativeOpenAIRoute route) {
        return rewriteModelForUpstream(model, route);
    }

    /**
     * 上游配置信息
     */
    private static class UpstreamInfo {
        final String name;
        final String baseUrl;
        final String authToken;
        final int timeout;
        final String chatCompletionsPath;

        UpstreamInfo(String name, String baseUrl, String authToken, int timeout, String chatCompletionsPath) {
            this.name = name;
            this.baseUrl = baseUrl;
            this.authToken = authToken;
            this.timeout = timeout;
            this.chatCompletionsPath = chatCompletionsPath;
        }
    }

    private record NativeOpenAIRoute(
            String modelPrefix,
            String supportedModel,
            String channelCode,
            ApiProvider provider,
            String endpointPath,
            boolean usesConfiguredChatCompletionsPath,
            boolean stripsModelPrefix,
            /** Per-call timeout (ms). 0 = 沿用 OkHttpClient 全局超时. compact / memories 用 300s */
            long callTimeoutMsOverride) {

        private static NativeOpenAIRoute defaultProvider(ApiProvider provider) {
            return new NativeOpenAIRoute("", null, null, provider, null, true, false, 0L);
        }

        private static NativeOpenAIRoute responsesChannel(String channelCode, String endpointPath, long callTimeoutMs) {
            return new NativeOpenAIRoute("", null, channelCode, null, endpointPath, false, false, callTimeoutMs);
        }

        private static NativeOpenAIRoute chatPrefix(String modelPrefix, String channelCode, String supportedModel) {
            return new NativeOpenAIRoute(modelPrefix, supportedModel, channelCode, null, null, true, true, 0L);
        }

        private static NativeOpenAIRoute supportedModel(String model) {
            return new NativeOpenAIRoute("", model, null, null, null, true, false, 0L);
        }

        private NativeOpenAIRoute withSupportedModel(String model) {
            return new NativeOpenAIRoute(
                    modelPrefix, model, channelCode, provider, endpointPath,
                    usesConfiguredChatCompletionsPath, stripsModelPrefix, callTimeoutMsOverride);
        }

        private boolean usesSupportedModel() {
            return supportedModel != null && !supportedModel.isBlank();
        }

        private boolean usesChannelCode() {
            return channelCode != null && !channelCode.isBlank();
        }

        private boolean hasModelConstraint() {
            return supportedModel != null && !supportedModel.isBlank();
        }

        private String resolveEndpointPath(UpstreamInfo upstream) {
            if (usesConfiguredChatCompletionsPath) {
                return upstream.chatCompletionsPath;
            }
            return endpointPath;
        }

        private String routingLabel(String selectionModel) {
            if (usesSupportedModel()) {
                return "supported_model=" + selectionModel;
            }
            return usesChannelCode() ? "channel=" + channelCode : "provider=" + provider;
        }
    }

    // ==================== 非流式请求 ====================

    /**
     * Handle OpenAI native non-streaming requests.
     * If the primary upstream fails after retries, fail over within the same
     * routing channel or provider route.
     *
     * @param requestBody  OpenAI 原生格式的请求体
     * @param upstreamName 上游名称（可选）
     * @param clientApiKey 客户端 API Key，用于 credit 消耗和使用量统计
     * @param headers      附加请求头
     * @return OpenAI 原生格式的响应
     * @throws IOException 如果所有上游均失败
     */
    public String handleNonStreamingRequest(String requestBody, String upstreamName,
                                            String clientApiKey, Map<String, String> headers) throws IOException {
        String model = extractModelFromRequest(requestBody);
        NativeOpenAIRoute route = resolveRoute(model);
        return handleNonStreamingRequest(requestBody, upstreamName, clientApiKey, headers, model, route);
    }

    public String handleResponsesNonStreamingRequest(String requestBody, String upstreamName,
                                                     String clientApiKey, Map<String, String> headers) throws IOException {
        // 兼容: 客户端把 chat-completions 风格 (有 messages, 没 input) 发到了 /v1/responses; 切到 chat 路径救一下。
        if (looksLikeChatCompletionsBody(requestBody)) {
            log.warn("client sent chat-style 'messages' body to /v1/responses; rerouting to /v1/chat/completions");
            return handleNonStreamingRequest(requestBody, upstreamName, clientApiKey, headers);
        }
        String model = extractModelFromRequest(requestBody);
        return handleNonStreamingRequest(requestBody, upstreamName, clientApiKey, headers, model, RESPONSES_ROUTE);
    }

    /**
     * 处理 Codex /v1/responses/compact 非流式请求。
     * 上游本质上是一次模型调用——会按 input/output_tokens 计费，
     * 因此走与 /v1/responses 完全相同的失败重试 / 故障转移 / 用量记录流程，
     * 实际是否扣 credit 由 {@link #extractTokenUsageFromOpenAIResponse(String)} 解析的 usage 决定：
     * 上游返回 usage 则按 ModelPricing 计费，未返回则 cost_usd=0、不扣余额。
     */
    public String handleResponsesCompactNonStreamingRequest(String requestBody, String upstreamName,
                                                            String clientApiKey, Map<String, String> headers) throws IOException {
        String model = extractModelFromRequest(requestBody);
        return handleNonStreamingRequest(requestBody, upstreamName, clientApiKey, headers, model, RESPONSES_COMPACT_ROUTE);
    }

    /**
     * 处理 Codex /v1/memories/trace_summarize 请求。
     * 上游响应是 {"output": [{"trace_summary":"...","memory_summary":"..."}]} —— Codex 客户端不解析 usage,
     * 但 OpenAI 仍是真实模型调用。沿用 compact 的策略: 上游回 usage 就计费, 没回就不扣余额。
     */
    public String handleMemoriesTraceSummarizeNonStreamingRequest(String requestBody, String upstreamName,
                                                                  String clientApiKey, Map<String, String> headers) throws IOException {
        String model = extractModelFromRequest(requestBody);
        return handleNonStreamingRequest(requestBody, upstreamName, clientApiKey, headers, model, MEMORIES_TRACE_SUMMARIZE_ROUTE);
    }

    private String handleNonStreamingRequest(String requestBody, String upstreamName,
                                             String clientApiKey, Map<String, String> headers, String model,
                                             NativeOpenAIRoute route) throws IOException {
        NativeOpenAIRoute requestRoute = route.withSupportedModel(model);
        UpstreamInfo upstream = getUpstreamInfo(upstreamName, requestRoute);
        String outboundRequestBody = rewriteRequestModelForUpstream(requestBody, model, requestRoute);
        String outboundModel = rewriteModelForUpstream(model, requestRoute);

        // Try the primary upstream first.
        try {
            return executeNonStreamingRequest(
                    outboundRequestBody, requestBody, outboundModel, model, clientApiKey, headers,
                    upstream, requestRoute);
        } catch (IOException primaryException) {
            log.warn("Primary upstream '{}' failed ({}), trying failover...",
                    upstream.name, requestRoute.routingLabel(routeModelForSelection(model, requestRoute)));

            List<String> triedUpstreams = new ArrayList<>();
            triedUpstreams.add(upstream.name);

            List<UpstreamConfig> failoverCandidates = getFailoverUpstreams(requestRoute, triedUpstreams);

            for (UpstreamConfig fallback : failoverCandidates) {
                try {
                    UpstreamInfo fallbackUpstream = toUpstreamInfo(fallback);

                    log.info("[upstream_error_transfer] trying upstream '{}' (priority={}, {})",
                            fallback.getName(), fallback.getPriority(),
                            requestRoute.routingLabel(routeModelForSelection(model, requestRoute)));

                    upstreamConfigService.updateLastUsedAt(fallback.getName());

                    String result = executeNonStreamingRequest(
                            outboundRequestBody, requestBody, outboundModel, model, clientApiKey, headers,
                            fallbackUpstream, requestRoute);

                    log.info("[upstream_error_transfer]✅ 故障转移成功，使用上游 '{}'", fallback.getName());
                    return result;
                } catch (IOException failoverException) {
                    log.warn("⚠️ 故障转移上游 '{}' 也失败了: {}",
                            fallback.getName(), failoverException.getMessage());
                    triedUpstreams.add(fallback.getName());
                }
            }

            log.error("OpenAI-compatible route exhausted all upstreams ({}), tried: {}",
                    requestRoute.routingLabel(routeModelForSelection(model, requestRoute)), triedUpstreams);
            // 所有上游都失败时再补打一次入参 (上面单上游失败已经打了一份, 这里给出完整故障转移视角)。
            log.error("failing requestBody after exhausting upstreams (model={}, outboundModel={}, bytes={}): {}",
                    model, outboundModel,
                    outboundRequestBody == null ? 0 : outboundRequestBody.length(),
                    truncateForLog(outboundRequestBody, 4000));
            throw primaryException;
        }
    }

    /**
     * 向指定 OpenAI 上游执行非流式请求（含重试逻辑）
     */
    private String executeNonStreamingRequest(String outboundRequestBody, String originalRequestBody,
                                              String outboundModel, String originalModel, String clientApiKey, Map<String, String> headers,
                                              UpstreamInfo upstream, NativeOpenAIRoute route) throws IOException {
        String endpointPath = route.resolveEndpointPath(upstream);
        String url = buildEndpointUrl(upstream.baseUrl, endpointPath);
        long startTime = System.currentTimeMillis();
        String requestId = UUID.randomUUID().toString();
        int bodyBytes = outboundRequestBody == null ? 0 : outboundRequestBody.getBytes(java.nio.charset.StandardCharsets.UTF_8).length;
        long effectiveTimeoutMs = route.callTimeoutMsOverride() > 0
                ? route.callTimeoutMsOverride() : openAIConfig.getTimeout();

        log.info("POST OpenAI-compatible native API - requestId={}, model={}, outboundModel={}, {}, upstream={}, url={}, bodyBytes={}, callTimeoutMs={}",
                requestId, originalModel, outboundModel,
                route.routingLabel(routeModelForSelection(originalModel, route)),
                upstream.name, url, bodyBytes, effectiveTimeoutMs);

        // 构建 HTTP 请求
        Request.Builder requestBuilder = new Request.Builder()
                .url(url)
                .post(RequestBody.create(outboundRequestBody, null))
                .addHeader("Authorization", buildAuthorizationHeader(upstream.authToken))
                .addHeader("Content-Type", "application/json");

        // 添加自定义请求头（排除敏感字段）
        if (headers != null) {
            headers.forEach((key, value) -> {
                if (!key.equalsIgnoreCase("Authorization") &&
                        !key.equalsIgnoreCase("x-api-key") &&
                        !key.equalsIgnoreCase("x-upstream")) {
                    requestBuilder.addHeader(key, value);
                }
            });
        }

        Request request = requestBuilder.build();

        // 重试逻辑：最多尝试 2 次，间隔 3000ms
        int maxAttempts = 2;
        int attempt = 0;
        IOException lastException = null;

        while (attempt < maxAttempts) {
            attempt++;
            // per-call timeout: 慢调用 (compact/memories) 走 route 的 override; 其他沿用全局
            okhttp3.Call call = getHttpClient().newCall(request);
            call.timeout().timeout(effectiveTimeoutMs, TimeUnit.MILLISECONDS);
            try (Response response = call.execute()) {
                long duration = System.currentTimeMillis() - startTime;
                String responseBody = response.body() != null ? response.body().string() : "{}";

                if (!response.isSuccessful()) {
                    if (attempt < maxAttempts) {
                        log.warn("⚠️ [{}] OpenAI 原生请求尝试 {}/{} 失败，状态码 {}，3000ms 后重试...",
                                requestId, attempt, maxAttempts, response.code());
                        sleepForRetry();
                        continue;
                    } else {
                        log.error("❌ [{}] 所有 {} 次 OpenAI 原生请求尝试均失败，状态码 {} - {}",
                                requestId, maxAttempts, response.code(), responseBody);
                        // 上游失败时把请求体打到日志, 方便线上复现 (截断, 避免炸日志)。
                        log.error("❌ [{}] failing requestBody (model={}, outboundModel={}, bytes={}): {}",
                                requestId, originalModel, outboundModel, bodyBytes,
                                truncateForLog(outboundRequestBody, 4000));

                        trackApiUsage(clientApiKey, requestId, originalModel, endpointPath, "POST",
                                response.code(), null, duration, null, headers,
                                "OpenAI native API error: " + responseBody);

                        requestLogService.logRequest(requestId, maskApiKey(clientApiKey), "POST",
                                endpointPath, originalModel, headers, originalRequestBody,
                                response.code(), responseBody, duration, TokenUsage.empty(),
                                "OpenAI native API error");

                        throw new IOException(
                                "OpenAI API request failed: " + response.code() + " - " + responseBody);
                    }
                }

                // 从原生响应中提取 token 用量
                TokenUsage tokenUsage = extractTokenUsageFromOpenAIResponse(responseBody);
                logOpenAINativeUsageDiagnostics("non_stream_response", requestId, endpointPath,
                        originalModel, upstream.name, responseBody, tokenUsage);

                log.info("📥 [{}] OpenAI 原生响应: {} - 耗时: {} ms, Tokens: {}/{}/{}",
                        requestId, response.code(), duration,
                        tokenUsage.getInputTokens(), tokenUsage.getOutputTokens(),
                        tokenUsage.getTotalTokens());

                // 记录 API 用量 (非流式: TTFT = null)
                trackApiUsage(clientApiKey, requestId, originalModel, endpointPath, "POST",
                        response.code(), responseBody, duration, null, headers, null);

                // 记录请求日志
                requestLogService.logRequest(requestId, maskApiKey(clientApiKey), "POST",
                        endpointPath, originalModel, headers, originalRequestBody,
                        response.code(), responseBody, duration, tokenUsage, null);

                // 直接返回 OpenAI 原生响应
                return responseBody;

            } catch (IOException e) {
                lastException = e;
                long attemptDuration = System.currentTimeMillis() - startTime;
                String exClass = e.getClass().getName();
                String exDetail = describeIoException(e, effectiveTimeoutMs);
                if (attempt < maxAttempts) {
                    log.warn("⚠️ [{}] OpenAI 原生请求尝试 {}/{} 抛异常 - upstream={}, url={}, bodyBytes={}, callTimeoutMs={}, attemptDurationMs={}, exType={}, detail={}, 3000ms 后重试...",
                            requestId, attempt, maxAttempts, upstream.name, url, bodyBytes,
                            effectiveTimeoutMs, attemptDuration, exClass, exDetail, e);
                    sleepForRetry();
                } else {
                    long duration = System.currentTimeMillis() - startTime;
                    log.error("❌ [{}] 所有 {} 次 OpenAI 原生请求尝试均失败 - upstream={}, url={}, bodyBytes={}, callTimeoutMs={}, totalDurationMs={}, exType={}, detail={}",
                            requestId, maxAttempts, upstream.name, url, bodyBytes,
                            effectiveTimeoutMs, duration, exClass, exDetail, e);
                    // 网络级 IOException (超时 / 拒连) 时也把请求体打到日志, 与 HTTP 非 2xx 路径对齐。
                    log.error("❌ [{}] failing requestBody (model={}, outboundModel={}, bytes={}): {}",
                            requestId, originalModel, outboundModel, bodyBytes,
                            truncateForLog(outboundRequestBody, 4000));

                    trackApiUsage(clientApiKey, requestId, originalModel, endpointPath, "POST",
                            500, null, duration, null, headers, exClass + ": " + exDetail);

                    requestLogService.logRequest(requestId, maskApiKey(clientApiKey), "POST",
                            endpointPath, originalModel, headers, originalRequestBody,
                            500, null, duration, TokenUsage.empty(), exClass + ": " + exDetail);

                    throw e;
                }
            }
        }

        throw lastException != null ? lastException
                : new IOException("Unexpected error: retry loop exited without exception");
    }

    /**
     * 把 IOException 翻译成可操作的诊断短句, 给 log 用。
     * 区分: connect timeout / read timeout / call timeout / DNS / 拒接, 以便用户立刻判断要查上游还是查网络。
     */
    private String describeIoException(IOException e, long callTimeoutMs) {
        String msg = e.getMessage() == null ? "" : e.getMessage();
        String lower = msg.toLowerCase(Locale.ROOT);
        if (e instanceof java.net.SocketTimeoutException) {
            if (lower.contains("connect")) {
                return "TCP connect timeout (>" + callTimeoutMs + "ms) — 上游不可达 / 防火墙阻断";
            }
            return "read/idle timeout (>" + callTimeoutMs + "ms) — 上游接受了连接但未在窗口内响应; 可能是慢调用或 hang 住";
        }
        if (lower.contains("timeout")) {
            // OkHttp callTimeout / 自定义 IOException("timeout")
            return "call timeout (>" + callTimeoutMs + "ms) — 整体调用超过 callTimeout";
        }
        if (e instanceof java.net.UnknownHostException) {
            return "DNS 解析失败: " + msg + " — 检查 upstream baseUrl";
        }
        if (e instanceof java.net.ConnectException) {
            return "TCP 拒接: " + msg + " — 上游端口未监听 / baseUrl 错";
        }
        if (e instanceof javax.net.ssl.SSLException) {
            return "SSL/TLS 错误: " + msg;
        }
        return msg;
    }

    // ==================== 流式请求 ====================

    /**
     * 处理 OpenAI 原生流式请求
     * 直接透传 SSE 事件到客户端，不做格式转换
     *
     * @param requestBody  OpenAI 原生格式的请求体
     * @param upstreamName 上游名称（可选）
     * @param clientApiKey 客户端 API Key
     * @param headers      附加请求头
     * @param emitter      SSE 发射器
     */
    public void handleStreamingRequest(String requestBody, String upstreamName,
                                       String clientApiKey, Map<String, String> headers, SseEmitter emitter) {
        long startTime = System.currentTimeMillis();
        String requestId = UUID.randomUUID().toString();

        String model = extractModelFromRequest(requestBody);
        NativeOpenAIRoute route = resolveRoute(model);
        handleStreamingRequest(requestBody, upstreamName, clientApiKey, headers, emitter,
                startTime, requestId, model, route);
    }

    public void handleResponsesStreamingRequest(String requestBody, String upstreamName,
                                                String clientApiKey, Map<String, String> headers, SseEmitter emitter) {
        // 兼容: 客户端把 chat-completions 风格 (有 messages, 没 input) 发到了 /v1/responses (stream); 切到 chat 路径。
        // 注意: 上游会返回 chat SSE 帧 ({choices:[{delta:...}]}), 而非 responses 的 response.output_text.delta;
        //       客户端要能消费 chat 格式 SSE, 否则会解析失败。
        if (looksLikeChatCompletionsBody(requestBody)) {
            log.warn("client sent chat-style 'messages' body to /v1/responses (stream); rerouting to /v1/chat/completions");
            handleStreamingRequest(requestBody, upstreamName, clientApiKey, headers, emitter);
            return;
        }
        long startTime = System.currentTimeMillis();
        String requestId = UUID.randomUUID().toString();

        String model = extractModelFromRequest(requestBody);
        handleStreamingRequest(requestBody, upstreamName, clientApiKey, headers, emitter,
                startTime, requestId, model, RESPONSES_ROUTE);
    }

    /**
     * 处理 /v1/responses/compact 流式请求。
     * Codex 当前以 unary 方式调用 compact，但留一条 SSE 通道以兼容未来的流式实现。
     */
    public void handleResponsesCompactStreamingRequest(String requestBody, String upstreamName,
                                                       String clientApiKey, Map<String, String> headers, SseEmitter emitter) {
        long startTime = System.currentTimeMillis();
        String requestId = UUID.randomUUID().toString();

        String model = extractModelFromRequest(requestBody);
        handleStreamingRequest(requestBody, upstreamName, clientApiKey, headers, emitter,
                startTime, requestId, model, RESPONSES_COMPACT_ROUTE);
    }

    private void handleStreamingRequest(String requestBody, String upstreamName,
                                        String clientApiKey, Map<String, String> headers, SseEmitter emitter,
                                        long startTime, String requestId, String model, NativeOpenAIRoute route) {
        try {
            NativeOpenAIRoute requestRoute = route.withSupportedModel(model);
            UpstreamInfo upstream = getUpstreamInfo(upstreamName, requestRoute);
            String outboundRequestBody = rewriteRequestModelForUpstream(requestBody, model, requestRoute);
            outboundRequestBody = ensureStreamUsageIncluded(outboundRequestBody);
            String outboundModel = rewriteModelForUpstream(model, requestRoute);
            String endpointPath = requestRoute.resolveEndpointPath(upstream);
            String url = buildEndpointUrl(upstream.baseUrl, endpointPath);

            log.info("POST OpenAI-compatible native stream API - requestId={}, model={}, outboundModel={}, {}, upstream={}, path={}",
                    requestId, model, outboundModel,
                    requestRoute.routingLabel(routeModelForSelection(model, requestRoute)), upstream.name, endpointPath);

            // 构建 HTTP 请求
            Request.Builder requestBuilder = new Request.Builder()
                    .url(url)
                    .post(RequestBody.create(outboundRequestBody, MediaType.parse("application/json")))
                    .addHeader("Authorization", buildAuthorizationHeader(upstream.authToken))
                    .addHeader("Content-Type", "application/json")
                    .addHeader("Accept", "text/event-stream");

            if (headers != null) {
                headers.forEach((key, value) -> {
                    if (!key.equalsIgnoreCase("Authorization") &&
                            !key.equalsIgnoreCase("x-api-key") &&
                            !key.equalsIgnoreCase("x-upstream")) {
                        requestBuilder.addHeader(key, value);
                    }
                });
            }

            Request request = requestBuilder.build();

            // 累积 token 用量
            AtomicReference<TokenUsage> accumulatedUsage = new AtomicReference<>(TokenUsage.empty());
            AtomicBoolean streamCompleted = new AtomicBoolean(false);
            // TTFT: 首事件抵达时刻
            java.util.concurrent.atomic.AtomicLong firstEventAt = new java.util.concurrent.atomic.AtomicLong(0L);

            EventSourceListener listener = new EventSourceListener() {
                @Override
                public void onOpen(EventSource eventSource, Response response) {
                    log.info("✅ OpenAI 原生 SSE 连接建立, 请求 ID: {}, 状态码: {}",
                            requestId, response.code());
                }

                @Override
                public void onEvent(EventSource eventSource, String id, String type, String data) {
                    if (streamCompleted.get()) {
                        return;
                    }
                    firstEventAt.compareAndSet(0L, System.currentTimeMillis());

                    try {
                        // 检查 [DONE] 标记
                        if ("[DONE]".equals(data)) {
                            log.info("收到 [DONE] 标记");
                            if (!streamCompleted.getAndSet(true)) {
                                // 发送 [DONE] 到客户端
                                emitter.send(SseEmitter.event().data("[DONE]"));
                                completeStream();
                            }
                            return;
                        }

                        // 尝试从 chunk 中提取 token 用量
                        try {
                            TokenUsage usage = extractTokenUsageFromOpenAIResponse(data);
                            if (usage.getInputTokens() > 0 || usage.getOutputTokens() > 0
                                    || usage.getCacheReadInputTokens() > 0 || usage.getCacheCreationInputTokens() > 0) {
                                accumulatedUsage.set(usage);
                                logOpenAINativeUsageDiagnostics("stream_chunk", requestId, endpointPath,
                                        model, upstream.name, data, usage);
                                log.info("📊 [{}] OpenAI 原生流式 usage chunk - event={}, Tokens: {}/{}/{}",
                                        requestId, type,
                                        usage.getInputTokens(),
                                        usage.getOutputTokens(),
                                        usage.getTotalTokens());
                            }
                        } catch (Exception ignored) {
                            // 解析失败时忽略，不影响透传
                        }

                        SseEmitter.SseEventBuilder eventBuilder = SseEmitter.event().data(data);
                        if (id != null && !id.isBlank()) {
                            eventBuilder.id(id);
                        }
                        if (type != null && !type.isBlank()) {
                            eventBuilder.name(type);
                        }
                        emitter.send(eventBuilder);

                    } catch (IOException e) {
                        log.error("发送 SSE 事件到客户端失败: {}", e.getMessage(), e);
                        streamCompleted.set(true);
                        emitter.completeWithError(e);
                        eventSource.cancel();
                    }
                }

                @Override
                public void onClosed(EventSource eventSource) {
                    log.info("🔒 OpenAI 原生 SSE 连接关闭, 请求 ID: {}", requestId);
                    if (!streamCompleted.getAndSet(true)) {
                        completeStream();
                    }
                }

                @Override
                public void onFailure(EventSource eventSource, Throwable t, Response response) {
                    if (streamCompleted.getAndSet(true)) {
                        return;
                    }

                    String errorMsg = t != null ? t.getMessage() : "SSE 连接意外关闭";
                    log.error("❌ OpenAI 原生 SSE 连接失败, 请求 ID: {}, 错误: {}",
                            requestId, errorMsg);

                    long duration = System.currentTimeMillis() - startTime;
                    int statusCode = response != null ? response.code() : 500;
                    String errorBody = null;

                    if (response != null && response.body() != null) {
                        try {
                            errorBody = response.body().string();
                            log.error("错误响应: {} - {}", response.code(), errorBody);

                            // 发送错误事件到客户端
                            String errorEvent = String.format(
                                    "{\"error\":{\"message\":\"%s\",\"type\":\"server_error\",\"code\":%d}}",
                                    errorBody.replace("\"", "\\\""), statusCode);
                            emitter.send(SseEmitter.event().data(errorEvent));
                        } catch (IOException e) {
                            log.error("无法读取或发送错误响应: {}", e.getMessage());
                        }
                    }

                    long feaErr = firstEventAt.get();
                    Long ttftErr = feaErr > 0L ? feaErr - startTime : null;
                    trackApiUsage(clientApiKey, requestId, model, endpointPath, "POST",
                            statusCode, null, duration, ttftErr, headers, errorMsg);

                    requestLogService.logRequest(requestId, maskApiKey(clientApiKey), "POST",
                            endpointPath, model, headers, requestBody,
                            statusCode, errorBody, duration, TokenUsage.empty(), errorMsg);

                    Throwable error = t != null ? t
                            : new IOException("OpenAI SSE failed with status " + statusCode);
                    emitter.completeWithError(error);
                    eventSource.cancel();
                }

                private void completeStream() {
                    long duration = System.currentTimeMillis() - startTime;
                    TokenUsage finalUsage = accumulatedUsage.get();

                    log.info("🏁 [{}] OpenAI 原生流式完成 - 耗时: {} ms, Tokens: {}/{}/{}",
                            requestId, duration,
                            finalUsage.getInputTokens(),
                            finalUsage.getOutputTokens(),
                            finalUsage.getTotalTokens());
                    log.info("api_usage.openai_native_stream.final requestId={} endpoint={} model={} upstream={} tokens input={} output={} total={} cacheCreation={} cacheRead={}",
                            requestId, endpointPath, model, upstream.name,
                            finalUsage.getInputTokens(),
                            finalUsage.getOutputTokens(),
                            finalUsage.getTotalTokens(),
                            finalUsage.getCacheCreationInputTokens(),
                            finalUsage.getCacheReadInputTokens());
                    if (finalUsage.getInputTokens() == 0 && finalUsage.getOutputTokens() == 0) {
                        log.warn("api_usage.zero_tokens_on_stream requestId={} endpoint={} model={} upstream={} - no usage event observed from upstream",
                                requestId, endpointPath, model, upstream.name);
                    }

                    // 记录 API 用量
                    long feaOk = firstEventAt.get();
                    Long ttftOk = feaOk > 0L ? feaOk - startTime : null;
                    trackApiUsageWithTokenUsage(clientApiKey, requestId, model, endpointPath, "POST",
                            200, finalUsage, duration, ttftOk, headers, null);

                    // 记录请求日志
                    requestLogService.logRequest(requestId, maskApiKey(clientApiKey), "POST",
                            endpointPath, model, headers, requestBody,
                            200, "{\"type\":\"stream\",\"status\":\"completed\"}",
                            duration, finalUsage, null);

                    emitter.complete();
                }
            };

            // 创建 EventSource
            EventSource eventSource = EventSources.createFactory(getHttpClient())
                    .newEventSource(request, listener);

            // 处理 emitter 生命周期
            emitter.onTimeout(() -> {
                log.warn("⏱️ SSE 发射器超时, 请求 ID: {}", requestId);
                long duration = System.currentTimeMillis() - startTime;

                trackApiUsage(clientApiKey, requestId, model, endpointPath, "POST",
                        408, null, duration, null, headers, "Request timeout");

                eventSource.cancel();
                emitter.complete();
            });

            emitter.onCompletion(() -> {
                log.info("✅ SSE 发射器完成, 请求 ID: {}", requestId);
                eventSource.cancel();
            });

            emitter.onError((e) -> {
                log.error("❌ SSE 发射器错误, 请求 ID: {}, 错误: {}", requestId, e.getMessage());
                eventSource.cancel();
            });

        } catch (Exception e) {
            log.error("初始化 OpenAI 原生流式请求失败: {}", e.getMessage(), e);
            emitter.completeWithError(e);
        }
    }

    // ==================== 辅助方法 ====================

    /**
     * 从请求体中提取模型名称
     *
     * @param requestBody 请求体 JSON
     * @return 模型名称，如果提取失败则返回 "unknown"
     */
    private String extractModelFromRequest(String requestBody) {
        try {
            JsonNode root = objectMapper.readTree(requestBody);
            if (root.has("model")) {
                return root.get("model").asText();
            }
        } catch (Exception e) {
            log.debug("从请求体中提取模型名称失败: {}", e.getMessage());
        }
        return "unknown";
    }

    /**
     * Extract token usage from OpenAI native responses.
     *
     * @param responseBody OpenAI native JSON response
     * @return token usage
     */
    private TokenUsage extractTokenUsageFromOpenAIResponse(String responseBody) {
        try {
            JsonNode root = objectMapper.readTree(responseBody);
            JsonNode usageNode = findUsageNode(root);
            if (usageNode != null) {
                int totalInputTokens = firstInt(usageNode, "prompt_tokens", "input_tokens");
                int cachedTokens = cachedTokens(usageNode);
                int inputTokens = Math.max(0, totalInputTokens - cachedTokens);
                int outputTokens = firstInt(usageNode, "completion_tokens", "output_tokens");
                int totalTokens = usageNode.has("total_tokens")
                        ? usageNode.get("total_tokens").asInt()
                        : totalInputTokens + outputTokens;
                return TokenUsage.builder()
                        .inputTokens(inputTokens)
                        .outputTokens(outputTokens)
                        .totalTokens(totalTokens)
                        .cacheCreationInputTokens(0)
                        .cacheReadInputTokens(cachedTokens)
                        .build();
            }
        } catch (Exception e) {
            log.debug("从 OpenAI 响应中提取 token 用量失败: {}", e.getMessage());
        }
        return TokenUsage.empty();
    }

    private JsonNode findUsageNode(JsonNode root) {
        JsonNode usageNode = root.get("usage");
        if (usageNode != null) {
            return usageNode;
        }

        JsonNode responseNode = root.get("response");
        if (responseNode != null) {
            return responseNode.get("usage");
        }

        return null;
    }

    private int firstInt(JsonNode node, String... fieldNames) {
        for (String fieldName : fieldNames) {
            JsonNode value = node.get(fieldName);
            if (value != null && value.isNumber()) {
                return value.asInt();
            }
        }
        return 0;
    }

    private int cachedTokens(JsonNode usageNode) {
        int cachedTokens = nestedInt(usageNode, "prompt_tokens_details", "cached_tokens");
        if (cachedTokens > 0) {
            return cachedTokens;
        }
        cachedTokens = nestedInt(usageNode, "input_tokens_details", "cached_tokens");
        if (cachedTokens > 0) {
            return cachedTokens;
        }
        return firstInt(usageNode, "cached_tokens");
    }

    private int nestedInt(JsonNode node, String objectFieldName, String fieldName) {
        JsonNode objectNode = node.get(objectFieldName);
        if (objectNode == null || !objectNode.isObject()) {
            return 0;
        }
        JsonNode value = objectNode.get(fieldName);
        return value != null && value.isNumber() ? value.asInt() : 0;
    }

    private void logOpenAINativeUsageDiagnostics(String phase, String requestId, String endpoint, String model,
                                                  String upstream, String responseBody, TokenUsage tokenUsage) {
        String usageJson = extractUsageJson(responseBody);
        log.info("api_usage.openai_native.raw phase={} requestId={} endpoint={} model={} upstream={} usage={}",
                phase, requestId, endpoint, model, upstream, usageJson);
        if (tokenUsage == null) {
            return;
        }
        boolean empty = tokenUsage.getInputTokens() == 0
                && tokenUsage.getOutputTokens() == 0
                && tokenUsage.getCacheCreationInputTokens() == 0
                && tokenUsage.getCacheReadInputTokens() == 0;
        log.info("api_usage.openai_native.parsed phase={} requestId={} endpoint={} model={} upstream={} input={} output={} total={} cacheCreation={} cacheRead={} empty={}",
                phase, requestId, endpoint, model, upstream,
                tokenUsage.getInputTokens(),
                tokenUsage.getOutputTokens(),
                tokenUsage.getTotalTokens(),
                tokenUsage.getCacheCreationInputTokens(),
                tokenUsage.getCacheReadInputTokens(),
                empty);
    }

    private String extractUsageJson(String responseBody) {
        if (responseBody == null || responseBody.isBlank()) {
            return "<empty-body>";
        }
        try {
            JsonNode root = objectMapper.readTree(responseBody);
            JsonNode usage = findUsageNode(root);
            if (usage == null || usage.isNull()) {
                return "<missing-usage>";
            }
            String usageString = usage.toString();
            return usageString.length() > 1000
                    ? usageString.substring(0, 1000) + "...[+" + (usageString.length() - 1000) + "]"
                    : usageString;
        } catch (Exception e) {
            return "<usage-parse-error:" + e.getMessage() + ">";
        }
    }

    /**
     * 构建 Authorization 头部
     * 如果 token 没有 "Bearer " 前缀则自动添加
     */
    private String buildAuthorizationHeader(String authToken) {
        if (authToken == null || authToken.isEmpty()) {
            throw new IllegalArgumentException("OpenAI auth token is missing");
        }
        if (authToken.toLowerCase().startsWith("bearer ")) {
            return authToken;
        } else {
            return "Bearer " + authToken;
        }
    }

    /**
     * 重试等待
     */
    private void sleepForRetry() throws IOException {
        try {
            Thread.sleep(3000);
        } catch (InterruptedException ie) {
            Thread.currentThread().interrupt();
            throw new IOException("Retry interrupted", ie);
        }
    }

    /**
     * 记录 API 用量
     */
    private void trackApiUsage(String apiKey, String requestId, String model, String endpoint,
                               String method, int statusCode, String responseBody,
                               long durationMs, Long firstTokenLatencyMs,
                               Map<String, String> headers, String errorMessage) {
        try {
            TokenUsage tokenUsage = TokenUsage.empty();
            if (responseBody != null) {
                tokenUsage = extractTokenUsageFromOpenAIResponse(responseBody);
            }
            trackApiUsageWithTokenUsage(apiKey, requestId, model, endpoint, method, statusCode,
                    tokenUsage, durationMs, firstTokenLatencyMs, headers, errorMessage);
        } catch (Exception e) {
            log.error("记录 API 用量失败: {}", e.getMessage(), e);
        }
    }

    private void trackApiUsageWithTokenUsage(String apiKey, String requestId, String model, String endpoint,
                                             String method, int statusCode, TokenUsage tokenUsage,
                                             long durationMs, Long firstTokenLatencyMs,
                                             Map<String, String> headers, String errorMessage) {
        try {
            if (tokenUsage == null) {
                tokenUsage = TokenUsage.empty();
            }
            log.info("api_usage.openai_native.track requestId={} endpoint={} model={} status={} tokens input={} output={} total={} cacheCreation={} cacheRead={} ttftMs={} error={}",
                    requestId, endpoint, model, statusCode,
                    tokenUsage.getInputTokens(),
                    tokenUsage.getOutputTokens(),
                    tokenUsage.getTotalTokens(),
                    tokenUsage.getCacheCreationInputTokens(),
                    tokenUsage.getCacheReadInputTokens(),
                    firstTokenLatencyMs,
                    errorMessage);
            ApiUsageRecord record = ApiUsageRecord.builder()
                    .apiKey(apiKey)
                    .requestId(requestId)
                    .model(model)
                    .endpoint(endpoint)
                    .method(method)
                    .statusCode(statusCode)
                    .tokenUsage(tokenUsage)
                    .durationMs(durationMs)
                    .firstTokenLatencyMs(firstTokenLatencyMs)
                    .errorMessage(errorMessage)
                    .requestMetadata(usageTrackingService.createRequestMetadata(
                            headers != null ? headers.get("user-agent") : null, headers))
                    .build();

            usageTrackingService.trackUsage(record);
        } catch (Exception e) {
            log.error("记录 API 用量失败: {}", e.getMessage(), e);
        }
    }

    /**
     * 掩码 API Key 用于日志输出
     */
    private String maskApiKey(String apiKey) {
        if (apiKey == null || apiKey.length() <= 8) {
            return "***";
        }
        return apiKey.substring(0, 4) + "..." + apiKey.substring(apiKey.length() - 4);
    }

    /**
     * 判断请求体是不是 chat-completions 风格 (有 messages, 没 input)。
     * /v1/chat/completions 必有 messages, 一定没有 input;
     * /v1/responses 可以 input 是结构化对象数组, 但不会有 messages 字段。
     * 满足 (messages && !input) 时, 视为客户端把 chat 格式发错到了 responses 端点。
     */
    private boolean looksLikeChatCompletionsBody(String body) {
        if (body == null || body.isBlank()) return false;
        try {
            JsonNode root = objectMapper.readTree(body);
            return root.has("messages") && !root.has("input");
        } catch (Exception e) {
            log.debug("looksLikeChatCompletionsBody parse failed: {}", e.getMessage());
            return false;
        }
    }

    /**
     * OpenAI-compatible streaming only includes final usage when requested.
     * Add the option server-side so first-party UI and compatible clients are billable.
     */
    private String ensureStreamUsageIncluded(String body) {
        if (body == null || body.isBlank()) return body;
        try {
            JsonNode root = objectMapper.readTree(body);
            if (!(root instanceof ObjectNode objectNode)) {
                return body;
            }
            JsonNode streamNode = objectNode.get("stream");
            if (streamNode == null || !streamNode.asBoolean(false)) {
                return body;
            }
            JsonNode optionsNode = objectNode.get("stream_options");
            ObjectNode options = optionsNode instanceof ObjectNode existing
                    ? existing
                    : objectMapper.createObjectNode();
            options.put("include_usage", true);
            objectNode.set("stream_options", options);
            return objectMapper.writeValueAsString(objectNode);
        } catch (Exception e) {
            log.debug("ensureStreamUsageIncluded parse failed: {}", e.getMessage());
            return body;
        }
    }

    /**
     * 把请求体截断成日志友好的长度: 短的原样, 长的留头 + 尾 + 中间标注省略字符数。
     */
    private static String truncateForLog(String body, int maxChars) {
        if (body == null) return "<null>";
        if (body.length() <= maxChars) return body;
        int head = Math.max(0, maxChars - 500);
        int tail = Math.min(500, maxChars / 4);
        int omitted = body.length() - head - tail;
        return body.substring(0, head)
                + "...[truncated " + omitted + " chars]..."
                + body.substring(body.length() - tail);
    }

}
