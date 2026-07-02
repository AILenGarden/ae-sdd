package ai.nubase.ai.gateway.dto.openai;

import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;

/**
 * OpenAI Delta object
 * Represents incremental content in streaming responses
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class OpenAIDelta {
    /**
     * Role of the message (usually only appears in first chunk)
     */
    private String role;

    /**
     * Incremental content text
     */
    private String content;

    /**
     * 增量工具调用（用于流式工具调用响应）
     */
    @JsonProperty("tool_calls")
    private List<OpenAIToolCallDelta> toolCalls;
}
