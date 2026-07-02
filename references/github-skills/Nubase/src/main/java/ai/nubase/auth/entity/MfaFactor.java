package ai.nubase.auth.entity;

import jakarta.persistence.*;
import lombok.*;

import java.time.Instant;
import java.util.UUID;

/**
 * An enrolled MFA factor (TOTP authenticator app or phone/SMS).
 * Mirrors Supabase GoTrue's {@code auth.mfa_factors}.
 */
@Entity
@Table(name = "mfa_factors", schema = "auth")
@Getter
@Setter
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class MfaFactor {

    public static final String TYPE_TOTP = "totp";
    public static final String TYPE_PHONE = "phone";
    public static final String STATUS_UNVERIFIED = "unverified";
    public static final String STATUS_VERIFIED = "verified";

    @Id
    @GeneratedValue(strategy = GenerationType.AUTO)
    private UUID id;

    @ManyToOne(fetch = FetchType.LAZY)
    @JoinColumn(name = "user_id", nullable = false)
    private User user;

    @Column(name = "friendly_name")
    private String friendlyName;

    @Column(name = "factor_type", nullable = false)
    private String factorType;

    @Column(name = "status", nullable = false)
    private String status;

    @Column(name = "secret")
    private String secret;

    @Column(name = "phone")
    private String phone;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at")
    private Instant updatedAt;

    @Column(name = "last_challenged_at")
    private Instant lastChallengedAt;

    @PrePersist
    protected void onCreate() {
        createdAt = Instant.now();
        updatedAt = Instant.now();
        if (status == null) {
            status = STATUS_UNVERIFIED;
        }
    }

    @PreUpdate
    protected void onUpdate() {
        updatedAt = Instant.now();
    }
}
