package ai.nubase.auth.util;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Pure unit tests for {@link TokenGenerator}.
 */
@DisplayName("TokenGenerator")
class TokenGeneratorTest {

    private final TokenGenerator gen = new TokenGenerator();

    @Test
    @DisplayName("sha256 is deterministic, lowercase hex, 64 chars")
    void sha256() {
        String a = gen.sha256("hello");
        String b = gen.sha256("hello");
        assertThat(a).isEqualTo(b).hasSize(64).matches("[0-9a-f]{64}");
        // Known vector for "hello"
        assertThat(a).isEqualTo("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
        assertThat(gen.sha256("hello!")).isNotEqualTo(a);
    }

    @Test
    @DisplayName("generateNumericOTP yields the requested number of digits")
    void numericOtp() {
        String otp = gen.generateNumericOTP(6);
        assertThat(otp).hasSize(6).matches("\\d{6}");
        assertThat(gen.generateNumericOTP(8)).hasSize(8).matches("\\d{8}");
    }

    @Test
    @DisplayName("secure tokens are URL-safe and unique; refresh tokens carry the v1 prefix")
    void tokens() {
        String t1 = gen.generateSecureToken();
        String t2 = gen.generateSecureToken();
        assertThat(t1).isNotEqualTo(t2).matches("[A-Za-z0-9_-]+");
        assertThat(gen.generateRefreshToken()).startsWith("v1.");
    }
}
