package ai.nubase.ai.gateway.service;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;

/**
 * 网关侧上下文裁剪。当请求 token 估算超出渠道支持的最大输入时，删除最旧的对话轮次，
 * 直到 fit 进渠道窗口为止。失败时返回原始 body —— 主链路对裁剪不感知，不会因裁剪异常而失败。
 *
 * <p>裁剪策略：
 * <ol>
 *   <li>chars/4 估算 input tokens（含 system + 各 message content + tool_use.input + tool_result.content）</li>
 *   <li>触发阈值：approxTokens &gt; maxInputTokens × {@link #TRIGGER_RATIO}</li>
 *   <li>裁剪目标：approxTokens &lt; maxInputTokens × {@link #TARGET_RATIO}（留 headroom 给输出 + tokenizer 估算误差）</li>
 *   <li>每次从最旧端删除 (user, assistant) 一对，直到达标或保留下限 {@link #MIN_KEPT_MESSAGES}</li>
 *   <li>裁剪完成后修复首条遗存 user message 中可能的 orphan tool_result（已无对应 tool_use_id）</li>
 *   <li>在首条遗存 user message 头部插入一行裁剪说明，便于模型识别</li>
 * </ol>
 *
 * <p>不会改写：system / tools / tool_choice / 模型名 / max_tokens 等任何非 messages 字段。
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class ContextPruneService {

    /** 触发阈值比例：approxTokens 超过 max × TRIGGER_RATIO 时启动裁剪。 */
    private static final double TRIGGER_RATIO = 0.85;

    /** 裁剪目标比例：裁到 approxTokens &lt; max × TARGET_RATIO 即停。 */
    private static final double TARGET_RATIO = 0.80;

    /** 最少保留的消息数（即使裁剪也不会低于这个）—— 3 轮对话 = 6 条消息。 */
    private static final int MIN_KEPT_MESSAGES = 6;

    private final ObjectMapper objectMapper;

    /**
     * 入口方法。如果 maxInputTokens 为 null/&lt;=0，或当前请求未触发阈值，原样返回。
     * 任何异常一律吞掉并 passthrough，**保证不影响主链路**。
     */
    public PruneResult pruneIfNeeded(String requestBody, Integer maxInputTokens, String requestId) {
        if (requestBody == null || requestBody.isEmpty()) {
            return PruneResult.passthrough(requestBody, 0);
        }
        if (maxInputTokens == null || maxInputTokens <= 0) {
            return PruneResult.passthrough(requestBody, 0);
        }

        try {
            JsonNode root = objectMapper.readTree(requestBody);
            if (!(root instanceof ObjectNode)) {
                return PruneResult.passthrough(requestBody, 0);
            }
            ObjectNode rootObj = (ObjectNode) root;

            long originalTokens = estimateInputTokens(rootObj);
            long triggerThreshold = (long) (maxInputTokens * TRIGGER_RATIO);
            if (originalTokens <= triggerThreshold) {
                return PruneResult.passthrough(requestBody, originalTokens);
            }

            JsonNode messagesNode = rootObj.get("messages");
            if (!(messagesNode instanceof ArrayNode)) {
                return PruneResult.passthrough(requestBody, originalTokens);
            }
            ArrayNode messages = (ArrayNode) messagesNode;
            int originalCount = messages.size();

            long target = (long) (maxInputTokens * TARGET_RATIO);
            int dropped = dropOldestPairsUntilUnderTarget(rootObj, messages, target);
            if (dropped == 0) {
                return PruneResult.passthrough(requestBody, originalTokens);
            }

            sanitizeFirstUserMessage(messages, dropped);

            long afterTokens = estimateInputTokens(rootObj);
            String prunedBody = objectMapper.writeValueAsString(rootObj);

            String summary = String.format(
                    "max=%d trigger=%d target=%d before=%d after=%d droppedMessages=%d (originalCount=%d, kept=%d)",
                    maxInputTokens, triggerThreshold, target,
                    originalTokens, afterTokens, dropped, originalCount, messages.size());
            return new PruneResult(prunedBody, true, originalTokens, afterTokens, dropped, summary);
        } catch (Exception e) {
            log.warn("context_prune.error requestId={} err={} —— passthrough original body", requestId, e.toString());
            return PruneResult.passthrough(requestBody, 0);
        }
    }

    /**
     * 反复删除 messages[0..1]（最旧的一对 user+assistant）直到估算 ≤ target，或剩余消息数到下限。
     * 返回删除的消息条数。
     */
    private int dropOldestPairsUntilUnderTarget(ObjectNode rootObj, ArrayNode messages, long target) {
        int dropped = 0;
        while (messages.size() > MIN_KEPT_MESSAGES) {
            long current = estimateInputTokens(rootObj);
            if (current <= target) break;

            // 删一对（user + assistant），保持 alternation invariant
            messages.remove(0);
            dropped++;
            if (messages.size() > MIN_KEPT_MESSAGES) {
                messages.remove(0);
                dropped++;
            }
        }
        return dropped;
    }

    /**
     * 删除最旧对之后，首条遗存 user message 里若仍带有 tool_result 块，
     * 那些 tool_use_id 一定指向已删的 assistant message —— 是 orphan，必须清理，否则上游 400。
     * 同时在 message 头部插入一行裁剪说明，告知模型早期上下文已被网关裁剪。
     */
    private void sanitizeFirstUserMessage(ArrayNode messages, int droppedCount) {
        if (messages.isEmpty()) return;
        JsonNode first = messages.get(0);
        if (!(first instanceof ObjectNode)) return;
        ObjectNode firstMsg = (ObjectNode) first;

        // 安全起见：若首条不是 user（理论上 alternation invariant 不该出现），再丢一条
        if (!"user".equals(firstMsg.path("role").asText())) {
            messages.remove(0);
            if (messages.isEmpty()) return;
            first = messages.get(0);
            if (!(first instanceof ObjectNode)) return;
            firstMsg = (ObjectNode) first;
        }

        String notice = "[gateway-context-prune] earlier " + droppedCount
                + " message(s) were removed by the gateway to fit the upstream context window. "
                + "Continue from the remaining context.";

        JsonNode contentNode = firstMsg.get("content");
        if (contentNode == null || contentNode.isNull()) {
            firstMsg.put("content", notice);
            return;
        }
        if (contentNode.isTextual()) {
            firstMsg.put("content", notice + "\n\n" + contentNode.asText());
            return;
        }
        if (!contentNode.isArray()) {
            return;
        }

        ArrayNode oldContent = (ArrayNode) contentNode;
        ArrayNode newContent = objectMapper.createArrayNode();
        ObjectNode noticeBlock = objectMapper.createObjectNode();
        noticeBlock.put("type", "text");
        noticeBlock.put("text", notice);
        newContent.add(noticeBlock);

        for (JsonNode block : oldContent) {
            String type = block.path("type").asText();
            if ("tool_result".equals(type)) {
                // orphan: 它对应的 tool_use 已经被删掉，必须丢弃
                continue;
            }
            newContent.add(block);
        }
        firstMsg.set("content", newContent);
    }

    /**
     * chars/4 估算 input tokens —— 保持和诊断日志的 approxInputTokens 同口径。
     */
    private long estimateInputTokens(ObjectNode rootObj) {
        long totalChars = 0;

        JsonNode system = rootObj.get("system");
        if (system != null && !system.isNull()) {
            if (system.isTextual()) {
                totalChars += system.asText().length();
            } else if (system.isArray()) {
                for (JsonNode b : system) {
                    totalChars += b.path("text").asText("").length();
                }
            }
        }

        JsonNode messages = rootObj.get("messages");
        if (messages != null && messages.isArray()) {
            for (JsonNode msg : messages) {
                JsonNode content = msg.get("content");
                if (content == null) continue;
                if (content.isTextual()) {
                    totalChars += content.asText().length();
                } else if (content.isArray()) {
                    for (JsonNode block : content) {
                        String type = block.path("type").asText("");
                        if ("text".equals(type)) {
                            totalChars += block.path("text").asText("").length();
                        } else if ("tool_result".equals(type)) {
                            JsonNode tc = block.get("content");
                            if (tc != null) totalChars += tc.toString().length();
                        } else if ("tool_use".equals(type)) {
                            JsonNode input = block.get("input");
                            if (input != null) totalChars += input.toString().length();
                        }
                    }
                }
            }
        }
        return totalChars / 4;
    }

    /** 裁剪结果。pruned=false 时表示原样透传，body 即原始 requestBody。 */
    public static final class PruneResult {
        public final String body;
        public final boolean pruned;
        public final long approxInputTokensBefore;
        public final long approxInputTokensAfter;
        public final int droppedMessages;
        public final String summary;

        public PruneResult(String body, boolean pruned, long before, long after, int dropped, String summary) {
            this.body = body;
            this.pruned = pruned;
            this.approxInputTokensBefore = before;
            this.approxInputTokensAfter = after;
            this.droppedMessages = dropped;
            this.summary = summary;
        }

        static PruneResult passthrough(String body, long approxTokens) {
            return new PruneResult(body, false, approxTokens, approxTokens, 0, null);
        }
    }
}
