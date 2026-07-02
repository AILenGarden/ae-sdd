package ai.nubase.common.config;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import lombok.Data;

import java.util.Map;
import java.util.List;

/**
 * Per-tenant authentication settings, stored as JSON in {@code database_configs.auth_config}
 * and layered over the global {@link AuthConfig} (application.yml {@code nubase.auth.*}).
 *
 * <p>Each group is nullable: when null the tenant inherits the global default for that group;
 * when present it fully replaces the global group. Resolution is done by
 * {@link ai.nubase.auth.service.EffectiveAuthConfig}. Unknown JSON fields are ignored so the
 * document can evolve without breaking older tenants.
 */
@Data
@JsonIgnoreProperties(ignoreUnknown = true)
public class TenantAuthConfig {

    private AuthConfig.MfaSettings mfa;
    private AuthConfig.OtpSettings otp;
    private AuthConfig.SmsSettings sms;
    private AuthConfig.CaptchaSettings captcha;
    private AuthConfig.RateLimitSettings rateLimit;
    private AuthConfig.RedirectSettings redirect;
    private AuthConfig.PasswordSettings password;

    /**
     * Platform-owned redirect allow-lists keyed by namespace.
     * <p>Used by external platforms to manage their own callback domains without replacing a
     * tenant's manually configured {@code redirect.allowList}.
     */
    private Map<String, List<String>> managedRedirectAllowLists;

    /** Override email-confirmation requirement (null → inherit global / OAuth config). */
    private Boolean emailConfirmationRequired;

    /** Disable public sign-up for this tenant (null/false → enabled). */
    private Boolean disableSignup;

    /**
     * Per-tenant email template overrides, keyed by template type
     * (confirmation | recovery | invite | magic_link | email_change | reauthentication).
     * A type absent here uses the built-in default from {@code EmailTemplateService}.
     */
    private Map<String, EmailTemplate> emailTemplates;
}
