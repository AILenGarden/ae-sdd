package ai.nubase.auth.service;

import ai.nubase.auth.dto.request.admin.GenerateLinkRequest;
import ai.nubase.auth.dto.response.admin.GenerateLinkResponse;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.IdentityRepository;
import ai.nubase.auth.repository.OneTimeTokenRepository;
import ai.nubase.auth.repository.UserRepository;
import ai.nubase.auth.util.TokenGenerator;
import ai.nubase.auth.util.UserMapper;
import ai.nubase.common.config.AuthConfig;
import ai.nubase.common.context.MultiTenancyContext;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.*;

/**
 * Unit tests for {@link AdminLinkService#generate}: link shapes for recovery / magiclink / signup.
 */
@DisplayName("AdminLinkService.generate")
class AdminLinkServiceTest {

    private UserRepository userRepository;
    private OneTimeTokenRepository ottRepo;
    private IdentityRepository identityRepository;
    private AdminLinkService svc;

    @BeforeEach
    void setUp() {
        userRepository = mock(UserRepository.class);
        ottRepo = mock(OneTimeTokenRepository.class);
        identityRepository = mock(IdentityRepository.class);
        AuthConfig authConfig = new AuthConfig();
        svc = new AdminLinkService(userRepository, identityRepository, ottRepo,
                new PasswordService(new EffectiveAuthConfig(authConfig)),
                new EffectiveAuthConfig(authConfig), new TokenGenerator(), new UserMapper(), authConfig);

        when(identityRepository.findByUserId(any())).thenReturn(List.of());
        when(userRepository.save(any())).thenAnswer(inv -> {
            User u = inv.getArgument(0);
            if (u.getId() == null) u.setId(UUID.randomUUID());
            return u;
        });
        MultiTenancyContext.setContext(MultiTenancyContext.ContextData.builder()
                .appCode("demo").schemaName("public").jwtSecret("s").apikey("APIKEY").build());
    }

    @AfterEach
    void clear() {
        MultiTenancyContext.clear();
    }

    private GenerateLinkRequest req(String type, String email) {
        GenerateLinkRequest r = new GenerateLinkRequest();
        r.setType(type);
        r.setEmail(email);
        return r;
    }

    @Test
    @DisplayName("recovery: sets recovery_token and builds a verify link")
    void recovery() {
        User u = User.builder().id(UUID.randomUUID()).email("a@b.com").build();
        when(userRepository.findByEmail("a@b.com")).thenReturn(Optional.of(u));

        GenerateLinkResponse res = svc.generate(req("recovery", "a@b.com"));

        assertThat(res.getVerificationType()).isEqualTo("recovery");
        assertThat(u.getRecoveryToken()).isEqualTo(res.getToken());
        assertThat(res.getActionLink())
                .contains("/auth/v1/verify").contains("type=recovery")
                .contains("email=a@b.com").contains("apikey=APIKEY").contains("token=" + res.getToken());
    }

    @Test
    @DisplayName("recovery: unknown email is rejected")
    void recoveryUnknown() {
        when(userRepository.findByEmail("x@b.com")).thenReturn(Optional.empty());
        assertThatThrownBy(() -> svc.generate(req("recovery", "x@b.com")))
                .isInstanceOf(RuntimeException.class).hasMessageContaining("User not found");
    }

    @Test
    @DisplayName("magiclink: returns an email OTP and a magiclink action link")
    void magiclink() {
        User u = User.builder().id(UUID.randomUUID()).email("a@b.com").build();
        when(userRepository.findByEmail("a@b.com")).thenReturn(Optional.of(u));

        GenerateLinkResponse res = svc.generate(req("magiclink", "a@b.com"));

        assertThat(res.getVerificationType()).isEqualTo("magiclink");
        assertThat(res.getEmailOtp()).isNotBlank();
        assertThat(res.getActionLink()).contains("type=magiclink");
        verify(ottRepo, atLeastOnce()).save(any());
    }

    @Test
    @DisplayName("signup: auto-creates the user and sets a confirmation token")
    void signup() {
        when(userRepository.findByEmail("new@b.com")).thenReturn(Optional.empty());

        GenerateLinkResponse res = svc.generate(req("signup", "new@b.com"));

        assertThat(res.getVerificationType()).isEqualTo("signup");
        assertThat(res.getActionLink()).contains("type=signup").contains("email=new@b.com");
        verify(identityRepository).save(any()); // email identity created
    }

    @Test
    @DisplayName("unsupported type is rejected")
    void unsupported() {
        assertThatThrownBy(() -> svc.generate(req("sms", "a@b.com")))
                .isInstanceOf(IllegalArgumentException.class).hasMessageContaining("Unsupported generate_link type");
    }
}
