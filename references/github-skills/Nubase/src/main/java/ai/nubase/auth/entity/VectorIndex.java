package ai.nubase.auth.entity;

import jakarta.persistence.*;
import lombok.*;
import org.hibernate.annotations.JdbcTypeCode;
import org.hibernate.type.SqlTypes;

import java.time.Instant;
import java.util.Map;

/**
 * Vector Index Entity.
 * Maps to the storage.vector_indexes table.
 */
@Entity
@Table(name = "vector_indexes", schema = "storage",
        uniqueConstraints = @UniqueConstraint(columnNames = {"name", "bucket_id"}))
@Getter
@Setter
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class VectorIndex {

    @Id
    @Column(name = "id")
    private String id;

    @Column(name = "name", nullable = false)
    private String name;

    @Column(name = "bucket_id", nullable = false)
    private String bucketId;

    @Column(name = "data_type", nullable = false)
    private String dataType;

    @Column(name = "dimension", nullable = false)
    private Integer dimension;

    @Column(name = "distance_metric", nullable = false)
    private String distanceMetric;

    @JdbcTypeCode(SqlTypes.JSON)
    @Column(name = "metadata_configuration", columnDefinition = "jsonb")
    private Map<String, Object> metadataConfiguration;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at")
    private Instant updatedAt;

    @PrePersist
    protected void onCreate() {
        if (id == null || id.isBlank()) {
            id = java.util.UUID.randomUUID().toString();
        }
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
