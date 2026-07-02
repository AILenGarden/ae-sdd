package ai.nubase.auth.service;

import ai.nubase.auth.dto.request.UpdateUserRequest;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.IdentityRepository;
import ai.nubase.auth.repository.UserRepository;
import ai.nubase.auth.util.TokenGenerator;
import ai.nubase.auth.util.UserMapper;
import ai.nubase.common.config.AuthConfig;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.time.Instant;
import java.util.List;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatCode;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.when;

/**
 * Unit tests for {@link UserService#updateUser} — focused on the reauthentication-nonce
 * enforcement on password change (the security-relevant branch).
 */
@DisplayName("UserService.updateUser (reauth nonce)")
class UserServiceTest {

    private final TokenGenerator tokenGenerator = new TokenGenerator();
    private UserRepository userRepository;
    private IdentityRepository identityRepository;
    private AuthConfig authConfig;
    private UserService svc;
    private UUID userId;

    @BeforeEach
    void setUp() {
        userRepository = mock(UserRepository.class);
        identityRepository = mock(IdentityRepository.class);
        authConfig = new AuthConfig();
        EffectiveAuthConfig eff = new EffectiveAuthConfig(authConfig);
        svc = new UserService(
                userRepository, identityRepository,
                new PasswordService(eff), mock(EmailService.class),
                tokenGenerator, new UserMapper(), eff, mock(OtpService.class));

        userId = UUID.randomUUID();
        when(identityRepository.findByUserId(userId)).thenReturn(List.of());
        when(userRepository.save(any(User.class))).thenAnswer(inv -> inv.getArgument(0));
    }

    private User userWithReauthToken(String nonce) {
        User u = User.builder().id(userId).email("u@test.local")
                .encryptedPassword("$2a$10$old").build();
        u.setReauthenticationToken(tokenGenerator.sha256(nonce));
        u.setReauthenticationSentAt(Instant.now());
        when(userRepository.findById(userId)).thenReturn(java.util.Optional.of(u));
        return u;
    }

    private UpdateUserRequest pwdReq(String password, String nonce) {
        UpdateUserRequest r = new UpdateUserRequest();
        r.setPassword(password);
        r.setNonce(nonce);
        return r;
    }

    @Test
    @DisplayName("with reauth required: correct nonce changes password and consumes the nonce")
    void correctNonce() {
        authConfig.getPassword().setRequireReauthentication(true);
        User u = userWithReauthToken("123456");

        svc.updateUser(userId, pwdReq("Password123!", "123456"));

        assertThat(u.getEncryptedPassword()).startsWith("$2"); // re-hashed
        assertThat(u.getReauthenticationToken()).isNull();     // consumed
        assertThat(u.getReauthenticationSentAt()).isNull();
    }

    @Test
    @DisplayName("with reauth required: wrong/missing nonce is rejected")
    void wrongNonce() {
        authConfig.getPassword().setRequireReauthentication(true);
        userWithReauthToken("123456");

        assertThatThrownBy(() -> svc.updateUser(userId, pwdReq("Password123!", "000000")))
                .isInstanceOf(RuntimeException.class).hasMessageContaining("Reauthentication required");
        assertThatThrownBy(() -> svc.updateUser(userId, pwdReq("Password123!", null)))
                .isInstanceOf(RuntimeException.class).hasMessageContaining("Reauthentication required");
    }

    @Test
    @DisplayName("with reauth disabled: password change needs no nonce")
    void disabledNoNonce() {
        authConfig.getPassword().setRequireReauthentication(false);
        User u = User.builder().id(userId).email("u@test.local").encryptedPassword("$2a$10$old").build();
        when(userRepository.findById(userId)).thenReturn(java.util.Optional.of(u));

        assertThatCode(() -> svc.updateUser(userId, pwdReq("Password123!", null))).doesNotThrowAnyException();
        assertThat(u.getEncryptedPassword()).startsWith("$2");
    }
}
