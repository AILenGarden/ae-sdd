package ai.nubase.auth.entity;

import io.hypersistence.utils.hibernate.type.json.JsonBinaryType;
import jakarta.persistence.*;
import lombok.*;
import org.hibernate.annotations.Type;

import java.time.Instant;
import java.util.Map;
import java.util.UUID;

/**
 * Storage object entity.
 * Represents a file/object within a bucket.
 */
@Entity
@Table(name = "objects", schema = "storage", indexes = {
        @Index(name = "idx_objects_bucket_id", columnList = "bucket_id"),
        @Index(name = "idx_objects_name", columnList = "name")
})
@Getter
@Setter
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class StorageObject {

    @Id
    @GeneratedValue(strategy = GenerationType.AUTO)
    private UUID id;

    @Column(name = "bucket_id", nullable = false)
    private String bucketId;

    @Column(name = "name", nullable = false)
    private String name;

    @Column(name = "owner")
    private UUID owner;

    @Column(name = "path_tokens", columnDefinition = "text[]", insertable = false, updatable = false)
    private String[] pathTokens;

    @Column(name = "version")
    private String version;

    @Type(JsonBinaryType.class)
    @Column(name = "metadata", columnDefinition = "jsonb")
    private Map<String, Object> metadata;

    @Type(JsonBinaryType.class)
    @Column(name = "user_metadata", columnDefinition = "jsonb")
    private Map<String, Object> userMetadata;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at")
    private Instant updatedAt;

    @Column(name = "last_accessed_at")
    private Instant lastAccessedAt;

    @PrePersist
    protected void onCreate() {
        if (createdAt == null) {
            createdAt = Instant.now();
        }
        if (updatedAt == null) {
            updatedAt = Instant.now();
        }
    }

    @PreUpdate
    protected void onUpdate() {
        updatedAt = Instant.now();
    }
}
