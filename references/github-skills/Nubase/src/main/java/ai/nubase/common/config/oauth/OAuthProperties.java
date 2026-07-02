package ai.nubase.common.config.oauth;

import lombok.Data;
import org.springframework.context.annotation.Configuration;

import java.util.HashMap;
import java.util.Map;

/**
 * OAuth Configuration Properties
 */
@Configuration
@Data
public class OAuthProperties {

    /**
     * OAuth providers configuration
     * Key: provider name (google, github, etc.)
     * Value: provider-specific configuration
     */
    private Map<String, ProviderConfig> providers = new HashMap<>();


    private Boolean emailConfirmationRequired = true;


    @Data
    public static class ProviderConfig {
        private boolean enabled = true;
        private String clientId;
        private String clientSecret;
        private String redirectUri;
        private String scope;
        private String callbackUrl;
    }

//    public static void main(String[] args) {
//        OAuthProperties oAuthProperties = new OAuthProperties();
//        OAuthProperties.ProviderConfig googleConfig = new OAuthProperties.ProviderConfig();
//        googleConfig.setEnabled(true);
//        googleConfig.setClientId("your-google-client-id");
//        googleConfig.setClientSecret("your-google-client-secret");
//        googleConfig.setRedirectUri("http://localhost:8080/oauth/callback/google");
//        googleConfig.setScope("openid profile email");
//        oAuthProperties.getProviders().put("google", googleConfig);
//        System.out.println(JSONUtil.toJsonStr(oAuthProperties));
//    }
}
