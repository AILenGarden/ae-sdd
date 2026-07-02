package ai.nubase.metadata.entity;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.GeneratedValue;
import jakarta.persistence.GenerationType;
import jakarta.persistence.Id;
import jakarta.persistence.PrePersist;
import jakarta.persistence.Table;
import jakarta.persistence.UniqueConstraint;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;
import java.util.UUID;

/**
 * Maps a platform_users row to a tenant project (database_configs row by db_key).
 * Per (user_id, db_key) is unique. Role is project-level: 'owner' for now.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@Entity
@Table(name = "platform_user_projects",
        uniqueConstraints = @UniqueConstraint(name = "uq_platform_user_projects",
                columnNames = {"user_id", "db_key"}))
public class PlatformUserProject {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    @Column(name = "user_id", nullable = false)
    private UUID userId;

    @Column(name = "db_key", nullable = false, length = 255)
    private String dbKey;

    @Column(name = "role", nullable = false, length = 32)
    private String role;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @PrePersist
    void onCreate() {
        if (createdAt == null) createdAt = Instant.now();
        if (role == null) role = "owner";
    }
}
