package ai.nubase.auth.dto.request.sso;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;
import java.util.Map;

/** Request body for {@code POST /auth/v1/admin/sso/providers}. */
@Data
@NoArgsConstructor
@AllArgsConstructor
public class CreateSsoProviderRequest {

    /** "saml" (only SAML is supported). */
    private String type;

    @JsonProperty("resource_id")
    private String resourceId;

    /** Email domains routed to this provider. */
    private List<String> domains;

    /** IdP entityID. */
    @JsonProperty("metadata_xml")
    private String metadataXml;

    @JsonProperty("metadata_url")
    private String metadataUrl;

    @JsonProperty("entity_id")
    private String entityId;

    @JsonProperty("sso_url")
    private String ssoUrl;

    /** IdP signing certificate (PEM or base64 DER). */
    @JsonProperty("x509_certificate")
    private String x509Certificate;

    @JsonProperty("attribute_mapping")
    private Map<String, Object> attributeMapping;
}
