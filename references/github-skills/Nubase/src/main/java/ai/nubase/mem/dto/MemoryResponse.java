package ai.nubase.mem.dto;

import ai.nubase.mem.entity.Memory;
import com.fasterxml.jackson.annotation.JsonInclude;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;
import java.util.Map;
import java.util.UUID;

/**
 * Lightweight projection of {@link Memory} for API responses.
 * Excludes the raw embedding vector for payload size.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class MemoryResponse {

    private UUID id;

    private UUID userId;
    private String agentId;
    private String runId;

    private String memory;
    private Map<String, Object> metadata;

    private String actorId;
    private String role;

    private Instant createdAt;
    private Instant updatedAt;

    /** Cosine distance to search query (only populated by /search). */
    private Double score;

    public static MemoryResponse from(Memory m) {
        return MemoryResponse.builder()
                .id(m.getId())
                .userId(m.getUserId())
                .agentId(m.getAgentId())
                .runId(m.getRunId())
                .memory(m.getMemory())
                .metadata(m.getMetadata())
                .actorId(m.getActorId())
                .role(m.getRole())
                .createdAt(m.getCreatedAt())
                .updatedAt(m.getUpdatedAt())
                .score(m.getScore())
                .build();
    }
}
