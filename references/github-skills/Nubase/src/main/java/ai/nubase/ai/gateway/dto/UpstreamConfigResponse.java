package ai.nubase.ai.gateway.dto;

import ai.nubase.ai.gateway.entity.UpstreamConfig;
import ai.nubase.common.enums.ApiProvider;
import lombok.Builder;
import lombok.Data;

import java.time.LocalDateTime;
import java.util.List;

@Data
@Builder
public class UpstreamConfigResponse {
    private Long id;
    private String name;
    private String baseUrl;
    private boolean authTokenSet;
    private ApiProvider provider;
    private String channelCode;
    private List<String> supportedModels;
    private String chatCompletionsPath;
    private String description;
    private Boolean isDefault;
    private Boolean isActive;
    private Integer timeoutMs;
    private Integer maxRetries;
    private Integer priority;
    private Integer maxInputTokens;
    private LocalDateTime createdAt;
    private LocalDateTime updatedAt;
    private LocalDateTime lastUsedAt;
    private LocalDateTime lastHealthCheck;
    private String healthStatus;
    private String healthMessage;

    public static UpstreamConfigResponse from(UpstreamConfig config) {
        return UpstreamConfigResponse.builder()
                .id(config.getId())
                .name(config.getName())
                .baseUrl(config.getBaseUrl())
                .authTokenSet(config.getAuthToken() != null && !config.getAuthToken().isBlank())
                .provider(config.getProvider())
                .channelCode(config.getChannelCode())
                .supportedModels(config.getSupportedModels())
                .chatCompletionsPath(config.getChatCompletionsPath())
                .description(config.getDescription())
                .isDefault(config.getIsDefault())
                .isActive(config.getIsActive())
                .timeoutMs(config.getTimeoutMs())
                .maxRetries(config.getMaxRetries())
                .priority(config.getPriority())
                .maxInputTokens(config.getMaxInputTokens())
                .createdAt(config.getCreatedAt())
                .updatedAt(config.getUpdatedAt())
                .lastUsedAt(config.getLastUsedAt())
                .lastHealthCheck(config.getLastHealthCheck())
                .healthStatus(config.getHealthStatus())
                .healthMessage(config.getHealthMessage())
                .build();
    }
}
