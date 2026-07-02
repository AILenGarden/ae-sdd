package ai.nubase.auth.entity;

import jakarta.persistence.*;
import lombok.*;

import java.time.Instant;
import java.util.UUID;

/**
 * A single MFA verification attempt against a {@link MfaFactor}.
 * Mirrors Supabase GoTrue's {@code auth.mfa_challenges}.
 */
@Entity
@Table(name = "mfa_challenges", schema = "auth")
@Getter
@Setter
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class MfaChallenge {

    @Id
    @GeneratedValue(strategy = GenerationType.AUTO)
    private UUID id;

    @ManyToOne(fetch = FetchType.LAZY)
    @JoinColumn(name = "factor_id", nullable = false)
    private MfaFactor factor;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "verified_at")
    private Instant verifiedAt;

    @Column(name = "ip_address")
    private String ipAddress;

    /** Only populated for phone factors (the SMS-delivered code). */
    @Column(name = "otp_code")
    private String otpCode;

    @PrePersist
    protected void onCreate() {
        createdAt = Instant.now();
    }
}
