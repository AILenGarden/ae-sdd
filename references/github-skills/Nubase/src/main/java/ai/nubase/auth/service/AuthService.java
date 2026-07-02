package ai.nubase.auth.service;

import ai.nubase.common.config.AuthConfig;
import ai.nubase.common.config.oauth.OAuthProperties;
import ai.nubase.auth.dto.request.RefreshTokenRequest;
import ai.nubase.auth.dto.request.SignInRequest;
import ai.nubase.auth.dto.request.SignUpRequest;
import ai.nubase.auth.dto.response.AuthResponse;
import ai.nubase.auth.entity.Identity;
import ai.nubase.auth.entity.MfaAmrClaim;
import ai.nubase.auth.entity.OneTimeToken;
import ai.nubase.auth.entity.RefreshToken;
import ai.nubase.auth.entity.Session;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.IdentityRepository;
import ai.nubase.auth.repository.OneTimeTokenRepository;
import ai.nubase.auth.repository.SessionRepository;
import ai.nubase.auth.util.TokenGenerator;
import ai.nubase.auth.util.UserMapper;
import ai.nubase.auth.repository.UserRepository;
import ai.nubase.common.context.MultiTenancyContext;
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

@Service
@RequiredArgsConstructor
@Slf4j
public class AuthService {

    private final UserRepository userRepository;
    private final SessionRepository sessionRepository;
    private final IdentityRepository identityRepository;
    private final OneTimeTokenRepository oneTimeTokenRepository;
    private final PasswordService passwordService;
    private final JwtSecretService jwtSecretService;
    private final TokenService tokenService;
    private final EmailService emailService;
    private final TokenGenerator tokenGenerator;
    private final UserMapper userMapper;
    private final AuthConfig authConfig;
    private final EffectiveAuthConfig effectiveAuthConfig;
    private final AuthResponseFactory authResponseFactory;
    private final AuditService auditService;
    private final RateLimiterService rateLimiterService;
    private final CaptchaService captchaService;

    /**
     * Sign up a new user. An empty request (no email/password) signs in anonymously.
     */
    @Transactional
    public AuthResponse signUp(SignUpRequest request) {
        // Anonymous sign-in: Supabase clients POST /signup with no credentials.
        if (StringUtils.isBlank(request.getEmail()) && StringUtils.isBlank(request.getPassword())) {
            return signInAnonymously(request.getData());
        }

        if (effectiveAuthConfig.signupDisabled()) {
            throw new RuntimeException("Signups not allowed for this instance");
        }
        captchaService.verify(request.getCaptchaToken());
        rateLimiterService.checkRate("signup", request.getEmail());

        if (StringUtils.isBlank(request.getPassword())) {
            throw new IllegalArgumentException("Password is required");
        }

        // Check if user already exists
        if (userRepository.existsByEmail(request.getEmail())) {
            throw new RuntimeException("User already registered");
        }

        // Validate password strength
        passwordService.validatePasswordStrength(request.getPassword());

        // Hash password
        String hashedPassword = passwordService.hashPassword(request.getPassword());

        String role = Role.AUTHENTICATED.getValue();
        // Create user
        User user = User.builder()
                .email(request.getEmail())
                .encryptedPassword(hashedPassword)
                .phone(request.getPhone())
                .role(role)
                .aud(role)
                .rawUserMetaData(request.getData() != null ? request.getData() : new HashMap<>())
                .rawAppMetaData(createAppMetadata("email"))
                .isSuperAdmin(false)
                .isSsoUser(false)
                .isAnonymous(false)
                .build();

        // Generate confirmation token if email confirmation is required
        if (isEmailConfirmationRequired()) {
            String confirmationToken = tokenGenerator.generateSecureToken();
            user.setConfirmationToken(confirmationToken);
            user.setConfirmationSentAt(Instant.now());
        } else {
            user.setEmailConfirmedAt(Instant.now());
        }

        user = userRepository.save(user);

        // Create identity
        Identity identity = createIdentity(user, "email", request.getEmail());
        identityRepository.save(identity);

        // Send confirmation email if required
        if (isEmailConfirmationRequired()) {
            emailService.sendConfirmationEmail(user, user.getConfirmationToken(), request.getRedirectTo());
            auditService.record(AuditService.USER_CONFIRMATION_REQUESTED, user);
            return authResponseFactory.noToken(user);
        }

        auditService.record(AuditService.SIGNUP, user);
        // Create session and tokens
        return authResponseFactory.newSignIn(user, MfaAmrClaim.METHOD_PASSWORD);
    }

    /**
     * Anonymous sign-in: create a credential-less user and issue a session.
     */
    @Transactional
    public AuthResponse signInAnonymously(Map<String, Object> data) {
        String role = Role.AUTHENTICATED.getValue();
        User user = User.builder()
                .role(role)
                .aud(role)
                .rawUserMetaData(data != null ? data : new HashMap<>())
                .rawAppMetaData(createAppMetadata("anonymous"))
                .isSuperAdmin(false)
                .isSsoUser(false)
                .isAnonymous(true)
                .build();
        user = userRepository.save(user);

        auditService.record(AuditService.SIGNUP, user, Map.of("provider", "anonymous"));
        return authResponseFactory.newSignIn(user, MfaAmrClaim.METHOD_ANONYMOUS);
    }

