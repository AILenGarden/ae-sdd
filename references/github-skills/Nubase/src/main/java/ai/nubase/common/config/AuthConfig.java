package ai.nubase.common.config;

import lombok.Getter;
import lombok.Setter;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.context.annotation.Configuration;

import java.util.Map;

@Configuration
@ConfigurationProperties(prefix = "nubase.auth")
@Getter
@Setter
public class AuthConfig {
    private JwtSettings jwt = new JwtSettings();
    private RefreshTokenSettings refreshToken = new RefreshTokenSettings();
    private PasswordSettings password = new PasswordSettings();
    private EmailSettings email = new EmailSettings();
    private AppSettings app = new AppSettings();
    private MfaSettings mfa = new MfaSettings();
    private OtpSettings otp = new OtpSettings();
    private SmsSettings sms = new SmsSettings();
    private RateLimitSettings rateLimit = new RateLimitSettings();
    private CaptchaSettings captcha = new CaptchaSettings();
    private IdTokenSettings idToken = new IdTokenSettings();
    private RedirectSettings redirect = new RedirectSettings();

    @Getter
    @Setter
    public static class JwtSettings {
        private String secret;
        private String algorithm = "HS256";
        private int expiration = 3600;
        private String issuer = "supabase";
    }

    @Getter
    @Setter
    public static class RefreshTokenSettings {
        private int expiration = 2592000; // 30 days
        private boolean rotation = true;
    }

    @Getter
    @Setter
    public static class PasswordSettings {
        private int minLength = 6;
        private boolean requireUppercase = false;
        private boolean requireLowercase = false;
        private boolean requireNumber = false;
        private boolean requireSpecial = false;
        /** Require a fresh reauthentication nonce when changing password via PUT /user. */
        private boolean requireReauthentication = false;
    }

    @Getter
    @Setter
    public static class EmailSettings {
        //        private boolean confirmationRequired = true;
        private int confirmationExpiration = 86400; // 24 hours
        private String fromAddress = "noreply@supabase.local";
        private String fromName = "Supabase Auth";
    }

    @Getter
    @Setter
    public static class AppSettings {
        private String serviceName = "localhost:9999";
        private String scheme = "http";

        public String getDomain(String appCode) {
            return String.format("%s://%s.%s", scheme, appCode, serviceName);
        }
    }

    /** Multi-factor authentication (TOTP / phone) settings. */
    @Getter
    @Setter
    public static class MfaSettings {
        private boolean enabled = true;
        /** Issuer label shown in authenticator apps (otpauth URI). */
        private String issuer = "Nubase";
        /** Number of digits in TOTP codes. */
        private int digits = 6;
        /** TOTP time step in seconds. */
        private int period = 30;
        /** Allowed +/- time-step drift when verifying a TOTP code. */
        private int allowedDrift = 1;
        /** Max verified factors a user may enroll. */
        private int maxEnrolledFactors = 10;
        /** Seconds a challenge remains valid. */
        private long challengeExpiration = 300;
    }

    /** Passwordless (magic link / email-OTP / phone-OTP) settings. */
    @Getter
    @Setter
    public static class OtpSettings {
        /** Length of numeric email/phone OTP codes. */
        private int length = 6;
        /** Seconds an OTP / magic link remains valid. */
        private long expiration = 3600;
        /** Allow magic-link / OTP sign-in to auto-create a new user. */
        private boolean allowAutoSignup = true;
    }

    /** Outbound SMS provider settings. */
    @Getter
    @Setter
    public static class SmsSettings {
        private boolean enabled = false;
        /** Provider id: 'log' (default, prints code) | 'twilio' | 'custom'. */
        private String provider = "log";
        private String accountSid;
        private String authToken;
        private String fromNumber;
    }

    /** Per-endpoint rate-limit + failed-login lockout settings. */
    @Getter
    @Setter
    public static class RateLimitSettings {
        private boolean enabled = true;
        /** Max requests per window for sensitive endpoints (otp/recover/signup/token). */
        private int maxRequests = 30;
        /** Sliding window length in seconds. */
        private int windowSeconds = 300;
        /** Consecutive failed sign-ins before the identity is temporarily locked. */
        private int maxFailedLogins = 5;
        /** Lockout duration in seconds after too many failed sign-ins. */
        private int lockoutSeconds = 900;
    }

    /** CAPTCHA verification (hCaptcha / Cloudflare Turnstile). */
    @Getter
    @Setter
    public static class CaptchaSettings {
        private boolean enabled = false;
        /** 'hcaptcha' | 'turnstile'. */
        private String provider = "hcaptcha";
        private String secret;
    }

    /**
     * Native social sign-in via ID token ({@code grant_type=id_token}). Holds per-provider
     * JWKS endpoints / issuers / accepted audiences used to verify the supplied ID token.
     */
    @Getter
    @Setter
    public static class IdTokenSettings {
        private Map<String, Provider> providers = defaultProviders();

        private static Map<String, Provider> defaultProviders() {
            Map<String, Provider> map = new java.util.HashMap<>();
            map.put("google", new Provider(
                    "https://www.googleapis.com/oauth2/v3/certs",
                    "https://accounts.google.com,accounts.google.com",
                    new java.util.ArrayList<>()));
            map.put("apple", new Provider(
                    "https://appleid.apple.com/auth/keys",
                    "https://appleid.apple.com",
                    new java.util.ArrayList<>()));
            return map;
        }

        @Getter
        @Setter
        public static class Provider {
            private String jwksUri;
            /** Comma-separated allowed issuers. */
            private String issuer;
            /** Accepted audiences (client IDs); empty = skip audience check. */
            private java.util.List<String> audiences = new java.util.ArrayList<>();

            public Provider() {}

            public Provider(String jwksUri, String issuer, java.util.List<String> audiences) {
                this.jwksUri = jwksUri;
                this.issuer = issuer;
                this.audiences = audiences;
            }
        }
    }

    /**
     * Allow-list for {@code redirect_to} targets (prevents open-redirect). A redirect URL is
     * accepted if it is relative ({@code /path}), points at the tenant's own domain, is a
     * localhost URL (when {@link #allowLocalhost} is on), or matches one of {@link #allowList}
     * (glob: {@code *} = within a path segment, {@code **} = across segments). Anything else is
     * rejected and the caller falls back to {@link #siteUrl} (or a non-redirect response).
     */
    @Getter
    @Setter
    public static class RedirectSettings {
        /** Permit redirects back to the requesting tenant's own domain (appCode.serviceName). */
        private boolean allowTenantDomain = true;
        /** Permit localhost / 127.0.0.1 redirects (handy for local development). */
        private boolean allowLocalhost = true;
        /** Optional fallback URL used when a supplied redirect_to is rejected. */
        private String siteUrl;
        /** Additional allowed redirect URL patterns (support * and ** wildcards). */
        private java.util.List<String> allowList = new java.util.ArrayList<>();
    }

}
