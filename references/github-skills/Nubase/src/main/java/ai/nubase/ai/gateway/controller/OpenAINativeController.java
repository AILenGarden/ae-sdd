package ai.nubase.ai.gateway.controller;

import ai.nubase.ai.gateway.entity.ModelPricing;
import ai.nubase.ai.gateway.repository.ModelPricingRepository;
import ai.nubase.ai.gateway.service.OpenAINativeApiService;
import ai.nubase.ai.gateway.util.GatewayKeyUtil;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import jakarta.servlet.http.HttpServletRequest;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.servlet.mvc.method.annotation.SseEmitter;

import java.io.IOException;
import java.time.ZoneOffset;
import java.util.*;

/**
 * OpenAI 原生 API 网关控制器
 * 直接接收和返回 OpenAI 原生格式，不经过 Claude 格式转换
 * 基础路径: /
 */
@Slf4j
@RestController
@RequestMapping("")
@RequiredArgsConstructor
public class OpenAINativeController {

    private final OpenAINativeApiService openAINativeApiService;
    private final ObjectMapper objectMapper;
    private final ModelPricingRepository modelPricingRepository;

    @PostMapping(value = "/v1/chat/completions")
    public Object chatCompletions(
            @RequestBody String requestBody,
            HttpServletRequest request) {
        return handleOpenAINativeRequest(requestBody, request, OpenAINativeEndpoint.CHAT_COMPLETIONS);
    }

    @PostMapping(value = "/v1/responses")
    public Object responses(
            @RequestBody String requestBody,
            HttpServletRequest request) {
        return handleOpenAINativeRequest(requestBody, request, OpenAINativeEndpoint.RESPONSES);
    }

    /**
     * Codex /v1/responses/compact —— 把对话历史压缩成一段加密 context。
     * 上游本质是一次模型调用，token 计费完全沿用 /v1/responses 的逻辑：
     * 由 OpenAINativeApiService 根据上游响应里的 usage 字段判断是否扣费。
     * QuotaFilter 通过 path.startsWith("/v1/responses") 自动覆盖额度预检。
     */
    @PostMapping(value = "/v1/responses/compact")
    public Object responsesCompact(
            @RequestBody String requestBody,
            HttpServletRequest request) {
        return handleOpenAINativeRequest(requestBody, request, OpenAINativeEndpoint.RESPONSES_COMPACT);
    }

    /**
     * Codex /v1/memories/trace_summarize —— 把多个原始记忆 trace 摘要成长期 memory。
     * 上游响应是 {"output":[{"trace_summary","memory_summary"}]}，是一次真实模型调用。
     * 计费策略与 compact 一致: 上游响应包含 usage 时按 ModelPricing 扣费, 没有则不扣。
     * 注意: 该路径不在 /v1/responses 前缀下, 已在 QuotaFilter 增加 /v1/memories 前缀。
     */
    @PostMapping(value = "/v1/memories/trace_summarize")
    public Object memoriesTraceSummarize(
            @RequestBody String requestBody,
            HttpServletRequest request) {
        return handleOpenAINativeRequest(requestBody, request, OpenAINativeEndpoint.MEMORIES_TRACE_SUMMARIZE);
    }

    /**
     * OpenAI 兼容的 /v1/models 列表。
     * 直接基于本地 model_pricing 表合成: 网关支持哪些模型就回哪些, 不消耗 token。
     * Codex CLI / openai 官方 SDK / 各类客户端都用这个 endpoint 拉模型清单。
     * <p>
     * headers="!anthropic-version" 与 ClaudeGatewayController.listModels() 区分:
     * Anthropic SDK / Claude Code 一定带 anthropic-version 头 (官方要求), 走那边返回 Anthropic 格式;
     * 不带的 (Codex / OpenAI SDK / 普通 curl) 走这里返回 OpenAI list 格式。
     */
    @GetMapping(value = "/v1/models", headers = "!anthropic-version")
    public ResponseEntity<Map<String, Object>> listModels() {
        List<Map<String, Object>> data = new ArrayList<>();
        for (ModelPricing m : modelPricingRepository.findByIsActiveTrueOrderBySortOrderAscProviderAscModelAsc()) {
            data.add(toOpenAIModelDto(m));
        }
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("object", "list");
        body.put("data", data);
        return ResponseEntity.ok().contentType(MediaType.APPLICATION_JSON).body(body);
    }

    /**
     * OpenAI 兼容的 /v1/models/{model} 单条查询。
     * Codex 启动时偶尔会用此接口校验模型存在性。
     * <p>
     * headers="!anthropic-version" 与 ClaudeGatewayController.getModel() 区分。
     */
    @GetMapping(value = "/v1/models/{model}", headers = "!anthropic-version")
    public ResponseEntity<?> retrieveModel(@PathVariable("model") String model) {
        ModelPricing pricing = modelPricingRepository
                .findByIsActiveTrueOrderBySortOrderAscProviderAscModelAsc()
                .stream()
                .filter(p -> model.equalsIgnoreCase(p.getModel()))
                .findFirst()
                .orElse(null);
        if (pricing == null) {
            Map<String, Object> err = Map.of(
                    "error", Map.of(
                            "message", "The model `" + model + "` does not exist",
                            "type", "invalid_request_error",
                            "param", "model",
                            "code", "model_not_found"));
            return ResponseEntity.status(HttpStatus.NOT_FOUND)
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(err);
        }
        return ResponseEntity.ok().contentType(MediaType.APPLICATION_JSON).body(toOpenAIModelDto(pricing));
    }

