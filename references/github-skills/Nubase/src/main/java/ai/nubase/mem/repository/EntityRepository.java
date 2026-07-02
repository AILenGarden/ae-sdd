package ai.nubase.mem.repository;

import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.entity.Entity;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.postgresql.util.PGobject;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.jdbc.core.RowMapper;
import org.springframework.stereotype.Repository;

import java.sql.Array;
import java.sql.SQLException;
import java.sql.Timestamp;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Optional;
import java.util.UUID;

import static ai.nubase.mem.repository.MemoryRepository.parseVector;
import static ai.nubase.mem.repository.MemoryRepository.serializeVector;

/**
 * JdbcTemplate repository for {@code mem.entities}.
 *
 * <p>Bypasses JPA for the same reason as {@link MemoryRepository}: the {@code embedding}
 * column uses pgvector. Linked memory ids are stored as a PostgreSQL {@code UUID[]} array.
 */
@Slf4j
@Repository
@RequiredArgsConstructor
public class EntityRepository {

    private final JdbcTemplate jdbcTemplate;
    private final MemProperties memProperties;

    private static final String TABLE = "mem.entities";

    /**
     * Insert a new entity row with the given memory link already in its array.
     *
     * @return generated entity id
     */
    public UUID insert(Entity e, UUID linkedMemoryId) {
        UUID id = e.getId() != null ? e.getId() : UUID.randomUUID();
        Instant now = Instant.now();
        jdbcTemplate.update(
                con -> {
                    var ps = con.prepareStatement(
                            "INSERT INTO " + TABLE + " (id, user_id, agent_id, run_id, "
                                    + "text, entity_type, embedding, linked_memory_ids, "
                                    + "created_at, updated_at) "
                                    + "VALUES (?, ?, ?, ?, ?, ?, ?::vector, ?, ?, ?)");
                    ps.setObject(1, id);
                    ps.setObject(2, e.getUserId());
                    ps.setString(3, e.getAgentId());
                    ps.setString(4, e.getRunId());
                    ps.setString(5, e.getText());
                    ps.setString(6, e.getEntityType());
                    ps.setString(7, serializeVector(e.getEmbedding()));
                    Array arr = con.createArrayOf("uuid",
                            linkedMemoryId == null ? new UUID[0] : new UUID[]{linkedMemoryId});
                    ps.setArray(8, arr);
                    ps.setTimestamp(9, Timestamp.from(now));
                    ps.setTimestamp(10, Timestamp.from(now));
                    return ps;
                });
        return id;
    }

    /**
     * Append a memory id to an existing entity's {@code linked_memory_ids} (idempotent).
     *
     * <p>Bounded by {@code nubase.mem.entity.maxLinkedMemoryIds}: once an entity's array
     * reaches the cap, further appends silently no-op. This prevents pathological growth
     * on hot entities (e.g. the user's own name) where the entity-boost contribution per
     * link is already negligible due to spread attenuation.
     *
     * @return rows affected (0 if entity was not found, already linked, or at cap)
     */
    public int appendLinkedMemory(UUID entityId, UUID memoryId) {
        int cap = memProperties != null && memProperties.getEntity() != null
                ? memProperties.getEntity().getMaxLinkedMemoryIds()
                : 1000;
        return jdbcTemplate.update(
                "UPDATE " + TABLE + " SET linked_memory_ids = "
                        + " CASE WHEN ? = ANY(linked_memory_ids) THEN linked_memory_ids "
                        + "      WHEN cardinality(linked_memory_ids) >= ? THEN linked_memory_ids "
                        + "      ELSE array_append(linked_memory_ids, ?) END "
                        + "WHERE id = ?",
                memoryId, cap, memoryId, entityId
        );
    }

