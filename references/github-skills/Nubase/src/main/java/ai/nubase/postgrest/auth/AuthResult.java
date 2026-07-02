package ai.nubase.postgrest.auth;

import io.jsonwebtoken.Claims;
import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Authentication result
 */
@Data
@Builder
@AllArgsConstructor
@NoArgsConstructor
public class AuthResult {
    private String role;
    private boolean authenticated;
    private Claims claims;
    private String error;
}