    private Map<String, Object> toOpenAIModelDto(ModelPricing m) {
        long createdEpochSec = (m.getCreatedAt() != null ? m.getCreatedAt() : m.getEffectiveFrom().atStartOfDay())
                .toEpochSecond(ZoneOffset.UTC);
        Map<String, Object> dto = new LinkedHashMap<>();
        dto.put("id", m.getModel());
        dto.put("object", "model");
        dto.put("created", createdEpochSec);
        dto.put("owned_by", m.getProvider() == null ? "openai" : m.getProvider().toLowerCase(Locale.ROOT));
        return dto;
    }

    private Object handleOpenAINativeRequest(
            String requestBody,
            HttpServletRequest request,
            OpenAINativeEndpoint endpoint) {
        try {
            String clientApiKey = extractClientApiKey(request);
            Map<String, String> headers = extractHeaders(request);
            String upstreamName = request.getHeader("x-upstream");

            JsonNode jsonNode = objectMapper.readTree(requestBody);
            boolean isStreaming = jsonNode.has("stream") && jsonNode.get("stream").asBoolean();
            String model = jsonNode.has("model") ? jsonNode.get("model").asText() : "unknown";

            if (isStreaming) {
                log.info("Received OpenAI native streaming request: {} (stream=true), model={}",
                        endpoint.requestPath, model);

                SseEmitter emitter = new SseEmitter(600_000L);
                try {
                    switch (endpoint) {
                        case RESPONSES -> openAINativeApiService.handleResponsesStreamingRequest(
                                requestBody, upstreamName, clientApiKey, headers, emitter);
                        case RESPONSES_COMPACT -> openAINativeApiService.handleResponsesCompactStreamingRequest(
                                requestBody, upstreamName, clientApiKey, headers, emitter);
                        case MEMORIES_TRACE_SUMMARIZE -> {
                            log.warn("memories.trace_summarize streaming requested but upstream is unary; falling back to non-streaming pipe is not supported in stream mode");
                            emitter.completeWithError(new UnsupportedOperationException(
                                    "/v1/memories/trace_summarize does not support streaming"));
                        }
                        default -> openAINativeApiService.handleStreamingRequest(
                                requestBody, upstreamName, clientApiKey, headers, emitter);
                    }
                } catch (Exception e) {
                    log.error("Failed to initialize OpenAI native streaming request: {}", e.getMessage());
                    emitter.completeWithError(e);
                }
                return emitter;
            } else {
                log.info("Received OpenAI native non-streaming request: {} (stream=false), model={}",
                        endpoint.requestPath, model);

                String response = switch (endpoint) {
                    case RESPONSES -> openAINativeApiService.handleResponsesNonStreamingRequest(
                            requestBody, upstreamName, clientApiKey, headers);
                    case RESPONSES_COMPACT -> openAINativeApiService.handleResponsesCompactNonStreamingRequest(
                            requestBody, upstreamName, clientApiKey, headers);
                    case MEMORIES_TRACE_SUMMARIZE -> openAINativeApiService.handleMemoriesTraceSummarizeNonStreamingRequest(
                            requestBody, upstreamName, clientApiKey, headers);
                    default -> openAINativeApiService.handleNonStreamingRequest(
                            requestBody, upstreamName, clientApiKey, headers);
                };

                log.info("========================================");
                log.info("OpenAI native API response:");
                log.info("Endpoint: {}", endpoint.requestPath);
                log.info("Model: {}", model);
                log.info("Response body: {}", response);
                log.info("========================================");

                return ResponseEntity.ok()
                        .contentType(MediaType.APPLICATION_JSON)
                        .body(response);
            }

        } catch (IOException e) {
            log.error("OpenAI native request forwarding failed: {}", e.getMessage());
            return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .contentType(MediaType.APPLICATION_JSON)
                    .body("{\"error\": {\"message\": \"" +
                            e.getMessage().replace("\"", "\\\"") +
                            "\", \"type\": \"server_error\", \"code\": \"internal_error\"}}");
        }
    }

    private enum OpenAINativeEndpoint {
        CHAT_COMPLETIONS("/openai/v1/chat/completions"),
        RESPONSES("/openai/v1/responses"),
        RESPONSES_COMPACT("/openai/v1/responses/compact"),
        MEMORIES_TRACE_SUMMARIZE("/openai/v1/memories/trace_summarize");

        private final String requestPath;

        OpenAINativeEndpoint(String requestPath) {
            this.requestPath = requestPath;
        }
    }

    /**
     * 从请求头中提取客户端 API Key
     * 优先级：x-api-key > Authorization: Bearer
     */
    private String extractClientApiKey(HttpServletRequest request) {
        // 尝试 x-api-key 头
        String apiKey = request.getHeader("x-api-key");
        if (apiKey != null && GatewayKeyUtil.isGatewayKey(apiKey.trim())) {
            return apiKey.trim();
        }

        // 尝试 Authorization 头
        String authHeader = request.getHeader("Authorization");
        if (authHeader != null && authHeader.startsWith("Bearer ")) {
            String bearer = authHeader.substring(7).trim();
            if (GatewayKeyUtil.isGatewayKey(bearer)) {
                return bearer;
            }
        }

        return null;
    }

    /**
     * 从传入请求中提取相关头部信息
     */
    private Map<String, String> extractHeaders(HttpServletRequest request) {
        Map<String, String> headers = new HashMap<>();

        Enumeration<String> headerNames = request.getHeaderNames();
        while (headerNames.hasMoreElements()) {
            String headerName = headerNames.nextElement();

            // 转发自定义头和 user-agent
            if (headerName.startsWith("x-") ||
                    headerName.equalsIgnoreCase("user-agent")) {
                headers.put(headerName, request.getHeader(headerName));
            }
        }

        return headers;
    }
}
