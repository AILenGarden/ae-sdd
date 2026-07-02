package ai.nubase.mem.llm.anthropic;

import ai.nubase.mem.config.AnthropicProperties;
import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.llm.ChatMessage;
import ai.nubase.mem.llm.ChatRequest;
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

/**
 * Wire-format verification for {@link AnthropicChatProvider} using OkHttp's MockWebServer.
 */
class AnthropicChatProviderTest {

    private MockWebServer server;
    private AnthropicChatProvider provider;
    private ObjectMapper objectMapper;

    @BeforeEach
    void setUp() throws Exception {
        server = new MockWebServer();
        server.start();

        AnthropicProperties props = new AnthropicProperties();
        props.setAuthToken("sk-anthropic-test");
        props.setBaseUrl(server.url("/").toString().replaceAll("/$", ""));
        props.setTimeout(5000);
        props.setVersion("2023-06-01");

        MemProperties memProps = new MemProperties();
        memProps.getChat().setModel("claude-3-5-haiku-latest");

        objectMapper = new ObjectMapper();
        provider = new AnthropicChatProvider(props, memProps, MemTestSupport.emptyProvider(), objectMapper);
    }

    @AfterEach
    void tearDown() throws Exception {
        server.shutdown();
    }

    @Test
    void hoistsSystemMessageAndParsesContentBlocks() throws Exception {
        server.enqueue(new MockResponse()
                .setHeader("Content-Type", "application/json")
                .setBody("""
                        {
                          "id":"msg_1",
                          "content":[
                            {"type":"text","text":"hello "},
                            {"type":"text","text":"world"}
                          ]
                        }
                        """));

        String result = provider.chat(ChatRequest.builder()
                .messages(List.of(
                        ChatMessage.system("be brief"),
                        ChatMessage.user("hi")))
                .jsonMode(true)
                .build());

        assertThat(result).isEqualTo("hello world");

        RecordedRequest req = server.takeRequest(2, TimeUnit.SECONDS);
        assertThat(req).isNotNull();
        assertThat(req.getPath()).isEqualTo("/v1/messages");
        assertThat(req.getHeader("x-api-key")).isEqualTo("sk-anthropic-test");
        assertThat(req.getHeader("anthropic-version")).isEqualTo("2023-06-01");

        JsonNode body = objectMapper.readTree(req.getBody().readUtf8());
        assertThat(body.get("model").asText()).isEqualTo("claude-3-5-haiku-latest");
        assertThat(body.has("max_tokens")).isTrue();
        assertThat(body.get("system").asText()).contains("be brief");
        // JSON-mode hint should be appended to system since Anthropic has no native json_object
        assertThat(body.get("system").asText()).contains("JSON object");
        // 'system' role should NOT appear in messages[]
        JsonNode messages = body.get("messages");
        assertThat(messages).hasSize(1);
        assertThat(messages.get(0).get("role").asText()).isEqualTo("user");
    }

    @Test
    void isAvailableFalseWithoutToken() {
        AnthropicProperties empty = new AnthropicProperties();
        empty.setAuthToken("");
        AnthropicChatProvider p = new AnthropicChatProvider(empty, new MemProperties(), MemTestSupport.emptyProvider(), objectMapper);
        assertThat(p.isAvailable()).isFalse();
    }
}