    /**
     * Remove a memory id from every entity that links to it. Called when a memory is deleted.
     *
     * <p>Owner filters are required (defense in depth — never strip links across tenants).
     *
     * @return rows affected
     */
    public int removeMemoryLinks(UUID memoryId,
                                 UUID userId, String agentId, String runId) {
        StringBuilder sql = new StringBuilder(
                "UPDATE " + TABLE
                        + " SET linked_memory_ids = array_remove(linked_memory_ids, ?) "
                        + "WHERE ? = ANY(linked_memory_ids)");
        List<Object> args = new ArrayList<>();
        args.add(memoryId);
        args.add(memoryId);
        if (userId != null) {
            sql.append(" AND user_id = ?");
            args.add(userId);
        }
        if (agentId != null) {
            sql.append(" AND agent_id = ?");
            args.add(agentId);
        }
        if (runId != null) {
            sql.append(" AND run_id = ?");
            args.add(runId);
        }
        return jdbcTemplate.update(sql.toString(), args.toArray());
    }

    /**
     * Cosine-similarity search for entities matching a query embedding within an owner scope.
     *
     * <p>{@link Entity#getScore()} carries the raw cosine <em>distance</em> (0 = identical).
     */
    public List<Entity> searchByVector(float[] embedding,
                                       UUID userId, String agentId, String runId,
                                       int topK) {
        StringBuilder sql = new StringBuilder(
                "SELECT *, (embedding <=> ?::vector) AS distance FROM " + TABLE
                        + " WHERE embedding IS NOT NULL");
        List<Object> args = new ArrayList<>();
        args.add(serializeVector(embedding));
        if (userId != null) {
            sql.append(" AND user_id = ?");
            args.add(userId);
        }
        if (agentId != null) {
            sql.append(" AND agent_id = ?");
            args.add(agentId);
        }
        if (runId != null) {
            sql.append(" AND run_id = ?");
            args.add(runId);
        }
        sql.append(" ORDER BY embedding <=> ?::vector LIMIT ?");
        args.add(serializeVector(embedding));
        args.add(topK);
        return jdbcTemplate.query(sql.toString(), rowMapper(true), args.toArray());
    }

    /**
     * Look up an entity by exact (case-insensitive) text + type within an owner scope.
     * Used as a fast path before falling back to vector dedupe.
     *
     * <p>This query is deliberately shaped to match the {@code entities_unique_idx}
     * functional unique index:
     * <pre>
     *   (lower(text), COALESCE(entity_type, ''),
     *    COALESCE(user_id::text, ''), COALESCE(agent_id, ''), COALESCE(run_id, ''))
     * </pre>
     * Using {@code COALESCE(field, '') = ?} on every column lets PG hit the index even
     * when owner fields are NULL — without this, {@code user_id IS NULL} would force a
     * sequential scan over millions of rows.
     */
    public Optional<Entity> findByExactText(String text, String entityType,
                                            UUID userId, String agentId, String runId) {
        // Match the EXACT shape of entities_unique_idx so the planner can use it for the
        // entire compound key, NULLs included.
        String sql = "SELECT * FROM " + TABLE
                + " WHERE lower(text) = lower(?)"
                + "   AND COALESCE(entity_type, '') = ?"
                + "   AND COALESCE(user_id::text, '') = ?"
                + "   AND COALESCE(agent_id, '') = ?"
                + "   AND COALESCE(run_id, '') = ?"
                + " LIMIT 1";
        List<Entity> rows = jdbcTemplate.query(
                sql,
                rowMapper(false),
                text,
                entityType == null ? "" : entityType,
                userId == null ? "" : userId.toString(),
                agentId == null ? "" : agentId,
                runId == null ? "" : runId
        );
        return rows.isEmpty() ? Optional.empty() : Optional.of(rows.get(0));
    }

