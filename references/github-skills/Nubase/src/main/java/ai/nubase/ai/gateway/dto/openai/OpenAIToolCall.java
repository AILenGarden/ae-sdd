package ai.nubase.ai.gateway.dto.openai;

import com.fasterxml.jackson.annotation.JsonInclude;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * OpenAI 工具调用
 * 表示助手消息或响应中的工具调用
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class OpenAIToolCall {
    /**
     * 此工具调用的唯一标识符
     * 格式："call_<随机字符串>"（例如 "call_abc123"）
     */
    private String id;

    /**
     * 工具调用的类型
     * 目前仅支持 "function"
     */
    private String type;

    /**
     * 函数调用详情
     */
    private OpenAIFunctionCall function;

    /**
     * 工具调用的索引（用于流式响应）
     */
    private Integer index;
}
