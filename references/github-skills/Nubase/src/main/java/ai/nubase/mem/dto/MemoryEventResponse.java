package ai.nubase.mem.dto;

import com.fasterxml.jackson.annotation.JsonInclude;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.UUID;

/**
 * Per-memory result emitted by {@code add()} — mirrors the mem0 event shape.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class MemoryEventResponse {

    /** Final memory id (only set for ADD / UPDATE / DELETE). */
    private UUID id;

    /** Final memory text. */
    private String memory;

    /** ADD | UPDATE | DELETE | NONE */
    private String event;

    /** Previous text, set for UPDATE. */
    private String previousMemory;
}
