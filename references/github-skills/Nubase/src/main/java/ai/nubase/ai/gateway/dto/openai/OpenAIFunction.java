package ai.nubase.ai.gateway.dto.openai;

import com.fasterxml.jackson.annotation.JsonInclude;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * OpenAI 函数定义
 * 表示模型可以调用的函数
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class OpenAIFunction {
    /**
     * 函数名称（必需）
     */
    private String name;

    /**
     * 函数描述（可选但推荐）
     */
    private String description;

    /**
     * 参数的 JSON Schema 格式定义
     * 通常是一个对象，包含 "type": "object", "properties": {...}, "required": [...]
     */
    private Object parameters;
}
