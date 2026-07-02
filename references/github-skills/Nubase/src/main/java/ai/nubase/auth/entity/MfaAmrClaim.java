package ai.nubase.auth.entity;

import jakarta.persistence.*;
import lombok.*;

import java.time.Instant;
import java.util.UUID;

/**
 * Authentication Methods Reference claim — records which method(s) backed a session.
 * Mirrors Supabase GoTrue's {@code auth.mfa_amr_claims}.
 */
@Entity
@Table(name = "mfa_amr_claims", schema = "auth",
        uniqueConstraints = @UniqueConstraint(columnNames = {"session_id", "authentication_method"}))
@Getter
@Setter
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class MfaAmrClaim {

    public static final String METHOD_PASSWORD = "password";
    public static final String METHOD_OTP = "otp";
    public static final String METHOD_TOTP = "totp";
    public static final String METHOD_OAUTH = "oauth";
    public static final String METHOD_ANONYMOUS = "anonymous";

    @Id
    @GeneratedValue(strategy = GenerationType.AUTO)
    private UUID id;

    @ManyToOne(fetch = FetchType.LAZY)
    @JoinColumn(name = "session_id", nullable = false)
    private Session session;

    @Column(name = "authentication_method", nullable = false)
    private String authenticationMethod;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at")
    private Instant updatedAt;

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
