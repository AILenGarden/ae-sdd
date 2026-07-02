package ai.nubase.auth.service;

import ai.nubase.auth.dto.request.platform.PlatformSignInRequest;
import ai.nubase.auth.dto.request.platform.PlatformSignUpRequest;
import ai.nubase.auth.dto.response.platform.PlatformAuthResponse;
import ai.nubase.auth.exception.EmailAlreadyExistsException;
import ai.nubase.metadata.entity.PlatformUser;
import ai.nubase.metadata.repository.PlatformUserRepository;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.test.context.ActiveProfiles;
import org.springframework.test.context.TestPropertySource;
import org.springframework.transaction.annotation.Transactional;

import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * Real-database integration test for the platform auth service.
 * Hits the dev metadata Postgres — each test creates accounts with a unique email
 * prefix and tears them down in {@link #cleanup()}.
 */
@SpringBootTest
@ActiveProfiles("dev")
// This IT exercises password + JWT mechanics directly; it can't fetch an emailed OTP, so it runs with
// email verification off (signUp/signIn then return a token immediately). The OTP flow is covered by
// PlatformAuthControllerTest and PlatformOtpServiceTest.
@TestPropertySource(properties = "nubase.platform.email-verification-enabled=false")
@DisplayName("PlatformAuthService (dev metadata DB)")
class PlatformAuthServiceIT {

    @Autowired
    private PlatformAuthService service;

    @Autowired
    private PlatformUserRepository repo;

    private final List<UUID> createdIds = new ArrayList<>();

    private String uniqueEmail() {
        String id = UUID.randomUUID().toString().substring(0, 8);
        return "test_platform_" + id + "@nubase-test.local";
    }

    @AfterEach
    @Transactional("metadataTransactionManager")
    void cleanup() {
        for (UUID id : createdIds) {
            try {
                repo.deleteById(id);
            } catch (Exception ignored) {
                // best-effort
            }
        }
        createdIds.clear();
    }

    @Test
    @DisplayName("signUp returns access token, user payload with assigned role")
    void signUp_succeeds() {
        PlatformSignUpRequest req = new PlatformSignUpRequest();
        req.setEmail(uniqueEmail());
        req.setPassword("test-password-12345");
        req.setFullName("Test User");

        PlatformAuthResponse res = service.signUp(req).token();
        track(res);

        assertThat(res.getAccessToken()).isNotBlank();
        assertThat(res.getTokenType()).isEqualTo("Bearer");
        assertThat(res.getExpiresIn()).isPositive();
        assertThat(res.getUser().getEmail()).isEqualTo(req.getEmail().toLowerCase());
        assertThat(res.getUser().getRole())
                .isIn(PlatformAuthService.PLATFORM_ROLE_SUPER_ADMIN, PlatformAuthService.PLATFORM_ROLE_USER);
    }

    @Test
    @DisplayName("issued JWT validates back to the same subject")
    void issuedJwt_roundtrips() {
        PlatformSignUpRequest req = new PlatformSignUpRequest();
        req.setEmail(uniqueEmail());
        req.setPassword("test-password-12345");

        PlatformAuthResponse res = service.signUp(req).token();
        track(res);

        UUID subject = service.validateAndGetSubject(res.getAccessToken());
        assertThat(subject.toString()).isEqualTo(res.getUser().getId());
    }

    @Test
    @DisplayName("duplicate email is rejected with EmailAlreadyExistsException")
    void duplicateEmail_rejected() {
        PlatformSignUpRequest first = new PlatformSignUpRequest();
        first.setEmail(uniqueEmail());
        first.setPassword("test-password-12345");
        PlatformAuthResponse one = service.signUp(first).token();
        track(one);

        PlatformSignUpRequest dup = new PlatformSignUpRequest();
        dup.setEmail(first.getEmail());
        dup.setPassword("another-password");

        assertThatThrownBy(() -> service.signUp(dup))
                .isInstanceOf(EmailAlreadyExistsException.class);
    }

    @Test
    @DisplayName("signIn succeeds with correct password, fails with wrong password")
    void signIn_passwordCheck() {
        String email = uniqueEmail();
        String password = "test-password-12345";

        PlatformSignUpRequest reg = new PlatformSignUpRequest();
        reg.setEmail(email);
        reg.setPassword(password);
        track(service.signUp(reg).token());

        PlatformSignInRequest ok = new PlatformSignInRequest();
        ok.setEmail(email);
        ok.setPassword(password);
        PlatformAuthResponse res = service.signIn(ok).token();
        assertThat(res.getAccessToken()).isNotBlank();

        PlatformSignInRequest bad = new PlatformSignInRequest();
        bad.setEmail(email);
        bad.setPassword("wrong-password");
        assertThatThrownBy(() -> service.signIn(bad))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("Invalid email or password");
    }

    @Test
    @DisplayName("signIn rejects disabled accounts")
    void disabled_cannotSignIn() {
        String email = uniqueEmail();
        String password = "test-password-12345";

        PlatformSignUpRequest reg = new PlatformSignUpRequest();
        reg.setEmail(email);
        reg.setPassword(password);
        PlatformAuthResponse one = service.signUp(reg).token();
        track(one);

        // Flip is_active = false directly via repo
        PlatformUser u = repo.findById(UUID.fromString(one.getUser().getId())).orElseThrow();
        u.setIsActive(false);
        repo.save(u);

        PlatformSignInRequest req = new PlatformSignInRequest();
        req.setEmail(email);
        req.setPassword(password);
        assertThatThrownBy(() -> service.signIn(req))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("Account is disabled");
    }

    @Test
    @DisplayName("validateAndGetSubject rejects a tampered or unsigned token")
    void invalidToken_rejected() {
        // Random string that's not a signed platform JWT.
        assertThatThrownBy(() -> service.validateAndGetSubject("not.a.real.jwt"))
                .isInstanceOf(Exception.class);
    }

    private void track(PlatformAuthResponse res) {
        if (res != null && res.getUser() != null && res.getUser().getId() != null) {
            createdIds.add(UUID.fromString(res.getUser().getId()));
        }
    }
}
