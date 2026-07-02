package ai.nubase.auth.entity;

import io.hypersistence.utils.hibernate.type.json.JsonBinaryType;
import jakarta.persistence.*;
import lombok.*;
import org.hibernate.annotations.Type;

import java.time.Instant;
import java.util.Map;
import java.util.UUID;

/**
 * SAML-specific configuration (the IdP metadata) for an {@link SsoProvider}.
 * Mirrors Supabase GoTrue's {@code auth.saml_providers}.
 */
@Entity
@Table(name = "saml_providers", schema = "auth")
@Getter
@Setter
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class SamlProvider {

    @Id
    @GeneratedValue(strategy = GenerationType.AUTO)
    private UUID id;

    @ManyToOne(fetch = FetchType.LAZY)
    @JoinColumn(name = "sso_provider_id", nullable = false)
    private SsoProvider ssoProvider;

    @Column(name = "entity_id", nullable = false)
    private String entityId;

    @Column(name = "metadata_xml", columnDefinition = "TEXT")
    private String metadataXml;

    @Column(name = "metadata_url")
    private String metadataUrl;

    @Column(name = "sso_url")
    private String ssoUrl;

    @Column(name = "x509_certificate", columnDefinition = "TEXT")
    private String x509Certificate;

    @Type(JsonBinaryType.class)
    @Column(name = "attribute_mapping", columnDefinition = "jsonb")
    private Map<String, Object> attributeMapping;

    @Column(name = "name_id_format")
    private String nameIdFormat;

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
