package ai.nubase.ai.gateway.converter;

import ai.nubase.ai.gateway.dto.TokenUsage;
import ai.nubase.ai.gateway.dto.openai.*;
import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Component;

import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * OpenAI API format to Claude API format converter
 * Handles response transformation from OpenAI to Claude
 */
@Slf4j
@Component
@RequiredArgsConstructor
public class OpenAIToClaudeConverter {

    private final ObjectMapper objectMapper;

    /**
     * Convert OpenAI non-streaming response to Claude format
     *
     * @param openAIResponse OpenAI response object
     * @param modelName      Model name to include in response
     * @return Claude format JSON string
     */
    public String convertResponse(OpenAIResponse openAIResponse, String modelName) {
        ObjectNode claudeResponse = objectMapper.createObjectNode();

        // Generate message ID
        String messageId = "msg_" + UUID.randomUUID().toString().replace("-", "").substring(0, 24);
        claudeResponse.put("id", messageId);
        claudeResponse.put("type", "message");
        claudeResponse.put("role", "assistant");

        // Convert content
        ArrayNode contentArray = claudeResponse.putArray("content");
        if (openAIResponse.getChoices() != null && !openAIResponse.getChoices().isEmpty()) {
            OpenAIChoice firstChoice = openAIResponse.getChoices().get(0);

            // Check for tool calls
            boolean hasToolCalls = firstChoice.getMessage() != null
                    && firstChoice.getMessage().getToolCalls() != null
                    && !firstChoice.getMessage().getToolCalls().isEmpty();

            // Add text content if present
            if (firstChoice.getMessage() != null && firstChoice.getMessage().getContent() != null) {
                String text = firstChoice.getMessage().getContent();
                if (!text.isEmpty()) {
                    ObjectNode textBlock = objectMapper.createObjectNode();
                    textBlock.put("type", "text");
                    textBlock.put("text", text);
                    contentArray.add(textBlock);
                }
            }

            // Add tool calls if present
            if (hasToolCalls) {
                for (OpenAIToolCall toolCall : firstChoice.getMessage().getToolCalls()) {
                    ObjectNode toolUseBlock = objectMapper.createObjectNode();
                    toolUseBlock.put("type", "tool_use");
                    toolUseBlock.put("id", toolCall.getId());
                    toolUseBlock.put("name", toolCall.getFunction().getName());

                    // Parse arguments JSON string to object
                    try {
                        Object inputObject = objectMapper.readValue(
                                toolCall.getFunction().getArguments(), Object.class);
                        toolUseBlock.putPOJO("input", inputObject);
                    } catch (JsonProcessingException e) {
                        log.error("Failed to parse tool call arguments: {}", toolCall.getFunction().getArguments(), e);
                        toolUseBlock.putObject("input");
                    }

                    contentArray.add(toolUseBlock);
                }
            }

            // If no content was added, add empty text block
            if (contentArray.size() == 0) {
                ObjectNode emptyText = objectMapper.createObjectNode();
                emptyText.put("type", "text");
                emptyText.put("text", "");
                contentArray.add(emptyText);
            }

            // Convert finish reason
            String stopReason = convertFinishReason(firstChoice.getFinishReason());
            claudeResponse.put("stop_reason", stopReason);
        } else {
            // Empty response
            ObjectNode emptyText = objectMapper.createObjectNode();
            emptyText.put("type", "text");
            emptyText.put("text", "");
            contentArray.add(emptyText);
            claudeResponse.put("stop_reason", "end_turn");
        }

        claudeResponse.put("model", modelName);
        claudeResponse.putNull("stop_sequence");

        // Convert usage
        if (openAIResponse.getUsage() != null) {
            ObjectNode usage = claudeResponse.putObject("usage");
            OpenAIUsage openAIUsage = openAIResponse.getUsage();
            usage.put("input_tokens",
                    openAIUsage.getPromptTokens() != null ? openAIUsage.getPromptTokens() : 0);
            usage.put("output_tokens",
                    openAIUsage.getCompletionTokens() != null ? openAIUsage.getCompletionTokens() : 0);
        }

        return claudeResponse.toString();
    }

    /**
     * SSE event with type and data.
     */
    public record SseEvent(String eventType, String data) {
    }

