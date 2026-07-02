package ai.nubase.auth.service;

import ai.nubase.common.config.AuthConfig;
import cn.hutool.core.codec.Base32;
import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Service;

import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;

/**
 * RFC 6238 (TOTP) / RFC 4226 (HOTP) implementation with no external dependency
 * beyond the JDK + hutool Base32. Compatible with Google Authenticator, Authy, 1Password, etc.
 */
@Service
@RequiredArgsConstructor
public class TotpService {

    private static final SecureRandom RANDOM = new SecureRandom();
    private static final String HMAC_ALGO = "HmacSHA1";

    private final EffectiveAuthConfig effectiveAuthConfig;

    /** Generate a new Base32-encoded shared secret (160 bits). */
    public String generateSecret() {
        byte[] bytes = new byte[20];
        RANDOM.nextBytes(bytes);
        return Base32.encode(bytes);
    }

    /**
     * Build the {@code otpauth://totp/...} provisioning URI used to render the QR code.
     *
     * @param accountName usually the user's email
     * @param secret      the Base32 secret
     */
    public String buildOtpAuthUri(String accountName, String secret) {
        AuthConfig.MfaSettings mfa = effectiveAuthConfig.mfa();
        String issuer = mfa.getIssuer();
        String label = enc(issuer) + ":" + enc(accountName);
        return "otpauth://totp/" + label
                + "?secret=" + secret
                + "&issuer=" + enc(issuer)
                + "&algorithm=SHA1"
                + "&digits=" + mfa.getDigits()
                + "&period=" + mfa.getPeriod();
    }

    /** Verify a code against the secret, tolerating the configured +/- step drift. */
    public boolean verifyCode(String secret, String code) {
        if (secret == null || code == null) {
            return false;
        }
        String trimmed = code.trim();
        if (!trimmed.matches("\\d{4,8}")) {
            return false;
        }
        AuthConfig.MfaSettings mfa = effectiveAuthConfig.mfa();
        long counter = System.currentTimeMillis() / 1000L / mfa.getPeriod();
        int drift = Math.max(0, mfa.getAllowedDrift());
        byte[] key = Base32.decode(secret);
        for (long i = -drift; i <= drift; i++) {
            String candidate = generate(key, counter + i, mfa.getDigits());
            if (constantTimeEquals(candidate, trimmed)) {
                return true;
            }
        }
        return false;
    }

    private String generate(byte[] key, long counter, int digits) {
        try {
            byte[] data = new byte[8];
            long value = counter;
            for (int i = 7; i >= 0; i--) {
                data[i] = (byte) (value & 0xff);
                value >>= 8;
            }
            Mac mac = Mac.getInstance(HMAC_ALGO);
            mac.init(new SecretKeySpec(key, HMAC_ALGO));
            byte[] hash = mac.doFinal(data);

            int offset = hash[hash.length - 1] & 0x0f;
            int binary = ((hash[offset] & 0x7f) << 24)
                    | ((hash[offset + 1] & 0xff) << 16)
                    | ((hash[offset + 2] & 0xff) << 8)
                    | (hash[offset + 3] & 0xff);
            int otp = binary % (int) Math.pow(10, digits);
            return String.format("%0" + digits + "d", otp);
        } catch (Exception e) {
            throw new RuntimeException("Failed to generate TOTP code", e);
        }
    }

    private boolean constantTimeEquals(String a, String b) {
        if (a.length() != b.length()) {
            return false;
        }
        int result = 0;
        for (int i = 0; i < a.length(); i++) {
            result |= a.charAt(i) ^ b.charAt(i);
        }
        return result == 0;
    }

    private static String enc(String value) {
        return URLEncoder.encode(value == null ? "" : value, StandardCharsets.UTF_8);
    }
}
