package ai.nubase.mem.service;

import jakarta.annotation.PostConstruct;
import lombok.extern.slf4j.Slf4j;
import org.springframework.core.io.ClassPathResource;
import org.springframework.stereotype.Component;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.stream.Collectors;

/**
 * Loads prompt text from {@code src/main/resources/prompts/} once on startup.
 */
@Slf4j
@Component
public class PromptLoader {

    private static final String FACT_RETRIEVAL_PATH = "prompts/mem_fact_retrieval.txt";
    private static final String UPDATE_MEMORY_PATH = "prompts/mem_update_memory.txt";
    private static final String QUERY_ENTITIES_PATH = "prompts/mem_query_entities.txt";

    private String factRetrievalPrompt;
    private String updateMemoryPrompt;
    private String queryEntitiesPrompt;

    @PostConstruct
    public void load() throws IOException {
        this.factRetrievalPrompt = readResource(FACT_RETRIEVAL_PATH);
        this.updateMemoryPrompt = readResource(UPDATE_MEMORY_PATH);
        this.queryEntitiesPrompt = readResource(QUERY_ENTITIES_PATH);
        log.info("Memory prompts loaded: fact_retrieval={} chars, update_memory={} chars, "
                + "query_entities={} chars",
                factRetrievalPrompt.length(), updateMemoryPrompt.length(),
                queryEntitiesPrompt.length());
    }

    public String factRetrievalPrompt() {
        return factRetrievalPrompt;
    }

    public String updateMemoryPrompt() {
        return updateMemoryPrompt;
    }

    public String queryEntitiesPrompt() {
        return queryEntitiesPrompt;
    }

    private static String readResource(String path) throws IOException {
        ClassPathResource r = new ClassPathResource(path);
        try (BufferedReader br = new BufferedReader(
                new InputStreamReader(r.getInputStream(), StandardCharsets.UTF_8))) {
            return br.lines().collect(Collectors.joining("\n"));
        }
    }
}
