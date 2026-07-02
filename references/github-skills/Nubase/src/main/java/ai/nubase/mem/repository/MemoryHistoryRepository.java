package ai.nubase.mem.repository;

import ai.nubase.mem.entity.MemoryHistory;
import lombok.RequiredArgsConstructor;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.jdbc.core.RowMapper;
import org.springframework.stereotype.Repository;

import java.sql.Timestamp;
import java.time.Instant;
import java.util.List;
import java.util.UUID;

/**
 * JdbcTemplate repository for {@code mem.memory_history}.
 */
@Repository
@RequiredArgsConstructor
public class MemoryHistoryRepository {

    private final JdbcTemplate jdbcTemplate;

    private static final RowMapper<MemoryHistory> ROW_MAPPER = (rs, rowNum) -> {
        MemoryHistory h = MemoryHistory.builder()
                .id((UUID) rs.getObject("id"))
                .memoryId((UUID) rs.getObject("memory_id"))
                .oldValue(rs.getString("old_value"))
                .newValue(rs.getString("new_value"))
                .event(rs.getString("event"))
                .actorId(rs.getString("actor_id"))
                .build();
        Timestamp ts = rs.getTimestamp("created_at");
        if (ts != null) h.setCreatedAt(ts.toInstant());
        return h;
    };

    public UUID insert(MemoryHistory entry) {
        UUID id = entry.getId() != null ? entry.getId() : UUID.randomUUID();
        jdbcTemplate.update(
                "INSERT INTO mem.memory_history (id, memory_id, old_value, new_value, event, actor_id, created_at) "
                        + "VALUES (?, ?, ?, ?, ?, ?, ?)",
                id,
                entry.getMemoryId(),
                entry.getOldValue(),
                entry.getNewValue(),
                entry.getEvent(),
                entry.getActorId(),
                Timestamp.from(Instant.now())
        );
        return id;
    }

    public List<MemoryHistory> findByMemoryId(UUID memoryId) {
        return jdbcTemplate.query(
                "SELECT * FROM mem.memory_history WHERE memory_id = ? ORDER BY created_at ASC",
                ROW_MAPPER,
                memoryId
        );
    }

    /**
     * Count history events of each type within the last {@code sinceMinutes} minutes,
     * optionally scoped to memories owned by {@code ownerUserId} (null = whole tenant).
     *
     * <p>One round-trip via GROUP BY; missing event types are populated as 0 by the caller.
     */
    public java.util.Map<String, Long> countEventsSince(int sinceMinutes, UUID ownerUserId) {
        StringBuilder sql = new StringBuilder(
                "SELECT h.event, COUNT(*) AS cnt FROM mem.memory_history h ");
        java.util.List<Object> args = new java.util.ArrayList<>();
        if (ownerUserId != null) {
            // JOIN so we can filter by the memory's owner. Soft-deleted memories still own
            // their historical events — don't exclude on deleted_at here.
            sql.append("JOIN mem.memories m ON m.id = h.memory_id ");
            sql.append("WHERE h.created_at > NOW() - (? || ' minutes')::interval ");
            sql.append("AND m.user_id = ? ");
            args.add(String.valueOf(sinceMinutes));
            args.add(ownerUserId);
        } else {
            sql.append("WHERE h.created_at > NOW() - (? || ' minutes')::interval ");
            args.add(String.valueOf(sinceMinutes));
        }
        sql.append("GROUP BY h.event");

        java.util.Map<String, Long> out = new java.util.HashMap<>();
        jdbcTemplate.query(sql.toString(), rs -> {
            out.put(rs.getString("event"), rs.getLong("cnt"));
        }, args.toArray());
        return out;
    }

    /** Drop every row. Destructive — used only by {@code reset()}. */
    public void truncate() {
        jdbcTemplate.execute("TRUNCATE TABLE mem.memory_history");
    }
}
