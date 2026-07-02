package ai.nubase.auth.entity;

import jakarta.persistence.*;
import lombok.*;
import org.hibernate.annotations.JdbcTypeCode;
import org.hibernate.type.SqlTypes;

import java.time.Instant;
import java.util.UUID;

/**
 * Storage Bucket Entity
 * Represents a storage bucket in Supabase Storage
 */
@Entity
@Table(name = "buckets",schema = "storage")
@Getter
@Setter
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class Bucket {

    @Id
    @Column(name = "id", length = 255)
    private String id;

    @Column(name = "name", nullable = false, unique = true)
    private String name;

    @Column(name = "owner")
    private UUID owner;

    @Column(name = "public", nullable = false)
    private Boolean isPublic = false;

    @Column(name = "avif_autodetection")
    private Boolean avifAutodetection = false;

    @Column(name = "file_size_limit")
    private Long fileSizeLimit;

    @JdbcTypeCode(SqlTypes.ARRAY)
    @Column(name = "allowed_mime_types", columnDefinition = "text[]")
    private String[] allowedMimeTypes;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at")
    private Instant updatedAt;

    @PrePersist
    protected void onCreate() {
        if (createdAt == null) {
            createdAt = Instant.now();
        }
        if (updatedAt == null) {
            updatedAt = Instant.now();
        }
        if (isPublic == null) {
            isPublic = false;
        }
        if (avifAutodetection == null) {
            avifAutodetection = false;
        }
    }

    @PreUpdate
    protected void onUpdate() {
        updatedAt = Instant.now();
    }
}
