package ai.nubase.ai.gateway.dto;

import lombok.Builder;
import lombok.Data;

/**
 * Token使用量统计DTO
 */
@Data
@Builder
public class TokenUsage {
    /**
     * 输入token数
     */
    private Integer inputTokens;

    /**
     * 输出token数
     */
    private Integer outputTokens;

    /**
     * 总token数
     */
    private Integer totalTokens;

    /**
     * 缓存创建输入token数
     */
    private Integer cacheCreationInputTokens;

    /**
     * 缓存读取输入token数
     */
    private Integer cacheReadInputTokens;

    public static TokenUsage empty() {
        return TokenUsage.builder()
                .inputTokens(0)
                .outputTokens(0)
                .totalTokens(0)
                .cacheCreationInputTokens(0)
                .cacheReadInputTokens(0)
                .build();
    }
}
