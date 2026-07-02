package ai.nubase.mem.service;

import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.llm.ChatLLMProvider;
import ai.nubase.mem.llm.ChatRequest;
import ai.nubase.mem.llm.LLMException;
import ai.nubase.mem.llm.LLMProviderRegistry;
import ai.nubase.mem.service.FactExtractionService.ExtractedEntity;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

class QueryEntityExtractionServiceTest {

    private LLMProviderRegistry registry;
    private ChatLLMProvider chat;
    private PromptLoader prompts;
    private MemProperties props;
    private MemConfigResolver resolver;
    private QueryEntityExtractionService svc;

    @BeforeEach
    void setUp() throws Exception {
        registry = mock(LLMProviderRegistry.class);
        chat = mock(ChatLLMProvider.class);
        when(registry.chat()).thenReturn(chat);
        when(chat.isAvailable()).thenReturn(true);
        prompts = new PromptLoader();
        prompts.load();
        props = new MemProperties();
        resolver = mock(MemConfigResolver.class);
        when(resolver.searchEntityBoostEnabled()).thenReturn(props.getSearch().isEntityBoostEnabled());
        svc = new QueryEntityExtractionService(registry, prompts, new ObjectMapper(), props, resolver);
    }

    @Test
    void providerUnavailable_returnsEmptyWithoutLlmCall() {
        when(chat.isAvailable()).thenReturn(false);
        List<ExtractedEntity> entities = svc.extract("who is John?");
        assertThat(entities).isEmpty();
        verify(chat, never()).chat(any());
    }

    @Test
    void parsesEntitiesFromJson() {
        when(chat.chat(any(ChatRequest.class))).thenReturn(
                "{\"entities\":[{\"text\":\"John\",\"type\":\"person\"},{\"text\":\"Tokyo\",\"type\":\"location\"}]}");

        List<ExtractedEntity> entities = svc.extract("who is John in Tokyo?");

        assertThat(entities).hasSize(2);
        assertThat(entities.get(0).getText()).isEqualTo("John");
        assertThat(entities.get(1).getType()).isEqualTo("location");
    }

    @Test
    void disabledByConfigSkipsLlmCall() {
        props.getSearch().setEntityBoostEnabled(false);
        when(resolver.searchEntityBoostEnabled()).thenReturn(false);

        List<ExtractedEntity> entities = svc.extract("who is John?");

        assertThat(entities).isEmpty();
        verify(chat, never()).chat(any());
    }

    @Test
    void emptyQueryReturnsEmpty() {
        assertThat(svc.extract(null)).isEmpty();
        assertThat(svc.extract("")).isEmpty();
        assertThat(svc.extract("   ")).isEmpty();
        verify(chat, never()).chat(any());
    }

    @Test
    void llmFailureReturnsEmpty() {
        when(chat.chat(any(ChatRequest.class))).thenThrow(new LLMException("boom"));
        List<ExtractedEntity> entities = svc.extract("anything");
        assertThat(entities).isEmpty();
    }

    @Test
    void malformedJsonReturnsEmpty() {
        when(chat.chat(any(ChatRequest.class))).thenReturn("not-json");
        assertThat(svc.extract("anything")).isEmpty();
    }

    @Test
    void emptyEntitiesArrayReturnsEmpty() {
        when(chat.chat(any(ChatRequest.class))).thenReturn("{\"entities\":[]}");
        assertThat(svc.extract("the weather today")).isEmpty();
    }
}