    /**
     * Strip every reference to any id in {@code memoryIds} from every entity in the owner
     * scope, in one round-trip.
     *
     * <p>SQL: {@code linked_memory_ids = array(unnest WHERE NOT = ANY(?))} — preserves order
     * and copes with arrays of any size. The {@code linked_memory_ids && ?} prefilter uses the
     * implicit array overlap to avoid touching rows that don't reference any victim.
     *
     * @return rows actually mutated
     */
    public int removeMemoryLinksBulk(List<UUID> memoryIds,
                                     UUID userId, String agentId, String runId) {
        if (memoryIds == null || memoryIds.isEmpty()) {
            return 0;
        }
        StringBuilder sql = new StringBuilder(
                "UPDATE " + TABLE + " SET linked_memory_ids = ("
                        + "  SELECT COALESCE(array_agg(elem), '{}'::uuid[]) "
                        + "  FROM unnest(linked_memory_ids) elem "
                        + "  WHERE elem <> ALL(?::uuid[])"
                        + ") WHERE linked_memory_ids && ?::uuid[]");
        List<Object> args = new ArrayList<>();
        // Two array bindings, then owner filters.
        if (userId != null) {
            sql.append(" AND user_id = ?");
        }
        if (agentId != null) {
            sql.append(" AND agent_id = ?");
        }
        if (runId != null) {
            sql.append(" AND run_id = ?");
        }
        return jdbcTemplate.update(con -> {
            var ps = con.prepareStatement(sql.toString());
            Array arr1 = con.createArrayOf("uuid", memoryIds.toArray(new UUID[0]));
            Array arr2 = con.createArrayOf("uuid", memoryIds.toArray(new UUID[0]));
            int idx = 1;
            ps.setArray(idx++, arr1);
            ps.setArray(idx++, arr2);
            if (userId != null) ps.setObject(idx++, userId);
            if (agentId != null) ps.setString(idx++, agentId);
            if (runId != null) ps.setString(idx, runId);
            return ps;
        });
    }

    /** Fetch a single entity by id (any owner). Caller is responsible for owner enforcement. */
    public Optional<Entity> findById(UUID id) {
        List<Entity> rows = jdbcTemplate.query(
                "SELECT * FROM " + TABLE + " WHERE id = ?",
                rowMapper(false),
                id);
        return rows.isEmpty() ? Optional.empty() : Optional.of(rows.get(0));
    }

    /**
     * Paginated list of entities matching the owner triple (any field may be null).
     * Optionally filter by {@code entityType} (e.g. {@code "person"}).
     *
     * @param page 1-based page number
     * @param pageSize rows per page
     */
    public List<Entity> listByOwnerPaged(UUID userId, String agentId, String runId,
                                         String entityType,
                                         int page, int pageSize) {
        StringBuilder sql = new StringBuilder("SELECT * FROM " + TABLE + " WHERE 1=1");
        List<Object> args = new ArrayList<>();
        if (userId != null) {
            sql.append(" AND user_id = ?");
            args.add(userId);
        }
        if (agentId != null) {
            sql.append(" AND agent_id = ?");
            args.add(agentId);
        }
        if (runId != null) {
            sql.append(" AND run_id = ?");
            args.add(runId);
        }
        if (entityType != null && !entityType.isBlank()) {
            sql.append(" AND entity_type = ?");
            args.add(entityType);
        }
        sql.append(" ORDER BY created_at DESC LIMIT ? OFFSET ?");
        args.add(pageSize);
        args.add(Math.max(0, (page - 1) * pageSize));
        return jdbcTemplate.query(sql.toString(), rowMapper(false), args.toArray());
    }

    /** Count variant of {@link #listByOwnerPaged} with the same filter contract. */
    public long countByOwnerAndType(UUID userId, String agentId, String runId, String entityType) {
        StringBuilder sql = new StringBuilder("SELECT COUNT(*) FROM " + TABLE + " WHERE 1=1");
        List<Object> args = new ArrayList<>();
        if (userId != null) { sql.append(" AND user_id = ?"); args.add(userId); }
        if (agentId != null) { sql.append(" AND agent_id = ?"); args.add(agentId); }
        if (runId != null) { sql.append(" AND run_id = ?"); args.add(runId); }
        if (entityType != null && !entityType.isBlank()) {
            sql.append(" AND entity_type = ?");
            args.add(entityType);
        }
        Long c = jdbcTemplate.queryForObject(sql.toString(), Long.class, args.toArray());
        return c == null ? 0L : c;
    }

