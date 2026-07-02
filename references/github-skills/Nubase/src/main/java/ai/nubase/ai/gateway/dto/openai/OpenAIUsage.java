package ai.nubase.ai.gateway.dto.openai;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * OpenAI Token Usage Metadata
 * Represents token consumption for a completion
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class OpenAIUsage {
    /**
     * Number of tokens in the prompt
     */
    @JsonProperty("prompt_tokens")
    private Integer promptTokens;

    /**
     * Number of tokens in the completion
     */
    @JsonProperty("completion_tokens")
    private Integer completionTokens;

    /**
     * Total tokens used (prompt + completion)
     */
    @JsonProperty("total_tokens")
    private Integer totalTokens;

    @JsonProperty("prompt_tokens_details")
    private TokenDetails promptTokensDetails;

    @JsonProperty("input_tokens_details")
    private TokenDetails inputTokensDetails;

    @Data
    @Builder
    @NoArgsConstructor
    @AllArgsConstructor
    public static class TokenDetails {
        @JsonProperty("cached_tokens")
        private Integer cachedTokens;
    }
}
