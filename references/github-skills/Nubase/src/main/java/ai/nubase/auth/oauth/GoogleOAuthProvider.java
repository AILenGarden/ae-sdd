package ai.nubase.auth.oauth;

import ai.nubase.common.config.oauth.OAuthProperties;
import ai.nubase.auth.dto.oauth.OAuthUserInfo;
import ai.nubase.common.context.MultiTenancyContext;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.http.*;
import org.springframework.stereotype.Component;
import org.springframework.util.LinkedMultiValueMap;
import org.springframework.util.MultiValueMap;
import org.springframework.web.client.RestTemplate;
import org.springframework.web.util.UriComponentsBuilder;

/**
 * Google OAuth 2.0 Provider Implementation
 * https://developers.google.com/identity/protocols/oauth2
 */
@Component
@RequiredArgsConstructor
@Slf4j
public class GoogleOAuthProvider implements OAuthProvider {

    private static final String AUTHORIZATION_URL = "https://accounts.google.com/o/oauth2/v2/auth";
    private static final String TOKEN_URL = "https://oauth2.googleapis.com/token";
    private static final String USER_INFO_URL = "https://www.googleapis.com/oauth2/v2/userinfo";
    private static final String SCOPE = "openid email profile";

    //    private final OAuthProperties oAuthProperties;
    private final RestTemplate restTemplate = new RestTemplate();
    private final ObjectMapper objectMapper = new ObjectMapper();

    @Override
    public String getProviderName() {
        return "google";
    }

    @Override
    public String getAuthorizationUrl(String redirectUri, String state) {
        OAuthProperties.ProviderConfig config = getProviderConfig();

        return UriComponentsBuilder.fromHttpUrl(AUTHORIZATION_URL)
                .queryParam("client_id", config.getClientId())
                .queryParam("redirect_uri", redirectUri)
                .queryParam("response_type", "code")
                .queryParam("scope", SCOPE)
                .queryParam("state", state)
                .queryParam("access_type", "offline")
                .queryParam("prompt", "consent")
                .build()
                .encode()  // Encode URL parameters, converting spaces to %20
                .toUriString();
    }

    @Override
    public OAuthUserInfo getUserInfo(String code, String redirectUri) {
        try {
            // Step 1: Exchange code for access token
            String accessToken = exchangeCodeForToken(code, redirectUri);

            // Step 2: Get user info using access token
            return fetchUserInfo(accessToken);

        } catch (Exception e) {
            log.error("Failed to get Google user info: {}", e.getMessage(), e);
            throw new RuntimeException("Failed to authenticate with Google: " + e.getMessage(), e);
        }
    }

    private String exchangeCodeForToken(String code, String redirectUri) throws Exception {
        OAuthProperties.ProviderConfig config = getProviderConfig();

        MultiValueMap<String, String> params = new LinkedMultiValueMap<>();
        params.add("code", code);
        params.add("client_id", config.getClientId());
        params.add("client_secret", config.getClientSecret());
        params.add("redirect_uri", redirectUri);
        params.add("grant_type", "authorization_code");

        HttpHeaders headers = new HttpHeaders();
        headers.setContentType(MediaType.APPLICATION_FORM_URLENCODED);

        HttpEntity<MultiValueMap<String, String>> request = new HttpEntity<>(params, headers);

        ResponseEntity<String> response = restTemplate.exchange(
                TOKEN_URL,
                HttpMethod.POST,
                request,
                String.class
        );

        JsonNode jsonNode = objectMapper.readTree(response.getBody());
        return jsonNode.get("access_token").asText();
    }

    private OAuthUserInfo fetchUserInfo(String accessToken) throws Exception {
        HttpHeaders headers = new HttpHeaders();
        headers.setBearerAuth(accessToken);

        HttpEntity<String> request = new HttpEntity<>(headers);

        ResponseEntity<String> response = restTemplate.exchange(
                USER_INFO_URL,
                HttpMethod.GET,
                request,
                String.class
        );

        JsonNode userInfo = objectMapper.readTree(response.getBody());

        return OAuthUserInfo.builder()
                .providerId(userInfo.get("id").asText())
                .provider("google")
                .email(userInfo.has("email") ? userInfo.get("email").asText() : null)
                .emailVerified(userInfo.has("verified_email") && userInfo.get("verified_email").asBoolean())
                .name(userInfo.has("name") ? userInfo.get("name").asText() : null)
                .avatarUrl(userInfo.has("picture") ? userInfo.get("picture").asText() : null)
                .rawData(response.getBody())
                .build();
    }

    private OAuthProperties.ProviderConfig getProviderConfig() {
        OAuthProperties properties = MultiTenancyContext.getOAuthProperties();
        if (properties == null || properties.getProviders() == null) {
            throw new IllegalStateException("OAuth properties not configured, please check your configuration.");
        }
        OAuthProperties.ProviderConfig config = properties.getProviders().get("google");
        if (config == null || !config.isEnabled()) {
            throw new IllegalStateException("Google OAuth provider not configured, please check your configuration.");
        }
        return config;
    }
}
