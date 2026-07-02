package ai.nubase.ai.gateway.controller;

import ai.nubase.ai.gateway.service.ClaudeGatewayService;
import ai.nubase.ai.gateway.service.OpenAIApiService;
import ai.nubase.ai.gateway.service.TokenCounterService;
import ai.nubase.ai.gateway.util.GatewayKeyUtil;
import ai.nubase.common.enums.ApiProvider;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.multipart.MultipartFile;
import org.springframework.web.servlet.mvc.method.annotation.SseEmitter;

import java.io.IOException;
import java.util.Enumeration;
import java.util.HashMap;
import java.util.Map;

/**
 * Claude API 网关控制器
 * 代理请求到 Claude API
 * 基础路径: /v1
 */
@Slf4j
@RestController
@RequestMapping("")
@RequiredArgsConstructor
public class ClaudeGatewayController {

    private final ClaudeGatewayService gatewayService;
    private final OpenAIApiService openAIApiService;
    private final ObjectMapper objectMapper;
    private final TokenCounterService tokenCounterService;

    /**
     * 健康检查端点
     * GET /ai/v1/health
     */
    @GetMapping("/v1/health")
    public ResponseEntity<Map<String, Object>> health() {
        Map<String, Object> response = new HashMap<>();
        response.put("status", "ok");
        response.put("service", "claude-gateway");
        response.put("timestamp", System.currentTimeMillis());
        return ResponseEntity.ok(response);
    }

    /**
     * 创建消息（统一端点，支持流式和非流式）
     * POST /ai/v1/messages
     * <p>
     * 根据请求体中的 stream 参数自动判断：
     * - stream=true: 返回 SSE 流式响应
     * - stream=false 或未设置: 返回完整 JSON 响应
     * <p>
     * 根据请求 JSON 中的 model 字段判断 API provider:
     * - model 格式: provider/model-name (例如: anthropic/claude-opus-4-5)
     * - 无 "/" 时默认使用 Claude
     */
    @PostMapping(value = "/v1/messages")
    public Object createMessage(
            @RequestBody String requestBody,
            HttpServletRequest request) {
        try {
            // Extract client API key
            String clientApiKey = extractClientApiKey(request);

            // Extract headers
            Map<String, String> headers = extractHeaders(request);

            // Parse request body to extract model field
            JsonNode jsonNode = objectMapper.readTree(requestBody);
            String modelField = jsonNode.has("model") ? jsonNode.get("model").asText() : null;

            // Determine provider based on model field
            ApiProvider provider = determineProviderFromModel(modelField);
            // upstreamName 为 null 时，各 service 内部会走 getDefaultByProvider 缓存
            String upstreamName = null;

            // Normalize model field: remove provider prefix before forwarding
            String normalizedRequestBody = normalizeModelField(requestBody, modelField);

            // Sanitize thinking blocks to avoid invalid signature errors when switching
            // providers
            normalizedRequestBody = sanitizeThinkingBlocks(normalizedRequestBody);

            // Check if request is streaming
            boolean isStreaming = jsonNode.has("stream") && jsonNode.get("stream").asBoolean();

            if (isStreaming) {
                // Streaming response
                log.info("Received streaming request: /ai/v1/messages (stream=true), model: {}, provider: {}",
                        modelField, provider);

                // Create SSE emitter, 10 minute timeout
                SseEmitter emitter = new SseEmitter(600_000L);

                try {
                    // Route based on provider
                    if (ApiProvider.OPENAI.equals(provider)) {
                        openAIApiService.handleStreamingRequest(normalizedRequestBody, upstreamName,
                                clientApiKey, headers, emitter, provider);
                    } else {
                        gatewayService.forwardStreamingRequest("/v1/messages", normalizedRequestBody,
                                headers, clientApiKey, upstreamName, emitter, provider);
                    }
                } catch (Exception e) {
                    log.error("Error initializing streaming request: {}", e.getMessage());
                    emitter.completeWithError(e);
                }

                return emitter;
            } else {
                // Non-streaming response
                log.info("Received non-streaming request: /ai/v1/messages (stream=false), model: {}, provider: {}",
                        modelField, provider);

                String response;
                if (ApiProvider.OPENAI.equals(provider)) {
                    response = openAIApiService.handleNonStreamingRequest(normalizedRequestBody, upstreamName,
                            clientApiKey, headers, provider);
                } else {
                    response = gatewayService.forwardRequest("/v1/messages", normalizedRequestBody,
                            headers, clientApiKey, upstreamName, provider);
                }

                log.info("========================================");
                log.info("[api_response]:");
                log.info("Model: {}", modelField != null ? modelField : "default");
                log.info("Provider: {}", provider);
                log.info("Response body: {}",
                        response);
                log.info("========================================");

                return ResponseEntity.ok()
                        .contentType(MediaType.APPLICATION_JSON)
                        .body(response);
            }

        } catch (IOException e) {
            log.error("Error forwarding request: {}", e.getMessage());
            return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .contentType(MediaType.APPLICATION_JSON)
                    .body("{\"error\": \"Request forwarding failed: " + e.getMessage() + "\"}");
        }
    }

