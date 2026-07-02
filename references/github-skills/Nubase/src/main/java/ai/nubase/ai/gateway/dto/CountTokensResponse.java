package ai.nubase.ai.gateway.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Count Tokens API 响应 DTO
 * 对应 Anthropic API /v1/messages/count_tokens 接口的响应
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class CountTokensResponse {

    /**
     * 输入 token 数量
     */
    @JsonProperty("input_tokens")
    private Integer inputTokens;

    /**
     * 缓存创建输入 token 数量（用于 Prompt Caching）
     */
    @JsonProperty("cache_creation_input_tokens")
    private Integer cacheCreationInputTokens;

    /**
     * 缓存读取输入 token 数量（用于 Prompt Caching）
     */
    @JsonProperty("cache_read_input_tokens")
    private Integer cacheReadInputTokens;

    /**
     * 从 TokenUsage 转换为 CountTokensResponse
     * 注意：count_tokens 只返回输入相关的 token，不包含输出 token
     */
    public static CountTokensResponse fromTokenUsage(TokenUsage tokenUsage) {
        return CountTokensResponse.builder()
                .inputTokens(tokenUsage.getInputTokens())
                .cacheCreationInputTokens(tokenUsage.getCacheCreationInputTokens())
                .cacheReadInputTokens(tokenUsage.getCacheReadInputTokens())
                .build();
    }
}
