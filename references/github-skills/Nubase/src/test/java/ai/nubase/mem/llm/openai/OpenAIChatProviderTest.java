package ai.nubase.mem.llm.openai;

import ai.nubase.common.config.OpenAIConfig;
import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.llm.ChatMessage;
import ai.nubase.mem.llm.ChatRequest;
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

/**
 * Wire-format verification for {@link OpenAIChatProvider} using OkHttp's MockWebServer.
 */
class OpenAIChatProviderTest {

    private MockWebServer server;
    private OpenAIChatProvider provider;
    private ObjectMapper objectMapper;

    @BeforeEach
    void setUp() throws Exception {
        server = new MockWebServer();
        server.start();

        OpenAIConfig openAIConfig = new OpenAIConfig();
        openAIConfig.setAuthToken("sk-test-token");
        openAIConfig.setBaseUrl(server.url("/").toString().replaceAll("/$", ""));
        openAIConfig.setTimeout(5000);

        MemProperties memProps = new MemProperties();
        memProps.getChat().setModel("gpt-4o-mini");

        objectMapper = new ObjectMapper();
        provider = new OpenAIChatProvider(openAIConfig, memProps, MemTestSupport.emptyProvider(), objectMapper);
    }

    @AfterEach
    void tearDown() throws Exception {
        server.shutdown();
    }

    @Test
    void sendsExpectedRequestAndParsesContent() throws Exception {
        server.enqueue(new MockResponse()
                .setHeader("Content-Type", "application/json")
                .setBody("""
                        {"id":"x","choices":[{"message":{"role":"assistant","content":"hello world"}}]}
                        """));

        String result = provider.chat(ChatRequest.builder()
                .messages(List.of(ChatMessage.system("be brief"), ChatMessage.user("hi")))
                .jsonMode(true)
                .temperature(0.0)
                .build());

        assertThat(result).isEqualTo("hello world");

        RecordedRequest req = server.takeRequest(2, TimeUnit.SECONDS);
        assertThat(req).isNotNull();
        assertThat(req.getPath()).isEqualTo("/v1/chat/completions");
        assertThat(req.getHeader("Authorization")).isEqualTo("Bearer sk-test-token");

        JsonNode body = objectMapper.readTree(req.getBody().readUtf8());
        assertThat(body.get("model").asText()).isEqualTo("gpt-4o-mini");
        assertThat(body.get("temperature").asDouble()).isEqualTo(0.0);
        assertThat(body.get("response_format").get("type").asText()).isEqualTo("json_object");
        assertThat(body.get("messages")).hasSize(2);
        assertThat(body.get("messages").get(0).get("role").asText()).isEqualTo("system");
        assertThat(body.get("messages").get(1).get("content").asText()).isEqualTo("hi");
    }

    @Test
    void throwsLLMExceptionOnNon2xx() {
        server.enqueue(new MockResponse()
                .setResponseCode(401)
                .setBody("{\"error\":\"unauthorized\"}"));

        assertThatThrownBy(() -> provider.chat(ChatRequest.builder()
                .messages(List.of(ChatMessage.user("hi")))
                .build()))
                .isInstanceOf(LLMException.class)
                .hasMessageContaining("401");
    }

    @Test
    void isAvailableReflectsToken() {
        OpenAIConfig conf = new OpenAIConfig();
        conf.setAuthToken("");
        OpenAIChatProvider empty = new OpenAIChatProvider(conf, new MemProperties(), MemTestSupport.emptyProvider(), objectMapper);
        assertThat(empty.isAvailable()).isFalse();
    }

    @Test
    void stripTrailingSlashHandlesEdges() {
        assertThat(OpenAIChatProvider.stripTrailingSlash(null)).isEmpty();
        assertThat(OpenAIChatProvider.stripTrailingSlash("https://api.openai.com/"))
                .isEqualTo("https://api.openai.com");
        assertThat(OpenAIChatProvider.stripTrailingSlash("https://api.openai.com"))
                .isEqualTo("https://api.openai.com");
    }
}
