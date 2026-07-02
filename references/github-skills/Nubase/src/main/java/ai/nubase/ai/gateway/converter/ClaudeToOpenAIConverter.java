package ai.nubase.ai.gateway.converter;

import ai.nubase.ai.gateway.dto.openai.*;
import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Component;

import java.io.IOException;
import java.util.*;

/**
 * Claude API format to OpenAI API format converter
 * Handles request transformation from Claude to OpenAI
 */
@Slf4j
@Component
@RequiredArgsConstructor
public class ClaudeToOpenAIConverter {

    private final ObjectMapper objectMapper;

    /**
     * Convert Claude request JSON to OpenAI request
     *
     * @param claudeRequestJson Claude format request body
     * @return OpenAIRequest object
     * @throws IOException if parsing fails
     */
    public OpenAIRequest convertRequest(String claudeRequestJson) throws IOException {
        JsonNode claudeRequest = objectMapper.readTree(claudeRequestJson);

        // Extract model
        String model = claudeRequest.has("model")
                ? claudeRequest.get("model").asText()
                : "gpt-3.5-turbo";

        // Extract system prompt (if exists)
        String systemPrompt = null;
        if (claudeRequest.has("system")) {
            JsonNode systemNode = claudeRequest.get("system");
            if (systemNode.isTextual()) {
                systemPrompt = systemNode.asText();
            } else if (systemNode.isArray() && systemNode.size() > 0) {
                // Handle system as array (extract first text block)
                systemPrompt = extractTextFromContentArray(systemNode);
            }
        }

        // Extract messages
        JsonNode messagesNode = claudeRequest.get("messages");
        if (messagesNode == null || !messagesNode.isArray()) {
            throw new IllegalArgumentException("Claude request missing or invalid 'messages' field");
        }

        List<OpenAIMessage> messages = convertMessages(systemPrompt, messagesNode);

        // Build request
        OpenAIRequest.OpenAIRequestBuilder builder = OpenAIRequest.builder()
                .model(model)
                .messages(messages);

        // Map parameters
        if (claudeRequest.has("max_tokens")) {
            int maxTokens = claudeRequest.get("max_tokens").asInt();
            // Cap max_tokens to OpenAI's completion token limit
            int openAIMaxCompletionTokens = 16384;
            if (maxTokens > openAIMaxCompletionTokens) {
                log.debug("Capping max_tokens from {} to {} for OpenAI model", maxTokens, openAIMaxCompletionTokens);
                maxTokens = openAIMaxCompletionTokens;
            }
            builder.maxTokens(maxTokens);
        }

        if (claudeRequest.has("temperature")) {
            builder.temperature(claudeRequest.get("temperature").asDouble());
        }

        if (claudeRequest.has("top_p")) {
            builder.topP(claudeRequest.get("top_p").asDouble());
        }

        // Map stop_sequences to stop
        if (claudeRequest.has("stop_sequences")) {
            JsonNode stopNode = claudeRequest.get("stop_sequences");
            if (stopNode.isArray()) {
                List<String> stopSequences = new ArrayList<>();
                stopNode.forEach(node -> stopSequences.add(node.asText()));
                builder.stop(stopSequences);
            }
        }

        // Handle streaming
        if (claudeRequest.has("stream")) {
            boolean stream = claudeRequest.get("stream").asBoolean();
            builder.stream(stream);

            // Enable usage tracking in streaming mode
            if (stream) {
                Map<String, Object> streamOptions = new HashMap<>();
                streamOptions.put("include_usage", true);
                builder.streamOptions(streamOptions);
            }
        }

        // Convert tools (only if non-empty)
        if (claudeRequest.has("tools") && claudeRequest.get("tools").isArray()
                && claudeRequest.get("tools").size() > 0) {
            List<OpenAITool> tools = convertTools(claudeRequest.get("tools"));
            if (tools != null && !tools.isEmpty()) {
                builder.tools(tools);
            }
        }

        // Convert tool_choice
        if (claudeRequest.has("tool_choice")) {
            JsonNode toolChoiceNode = claudeRequest.get("tool_choice");
            if (toolChoiceNode.isTextual()) {
                // Simple values: "auto", "any", "none"
                String toolChoice = toolChoiceNode.asText();
                // Map Claude "any" to OpenAI "required"
                if ("any".equals(toolChoice)) {
                    builder.toolChoice("required");
                } else {
                    builder.toolChoice(toolChoice);
                }
            } else if (toolChoiceNode.isObject()) {
                // Specific tool selection
                builder.toolChoice(toolChoiceNode);
            }
        }

        // Log unsupported parameters
        if (claudeRequest.has("top_k")) {
            log.debug("OpenAI does not support 'top_k' parameter, will be ignored");
        }
        if (claudeRequest.has("metadata")) {
            log.debug("Claude 'metadata' parameter will be ignored in OpenAI request");
        }

        return builder.build();
    }

