package ai.nubase.auth.service;

import ai.nubase.auth.entity.RefreshToken;
import ai.nubase.auth.entity.Session;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.RefreshTokenRepository;
import ai.nubase.auth.util.TokenGenerator;
import ai.nubase.common.config.AuthConfig;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.time.Instant;
import java.util.List;
import java.util.Optional;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.*;

/**
 * Unit tests for {@link TokenService}: validation/expiry, rotation, and revocation.
 */
@DisplayName("TokenService")
class TokenServiceTest {

    private RefreshTokenRepository repo;
    private AuthConfig authConfig;
    private TokenService svc;

    @BeforeEach
    void setUp() {
        repo = mock(RefreshTokenRepository.class);
        authConfig = new AuthConfig();
        svc = new TokenService(repo, new TokenGenerator(), authConfig);
        when(repo.save(any())).thenAnswer(inv -> inv.getArgument(0));
    }

    private RefreshToken rt(String token, Instant createdAt) {
        RefreshToken t = RefreshToken.builder()
                .token(token).user(User.builder().id(UUID.randomUUID()).build())
                .session(Session.builder().id(UUID.randomUUID()).build()).revoked(false).build();
        t.setCreatedAt(createdAt);
        return t;
    }

    @Test
    @DisplayName("generateRefreshToken persists a v1-prefixed token")
    void generate() {
        RefreshToken t = svc.generateRefreshToken(
                User.builder().id(UUID.randomUUID()).build(), Session.builder().id(UUID.randomUUID()).build());
        assertThat(t.getToken()).startsWith("v1.");
        assertThat(t.getRevoked()).isFalse();
        verify(repo).save(any(RefreshToken.class));
    }

    @Test
    @DisplayName("validateRefreshToken: unknown/revoked → error")
    void validateUnknown() {
        when(repo.findByTokenAndRevokedFalse("x")).thenReturn(Optional.empty());
        assertThatThrownBy(() -> svc.validateRefreshToken("x"))
                .isInstanceOf(RuntimeException.class).hasMessageContaining("Invalid or revoked");
    }

    @Test
    @DisplayName("validateRefreshToken: expired → error")
    void validateExpired() {
        when(repo.findByTokenAndRevokedFalse("old"))
                .thenReturn(Optional.of(rt("old", Instant.now().minusSeconds(3_000_000))));
        assertThatThrownBy(() -> svc.validateRefreshToken("old"))
                .isInstanceOf(RuntimeException.class).hasMessageContaining("expired");
    }

    @Test
    @DisplayName("validateRefreshToken: fresh token returns it")
    void validateOk() {
        RefreshToken t = rt("good", Instant.now());
        when(repo.findByTokenAndRevokedFalse("good")).thenReturn(Optional.of(t));
        assertThat(svc.validateRefreshToken("good")).isSameAs(t);
    }

    @Test
    @DisplayName("rotateRefreshToken: rotation on → revokes old, issues child")
    void rotateOn() {
        authConfig.getRefreshToken().setRotation(true);
        RefreshToken old = rt("old", Instant.now());

        RefreshToken next = svc.rotateRefreshToken(old);

        assertThat(old.getRevoked()).isTrue();
        assertThat(next.getToken()).isNotEqualTo("old");
        assertThat(next.getParent()).isEqualTo("old");
        verify(repo, times(2)).save(any()); // old + new
    }

    @Test
    @DisplayName("rotateRefreshToken: rotation off → same token, no writes")
    void rotateOff() {
        authConfig.getRefreshToken().setRotation(false);
        RefreshToken old = rt("old", Instant.now());
        assertThat(svc.rotateRefreshToken(old)).isSameAs(old);
        verify(repo, never()).save(any());
    }

    @Test
    @DisplayName("revokeAllUserTokens marks every token revoked")
    void revokeAll() {
        User u = User.builder().id(UUID.randomUUID()).build();
        RefreshToken a = rt("a", Instant.now());
        RefreshToken b = rt("b", Instant.now());
        when(repo.findByUser(u)).thenReturn(List.of(a, b));

        svc.revokeAllUserTokens(u);

        assertThat(a.getRevoked()).isTrue();
        assertThat(b.getRevoked()).isTrue();
        verify(repo, times(2)).save(any());
    }
}
