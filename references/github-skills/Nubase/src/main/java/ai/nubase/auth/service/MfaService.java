package ai.nubase.auth.service;

import ai.nubase.auth.dto.response.AuthResponse;
import ai.nubase.auth.dto.response.mfa.ChallengeResponse;
import ai.nubase.auth.dto.response.mfa.EnrollFactorResponse;
import ai.nubase.auth.dto.response.mfa.FactorResponse;
import ai.nubase.auth.entity.MfaChallenge;
import ai.nubase.auth.entity.MfaAmrClaim;
import ai.nubase.auth.entity.MfaFactor;
import ai.nubase.auth.entity.Session;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.MfaChallengeRepository;
import ai.nubase.auth.repository.MfaFactorRepository;
import ai.nubase.auth.repository.SessionRepository;
import ai.nubase.common.config.AuthConfig;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang3.StringUtils;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.stream.Collectors;

/**
 * Multi-factor authentication: enroll / challenge / verify / unenroll TOTP and phone factors.
 * Mirrors Supabase GoTrue's {@code /factors} API and AAL2 session upgrade.
 */
@Service
@RequiredArgsConstructor
@Slf4j
public class MfaService {

    private final MfaFactorRepository factorRepository;
    private final MfaChallengeRepository challengeRepository;
    private final SessionRepository sessionRepository;
    private final TotpService totpService;
    private final SmsService smsService;
    private final AuthResponseFactory authResponseFactory;
    private final AuditService auditService;
    private final RateLimiterService rateLimiterService;
    private final EffectiveAuthConfig effectiveAuthConfig;

    /** List the factors enrolled for a user. */
    @Transactional(readOnly = true)
    public List<FactorResponse> listFactors(User user) {
        return factorRepository.findByUserId(user.getId()).stream()
                .map(f -> FactorResponse.builder()
                        .id(f.getId().toString())
                        .friendlyName(f.getFriendlyName())
                        .factorType(f.getFactorType())
                        .status(f.getStatus())
                        .phone(f.getPhone())
                        .createdAt(f.getCreatedAt())
                        .updatedAt(f.getUpdatedAt())
                        .build())
                .collect(Collectors.toList());
    }

    /** Enroll a new (unverified) factor. */
    @Transactional
    public EnrollFactorResponse enroll(User user, String factorType, String friendlyName, String phone) {
        if (!effectiveAuthConfig.mfa().isEnabled()) {
            throw new IllegalArgumentException("MFA is disabled");
        }
        long verified = factorRepository.countByUserIdAndStatus(user.getId(), MfaFactor.STATUS_VERIFIED);
        if (verified >= effectiveAuthConfig.mfa().getMaxEnrolledFactors()) {
            throw new IllegalArgumentException("Maximum number of enrolled factors reached");
        }

        String type = StringUtils.isBlank(factorType) ? MfaFactor.TYPE_TOTP : factorType;
        MfaFactor.MfaFactorBuilder builder = MfaFactor.builder()
                .user(user)
                .friendlyName(friendlyName)
                .factorType(type)
                .status(MfaFactor.STATUS_UNVERIFIED);

        if (MfaFactor.TYPE_TOTP.equals(type)) {
            String secret = totpService.generateSecret();
            builder.secret(secret);
            MfaFactor factor = factorRepository.save(builder.build());

            String account = StringUtils.isNotBlank(user.getEmail())
                    ? user.getEmail() : user.getId().toString();
            String uri = totpService.buildOtpAuthUri(account, secret);

            auditService.record(AuditService.FACTOR_ENROLLED, user, Map.of("factor_type", type));
            return EnrollFactorResponse.builder()
                    .id(factor.getId().toString())
                    .type(type)
                    .friendlyName(friendlyName)
                    .totp(EnrollFactorResponse.Totp.builder()
                            .secret(secret)
                            .uri(uri)
                            .qrCode(uri)
                            .build())
                    .build();
        } else if (MfaFactor.TYPE_PHONE.equals(type)) {
            if (StringUtils.isBlank(phone)) {
                throw new IllegalArgumentException("phone is required for a phone factor");
            }
            builder.phone(phone);
            MfaFactor factor = factorRepository.save(builder.build());
            auditService.record(AuditService.FACTOR_ENROLLED, user, Map.of("factor_type", type));
            return EnrollFactorResponse.builder()
                    .id(factor.getId().toString())
                    .type(type)
                    .friendlyName(friendlyName)
                    .phone(phone)
                    .build();
        }
        throw new IllegalArgumentException("Unsupported factor_type: " + type);
    }

