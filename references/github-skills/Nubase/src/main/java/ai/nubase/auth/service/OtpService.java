package ai.nubase.auth.service;

import ai.nubase.auth.dto.response.AuthResponse;
import ai.nubase.auth.entity.Identity;
import ai.nubase.auth.entity.MfaAmrClaim;
import ai.nubase.auth.entity.OneTimeToken;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.IdentityRepository;
import ai.nubase.auth.repository.OneTimeTokenRepository;
import ai.nubase.auth.repository.UserRepository;
import ai.nubase.auth.util.TokenGenerator;
import ai.nubase.common.config.AuthConfig;
import ai.nubase.common.enums.Role;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang3.StringUtils;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Passwordless authentication: magic links and one-time codes over email or SMS.
 * Mirrors Supabase GoTrue's {@code POST /otp}, {@code POST /resend} and the
 * {@code magiclink}/{@code email}/{@code sms} verification types.
 */
@Service
@RequiredArgsConstructor
@Slf4j
public class OtpService {

    private final UserRepository userRepository;
    private final IdentityRepository identityRepository;
    private final OneTimeTokenRepository oneTimeTokenRepository;
    private final TokenGenerator tokenGenerator;
    private final EmailService emailService;
    private final SmsService smsService;
    private final EffectiveAuthConfig effectiveAuthConfig;
    private final AuthResponseFactory authResponseFactory;
    private final AuditService auditService;
    private final RateLimiterService rateLimiterService;
    private final CaptchaService captchaService;
    private final PkceService pkceService;

    /** Verification result: either a session, or a PKCE auth code to be exchanged later. */
    public record VerifyResult(AuthResponse session, String pkceAuthCode) {
        public boolean isPkce() {
            return pkceAuthCode != null;
        }
    }

    /**
     * Request a passwordless email login (magic link + OTP code).
     *
     * @param shouldCreateUser whether to auto-create a user that doesn't exist yet
     */
    @Transactional
    public void signInWithEmailOtp(String email, boolean shouldCreateUser, String captchaToken, String redirectTo) {
        signInWithEmailOtp(email, shouldCreateUser, captchaToken, redirectTo, null, null);
    }

    @Transactional
    public void signInWithEmailOtp(String email, boolean shouldCreateUser, String captchaToken,
                                   String redirectTo, String codeChallenge, String codeChallengeMethod) {
        captchaService.verify(captchaToken);
        rateLimiterService.checkRate("otp", email);

        User user = userRepository.findByEmail(email).orElse(null);
        if (user == null) {
            if (!shouldCreateUser || !effectiveAuthConfig.otp().isAllowAutoSignup()) {
                // Do not reveal whether the user exists.
                log.debug("OTP requested for unknown email and auto-signup disabled");
                return;
            }
            user = createOtpUser(email, null);
        }

        String linkToken = tokenGenerator.generateSecureToken();
        String otpCode = tokenGenerator.generateNumericOTP(effectiveAuthConfig.otp().getLength());

        storeToken(user, OneTimeToken.TYPE_MAGICLINK, linkToken, email, codeChallenge, codeChallengeMethod);
        storeToken(user, OneTimeToken.TYPE_OTP, otpCode, email, codeChallenge, codeChallengeMethod);

        emailService.sendMagicLinkEmail(user, linkToken, otpCode, redirectTo);
        auditService.record(AuditService.OTP_REQUESTED, user, Map.of("channel", "email"));
    }

    /**
     * Request a passwordless phone login (SMS OTP code).
     */
    @Transactional
    public void signInWithPhoneOtp(String phone, boolean shouldCreateUser, String captchaToken) {
        captchaService.verify(captchaToken);
        rateLimiterService.checkRate("otp", phone);

        User user = userRepository.findByPhone(phone).orElse(null);
        if (user == null) {
            if (!shouldCreateUser || !effectiveAuthConfig.otp().isAllowAutoSignup()) {
                return;
            }
            user = createOtpUser(null, phone);
        }

        String otpCode = tokenGenerator.generateNumericOTP(effectiveAuthConfig.otp().getLength());
        storeToken(user, OneTimeToken.TYPE_PHONE_OTP, otpCode, phone);

        smsService.sendOtp(phone, otpCode);
        auditService.record(AuditService.OTP_REQUESTED, user, Map.of("channel", "sms"));
    }

