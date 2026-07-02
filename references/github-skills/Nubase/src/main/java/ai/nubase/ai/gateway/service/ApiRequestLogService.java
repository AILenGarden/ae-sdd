package ai.nubase.ai.gateway.service;

import ai.nubase.ai.gateway.dto.TokenUsage;
import ai.nubase.common.config.AnthropicConfig;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ObjectNode;
import jakarta.annotation.PostConstruct;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.core.env.Environment;
import org.springframework.core.env.Profiles;
import org.springframework.scheduling.annotation.Async;
import org.springframework.stereotype.Service;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.time.LocalDateTime;
import java.time.format.DateTimeFormatter;
import java.util.Map;

/**
 * API 请求日志持久化服务
 * 将每次 AI 接口的请求入参和出参保存到指定目录中
 * DEV环境: 项目根目录/log/ai-gateway-logs
 * RELEASE环境: /root/nubase/logs/ai-gateway-logs
 * <p>
 * 每次调用创建一个独立的文件夹，使用年月日时分秒毫秒作为文件夹名称
 * 文件夹结构：
 * .../ai-gateway-logs/yyyyMMdd_HHmmss_SSS/
 * ├── request.json # 请求信息（包含请求头、请求体等）
 * ├── response.json # 响应信息（包含状态码、响应体等）
 * └── metadata.json # 元数据（请求ID、耗时、Token使用量等）
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class ApiRequestLogService {

    private final ObjectMapper objectMapper;
    private final AnthropicConfig anthropicConfig;
    private final Environment environment;

    private String logBaseDir;

    private static final DateTimeFormatter FOLDER_NAME_FORMATTER = DateTimeFormatter.ofPattern("yyyyMMdd_HHmmss_SSS");
    private static final DateTimeFormatter DATETIME_FORMATTER = DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm:ss.SSS");

    @PostConstruct
    public void init() {
        if (environment.acceptsProfiles(Profiles.of("dev"))) {
            // dev 环境
            logBaseDir = System.getProperty("user.dir") + File.separator + "logs" + File.separator + "ai-gateway-logs";
        } else {
            // release 环境 (默认)
            logBaseDir = "/root/nubase/logs/ai-gateway-logs";
        }
        log.info("ApiRequestLogService 使用日志目录: {}", logBaseDir);
    }

    /**
     * 异步记录请求日志到文件
     * 为每次调用创建独立的文件夹
     *
     * @param requestId      请求ID
     * @param clientApiKey   客户端 API Key（掩码后的）
     * @param method         HTTP 方法
     * @param path           请求路径
     * @param model          模型名称
     * @param requestHeaders 请求头
     * @param requestBody    请求体
     * @param statusCode     响应状态码
     * @param responseBody   响应体
     * @param durationMs     请求耗时
     * @param tokenUsage     Token 使用量
     * @param errorMessage   错误信息
     */
    @Async
    public void logRequest(String requestId, String clientApiKey, String method, String path,
            String model, Map<String, String> requestHeaders, String requestBody,
            int statusCode, String responseBody, long durationMs,
            TokenUsage tokenUsage, String errorMessage) {
        // 检查是否启用日志记录
        if (!anthropicConfig.getLogging().isEnabled()) {
            log.debug("AI 请求日志记录已禁用，跳过记录: requestId={}", requestId);
            return;
        }

        log.info("开始记录请求日志: requestId={}, method={}, path={}", requestId, method, path);
        try {
            // 创建本次调用的文件夹
            String folderName = LocalDateTime.now().format(FOLDER_NAME_FORMATTER);
            Path logFolder = Paths.get(logBaseDir, folderName);
            Files.createDirectories(logFolder);

            log.debug("创建日志文件夹: {}", logFolder);

            // 构建并写入请求信息
            ObjectNode requestInfo = createRequestInfo(requestId, clientApiKey, method, path,
                    model, requestHeaders, requestBody);
            writeJsonToFile(logFolder.resolve("request.json"), requestInfo);

            // 构建并写入响应信息
            ObjectNode responseInfo = createResponseInfo(statusCode, responseBody, durationMs, errorMessage);
            writeJsonToFile(logFolder.resolve("response.json"), responseInfo);

            // 构建并写入元数据
            ObjectNode metadata = createMetadata(requestId, clientApiKey, model, durationMs, tokenUsage);
            writeJsonToFile(logFolder.resolve("metadata.json"), metadata);

            log.debug("请求日志已写入文件夹: requestId={}, folder={}", requestId, logFolder);
        } catch (Exception e) {
            log.error("写入请求日志失败: requestId={}, error={}", requestId, e.getMessage(), e);
        }
    }

    /**
     * 创建请求信息 JSON 对象
     */
    private ObjectNode createRequestInfo(String requestId, String clientApiKey, String method,
            String path, String model, Map<String, String> requestHeaders,
            String requestBody) throws IOException {
        ObjectNode requestInfo = objectMapper.createObjectNode();

        requestInfo.put("timestamp", LocalDateTime.now().format(DATETIME_FORMATTER));
        requestInfo.put("requestId", requestId);
        requestInfo.put("clientApiKey", clientApiKey);
        requestInfo.put("method", method);
        requestInfo.put("path", path);
        if (model != null) {
            requestInfo.put("model", model);
        }

        // 请求头
        if (requestHeaders != null && !requestHeaders.isEmpty()) {
            requestInfo.set("headers", objectMapper.valueToTree(requestHeaders));
        }

        // 请求体
        if (requestBody != null && !requestBody.isEmpty()) {
            try {
                requestInfo.set("body", objectMapper.readTree(requestBody));
            } catch (IOException e) {
                requestInfo.put("body", requestBody);
            }
        }

        return requestInfo;
    }

    /**
     * 创建响应信息 JSON 对象
     */
    private ObjectNode createResponseInfo(int statusCode, String responseBody,
            long durationMs, String errorMessage) throws IOException {
        ObjectNode responseInfo = objectMapper.createObjectNode();

        responseInfo.put("timestamp", LocalDateTime.now().format(DATETIME_FORMATTER));
        responseInfo.put("statusCode", statusCode);
        responseInfo.put("durationMs", durationMs);

        // 响应体
        if (responseBody != null && !responseBody.isEmpty()) {
            try {
                responseInfo.set("body", objectMapper.readTree(responseBody));
            } catch (IOException e) {
                responseInfo.put("body", responseBody);
            }
        }

        // 错误信息
        if (errorMessage != null) {
            responseInfo.put("error", errorMessage);
        }

        return responseInfo;
    }

    /**
     * 创建元数据 JSON 对象
     */
    private ObjectNode createMetadata(String requestId, String clientApiKey, String model,
            long durationMs, TokenUsage tokenUsage) {
        ObjectNode metadata = objectMapper.createObjectNode();

        metadata.put("requestId", requestId);
        metadata.put("clientApiKey", clientApiKey);
        if (model != null) {
            metadata.put("model", model);
        }
        metadata.put("durationMs", durationMs);

        // Token 使用量
        if (tokenUsage != null) {
            ObjectNode usage = objectMapper.createObjectNode();
            usage.put("inputTokens", tokenUsage.getInputTokens());
            usage.put("outputTokens", tokenUsage.getOutputTokens());
            usage.put("totalTokens", tokenUsage.getTotalTokens());
            if (tokenUsage.getCacheCreationInputTokens() > 0) {
                usage.put("cacheCreationInputTokens", tokenUsage.getCacheCreationInputTokens());
            }
            if (tokenUsage.getCacheReadInputTokens() > 0) {
                usage.put("cacheReadInputTokens", tokenUsage.getCacheReadInputTokens());
            }
            metadata.set("tokenUsage", usage);
        }

        return metadata;
    }

    /**
     * 将 JSON 对象写入文件（格式化输出，便于阅读）
     */
    private void writeJsonToFile(Path filePath, ObjectNode jsonNode) throws IOException {
        String jsonString = objectMapper.writerWithDefaultPrettyPrinter()
                .writeValueAsString(jsonNode);
        Files.write(filePath, jsonString.getBytes(StandardCharsets.UTF_8));
    }
}
