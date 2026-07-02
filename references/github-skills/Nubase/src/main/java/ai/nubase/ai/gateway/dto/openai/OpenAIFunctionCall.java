package ai.nubase.ai.gateway.dto.openai;

import com.fasterxml.jackson.annotation.JsonInclude;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * OpenAI 函数调用
 * 表示助手发起的函数调用
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class OpenAIFunctionCall {
    /**
     * 被调用的函数名称
     */
    private String name;

    /**
     * 函数调用的参数，JSON 字符串格式
     * 注意：OpenAI 使用 JSON 字符串格式，而 Claude 使用对象
     */
    private String arguments;
}