    private boolean isEmailConfirmationRequired() {
        // tenant auth_config override → per-tenant OAuth flag → global default (true)
        return effectiveAuthConfig.emailConfirmationRequired();
    }

    /**
     * Sign in with email and password
     */
    @Transactional
    public AuthResponse signIn(SignInRequest request) {
        String email = request.getEmail();
        rateLimiterService.assertNotLockedOut(email);
        rateLimiterService.checkRate("token", email);

        // Find user by email
        User user = userRepository.findByEmail(email).orElse(null);

        // Verify password (record a failed attempt for non-existent users too,
        // to throttle enumeration, but keep the response message uniform)
        if (user == null || !passwordService.verifyPassword(request.getPassword(), user.getEncryptedPassword())) {
            rateLimiterService.recordFailure(email);
            auditService.record(AuditService.LOGIN_FAILED, user, Map.of("email", email));
            throw new RuntimeException("Invalid login credentials");
        }

        // Check if email is confirmed (if required)
        if (isEmailConfirmationRequired() && user.getEmailConfirmedAt() == null) {
            throw new RuntimeException("Email not confirmed");
        }

        // Check if user is banned
        if (user.getBannedUntil() != null && Instant.now().isBefore(user.getBannedUntil())) {
            throw new RuntimeException("User is banned");
        }

        rateLimiterService.recordSuccess(email);

        // Update last sign in
        user.setLastSignInAt(Instant.now());
        user = userRepository.save(user);

        // Update identity last sign in
        identityRepository.findByProviderAndProviderId("email", user.getId().toString())
                .ifPresent(identity -> {
                    identity.setLastSignInAt(Instant.now());
                    identityRepository.save(identity);
                });

        auditService.record(AuditService.LOGIN, user, Map.of("provider", "email"));
        // Create session and tokens (aal1; MFA verify upgrades to aal2)
        return authResponseFactory.newSignIn(user, MfaAmrClaim.METHOD_PASSWORD);
    }

    /**
     * Refresh access token using refresh token
     */
    @Transactional
    public AuthResponse refreshToken(RefreshTokenRequest request) {
        // Validate refresh token
        RefreshToken refreshToken = tokenService.validateRefreshToken(request.getRefreshToken());

        User user = refreshToken.getUser();
        Session session = refreshToken.getSession();

        // Update session refreshed_at
        session.setRefreshedAt(Instant.now());
        sessionRepository.save(session);

        // Rotate refresh token if enabled
        RefreshToken newRefreshToken = tokenService.rotateRefreshToken(refreshToken);

        // Generate new access token (amr/aal carried by the session)
        String accessToken = jwtSecretService.generateToken(user, session);

        // Build response
        List<Identity> identities = identityRepository.findByUserId(user.getId());
        var userResponse = userMapper.toUserResponse(user, identities);

        auditService.record(AuditService.TOKEN_REFRESHED, user);
        return AuthResponse.success(
                accessToken,
                newRefreshToken.getToken(),
                authConfig.getJwt().getExpiration(),
                userResponse
        );
    }

    /**
     * Logout user (revoke session and tokens)
     */
    @Transactional
    public void logout(User user) {
        // Revoke all user's refresh tokens
        tokenService.revokeAllUserTokens(user);
        auditService.record(AuditService.LOGOUT, user);
    }

    /**
     * Request password recovery
     */
    @Transactional
    public void recoverPassword(String email) {
        recoverPassword(email, null);
    }

    @Transactional
    public void recoverPassword(String email, String captchaToken) {
        captchaService.verify(captchaToken);
        rateLimiterService.checkRate("recover", email);

        userRepository.findByEmail(email).ifPresent(user -> {
            String recoveryToken = tokenGenerator.generateSecureToken();
            user.setRecoveryToken(recoveryToken);
            user.setRecoverySentAt(Instant.now());
            userRepository.save(user);

            emailService.sendRecoveryEmail(user, recoveryToken);
            auditService.record(AuditService.USER_RECOVERY_REQUESTED, user);
        });
        // Don't reveal if user exists or not
    }

    /**
     * Begin a reauthentication flow: issue a short-lived nonce (6-digit code) sent by
     * email, required before sensitive operations such as changing a password without
     * supplying the current one. Mirrors GoTrue's {@code POST /reauthenticate}.
     */
    @Transactional
    public void reauthenticate(User user) {
        String nonce = tokenGenerator.generateNumericOTP(effectiveAuthConfig.otp().getLength());
        // Store hashed nonce; also mirror onto the user row for compatibility.
        oneTimeTokenRepository.deleteByUserIdAndTokenType(user.getId(), OneTimeToken.TYPE_REAUTHENTICATION);
        oneTimeTokenRepository.save(OneTimeToken.builder()
                .user(user)
                .tokenType(OneTimeToken.TYPE_REAUTHENTICATION)
                .tokenHash(tokenGenerator.sha256(nonce))
                .relatesTo(user.getEmail())
                .build());
        user.setReauthenticationToken(tokenGenerator.sha256(nonce));
        user.setReauthenticationSentAt(Instant.now());
        userRepository.save(user);

        if (StringUtils.isNotBlank(user.getEmail())) {
            emailService.sendReauthenticationEmail(user, nonce);
        }
        auditService.record(AuditService.REAUTHENTICATION_REQUESTED, user);
    }

