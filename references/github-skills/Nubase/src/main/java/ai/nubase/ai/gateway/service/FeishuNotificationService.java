package ai.nubase.ai.gateway.service;

import ai.nubase.ai.gateway.config.FeishuConfig;
import ai.nubase.ai.gateway.entity.UpstreamConfig;
import com.fasterxml.jackson.databind.ObjectMapper;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import okhttp3.*;
import org.springframework.scheduling.annotation.Async;
import org.springframework.stereotype.Service;

import java.time.LocalDateTime;
import java.time.format.DateTimeFormatter;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;

/**
 * 用于处理上游健康状态变化的飞书 webhook 通知服务。
 * 当上游服务不可用或恢复时发送交互式卡片消息。
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class FeishuNotificationService {

    private static final MediaType JSON = MediaType.get("application/json; charset=utf-8");
    private static final DateTimeFormatter FORMATTER = DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm:ss");

    private static final OkHttpClient HTTP_CLIENT = new OkHttpClient.Builder()
            .connectTimeout(10, TimeUnit.SECONDS)
            .readTimeout(10, TimeUnit.SECONDS)
            .writeTimeout(10, TimeUnit.SECONDS)
            .build();

    private final FeishuConfig feishuConfig;
    private final ObjectMapper objectMapper;

    /**
     * 当上游变为不可用时发送告警。
     *
     * @param config       已下线的上游配置
     * @param errorMessage 健康检查错误信息
     */
    @Async
    public void notifyUpstreamDown(UpstreamConfig config, String errorMessage) {
        if (!feishuConfig.isEnabled()) {
            return;
        }

        String title = "\uD83D\uDD34 上游服务不可用告警";
        String color = "red";

        List<Map<String, Object>> fields = List.of(
                buildField("Provider", config.getProvider().name()),
                buildField("上游名称", config.getName()),
                buildField("Base URL", config.getBaseUrl()),
                buildField("API Key", config.getAuthToken()),
                buildField("错误信息", errorMessage),
                buildField("检测时间", LocalDateTime.now().format(FORMATTER))
        );

        sendCardMessage(title, color, fields);
    }

    /**
     * 当上游恢复健康状态时发送通知。
     *
     * @param config 已恢复的上游配置
     */
    @Async
    public void notifyUpstreamRecovered(UpstreamConfig config) {
        if (!feishuConfig.isEnabled()) {
            return;
        }

        String title = "\uD83D\uDFE2 上游服务已恢复";
        String color = "green";

        List<Map<String, Object>> fields = List.of(
                buildField("Provider", config.getProvider().name()),
                buildField("上游名称", config.getName()),
                buildField("Base URL", config.getBaseUrl()),
                buildField("API Key", config.getAuthToken()),
                buildField("恢复时间", LocalDateTime.now().format(FORMATTER))
        );

        sendCardMessage(title, color, fields);
    }

    /**
     * 当通用服务不可达时发送告警。
     *
     * @param serviceName 服务名称
     * @param baseUrl     服务基础地址
     * @param errorMessage 错误描述
     */
    @Async
    public void notifyServiceDown(String serviceName, String baseUrl, String errorMessage) {
        if (!feishuConfig.isEnabled()) {
            return;
        }

        String title = "\uD83D\uDD34 " + serviceName + " 服务不可用告警";
        String color = "red";

        List<Map<String, Object>> fields = List.of(
                buildField("服务名称", serviceName),
                buildField("Base URL", baseUrl),
                buildField("错误信息", errorMessage),
                buildField("检测时间", LocalDateTime.now().format(FORMATTER))
        );

        sendCardMessage(title, color, fields);
    }

    /**
     * 当通用服务恢复时发送通知。
     *
     * @param serviceName 服务名称
     * @param baseUrl     服务基础地址
     */
    @Async
    public void notifyServiceRecovered(String serviceName, String baseUrl) {
        if (!feishuConfig.isEnabled()) {
            return;
        }

        String title = "\uD83D\uDFE2 " + serviceName + " 服务已恢复";
        String color = "green";

        List<Map<String, Object>> fields = List.of(
                buildField("服务名称", serviceName),
                buildField("Base URL", baseUrl),
                buildField("恢复时间", LocalDateTime.now().format(FORMATTER))
        );

        sendCardMessage(title, color, fields);
    }

    @Async
    public void notifyMem0AsyncWriteFailed(String requestId,
                                           Long failureRecordId,
                                           String userId,
                                           Integer upstreamStatusCode,
                                           String errorMessage,
                                           String requestPayload) {
        if (!feishuConfig.isEnabled()) {
            return;
        }

        String title = "\uD83D\uDD34 mem0 async write failed";
        String color = "red";

        List<Map<String, Object>> fields = List.of(
                buildField("Request ID", safeValue(requestId)),
                buildField("Failure Record ID", failureRecordId == null ? "N/A" : String.valueOf(failureRecordId)),
                buildField("User ID", safeValue(userId)),
                buildField("Upstream Status", upstreamStatusCode == null ? "N/A" : String.valueOf(upstreamStatusCode)),
                buildField("Error", safeValue(errorMessage)),
                buildField("Request Preview", truncate(safeValue(requestPayload), 500)),
                buildField("Detected At", LocalDateTime.now().format(FORMATTER))
        );

        sendCardMessage(title, color, fields);
    }

    /**
     * 构建卡片正文中的单个字段元素。
     */
    private Map<String, Object> buildField(String label, String value) {
        Map<String, Object> field = new LinkedHashMap<>();
        field.put("is_short", false);
        field.put("text", Map.of(
                "tag", "lark_md",
                "content", "**" + label + "**: " + value
        ));
        return field;
    }

    /**
     * 向配置好的飞书 webhook 发送交互式卡片消息。
     */
    private void sendCardMessage(String title, String color, List<Map<String, Object>> fields) {
        try {
            Map<String, Object> header = Map.of(
                    "title", Map.of("tag", "plain_text", "content", title),
                    "template", color
            );

            Map<String, Object> card = Map.of(
                    "header", header,
                    "elements", List.of(
                            Map.of(
                                    "tag", "div",
                                    "fields", fields
                            )
                    )
            );

            Map<String, Object> payload = Map.of(
                    "msg_type", "interactive",
                    "card", card
            );

            String json = objectMapper.writeValueAsString(payload);

            Request request = new Request.Builder()
                    .url(feishuConfig.getWebhookUrl())
                    .post(RequestBody.create(json, JSON))
                    .build();

            try (Response response = HTTP_CLIENT.newCall(request).execute()) {
                if (response.isSuccessful()) {
                    log.info("\uD83D\uDCE8 飞书通知发送成功: {}", title);
                } else {
                    log.warn("\uD83D\uDCE8 飞书通知发送失败: HTTP {}, body={}",
                            response.code(),
                            response.body() != null ? response.body().string() : "null");
                }
            }
        } catch (Exception e) {
            log.error("\uD83D\uDCE8 飞书通知发送异常: {}", e.getMessage(), e);
        }
    }

    private String safeValue(String value) {
        return value == null || value.isBlank() ? "N/A" : value;
    }

    private String truncate(String value, int maxLength) {
        if (value == null || value.length() <= maxLength) {
            return value;
        }
        return value.substring(0, maxLength) + "...";
    }
}
