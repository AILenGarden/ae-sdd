package ai.nubase.auth.service;

import ai.nubase.common.config.AuthConfig;
import ai.nubase.auth.entity.RefreshToken;
import ai.nubase.auth.entity.Session;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.util.TokenGenerator;
import ai.nubase.auth.repository.RefreshTokenRepository;
import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.time.temporal.ChronoUnit;

@Service
@RequiredArgsConstructor
public class TokenService {

    private final RefreshTokenRepository refreshTokenRepository;
    private final TokenGenerator tokenGenerator;
    private final AuthConfig authConfig;

    /**
     * Generate a new refresh token
     */
    @Transactional
    public RefreshToken generateRefreshToken(User user, Session session) {
        String tokenString = tokenGenerator.generateRefreshToken();

        RefreshToken refreshToken = RefreshToken.builder()
                .token(tokenString)
                .user(user)
                .session(session)
                .revoked(false)
                .build();

        return refreshTokenRepository.save(refreshToken);
    }

    /**
     * Validate and retrieve a refresh token
     */
    @Transactional(readOnly = true)
    public RefreshToken validateRefreshToken(String tokenString) {
        RefreshToken refreshToken = refreshTokenRepository.findByTokenAndRevokedFalse(tokenString)
                .orElseThrow(() -> new RuntimeException("Invalid or revoked refresh token"));

        // Check if token is expired
        Instant expirationTime = refreshToken.getCreatedAt()
                .plus(authConfig.getRefreshToken().getExpiration(), ChronoUnit.SECONDS);

        if (Instant.now().isAfter(expirationTime)) {
            throw new RuntimeException("Refresh token expired");
        }

        return refreshToken;
    }

    /**
     * Rotate refresh token (create new one and revoke old one)
     */
    @Transactional
    public RefreshToken rotateRefreshToken(RefreshToken oldToken) {
        if (!authConfig.getRefreshToken().isRotation()) {
            return oldToken;
        }

        // Create new refresh token
        String newTokenString = tokenGenerator.generateRefreshToken();

        RefreshToken newToken = RefreshToken.builder()
                .token(newTokenString)
                .user(oldToken.getUser())
                .session(oldToken.getSession())
                .parent(oldToken.getToken())  // Track the parent token
                .revoked(false)
                .build();

        // Revoke old token
        oldToken.setRevoked(true);
        refreshTokenRepository.save(oldToken);

        return refreshTokenRepository.save(newToken);
    }

    /**
     * Revoke a refresh token
     */
    @Transactional
    public void revokeRefreshToken(String tokenString) {
        refreshTokenRepository.findByToken(tokenString).ifPresent(token -> {
            token.setRevoked(true);
            refreshTokenRepository.save(token);
        });
    }

    /**
     * Revoke all refresh tokens for a user
     */
    @Transactional
    public void revokeAllUserTokens(User user) {
        refreshTokenRepository.findByUser(user).forEach(token -> {
            token.setRevoked(true);
            refreshTokenRepository.save(token);
        });
    }

    /**
     * Revoke all refresh tokens for a session
     */
    @Transactional
    public void revokeAllSessionTokens(Session session) {
        refreshTokenRepository.findBySession(session).forEach(token -> {
            token.setRevoked(true);
            refreshTokenRepository.save(token);
        });
    }
}