    /**
     * 计算 token 数量（不创建消息）
     * POST /ai/v1/messages/count_tokens
     * <p>
     * 该接口用于在实际调用 API 前预先计算请求将消耗的 token 数量
     * 支持所有消息格式，包括文本、图片、文档、工具调用等
     * <p>
     * 注意：由于使用本地估算，结果可能与实际 API 消耗有 ±10% 的误差
     */
    @PostMapping(value = "/v1/messages/count_tokens", produces = MediaType.APPLICATION_JSON_VALUE)
    public ResponseEntity<String> countTokens(
            @RequestBody String requestBody,
            HttpServletRequest request) {
        try {

            // 使用本地 token 计算服务
            int inputTokens = tokenCounterService.countTokens(requestBody);

            // 构建响应 - 只返回 input_tokens
            Map<String, Object> response = new HashMap<>();
            response.put("input_tokens", inputTokens);

            String responseJson = objectMapper.writeValueAsString(response);

            return ResponseEntity.ok()
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(responseJson);

        } catch (Exception e) {
            log.error("计算 token 数量时出错: {}", e.getMessage(), e);
            return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .body("{\"error\": \"计算 token 失败: " + e.getMessage() + "\"}");
        }
    }

    /**
     * 获取模型信息 (Anthropic 格式)。
     * GET /v1/models/{model_id}
     * <p>
     * 用 headers="anthropic-version" 与 OpenAINativeController.retrieveModel() 区分:
     * Anthropic SDK / Claude Code 必带此 header (官方文档要求), Codex / OpenAI SDK 不带。
     */
    @GetMapping(value = "/v1/models/{modelId}", headers = "anthropic-version")
    public ResponseEntity<String> getModel(
            @PathVariable String modelId,
            HttpServletRequest request) {
        try {
            String clientApiKey = extractClientApiKey(request);
            Map<String, String> headers = extractHeaders(request);
            String response = gatewayService.forwardGetRequest("/v1/models/" + modelId, headers, clientApiKey);

            return ResponseEntity.ok()
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(response);

        } catch (IOException e) {
            log.error("Error fetching model info: {}", e.getMessage());
            return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .body("{\"error\": \"Failed to fetch model info: " + e.getMessage() + "\"}");
        }
    }

    /**
     * List available models (Anthropic 格式, 透传上游)。
     * GET /v1/models
     * <p>
     * 用 headers="anthropic-version" 与 OpenAINativeController.listModels() 区分。
     * 没带 anthropic-version 的请求会路由到 OpenAI 那个 mapping (返回 OpenAI list 格式)。
     */
    @GetMapping(value = "/v1/models", headers = "anthropic-version")
    public ResponseEntity<String> listModels(HttpServletRequest request) {
        try {
            String clientApiKey = extractClientApiKey(request);
            Map<String, String> headers = extractHeaders(request);
            String response = gatewayService.forwardGetRequest("/v1/models", headers, clientApiKey);

            return ResponseEntity.ok()
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(response);

        } catch (IOException e) {
            log.error("Error listing models: {}", e.getMessage());
            return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .body("{\"error\": \"Failed to list models: " + e.getMessage() + "\"}");
        }
    }

