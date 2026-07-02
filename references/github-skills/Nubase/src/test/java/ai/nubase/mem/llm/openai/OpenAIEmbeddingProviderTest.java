package ai.nubase.mem.llm.openai;

import ai.nubase.common.config.OpenAIConfig;
import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.llm.LLMException;
import ai.nubase.test.MemTestSupport;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.concurrent.TimeUnit;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

class OpenAIEmbeddingProviderTest {

    private MockWebServer server;
    private OpenAIEmbeddingProvider provider;
    private ObjectMapper objectMapper;

    @BeforeEach
    void setUp() throws Exception {
        server = new MockWebServer();
        server.start();

        OpenAIConfig openAIConfig = new OpenAIConfig();
        openAIConfig.setAuthToken("sk-test");
        openAIConfig.setBaseUrl(server.url("/").toString().replaceAll("/$", ""));
        openAIConfig.setTimeout(5000);

        MemProperties memProps = new MemProperties();
        memProps.getEmbedding().setDimensions(3);
        memProps.getEmbedding().setModel("text-embedding-3-small");

        objectMapper = new ObjectMapper();
        provider = new OpenAIEmbeddingProvider(openAIConfig, memProps, MemTestSupport.emptyProvider(), objectMapper);
    }

    @AfterEach
    void tearDown() throws Exception {
        server.shutdown();
    }

    @Test
    void embedsBatchAndPreservesOrder() throws Exception {
        server.enqueue(new MockResponse()
                .setHeader("Content-Type", "application/json")
                .setBody("""
                        {"data":[
                          {"embedding":[0.1,0.2,0.3],"index":0},
                          {"embedding":[0.4,0.5,0.6],"index":1}
                        ]}
                        """));

        List<float[]> vectors = provider.embedBatch(List.of("hello", "world"));

        assertThat(vectors).hasSize(2);
        assertThat(vectors.get(0)).containsExactly(0.1f, 0.2f, 0.3f);
        assertThat(vectors.get(1)).containsExactly(0.4f, 0.5f, 0.6f);

        RecordedRequest req = server.takeRequest(2, TimeUnit.SECONDS);
        assertThat(req).isNotNull();
        assertThat(req.getPath()).isEqualTo("/v1/embeddings");
        JsonNode body = objectMapper.readTree(req.getBody().readUtf8());
        assertThat(body.get("model").asText()).isEqualTo("text-embedding-3-small");
        assertThat(body.get("dimensions").asInt()).isEqualTo(3);
        assertThat(body.get("input")).hasSize(2);
    }

    @Test
    void embedSingleReturnsFloatArray() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("{\"data\":[{\"embedding\":[1.0,2.0,3.0]}]}"));

        float[] vec = provider.embed("hi");

        assertThat(vec).containsExactly(1.0f, 2.0f, 3.0f);
    }

    @Test
    void throwsOnError() {
        server.enqueue(new MockResponse().setResponseCode(500).setBody("oops"));

        assertThatThrownBy(() -> provider.embed("hi"))
                .isInstanceOf(LLMException.class)
                .hasMessageContaining("500");
    }

    @Test
    void emptyInputReturnsEmptyList() {
        List<float[]> out = provider.embedBatch(List.of());
        assertThat(out).isEmpty();
    }
}
