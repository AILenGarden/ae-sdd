/**
 * Shared TypeScript types for the project Authentication settings page.
 *
 * Mirrors the backend DTO ai.nubase.common.config.TenantAuthConfig returned/accepted by
 * GET/PUT /auth/v1/admin/settings/auth. Jackson uses default camelCase serialization, so the
 * field names here must match exactly. Every group is optional: the GET returns the EFFECTIVE
 * (merged) config, and a PUT stores the supplied groups as the per-tenant override.
 */

export interface MfaSettings {
  enabled: boolean;
  issuer: string;
  digits: number;
  period: number;
  allowedDrift: number;
  maxEnrolledFactors: number;
  challengeExpiration: number;
}

export interface OtpSettings {
  length: number;
  expiration: number;
  allowAutoSignup: boolean;
}

export interface SmsSettings {
  enabled: boolean;
  provider: string; // 'log' | 'twilio' | 'custom'
  accountSid?: string | null;
  authToken?: string | null;
  fromNumber?: string | null;
}

export interface CaptchaSettings {
  enabled: boolean;
  provider: string; // 'hcaptcha' | 'turnstile'
  secret?: string | null;
}

export interface RateLimitSettings {
  enabled: boolean;
  maxRequests: number;
  windowSeconds: number;
  maxFailedLogins: number;
  lockoutSeconds: number;
}

export interface RedirectSettings {
  allowTenantDomain: boolean;
  allowLocalhost: boolean;
  siteUrl?: string | null;
  allowList: string[];
}

export interface PasswordSettings {
  minLength: number;
  requireUppercase: boolean;
  requireLowercase: boolean;
  requireNumber: boolean;
  requireSpecial: boolean;
  requireReauthentication: boolean;
}

export interface TenantAuthConfig {
  mfa: MfaSettings;
  otp: OtpSettings;
  sms: SmsSettings;
  captcha: CaptchaSettings;
  rateLimit: RateLimitSettings;
  redirect: RedirectSettings;
  password: PasswordSettings;
  emailConfirmationRequired: boolean;
  disableSignup: boolean;
}

/**
 * OAuth provider config — mirrors ai.nubase.common.config.oauth.OAuthProperties, stored per
 * tenant in database_configs.oauth_config (GET/PUT /auth/v1/admin/oauth).
 */
export interface OAuthProviderConfig {
  enabled: boolean;
  clientId?: string | null;
  clientSecret?: string | null;
  redirectUri?: string | null;
  scope?: string | null;
  callbackUrl?: string | null;
}

export interface OAuthProperties {
  providers: Record<string, OAuthProviderConfig>;
  emailConfirmationRequired?: boolean | null;
}

/**
 * SAML SSO provider — mirrors ai.nubase.auth.dto.response.sso.SsoProviderResponse and
 * .request.sso.CreateSsoProviderRequest (/auth/v1/admin/sso/providers).
 */
export interface SsoProviderResponse {
  id: string;
  resourceId?: string | null;
  enabled: boolean;
  domains: string[];
  entityId?: string | null;
  ssoUrl?: string | null;
  createdAt?: string | null;
}

export interface CreateSsoProviderRequest {
  type?: string;
  resourceId?: string;
  domains?: string[];
  metadataXml?: string;
  metadataUrl?: string;
  entityId?: string;
  ssoUrl?: string;
  x509Certificate?: string;
  attributeMapping?: Record<string, unknown>;
}

/** Email template editor — /auth/v1/admin/settings/email-templates. */
export interface EmailTemplate {
  subject: string;
  content: string;
}

export interface EmailTemplatesResponse {
  types: string[];
  templates: Record<string, EmailTemplate>;
  variables: Record<string, string[]>;
  customized: Record<string, boolean>;
}
