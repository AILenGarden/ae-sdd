package ai.nubase.ai.gateway.dto.openai;

import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * OpenAI Choice object
 * Represents a single completion choice in the response
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class OpenAIChoice {
    /**
     * Index of this choice in the list of choices
     */
    private Integer index;

    /**
     * The completed message (for non-streaming responses)
     */
    private OpenAIMessage message;

    /**
     * Delta content (for streaming responses)
     */
    private OpenAIDelta delta;

    /**
     * Reason why the completion finished
     * Possible values: "stop", "length", "content_filter", "function_call",
     * "tool_calls"
     */
    @JsonProperty("finish_reason")
    private String finishReason;
}
