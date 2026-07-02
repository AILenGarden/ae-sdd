package ai.nubase.auth.service;

import ai.nubase.auth.entity.Identity;
import ai.nubase.auth.entity.MfaAmrClaim;
import ai.nubase.auth.entity.RefreshToken;
import ai.nubase.auth.entity.Session;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.dto.response.AuthResponse;
import ai.nubase.auth.dto.response.UserResponse;
import ai.nubase.auth.repository.IdentityRepository;
import ai.nubase.auth.repository.MfaAmrClaimRepository;
import ai.nubase.auth.repository.SessionRepository;
import ai.nubase.auth.util.UserMapper;
import ai.nubase.common.config.AuthConfig;
import jakarta.servlet.http.HttpServletRequest;
import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Component;
import org.springframework.web.context.request.RequestContextHolder;
import org.springframework.web.context.request.ServletRequestAttributes;

import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.List;

/**
 * Builds {@link Session} rows (with their {@code amr} claim) and the resulting
 * {@link AuthResponse} (access + refresh token + user payload). Shared by the
 * password, OAuth, passwordless (OTP/magic-link), anonymous and MFA flows so the
 * session/AMR/token-issuance logic lives in exactly one place.
 */
@Component
@RequiredArgsConstructor
public class AuthResponseFactory {

    public static final String AAL1 = "aal1";
    public static final String AAL2 = "aal2";

    private final SessionRepository sessionRepository;
    private final MfaAmrClaimRepository mfaAmrClaimRepository;
    private final IdentityRepository identityRepository;
    private final JwtSecretService jwtSecretService;
    private final TokenService tokenService;
    private final UserMapper userMapper;
    private final AuthConfig authConfig;

    /**
     * Create a new session at the given assurance level and record the initial AMR method.
     */
    public Session createSession(User user, String aal, String amrMethod) {
        HttpServletRequest request = currentRequest();
        Instant now = Instant.now();
        Instant notAfter = now.plus(authConfig.getJwt().getExpiration(), ChronoUnit.SECONDS);

        Session session = Session.builder()
                .user(user)
                .aal(aal != null ? aal : AAL1)
                .notAfter(notAfter)
                .userAgent(request != null ? request.getHeader("User-Agent") : null)
                .ip(request != null ? clientIp(request) : null)
                .build();
        session = sessionRepository.save(session);

        if (amrMethod != null) {
            recordAmr(session, amrMethod);
        }
        return session;
    }

    /** Idempotently add an AMR (authentication method reference) row to a session. */
    public void recordAmr(Session session, String method) {
        boolean exists = mfaAmrClaimRepository.findBySessionId(session.getId()).stream()
                .anyMatch(c -> method.equals(c.getAuthenticationMethod()));
        if (!exists) {
            mfaAmrClaimRepository.save(MfaAmrClaim.builder()
                    .session(session)
                    .authenticationMethod(method)
                    .build());
        }
    }

    /** Issue access + refresh tokens for an existing session and build the full response. */
    public AuthResponse issueTokens(User user, Session session) {
        String accessToken = jwtSecretService.generateToken(user, session);
        RefreshToken refreshToken = tokenService.generateRefreshToken(user, session);

        List<Identity> identities = identityRepository.findByUserId(user.getId());
        UserResponse userResponse = userMapper.toUserResponse(user, identities);

        return AuthResponse.success(
                accessToken,
                refreshToken.getToken(),
                authConfig.getJwt().getExpiration(),
                userResponse
        );
    }

    /** Convenience: create an aal1 session for {@code amrMethod} and issue tokens. */
    public AuthResponse newSignIn(User user, String amrMethod) {
        Session session = createSession(user, AAL1, amrMethod);
        return issueTokens(user, session);
    }

    /** Build a token-less response (e.g. signup that still requires confirmation). */
    public AuthResponse noToken(User user) {
        List<Identity> identities = identityRepository.findByUserId(user.getId());
        UserResponse userResponse = userMapper.toUserResponse(user, identities);
        return AuthResponse.success(null, null, 0, userResponse);
    }

    private HttpServletRequest currentRequest() {
        ServletRequestAttributes attributes =
                (ServletRequestAttributes) RequestContextHolder.getRequestAttributes();
        return attributes != null ? attributes.getRequest() : null;
    }

    private String clientIp(HttpServletRequest request) {
        String xForwardedFor = request.getHeader("X-Forwarded-For");
        if (xForwardedFor != null && !xForwardedFor.isEmpty()) {
            return xForwardedFor.split(",")[0].strip();
        }
        return request.getRemoteAddr();
    }
}
