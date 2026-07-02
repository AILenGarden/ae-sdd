package ai.nubase.mem.controller;

import ai.nubase.mem.dto.MemConfigResponse;
import ai.nubase.mem.dto.MemoryEventResponse;
import ai.nubase.mem.dto.MemoryResponse;
import ai.nubase.mem.dto.MemoryStatsResponse;
import ai.nubase.mem.dto.PagedResponse;
import ai.nubase.mem.service.MemConfigService;
import ai.nubase.mem.service.MemConfigStoreService;
import ai.nubase.mem.service.MemoryService;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MockMvc;

import java.time.Instant;
import java.util.List;
import java.util.Optional;
import java.util.UUID;

import static ai.nubase.test.ControllerTestSupport.mockMvc;
import static org.hamcrest.Matchers.hasSize;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.delete;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

class MemoryControllerTest {

    private MemoryService memoryService;
    private MemConfigService memConfigService;
    private MemConfigStoreService memConfigStore;
    private ObjectMapper objectMapper;
    private MockMvc mvc;

    @BeforeEach
    void setUp() {
        memoryService = mock(MemoryService.class);
        memConfigService = mock(MemConfigService.class);
        memConfigStore = mock(MemConfigStoreService.class);
        objectMapper = new ObjectMapper();
        mvc = mockMvc(new MemoryController(memoryService, memConfigService, memConfigStore, objectMapper));
    }

    @Test
    void addReturnsMemoryEvents() throws Exception {
        UUID id = UUID.randomUUID();
        when(memoryService.add(any())).thenReturn(List.of(MemoryEventResponse.builder()
                .id(id)
                .event("ADD")
                .memory("User prefers tea")
                .build()));

        mvc.perform(post("/mem/v1/memories")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("""
                                {
                                  "userId": "11111111-1111-1111-1111-111111111111",
                                  "infer": false,
                                  "messages": [
                                    {"role": "user", "content": "I prefer tea"}
                                  ]
                                }
                                """))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.results", hasSize(1)))
                .andExpect(jsonPath("$.results[0].event").value("ADD"))
                .andExpect(jsonPath("$.results[0].memory").value("User prefers tea"));
    }

