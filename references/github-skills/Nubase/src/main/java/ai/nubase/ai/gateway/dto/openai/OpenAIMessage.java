package ai.nubase.ai.gateway.dto.openai;

import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;

/**
 * OpenAI Chat Message
 * Represents a single message in the conversation
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class OpenAIMessage {
    /**
     * Role of the message sender
     * Possible values: "system", "user", "assistant"
     */
    private String role;

    /**
     * Content of the message
     * Currently supports text content only
     * Future versions may support multimodal content (images, etc.)
     */
    private String content;

    /**
     * Optional name of the participant
     * Used to distinguish between multiple users or assistants
     */
    private String name;

    /**
     * 助手发起的工具调用
     * 仅在助手消息中出现，当模型想要调用函数时
     */
    @JsonProperty("tool_calls")
    private List<OpenAIToolCall> toolCalls;

    /**
     * 工具结果消息的工具调用 ID
     * 仅在角色为 "tool" 的消息中出现
     */
    @JsonProperty("tool_call_id")
    private String toolCallId;
}
