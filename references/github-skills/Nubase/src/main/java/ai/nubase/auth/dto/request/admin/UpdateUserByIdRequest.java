package ai.nubase.auth.dto.request.admin;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.Email;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.Map;

/**
 * Request DTO for updating a user by ID via Admin API.
 * All fields are optional - only provided fields will be updated.
 */
@Data
@NoArgsConstructor
@AllArgsConstructor
public class UpdateUserByIdRequest {

    @Email(message = "Invalid email format")
    private String email;

    /**
     * New password for the user
     */
    private String password;

    private String phone;

    /**
     * User role for RLS (Row Level Security)
     */
    private String role;

    /**
     * Custom user metadata (merged with existing)
     */
    @JsonProperty("user_metadata")
    private Map<String, Object> userMetadata;

    /**
     * Application-level metadata (merged with existing, admin-only)
     */
    @JsonProperty("app_metadata")
    private Map<String, Object> appMetadata;

    /**
     * Immediately confirm email (bypass email confirmation flow)
     */
    @JsonProperty("email_confirm")
    private Boolean emailConfirm;

    /**
     * Immediately confirm phone
     */
    @JsonProperty("phone_confirm")
    private Boolean phoneConfirm;

    /**
     * Ban duration: "24h", "7d", "none", "permanent"
     */
    @JsonProperty("ban_duration")
    private String banDuration;
}
