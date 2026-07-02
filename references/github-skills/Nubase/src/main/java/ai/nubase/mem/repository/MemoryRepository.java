package ai.nubase.mem.repository;

import ai.nubase.mem.config.MemProperties;
import ai.nubase.mem.entity.Memory;
import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.postgresql.util.PGobject;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.jdbc.core.RowMapper;
import org.springframework.stereotype.Repository;

import java.util.Set;

import java.sql.Timestamp;
import java.sql.Types;
import java.time.Instant;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

/**
 * JdbcTemplate-based repository for {@code mem.memories}.
 *
 * <p>Bypasses JPA because the {@code embedding} column uses the pgvector type, which is
 * cleanest to serialize as a {@code String} literal of the form {@code "[0.1,0.2,...]"}.
 */
@Slf4j
@Repository
@RequiredArgsConstructor
public class MemoryRepository {

    private final JdbcTemplate jdbcTemplate;
    private final ObjectMapper objectMapper;
    private final MemProperties memProperties;

    private static final String TABLE = "mem.memories";

    /**
     * Whitelist of PostgreSQL text-search configurations that may be interpolated into the
     * BM25 SQL string. Anything outside this set falls back to {@code 'simple'} — this is
     * the only thing keeping a misconfigured {@code nubase.mem.search.ftsConfig} from
     * becoming a SQL injection vector.
     */
    private static final Set<String> ALLOWED_FTS_CONFIGS = Set.of(
            "simple", "english", "spanish", "french", "german", "italian",
            "portuguese", "russian", "dutch", "norwegian", "swedish", "danish",
            "finnish", "hungarian", "turkish", "chinese", "zhparser", "jieba"
    );

    /**
     * Insert a new memory.
     *
     * @return generated memory id
     */
    public UUID insert(Memory mem) {
        UUID id = mem.getId() != null ? mem.getId() : UUID.randomUUID();
        String sql = "INSERT INTO " + TABLE + " ("
                + "id, user_id, agent_id, run_id, memory, embedding, metadata, "
                + "actor_id, role, hash, created_at, updated_at) "
                + "VALUES (?, ?, ?, ?, ?, ?::vector, ?::jsonb, ?, ?, ?, ?, ?)";
        Instant now = Instant.now();
        jdbcTemplate.update(
                sql,
                id,
                mem.getUserId(),
                mem.getAgentId(),
                mem.getRunId(),
                mem.getMemory(),
                serializeVector(mem.getEmbedding()),
                serializeJson(mem.getMetadata()),
                mem.getActorId(),
                mem.getRole(),
                mem.getHash(),
                Timestamp.from(now),
                Timestamp.from(now)
        );
        return id;
    }

    /** Soft-delete by setting {@code deleted_at}. */
    public int softDelete(UUID id) {
        return jdbcTemplate.update(
                "UPDATE " + TABLE + " SET deleted_at = NOW() WHERE id = ? AND deleted_at IS NULL",
                id
        );
    }

    /** Update memory text and embedding (used when the LLM decides UPDATE). */
    public int updateContent(UUID id, String newMemory, float[] newEmbedding, String newHash) {
        return jdbcTemplate.update(
                "UPDATE " + TABLE + " SET memory = ?, embedding = ?::vector, hash = ? WHERE id = ?",
                newMemory,
                serializeVector(newEmbedding),
                newHash,
                id
        );
    }

    /**
     * Fetch a memory by id with no owner check.
     *
     * <p><b>Internal use only.</b> Any code path reachable from a user request must use
     * {@link #findByIdForOwner(UUID, UUID)} instead — this overload trusts the caller has
     * already authorized the lookup. Currently used only by mem-service internal flows
     * (applyUpdate/applyDelete) where the owner check was performed up-thread via
     * {@code MemoryAuthScope}.
     */
    public Optional<Memory> findById(UUID id) {
        List<Memory> rows = jdbcTemplate.query(
                "SELECT * FROM " + TABLE + " WHERE id = ? AND deleted_at IS NULL",
                memoryRowMapper(false),
                id
        );
        return rows.isEmpty() ? Optional.empty() : Optional.of(rows.get(0));
    }

