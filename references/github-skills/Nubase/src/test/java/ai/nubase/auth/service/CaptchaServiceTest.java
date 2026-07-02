package ai.nubase.auth.service;

import ai.nubase.common.config.AuthConfig;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThatCode;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * Pure unit tests for {@link CaptchaService} (the local guard logic; the remote siteverify
 * call is not exercised here).
 */
@DisplayName("CaptchaService")
class CaptchaServiceTest {

    @Test
    @DisplayName("disabled CAPTCHA is a no-op even with a blank token")
    void disabledNoOp() {
        AuthConfig cfg = new AuthConfig();
        cfg.getCaptcha().setEnabled(false);
        CaptchaService captcha = new CaptchaService(new EffectiveAuthConfig(cfg));
        assertThatCode(() -> captcha.verify(null)).doesNotThrowAnyException();
        assertThatCode(() -> captcha.verify("")).doesNotThrowAnyException();
    }

    @Test
    @DisplayName("enabled CAPTCHA rejects a missing token before any network call")
    void enabledRequiresToken() {
        AuthConfig cfg = new AuthConfig();
        cfg.getCaptcha().setEnabled(true);
        cfg.getCaptcha().setSecret("test-secret");
        CaptchaService captcha = new CaptchaService(new EffectiveAuthConfig(cfg));
        assertThatThrownBy(() -> captcha.verify("  "))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("captcha");
    }
}
