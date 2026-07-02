package ai.nubase.auth.service;

import ai.nubase.auth.dto.request.SignInRequest;
import ai.nubase.auth.dto.request.SignUpRequest;
import ai.nubase.auth.dto.response.AuthResponse;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.UserRepository;
import ai.nubase.common.config.AuthConfig;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.util.Optional;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.*;

/**
 * Unit tests for {@link AuthService#signIn} / {@link AuthService#signUp} guard logic
 * (lockout, bad credentials, signup-disabled), with collaborators mocked.
 */
@DisplayName("AuthService guards")
class AuthServiceTest {

    private UserRepository userRepository;
    private PasswordService passwordService;
    private AuthResponseFactory authResponseFactory;
    private RateLimiterService rateLimiterService;
    private CaptchaService captchaService;
    private EffectiveAuthConfig effectiveAuthConfig;
    private AuthService svc;

    private final AuthResponse sentinel = AuthResponse.success("acc", "ref", 3600, null);

    @BeforeEach
    void setUp() {
        userRepository = mock(UserRepository.class);
        passwordService = mock(PasswordService.class);
        authResponseFactory = mock(AuthResponseFactory.class);
        rateLimiterService = mock(RateLimiterService.class);
        captchaService = mock(CaptchaService.class);
        effectiveAuthConfig = mock(EffectiveAuthConfig.class);

        svc = new AuthService(
                userRepository, mock(ai.nubase.auth.repository.SessionRepository.class),
                mock(ai.nubase.auth.repository.IdentityRepository.class),
                mock(ai.nubase.auth.repository.OneTimeTokenRepository.class),
                passwordService, mock(JwtSecretService.class), mock(TokenService.class),
                mock(EmailService.class), mock(ai.nubase.auth.util.TokenGenerator.class),
                mock(ai.nubase.auth.util.UserMapper.class), new AuthConfig(), effectiveAuthConfig,
                authResponseFactory, mock(AuditService.class), rateLimiterService, captchaService);

        when(authResponseFactory.newSignIn(any(), anyString())).thenReturn(sentinel);
    }

    @Test
    @DisplayName("signIn: locked-out identity is rejected before credential check")
    void lockout() {
        doThrow(new RateLimiterService.RateLimitExceededException("locked"))
                .when(rateLimiterService).assertNotLockedOut("a@b.com");

        assertThatThrownBy(() -> svc.signIn(new SignInRequest("a@b.com", "pw")))
                .isInstanceOf(RateLimiterService.RateLimitExceededException.class);
        verify(passwordService, never()).verifyPassword(anyString(), anyString());
    }

    @Test
    @DisplayName("signIn: unknown user records a failure and returns a uniform error")
    void unknownUser() {
        when(userRepository.findByEmail("a@b.com")).thenReturn(Optional.empty());

        assertThatThrownBy(() -> svc.signIn(new SignInRequest("a@b.com", "pw")))
                .isInstanceOf(RuntimeException.class).hasMessageContaining("Invalid login credentials");
        verify(rateLimiterService).recordFailure("a@b.com");
    }

    @Test
    @DisplayName("signIn: wrong password records a failure")
    void wrongPassword() {
        User u = User.builder().id(UUID.randomUUID()).email("a@b.com").encryptedPassword("$2a$10$x").build();
        when(userRepository.findByEmail("a@b.com")).thenReturn(Optional.of(u));
        when(passwordService.verifyPassword("pw", "$2a$10$x")).thenReturn(false);

        assertThatThrownBy(() -> svc.signIn(new SignInRequest("a@b.com", "pw")))
                .isInstanceOf(RuntimeException.class).hasMessageContaining("Invalid login credentials");
        verify(rateLimiterService).recordFailure("a@b.com");
        verify(authResponseFactory, never()).newSignIn(any(), anyString());
    }

    @Test
    @DisplayName("signIn: valid credentials clear failures and issue a session")
    void success() {
        User u = User.builder().id(UUID.randomUUID()).email("a@b.com")
                .encryptedPassword("$2a$10$x").emailConfirmedAt(java.time.Instant.now()).build();
        when(userRepository.findByEmail("a@b.com")).thenReturn(Optional.of(u));
        when(passwordService.verifyPassword("pw", "$2a$10$x")).thenReturn(true);
        when(effectiveAuthConfig.emailConfirmationRequired()).thenReturn(false);
        when(userRepository.save(any())).thenAnswer(inv -> inv.getArgument(0));

        AuthResponse res = svc.signIn(new SignInRequest("a@b.com", "pw"));

        assertThat(res).isSameAs(sentinel);
        verify(rateLimiterService).recordSuccess("a@b.com");
    }

    @Test
    @DisplayName("signUp: disabled signup is rejected")
    void signupDisabled() {
        when(effectiveAuthConfig.signupDisabled()).thenReturn(true);
        SignUpRequest req = new SignUpRequest();
        req.setEmail("new@b.com");
        req.setPassword("Password123!");

        assertThatThrownBy(() -> svc.signUp(req))
                .isInstanceOf(RuntimeException.class).hasMessageContaining("Signups not allowed");
        verify(userRepository, never()).save(any());
    }
}
