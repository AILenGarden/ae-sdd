package ai.nubase.ai.gateway.dto.openai;

import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;
import java.util.Map;

/**
 * OpenAI Chat Completion Request
 * Maps to OpenAI /v1/chat/completions endpoint
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class OpenAIRequest {
    /**
     * Model to use for completion
     * Examples: "gpt-4", "gpt-3.5-turbo", "gpt-4-turbo-preview"
     */
    private String model;

    /**
     * List of messages in the conversation
     */
    private List<OpenAIMessage> messages;

    /**
     * Maximum number of tokens to generate
     */
    @JsonProperty("max_tokens")
    private Integer maxTokens;

    /**
     * Sampling temperature (0.0 - 2.0)
     * Higher values make output more random
     */
    private Double temperature;

    /**
     * Nucleus sampling probability (0.0 - 1.0)
     * Alternative to temperature sampling
     */
    @JsonProperty("top_p")
    private Double topP;

    /**
     * Number of completions to generate
     * Default: 1
     */
    private Integer n;

    /**
     * Whether to stream the response
     */
    private Boolean stream;

    /**
     * Up to 4 sequences where the API will stop generating
     */
    private List<String> stop;

    /**
     * Presence penalty (-2.0 to 2.0)
     * Positive values penalize new tokens based on whether they appear in the text
     * so far
     */
    @JsonProperty("presence_penalty")
    private Double presencePenalty;

    /**
     * Frequency penalty (-2.0 to 2.0)
     * Positive values penalize new tokens based on their existing frequency in the
     * text
     */
    @JsonProperty("frequency_penalty")
    private Double frequencyPenalty;

    /**
     * A unique identifier representing your end-user
     */
    private String user;

    /**
     * Stream options (for streaming responses)
     * Example: {"include_usage": true}
     */
    @JsonProperty("stream_options")
    private Map<String, Object> streamOptions;

    /**
     * 模型可调用的工具（函数）列表
     */
    private List<OpenAITool> tools;

    /**
     * 控制模型是否调用以及调用哪个函数
     * - "none": 模型不会调用任何函数
     * - "auto": 模型可以选择生成消息或调用函数
     * - {"type": "function", "function": {"name": "my_function"}}: 强制调用特定函数
     */
    @JsonProperty("tool_choice")
    private Object toolChoice;
}