    @Test
    void listWithoutPageReturnsLegacyArray() throws Exception {
        UUID userId = UUID.randomUUID();
        when(memoryService.list(userId, "agent-a", "run-1", 50))
                .thenReturn(List.of(memory(userId, "Prefers tea")));

        mvc.perform(get("/mem/v1/memories")
                        .param("userId", userId.toString())
                        .param("agentId", "agent-a")
                        .param("runId", "run-1")
                        .param("limit", "50"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$[0].memory").value("Prefers tea"));
    }

    @Test
    void listWithPageReturnsPagedEnvelope() throws Exception {
        UUID userId = UUID.randomUUID();
        when(memoryService.listPaged(userId, null, null, null, 2, 10))
                .thenReturn(PagedResponse.of(List.of(memory(userId, "Paged memory")), 11, 2, 10));

        mvc.perform(get("/mem/v1/memories")
                        .param("userId", userId.toString())
                        .param("page", "2")
                        .param("pageSize", "10"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items[0].memory").value("Paged memory"))
                .andExpect(jsonPath("$.total").value(11))
                .andExpect(jsonPath("$.page").value(2))
                .andExpect(jsonPath("$.pageSize").value(10));
    }

    @Test
    void getReturns404WhenMemoryMissing() throws Exception {
        UUID id = UUID.randomUUID();
        when(memoryService.get(id)).thenReturn(Optional.empty());

        mvc.perform(get("/mem/v1/memories/{id}", id))
                .andExpect(status().isNotFound());
    }

    @Test
    void updateRejectsBlankMemoryText() throws Exception {
        mvc.perform(put("/mem/v1/memories/{id}", UUID.randomUUID())
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"memory\":\"   \"}"))
                .andExpect(status().isBadRequest());
    }

    @Test
    void updateReturnsEventWhenMemoryExists() throws Exception {
        UUID id = UUID.randomUUID();
        when(memoryService.update(id, "new memory")).thenReturn(Optional.of(MemoryEventResponse.builder()
                .id(id)
                .event("UPDATE")
                .memory("new memory")
                .previousMemory("old memory")
                .build()));

        mvc.perform(put("/mem/v1/memories/{id}", id)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"memory\":\"new memory\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.event").value("UPDATE"))
                .andExpect(jsonPath("$.previousMemory").value("old memory"));
    }

    @Test
    void deleteReturnsDeletedFlag() throws Exception {
        UUID id = UUID.randomUUID();
        when(memoryService.delete(id)).thenReturn(true);

        mvc.perform(delete("/mem/v1/memories/{id}", id))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.deleted").value(true))
                .andExpect(jsonPath("$.id").value(id.toString()));
    }

    @Test
    void searchReturnsHits() throws Exception {
        UUID userId = UUID.randomUUID();
        when(memoryService.search(any())).thenReturn(List.of(memory(userId, "Tea over coffee")));

        mvc.perform(post("/mem/v1/search")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("""
                                {
                                  "userId": "%s",
                                  "query": "drink preference",
                                  "topK": 5
                                }
                                """.formatted(userId)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.results[0].memory").value("Tea over coffee"));
    }

    @Test
    void statsReturnsAggregateDashboardNumbers() throws Exception {
        when(memoryService.stats(null, null, null)).thenReturn(MemoryStatsResponse.builder()
                .totalMemories(3)
                .totalEntities(2)
                .last24h(MemoryStatsResponse.RecentActivity.builder().add(1).update(1).delete(0).build())
                .build());

        mvc.perform(get("/mem/v1/stats"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalMemories").value(3))
                .andExpect(jsonPath("$.totalEntities").value(2))
                .andExpect(jsonPath("$.last24h.add").value(1));
    }

    @Test
    void configReturnsSnapshot() throws Exception {
        when(memConfigService.snapshot()).thenReturn(MemConfigResponse.builder()
                .enabled(true)
                .historyEnabled(true)
                .chat(MemConfigResponse.Chat.builder()
                        .provider("openai")
                        .model("gpt-4.1-mini")
                        .temperature(0.1)
                        .build())
                .build());

        mvc.perform(get("/mem/v1/config"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.enabled").value(true))
                .andExpect(jsonPath("$.chat.provider").value("openai"))
                .andExpect(jsonPath("$.chat.model").value("gpt-4.1-mini"));
    }

    @Test
    void updateConfigSanitizesWritablePatchBeforeStoring() throws Exception {
        when(memConfigService.snapshot()).thenReturn(MemConfigResponse.builder().enabled(true).build());

        mvc.perform(put("/mem/v1/config")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("""
                                {
                                  "historyEnabled": false,
                                  "search": {"defaultTopK": 12, "ftsConfig": "simple"},
                                  "embedding": {"model": "text-embedding-3-small", "dimensions": 999},
                                  "providers": {"openai": {"authToken": "sk-test", "baseUrl": "https://api.example.com"}},
                                  "notAllowed": true
                                }
                                """))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.enabled").value(true));

        verify(memConfigStore).update(any(JsonNode.class), eq(null));
        org.mockito.ArgumentCaptor<JsonNode> captor = org.mockito.ArgumentCaptor.forClass(JsonNode.class);
        verify(memConfigStore).update(captor.capture(), eq(null));
        JsonNode sanitized = captor.getValue();
        org.junit.jupiter.api.Assertions.assertFalse(sanitized.has("notAllowed"));
        org.junit.jupiter.api.Assertions.assertFalse(sanitized.path("embedding").has("dimensions"));
        org.junit.jupiter.api.Assertions.assertEquals(12, sanitized.path("search").path("defaultTopK").asInt());
        org.junit.jupiter.api.Assertions.assertEquals("sk-test",
                sanitized.path("providers").path("openai").path("authToken").asText());
    }

    @Test
    void resetReturnsSuccessFlag() throws Exception {
        mvc.perform(post("/mem/v1/reset"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.reset").value(true));

        verify(memoryService).reset();
    }

    private MemoryResponse memory(UUID userId, String text) {
        return MemoryResponse.builder()
                .id(UUID.randomUUID())
                .userId(userId)
                .agentId("agent-a")
                .runId("run-1")
                .memory(text)
                .createdAt(Instant.parse("2026-05-24T00:00:00Z"))
                .updatedAt(Instant.parse("2026-05-24T00:00:00Z"))
                .build();
    }
}
