package ai.nubase.auth.oauth;

import ai.nubase.auth.dto.oauth.OAuthUserInfo;
import ai.nubase.common.config.oauth.OAuthProperties;
import ai.nubase.common.context.MultiTenancyContext;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang3.StringUtils;
import org.springframework.http.HttpMethod;
import org.springframework.http.ResponseEntity;
import org.springframework.stereotype.Component;
import org.springframework.web.client.RestTemplate;
import org.springframework.web.util.UriComponentsBuilder;

/**
 * WeChat Open Platform website OAuth provider.
 * Uses QR Connect flow and creates/login users with openid/unionid identity.
 */
@Component
@RequiredArgsConstructor
@Slf4j
public class WeChatOAuthProvider implements OAuthProvider {

    private static final String AUTHORIZATION_URL = "https://open.weixin.qq.com/connect/qrconnect";
    private static final String TOKEN_URL = "https://api.weixin.qq.com/sns/oauth2/access_token";
    private static final String USER_INFO_URL = "https://api.weixin.qq.com/sns/userinfo";
    private static final String SCOPE = "snsapi_login";

    private final RestTemplate restTemplate = new RestTemplate();
    private final ObjectMapper objectMapper = new ObjectMapper();

    @Override
    public String getProviderName() {
        return "wechat";
    }

    @Override
    public String getAuthorizationUrl(String redirectUri, String state) {
        OAuthProperties.ProviderConfig config = getProviderConfig();
        String scope = StringUtils.defaultIfBlank(config.getScope(), SCOPE);

        return UriComponentsBuilder.fromHttpUrl(AUTHORIZATION_URL)
                .queryParam("appid", config.getClientId())
                .queryParam("redirect_uri", redirectUri)
                .queryParam("response_type", "code")
                .queryParam("scope", scope)
                .queryParam("state", state)
                .fragment("wechat_redirect")
                .build()
                .encode()
                .toUriString();
    }

    @Override
    public OAuthUserInfo getUserInfo(String code, String redirectUri) {
        try {
            JsonNode tokenResponse = exchangeCodeForToken(code);
            String accessToken = readRequired(tokenResponse, "access_token");
            String openId = readRequired(tokenResponse, "openid");
            String unionId = readOptional(tokenResponse, "unionid");

            ResponseEntity<String> response = restTemplate.exchange(
                    UriComponentsBuilder.fromHttpUrl(USER_INFO_URL)
                            .queryParam("access_token", accessToken)
                            .queryParam("openid", openId)
                            .queryParam("lang", "zh_CN")
                            .build()
                            .encode()
                            .toUriString(),
                    HttpMethod.GET,
                    null,
                    String.class
            );

            JsonNode userInfo = objectMapper.readTree(response.getBody());
            validateWeChatResponse(userInfo, "Failed to fetch WeChat user info");

            String providerId = StringUtils.defaultIfBlank(unionId, openId);
            return OAuthUserInfo.builder()
                    .providerId(providerId)
                    .provider("wechat")
                    .email(null)
                    .emailVerified(false)
                    .name(readOptional(userInfo, "nickname"))
                    .avatarUrl(readOptional(userInfo, "headimgurl"))
                    .rawData(response.getBody())
                    .build();
        } catch (Exception e) {
            log.error("Failed to get WeChat user info: {}", e.getMessage(), e);
            throw new RuntimeException("Failed to authenticate with WeChat: " + e.getMessage(), e);
        }
    }

    private JsonNode exchangeCodeForToken(String code) throws Exception {
        OAuthProperties.ProviderConfig config = getProviderConfig();
        ResponseEntity<String> response = restTemplate.exchange(
                UriComponentsBuilder.fromHttpUrl(TOKEN_URL)
                        .queryParam("appid", config.getClientId())
                        .queryParam("secret", config.getClientSecret())
                        .queryParam("code", code)
                        .queryParam("grant_type", "authorization_code")
                        .build()
                        .encode()
                        .toUriString(),
                HttpMethod.GET,
                null,
                String.class
        );

        JsonNode tokenResponse = objectMapper.readTree(response.getBody());
        validateWeChatResponse(tokenResponse, "Failed to exchange WeChat authorization code");
        return tokenResponse;
    }

    private OAuthProperties.ProviderConfig getProviderConfig() {
        OAuthProperties properties = MultiTenancyContext.getOAuthProperties();
        if (properties == null || properties.getProviders() == null) {
            throw new IllegalStateException("OAuth properties not configured, please check your configuration.");
        }
        OAuthProperties.ProviderConfig config = properties.getProviders().get("wechat");
        if (config == null || !config.isEnabled()) {
            throw new IllegalStateException("WeChat OAuth provider not configured, please check your configuration.");
        }
        if (StringUtils.isAnyBlank(config.getClientId(), config.getClientSecret())) {
            throw new IllegalStateException("WeChat OAuth provider is missing clientId/clientSecret.");
        }
        return config;
    }

    private void validateWeChatResponse(JsonNode response, String defaultMessage) {
        if (response.has("errcode") && response.get("errcode").asInt() != 0) {
            String message = response.has("errmsg") ? response.get("errmsg").asText() : defaultMessage;
            throw new IllegalStateException(message + " (errcode=" + response.get("errcode").asInt() + ")");
        }
    }

    private String readRequired(JsonNode node, String fieldName) {
        if (!node.has(fieldName) || node.get(fieldName).isNull()) {
            throw new IllegalStateException("WeChat response missing field: " + fieldName);
        }
        return node.get(fieldName).asText();
    }

    private String readOptional(JsonNode node, String fieldName) {
        if (!node.has(fieldName) || node.get(fieldName).isNull()) {
            return null;
        }
        return node.get(fieldName).asText();
    }
}
