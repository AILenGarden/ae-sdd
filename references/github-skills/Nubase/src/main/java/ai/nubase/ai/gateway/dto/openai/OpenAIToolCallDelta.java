package ai.nubase.ai.gateway.dto.openai;

import com.fasterxml.jackson.annotation.JsonInclude;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * OpenAI 工具调用增量
 * 表示流式响应中增量的工具调用数据
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class OpenAIToolCallDelta {
    /**
     * tool_calls 数组中的工具调用索引
     */
    private Integer index;

    /**
     * 工具调用 ID（出现在此工具调用的第一个块中）
     */
    private String id;

    /**
     * 工具调用的类型（出现在第一个块中）
     * 目前仅支持 "function"
     */
    private String type;

    /**
     * 增量函数调用数据
     */
    private OpenAIFunctionCallDelta function;
}
