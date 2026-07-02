package ai.nubase.mem.controller;

import ai.nubase.auth.annotation.RequireServiceRole;
import ai.nubase.mem.dto.AddMemoryRequest;
import ai.nubase.mem.dto.MemoryEventResponse;
import ai.nubase.mem.dto.MemoryResponse;
import ai.nubase.mem.dto.SearchMemoryRequest;
import ai.nubase.mem.entity.MemoryHistory;
import ai.nubase.mem.service.MemConfigService;
import ai.nubase.mem.service.MemConfigStoreService;
import ai.nubase.mem.service.MemoryService;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.PutMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * REST API for AI memory.
 *
 * <p>Mirrors the public surface of mem0's server with a {@code /mem/v1} prefix.
 * Routes go through the standard multi-tenant authentication flow, so the underlying
 * database is per-tenant via {@code MultiTenancyContext}.
 *
 * <p>Gated by {@code nubase.mem.enabled} — when false, the bean is never registered, so
 * any request to {@code /mem/v1/**} returns 404. Defaults to true.
 */
@Slf4j
@RestController
@RequestMapping("/mem/v1")
@RequiredArgsConstructor
@ConditionalOnProperty(value = "nubase.mem.enabled", havingValue = "true", matchIfMissing = true)
public class MemoryController {

    private final MemoryService memoryService;
    private final MemConfigService memConfigService;
    private final MemConfigStoreService memConfigStore;
    private final ObjectMapper objectMapper;

    @PostMapping("/memories")
    public ResponseEntity<Map<String, Object>> add(@RequestBody AddMemoryRequest req) {
        List<MemoryEventResponse> events = memoryService.add(req);
        return ResponseEntity.ok(Map.of("results", events));
    }

    /**
     * List memories. Two response shapes:
     * <ul>
     *   <li>Without {@code page} param: legacy plain-array response (back-compat for SDK
     *       consumers that already use {@code limit}).</li>
     *   <li>With {@code page} param: paginated envelope {@link ai.nubase.mem.dto.PagedResponse}
     *       used by the admin UI.</li>
     * </ul>
     */
    @GetMapping("/memories")
    public ResponseEntity<?> list(
            @RequestParam(required = false) UUID userId,
            @RequestParam(required = false) String agentId,
            @RequestParam(required = false) String runId,
            @RequestParam(required = false) Integer page,
            @RequestParam(required = false, defaultValue = "25") Integer pageSize,
            @RequestParam(required = false, defaultValue = "100") Integer limit) {
        if (page != null) {
            return ResponseEntity.ok(memoryService.listPaged(
                    userId, agentId, runId, null, page, pageSize));
        }
        return ResponseEntity.ok(memoryService.list(userId, agentId, runId, limit));
    }

    @GetMapping("/memories/{id}")
    public ResponseEntity<MemoryResponse> get(@PathVariable UUID id) {
        return memoryService.get(id)
                .map(ResponseEntity::ok)
                .orElseGet(() -> ResponseEntity.notFound().build());
    }

    @GetMapping("/memories/{id}/history")
    public ResponseEntity<List<MemoryHistory>> history(@PathVariable UUID id) {
        return ResponseEntity.ok(memoryService.history(id));
    }

    /**
     * Entities linked to this memory. Returns empty list if the memory doesn't exist or
     * the caller doesn't own it (matches the existence-non-leak pattern of {@link #get}).
     */
    @GetMapping("/memories/{id}/entities")
    public ResponseEntity<List<ai.nubase.mem.entity.Entity>> linkedEntities(@PathVariable UUID id) {
        return ResponseEntity.ok(memoryService.linkedEntities(id));
    }

    @PutMapping("/memories/{id}")
    public ResponseEntity<MemoryEventResponse> update(@PathVariable UUID id,
                                                      @RequestBody Map<String, String> body) {
        String text = body.get("memory");
        if (text == null || text.isBlank()) {
            return ResponseEntity.badRequest().build();
        }
        return memoryService.update(id, text)
                .map(ResponseEntity::ok)
                .orElseGet(() -> ResponseEntity.notFound().build());
    }

    @DeleteMapping("/memories/{id}")
    public ResponseEntity<Map<String, Object>> delete(@PathVariable UUID id) {
        boolean ok = memoryService.delete(id);
        return ResponseEntity.ok(Map.of("deleted", ok, "id", id));
    }

    /**
     * Batch soft-delete all memories matching the owner triple.
     *
     * <p>At least one of the query params is required; this is enforced in the service layer.
     * Example: {@code DELETE /mem/v1/memories?userId=...}.
     */
    @DeleteMapping("/memories")
    public ResponseEntity<Map<String, Object>> deleteAll(
            @RequestParam(required = false) UUID userId,
            @RequestParam(required = false) String agentId,
            @RequestParam(required = false) String runId) {
        int count = memoryService.deleteAll(userId, agentId, runId);
        return ResponseEntity.ok(Map.of("deleted", count));
    }

    @PostMapping("/search")
    public ResponseEntity<Map<String, Object>> search(@RequestBody SearchMemoryRequest req) {
        List<MemoryResponse> hits = memoryService.search(req);
        return ResponseEntity.ok(Map.of("results", hits));
    }

    /**
     * Aggregate metrics for the admin Memory dashboard.
     * Service-role callers get unrestricted (tenant-wide) numbers; authenticated users get
     * stats scoped to their own JWT sub.
     */
    @GetMapping("/stats")
    public ResponseEntity<ai.nubase.mem.dto.MemoryStatsResponse> stats(
            @RequestParam(required = false) UUID userId,
            @RequestParam(required = false) String agentId,
            @RequestParam(required = false) String runId) {
        return ResponseEntity.ok(memoryService.stats(userId, agentId, runId));
    }

    /**
     * Read-only snapshot of the mem module's runtime configuration: providers + models +
     * thresholds + availability. Service-role only since it reveals operational details
     * (model choice, dimensions). Contains no secrets.
     */
    @GetMapping("/config")
    @RequireServiceRole(message = "Memory config requires service_role")
    public ResponseEntity<ai.nubase.mem.dto.MemConfigResponse> config() {
        return ResponseEntity.ok(memConfigService.snapshot());
    }

    /**
     * Update per-project memory config. Body is a partial JSON document — only the keys
     * supplied are written; everything else stays at the platform YAML default.
     *
     * <p>Editable subset enforced at write time: anything outside {@code historyEnabled},
     * {@code search.*}, {@code session.*}, {@code entity.maxLinkedMemoryIds} is silently
     * stripped. Embedding model / dimensions / fts_config cannot be changed here because
     * they're baked into the pgvector column type and GIN index.
     */
    @PutMapping("/config")
    @RequireServiceRole(message = "Memory config update requires service_role")
    public ResponseEntity<ai.nubase.mem.dto.MemConfigResponse> updateConfig(
            @RequestBody JsonNode patch) {
        JsonNode sanitized = sanitizeMemConfigPatch(patch);
        memConfigStore.update(sanitized, null);
        return ResponseEntity.ok(memConfigService.snapshot());
    }

    private JsonNode sanitizeMemConfigPatch(JsonNode raw) {
        com.fasterxml.jackson.databind.node.ObjectNode out = objectMapper.createObjectNode();
        if (raw == null || !raw.isObject()) return out;
        if (raw.has("historyEnabled")) out.set("historyEnabled", raw.get("historyEnabled"));
        copyKeys(raw, out, "search",
                "defaultTopK", "defaultThreshold", "entityBoostEnabled", "entityMatchSimilarity");
        copyKeys(raw, out, "session",
                "enabled", "maxMessages", "injectIntoExtraction");
        copyKeys(raw, out, "entity", "maxLinkedMemoryIds");
        // Editable chat / embedding fields. dimensions is NOT in this whitelist —
        // it's baked into the pgvector column type.
        copyKeys(raw, out, "chat", "provider", "model", "temperature");
        copyKeys(raw, out, "embedding", "provider", "model");
        // Per-tenant provider credentials. An empty-string authToken clears the override
        // (resolver then falls back to YAML). A non-empty value persists as-is.
        if (raw.has("providers") && raw.get("providers").isObject()) {
            com.fasterxml.jackson.databind.node.ObjectNode providers = objectMapper.createObjectNode();
            copyProviderCreds(raw.get("providers"), providers, "openai",
                    "authToken", "baseUrl", "timeout", "maxRetries");
            copyProviderCreds(raw.get("providers"), providers, "anthropic",
                    "authToken", "baseUrl", "timeout", "version");
            copyProviderCreds(raw.get("providers"), providers, "generic",
                    "authToken", "baseUrl", "timeout");
            if (!providers.isEmpty()) out.set("providers", providers);
        }
        return out;
    }

    private void copyKeys(JsonNode src, com.fasterxml.jackson.databind.node.ObjectNode dst,
                          String container, String... keys) {
        if (!src.has(container) || !src.get(container).isObject()) return;
        com.fasterxml.jackson.databind.node.ObjectNode sub = objectMapper.createObjectNode();
        JsonNode srcSub = src.get(container);
        for (String k : keys) {
            if (srcSub.has(k)) sub.set(k, srcSub.get(k));
        }
        if (!sub.isEmpty()) dst.set(container, sub);
    }

    private void copyProviderCreds(JsonNode src,
                                   com.fasterxml.jackson.databind.node.ObjectNode dst,
                                   String container, String... keys) {
        if (!src.has(container) || !src.get(container).isObject()) return;
        com.fasterxml.jackson.databind.node.ObjectNode sub = objectMapper.createObjectNode();
        JsonNode srcSub = src.get(container);
        for (String k : keys) {
            if (srcSub.has(k)) sub.set(k, srcSub.get(k));
        }
        if (!sub.isEmpty()) dst.set(container, sub);
    }

    /**
     * Wipe every memory, history entry, and session message in the current tenant.
     * Restricted to {@code service_role} callers — see {@link RequireServiceRole}.
     */
    @PostMapping("/reset")
    @RequireServiceRole(message = "Memory reset requires service_role")
    public ResponseEntity<Map<String, Object>> reset() {
        memoryService.reset();
        return ResponseEntity.ok(Map.of("reset", true));
    }
}