    /**
     * Claude Files API: 上传文件 (multipart/form-data)。
     * POST /v1/files
     * <p>
     * Claude Code 用户附 PDF / 图片 / MCP 上传都走这里。必须用 multipart, 不能塞 JSON。
     * 客户端需要带 anthropic-beta: files-api-2025-04-14 header (我们透传)。
     * 文件不消耗 token, QuotaFilter 不拦。
     */
    @PostMapping(value = "/v1/files", consumes = MediaType.MULTIPART_FORM_DATA_VALUE)
    public ResponseEntity<String> uploadFile(
            @RequestPart("file") MultipartFile file,
            HttpServletRequest request) {
        try {
            if (file == null || file.isEmpty()) {
                return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                        .contentType(MediaType.APPLICATION_JSON)
                        .body("{\"error\":{\"type\":\"invalid_request_error\",\"message\":\"missing 'file' part\"}}");
            }

            String clientApiKey = extractClientApiKey(request);
            Map<String, String> headers = extractHeaders(request);
            String response = gatewayService.forwardFileUpload("/v1/files", file, headers, clientApiKey);

            return ResponseEntity.ok().contentType(MediaType.APPLICATION_JSON).body(response);
        } catch (IOException e) {
            log.error("File upload forwarding failed: {}", e.getMessage());
            return ResponseEntity.status(HttpStatus.BAD_GATEWAY)
                    .contentType(MediaType.APPLICATION_JSON)
                    .body("{\"error\":{\"type\":\"upstream_error\",\"message\":\""
                            + e.getMessage().replace("\"", "\\\"") + "\"}}");
        }
    }

    /**
     * Claude Files API: 下载文件二进制内容。
     * GET /v1/files/{fileId}/content
     * <p>
     * 上游返回的是 raw bytes (PDF/图片/任意 MIME), 不能当 JSON 处理。
     * 这里直接 stream 上游响应到 HttpServletResponse, 不读入内存。
     * 兜底 proxyRequest 会把 binary 当 String 解码导致破坏, 必须独立映射。
     */
    @GetMapping("/v1/files/{fileId}/content")
    public void downloadFile(
            @PathVariable("fileId") String fileId,
            HttpServletRequest request,
            HttpServletResponse response) throws IOException {
        String clientApiKey = extractClientApiKey(request);
        Map<String, String> headers = extractHeaders(request);
        gatewayService.forwardFileDownload("/v1/files/" + fileId + "/content", headers, clientApiKey, response);
    }

    /**
     * Event logging batch endpoint
     * POST /ai/v1/event_logging/batch
     * Used by Claude clients for telemetry and usage tracking
     */
    @PostMapping(value = "/v1/event_logging/batch", produces = MediaType.APPLICATION_JSON_VALUE)
    public ResponseEntity<String> eventLoggingBatch(
            @RequestBody String requestBody,
            HttpServletRequest request) {

        log.info("Received event logging batch request");

        try {
            // Validate request body is not empty
            if (requestBody == null || requestBody.strip().isEmpty()) {
                log.warn("Empty event logging request body");
                return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                        .body("{\"error\": \"Request body cannot be empty\"}");
            }

            // Validate JSON format
            try {
                objectMapper.readTree(requestBody);
            } catch (IOException e) {
                log.error("Invalid JSON in event logging request: {}", e.getMessage());
                return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                        .body("{\"error\": \"Invalid JSON format: " + e.getMessage() + "\"}");
            }

            // Extract headers
            Map<String, String> headers = extractHeaders(request);

            // Forward to Claude API
            String response = gatewayService.forwardEventLoggingRequest(requestBody, headers);

            log.info("Event logging batch request completed successfully");

            return ResponseEntity.ok()
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(response);

        } catch (IOException e) {
            log.error("Error forwarding event logging request to Claude API: {}", e.getMessage(), e);
            return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .body("{\"error\": \"Failed to forward event logging request: " + e.getMessage() + "\"}");
        } catch (Exception e) {
            log.error("Unexpected error processing event logging request: {}", e.getMessage(), e);
            return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .body("{\"error\": \"Internal server error: " + e.getMessage() + "\"}");
        }
    }