    /** Validate a reauthentication nonce previously issued to the user. */
    @Transactional(readOnly = true)
    public boolean verifyReauthenticationNonce(User user, String nonce) {
        if (StringUtils.isBlank(nonce) || user.getReauthenticationToken() == null) {
            return false;
        }
        if (user.getReauthenticationSentAt() != null) {
            Instant expiry = user.getReauthenticationSentAt()
                    .plus(effectiveAuthConfig.otp().getExpiration(), ChronoUnit.SECONDS);
            if (Instant.now().isAfter(expiry)) {
                return false;
            }
        }
        return user.getReauthenticationToken().equals(tokenGenerator.sha256(nonce));
    }

    /**
     * Verify email/phone with token
     */
    @Transactional
    public AuthResponse verify(String type, String token, String email) {
        User user;

        switch (type) {
            case "signup":
                user = userRepository.findByConfirmationToken(token)
                        .orElseThrow(() -> new RuntimeException("Invalid confirmation token"));

                if (!user.getEmail().equals(email)) {
                    throw new RuntimeException("Email mismatch");
                }

                // Check if token is expired
                Instant expirationTime = user.getConfirmationSentAt()
                        .plus(authConfig.getEmail().getConfirmationExpiration(), ChronoUnit.SECONDS);
                if (Instant.now().isAfter(expirationTime)) {
                    throw new RuntimeException("Confirmation token expired");
                }

                user.setEmailConfirmedAt(Instant.now());
                user.setConfirmationToken(null);
                user.setConfirmationSentAt(null);
                userRepository.save(user);

                auditService.record(AuditService.SIGNUP, user);
                return authResponseFactory.newSignIn(user, MfaAmrClaim.METHOD_PASSWORD);

            case "recovery":
                user = userRepository.findByRecoveryToken(token)
                        .orElseThrow(() -> new RuntimeException("Invalid recovery token"));

                // Token is valid, allow password reset
                // In a real implementation, you'd redirect to a password reset page
                return authResponseFactory.newSignIn(user, MfaAmrClaim.METHOD_OTP);

            case "invite":
                // Admin-invited users carry a confirmation_token; verifying it confirms the
                // account and establishes a session so they can set a password via PUT /user.
                user = userRepository.findByConfirmationToken(token)
                        .orElseThrow(() -> new RuntimeException("Invalid invite token"));
                user.setEmailConfirmedAt(Instant.now());
                user.setConfirmationToken(null);
                user.setConfirmationSentAt(null);
                userRepository.save(user);
                auditService.record(AuditService.LOGIN, user, Map.of("flow", "invite"));
                return authResponseFactory.newSignIn(user, MfaAmrClaim.METHOD_OTP);

            case "email_change":
                user = userRepository.findByEmailChangeTokenNew(token)
                        .orElseThrow(() -> new RuntimeException("Invalid email change token"));
                if (user.getEmailChangeSentAt() != null) {
                    Instant changeExpiry = user.getEmailChangeSentAt()
                            .plus(authConfig.getEmail().getConfirmationExpiration(), ChronoUnit.SECONDS);
                    if (Instant.now().isAfter(changeExpiry)) {
                        throw new RuntimeException("Email change token expired");
                    }
                }
                if (StringUtils.isNotBlank(user.getEmailChange())) {
                    user.setEmail(user.getEmailChange());   // apply the pending new email
                }
                user.setEmailChange(null);
                user.setEmailChangeTokenNew(null);
                user.setEmailChangeTokenCurrent(null);
                user.setEmailChangeConfirmStatus((short) 0);
                user.setEmailConfirmedAt(Instant.now());
                userRepository.save(user);
                auditService.record(AuditService.USER_MODIFIED, user, Map.of("action", "email_change"));
                return authResponseFactory.newSignIn(user, MfaAmrClaim.METHOD_PASSWORD);

            default:
                throw new RuntimeException("Invalid verification type");
        }
    }

    /**
     * Create identity for user
     */
    private Identity createIdentity(User user, String provider, String providerId) {
        Map<String, Object> identityData = new HashMap<>();
        identityData.put("sub", user.getId().toString());
        identityData.put("email", user.getEmail());

        return Identity.builder()
                .user(user)
                .provider(provider)
                .providerId(providerId)
                .identityData(identityData)
                .lastSignInAt(Instant.now())
                .build();
    }

    /**
     * Create app metadata
     */
    private Map<String, Object> createAppMetadata(String provider) {
        Map<String, Object> metadata = new HashMap<>();
        metadata.put("provider", provider);
        metadata.put("providers", List.of(provider));
        return metadata;
    }
}
