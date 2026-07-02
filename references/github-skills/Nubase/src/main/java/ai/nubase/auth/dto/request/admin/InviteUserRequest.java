package ai.nubase.auth.dto.request.admin;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.Email;
import jakarta.validation.constraints.NotBlank;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.Map;

/**
 * Request DTO for inviting a user via Admin API.
 * Sends an invitation email with a confirmation link.
 */
@Data
@NoArgsConstructor
@AllArgsConstructor
public class InviteUserRequest {

    @Email(message = "Invalid email format")
    @NotBlank(message = "Email is required")
    private String email;

    /**
     * User metadata to set for the invited user
     */
    @JsonProperty("data")
    private Map<String, Object> data;

    /**
     * Optional redirect URL after invitation acceptance
     */
    @JsonProperty("redirect_to")
    private String redirectTo;
}
