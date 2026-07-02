package ai.nubase.mem.service;

import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.llm.ChatLLMProvider;
import ai.nubase.mem.llm.ChatRequest;
import ai.nubase.mem.llm.EmbeddingProvider;
import ai.nubase.mem.llm.LLMException;
import ai.nubase.mem.llm.LLMProviderRegistry;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.when;

/**
 * Tests for {@link MemConfigService}'s snapshot assembly.
 *
 * <p>The service is a pure projector — no I/O, no DB. The interesting behaviors are
 * (a) field-mapping fidelity from {@link MemProperties} to the wire DTO and (b) defensive
 * handling of a misconfigured provider name so the settings page can render the broken
 * state instead of 500-ing.
 */
class MemConfigServiceTest {

    private MemProperties props;
    private LLMProviderRegistry registry;
    private MemConfigResolver resolver;
    private MemConfigService svc;

    @BeforeEach
    void setUp() {
        props = new MemProperties();
        registry = mock(LLMProviderRegistry.class);
        resolver = mock(MemConfigResolver.class);
        when(resolver.chatProvider()).thenAnswer(invocation -> props.getChatProvider());
        when(resolver.chatModel()).thenAnswer(invocation -> props.getChat().getModel());
        when(resolver.chatTemperature()).thenAnswer(invocation -> props.getChat().getTemperature());
        when(resolver.embeddingProvider()).thenAnswer(invocation -> props.getEmbeddingProvider());
        when(resolver.embeddingModel()).thenAnswer(invocation -> props.getEmbedding().getModel());
        when(resolver.searchDefaultTopK()).thenAnswer(invocation -> props.getSearch().getDefaultTopK());
        when(resolver.searchDefaultThreshold()).thenAnswer(invocation -> props.getSearch().getDefaultThreshold());
        when(resolver.searchEntityBoostEnabled()).thenAnswer(invocation -> props.getSearch().isEntityBoostEnabled());
        when(resolver.searchEntityMatchSimilarity()).thenAnswer(invocation -> props.getSearch().getEntityMatchSimilarity());
        when(resolver.sessionEnabled()).thenAnswer(invocation -> props.getSession().isEnabled());
        when(resolver.sessionMaxMessages()).thenAnswer(invocation -> props.getSession().getMaxMessages());
        when(resolver.sessionInjectIntoExtraction()).thenAnswer(invocation -> props.getSession().isInjectIntoExtraction());
        when(resolver.entityMaxLinkedMemoryIds()).thenAnswer(invocation -> props.getEntity().getMaxLinkedMemoryIds());
        when(resolver.isHistoryEnabled()).thenAnswer(invocation -> props.isHistoryEnabled());
        svc = new MemConfigService(props, registry, resolver);
    }

    @Test
    void snapshot_mirrorsDefaultProperties() {
        when(registry.chat()).thenReturn(stubChat("openai", true));
        when(registry.embedding()).thenReturn(stubEmbedding("openai", true, 1536));

        var snap = svc.snapshot();

        assertThat(snap.isEnabled()).isTrue();
        assertThat(snap.getChat().getProvider()).isEqualTo("openai");
        assertThat(snap.getChat().getModel()).isEqualTo("gpt-4o-mini");
        assertThat(snap.getEmbedding().getModel()).isEqualTo("text-embedding-3-small");
        assertThat(snap.getEmbedding().getDimensions()).isEqualTo(1536);
        assertThat(snap.getSearch().getDefaultTopK()).isEqualTo(5);
        assertThat(snap.getSearch().getFtsConfig()).isEqualTo("simple");
        assertThat(snap.getSession().isInjectIntoExtraction()).isFalse();
        assertThat(snap.getEntity().getMaxLinkedMemoryIds()).isEqualTo(1000);

        assertThat(snap.getProviderStatus().isChatAvailable()).isTrue();
        assertThat(snap.getProviderStatus().getChatProviderName()).isEqualTo("openai");
        assertThat(snap.getProviderStatus().isEmbeddingAvailable()).isTrue();
    }

