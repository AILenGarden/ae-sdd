package ai.nubase.auth.dto.oauth;

import lombok.AllArgsConstructor;
import lombok.Builder;
import lombok.Data;
import lombok.NoArgsConstructor;

/**
 * Standard OAuth User Information
 * Normalized from different OAuth providers
 */
@Data
@Builder
@NoArgsConstructor
@AllArgsConstructor
public class OAuthUserInfo {
    private String providerId;      // Unique ID from provider
    private String provider;         // google, github, etc.
    private String email;
    private boolean emailVerified;
    private String name;
    private String avatarUrl;
    private String rawData;          // JSON string of raw provider response
}
