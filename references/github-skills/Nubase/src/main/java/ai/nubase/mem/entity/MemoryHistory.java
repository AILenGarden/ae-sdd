package ai.nubase.mem.entity;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;
import java.util.UUID;

/**
 * Row in {@code mem.memory_history}.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class MemoryHistory {

    public enum Event {
        ADD,
        UPDATE,
        DELETE
    }

    private UUID id;
    private UUID memoryId;
    private String oldValue;
    private String newValue;
    private String event;
    private String actorId;
    private Instant createdAt;
}
