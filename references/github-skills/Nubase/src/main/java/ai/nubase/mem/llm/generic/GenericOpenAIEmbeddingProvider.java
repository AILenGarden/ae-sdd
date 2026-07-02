package ai.nubase.mem.llm.generic;

import ai.nubase.mem.config.GenericLlmProperties;
import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.llm.EmbeddingProvider;
import ai.nubase.mem.llm.LLMException;
import ai.nubase.mem.service.MemConfigResolver;
import com.fasterxml.jackson.databind.JsonNode;
import org.springframework.beans.factory.ObjectProvider;
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
 * Generic OpenAI-compatible embeddings provider (DashScope / DeepSeek-style endpoints).
 *
 * <p>Talks to {@code {baseUrl}/embeddings} using the OpenAI request shape. Honors
 * {@link MemProperties} for model name and dimensions.
 */
@Slf4j
@Component
public class GenericOpenAIEmbeddingProvider implements EmbeddingProvider {

    private static final MediaType JSON = MediaType.parse("application/json");

    private final GenericLlmProperties props;
    private final MemProperties memProperties;
    private final ObjectProvider<MemConfigResolver> resolverProvider;
    private final OkHttpClient httpClient;
    private final ObjectMapper objectMapper;

    public GenericOpenAIEmbeddingProvider(GenericLlmProperties props,
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
        return "generic";
    }

    @Override
    public boolean isAvailable() {
        String t = currentAuthToken();
        String b = currentBaseUrl();
        return t != null && !t.isBlank() && b != null && !b.isBlank();
    }

    @Override
    public int dimensions() {
        // pgvector column type is baked into DDL — dimensions stays YAML-only.
        return memProperties.getEmbedding().getDimensions();
    }

    @Override
    public float[] embed(String text) throws LLMException {
        return embedBatch(List.of(text)).get(0);
    }

    @Override
    public List<float[]> embedBatch(List<String> texts) throws LLMException {
        if (!isAvailable()) {
            throw new LLMException("Generic embedding provider is not configured "
                    + "(nubase.mem.generic.{authToken,baseUrl} missing)");
        }
        if (texts == null || texts.isEmpty()) {
            return List.of();
        }

        ObjectNode body = objectMapper.createObjectNode();
        body.put("model", currentEmbeddingModel());
        ArrayNode inputs = body.putArray("input");
        for (String t : texts) {
            inputs.add(t);
        }

        String url = stripTrailingSlash(currentBaseUrl()) + "/embeddings";
        Request req;
        try {
            req = new Request.Builder()
                    .url(url)
                    .header("Authorization", "Bearer " + currentAuthToken())
                    .header("Content-Type", "application/json")
                    .post(RequestBody.create(objectMapper.writeValueAsBytes(body), JSON))
                    .build();
        } catch (Exception e) {
            throw new LLMException("Failed to build generic embeddings request: " + e.getMessage(), e);
        }

        try (Response resp = httpClient.newCall(req).execute()) {
            ResponseBody rb = resp.body();
            String respText = rb != null ? rb.string() : "";
            if (!resp.isSuccessful()) {
                throw new LLMException("Generic embeddings error " + resp.code() + ": " + respText);
            }
            JsonNode root = objectMapper.readTree(respText);
            JsonNode data = root.path("data");
            if (!data.isArray()) {
                throw new LLMException("Generic embeddings response missing data[]: " + respText);
            }
            List<float[]> out = new ArrayList<>(data.size());
            for (JsonNode item : data) {
                JsonNode embArr = item.path("embedding");
                if (!embArr.isArray()) {
                    throw new LLMException("Generic embeddings response missing embedding[]: " + respText);
                }
                float[] vec = new float[embArr.size()];
                for (int i = 0; i < embArr.size(); i++) {
                    vec[i] = (float) embArr.get(i).asDouble();
                }
                out.add(vec);
            }
            return out;
        } catch (IOException e) {
            throw new LLMException("Generic embeddings IO error: " + e.getMessage(), e);
        }
    }

    private String currentAuthToken() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.genericAuthToken() : props.getAuthToken();
    }

    private String currentBaseUrl() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.genericBaseUrl() : props.getBaseUrl();
    }

    private String currentEmbeddingModel() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.embeddingModel() : memProperties.getEmbedding().getModel();
    }

    private static String stripTrailingSlash(String s) {
        if (s == null) return "";
        return s.endsWith("/") ? s.substring(0, s.length() - 1) : s;
    }
}
