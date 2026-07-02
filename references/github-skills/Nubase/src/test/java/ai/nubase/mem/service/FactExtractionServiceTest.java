package ai.nubase.mem.service;

import ai.nubase.mem.llm.ChatLLMProvider;
import ai.nubase.mem.llm.ChatMessage;
import ai.nubase.mem.llm.ChatRequest;
import ai.nubase.mem.llm.LLMProviderRegistry;
import ai.nubase.mem.service.FactExtractionService.ExtractedEntity;
import ai.nubase.mem.service.FactExtractionService.FactExtractionResult;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.ArgumentCaptor;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

/**
 * Unit tests for fact extraction. The chat provider is fully mocked.
 */
class FactExtractionServiceTest {

    private LLMProviderRegistry registry;
    private ChatLLMProvider chatProvider;
    private PromptLoader prompts;
    private ObjectMapper objectMapper;
    private FactExtractionService svc;

    @BeforeEach
    void setUp() throws Exception {
        registry = mock(LLMProviderRegistry.class);
        chatProvider = mock(ChatLLMProvider.class);
        when(registry.chat()).thenReturn(chatProvider);
        // Default: provider is configured. Tests covering the unconfigured path override this.
        when(chatProvider.isAvailable()).thenReturn(true);
        prompts = new PromptLoader();
        prompts.load();
        objectMapper = new ObjectMapper().registerModule(new JavaTimeModule());
        svc = new FactExtractionService(registry, prompts, objectMapper);
    }

    @Test
    void parsesFactsFromValidJson() {
        when(chatProvider.chat(any(ChatRequest.class)))
                .thenReturn("{\"facts\": [\"Name is John\", \"Likes pizza\"], \"entities\": [[], []]}");

        FactExtractionResult result = svc.extract(List.of(
                ChatMessage.user("My name is John and I love pizza.")
        ));

        assertThat(result.getFacts()).containsExactly("Name is John", "Likes pizza");
        assertThat(result.getEntities()).hasSize(2);
        assertThat(result.getEntities().get(0)).isEmpty();
    }

    @Test
    void parsesEntitiesAlignedWithFacts() {
        when(chatProvider.chat(any(ChatRequest.class))).thenReturn("""
                {
                  "facts": ["Name is John", "Works at Anthropic"],
                  "entities": [
                    [{"text":"John","type":"person"}],
                    [{"text":"Anthropic","type":"organization"}]
                  ]
                }
                """);

        FactExtractionResult result = svc.extract(List.of(ChatMessage.user("hi")));

        assertThat(result.getFacts()).hasSize(2);
        assertThat(result.getEntities()).hasSize(2);
        assertThat(result.getEntities().get(0)).hasSize(1);
        ExtractedEntity e0 = result.getEntities().get(0).get(0);
        assertThat(e0.getText()).isEqualTo("John");
        assertThat(e0.getType()).isEqualTo("person");
        ExtractedEntity e1 = result.getEntities().get(1).get(0);
        assertThat(e1.getText()).isEqualTo("Anthropic");
        assertThat(e1.getType()).isEqualTo("organization");
    }

    @Test
    void entitiesMissing_fallsBackToEmptyPerFact() {
        // Old shape (facts only) — entities array absent.
        when(chatProvider.chat(any(ChatRequest.class)))
                .thenReturn("{\"facts\": [\"A\", \"B\"]}");

        FactExtractionResult result = svc.extract(List.of(ChatMessage.user("x")));

        assertThat(result.getFacts()).hasSize(2);
        assertThat(result.getEntities()).hasSize(2);
        assertThat(result.getEntities()).allSatisfy(e -> assertThat(e).isEmpty());
    }

