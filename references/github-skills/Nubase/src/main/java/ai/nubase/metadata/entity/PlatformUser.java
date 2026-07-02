package ai.nubase.metadata.entity;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.PrePersist;
import jakarta.persistence.PreUpdate;
import jakarta.persistence.Table;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;
import java.util.UUID;

/**
 * Platform-level developer account. One row per Studio user.
 * Stored in the metadata database, not in any tenant database.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@Entity
@Table(name = "platform_users")
public class PlatformUser {

    @Id
    private UUID id;

    @Column(nullable = false, unique = true, length = 255)
    private String email;

    @Column(name = "encrypted_password", nullable = false, length = 255)
    private String encryptedPassword;

    @Column(name = "full_name", length = 255)
    private String fullName;

    /** Platform-level role: 'super_admin' (sees all projects) or 'user' (sees own). */
    @Column(name = "role", nullable = false, length = 32)
    private String role;

    @Column(name = "is_active", nullable = false)
    private Boolean isActive;

    @Column(name = "last_signed_in_at")
    private Instant lastSignedInAt;

    /** When the account's email was confirmed via OTP (or OAuth). Null → not yet verified. */
    @Column(name = "email_verified_at")
    private Instant emailVerifiedAt;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    @PrePersist
    void onCreate() {
        if (id == null) id = UUID.randomUUID();
        Instant now = Instant.now();
        if (createdAt == null) createdAt = now;
        updatedAt = now;
        if (isActive == null) isActive = Boolean.TRUE;
        if (role == null) role = "user";
    }

    @PreUpdate
    void onUpdate() {
        updatedAt = Instant.now();
    }
}
