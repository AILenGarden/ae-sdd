package ai.nubase.auth.util;

import org.springframework.stereotype.Component;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.SecureRandom;
import java.util.Base64;
import java.util.HexFormat;

@Component
public class TokenGenerator {

    private static final SecureRandom RANDOM = new SecureRandom();
    private static final Base64.Encoder ENCODER = Base64.getUrlEncoder().withoutPadding();

    /**
     * Generate a secure random token
     * @return Base64 encoded random token
     */
    public String generateSecureToken() {
        byte[] randomBytes = new byte[32];
        RANDOM.nextBytes(randomBytes);
        return ENCODER.encodeToString(randomBytes);
    }

    /**
     * Generate a secure random token with custom length
     * @param byteLength Number of random bytes
     * @return Base64 encoded random token
     */
    public String generateSecureToken(int byteLength) {
        byte[] randomBytes = new byte[byteLength];
        RANDOM.nextBytes(randomBytes);
        return ENCODER.encodeToString(randomBytes);
    }

    /**
     * Generate a numeric OTP
     * @param length Number of digits
     * @return Numeric OTP string
     */
    public String generateNumericOTP(int length) {
        StringBuilder otp = new StringBuilder();
        for (int i = 0; i < length; i++) {
            otp.append(RANDOM.nextInt(10));
        }
        return otp.toString();
    }

    /**
     * Generate a refresh token with v1 prefix (compatible with Supabase)
     * @return Refresh token string
     */
    public String generateRefreshToken() {
        return "v1." + generateSecureToken(48);
    }

    /**
     * SHA-256 hash a value (token or OTP code) and return lowercase hex.
     * Used to store one-time tokens at rest rather than the plaintext value.
     */
    public String sha256(String value) {
        try {
            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            byte[] hash = digest.digest(value.getBytes(StandardCharsets.UTF_8));
            return HexFormat.of().formatHex(hash);
        } catch (Exception e) {
            throw new RuntimeException("Failed to hash token", e);
        }
    }
}
