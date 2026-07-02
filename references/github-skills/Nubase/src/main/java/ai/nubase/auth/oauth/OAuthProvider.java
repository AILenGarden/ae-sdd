package ai.nubase.auth.oauth;

import ai.nubase.auth.dto.oauth.OAuthUserInfo;

/**
 * OAuth Provider Interface
 * Defines contract for OAuth 2.0 provider implementations
 */
public interface OAuthProvider {

    /**
     * Get provider name (e.g., "google", "github")
     */
    String getProviderName();

    /**
     * Generate authorization URL for OAuth flow
     *
     * @param redirectUri Callback URL after authorization
     * @param state       State parameter for CSRF protection
     * @return Authorization URL to redirect user
     */
    String getAuthorizationUrl(String redirectUri, String state);

    /**
     * Exchange authorization code for user information
     *
     * @param code        Authorization code from OAuth callback
     * @param redirectUri Same redirect URI used in authorization
     * @return Normalized user information
     */
    OAuthUserInfo getUserInfo(String code, String redirectUri);
}
