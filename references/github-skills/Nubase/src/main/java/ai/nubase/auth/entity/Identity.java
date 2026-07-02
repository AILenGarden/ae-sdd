package ai.nubase.auth.entity;

import io.hypersistence.utils.hibernate.type.json.JsonBinaryType;
import jakarta.persistence.*;
import lombok.*;
import org.hibernate.annotations.Type;

import java.time.Instant;
import java.util.Map;
import java.util.UUID;

@Entity
@Table(name = "identities", schema = "auth",
        uniqueConstraints = @UniqueConstraint(columnNames = {"provider_id", "provider"}))
@Getter
@Setter
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class Identity {

    @Id
    @GeneratedValue(strategy = GenerationType.AUTO)
    private UUID id;

    @Column(name = "provider_id", nullable = false)
    private String providerId;

    @ManyToOne(fetch = FetchType.LAZY)
    @JoinColumn(name = "user_id", nullable = false)
    private User user;

    @Type(JsonBinaryType.class)
    @Column(name = "identity_data", columnDefinition = "jsonb", nullable = false)
    private Map<String, Object> identityData;

    @Column(name = "provider", nullable = false)
    private String provider;

    @Column(name = "last_sign_in_at")
    private Instant lastSignInAt;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at")
    private Instant updatedAt;

    // Email is a generated column in database, so we don't map it here
    // It's automatically generated from identity_data->>'email'

    @PrePersist
    protected void onCreate() {
        createdAt = Instant.now();
        updatedAt = Instant.now();
    }

    @PreUpdate
    protected void onUpdate() {
        updatedAt = Instant.now();
    }
}