    @Test
    void entitiesLengthMismatch_ignoredAndDefaultedToEmpty() {
        when(chatProvider.chat(any(ChatRequest.class)))
                .thenReturn("{\"facts\": [\"A\", \"B\"], \"entities\": [[{\"text\":\"X\",\"type\":\"other\"}]]}");

        FactExtractionResult result = svc.extract(List.of(ChatMessage.user("x")));

        assertThat(result.getFacts()).hasSize(2);
        assertThat(result.getEntities()).hasSize(2);
        assertThat(result.getEntities()).allSatisfy(e -> assertThat(e).isEmpty());
    }

    @Test
    void stripsMarkdownCodeFences() {
        when(chatProvider.chat(any(ChatRequest.class)))
                .thenReturn("```json\n{\"facts\": [\"Lives in Tokyo\"], \"entities\": [[]]}\n```");

        FactExtractionResult result = svc.extract(List.of(ChatMessage.user("I live in Tokyo.")));

        assertThat(result.getFacts()).containsExactly("Lives in Tokyo");
    }

    @Test
    void returnsEmptyOnEmptyFactsArray() {
        when(chatProvider.chat(any(ChatRequest.class)))
                .thenReturn("{\"facts\": [], \"entities\": []}");

        FactExtractionResult result = svc.extract(List.of(ChatMessage.user("Hi.")));

        assertThat(result.getFacts()).isEmpty();
        assertThat(result.getEntities()).isEmpty();
    }

    @Test
    void returnsEmptyOnMalformedJson() {
        when(chatProvider.chat(any(ChatRequest.class))).thenReturn("not-json");

        FactExtractionResult result = svc.extract(List.of(ChatMessage.user("anything")));

        assertThat(result.isEmpty()).isTrue();
    }

    @Test
    void providerUnavailable_shortCircuitsWithoutLlmCall() {
        when(chatProvider.isAvailable()).thenReturn(false);

        FactExtractionResult result = svc.extract(List.of(ChatMessage.user("hello")));

        assertThat(result.isEmpty()).isTrue();
        verify(chatProvider, org.mockito.Mockito.never()).chat(any());
    }

    @Test
    void emptyMessagesShortCircuitsWithoutLlmCall() {
        FactExtractionResult result = svc.extract(List.of());

        assertThat(result.isEmpty()).isTrue();
        verify(chatProvider, org.mockito.Mockito.never()).chat(any());
    }

    @Test
    void extractFacts_convenienceReturnsJustFacts() {
        when(chatProvider.chat(any(ChatRequest.class)))
                .thenReturn("{\"facts\": [\"A\"], \"entities\": [[]]}");
        List<String> facts = svc.extractFacts(List.of(ChatMessage.user("hi")));
        assertThat(facts).containsExactly("A");
    }

    @Test
    void sendsJsonModeAndSystemPromptToProvider() {
        when(chatProvider.chat(any(ChatRequest.class)))
                .thenReturn("{\"facts\":[],\"entities\":[]}");

        svc.extract(List.of(ChatMessage.user("hello")));

        ArgumentCaptor<ChatRequest> captor = ArgumentCaptor.forClass(ChatRequest.class);
        verify(chatProvider).chat(captor.capture());
        ChatRequest req = captor.getValue();
        assertThat(req.isJsonMode()).isTrue();
        assertThat(req.getMessages()).hasSize(2);
        assertThat(req.getMessages().get(0).getRole()).isEqualTo("system");
        assertThat(req.getMessages().get(0).getContent())
                .contains("Personal Information Organizer");
        assertThat(req.getMessages().get(1).getRole()).isEqualTo("user");
        assertThat(req.getMessages().get(1).getContent()).contains("user: hello");
    }

    @Test
    void formatMessages_skipsSystemMessages() {
        String formatted = FactExtractionService.formatMessages(List.of(
                ChatMessage.system("be polite"),
                ChatMessage.user("hi"),
                ChatMessage.assistant("hello")
        ));
        assertThat(formatted).doesNotContain("be polite");
        assertThat(formatted).contains("user: hi");
        assertThat(formatted).contains("assistant: hello");
    }
}
