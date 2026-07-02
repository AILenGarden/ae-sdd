package ai.nubase.mem.llm.openai;

import ai.nubase.common.config.OpenAIConfig;
import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.llm.EmbeddingProvider;
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
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;

import static ai.nubase.mem.llm.openai.OpenAIChatProvider.stripTrailingSlash;

/**
 * OpenAI Embeddings provider (text-embedding-3-small by default, 1536 dims).
 */
@Slf4j
@Component
public class OpenAIEmbeddingProvider implements EmbeddingProvider {

    private static final MediaType JSON = MediaType.parse("application/json");

    private final OpenAIConfig openAIConfig;
    private final MemProperties memProperties;
    private final ObjectProvider<MemConfigResolver> resolverProvider;
    private final OkHttpClient httpClient;
    private final ObjectMapper objectMapper;

    public OpenAIEmbeddingProvider(OpenAIConfig openAIConfig,
                                   MemProperties memProperties,
                                   ObjectProvider<MemConfigResolver> resolverProvider,
                                   ObjectMapper objectMapper) {
        this.openAIConfig = openAIConfig;
        this.memProperties = memProperties;
        this.resolverProvider = resolverProvider;
        this.objectMapper = objectMapper;
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
    public int dimensions() {
        // pgvector column type is baked into DDL — dimensions is intentionally NOT
        // overridable per tenant. Always YAML-defined.
        return memProperties.getEmbedding().getDimensions();
    }

    @Override
    public float[] embed(String text) throws LLMException {
        return embedBatch(List.of(text)).get(0);
    }

    @Override
    public List<float[]> embedBatch(List<String> texts) throws LLMException {
        if (!isAvailable()) {
            throw new LLMException("OpenAI provider is not configured (openai.authToken missing)");
        }
        if (texts == null || texts.isEmpty()) {
            return List.of();
        }

        ObjectNode body = objectMapper.createObjectNode();
        body.put("model", currentEmbeddingModel());
        body.put("dimensions", memProperties.getEmbedding().getDimensions());
        ArrayNode inputs = body.putArray("input");
        for (String t : texts) {
            inputs.add(t);
        }

        String url = stripTrailingSlash(currentBaseUrl()) + "/v1/embeddings";
        Request req;
        try {
            req = new Request.Builder()
                    .url(url)
                    .header("Authorization", "Bearer " + currentAuthToken())
                    .header("Content-Type", "application/json")
                    .post(RequestBody.create(objectMapper.writeValueAsBytes(body), JSON))
                    .build();
        } catch (Exception e) {
            throw new LLMException("Failed to build OpenAI embeddings request: " + e.getMessage(), e);
        }

        try (Response resp = httpClient.newCall(req).execute()) {
            ResponseBody rb = resp.body();
            String respText = rb != null ? rb.string() : "";
            if (!resp.isSuccessful()) {
                throw new LLMException("OpenAI embeddings error " + resp.code() + ": " + respText);
            }
            JsonNode root = objectMapper.readTree(respText);
            JsonNode data = root.path("data");
            if (!data.isArray()) {
                throw new LLMException("OpenAI embeddings response missing data[]: " + respText);
            }
            List<float[]> out = new ArrayList<>(data.size());
            for (JsonNode item : data) {
                JsonNode embArr = item.path("embedding");
                if (!embArr.isArray()) {
                    throw new LLMException("OpenAI embeddings response missing embedding[]: " + respText);
                }
                float[] vec = new float[embArr.size()];
                for (int i = 0; i < embArr.size(); i++) {
                    vec[i] = (float) embArr.get(i).asDouble();
                }
                out.add(vec);
            }
            return out;
        } catch (IOException e) {
            throw new LLMException("OpenAI embeddings IO error: " + e.getMessage(), e);
        }
    }

    private String currentAuthToken() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.openaiAuthToken() : openAIConfig.getAuthToken();
    }

    private String currentBaseUrl() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.openaiBaseUrl() : openAIConfig.getBaseUrl();
    }

    private String currentEmbeddingModel() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.embeddingModel() : memProperties.getEmbedding().getModel();
    }
}
