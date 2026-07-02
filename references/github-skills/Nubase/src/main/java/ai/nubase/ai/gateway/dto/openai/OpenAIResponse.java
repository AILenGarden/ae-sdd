package ai.nubase.ai.gateway.dto.openai;

import com.fasterxml.jackson.annotation.JsonInclude;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;

/**
 * OpenAI Chat Completion Response
 * Response from /v1/chat/completions endpoint (non-streaming)
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class OpenAIResponse {
    /**
     * Unique identifier for this completion
     */
    private String id;

    /**
     * Object type, always "chat.completion"
     */
    private String object;

    /**
     * Unix timestamp of when the completion was created
     */
    private Long created;

    /**
     * Model used for this completion
     */
    private String model;

    /**
     * List of completion choices
     * Typically contains one choice unless n > 1
     */
    private List<OpenAIChoice> choices;

    /**
     * Token usage information
     */
    private OpenAIUsage usage;
}
