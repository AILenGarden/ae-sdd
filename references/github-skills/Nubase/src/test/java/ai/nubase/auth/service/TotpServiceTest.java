package ai.nubase.auth.service;

import ai.nubase.common.config.AuthConfig;
import cn.hutool.core.codec.Base32;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Pure unit tests for {@link TotpService} (RFC 6238). No Spring / DB.
 */
@DisplayName("TotpService (RFC 6238)")
class TotpServiceTest {

    private final AuthConfig authConfig = new AuthConfig();
    private final TotpService totp = new TotpService(new EffectiveAuthConfig(authConfig));

    @Test
    @DisplayName("generateSecret returns a decodable 160-bit Base32 secret")
    void generateSecret() {
        String secret = totp.generateSecret();
        assertThat(secret).isNotBlank();
        byte[] decoded = Base32.decode(secret);
        assertThat(decoded).hasSize(20); // 160 bits
    }

    @Test
    @DisplayName("buildOtpAuthUri produces a well-formed otpauth:// URI")
    void otpAuthUri() {
        String uri = totp.buildOtpAuthUri("alice@example.com", "JBSWY3DPEHPK3PXP");
        assertThat(uri).startsWith("otpauth://totp/");
        assertThat(uri).contains("secret=JBSWY3DPEHPK3PXP");
        assertThat(uri).contains("issuer=Nubase");
        assertThat(uri).contains("digits=6");
        assertThat(uri).contains("period=30");
        // account + issuer are URL-encoded into the label
        assertThat(uri).contains("alice%40example.com");
    }

    @Test
    @DisplayName("verifyCode accepts the current code and codes within the drift window")
    void verifyCurrentAndDrift() throws Exception {
        String secret = totp.generateSecret();
        long step = System.currentTimeMillis() / 1000L / 30L;

        assertThat(totp.verifyCode(secret, codeFor(secret, step))).isTrue();
        // allowedDrift defaults to 1 → previous and next steps are accepted
        assertThat(totp.verifyCode(secret, codeFor(secret, step - 1))).isTrue();
        assertThat(totp.verifyCode(secret, codeFor(secret, step + 1))).isTrue();
    }

    @Test
    @DisplayName("verifyCode rejects a code outside the drift window")
    void rejectsOutsideDrift() throws Exception {
        String secret = totp.generateSecret();
        long step = System.currentTimeMillis() / 1000L / 30L;
        assertThat(totp.verifyCode(secret, codeFor(secret, step + 5))).isFalse();
    }

    @Test
    @DisplayName("verifyCode rejects malformed input")
    void rejectsMalformed() {
        String secret = totp.generateSecret();
        assertThat(totp.verifyCode(secret, null)).isFalse();
        assertThat(totp.verifyCode(secret, "")).isFalse();
        assertThat(totp.verifyCode(secret, "abcdef")).isFalse();   // non-numeric
        assertThat(totp.verifyCode(secret, "12")).isFalse();        // too short
        assertThat(totp.verifyCode(null, "123456")).isFalse();
    }

    /** Reference RFC 6238 generator (SHA1, 6 digits) to cross-check the service. */
    private static String codeFor(String base32Secret, long counter) throws Exception {
        byte[] key = Base32.decode(base32Secret);
        byte[] data = new byte[8];
        long v = counter;
        for (int i = 7; i >= 0; i--) {
            data[i] = (byte) (v & 0xff);
            v >>= 8;
        }
        Mac mac = Mac.getInstance("HmacSHA1");
        mac.init(new SecretKeySpec(key, "HmacSHA1"));
        byte[] hash = mac.doFinal(data);
        int offset = hash[hash.length - 1] & 0x0f;
        int binary = ((hash[offset] & 0x7f) << 24)
                | ((hash[offset + 1] & 0xff) << 16)
                | ((hash[offset + 2] & 0xff) << 8)
                | (hash[offset + 3] & 0xff);
        return String.format("%06d", binary % 1_000_000);
    }
}