    /**
     * Generic proxy for other Claude API endpoints
     * Forwards any request under /ai/** or /v1/** to upstream, **preserving the original HTTP method**.
     * 之前硬编码 POST 导致 GET /v1/files、DELETE /v1/files/{id} 等被错转成 POST。
     * IMPORTANT: don't use a bare "/**" — that swallows the static frontend.
     */
    @RequestMapping(value = {"/ai/**", "/v1/**"}, method = {RequestMethod.GET, RequestMethod.POST, RequestMethod.PUT,
            RequestMethod.DELETE, RequestMethod.PATCH})
    public ResponseEntity<String> proxyRequest(
            @RequestBody(required = false) String requestBody,
            HttpServletRequest request) {
        try {
            // Get the path after /ai/v1
            String path = request.getRequestURI().replaceFirst("/ai/v1", "/v1");
            String httpMethod = request.getMethod();

            String clientApiKey = extractClientApiKey(request);
            Map<String, String> headers = extractHeaders(request);

            boolean methodHasBody = "POST".equalsIgnoreCase(httpMethod)
                    || "PUT".equalsIgnoreCase(httpMethod)
                    || "PATCH".equalsIgnoreCase(httpMethod);

            // GET / DELETE / HEAD: 没有请求体, 不需要解析 model / 路由 provider, 直接透传
            if (!methodHasBody) {
                log.info("Proxying {} request to: {} (no-body)", httpMethod, path);
                String response = gatewayService.forwardGenericRequest(httpMethod, path, null,
                        headers, clientApiKey, ApiProvider.CLAUDE);
                return ResponseEntity.ok().contentType(MediaType.APPLICATION_JSON).body(response);
            }

            // POST / PUT / PATCH: 有请求体, 走旧的 model-routing 逻辑
            if (requestBody == null || requestBody.isEmpty()) {
                requestBody = "{}";
            }

            String modelField = null;
            try {
                JsonNode jsonNode = objectMapper.readTree(requestBody);
                modelField = jsonNode.has("model") ? jsonNode.get("model").asText() : null;
            } catch (Exception e) {
                log.debug("Unable to parse model from request body: {}", e.getMessage());
            }
            ApiProvider provider = determineProviderFromModel(modelField);

            String normalizedRequestBody = normalizeModelField(requestBody, modelField);
            normalizedRequestBody = sanitizeThinkingBlocks(normalizedRequestBody);

            log.info("Proxying {} request to: {}, model: {}, provider: {}",
                    httpMethod, path, modelField, provider);

            String response;
            if ("POST".equalsIgnoreCase(httpMethod)) {
                // POST 复用现有的 forwardRequest, 含上游 failover + token tracking
                response = gatewayService.forwardRequest(path, normalizedRequestBody, headers, clientApiKey,
                        null, provider);
            } else {
                // PUT / PATCH: 现有 forwardRequest 不支持, 走 generic
                response = gatewayService.forwardGenericRequest(httpMethod, path, normalizedRequestBody,
                        headers, clientApiKey, provider);
            }

            return ResponseEntity.ok()
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(response);

        } catch (IOException e) {
            log.error("Error proxying request: {}", e.getMessage());
            return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .body("{\"error\": \"Failed to proxy request: " + e.getMessage() + "\"}");
        }
    }