    /**
     * Fetch a memory by id, returning empty if the row exists but is owned by a different
     * user. {@code ownerUserId == null} means "no restriction" — used by service-role
     * callers via {@code MemoryAuthScope.isUnrestricted()}.
     *
     * <p>Defense-in-depth complement to the service-layer scope check: even if a future
     * caller forgets to wrap, the SQL itself refuses cross-owner reads.
     */
    public Optional<Memory> findByIdForOwner(UUID id, UUID ownerUserId) {
        if (ownerUserId == null) {
            return findById(id);
        }
        List<Memory> rows = jdbcTemplate.query(
                "SELECT * FROM " + TABLE
                        + " WHERE id = ? AND deleted_at IS NULL AND user_id = ?",
                memoryRowMapper(false),
                id, ownerUserId
        );
        return rows.isEmpty() ? Optional.empty() : Optional.of(rows.get(0));
    }

    /**
     * List all (non-deleted) memories for an owner triple.
     *
     * <p>At least one of {@code userId} / {@code agentId} / {@code runId} must be non-null;
     * filters are AND-combined.
     */
    public List<Memory> listByOwner(UUID userId, String agentId, String runId, int limit) {
        return listByOwner(userId, agentId, runId, null, limit);
    }

    /**
     * List variant accepting an optional metadata filter (parsed by
     * {@link MetadataFilterParser}). Passing {@code null} or an empty filter behaves like
     * the no-filter overload.
     */
    public List<Memory> listByOwner(UUID userId, String agentId, String runId,
                                    Map<String, Object> metadataFilters,
                                    int limit) {
        StringBuilder sql = new StringBuilder(
                "SELECT * FROM " + TABLE + " WHERE deleted_at IS NULL");
        List<Object> args = new ArrayList<>();
        appendOwnerClauses(sql, args, userId, agentId, runId);
        appendMetadataClauses(sql, args, metadataFilters);
        sql.append(" ORDER BY created_at DESC LIMIT ?");
        args.add(limit);
        return jdbcTemplate.query(sql.toString(), memoryRowMapper(false), args.toArray());
    }

    /**
     * Paginated list variant — same filter semantics as {@link #listByOwner(UUID, String, String, Map, int)}
     * but with LIMIT + OFFSET, intended for the admin UI's table view.
     *
     * @param page 1-based page number
     * @param pageSize rows per page
     */
    public List<Memory> listByOwnerPaged(UUID userId, String agentId, String runId,
                                         Map<String, Object> metadataFilters,
                                         int page, int pageSize) {
        StringBuilder sql = new StringBuilder(
                "SELECT * FROM " + TABLE + " WHERE deleted_at IS NULL");
        List<Object> args = new ArrayList<>();
        appendOwnerClauses(sql, args, userId, agentId, runId);
        appendMetadataClauses(sql, args, metadataFilters);
        sql.append(" ORDER BY created_at DESC LIMIT ? OFFSET ?");
        args.add(pageSize);
        args.add(Math.max(0, (page - 1) * pageSize));
        return jdbcTemplate.query(sql.toString(), memoryRowMapper(false), args.toArray());
    }

    /**
     * Count non-deleted memories matching the same filter the list methods use. Returned
     * as a {@code long} since the row count can exceed {@code Integer.MAX_VALUE} for
     * admin views over a large tenant.
     */
    public long countByOwner(UUID userId, String agentId, String runId,
                             Map<String, Object> metadataFilters) {
        StringBuilder sql = new StringBuilder(
                "SELECT COUNT(*) FROM " + TABLE + " WHERE deleted_at IS NULL");
        List<Object> args = new ArrayList<>();
        appendOwnerClauses(sql, args, userId, agentId, runId);
        appendMetadataClauses(sql, args, metadataFilters);
        Long c = jdbcTemplate.queryForObject(sql.toString(), Long.class, args.toArray());
        return c == null ? 0L : c;
    }

