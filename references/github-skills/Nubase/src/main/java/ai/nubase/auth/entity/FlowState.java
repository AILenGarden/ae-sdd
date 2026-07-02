package ai.nubase.auth.entity;

import jakarta.persistence.*;
import lombok.*;

import java.time.Instant;
import java.util.UUID;

/**
 * PKCE / SSO flow state. An {@code authCode} is issued at the end of an OAuth, magic-link
 * or SAML flow and later exchanged for a session via {@code POST /token?grant_type=pkce}
 * after the client proves possession of the original {@code codeVerifier}.
 * Mirrors Supabase GoTrue's {@code auth.flow_state}.
 */
@Entity
@Table(name = "flow_state", schema = "auth")
@Getter
@Setter
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class FlowState {

    public static final String METHOD_S256 = "s256";
    public static final String METHOD_PLAIN = "plain";

    @Id
    @GeneratedValue(strategy = GenerationType.AUTO)
    private UUID id;

    @ManyToOne(fetch = FetchType.LAZY)
    @JoinColumn(name = "user_id")
    private User user;

    @Column(name = "auth_code", nullable = false)
    private String authCode;

    @Column(name = "code_challenge_method", nullable = false)
    private String codeChallengeMethod;

    @Column(name = "code_challenge", nullable = false)
    private String codeChallenge;

    @Column(name = "provider_type")
    private String providerType;

    @Column(name = "authentication_method", nullable = false)
    private String authenticationMethod;

    @Column(name = "auth_code_issued_at")
    private Instant authCodeIssuedAt;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at")
    private Instant updatedAt;

    @PrePersist
    protected void onCreate() {
        createdAt = Instant.now();
        updatedAt = Instant.now();
        if (authCodeIssuedAt == null) {
            authCodeIssuedAt = Instant.now();
        }
    }

    @PreUpdate
    protected void onUpdate() {
        updatedAt = Instant.now();
    }
}
