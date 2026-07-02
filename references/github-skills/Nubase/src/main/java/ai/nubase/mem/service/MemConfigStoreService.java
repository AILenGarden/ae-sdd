package ai.nubase.mem.service;

import ai.nubase.common.context.MultiTenancyContext;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.JsonNodeFactory;
import com.fasterxml.jackson.databind.node.ObjectNode;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.dao.EmptyResultDataAccessException;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Service;

import java.util.Iterator;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Per-tenant store for the {@code mem.config} singleton row.
 *
 * <p>Reads / writes go through the routing {@link JdbcTemplate} so each call lands in the
 * caller's tenant database (determined by {@link MultiTenancyContext}). The JSONB blob is
 * a partial override — {@link MemConfigResolver} merges it on top of the platform-wide
 * YAML defaults in {@code nubase.mem.*}.
 *
 * <p>Reads are cached in-process keyed by tenant app-code; writes evict the entry.
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class MemConfigStoreService {

    private final JdbcTemplate jdbcTemplate;
    private final ObjectMapper objectMapper;

    /** appCode → parsed JsonNode override. Empty object when row is fresh. */
    private final ConcurrentHashMap<String, JsonNode> cache = new ConcurrentHashMap<>();

    /**
     * Current tenant's override blob. Falls back to an empty object when the row is
     * missing (older tenant whose {@code mem.config} table doesn't exist yet) so callers
     * can always treat the result as a non-null override layer.
     */
    public JsonNode read() {
        String tenant = MultiTenancyContext.getAppCode();
        if (tenant == null) {
            return JsonNodeFactory.instance.objectNode();
        }
        return cache.computeIfAbsent(tenant, this::loadFromDb);
    }

    /**
     * Deep-merge {@code patch} into the existing config and persist. Returns the merged
     * view. Patch semantics: object values are recursed into; everything else (primitives,
     * arrays) replaces the existing value. JSON null at the leaf clears the override and
     * lets the YAML default win again.
     */
    public JsonNode update(JsonNode patch, UUID actor) {
        if (patch == null) {
            patch = JsonNodeFactory.instance.objectNode();
        }
        String tenant = MultiTenancyContext.getAppCode();
        if (tenant == null) {
            throw new IllegalStateException("No tenant context — mem config can only be "
                    + "updated within a tenant-bound request");
        }
        ObjectNode current = readMutable();
        deepMerge(current, patch);
        String serialized;
        try {
            serialized = objectMapper.writeValueAsString(current);
        } catch (Exception e) {
            throw new IllegalStateException("Failed to serialise mem config", e);
        }
        // UPSERT — the row is created by init_mem_schema.sql but older tenants may not
        // have it. INSERT … ON CONFLICT covers both.
        jdbcTemplate.update(
                "INSERT INTO mem.config (id, config, updated_at, updated_by) "
                        + "VALUES (1, ?::jsonb, NOW(), ?) "
                        + "ON CONFLICT (id) DO UPDATE "
                        + "SET config = EXCLUDED.config, "
                        + "    updated_at = EXCLUDED.updated_at, "
                        + "    updated_by = EXCLUDED.updated_by",
                serialized, actor);
        cache.remove(tenant);
        log.info("mem.config updated for tenant={} by={}", tenant, actor);
        return current;
    }

    /** Drop cache for the current tenant — call after any out-of-band UPDATE. */
    public void invalidate() {
        String tenant = MultiTenancyContext.getAppCode();
        if (tenant != null) {
            cache.remove(tenant);
        }
    }

    // ---------- internals ----------

    private JsonNode loadFromDb(String tenant) {
        try {
            String json = jdbcTemplate.queryForObject(
                    "SELECT config::text FROM mem.config WHERE id = 1", String.class);
            if (json == null || json.isEmpty()) {
                return JsonNodeFactory.instance.objectNode();
            }
            return objectMapper.readTree(json);
        } catch (EmptyResultDataAccessException e) {
            // No row yet — treat as empty override.
            return JsonNodeFactory.instance.objectNode();
        } catch (Exception e) {
            // mem.config table missing (older tenant) or parse error: log + degrade to
            // YAML-only. Never let config read failures break the main flow.
            log.warn("Failed to load mem.config for tenant={}, falling back to YAML "
                    + "defaults: {}", tenant, e.getMessage());
            return JsonNodeFactory.instance.objectNode();
        }
    }

    private ObjectNode readMutable() {
        JsonNode node = read();
        return node instanceof ObjectNode obj ? obj.deepCopy()
                : JsonNodeFactory.instance.objectNode();
    }

    private static void deepMerge(ObjectNode target, JsonNode patch) {
        if (patch == null || !patch.isObject()) return;
        Iterator<Map.Entry<String, JsonNode>> it = patch.fields();
        while (it.hasNext()) {
            Map.Entry<String, JsonNode> entry = it.next();
            String key = entry.getKey();
            JsonNode value = entry.getValue();
            JsonNode existing = target.get(key);
            if (existing instanceof ObjectNode existingObj && value.isObject()) {
                deepMerge(existingObj, value);
            } else {
                target.set(key, value);
            }
        }
    }
}