    /**
     * Cosine-distance vector search. Filters by owner triple and (optionally) a max distance.
     *
     * @param embedding query embedding
     * @param userId optional user filter
     * @param agentId optional agent filter
     * @param runId optional run filter
     * @param topK max rows returned
     * @param maxDistance results with cosine distance &gt; this are excluded (range 0..2)
     */
    public List<Memory> searchByVector(float[] embedding,
                                       UUID userId,
                                       String agentId,
                                       String runId,
                                       int topK,
                                       Double maxDistance) {
        return searchByVector(embedding, userId, agentId, runId, null, topK, maxDistance);
    }

    /**
     * Vector search with an optional metadata filter.
     */
    public List<Memory> searchByVector(float[] embedding,
                                       UUID userId,
                                       String agentId,
                                       String runId,
                                       Map<String, Object> metadataFilters,
                                       int topK,
                                       Double maxDistance) {
        StringBuilder sql = new StringBuilder(
                "SELECT *, (embedding <=> ?::vector) AS distance FROM " + TABLE
                        + " WHERE deleted_at IS NULL AND embedding IS NOT NULL");
        List<Object> args = new ArrayList<>();
        args.add(serializeVector(embedding));
        appendOwnerClauses(sql, args, userId, agentId, runId);
        appendMetadataClauses(sql, args, metadataFilters);
        if (maxDistance != null) {
            sql.append(" AND (embedding <=> ?::vector) <= ?");
            args.add(serializeVector(embedding));
            args.add(maxDistance);
        }
        sql.append(" ORDER BY embedding <=> ?::vector LIMIT ?");
        args.add(serializeVector(embedding));
        args.add(topK);

        return jdbcTemplate.query(sql.toString(), memoryRowMapper(true), args.toArray());
    }

    /**
     * BM25-style full-text search over the {@code memory} column.
     *
     * <p>Uses {@code to_tsvector('simple', memory) @@ plainto_tsquery('simple', ?)} against the
     * existing {@code memories_memory_fts_idx} GIN index, ranking with {@code ts_rank_cd}.
     * Filters by the owner triple (AND-combined).
     *
     * <p>Each returned {@link Memory} carries the raw rank in {@link Memory#getScore()};
     * normalization to {@code [0, 1]} happens in {@code ScoreFusion}.
     */
    public List<Memory> searchByText(String query,
                                     UUID userId,
                                     String agentId,
                                     String runId,
                                     int topK) {
        return searchByText(query, userId, agentId, runId, null, topK);
    }

    /**
     * BM25 search with an optional metadata filter.
     */
    public List<Memory> searchByText(String query,
                                     UUID userId,
                                     String agentId,
                                     String runId,
                                     Map<String, Object> metadataFilters,
                                     int topK) {
        if (query == null || query.isBlank()) {
            return List.of();
        }
        String fts = resolveFtsConfig();
        StringBuilder sql = new StringBuilder(
                "SELECT *, ts_rank_cd(to_tsvector('" + fts + "', memory), "
                        + "plainto_tsquery('" + fts + "', ?)) AS rank "
                        + "FROM " + TABLE
                        + " WHERE deleted_at IS NULL "
                        + " AND to_tsvector('" + fts + "', memory) "
                        + "@@ plainto_tsquery('" + fts + "', ?)");
        List<Object> args = new ArrayList<>();
        args.add(query);
        args.add(query);
        appendOwnerClauses(sql, args, userId, agentId, runId);
        appendMetadataClauses(sql, args, metadataFilters);
        sql.append(" ORDER BY rank DESC LIMIT ?");
        args.add(topK);

        return jdbcTemplate.query(sql.toString(), memoryRowMapper(false, "rank"), args.toArray());
    }

