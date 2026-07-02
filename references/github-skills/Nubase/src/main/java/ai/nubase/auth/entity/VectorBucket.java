package ai.nubase.auth.entity;

import jakarta.persistence.*;
import lombok.*;

import java.time.Instant;

/**
 * Vector Bucket Entity.
 * Maps to the storage.buckets_vectors table.
 */
@Entity
@Table(name = "buckets_vectors", schema = "storage")
@Getter
@Setter
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class VectorBucket {

    @Id
    @Column(name = "id")
    private String id;

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
    }

    @PreUpdate
    protected void onUpdate() {
        updatedAt = Instant.now();
    }
}
