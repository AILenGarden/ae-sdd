package ai.nubase.mem.controller;

import ai.nubase.mem.dto.PagedResponse;
import ai.nubase.mem.entity.Entity;
import ai.nubase.mem.service.EntityStoreService;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.util.Map;
import java.util.UUID;

/**
 * REST API for inspecting and managing the entity store ({@code mem.entities}).
 *
 * <p>Same auth model as {@link MemoryController} — gated by {@code nubase.mem.enabled},
 * goes through the standard tenant filter, and {@link ai.nubase.mem.service.MemoryAuthScope}
 * decides whether the caller may see entities outside their JWT sub.
 */
@Slf4j
@RestController
@RequestMapping("/mem/v1/entities")
@RequiredArgsConstructor
@ConditionalOnProperty(value = "nubase.mem.enabled", havingValue = "true", matchIfMissing = true)
public class EntityController {

    private final EntityStoreService entityStoreService;

    /**
     * Paginated list of entities. Filter by owner triple and/or {@code type}
     * (e.g. {@code type=person}). Service-role with all filters null returns the whole tenant.
     */
    @GetMapping
    public ResponseEntity<PagedResponse<Entity>> list(
            @RequestParam(required = false) UUID userId,
            @RequestParam(required = false) String agentId,
            @RequestParam(required = false) String runId,
            @RequestParam(required = false) String type,
            @RequestParam(required = false, defaultValue = "1") Integer page,
            @RequestParam(required = false, defaultValue = "25") Integer pageSize) {
        return ResponseEntity.ok(entityStoreService.listPaged(
                userId, agentId, runId, type, page, pageSize));
    }

    @GetMapping("/{id}")
    public ResponseEntity<Entity> get(@PathVariable UUID id) {
        return entityStoreService.findByIdForScope(id)
                .map(ResponseEntity::ok)
                .orElseGet(() -> ResponseEntity.notFound().build());
    }

    @DeleteMapping("/{id}")
    public ResponseEntity<Map<String, Object>> delete(@PathVariable UUID id) {
        boolean ok = entityStoreService.deleteForScope(id);
        return ResponseEntity.ok(Map.of("deleted", ok, "id", id));
    }
}
