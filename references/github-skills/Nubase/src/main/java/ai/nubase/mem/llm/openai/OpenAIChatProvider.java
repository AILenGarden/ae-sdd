package ai.nubase.mem.llm.openai;

import ai.nubase.common.config.OpenAIConfig;
import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.llm.ChatLLMProvider;
import ai.nubase.mem.llm.ChatRequest;
import ai.nubase.mem.llm.LLMException;
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
import java.util.concurrent.TimeUnit;

/**
 * OpenAI Chat Completions provider.
 *
 * <p>Uses the official OpenAI API contract; {@code OpenAIConfig} controls credentials and base URL.
 */
@Slf4j
@Component
public class OpenAIChatProvider implements ChatLLMProvider {

    private static final MediaType JSON = MediaType.parse("application/json");

    private final OpenAIConfig openAIConfig;
    private final MemProperties memProperties;
    private final ObjectProvider<MemConfigResolver> resolverProvider;
    private final OkHttpClient httpClient;
    private final ObjectMapper objectMapper;

    public OpenAIChatProvider(OpenAIConfig openAIConfig,
                              MemProperties memProperties,
                              ObjectProvider<MemConfigResolver> resolverProvider,
                              ObjectMapper objectMapper) {
        this.openAIConfig = openAIConfig;
        this.memProperties = memProperties;
        this.resolverProvider = resolverProvider;
        this.objectMapper = objectMapper;
        // Connection-level timeout stays at YAML-time — making it per-tenant would require
        // a new OkHttpClient per call (heavy). Per-tenant timeout override is not yet
        // exposed through the resolver.
        this.httpClient = new OkHttpClient.Builder()
                .connectTimeout(openAIConfig.getTimeout(), TimeUnit.MILLISECONDS)
                .readTimeout(openAIConfig.getTimeout(), TimeUnit.MILLISECONDS)
                .writeTimeout(openAIConfig.getTimeout(), TimeUnit.MILLISECONDS)
                .build();
    }

    @Override
    public String name() {
        return "openai";
    }

    @Override
    public boolean isAvailable() {
        String t = currentAuthToken();
        return t != null && !t.isBlank();
    }

    @Override
    public String chat(ChatRequest request) throws LLMException {
        if (!isAvailable()) {
            throw new LLMException("OpenAI provider is not configured (openai.authToken missing)");
        }

        ObjectNode body = objectMapper.createObjectNode();
        body.put("model", request.getModel() != null ? request.getModel() : currentChatModel());
        body.put("temperature",
                request.getTemperature() != null ? request.getTemperature() : currentChatTemperature());
        if (request.getMaxTokens() != null) {
            body.put("max_tokens", request.getMaxTokens());
        }
        if (request.isJsonMode()) {
            ObjectNode rf = body.putObject("response_format");
            rf.put("type", "json_object");
        }

        ArrayNode messages = body.putArray("messages");
        for (var msg : request.getMessages()) {
            ObjectNode m = messages.addObject();
            m.put("role", msg.getRole());
            m.put("content", msg.getContent());
            if (msg.getName() != null) {
                m.put("name", msg.getName());
            }
        }

        String url = stripTrailingSlash(currentBaseUrl()) + "/v1/chat/completions";
        return doRequest(url, body, currentAuthToken());
    }

    private String currentAuthToken() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.openaiAuthToken() : openAIConfig.getAuthToken();
    }

    private String currentBaseUrl() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.openaiBaseUrl() : openAIConfig.getBaseUrl();
    }

    private String currentChatModel() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.chatModel() : memProperties.getChat().getModel();
    }

    private double currentChatTemperature() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.chatTemperature() : memProperties.getChat().getTemperature();
    }

    String doRequest(String url, ObjectNode body, String authToken) {
        Request req;
        try {
            req = new Request.Builder()
                    .url(url)
                    .header("Authorization", "Bearer " + authToken)
                    .header("Content-Type", "application/json")
                    .post(RequestBody.create(objectMapper.writeValueAsBytes(body), JSON))
                    .build();
        } catch (Exception e) {
            throw new LLMException("Failed to build OpenAI chat request: " + e.getMessage(), e);
        }

        try (Response resp = httpClient.newCall(req).execute()) {
            ResponseBody rb = resp.body();
            String respText = rb != null ? rb.string() : "";
            if (!resp.isSuccessful()) {
                throw new LLMException("OpenAI chat error " + resp.code() + ": " + respText);
            }
            JsonNode root = objectMapper.readTree(respText);
            JsonNode content = root.path("choices").path(0).path("message").path("content");
            if (content.isMissingNode() || content.isNull()) {
                throw new LLMException("OpenAI chat response missing choices[0].message.content: " + respText);
            }
            return content.asText();
        } catch (IOException e) {
            throw new LLMException("OpenAI chat IO error: " + e.getMessage(), e);
        }
    }

    public static String stripTrailingSlash(String s) {
        if (s == null) {
            return "";
        }
        return s.endsWith("/") ? s.substring(0, s.length() - 1) : s;
    }
}
