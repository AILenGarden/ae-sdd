package ai.nubase.auth.entity;

import jakarta.persistence.*;
import lombok.*;

import java.time.Instant;
import java.util.UUID;

/**
 * Single-use token for passwordless (magic link / email OTP / phone OTP) and
 * reauthentication flows. The {@code tokenHash} stores a SHA-256 hash of the
 * token or numeric code rather than the plaintext value.
 * Mirrors Supabase GoTrue's {@code auth.one_time_tokens}.
 */
@Entity
@Table(name = "one_time_tokens", schema = "auth")
@Getter
@Setter
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class OneTimeToken {

    public static final String TYPE_MAGICLINK = "magiclink";
    public static final String TYPE_OTP = "otp";
    public static final String TYPE_PHONE_OTP = "phone_otp";
    public static final String TYPE_REAUTHENTICATION = "reauthentication";

    @Id
    @GeneratedValue(strategy = GenerationType.AUTO)
    private UUID id;

    @ManyToOne(fetch = FetchType.LAZY)
    @JoinColumn(name = "user_id", nullable = false)
    private User user;

    @Column(name = "token_type", nullable = false)
    private String tokenType;

    @Column(name = "token_hash", nullable = false)
    private String tokenHash;

    @Column(name = "relates_to")
    private String relatesTo;

    /** PKCE code challenge captured when the flow was initiated (optional). */
    @Column(name = "code_challenge")
    private String codeChallenge;

    @Column(name = "code_challenge_method")
    private String codeChallengeMethod;

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
