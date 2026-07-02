package ai.nubase.auth.dto.request.admin;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.Email;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.Map;

/**
 * Request DTO for creating a user via Admin API.
 * Supports auto-confirmation, password generation, and user metadata.
 */
@Data
@NoArgsConstructor
@AllArgsConstructor
public class CreateUserRequest {

    @Email(message = "Invalid email format")
    private String email;

    /**
     * User password. If not provided, will be auto-generated.
     */
    private String password;

    private String phone;

    /**
     * User role for RLS (Row Level Security).
     * If not provided, defaults to "authenticated"
     */
    private String role;

    /**
     * Custom user metadata (user-facing data)
     */
    @JsonProperty("user_metadata")
    private Map<String, Object> userMetadata;

    /**
     * Application-level metadata (admin-controlled)
     */
    @JsonProperty("app_metadata")
    private Map<String, Object> appMetadata;

    /**
     * Auto-confirm email without sending confirmation email
     */
    @JsonProperty("email_confirm")
    private Boolean emailConfirm;

    /**
     * Auto-confirm phone without sending SMS
     */
    @JsonProperty("phone_confirm")
    private Boolean phoneConfirm;

    /**
     * Ban duration: "24h", "7d", "none", "permanent"
     */
    @JsonProperty("ban_duration")
    private String banDuration;
}
