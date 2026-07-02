package ai.nubase.auth.service;

import ai.nubase.auth.dto.response.AuthResponse;
import ai.nubase.auth.entity.MfaFactor;
import ai.nubase.auth.entity.Session;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.MfaChallengeRepository;
import ai.nubase.auth.repository.MfaFactorRepository;
import ai.nubase.auth.repository.SessionRepository;
import ai.nubase.common.config.AuthConfig;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.util.Optional;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.*;

/**
 * Unit tests for {@link MfaService#verify}: wrong code rejected; valid TOTP upgrades the
 * session to AAL2, records the AMR, and issues fresh tokens.
 */
@DisplayName("MfaService.verify")
class MfaServiceTest {

    private MfaFactorRepository factorRepository;
    private SessionRepository sessionRepository;
    private TotpService totpService;
    private AuthResponseFactory authResponseFactory;
    private MfaService svc;

    private final AuthResponse sentinel = AuthResponse.success("acc", "ref", 3600, null);
    private UUID userId;
    private UUID factorId;
    private User user;
    private MfaFactor factor;
    private Session session;

    @BeforeEach
    void setUp() {
        factorRepository = mock(MfaFactorRepository.class);
        sessionRepository = mock(SessionRepository.class);
        totpService = mock(TotpService.class);
        authResponseFactory = mock(AuthResponseFactory.class);

        svc = new MfaService(factorRepository, mock(MfaChallengeRepository.class), sessionRepository,
                totpService, mock(SmsService.class), authResponseFactory, mock(AuditService.class),
                mock(RateLimiterService.class), new EffectiveAuthConfig(new AuthConfig()));

        userId = UUID.randomUUID();
        factorId = UUID.randomUUID();
        user = User.builder().id(userId).email("a@b.com").build();
        factor = MfaFactor.builder().id(factorId).user(user)
                .factorType(MfaFactor.TYPE_TOTP).status(MfaFactor.STATUS_UNVERIFIED).secret("SECRET32").build();
        session = Session.builder().id(UUID.randomUUID()).user(user).aal("aal1").build();

        when(factorRepository.findByIdAndUserId(factorId, userId)).thenReturn(Optional.of(factor));
        when(factorRepository.save(any())).thenAnswer(inv -> inv.getArgument(0));
        when(sessionRepository.findById(session.getId())).thenReturn(Optional.of(session));
        when(sessionRepository.save(any())).thenAnswer(inv -> inv.getArgument(0));
        when(authResponseFactory.issueTokens(any(), any())).thenReturn(sentinel);
    }

    @Test
    @DisplayName("wrong TOTP code is rejected; no session upgrade")
    void wrongCode() {
        when(totpService.verifyCode("SECRET32", "000000")).thenReturn(false);

        assertThatThrownBy(() -> svc.verify(user, factorId.toString(), null, "000000", session.getId().toString()))
                .isInstanceOf(RuntimeException.class).hasMessageContaining("Invalid MFA code");

        assertThat(factor.getStatus()).isEqualTo(MfaFactor.STATUS_UNVERIFIED);
        verify(authResponseFactory, never()).issueTokens(any(), any());
    }

    @Test
    @DisplayName("valid TOTP code verifies the factor, upgrades session to AAL2, records AMR, issues tokens")
    void validCode() {
        when(totpService.verifyCode("SECRET32", "123456")).thenReturn(true);

        AuthResponse res = svc.verify(user, factorId.toString(), null, "123456", session.getId().toString());

        assertThat(res).isSameAs(sentinel);
        assertThat(factor.getStatus()).isEqualTo(MfaFactor.STATUS_VERIFIED);
        assertThat(session.getAal()).isEqualTo(AuthResponseFactory.AAL2);
        assertThat(session.getFactorId()).isEqualTo(factorId);
        verify(authResponseFactory).recordAmr(session, ai.nubase.auth.entity.MfaAmrClaim.METHOD_TOTP);
        verify(authResponseFactory).issueTokens(user, session);
    }
}
