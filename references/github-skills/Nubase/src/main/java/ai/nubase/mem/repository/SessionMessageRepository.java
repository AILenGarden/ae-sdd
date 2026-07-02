package ai.nubase.mem.repository;

import ai.nubase.mem.entity.SessionMessage;
import lombok.RequiredArgsConstructor;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.jdbc.core.RowMapper;
import org.springframework.stereotype.Repository;

import java.sql.Timestamp;
import java.time.Instant;
import java.util.Collections;
import java.util.List;
import java.util.UUID;

/**
 * JdbcTemplate repository for {@code mem.session_messages}.
 *
 * <p>Implements a rolling window: each insert evicts rows beyond the most-recent N within
 * the same {@code session_scope}.
 */
@Repository
@RequiredArgsConstructor
public class SessionMessageRepository {

    private final JdbcTemplate jdbcTemplate;

    private static final RowMapper<SessionMessage> ROW_MAPPER = (rs, rowNum) -> {
        SessionMessage m = SessionMessage.builder()
                .id((UUID) rs.getObject("id"))
                .sessionScope(rs.getString("session_scope"))
                .userId((UUID) rs.getObject("user_id"))
                .role(rs.getString("role"))
                .content(rs.getString("content"))
                .name(rs.getString("name"))
                .build();
        Timestamp ts = rs.getTimestamp("created_at");
        if (ts != null) m.setCreatedAt(ts.toInstant());
        return m;
    };

    public UUID insert(SessionMessage msg) {
        UUID id = msg.getId() != null ? msg.getId() : UUID.randomUUID();
        jdbcTemplate.update(
                "INSERT INTO mem.session_messages "
                        + "(id, session_scope, user_id, role, content, name, created_at) "
                        + "VALUES (?, ?, ?, ?, ?, ?, ?)",
                id,
                msg.getSessionScope(),
                msg.getUserId(),
                msg.getRole(),
                msg.getContent(),
                msg.getName(),
                Timestamp.from(Instant.now())
        );
        return id;
    }

    /**
     * Insert a message and evict any rows in {@code session_scope} beyond the most-recent
     * {@code maxMessages}.
     */
    public UUID insertWithEviction(SessionMessage msg, int maxMessages) {
        UUID id = insert(msg);
        jdbcTemplate.update(
                "DELETE FROM mem.session_messages WHERE session_scope = ? AND id NOT IN ("
                        + " SELECT id FROM mem.session_messages WHERE session_scope = ? "
                        + " ORDER BY created_at DESC LIMIT ?"
                        + ")",
                msg.getSessionScope(), msg.getSessionScope(), maxMessages
        );
        return id;
    }

    /** Drop every row. Destructive — used only by {@code reset()}. */
    public void truncate() {
        jdbcTemplate.execute("TRUNCATE TABLE mem.session_messages");
    }

    public List<SessionMessage> findRecent(String sessionScope, int limit) {
        if (sessionScope == null || sessionScope.isBlank()) {
            return Collections.emptyList();
        }
        return jdbcTemplate.query(
                "SELECT * FROM ("
                        + "  SELECT * FROM mem.session_messages "
                        + "  WHERE session_scope = ? "
                        + "  ORDER BY created_at DESC LIMIT ? "
                        + ") sub ORDER BY created_at ASC",
                ROW_MAPPER,
                sessionScope,
                limit
        );
    }
}
