package ai.nubase.auth.controller;

import ai.nubase.auth.dto.response.SettingsResponse;
import ai.nubase.common.config.AuthConfig;
import ai.nubase.common.config.oauth.OAuthProperties;
import ai.nubase.common.context.MultiTenancyContext;
import lombok.RequiredArgsConstructor;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * {@code GET /auth/v1/settings} — advertises the auth methods/providers enabled for the
 * current tenant. The Supabase client calls this on startup to decide which UI to render.
 */
@RestController
@RequiredArgsConstructor
@RequestMapping("/auth/v1")
public class SettingsController {

    private final ai.nubase.auth.service.EffectiveAuthConfig effectiveAuthConfig;

    private static final String[] KNOWN_PROVIDERS = {
            "google", "github", "wechat", "apple", "azure", "bitbucket",
            "discord", "facebook", "gitlab", "linkedin", "twitter", "slack"
    };

    @GetMapping("/settings")
    public ResponseEntity<SettingsResponse> settings() {
        Map<String, Boolean> external = new LinkedHashMap<>();
        OAuthProperties oauth = MultiTenancyContext.getOAuthProperties();
        for (String name : KNOWN_PROVIDERS) {
            boolean enabled = oauth != null
                    && oauth.getProviders() != null
                    && oauth.getProviders().containsKey(name)
                    && oauth.getProviders().get(name).isEnabled();
            external.put(name, enabled);
        }
        // Email + phone are always advertised as available transports.
        external.put("email", true);
        external.put("phone", effectiveAuthConfig.sms().isEnabled());
        external.put("anonymous", true);

        SettingsResponse response = SettingsResponse.builder()
                .external(external)
                .disableSignup(effectiveAuthConfig.signupDisabled())
                .mailerAutoconfirm(!effectiveAuthConfig.emailConfirmationRequired())
                .phoneAutoconfirm(false)
                .smsProvider(effectiveAuthConfig.sms().getProvider())
                .mfaEnabled(effectiveAuthConfig.mfa().isEnabled())
                .saml(true)
                .build();
        return ResponseEntity.ok(response);
    }
}