    /**
     * Convert messages array, prepending system message if exists
     *
     * @param systemPrompt Optional system prompt
     * @param messagesNode Claude messages array
     * @return List of OpenAI messages
     */
    private List<OpenAIMessage> convertMessages(String systemPrompt, JsonNode messagesNode) {
        List<OpenAIMessage> messages = new ArrayList<>();

        // 如果有系统提示则先添加
        if (systemPrompt != null && !systemPrompt.isEmpty()) {
            messages.add(OpenAIMessage.builder()
                    .role("system")
                    .content(systemPrompt)
                    .build());
        }

        // 共享映射：Claude tool_use ID -> OpenAI call_id
        // 确保 tool_result 块引用与其 tool_use 块相同的 call_id
        Map<String, String> toolIdMapping = new HashMap<>();

        // 转换用户/助手消息
        for (JsonNode messageNode : messagesNode) {
            String role = messageNode.get("role").asText();
            JsonNode contentNode = messageNode.get("content");

            if (contentNode.isTextual()) {
                // 简单字符串内容
                messages.add(OpenAIMessage.builder()
                        .role(role)
                        .content(contentNode.asText())
                        .build());
            } else if (contentNode.isArray()) {
                // 内容块数组 - 处理 text、tool_use 和 tool_result
                List<OpenAIMessage> convertedMessages = convertContentBlocks(role, contentNode, toolIdMapping);
                messages.addAll(convertedMessages);
            } else {
                log.warn("Unexpected content format in message: {}", contentNode);
                messages.add(OpenAIMessage.builder()
                        .role(role)
                        .content(contentNode.toString())
                        .build());
            }
        }

        return messages;
    }

