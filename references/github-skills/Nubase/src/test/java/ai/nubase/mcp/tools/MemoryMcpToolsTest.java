package ai.nubase.mcp.tools;

import ai.nubase.mem.dto.AddMemoryRequest;
import ai.nubase.mem.dto.MemoryEventResponse;
import ai.nubase.mem.dto.MemoryResponse;
import ai.nubase.mem.dto.SearchMemoryRequest;
import ai.nubase.mem.service.MemoryService;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.ArgumentCaptor;

import java.util.List;
import java.util.Map;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.verifyNoInteractions;
import static org.mockito.Mockito.when;

class MemoryMcpToolsTest {

    private MemoryService memoryService;
    private MemoryMcpTools tools;

    @BeforeEach
    void setUp() {
        memoryService = mock(MemoryService.class);
        tools = new MemoryMcpTools(memoryService);
    }

    @Test
    void memorySearchReturnsResults() {
        UUID userId = UUID.randomUUID();
        MemoryResponse memory = MemoryResponse.builder()
                .id(UUID.randomUUID())
                .userId(userId)
                .agentId("codex")
                .runId("run-1")
                .memory("Use Postgres RLS for tenant isolation")
                .build();
        when(memoryService.search(org.mockito.ArgumentMatchers.any()))
                .thenReturn(List.of(memory));

        Map<String, Object> response = tools.memorySearch(
                userId.toString(), "codex", "run-1", "tenant isolation", 5);

        assertThat(response).containsEntry("success", true);
        assertThat(response).containsEntry("count", 1);
        assertThat(response.get("results")).isEqualTo(List.of(memory));

        ArgumentCaptor<SearchMemoryRequest> captor = ArgumentCaptor.forClass(SearchMemoryRequest.class);
        verify(memoryService).search(captor.capture());
        SearchMemoryRequest request = captor.getValue();
        assertThat(request.getUserId()).isEqualTo(userId);
        assertThat(request.getAgentId()).isEqualTo("codex");
        assertThat(request.getRunId()).isEqualTo("run-1");
        assertThat(request.getQuery()).isEqualTo("tenant isolation");
        assertThat(request.getTopK()).isEqualTo(5);
    }

    @Test
    void memorySearchRejectsBlankQuery() {
        Map<String, Object> response = tools.memorySearch(null, null, null, "   ", 5);

        assertThat(response).containsEntry("success", false);
        assertThat(response).containsEntry("error", "query is required");
        verifyNoInteractions(memoryService);
    }

    @Test
    void memoryWriteReturnsEvents() {
        UUID userId = UUID.randomUUID();
        MemoryEventResponse event = MemoryEventResponse.builder()
                .id(UUID.randomUUID())
                .event("ADD")
                .memory("Use compact pull requests")
                .build();
        when(memoryService.add(org.mockito.ArgumentMatchers.any()))
                .thenReturn(List.of(event));

        Map<String, Object> response = tools.memoryWrite(
                userId.toString(), "codex", "run-2", "Use compact pull requests", null);

        assertThat(response).containsEntry("success", true);
        assertThat(response).containsEntry("count", 1);
        assertThat(response.get("results")).isEqualTo(List.of(event));

        ArgumentCaptor<AddMemoryRequest> captor = ArgumentCaptor.forClass(AddMemoryRequest.class);
        verify(memoryService).add(captor.capture());
        AddMemoryRequest request = captor.getValue();
        assertThat(request.getUserId()).isEqualTo(userId);
        assertThat(request.getAgentId()).isEqualTo("codex");
        assertThat(request.getRunId()).isEqualTo("run-2");
        assertThat(request.getInfer()).isTrue();
        assertThat(request.getMessages()).hasSize(1);
        assertThat(request.getMessages().get(0).getRole()).isEqualTo("user");
        assertThat(request.getMessages().get(0).getContent()).isEqualTo("Use compact pull requests");
    }

    @Test
    void memoryWriteUsesExplicitInferFalse() {
        when(memoryService.add(org.mockito.ArgumentMatchers.any())).thenReturn(List.of());

        tools.memoryWrite(null, "codex", null, "Store verbatim", false);

        ArgumentCaptor<AddMemoryRequest> captor = ArgumentCaptor.forClass(AddMemoryRequest.class);
        verify(memoryService).add(captor.capture());
        assertThat(captor.getValue().getInfer()).isFalse();
    }

    @Test
    void memoryWriteRejectsBlankContent() {
        Map<String, Object> response = tools.memoryWrite(null, null, null, " ", null);

        assertThat(response).containsEntry("success", false);
        assertThat(response).containsEntry("error", "content is required");
        verifyNoInteractions(memoryService);
    }

    @Test
    void memoryContextReturnsCompactContext() {
        MemoryResponse first = MemoryResponse.builder()
                .id(UUID.randomUUID())
                .memory("Use service role only on the server")
                .build();
        MemoryResponse second = MemoryResponse.builder()
                .id(UUID.randomUUID())
                .memory("Prefer RLS policies for user data")
                .build();
        when(memoryService.search(org.mockito.ArgumentMatchers.any()))
                .thenReturn(List.of(first, second));

        Map<String, Object> response = tools.memoryContext(
                null, "codex", null, "auth implementation", null);

        assertThat(response).containsEntry("success", true);
        assertThat(response).containsEntry("count", 2);
        assertThat(response.get("memories")).isEqualTo(List.of(first, second));
        assertThat(response.get("context").toString())
                .contains("- Use service role only on the server")
                .contains("- Prefer RLS policies for user data");

        ArgumentCaptor<SearchMemoryRequest> captor = ArgumentCaptor.forClass(SearchMemoryRequest.class);
        verify(memoryService).search(captor.capture());
        assertThat(captor.getValue().getAgentId()).isEqualTo("codex");
        assertThat(captor.getValue().getQuery()).isEqualTo("auth implementation");
        assertThat(captor.getValue().getTopK()).isEqualTo(8);
    }

    @Test
    void memoryContextReturnsEmptyContextWhenNoMemoriesMatch() {
        when(memoryService.search(org.mockito.ArgumentMatchers.any())).thenReturn(List.of());

        Map<String, Object> response = tools.memoryContext(null, "codex", null, "unknown task", 3);

        assertThat(response).containsEntry("success", true);
        assertThat(response).containsEntry("count", 0);
        assertThat(response).containsEntry("context", "");
        assertThat(response.get("memories")).isEqualTo(List.of());
    }
}
