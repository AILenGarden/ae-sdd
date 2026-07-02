package ai.nubase.mem.entity;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;
import java.util.UUID;

/**
 * Row in {@code mem.session_messages}.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class SessionMessage {

    private UUID id;
    private String sessionScope;
    /**
     * Owning auth user. Drives RLS together with the FK on mem.session_messages.
     * Null for agent/run-only sessions (only reachable via service_role).
     */
    private UUID userId;
    private String role;
    private String content;
    private String name;
    private Instant createdAt;
}
