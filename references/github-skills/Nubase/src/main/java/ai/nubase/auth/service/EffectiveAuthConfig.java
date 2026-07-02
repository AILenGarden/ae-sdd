package ai.nubase.auth.service;

import ai.nubase.common.config.AuthConfig;
import ai.nubase.common.config.TenantAuthConfig;
import ai.nubase.common.config.oauth.OAuthProperties;
import ai.nubase.common.context.MultiTenancyContext;
import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Component;

import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;

/**
 * Resolves the EFFECTIVE auth settings for the current request: a per-tenant
 * {@link TenantAuthConfig} override (from {@code database_configs.auth_config}) layered over the
 * global {@link AuthConfig} (application.yml {@code nubase.auth.*}). Each group falls back to the
 * global default when the tenant has not overridden it.
 *
 * <p>All auth services should read settings through this resolver rather than directly off
 * {@link AuthConfig}, so per-tenant configuration takes effect.
 */
@Component
@RequiredArgsConstructor
public class EffectiveAuthConfig {

    private final AuthConfig global;

    public AuthConfig global() {
        return global;
    }

    private TenantAuthConfig tenant() {
        return MultiTenancyContext.getTenantAuthConfig();
    }

    public AuthConfig.MfaSettings mfa() {
        TenantAuthConfig t = tenant();
        return t != null && t.getMfa() != null ? t.getMfa() : global.getMfa();
    }

    public AuthConfig.OtpSettings otp() {
        TenantAuthConfig t = tenant();
        return t != null && t.getOtp() != null ? t.getOtp() : global.getOtp();
    }

    public AuthConfig.SmsSettings sms() {
        TenantAuthConfig t = tenant();
        return t != null && t.getSms() != null ? t.getSms() : global.getSms();
    }

    public AuthConfig.CaptchaSettings captcha() {
        TenantAuthConfig t = tenant();
        return t != null && t.getCaptcha() != null ? t.getCaptcha() : global.getCaptcha();
    }

    public AuthConfig.RateLimitSettings rateLimit() {
        TenantAuthConfig t = tenant();
        return t != null && t.getRateLimit() != null ? t.getRateLimit() : global.getRateLimit();
    }

    public AuthConfig.RedirectSettings redirect() {
        TenantAuthConfig t = tenant();
        AuthConfig.RedirectSettings base = t != null && t.getRedirect() != null
                ? t.getRedirect()
                : global.getRedirect();
        if (t == null || t.getManagedRedirectAllowLists() == null
                || t.getManagedRedirectAllowLists().isEmpty()) {
            return base;
        }
        return withManagedRedirectAllowLists(base, t.getManagedRedirectAllowLists());
    }

    private AuthConfig.RedirectSettings withManagedRedirectAllowLists(
            AuthConfig.RedirectSettings base,
            Map<String, List<String>> managed) {
        AuthConfig.RedirectSettings merged = new AuthConfig.RedirectSettings();
        merged.setAllowTenantDomain(base.isAllowTenantDomain());
        merged.setAllowLocalhost(base.isAllowLocalhost());
        merged.setSiteUrl(base.getSiteUrl());

        LinkedHashSet<String> allowList = new LinkedHashSet<>();
        if (base.getAllowList() != null) {
            allowList.addAll(base.getAllowList());
        }
        managed.values().forEach(urls -> {
            if (urls != null) {
                allowList.addAll(urls);
            }
        });
        merged.setAllowList(new ArrayList<>(allowList));
        return merged;
    }

    public AuthConfig.PasswordSettings password() {
        TenantAuthConfig t = tenant();
        return t != null && t.getPassword() != null ? t.getPassword() : global.getPassword();
    }

    /**
     * Whether email confirmation is required. Precedence: tenant auth_config override →
     * per-tenant OAuth config flag → global default (true).
     */
    public boolean emailConfirmationRequired() {
        TenantAuthConfig t = tenant();
        if (t != null && t.getEmailConfirmationRequired() != null) {
            return t.getEmailConfirmationRequired();
        }
        OAuthProperties oauth = MultiTenancyContext.getOAuthProperties();
        if (oauth != null && oauth.getEmailConfirmationRequired() != null) {
            return oauth.getEmailConfirmationRequired();
        }
        return true;
    }

    /** Whether public sign-up is disabled for this tenant (default false → enabled). */
    public boolean signupDisabled() {
        TenantAuthConfig t = tenant();
        return t != null && Boolean.TRUE.equals(t.getDisableSignup());
    }

    /**
     * Per-tenant override for an email template type, or {@code null} when the tenant has not
     * customized it (→ caller uses the built-in default).
     */
    public ai.nubase.common.config.EmailTemplate emailTemplateOverride(String type) {
        TenantAuthConfig t = tenant();
        if (t == null || t.getEmailTemplates() == null) {
            return null;
        }
        return t.getEmailTemplates().get(type);
    }
}