    /**
     * Verify a passwordless token/code and establish a session.
     *
     * @param type  magiclink | email | sms | phone
     * @param token the magic-link token or numeric code
     * @param email email associated with the request (email/magiclink types)
     * @param phone phone associated with the request (sms/phone types)
     */
    @Transactional
    public AuthResponse verify(String type, String token, String email, String phone) {
        VerifyResult result = verifyFlow(type, token, email, phone);
        if (result.isPkce()) {
            // A PKCE challenge was attached; the caller must exchange the auth code.
            throw new IllegalStateException(
                    "This is a PKCE flow; exchange the auth code via grant_type=pkce");
        }
        return result.session();
    }

    /**
     * Verify a passwordless token/code. Returns a session, OR (when the originating request
     * carried a PKCE code challenge) a one-time auth code to be exchanged via grant_type=pkce.
     */
    @Transactional
    public VerifyResult verifyFlow(String type, String token, String email, String phone) {
        String tokenType = switch (type) {
            case "magiclink" -> OneTimeToken.TYPE_MAGICLINK;
            case "email", "otp" -> OneTimeToken.TYPE_OTP;
            case "sms", "phone", "phone_change" -> OneTimeToken.TYPE_PHONE_OTP;
            default -> throw new IllegalArgumentException("Unsupported OTP verification type: " + type);
        };

        String hash = tokenGenerator.sha256(token);
        OneTimeToken ott = oneTimeTokenRepository.findByTokenHashAndTokenType(hash, tokenType)
                .orElseThrow(() -> new RuntimeException("Token has expired or is invalid"));

        // Expiry check
        Instant expiry = ott.getCreatedAt().plus(effectiveAuthConfig.otp().getExpiration(), ChronoUnit.SECONDS);
        if (Instant.now().isAfter(expiry)) {
            oneTimeTokenRepository.delete(ott);
            throw new RuntimeException("Token has expired or is invalid");
        }

        // relates_to must match the supplied identifier when provided
        String identifier = StringUtils.isNotBlank(email) ? email : phone;
        if (StringUtils.isNotBlank(identifier) && ott.getRelatesTo() != null
                && !ott.getRelatesTo().equalsIgnoreCase(identifier)) {
            throw new RuntimeException("Token does not match the supplied identifier");
        }

        User user = ott.getUser();
        String codeChallenge = ott.getCodeChallenge();
        String codeChallengeMethod = ott.getCodeChallengeMethod();

        // Confirm the relevant channel on first successful verification.
        if (OneTimeToken.TYPE_PHONE_OTP.equals(tokenType)) {
            if (user.getPhoneConfirmedAt() == null) {
                user.setPhoneConfirmedAt(Instant.now());
            }
        } else if (user.getEmailConfirmedAt() == null) {
            user.setEmailConfirmedAt(Instant.now());
        }
        user.setLastSignInAt(Instant.now());
        userRepository.save(user);

        // One-time: invalidate every passwordless token issued to this user.
        oneTimeTokenRepository.deleteByUserIdAndTokenType(user.getId(), OneTimeToken.TYPE_MAGICLINK);
        oneTimeTokenRepository.deleteByUserIdAndTokenType(user.getId(), OneTimeToken.TYPE_OTP);
        oneTimeTokenRepository.deleteByUserIdAndTokenType(user.getId(), OneTimeToken.TYPE_PHONE_OTP);

        // PKCE: hand back an auth code instead of a session.
        if (StringUtils.isNotBlank(codeChallenge)) {
            String authCode = pkceService.issueAuthCode(
                    user, codeChallenge, codeChallengeMethod, "magiclink", MfaAmrClaim.METHOD_OTP);
            auditService.record(AuditService.OTP_REQUESTED, user, Map.of("flow", "pkce"));
            return new VerifyResult(null, authCode);
        }

        auditService.record(AuditService.LOGIN, user, Map.of("provider", "otp"));
        return new VerifyResult(authResponseFactory.newSignIn(user, MfaAmrClaim.METHOD_OTP), null);
    }