    /**
     * Resolve and validate the configured PostgreSQL text-search config name.
     * Strict whitelist: anything not in {@link #ALLOWED_FTS_CONFIGS} falls back to {@code 'simple'}.
     * This guarantees the value can be safely interpolated into the BM25 SQL (the config name
     * cannot be a prepared-statement parameter — it's a syntactic position, not a value).
     */
    String resolveFtsConfig() {
        String configured = memProperties != null && memProperties.getSearch() != null
                ? memProperties.getSearch().getFtsConfig()
                : null;
        if (configured == null || !ALLOWED_FTS_CONFIGS.contains(configured.toLowerCase())) {
            if (configured != null) {
                log.warn("Unknown ftsConfig '{}' — falling back to 'simple'. Allowed: {}",
                        configured, ALLOWED_FTS_CONFIGS);
            }
            return "simple";
        }
        return configured.toLowerCase();
    }

    /**
     * Top {@code limit} {@code user_id} values by non-deleted memory count, descending.
     *
     * <p>Intended for service-role stats dashboards — non-admin callers can still invoke it,
     * but their result will be at most one row (their own JWT sub).
     */
    public List<java.util.Map<String, Object>> topUsersByMemoryCount(int limit) {
        return jdbcTemplate.query(
                "SELECT user_id, COUNT(*) AS cnt FROM " + TABLE
                        + " WHERE deleted_at IS NULL AND user_id IS NOT NULL "
                        + "GROUP BY user_id ORDER BY cnt DESC LIMIT ?",
                (rs, rowNum) -> {
                    java.util.Map<String, Object> row = new java.util.HashMap<>();
                    row.put("user_id", rs.getObject("user_id"));
                    row.put("count", rs.getLong("cnt"));
                    return row;
                },
                limit);
    }

    /**
     * Return the ids of every non-deleted memory matching the owner triple.
     * Used by {@code deleteAll} to drive entity-link cleanup before the soft-delete UPDATE.
     */
    public List<UUID> selectIdsByOwner(UUID userId, String agentId, String runId) {
        StringBuilder sql = new StringBuilder(
                "SELECT id FROM " + TABLE + " WHERE deleted_at IS NULL");
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
        return jdbcTemplate.query(sql.toString(),
                (rs, rowNum) -> (UUID) rs.getObject("id"),
                args.toArray());
    }

    /**
     * Soft-delete every non-deleted memory matching the owner triple.
     *
     * <p>At least one of {@code userId}/{@code agentId}/{@code runId} must be non-null;
     * filters are AND-combined.
     *
     * @return number of rows affected
     */
    public int softDeleteByOwner(UUID userId, String agentId, String runId) {
        if (userId == null && agentId == null && runId == null) {
            throw new IllegalArgumentException(
                    "At least one owner id must be provided for batch delete");
        }
        StringBuilder sql = new StringBuilder(
                "UPDATE " + TABLE + " SET deleted_at = NOW() WHERE deleted_at IS NULL");
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
        return jdbcTemplate.update(sql.toString(), args.toArray());
    }

    /**
     * Wipe the memories table entirely. Used by {@code reset()}.
     *
     * <p>This is destructive and bypasses soft-delete — only call from service-role admin paths.
     */
    public void truncate() {
        jdbcTemplate.execute("TRUNCATE TABLE " + TABLE);
    }

    /** Find an existing non-deleted memory with the same hash (for dedupe). */
    public Optional<Memory> findByHash(UUID userId, String agentId, String runId, String hash) {
        if (hash == null) {
            return Optional.empty();
        }
        StringBuilder sql = new StringBuilder(
                "SELECT * FROM " + TABLE + " WHERE deleted_at IS NULL AND hash = ?");
        List<Object> args = new ArrayList<>();
        args.add(hash);
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
        sql.append(" LIMIT 1");
        List<Memory> rows = jdbcTemplate.query(sql.toString(), memoryRowMapper(false), args.toArray());
        return rows.isEmpty() ? Optional.empty() : Optional.of(rows.get(0));
    }

    // ---------- helpers ----------

