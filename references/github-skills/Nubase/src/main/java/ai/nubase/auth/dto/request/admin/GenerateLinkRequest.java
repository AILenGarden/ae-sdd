package ai.nubase.auth.dto.request.admin;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.NotBlank;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.Map;

/**
 * Request for {@code POST /auth/v1/admin/generate_link}. Mirrors GoTrue's admin generate-link API.
 */
@Data
@NoArgsConstructor
@AllArgsConstructor
public class GenerateLinkRequest {

    /** signup | magiclink | recovery */
    @NotBlank(message = "type is required")
    private String type;

    @NotBlank(message = "email is required")
    private String email;

    /** Optional initial password (signup). */
    private String password;

    /** Optional user metadata (signup). */
    @JsonProperty("data")
    private Map<String, Object> data;

    @JsonProperty("redirect_to")
    private String redirectTo;
}