    /**
     * Find entities that mention {@code memoryId} in their {@code linked_memory_ids} array.
     * Used by the memory detail page's "Linked Entities" tab.
     *
     * <p>{@code ownerUserId} non-null enforces an owner check at SQL level (defense in depth).
     */
    public List<Entity> findByLinkedMemoryId(UUID memoryId, UUID ownerUserId) {
        StringBuilder sql = new StringBuilder(
                "SELECT * FROM " + TABLE + " WHERE ? = ANY(linked_memory_ids)");
        List<Object> args = new ArrayList<>();
        args.add(memoryId);
        if (ownerUserId != null) {
            sql.append(" AND user_id = ?");
            args.add(ownerUserId);
        }
        sql.append(" ORDER BY created_at DESC");
        return jdbcTemplate.query(sql.toString(), rowMapper(false), args.toArray());
    }

    /** Hard-delete an entity by id. Owner-scoped — pass {@code null} for admin/unrestricted. */
    public int deleteByIdForOwner(UUID id, UUID ownerUserId) {
        if (ownerUserId == null) {
            return jdbcTemplate.update("DELETE FROM " + TABLE + " WHERE id = ?", id);
        }
        return jdbcTemplate.update(
                "DELETE FROM " + TABLE + " WHERE id = ? AND user_id = ?",
                id, ownerUserId);
    }

    /**
     * Count entities in an owner scope. Owners can be partial — passing null on a field
     * means "don't filter by it".
     */
    public long countByOwner(UUID userId, String agentId, String runId) {
        StringBuilder sql = new StringBuilder("SELECT COUNT(*) FROM " + TABLE + " WHERE 1=1");
        List<Object> args = new ArrayList<>();
        if (userId != null) {
            sql.append(" AND user_id = ?");
            args.add(userId);
        }
        if (agentId != null) {
            sql.append(" AND agent_id = ?");
            args.add(agentId);
        }
        if (runId != null) {
            sql.append(" AND run_id = ?");
            args.add(runId);
        }
        Long c = jdbcTemplate.queryForObject(sql.toString(), Long.class, args.toArray());
        return c == null ? 0L : c;
    }

    /**
     * Drop every row. Destructive — used only by {@code reset()}.
     *
     * <p>If the {@code mem.entities} table doesn't yet exist on this tenant (pre-Batch-B
     * tenant that hasn't been migrated), silently no-op rather than failing the whole reset.
     */
    public void truncate() {
        try {
            jdbcTemplate.execute("TRUNCATE TABLE " + TABLE);
        } catch (org.springframework.jdbc.BadSqlGrammarException e) {
            log.warn("mem.entities not present on this tenant — skipping truncate ({})",
                    e.getMostSpecificCause().getMessage());
        }
    }

    // ---------- helpers ----------

    private RowMapper<Entity> rowMapper(boolean withDistance) {
        return (rs, rowNum) -> {
            Entity e = Entity.builder()
                    .id((UUID) rs.getObject("id"))
                    .userId((UUID) rs.getObject("user_id"))
                    .agentId(rs.getString("agent_id"))
                    .runId(rs.getString("run_id"))
                    .text(rs.getString("text"))
                    .entityType(rs.getString("entity_type"))
                    .embedding(readVector(rs.getObject("embedding")))
                    .linkedMemoryIds(readUuidArray(rs.getArray("linked_memory_ids")))
                    .build();
            Timestamp createdAt = rs.getTimestamp("created_at");
            if (createdAt != null) e.setCreatedAt(createdAt.toInstant());
            Timestamp updatedAt = rs.getTimestamp("updated_at");
            if (updatedAt != null) e.setUpdatedAt(updatedAt.toInstant());
            if (withDistance) {
                try {
                    e.setScore(rs.getDouble("distance"));
                } catch (Exception ignore) {
                    // distance column not present
                }
            }
            return e;
        };
    }

    private static float[] readVector(Object value) {
        if (value == null) return null;
        if (value instanceof PGobject pg) return parseVector(pg.getValue());
        return parseVector(value.toString());
    }

    private static List<UUID> readUuidArray(Array array) throws SQLException {
        if (array == null) return Collections.emptyList();
        Object[] raw = (Object[]) array.getArray();
        List<UUID> out = new ArrayList<>(raw.length);
        for (Object o : raw) {
            if (o == null) continue;
            if (o instanceof UUID u) {
                out.add(u);
            } else {
                out.add(UUID.fromString(o.toString()));
            }
        }
        return out;
    }
}
