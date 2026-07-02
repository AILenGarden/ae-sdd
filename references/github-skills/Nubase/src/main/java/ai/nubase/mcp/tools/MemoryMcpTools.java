package ai.nubase.mcp.tools;

import ai.nubase.mem.dto.AddMemoryRequest;
import ai.nubase.mem.dto.MemoryEventResponse;
import ai.nubase.mem.dto.MemoryResponse;
import ai.nubase.mem.dto.SearchMemoryRequest;
import ai.nubase.mem.llm.ChatMessage;
import ai.nubase.mem.service.MemoryService;
import lombok.RequiredArgsConstructor;
import org.springframework.ai.tool.annotation.Tool;
import org.springframework.stereotype.Component;

import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.UUID;
import java.util.stream.Collectors;

@Component
@RequiredArgsConstructor
public class MemoryMcpTools {

    private final MemoryService memoryService;

    @Tool(description = "Search Nubase long-term memory for the current user, agent, or run. Parameters: userId UUID string optional, agentId optional, runId optional, query required, topK optional.")
    public Map<String, Object> memorySearch(String userId, String agentId, String runId, String query, Integer topK) {
        if (query == null || query.isBlank()) {
            return Map.of("success", false, "error", "query is required");
        }
        SearchMemoryRequest request = new SearchMemoryRequest();
        request.setUserId(parseUuid(userId));
        request.setAgentId(blankToNull(agentId));
        request.setRunId(blankToNull(runId));
        request.setQuery(query);
        request.setTopK(topK);
        List<MemoryResponse> results = memoryService.search(request);
        return Map.of("success", true, "results", results, "count", results.size());
    }

    @Tool(description = "Write durable Nubase memory. Parameters: userId UUID string optional, agentId optional, runId optional, content required, infer optional defaults true.")
    public Map<String, Object> memoryWrite(String userId, String agentId, String runId, String content, Boolean infer) {
        if (content == null || content.isBlank()) {
            return Map.of("success", false, "error", "content is required");
        }
        AddMemoryRequest request = new AddMemoryRequest();
        request.setUserId(parseUuid(userId));
        request.setAgentId(blankToNull(agentId));
        request.setRunId(blankToNull(runId));
        request.setInfer(infer == null || infer);
        request.setMessages(List.of(ChatMessage.user(content)));
        List<MemoryEventResponse> events = memoryService.add(request);
        return Map.of("success", true, "results", events, "count", events.size());
    }

    @Tool(description = "Return compact relevant context for an AI agent before it starts a task. Parameters: userId optional, agentId optional, runId optional, task required, topK optional.")
    public Map<String, Object> memoryContext(String userId, String agentId, String runId, String task, Integer topK) {
        Map<String, Object> search = memorySearch(userId, agentId, runId, task, topK == null ? 8 : topK);
        if (Boolean.FALSE.equals(search.get("success"))) {
            return search;
        }
        @SuppressWarnings("unchecked")
        List<MemoryResponse> results = (List<MemoryResponse>) search.get("results");
        List<String> lines = results.stream()
                .map(MemoryResponse::getMemory)
                .filter(Objects::nonNull)
                .limit(topK == null ? 8 : topK)
                .toList();
        String context = lines.isEmpty() ? "" : lines.stream().collect(Collectors.joining("\n- ", "- ", ""));
        return Map.of("success", true, "context", context, "memories", results, "count", results.size());
    }

    private UUID parseUuid(String value) {
        return value == null || value.isBlank() ? null : UUID.fromString(value);
    }

    private String blankToNull(String value) {
        return value == null || value.isBlank() ? null : value;
    }
}