    /** Create a challenge for a factor. For phone factors this dispatches an SMS code. */
    @Transactional
    public ChallengeResponse challenge(User user, String factorId) {
        MfaFactor factor = requireFactor(user, factorId);
        rateLimiterService.checkRate("mfa_challenge", factorId);

        MfaChallenge.MfaChallengeBuilder builder = MfaChallenge.builder().factor(factor);
        if (MfaFactor.TYPE_PHONE.equals(factor.getFactorType())) {
            String code = totpService_phoneCode();
            builder.otpCode(code);
            smsService.sendOtp(factor.getPhone(), code);
        }
        MfaChallenge challenge = challengeRepository.save(builder.build());

        factor.setLastChallengedAt(Instant.now());
        factorRepository.save(factor);

        long expiresAt = challenge.getCreatedAt()
                .plus(effectiveAuthConfig.mfa().getChallengeExpiration(), ChronoUnit.SECONDS)
                .getEpochSecond();
        auditService.record(AuditService.CHALLENGE_CREATED, user, Map.of("factor_id", factorId));
        return ChallengeResponse.builder()
                .id(challenge.getId().toString())
                .type(factor.getFactorType())
                .expiresAt(expiresAt)
                .build();
    }

    /**
     * Verify a code against a factor's challenge. On success the factor becomes verified,
     * the current session is upgraded to AAL2, and fresh tokens are issued.
     *
     * @param sessionId the session backing the caller's current access token
     */
    @Transactional
    public AuthResponse verify(User user, String factorId, String challengeId, String code, String sessionId) {
        MfaFactor factor = requireFactor(user, factorId);
        rateLimiterService.checkRate("mfa_verify", factorId);

        MfaChallenge challenge = null;
        if (StringUtils.isNotBlank(challengeId)) {
            challenge = challengeRepository.findByIdAndFactorId(
                            UUID.fromString(challengeId), factor.getId())
                    .orElseThrow(() -> new RuntimeException("Challenge not found"));
            Instant expiry = challenge.getCreatedAt()
                    .plus(effectiveAuthConfig.mfa().getChallengeExpiration(), ChronoUnit.SECONDS);
            if (Instant.now().isAfter(expiry)) {
                throw new RuntimeException("MFA challenge has expired");
            }
        }

        boolean ok;
        if (MfaFactor.TYPE_TOTP.equals(factor.getFactorType())) {
            ok = totpService.verifyCode(factor.getSecret(), code);
        } else if (MfaFactor.TYPE_PHONE.equals(factor.getFactorType())) {
            ok = challenge != null && challenge.getOtpCode() != null
                    && constantTimeEquals(challenge.getOtpCode(), code.trim());
        } else {
            throw new IllegalArgumentException("Unsupported factor type");
        }

        if (!ok) {
            auditService.record(AuditService.LOGIN_FAILED, user, Map.of("factor_id", factorId, "reason", "invalid_mfa_code"));
            throw new RuntimeException("Invalid MFA code");
        }

        // Mark factor verified + challenge consumed.
        factor.setStatus(MfaFactor.STATUS_VERIFIED);
        factorRepository.save(factor);
        if (challenge != null) {
            challenge.setVerifiedAt(Instant.now());
            challengeRepository.save(challenge);
        }

        // Upgrade the current session to AAL2 and record the TOTP/phone AMR.
        Session session = resolveSession(user, sessionId);
        session.setAal(AuthResponseFactory.AAL2);
        session.setFactorId(factor.getId());
        sessionRepository.save(session);
        authResponseFactory.recordAmr(session, MfaAmrClaim.METHOD_TOTP);

        auditService.record(AuditService.FACTOR_VERIFIED, user, Map.of("factor_id", factorId));
        // Issue fresh tokens carrying aal2 + the new amr claim.
        return authResponseFactory.issueTokens(user, session);
    }

    /** Unenroll (delete) a factor. */
    @Transactional
    public String unenroll(User user, String factorId) {
        MfaFactor factor = requireFactor(user, factorId);
        factorRepository.delete(factor);
        auditService.record(AuditService.FACTOR_UNENROLLED, user, Map.of("factor_id", factorId));
        return factor.getId().toString();
    }

    private MfaFactor requireFactor(User user, String factorId) {
        return factorRepository.findByIdAndUserId(UUID.fromString(factorId), user.getId())
                .orElseThrow(() -> new RuntimeException("MFA factor not found"));
    }

    private Session resolveSession(User user, String sessionId) {
        if (StringUtils.isNotBlank(sessionId)) {
            Session session = sessionRepository.findById(UUID.fromString(sessionId)).orElse(null);
            if (session != null && session.getUser().getId().equals(user.getId())) {
                return session;
            }
        }
        // Fallback: most recent session for the user.
        return sessionRepository.findByUserId(user.getId()).stream()
                .max((a, b) -> a.getCreatedAt().compareTo(b.getCreatedAt()))
                .orElseGet(() -> authResponseFactory.createSession(
                        user, AuthResponseFactory.AAL1, MfaAmrClaim.METHOD_PASSWORD));
    }

    private String totpService_phoneCode() {
        // Reuse the OTP length config for phone-MFA codes.
        StringBuilder sb = new StringBuilder();
        java.security.SecureRandom random = new java.security.SecureRandom();
        for (int i = 0; i < effectiveAuthConfig.otp().getLength(); i++) {
            sb.append(random.nextInt(10));
        }
        return sb.toString();
    }

    private boolean constantTimeEquals(String a, String b) {
        if (a == null || b == null || a.length() != b.length()) {
            return false;
        }
        int result = 0;
        for (int i = 0; i < a.length(); i++) {
            result |= a.charAt(i) ^ b.charAt(i);
        }
        return result == 0;
    }
}