    /**
     * 将 Claude 内容块转换为 OpenAI 消息
     * 处理 text、tool_use 和 tool_result 块
     *
     * @param role          消息角色
     * @param contentArray  内容块数组
     * @param toolIdMapping Claude tool_use ID 到 OpenAI call_id 的共享映射
     * @return OpenAI 消息列表
     */
    private List<OpenAIMessage> convertContentBlocks(String role, JsonNode contentArray,
            Map<String, String> toolIdMapping) {
        List<OpenAIMessage> messages = new ArrayList<>();
        List<OpenAIToolCall> toolCalls = new ArrayList<>();
        StringBuilder textContent = new StringBuilder();

        for (JsonNode block : contentArray) {
            String blockType = block.has("type") ? block.get("type").asText() : null;

            if ("text".equals(blockType)) {
                // 文本块
                textContent.append(block.get("text").asText());
            } else if ("tool_use".equals(blockType)) {
                // 工具使用块 - 转换为 OpenAI 工具调用
                String toolUseId = block.get("id").asText();
                String toolName = block.get("name").asText();
                JsonNode inputNode = block.get("input");

                // 将输入对象转换为 JSON 字符串（OpenAI 格式要求）
                String arguments;
                try {
                    arguments = objectMapper.writeValueAsString(inputNode);
                } catch (JsonProcessingException e) {
                    log.error("Failed to serialize tool input to JSON string", e);
                    arguments = "{}";
                }

                // 生成 OpenAI 风格的工具调用 ID 并存储映射
                String callId = toolUseId.startsWith("call_") ? toolUseId
                        : "call_" + UUID.randomUUID().toString().replace("-", "").substring(0, 24);
                toolIdMapping.put(toolUseId, callId);

                OpenAIToolCall toolCall = OpenAIToolCall.builder()
                        .id(callId)
                        .type("function")
                        .function(OpenAIFunctionCall.builder()
                                .name(toolName)
                                .arguments(arguments)
                                .build())
                        .build();
                toolCalls.add(toolCall);
            } else if ("tool_result".equals(blockType)) {
                // 工具结果块 - 创建角色为 "tool" 的独立消息
                String toolUseId = block.get("tool_use_id").asText();
                JsonNode contentNode = block.get("content");

                // 提取内容（可以是字符串或数组）
                String resultContent;
                if (contentNode == null) {
                    resultContent = "";
                } else if (contentNode.isTextual()) {
                    resultContent = contentNode.asText();
                } else if (contentNode.isArray()) {
                    // 从内容块中提取文本
                    StringBuilder resultBuilder = new StringBuilder();
                    for (JsonNode resultBlock : contentNode) {
                        if (resultBlock.has("type") && "text".equals(resultBlock.get("type").asText())) {
                            resultBuilder.append(resultBlock.get("text").asText());
                        }
                    }
                    resultContent = resultBuilder.toString();
                } else {
                    resultContent = contentNode.toString();
                }

                // 查找映射的 call_id；如未找到则生成新的
                String callId = toolIdMapping.getOrDefault(toolUseId,
                        toolUseId.startsWith("call_") ? toolUseId
                                : "call_" + UUID.randomUUID().toString().replace("-", "").substring(0, 24));

                messages.add(OpenAIMessage.builder()
                        .role("tool")
                        .toolCallId(callId)
                        .content(resultContent)
                        .build());
            }
        }

        // 如果有文本内容或工具调用，添加主消息
        if (textContent.length() > 0 || !toolCalls.isEmpty()) {
            OpenAIMessage.OpenAIMessageBuilder messageBuilder = OpenAIMessage.builder().role(role);

            if (textContent.length() > 0) {
                messageBuilder.content(textContent.toString());
            } else if (!toolCalls.isEmpty()) {
                // 仅有工具调用时，content 设为 null
                messageBuilder.content(null);
            }

            if (!toolCalls.isEmpty()) {
                messageBuilder.toolCalls(toolCalls);
            }

            messages.add(0, messageBuilder.build()); // 插入到开头
        }

        return messages;
    }

    /**
     * Convert Claude tools array to OpenAI tools format
     *
     * @param toolsNode Claude tools JSON array
     * @return List of OpenAI tools
     */
    private List<OpenAITool> convertTools(JsonNode toolsNode) {
        if (toolsNode == null || !toolsNode.isArray()) {
            return null;
        }

        List<OpenAITool> tools = new ArrayList<>();

        for (JsonNode toolNode : toolsNode) {
            String name = toolNode.get("name").asText();
            String description = toolNode.has("description") ? toolNode.get("description").asText() : "";

            // Extract input_schema and use as parameters
            Object parameters = null;
            if (toolNode.has("input_schema")) {
                parameters = toolNode.get("input_schema");
            }

            OpenAIFunction function = OpenAIFunction.builder()
                    .name(name)
                    .description(description)
                    .parameters(parameters)
                    .build();

            OpenAITool tool = OpenAITool.builder()
                    .type("function")
                    .function(function)
                    .build();

            tools.add(tool);
        }

        return tools;
    }

    /**
     * Extract text from Claude content blocks array
     *
     * @param contentArray Array of content blocks
     * @return Combined text content
     */
    private String extractTextFromContentArray(JsonNode contentArray) {
        StringBuilder textBuilder = new StringBuilder();
        for (JsonNode block : contentArray) {
            if (block.has("type") && "text".equals(block.get("type").asText())) {
                textBuilder.append(block.get("text").asText());
            }
        }
        return textBuilder.toString();
    }

    /**
     * Extract model name from Claude request
     *
     * @param claudeRequestJson Claude request JSON
     * @return Model name
     * @throws IOException if parsing fails
     */
    public String extractModelName(String claudeRequestJson) throws IOException {
        JsonNode claudeRequest = objectMapper.readTree(claudeRequestJson);
        if (claudeRequest.has("model")) {
            return claudeRequest.get("model").asText();
        }
        throw new IllegalArgumentException("Claude request missing 'model' field");
    }
}
