package ai.nubase.ai.gateway.dto.openai;

import com.fasterxml.jackson.annotation.JsonInclude;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * OpenAI 函数调用增量
 * 表示流式响应中增量的函数调用数据
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class OpenAIFunctionCallDelta {
    /**
     * 函数名称（出现在第一个块中）
     */
    private String name;

    /**
     * 增量参数 JSON 字符串
     * 参数以块的形式流式传输
     */
    private String arguments;
}
