package ai.nubase.auth.dto.response.admin;

import ai.nubase.auth.dto.response.UserResponse;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Admin API response format compatible with Supabase.
 * Returns either user data or error message.
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class AdminUserResponse {

    /**
     * User data (if success)
     */
    private UserResponse user;

    /**
     * Error message (if failed)
     */
    private String error;
}
