package ai.nubase.mem.llm;

import ai.nubase.mem.config.MemProperties;
import ai.nubase.test.MemTestSupport;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

class LLMProviderRegistryTest {

    @Test
    void selectsConfiguredChatProvider() {
        MemProperties props = new MemProperties();
        props.setChatProvider("anthropic");
        props.setEmbeddingProvider("openai");

        ChatLLMProvider openaiChat = stubChat("openai");
        ChatLLMProvider anthropicChat = stubChat("anthropic");
        EmbeddingProvider openaiEmb = stubEmbedding("openai");

        LLMProviderRegistry reg = new LLMProviderRegistry(
                props,
                MemTestSupport.emptyProvider(),
                List.of(openaiChat, anthropicChat),
                List.of(openaiEmb)
        );
        assertThat(reg.chat()).isSameAs(anthropicChat);
        assertThat(reg.embedding()).isSameAs(openaiEmb);
    }

    @Test
    void throwsWhenConfiguredProviderMissing() {
        MemProperties props = new MemProperties();
        props.setChatProvider("nonexistent");
        LLMProviderRegistry reg = new LLMProviderRegistry(
                props, MemTestSupport.emptyProvider(), List.of(stubChat("openai")), List.of(stubEmbedding("openai")));
        assertThatThrownBy(reg::chat)
                .isInstanceOf(LLMException.class)
                .hasMessageContaining("nonexistent");
    }

    @Test
    void throwsWhenEmbeddingProviderMissing() {
        MemProperties props = new MemProperties();
        props.setChatProvider("openai");
        props.setEmbeddingProvider("nonexistent");
        LLMProviderRegistry reg = new LLMProviderRegistry(
                props, MemTestSupport.emptyProvider(), List.of(stubChat("openai")), List.of(stubEmbedding("openai")));
        assertThatThrownBy(reg::embedding)
                .isInstanceOf(LLMException.class)
                .hasMessageContaining("nonexistent");
    }

    private static ChatLLMProvider stubChat(String name) {
        return new ChatLLMProvider() {
            @Override public String name() { return name; }
            @Override public boolean isAvailable() { return true; }
            @Override public String chat(ChatRequest request) { return ""; }
        };
    }

    private static EmbeddingProvider stubEmbedding(String name) {
        return new EmbeddingProvider() {
            @Override public String name() { return name; }
            @Override public boolean isAvailable() { return true; }
            @Override public int dimensions() { return 1536; }
            @Override public float[] embed(String text) { return new float[1536]; }
        };
    }
}
