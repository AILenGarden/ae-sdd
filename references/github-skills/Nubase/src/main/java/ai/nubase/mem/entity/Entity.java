package ai.nubase.mem.entity;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;
import java.util.List;
import java.util.UUID;

/**
 * Row in {@code mem.entities}.
 *
 * <p>Stores a distinct named entity per owner triple with an array of linked memory ids.
 * Used at search time to compute entity-based boosts for retrieval scoring.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class Entity {

    private UUID id;

    private UUID userId;
    private String agentId;
    private String runId;

    private String text;
    private String entityType;
    private float[] embedding;

    private List<UUID> linkedMemoryIds;

    private Instant createdAt;
    private Instant updatedAt;

    /** Search-only: cosine distance to query entity. Not stored. */
    private Double score;
}