    /**
     * 将 OpenAI 流式 chunk 转换为 Claude SSE 格式。
     * 使用状态机生成正确的 Claude 事件序列，包括流式工具调用。
     *
     * <p>
     * OpenAI 流式工具调用行为：
     * <ol>
     * <li>第一个包含 tool_calls 的 chunk：包含 id、type、function.name</li>
     * <li>后续 chunk：function.arguments 以增量方式到达</li>
     * <li>最后一个 chunk：finish_reason = "tool_calls"</li>
     * </ol>
     *
     * <p>
     * 转换为 Claude SSE 事件序列：
     * <ol>
     * <li>content_block_stop（关闭 index 0 的文本 block）</li>
     * <li>content_block_start（type=tool_use，包含 id 和 name）</li>
     * <li>content_block_delta（type=input_json_delta）× N</li>
     * <li>content_block_stop（工具调用 block，由 finish_reason 触发）</li>
     * </ol>
     *
     * @param chunk             OpenAI 流式 chunk
     * @param modelName         模型名称
     * @param isFirstChunk      是否为第一个 chunk
     * @param isLastChunk       是否为最后一个 chunk
     * @param hasToolCall       本次流中是否已遇到工具调用
     * @param contentBlockIndex 当前 content block 索引追踪器
     * @return Claude SSE 事件数组
     */
    public SseEvent[] convertStreamChunk(OpenAIStreamChunk chunk, String modelName,
            AtomicBoolean isFirstChunk, AtomicBoolean isLastChunk,
            AtomicBoolean hasToolCall, AtomicInteger contentBlockIndex) {
        List<SseEvent> events = new ArrayList<>();

        // 检查是否为最后一个 chunk（包含 usage 或 finish_reason）
        boolean hasUsage = chunk.getUsage() != null;
        boolean hasFinishReason = chunk.getChoices() != null
                && !chunk.getChoices().isEmpty()
                && chunk.getChoices().get(0).getFinishReason() != null;

        if (hasUsage || hasFinishReason) {
            isLastChunk.set(true);
        }

        // 第一个 chunk：发送 message_start 和 content_block_start
        if (isFirstChunk.getAndSet(false)) {
            String messageId = "msg_" + UUID.randomUUID().toString().replace("-", "").substring(0, 24);

            // message_start event
            ObjectNode messageStart = objectMapper.createObjectNode();
            messageStart.put("type", "message_start");

            ObjectNode message = messageStart.putObject("message");
            message.put("id", messageId);
            message.put("type", "message");
            message.put("role", "assistant");
            message.putArray("content");
            message.put("model", modelName);
            message.putNull("stop_reason");
            message.putNull("stop_sequence");

            ObjectNode usage = message.putObject("usage");
            usage.put("input_tokens", 0);
            usage.put("output_tokens", 0);

            events.add(new SseEvent("message_start", messageStart.toString()));

            // content_block_start 事件（初始文本 block，index 0）
            ObjectNode blockStart = objectMapper.createObjectNode();
            blockStart.put("type", "content_block_start");
            blockStart.put("index", 0);

            ObjectNode contentBlock = blockStart.putObject("content_block");
            contentBlock.put("type", "text");
            contentBlock.put("text", "");

            events.add(new SseEvent("content_block_start", blockStart.toString()));
            contentBlockIndex.set(0);
        }

        // 提取 delta 内容
        if (chunk.getChoices() != null && !chunk.getChoices().isEmpty()) {
            OpenAIChoice choice = chunk.getChoices().get(0);
            if (choice.getDelta() != null) {
                // 处理文本 delta
                if (choice.getDelta().getContent() != null) {
                    String content = choice.getDelta().getContent();
                    if (!content.isEmpty()) {
                        ObjectNode delta = objectMapper.createObjectNode();
                        delta.put("type", "content_block_delta");
                        delta.put("index", 0);

                        ObjectNode deltaContent = delta.putObject("delta");
                        deltaContent.put("type", "text_delta");
                        deltaContent.put("text", content);

                        events.add(new SseEvent("content_block_delta", delta.toString()));
                    }
                }

                // 处理流式工具调用
                if (choice.getDelta().getToolCalls() != null && !choice.getDelta().getToolCalls().isEmpty()) {
                    for (OpenAIToolCallDelta toolCallDelta : choice.getDelta().getToolCalls()) {
                        // 此工具调用的第一个 chunk：包含 id 和 function.name
                        if (toolCallDelta.getId() != null) {
                            if (!hasToolCall.getAndSet(true)) {
                                // 关闭前一个文本 block（仅在首次遇到工具调用时）
                                ObjectNode textBlockStop = objectMapper.createObjectNode();
                                textBlockStop.put("type", "content_block_stop");
                                textBlockStop.put("index", 0);
                                events.add(new SseEvent("content_block_stop", textBlockStop.toString()));
                            }

                            // 递增 content block 索引
                            int toolIndex = contentBlockIndex.incrementAndGet();

                            // 工具调用的 content_block_start
                            ObjectNode toolBlockStart = objectMapper.createObjectNode();
                            toolBlockStart.put("type", "content_block_start");
                            toolBlockStart.put("index", toolIndex);

                            ObjectNode toolBlock = toolBlockStart.putObject("content_block");
                            toolBlock.put("type", "tool_use");
                            toolBlock.put("id", toolCallDelta.getId());
                            String funcName = toolCallDelta.getFunction() != null
                                    ? toolCallDelta.getFunction().getName()
                                    : "";
                            toolBlock.put("name", funcName != null ? funcName : "");
                            toolBlock.putObject("input");

                            events.add(new SseEvent("content_block_start", toolBlockStart.toString()));
                        }

                        // 增量参数片段
                        if (toolCallDelta.getFunction() != null
                                && toolCallDelta.getFunction().getArguments() != null
                                && !toolCallDelta.getFunction().getArguments().isEmpty()) {
                            // 确定此工具调用对应的 content block 索引
                            // 单个工具调用使用 contentBlockIndex；多个时根据 delta.index 区分
                            int toolBlockIdx = contentBlockIndex.get();

                            ObjectNode inputDelta = objectMapper.createObjectNode();
                            inputDelta.put("type", "content_block_delta");
                            inputDelta.put("index", toolBlockIdx);

                            ObjectNode inputDeltaContent = inputDelta.putObject("delta");
                            inputDeltaContent.put("type", "input_json_delta");
                            inputDeltaContent.put("partial_json",
                                    toolCallDelta.getFunction().getArguments());

                            events.add(new SseEvent("content_block_delta", inputDelta.toString()));
                        }
                    }
                }
            }
        }

        // 最后一个 chunk：发送 content_block_stop、message_delta 和 message_stop
        if (isLastChunk.get()) {
            // 关闭当前 content block
            ObjectNode blockStop = objectMapper.createObjectNode();
            blockStop.put("type", "content_block_stop");
            blockStop.put("index", contentBlockIndex.get());
            events.add(new SseEvent("content_block_stop", blockStop.toString()));

            // 带有 usage 的 message_delta 事件
            ObjectNode messageDelta = objectMapper.createObjectNode();
            messageDelta.put("type", "message_delta");

            ObjectNode deltaObj = messageDelta.putObject("delta");

            // 确定 stop_reason：当存在工具调用时覆盖为 "tool_use"
            String stopReason;
            if (hasToolCall.get()) {
                stopReason = "tool_use";
            } else {
                stopReason = "end_turn";
                if (chunk.getChoices() != null && !chunk.getChoices().isEmpty()) {
                    OpenAIChoice choice = chunk.getChoices().get(0);
                    if (choice.getFinishReason() != null) {
                        stopReason = convertFinishReason(choice.getFinishReason());
                    }
                }
            }
            deltaObj.put("stop_reason", stopReason);
            deltaObj.putNull("stop_sequence");

            // 如果有 usage 信息则添加
            if (hasUsage) {
                ObjectNode usage = messageDelta.putObject("usage");
                OpenAIUsage openAIUsage = chunk.getUsage();
                usage.put("output_tokens",
                        openAIUsage.getCompletionTokens() != null ? openAIUsage.getCompletionTokens() : 0);
            }

            events.add(new SseEvent("message_delta", messageDelta.toString()));

            // message_stop event
            ObjectNode messageStop = objectMapper.createObjectNode();
            messageStop.put("type", "message_stop");
            events.add(new SseEvent("message_stop", messageStop.toString()));
        }

        return events.toArray(new SseEvent[0]);
    }

