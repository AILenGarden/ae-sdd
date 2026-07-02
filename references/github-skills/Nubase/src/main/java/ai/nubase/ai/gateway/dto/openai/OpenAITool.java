package ai.nubase.ai.gateway.dto.openai;

import com.fasterxml.jackson.annotation.JsonInclude;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * OpenAI 工具定义
 * OpenAI API 中函数声明的包装器
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class OpenAITool {
    /**
     * 工具类型
     * 目前仅支持 "function"
     */
    private String type;

    /**
     * 函数定义
     */
    private OpenAIFunction function;
}
