package ai.nubase.mem.entity;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;
import java.util.Map;
import java.util.UUID;

/**
 * Row in {@code mem.memories}.
 *
 * <p>Plain POJO — not a JPA entity, because the embedding column uses the pgvector
 * type which is awkward to register with Hibernate. Access is via {@code MemoryRepository}
 * (JdbcTemplate).
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class Memory {

    private UUID id;

    private UUID userId;
    private String agentId;
    private String runId;

    private String memory;
    private float[] embedding;
    private Map<String, Object> metadata;

    private String actorId;
    private String role;

    private String hash;

    private Instant createdAt;
    private Instant updatedAt;
    private Instant deletedAt;

    /** Search-only field: cosine distance to the query (lower = more similar). Not stored. */
    private Double score;
}
