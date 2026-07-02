package ai.nubase.mem.llm.anthropic;

import ai.nubase.mem.config.AnthropicProperties;
import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.llm.ChatLLMProvider;
import ai.nubase.mem.llm.ChatMessage;
import ai.nubase.mem.llm.ChatRequest;
import ai.nubase.mem.llm.LLMException;
import ai.nubase.mem.llm.openai.OpenAIChatProvider;
import ai.nubase.mem.service.MemConfigResolver;
import org.springframework.beans.factory.ObjectProvider;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import lombok.extern.slf4j.Slf4j;
import okhttp3.MediaType;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.RequestBody;
import okhttp3.Response;
import okhttp3.ResponseBody;
import org.springframework.stereotype.Component;

import java.io.IOException;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;

/**
 * Anthropic Claude Messages API provider.
 *
 * <p>Translates the provider-neutral {@link ChatRequest} into Anthropic's
 * {@code /v1/messages} contract: the {@code system} role is hoisted to a top-level
 * {@code system} field, the remaining messages are sent as {@code messages}.
 *
 * <p>JSON mode is implemented by appending an explicit instruction to the system prompt,
 * since Anthropic does not (yet) support {@code response_format=json_object} natively.
 */
@Slf4j
@Component
public class AnthropicChatProvider implements ChatLLMProvider {

    private static final MediaType JSON = MediaType.parse("application/json");
    private static final String JSON_MODE_HINT =
            "\n\nIMPORTANT: Respond with a single JSON object only — no prose, no markdown fences.";

    private final AnthropicProperties props;
    private final MemProperties memProperties;
    private final ObjectProvider<MemConfigResolver> resolverProvider;
    private final OkHttpClient httpClient;
    private final ObjectMapper objectMapper;

    public AnthropicChatProvider(AnthropicProperties props,
                                 MemProperties memProperties,
                                 ObjectProvider<MemConfigResolver> resolverProvider,
                                 ObjectMapper objectMapper) {
        this.props = props;
        this.memProperties = memProperties;
        this.resolverProvider = resolverProvider;
        this.objectMapper = objectMapper;
        this.httpClient = new OkHttpClient.Builder()
                .connectTimeout(props.getTimeout(), TimeUnit.MILLISECONDS)
                .readTimeout(props.getTimeout(), TimeUnit.MILLISECONDS)
                .writeTimeout(props.getTimeout(), TimeUnit.MILLISECONDS)
                .build();
    }

    @Override
    public String name() {
        return "anthropic";
    }

    @Override
    public boolean isAvailable() {
        String t = currentAuthToken();
        return t != null && !t.isBlank();
    }

    @Override
    public String chat(ChatRequest request) throws LLMException {
        if (!isAvailable()) {
            throw new LLMException("Anthropic provider is not configured (anthropic.authToken missing)");
        }

        // Hoist system messages
        StringBuilder systemBuf = new StringBuilder();
        List<ChatMessage> convoMessages = new ArrayList<>();
        for (ChatMessage m : request.getMessages()) {
            if ("system".equalsIgnoreCase(m.getRole())) {
                if (systemBuf.length() > 0) {
                    systemBuf.append("\n\n");
                }
                systemBuf.append(m.getContent());
            } else {
                convoMessages.add(m);
            }
        }
        if (request.isJsonMode()) {
            systemBuf.append(JSON_MODE_HINT);
        }

        ObjectNode body = objectMapper.createObjectNode();
        body.put("model", request.getModel() != null ? request.getModel() : currentChatModel());
        body.put("max_tokens", request.getMaxTokens() != null ? request.getMaxTokens() : 2048);
        body.put("temperature",
                request.getTemperature() != null ? request.getTemperature() : currentChatTemperature());
        if (systemBuf.length() > 0) {
            body.put("system", systemBuf.toString());
        }

        ArrayNode messages = body.putArray("messages");
        for (ChatMessage m : convoMessages) {
            ObjectNode node = messages.addObject();
            node.put("role", m.getRole());
            node.put("content", m.getContent());
        }

        String url = OpenAIChatProvider.stripTrailingSlash(currentBaseUrl()) + "/v1/messages";
        Request req;
        try {
            req = new Request.Builder()
                    .url(url)
                    .header("x-api-key", currentAuthToken())
                    .header("anthropic-version", currentVersion())
                    .header("Content-Type", "application/json")
                    .post(RequestBody.create(objectMapper.writeValueAsBytes(body), JSON))
                    .build();
        } catch (Exception e) {
            throw new LLMException("Failed to build Anthropic chat request: " + e.getMessage(), e);
        }

        try (Response resp = httpClient.newCall(req).execute()) {
            ResponseBody rb = resp.body();
            String respText = rb != null ? rb.string() : "";
            if (!resp.isSuccessful()) {
                throw new LLMException("Anthropic chat error " + resp.code() + ": " + respText);
            }
            JsonNode root = objectMapper.readTree(respText);
            JsonNode contentArr = root.path("content");
            if (!contentArr.isArray() || contentArr.isEmpty()) {
                throw new LLMException("Anthropic chat response missing content[]: " + respText);
            }
            StringBuilder textBuf = new StringBuilder();
            for (JsonNode block : contentArr) {
                if ("text".equals(block.path("type").asText())) {
                    textBuf.append(block.path("text").asText());
                }
            }
            return textBuf.toString();
        } catch (IOException e) {
            throw new LLMException("Anthropic chat IO error: " + e.getMessage(), e);
        }
    }

    private String currentAuthToken() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.anthropicAuthToken() : props.getAuthToken();
    }

    private String currentBaseUrl() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.anthropicBaseUrl() : props.getBaseUrl();
    }

    private String currentVersion() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.anthropicVersion() : props.getVersion();
    }

    private String currentChatModel() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.chatModel() : memProperties.getChat().getModel();
    }

    private double currentChatTemperature() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.chatTemperature() : memProperties.getChat().getTemperature();
    }
}
