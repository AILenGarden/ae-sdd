package ai.nubase.ai.gateway.service;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;

/**
 * Token 计算服务（本地实现）
 *
 * 由于 Claude 使用专有 tokenizer，此服务提供基于启发式规则的近似计算
 *
 * 估算规则：
 * - 英文文本：约 4 个字符 = 1 token
 * - 中文文本：约 1.5 个字符 = 1 token
 * - 代码：约 3.5 个字符 = 1 token
 * - 结构化内容有额外开销
 *
 * 注意：这是近似值，实际 token 数可能有 ±10% 的误差
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class TokenCounterService {

    private final ObjectMapper objectMapper;

    /**
     * 计算消息的 token 数量
     *
     * @param requestBody 请求体 JSON 字符串
     * @return 估算的 token 数量
     */
    public int countTokens(String requestBody) {
        try {
            JsonNode root = objectMapper.readTree(requestBody);
            int totalTokens = 0;

            // 1. 计算 system 提示的 token
            if (root.has("system")) {
                String systemPrompt = root.get("system").asText();
                totalTokens += estimateTokens(systemPrompt);
            }

            // 2. 计算 messages 的 token
            if (root.has("messages")) {
                JsonNode messages = root.get("messages");
                for (JsonNode message : messages) {
                    totalTokens += countMessageTokens(message);
                }
            }

            // 3. 计算 tools 的 token（如果有）
            if (root.has("tools")) {
                JsonNode tools = root.get("tools");
                String toolsJson = tools.toString();
                totalTokens += estimateTokens(toolsJson) / 2; // 结构化数据折半
            }

            // 4. 添加固定开销（API 调用的基础开销）
            totalTokens += 10; // 每个请求约 10 token 的固定开销

            return totalTokens;

        } catch (Exception e) {
            // 返回一个保守的估算值
            return estimateTokens(requestBody);
        }
    }

    /**
     * 计算单个消息的 token 数量
     */
    private int countMessageTokens(JsonNode message) {
        int tokens = 0;

        // role 字段固定开销
        tokens += 4;

        // content 字段
        if (message.has("content")) {
            JsonNode content = message.get("content");

            if (content.isTextual()) {
                // 简单文本
                tokens += estimateTokens(content.asText());
            } else if (content.isArray()) {
                // 结构化内容（文本、图片、文档等）
                for (JsonNode block : content) {
                    tokens += countContentBlockTokens(block);
                }
            }
        }

        return tokens;
    }

    /**
     * 计算内容块的 token 数量
     */
    private int countContentBlockTokens(JsonNode block) {
        int tokens = 0;

        if (!block.has("type")) {
            return 0;
        }

        String type = block.get("type").asText();

        switch (type) {
            case "text":
                if (block.has("text")) {
                    tokens += estimateTokens(block.get("text").asText());
                }
                break;

            case "image":
                // 图片的 token 消耗取决于分辨率
                // 一般 200x200 = 85 tokens, 1568x1568 = 1600 tokens
                // 这里使用一个中等值作为默认估算
                tokens += 800; // 默认估算值
                log.debug("Image block estimated at {} tokens", 800);
                break;

            case "document":
                // PDF 文档每页约 300-500 tokens
                // 这里假设平均 5 页
                tokens += 2000; // 默认估算值
                log.debug("Document block estimated at {} tokens", 2000);
                break;

            case "tool_use":
                // 工具调用的开销
                if (block.has("input")) {
                    String input = block.get("input").toString();
                    tokens += estimateTokens(input);
                }
                tokens += 20; // 工具调用固定开销
                break;

            case "tool_result":
                // 工具结果
                if (block.has("content")) {
                    String result = block.get("content").toString();
                    tokens += estimateTokens(result);
                }
                break;

            default:
                // 未知类型，使用整个 block 的 JSON 长度估算
                tokens += estimateTokens(block.toString());
                log.debug("Unknown block type: {}, estimated {} tokens", type, tokens);
        }

        return tokens;
    }

    /**
     * 基于文本内容估算 token 数量
     *
     * 使用启发式规则：
     * - 检测语言类型（英文、中文、代码）
     * - 应用不同的字符/token 比率
     */
    private int estimateTokens(String text) {
        if (text == null || text.isEmpty()) {
            return 0;
        }

        int length = text.length();

        // 统计不同类型字符的数量
        int chineseChars = 0;
        int englishChars = 0;
        int codeChars = 0;

        for (char c : text.toCharArray()) {
            if (isChinese(c)) {
                chineseChars++;
            } else if (isCodeChar(c)) {
                codeChars++;
            } else {
                englishChars++;
            }
        }

        // 按不同规则计算 token
        double tokens = 0;

        // 中文：约 1.5 个字符 = 1 token
        tokens += chineseChars / 1.5;

        // 英文：约 4 个字符 = 1 token
        tokens += englishChars / 4.0;

        // 代码符号：约 3.5 个字符 = 1 token
        tokens += codeChars / 3.5;

        // 至少返回 1 token（如果有内容）
        return Math.max(1, (int) Math.ceil(tokens));
    }

    /**
     * 判断是否为中文字符
     */
    private boolean isChinese(char c) {
        return c >= 0x4E00 && c <= 0x9FFF;
    }

    /**
     * 判断是否为代码特殊字符
     */
    private boolean isCodeChar(char c) {
        return "{}[]()<>;:,.|&^%$#@!~`+-*/=\\\"'".indexOf(c) >= 0;
    }

    /**
     * 计算详细的 token 使用情况（包含缓存相关）
     *
     * 注意：cache_creation_input_tokens 和 cache_read_input_tokens
     * 需要结合实际的 prompt caching 配置计算，这里返回 0
     */
    public TokenCountResult countTokensDetailed(String requestBody) {
        int inputTokens = countTokens(requestBody);

        return TokenCountResult.builder()
                .inputTokens(inputTokens)
                .cacheCreationInputTokens(0) // 本地计算无法准确判断缓存
                .cacheReadInputTokens(0)      // 本地计算无法准确判断缓存
                .build();
    }

    /**
     * Token 计数结果
     */
    @lombok.Data
    @lombok.Builder
    public static class TokenCountResult {
        private int inputTokens;
        private int cacheCreationInputTokens;
        private int cacheReadInputTokens;
    }
}
