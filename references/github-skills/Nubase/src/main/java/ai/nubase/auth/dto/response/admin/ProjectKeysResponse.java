package ai.nubase.auth.dto.response.admin;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Both API keys for a project so the Studio can render an "API keys" card
 * without re-fetching the full project list (which omits the authenticated
 * token by default).
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class ProjectKeysResponse {
    @JsonProperty("service_role_token")
    private String serviceRoleToken;

    @JsonProperty("authenticated_token")
    private String authenticatedToken;
}