    @Test
    void snapshot_mirrorsCustomOverrides() {
        props.setChatProvider("anthropic");
        props.getChat().setModel("claude-3-5-haiku-latest");
        props.getChat().setTemperature(0.3);
        props.getSearch().setFtsConfig("zhparser");
        props.getSession().setInjectIntoExtraction(true);
        props.getEntity().setMaxLinkedMemoryIds(2000);

        when(registry.chat()).thenReturn(stubChat("anthropic", true));
        when(registry.embedding()).thenReturn(stubEmbedding("openai", true, 1536));

        var snap = svc.snapshot();

        assertThat(snap.getChat().getProvider()).isEqualTo("anthropic");
        assertThat(snap.getChat().getModel()).isEqualTo("claude-3-5-haiku-latest");
        assertThat(snap.getChat().getTemperature()).isEqualTo(0.3);
        assertThat(snap.getSearch().getFtsConfig()).isEqualTo("zhparser");
        assertThat(snap.getSession().isInjectIntoExtraction()).isTrue();
        assertThat(snap.getEntity().getMaxLinkedMemoryIds()).isEqualTo(2000);
    }

    @Test
    void snapshot_chatProviderUnavailable_keepsConfiguredName() {
        when(registry.chat()).thenReturn(stubChat("openai", false));
        when(registry.embedding()).thenReturn(stubEmbedding("openai", true, 1536));

        var snap = svc.snapshot();

        // Settings page surfaces "available: false" so admins can see the missing API key.
        assertThat(snap.getProviderStatus().isChatAvailable()).isFalse();
        assertThat(snap.getProviderStatus().getChatProviderName()).isEqualTo("openai");
    }

    @Test
    void snapshot_misconfiguredChatProvider_doesNotThrow() {
        // Bad yml → registry.chat() blows up. The settings endpoint must NOT 500 — it's
        // the one tool admins have to see and fix the misconfiguration.
        when(registry.chat()).thenThrow(new LLMException("no chat provider registered for 'bogus'"));
        when(registry.embedding()).thenReturn(stubEmbedding("openai", true, 1536));
        props.setChatProvider("bogus");

        var snap = svc.snapshot();

        assertThat(snap.getProviderStatus().isChatAvailable()).isFalse();
        assertThat(snap.getProviderStatus().getChatProviderName()).isEqualTo("bogus");
        // Embedding still resolves correctly — the two failures are independent.
        assertThat(snap.getProviderStatus().isEmbeddingAvailable()).isTrue();
    }

    @Test
    void snapshot_misconfiguredEmbeddingProvider_doesNotThrow() {
        when(registry.chat()).thenReturn(stubChat("openai", true));
        when(registry.embedding()).thenThrow(new LLMException("no embedding provider 'bad'"));
        props.setEmbeddingProvider("bad");

        var snap = svc.snapshot();

        assertThat(snap.getProviderStatus().isEmbeddingAvailable()).isFalse();
        assertThat(snap.getProviderStatus().getEmbeddingProviderName()).isEqualTo("bad");
    }

    @Test
    void snapshot_omitsSecrets_neverExposesAnyApiKey() {
        // Defensive: provider config may expose a boolean "authTokenSet", but never a raw token field.
        when(registry.chat()).thenReturn(stubChat("openai", true));
        when(registry.embedding()).thenReturn(stubEmbedding("openai", true, 1536));
        try {
            String json = new com.fasterxml.jackson.databind.ObjectMapper()
                    .writeValueAsString(svc.snapshot());
            assertThat(json).contains("authTokenSet");
            assertThat(json).doesNotContain("\"authToken\"");
            assertThat(json).doesNotContain("apiKey");
            assertThat(json).doesNotContain("api_key");
            assertThat(json).doesNotContainIgnoringCase("secret");
            assertThat(json).doesNotContain("password");
        } catch (Exception e) {
            throw new AssertionError(e);
        }
    }

    private static ChatLLMProvider stubChat(String name, boolean available) {
        return new ChatLLMProvider() {
            @Override public String name() { return name; }
            @Override public boolean isAvailable() { return available; }
            @Override public String chat(ChatRequest request) { return ""; }
        };
    }

    private static EmbeddingProvider stubEmbedding(String name, boolean available, int dims) {
        return new EmbeddingProvider() {
            @Override public String name() { return name; }
            @Override public boolean isAvailable() { return available; }
            @Override public int dimensions() { return dims; }
            @Override public float[] embed(String text) { return new float[dims]; }
        };
    }
}