    /**
     * Extract client's API key from request header
     */
    private String extractClientApiKey(HttpServletRequest request) {
        // Try x-api-key header first
        String apiKey = request.getHeader("x-api-key");
        if (apiKey != null && GatewayKeyUtil.isGatewayKey(apiKey.trim())) {
            return apiKey.trim();
        }

        // Try Authorization header
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
     * Extract relevant headers from the incoming request
     */
    private Map<String, String> extractHeaders(HttpServletRequest request) {
        Map<String, String> headers = new HashMap<>();

        Enumeration<String> headerNames = request.getHeaderNames();
        while (headerNames.hasMoreElements()) {
            String headerName = headerNames.nextElement();

            // Forward specific headers
            if (headerName.equalsIgnoreCase("anthropic-version") ||
                    headerName.equalsIgnoreCase("anthropic-beta") ||
                    headerName.startsWith("x-") ||
                    headerName.equalsIgnoreCase("user-agent")) {

                headers.put(headerName, request.getHeader(headerName));
            }
        }

        return headers;
    }

    /**
     * Determine provider based on model field
     *
     * @param modelField model field from request JSON (format: provider/model-name)
     * @return ApiProvider (CLAUDE or OPENAI)
     */
    private ApiProvider determineProviderFromModel(String modelField) {
        if (modelField == null || modelField.isEmpty()) {
            log.debug("No model field provided, defaulting to CLAUDE");
            return ApiProvider.CLAUDE;
        }

        // Check if model contains provider prefix
        if (modelField.contains("/")) {
            String providerPrefix = modelField.substring(0, modelField.indexOf("/")).toLowerCase();

            switch (providerPrefix) {
                case "anthropic":
                    log.info("Detected provider: CLAUDE from model field: {}", modelField);
                    return ApiProvider.CLAUDE;
                case "openai":
                    log.info("Detected provider: OPENAI from model field: {}", modelField);
                    return ApiProvider.OPENAI;
                default:
                    log.warn("Unknown provider prefix: {}, defaulting to CLAUDE", providerPrefix);
                    return ApiProvider.CLAUDE;
            }
        } else {
            // No "/" separator, default to Claude
            log.info("No provider prefix in model field: {}, defaulting to CLAUDE", modelField);
            return ApiProvider.CLAUDE;
        }
    }



    /**
     * 创建消息（流式）- 保留兼容性端点
     * POST /ai/v1/messages/stream
     *
     * @deprecated 建议使用 /v1/messages 端点并设置 stream=true
     */
    @Deprecated
    @PostMapping(value = "/v1/messages/stream", produces = MediaType.TEXT_EVENT_STREAM_VALUE)
    public SseEmitter createMessageStream(
            @RequestBody String requestBody,
            HttpServletRequest request) {

        // 创建 SSE 发射器，超时时间 10 分钟
        SseEmitter emitter = new SseEmitter(600_000L);

        try {
            // Parse model field from request
            String modelField = null;
            try {
                JsonNode jsonNode = objectMapper.readTree(requestBody);
                modelField = jsonNode.has("model") ? jsonNode.get("model").asText() : null;
            } catch (Exception e) {
                log.debug("Unable to parse model from request body: {}", e.getMessage());
            }

            ApiProvider provider = determineProviderFromModel(modelField);
            String upstreamName = null;

            log.info("收到流式请求: /ai/v1/messages/stream, model: {}, provider: {}", modelField, provider);

            // 提取客户端 API Key
            String clientApiKey = extractClientApiKey(request);

            // 提取请求头
            Map<String, String> headers = extractHeaders(request);

            // Normalize model field before forwarding
            String normalizedRequestBody = normalizeModelField(requestBody, modelField);
            normalizedRequestBody = sanitizeThinkingBlocks(normalizedRequestBody);

            // 以异步方式转发到 Claude API
            gatewayService.forwardStreamingRequest("/v1/messages", normalizedRequestBody, headers, clientApiKey,
                    upstreamName, emitter, provider);

        } catch (Exception e) {
            log.error("初始化流式请求时出错: {}", e.getMessage());
            emitter.completeWithError(e);
        }

        return emitter;
    }

    /**
     * 移除 model 字段中的提供商前缀
     * 例如："openai/gpt-5.4-mini" -> "gpt-5.4-mini"
     *
     * @param modelField 原始 model 字段
     * @return 不包含提供商前缀的模型名称
     */
    private String stripProviderPrefix(String modelField) {
        if (modelField == null || !modelField.contains("/")) {
            return modelField;
        }
        return modelField.substring(modelField.indexOf("/") + 1);
    }

    /**
     * 通过移除提供商前缀来规范化请求体中的 model 字段
     *
     * @param requestBody 原始请求体
     * @param modelField  原始 model 字段值
     * @return 移除提供商前缀后的规范化请求体
     */
    private String normalizeModelField(String requestBody, String modelField) {
        if (modelField == null || !modelField.contains("/")) {
            return requestBody;
        }

        try {
            String strippedModel = stripProviderPrefix(modelField);
            JsonNode jsonNode = objectMapper.readTree(requestBody);

            // Modify the model field
            if (jsonNode instanceof ObjectNode) {
                ((ObjectNode) jsonNode).put("model", strippedModel);
                return objectMapper.writeValueAsString(jsonNode);
            }

            return requestBody;
        } catch (Exception e) {
            log.warn("Failed to normalize model field, using original request body: {}", e.getMessage());
            return requestBody;
        }
    }

    /**
     * 从助手消息中移除思考块，以防止在切换提供商时出现
     * "Invalid signature in thinking block"（思考块签名无效）错误。
     * <p>
     * 思考块签名是特定于提供商的，会被 Claude 的签名验证拒绝。
     */
    private String sanitizeThinkingBlocks(String requestBody) {
        try {
            JsonNode root = objectMapper.readTree(requestBody);
            JsonNode messagesNode = root.get("messages");
            if (messagesNode == null || !messagesNode.isArray()) {
                return requestBody;
            }

            boolean modified = false;
            for (JsonNode message : messagesNode) {
                if (!"assistant".equals(message.path("role").asText())) {
                    continue;
                }
                JsonNode contentNode = message.get("content");
                if (contentNode == null || !contentNode.isArray()) {
                    continue;
                }

                ArrayNode contentArray = (ArrayNode) contentNode;
                ArrayNode filtered = objectMapper.createArrayNode();
                boolean hasThinking = false;
                for (JsonNode block : contentArray) {
                    if ("thinking".equals(block.path("type").asText())) {
                        hasThinking = true;
                    } else {
                        filtered.add(block);
                    }
                }
                if (hasThinking) {
                    ((ObjectNode) message).set("content", filtered);
                    modified = true;
                }
            }

            return modified ? objectMapper.writeValueAsString(root) : requestBody;
        } catch (Exception e) {
            log.warn("Failed to sanitize thinking blocks: {}", e.getMessage());
            return requestBody;
        }
    }
}
