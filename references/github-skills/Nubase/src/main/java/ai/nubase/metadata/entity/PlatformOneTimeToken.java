package ai.nubase.metadata.entity;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.PrePersist;
import jakarta.persistence.Table;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;
import java.util.UUID;

/**
 * One-time numeric code for a platform (Studio) account flow: signup / login / password change.
 * Stored in the metadata database. Only the SHA-256 hash of the code is persisted.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@Entity
@Table(name = "platform_one_time_tokens")
public class PlatformOneTimeToken {

    public static final String PURPOSE_SIGNUP = "signup";
    public static final String PURPOSE_LOGIN = "login";
    public static final String PURPOSE_PASSWORD_CHANGE = "password_change";

    @Id
    private UUID id;

    @Column(nullable = false, length = 255)
    private String email;

    @Column(nullable = false, length = 32)
    private String purpose;

    @Column(name = "token_hash", nullable = false, length = 128)
    private String tokenHash;

    @Column(name = "expires_at", nullable = false)
    private Instant expiresAt;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @PrePersist
    void onCreate() {
        if (id == null) id = UUID.randomUUID();
        if (createdAt == null) createdAt = Instant.now();
    }
}