    /**
     * Convert OpenAI finish_reason to Claude stop_reason
     *
     * @param finishReason OpenAI finish reason
     * @return Claude stop reason
     */
    private String convertFinishReason(String finishReason) {
        if (finishReason == null) {
            return "end_turn";
        }

        return switch (finishReason) {
            case "stop" -> "end_turn";
            case "length" -> "max_tokens";
            case "content_filter" -> "stop_sequence"; // Approximate mapping
            case "tool_calls" -> "tool_use";
            default -> {
                log.debug("Unknown OpenAI finish_reason: {}, mapping to end_turn", finishReason);
                yield "end_turn";
            }
        };
    }

    /**
     * Convert OpenAI usage metadata to TokenUsage DTO
     *
     * @param usage OpenAI usage object
     * @return TokenUsage object
     */
    public TokenUsage convertUsage(OpenAIUsage usage) {
        if (usage == null) {
            return TokenUsage.empty();
        }

        int totalInputTokens = usage.getPromptTokens() != null ? usage.getPromptTokens() : 0;
        int cachedTokens = cachedTokens(usage);
        int billableInputTokens = Math.max(0, totalInputTokens - cachedTokens);
        int outputTokens = usage.getCompletionTokens() != null ? usage.getCompletionTokens() : 0;
        int totalTokens = usage.getTotalTokens() != null ? usage.getTotalTokens() : totalInputTokens + outputTokens;

        return TokenUsage.builder()
                .inputTokens(billableInputTokens)
                .outputTokens(outputTokens)
                .totalTokens(totalTokens)
                .cacheCreationInputTokens(0)
                .cacheReadInputTokens(cachedTokens)
                .build();
    }

    private int cachedTokens(OpenAIUsage usage) {
        if (usage.getPromptTokensDetails() != null && usage.getPromptTokensDetails().getCachedTokens() != null) {
            return usage.getPromptTokensDetails().getCachedTokens();
        }
        if (usage.getInputTokensDetails() != null && usage.getInputTokensDetails().getCachedTokens() != null) {
            return usage.getInputTokensDetails().getCachedTokens();
        }
        return 0;
    }
}
