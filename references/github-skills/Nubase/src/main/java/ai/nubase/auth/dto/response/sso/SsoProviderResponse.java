package ai.nubase.auth.dto.response.sso;

import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;
import java.util.List;

/** Response describing a registered SSO provider. */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
@JsonInclude(JsonInclude.Include.NON_NULL)
public class SsoProviderResponse {

    private String id;

    @JsonProperty("resource_id")
    private String resourceId;

    private boolean enabled;

    private List<String> domains;

    @JsonProperty("entity_id")
    private String entityId;

    @JsonProperty("sso_url")
    private String ssoUrl;

    @JsonProperty("created_at")
    private Instant createdAt;
}
