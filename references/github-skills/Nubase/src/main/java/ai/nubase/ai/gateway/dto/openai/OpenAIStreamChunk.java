package ai.nubase.ai.gateway.dto.openai;

import com.fasterxml.jackson.annotation.JsonInclude;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;

/**
 * OpenAI Streaming Chunk
 * Represents a single chunk in a streaming chat completion response
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class OpenAIStreamChunk {
    /**
     * Unique identifier for this chunk
     */
    private String id;

    /**
     * Object type, always "chat.completion.chunk"
     */
    private String object;

    /**
     * Unix timestamp of when this chunk was created
     */
    private Long created;

    /**
     * Model used for this completion
     */
    private String model;

    /**
     * List of choices with delta content
     */
    private List<OpenAIChoice> choices;

    /**
     * Token usage (only present in last chunk if stream_options.include_usage is
     * true)
     */
    private OpenAIUsage usage;
}
