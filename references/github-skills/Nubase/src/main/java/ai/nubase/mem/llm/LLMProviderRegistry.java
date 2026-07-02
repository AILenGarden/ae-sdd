package ai.nubase.mem.llm;

import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.service.MemConfigResolver;
import jakarta.annotation.PostConstruct;
import lombok.extern.slf4j.Slf4j;
import org.springframework.beans.factory.ObjectProvider;
import org.springframework.stereotype.Component;

import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;

/**
 * Selects the active chat and embedding providers per-request.
 *
 * <p>All provider beans are collected at startup; selection by name happens on every
 * {@link #chat()} / {@link #embedding()} call via {@link MemConfigResolver}, which reads
 * the current tenant's override and falls back to {@link MemProperties} when absent.
 *
 * <p>{@link MemConfigResolver} is injected via {@link ObjectProvider} to break a
 * bean-init cycle (resolver → providers → registry → resolver).
 */
@Slf4j
@Component
public class LLMProviderRegistry {

    private final MemProperties memProperties;
    private final ObjectProvider<MemConfigResolver> resolverProvider;
    private final Map<String, ChatLLMProvider> chatProviders;
    private final Map<String, EmbeddingProvider> embeddingProviders;

    public LLMProviderRegistry(MemProperties memProperties,
                               ObjectProvider<MemConfigResolver> resolverProvider,
                               List<ChatLLMProvider> chatProviderBeans,
                               List<EmbeddingProvider> embeddingProviderBeans) {
        this.memProperties = memProperties;
        this.resolverProvider = resolverProvider;
        this.chatProviders = chatProviderBeans.stream()
                .collect(Collectors.toMap(ChatLLMProvider::name, p -> p));
        this.embeddingProviders = embeddingProviderBeans.stream()
                .collect(Collectors.toMap(EmbeddingProvider::name, p -> p));
    }

    @PostConstruct
    public void logSelection() {
        log.info("Memory LLM providers registered: chat={}, embedding={}",
                chatProviders.keySet(), embeddingProviders.keySet());
        log.info("Memory LLM YAML defaults: chatProvider='{}', embeddingProvider='{}' "
                        + "(per-tenant overrides live in mem.config)",
                memProperties.getChatProvider(), memProperties.getEmbeddingProvider());
    }

    /**
     * Resolve the chat provider for the current tenant — override first, YAML otherwise.
     *
     * @throws LLMException if the resolved provider name is not registered
     */
    public ChatLLMProvider chat() {
        String name = currentChatProvider();
        ChatLLMProvider p = chatProviders.get(name);
        if (p == null) {
            throw new LLMException("No chat provider registered for name '" + name
                    + "'. Available: " + chatProviders.keySet());
        }
        return p;
    }

    /**
     * Resolve the embedding provider for the current tenant — override first, YAML otherwise.
     *
     * @throws LLMException if the resolved provider name is not registered
     */
    public EmbeddingProvider embedding() {
        String name = currentEmbeddingProvider();
        EmbeddingProvider p = embeddingProviders.get(name);
        if (p == null) {
            throw new LLMException("No embedding provider registered for name '" + name
                    + "'. Available: " + embeddingProviders.keySet());
        }
        return p;
    }

    private String currentChatProvider() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.chatProvider() : memProperties.getChatProvider();
    }

    private String currentEmbeddingProvider() {
        MemConfigResolver r = resolverProvider.getIfAvailable();
        return r != null ? r.embeddingProvider() : memProperties.getEmbeddingProvider();
    }
}