    private static void appendOwnerClauses(StringBuilder sql, List<Object> args,
                                           UUID userId, String agentId, String runId) {
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
    }

    private static void appendMetadataClauses(StringBuilder sql, List<Object> args,
                                              Map<String, Object> metadataFilters) {
        if (metadataFilters == null || metadataFilters.isEmpty()) {
            return;
        }
        MetadataFilterParser.Compiled c = MetadataFilterParser.compile(metadataFilters);
        if (!c.isEmpty()) {
            sql.append(" AND ").append(c.getSql());
            args.addAll(c.getArgs());
        }
    }

    public static String serializeVector(float[] v) {
        if (v == null) {
            return null;
        }
        StringBuilder sb = new StringBuilder(v.length * 8 + 2);
        sb.append('[');
        for (int i = 0; i < v.length; i++) {
            if (i > 0) sb.append(',');
            sb.append(v[i]);
        }
        sb.append(']');
        return sb.toString();
    }

    public static float[] parseVector(String s) {
        if (s == null || s.length() < 2) {
            return null;
        }
        String inner = s.substring(1, s.length() - 1);
        if (inner.isEmpty()) {
            return new float[0];
        }
        String[] parts = inner.split(",");
        float[] out = new float[parts.length];
        for (int i = 0; i < parts.length; i++) {
            out[i] = Float.parseFloat(parts[i].trim());
        }
        return out;
    }

    private String serializeJson(Map<String, Object> map) {
        if (map == null || map.isEmpty()) {
            return "{}";
        }
        try {
            return objectMapper.writeValueAsString(map);
        } catch (Exception e) {
            log.warn("Failed to serialize metadata, defaulting to empty: {}", e.getMessage());
            return "{}";
        }
    }

    private Map<String, Object> parseJson(String s) {
        if (s == null || s.isBlank()) {
            return new HashMap<>();
        }
        try {
            return objectMapper.readValue(s, new TypeReference<Map<String, Object>>() {});
        } catch (Exception e) {
            log.warn("Failed to parse metadata JSON, defaulting to empty: {}", e.getMessage());
            return new HashMap<>();
        }
    }

    private RowMapper<Memory> memoryRowMapper(boolean withDistance) {
        return memoryRowMapper(withDistance, withDistance ? "distance" : null);
    }

    private RowMapper<Memory> memoryRowMapper(boolean withScore, String scoreColumn) {
        return (rs, rowNum) -> {
            Memory m = Memory.builder()
                    .id((UUID) rs.getObject("id"))
                    .userId((UUID) rs.getObject("user_id"))
                    .agentId(rs.getString("agent_id"))
                    .runId(rs.getString("run_id"))
                    .memory(rs.getString("memory"))
                    .embedding(readVector(rs.getObject("embedding")))
                    .metadata(parseJson(stringFromObject(rs.getObject("metadata"))))
                    .actorId(rs.getString("actor_id"))
                    .role(rs.getString("role"))
                    .hash(rs.getString("hash"))
                    .build();
            Timestamp createdAt = rs.getTimestamp("created_at");
            if (createdAt != null) m.setCreatedAt(createdAt.toInstant());
            Timestamp updatedAt = rs.getTimestamp("updated_at");
            if (updatedAt != null) m.setUpdatedAt(updatedAt.toInstant());
            Timestamp deletedAt = rs.getTimestamp("deleted_at");
            if (deletedAt != null) m.setDeletedAt(deletedAt.toInstant());
            if (withScore && scoreColumn != null) {
                try {
                    m.setScore(rs.getDouble(scoreColumn));
                } catch (Exception ignore) {
                    // score column not present
                }
            }
            return m;
        };
    }

    private static float[] readVector(Object value) {
        if (value == null) {
            return null;
        }
        if (value instanceof PGobject pg) {
            return parseVector(pg.getValue());
        }
        return parseVector(value.toString());
    }

    private static String stringFromObject(Object o) {
        if (o == null) return null;
        if (o instanceof PGobject pg) return pg.getValue();
        return o.toString();
    }
}
