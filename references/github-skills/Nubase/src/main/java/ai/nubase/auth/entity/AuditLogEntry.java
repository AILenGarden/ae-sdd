package ai.nubase.auth.entity;

import io.hypersistence.utils.hibernate.type.json.JsonBinaryType;
import jakarta.persistence.*;
import lombok.*;
import org.hibernate.annotations.Type;

import java.time.Instant;
import java.util.Map;
import java.util.UUID;

/**
 * Auth audit trail entry. Mirrors Supabase GoTrue's {@code auth.audit_log_entries}.
 */
@Entity
@Table(name = "audit_log_entries", schema = "auth")
@Getter
@Setter
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class AuditLogEntry {

    @Id
    @GeneratedValue(strategy = GenerationType.AUTO)
    private UUID id;

    @Column(name = "instance_id")
    private UUID instanceId;

    @Type(JsonBinaryType.class)
    @Column(name = "payload", columnDefinition = "jsonb")
    private Map<String, Object> payload;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "ip_address")
    private String ipAddress;

    @PrePersist
    protected void onCreate() {
        createdAt = Instant.now();
    }
}
