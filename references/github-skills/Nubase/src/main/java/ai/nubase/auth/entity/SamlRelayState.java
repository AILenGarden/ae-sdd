package ai.nubase.auth.entity;

import jakarta.persistence.*;
import lombok.*;

import java.time.Instant;
import java.util.UUID;

/**
 * An in-flight SAML AuthnRequest awaiting the IdP's response (matched by request id).
 * Mirrors Supabase GoTrue's {@code auth.saml_relay_states}.
 */
@Entity
@Table(name = "saml_relay_states", schema = "auth")
@Getter
@Setter
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class SamlRelayState {

    @Id
    @GeneratedValue(strategy = GenerationType.AUTO)
    private UUID id;

    @ManyToOne(fetch = FetchType.LAZY)
    @JoinColumn(name = "sso_provider_id", nullable = false)
    private SsoProvider ssoProvider;

    @Column(name = "request_id", nullable = false)
    private String requestId;

    @Column(name = "for_email")
    private String forEmail;

    @Column(name = "redirect_to")
    private String redirectTo;

    @ManyToOne(fetch = FetchType.LAZY)
    @JoinColumn(name = "flow_state_id")
    private FlowState flowState;

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