    /**
     * Resend a signup confirmation or OTP. Type: signup | email_change | sms | phone_change.
     */
    @Transactional
    public void resend(String type, String email, String phone) {
        switch (type) {
            case "signup" -> {
                rateLimiterService.checkRate("resend", email);
                userRepository.findByEmail(email).ifPresent(user -> {
                    if (user.getEmailConfirmedAt() != null) {
                        return; // already confirmed — nothing to resend
                    }
                    String confirmationToken = tokenGenerator.generateSecureToken();
                    user.setConfirmationToken(confirmationToken);
                    user.setConfirmationSentAt(Instant.now());
                    userRepository.save(user);
                    emailService.sendConfirmationEmail(user, confirmationToken, null);
                    auditService.record(AuditService.USER_CONFIRMATION_REQUESTED, user);
                });
            }
            case "sms", "phone_change" -> {
                rateLimiterService.checkRate("resend", phone);
                userRepository.findByPhone(phone).ifPresent(user -> {
                    String otpCode = tokenGenerator.generateNumericOTP(effectiveAuthConfig.otp().getLength());
                    storeToken(user, OneTimeToken.TYPE_PHONE_OTP, otpCode, phone);
                    smsService.sendOtp(phone, otpCode);
                    auditService.record(AuditService.OTP_REQUESTED, user, Map.of("channel", "sms"));
                });
            }
            default -> throw new IllegalArgumentException("Unsupported resend type: " + type);
        }
    }

    /**
     * Send a phone-change confirmation OTP to {@code phone}. The user later confirms ownership
     * via {@code /verify?type=sms} (or phone), which sets {@code phone_confirmed_at}.
     */
    @Transactional
    public void sendPhoneChangeOtp(User user, String phone) {
        String otpCode = tokenGenerator.generateNumericOTP(effectiveAuthConfig.otp().getLength());
        storeToken(user, OneTimeToken.TYPE_PHONE_OTP, otpCode, phone);
        smsService.sendOtp(phone, otpCode);
        auditService.record(AuditService.OTP_REQUESTED, user, Map.of("channel", "sms", "flow", "phone_change"));
    }

    private User createOtpUser(String email, String phone) {
        String role = Role.AUTHENTICATED.getValue();
        User user = User.builder()
                .email(email)
                .phone(phone)
                .role(role)
                .aud(role)
                .rawUserMetaData(new HashMap<>())
                .rawAppMetaData(appMetadata(email != null ? "email" : "phone"))
                .isSuperAdmin(false)
                .isSsoUser(false)
                .isAnonymous(false)
                .build();
        user = userRepository.save(user);

        // Mirror the email/phone identity so /user returns it.
        Map<String, Object> identityData = new HashMap<>();
        identityData.put("sub", user.getId().toString());
        if (email != null) identityData.put("email", email);
        if (phone != null) identityData.put("phone", phone);
        identityRepository.save(Identity.builder()
                .user(user)
                .provider(email != null ? "email" : "phone")
                .providerId(user.getId().toString())
                .identityData(identityData)
                .lastSignInAt(Instant.now())
                .build());
        return user;
    }

    private void storeToken(User user, String type, String value, String relatesTo) {
        storeToken(user, type, value, relatesTo, null, null);
    }

    private void storeToken(User user, String type, String value, String relatesTo,
                            String codeChallenge, String codeChallengeMethod) {
        oneTimeTokenRepository.deleteByUserIdAndTokenType(user.getId(), type);
        oneTimeTokenRepository.save(OneTimeToken.builder()
                .user(user)
                .tokenType(type)
                .tokenHash(tokenGenerator.sha256(value))
                .relatesTo(relatesTo)
                .codeChallenge(codeChallenge)
                .codeChallengeMethod(codeChallengeMethod)
                .build());
    }

    private Map<String, Object> appMetadata(String provider) {
        Map<String, Object> metadata = new HashMap<>();
        metadata.put("provider", provider);
        metadata.put("providers", List.of(provider));
        return metadata;
    }
}
