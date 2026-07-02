package ai.nubase.auth.service;

import ai.nubase.auth.dto.response.AuthResponse;
import ai.nubase.auth.entity.OneTimeToken;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.OneTimeTokenRepository;
import ai.nubase.auth.repository.UserRepository;
import ai.nubase.auth.util.TokenGenerator;
import ai.nubase.common.config.AuthConfig;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.time.Instant;
import java.util.Optional;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.*;

/**
 * Unit tests for {@link OtpService#verifyFlow}: expiry, relates_to match, single-use, PKCE branch.
 */
@DisplayName("OtpService.verifyFlow")
class OtpServiceTest {

    private final TokenGenerator tokenGenerator = new TokenGenerator();
    private OneTimeTokenRepository ottRepo;
    private UserRepository userRepository;
    private AuthResponseFactory authResponseFactory;
    private PkceService pkceService;
    private OtpService svc;

    private final AuthResponse sentinel = AuthResponse.success("acc", "ref", 3600, null);

    @BeforeEach
    void setUp() {
        ottRepo = mock(OneTimeTokenRepository.class);
        userRepository = mock(UserRepository.class);
        authResponseFactory = mock(AuthResponseFactory.class);
        pkceService = mock(PkceService.class);
        svc = new OtpService(
                userRepository, mock(ai.nubase.auth.repository.IdentityRepository.class),
                ottRepo, tokenGenerator, mock(EmailService.class), mock(SmsService.class),
                new EffectiveAuthConfig(new AuthConfig()), authResponseFactory,
                mock(AuditService.class), mock(RateLimiterService.class), mock(CaptchaService.class),
                pkceService);
        when(authResponseFactory.newSignIn(any(), anyString())).thenReturn(sentinel);
        when(userRepository.save(any())).thenAnswer(inv -> inv.getArgument(0));
    }

    private OneTimeToken token(String value, String type, String relatesTo, Instant createdAt, String challenge) {
        User u = User.builder().id(UUID.randomUUID()).email("a@b.com").build();
        OneTimeToken t = OneTimeToken.builder()
                .user(u).tokenType(type).tokenHash(tokenGenerator.sha256(value))
                .relatesTo(relatesTo).codeChallenge(challenge).build();
        t.setCreatedAt(createdAt);
        return t;
    }

    @Test
    @DisplayName("valid magic link establishes a session and is single-use")
    void valid() {
        OneTimeToken t = token("tok", OneTimeToken.TYPE_MAGICLINK, "a@b.com", Instant.now(), null);
        when(ottRepo.findByTokenHashAndTokenType(tokenGenerator.sha256("tok"), OneTimeToken.TYPE_MAGICLINK))
                .thenReturn(Optional.of(t));

        OtpService.VerifyResult r = svc.verifyFlow("magiclink", "tok", "a@b.com", null);

        assertThat(r.isPkce()).isFalse();
        assertThat(r.session()).isSameAs(sentinel);
        assertThat(t.getUser().getEmailConfirmedAt()).isNotNull();
        // single-use: all passwordless tokens for the user are invalidated
        verify(ottRepo).deleteByUserIdAndTokenType(t.getUser().getId(), OneTimeToken.TYPE_MAGICLINK);
        verify(ottRepo).deleteByUserIdAndTokenType(t.getUser().getId(), OneTimeToken.TYPE_OTP);
    }

    @Test
    @DisplayName("expired token is deleted and rejected")
    void expired() {
        OneTimeToken t = token("tok", OneTimeToken.TYPE_MAGICLINK, "a@b.com",
                Instant.now().minusSeconds(7200), null);
        when(ottRepo.findByTokenHashAndTokenType(anyString(), eq(OneTimeToken.TYPE_MAGICLINK)))
                .thenReturn(Optional.of(t));

        assertThatThrownBy(() -> svc.verifyFlow("magiclink", "tok", "a@b.com", null))
                .isInstanceOf(RuntimeException.class).hasMessageContaining("expired");
        verify(ottRepo).delete(t);
    }

    @Test
    @DisplayName("relates_to mismatch is rejected")
    void relatesToMismatch() {
        OneTimeToken t = token("tok", OneTimeToken.TYPE_MAGICLINK, "a@b.com", Instant.now(), null);
        when(ottRepo.findByTokenHashAndTokenType(anyString(), eq(OneTimeToken.TYPE_MAGICLINK)))
                .thenReturn(Optional.of(t));

        assertThatThrownBy(() -> svc.verifyFlow("magiclink", "tok", "someone-else@b.com", null))
                .isInstanceOf(RuntimeException.class).hasMessageContaining("does not match");
    }

    @Test
    @DisplayName("unknown token is rejected")
    void unknown() {
        when(ottRepo.findByTokenHashAndTokenType(anyString(), anyString())).thenReturn(Optional.empty());
        assertThatThrownBy(() -> svc.verifyFlow("magiclink", "nope", "a@b.com", null))
                .isInstanceOf(RuntimeException.class).hasMessageContaining("expired or is invalid");
    }

    @Test
    @DisplayName("PKCE-bound token returns an auth code instead of a session")
    void pkce() {
        OneTimeToken t = token("tok", OneTimeToken.TYPE_MAGICLINK, "a@b.com", Instant.now(), "challenge123");
        when(ottRepo.findByTokenHashAndTokenType(anyString(), eq(OneTimeToken.TYPE_MAGICLINK)))
                .thenReturn(Optional.of(t));
        when(pkceService.issueAuthCode(any(), eq("challenge123"), any(), eq("magiclink"), anyString()))
                .thenReturn("AUTHCODE");

        OtpService.VerifyResult r = svc.verifyFlow("magiclink", "tok", "a@b.com", null);

        assertThat(r.isPkce()).isTrue();
        assertThat(r.pkceAuthCode()).isEqualTo("AUTHCODE");
        assertThat(r.session()).isNull();
    }
}
