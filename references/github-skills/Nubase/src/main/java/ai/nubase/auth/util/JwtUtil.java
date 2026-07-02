package ai.nubase.auth.util;

import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.ObjectMapper;
import io.jsonwebtoken.*;
import io.jsonwebtoken.security.Keys;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang3.StringUtils;

import javax.crypto.SecretKey;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.Date;
import java.util.HashMap;
import java.util.Map;

/**
 * JWT utility class.
 * Provides JWT parsing and processing functionality.
 */
@Slf4j
public class JwtUtil {

    private static final ObjectMapper objectMapper = new ObjectMapper();

    /**
     * Parse a JWT token (without verifying the signature).
     *
     * @param token JWT token string
     * @return the Claims object, or null if parsing fails
     */
    public static Claims decode(String token) {
        if (StringUtils.isBlank(token)) {
            log.warn("JWT token is empty or null");
            return null;
        }

        try {
            // Use unsecured() to parse the JWT without verifying the signature
            return Jwts.parser()
                    .unsecured()
                    .build()
                    .parseUnsecuredClaims(token)
                    .getPayload();
        } catch (Exception e) {
            log.error("Failed to decode JWT token: {}", e.getMessage());
            return null;
        }
    }

    /**
     * Get the integer value of a specific claim.
     *
     * @param token     JWT token
     * @param claimName claim name
     * @return the integer claim value, or null if not present or parsing fails
     */
    public static Integer getClaimAsInteger(String token, String claimName) {
        Claims claims = decode(token);
        if (claims == null) {
            return null;
        }

        try {
            Object value = claims.get(claimName);
            if (value == null) {
                return null;
            }
            if (value instanceof Integer) {
                return (Integer) value;
            }
            return Integer.valueOf(value.toString());
        } catch (Exception e) {
            log.debug("Failed to parse claim '{}' as Integer: {}", claimName, e.getMessage());
            return null;
        }
    }

    /**
     * Extract the JWT payload as a JSON string.
     *
     * @param token JWT token string
     * @return the payload as a JSON string, or null on failure
     */
    public static String extractPayloadAsJson(String token) {
        if (StringUtils.isBlank(token)) {
            return null;
        }
        Claims claims = decode(token);
        if (claims == null) {
            return null;
        }

        try {
            // Claims is itself a Map and can be serialized directly
            Map<String, Object> payload = new HashMap<>(claims);
            return objectMapper.writeValueAsString(payload);
        } catch (JsonProcessingException e) {
            log.error("Failed to serialize JWT payload to JSON: {}", e.getMessage());
            return null;
        }
    }

    /**
     * Get the value of a specific claim.
     *
     * @param token     JWT token
     * @param claimName claim name
     * @return the claim value, or null if not present
     */
    public static String getClaimAsString(String token, String claimName) {
        Claims claims = decode(token);
        if (claims == null) {
            return null;
        }

        Object value = claims.get(claimName);
        return value != null ? value.toString() : null;
    }

    /**
     * Get the sub field of the JWT payload.
     *
     * @param token JWT token
     * @return the sub field value, or null if not present
     */
    public static String getSub(String token) {
        Claims claims = decode(token);
        return claims != null ? claims.getSubject() : null;
    }

    public static String getRole(String token) {
        return getClaimAsString(token, JwtConstants.CLAIM_ROLE);
    }

    /**
     * Generate a JWT token.
     *
     * @param email             user email
     * @param jwtSecret         JWT secret
     * @param expirationMinutes expiration time in minutes
     * @return the generated JWT token
     */
    public static String generateToken(String email, String jwtSecret, int expirationMinutes) {
        if (StringUtils.isBlank(email) || StringUtils.isBlank(jwtSecret)) {
            throw new IllegalArgumentException("email and jwtSecret must not be blank");
        }

        try {
            Instant now = Instant.now();
            Instant expiration = now.plusSeconds(expirationMinutes * 60L);

            SecretKey key = Keys.hmacShaKeyFor(jwtSecret.getBytes(StandardCharsets.UTF_8));

            String token = Jwts.builder()
                    .setSubject(email)
                    .setIssuedAt(Date.from(now))
                    .setExpiration(Date.from(expiration))
                    .signWith(key, SignatureAlgorithm.HS256)
                    .compact();

            log.debug(JwtConstants.LOG_TOKEN_GENERATED, email);
            return token;

        } catch (Exception e) {
            log.error(JwtConstants.ERROR_TOKEN_GENERATION_FAILED + ", user: {}", email, e);
            throw new RuntimeException(JwtConstants.ERROR_TOKEN_GENERATION_FAILED, e);
        }
    }

    /**
     * Generate a JWT token (using the default expiration of 3 minutes).
     *
     * @param email     user email
     * @param jwtSecret JWT secret
     * @return the generated JWT token
     */
    public static String generateToken(String email, String jwtSecret) {
        return generateToken(email, jwtSecret, JwtConstants.DEFAULT_EXPIRATION_MINUTES);
    }

    /**
     * Generate a JWT token (with the expiration specified in hours).
     *
     * @param email           user email
     * @param jwtSecret       JWT secret
     * @param expirationHours expiration time in hours
     * @return the generated JWT token
     */
    public static String generateTokenByHours(String email, String jwtSecret, int expirationHours) {
        return generateToken(email, jwtSecret, expirationHours * 60);
    }

    /**
     * Verify and parse a JWT token.
     *
     * @param token     JWT token string
     * @param jwtSecret JWT secret
     * @return the Claims object, or null if verification fails
     */
    public static Claims parseAndVerify(String token, String jwtSecret) {
        if (StringUtils.isBlank(token) || StringUtils.isBlank(jwtSecret)) {
            return null;
        }

        try {
            SecretKey key = Keys.hmacShaKeyFor(jwtSecret.getBytes(StandardCharsets.UTF_8));
            return Jwts.parser()
                    .setSigningKey(key)
                    .build()
                    .parseClaimsJws(token)
                    .getBody();
        } catch (Exception e) {
            log.debug("Failed to verify JWT token: {}", e.getMessage());
            return null;
        }
    }

    /**
     * Issue a JWT with custom claims and an expiration in seconds.
     *
     * @param jwtSecret       JWT secret
     * @param claims          custom claims
     * @param expiresInSeconds expiration time in seconds
     * @return the signed JWT string
     */
    public static String createToken(String jwtSecret, Map<String, Object> claims, int expiresInSeconds) {
        SecretKey key = Keys.hmacShaKeyFor(jwtSecret.getBytes(StandardCharsets.UTF_8));
        Instant now = Instant.now();

        return Jwts.builder()
                .setClaims(claims)
                .setIssuedAt(Date.from(now))
                .setExpiration(Date.from(now.plusSeconds(expiresInSeconds)))
                .signWith(key, SignatureAlgorithm.HS256)
                .compact();
    }

    /**
     * Verify a JWT and return the Claims; throws IllegalArgumentException on failure.
     *
     * @param token     JWT token
     * @param jwtSecret JWT secret
     * @return the Claims object
     * @throws IllegalArgumentException when verification fails
     */
    public static Claims verifyToken(String token, String jwtSecret) {
        SecretKey key = Keys.hmacShaKeyFor(jwtSecret.getBytes(StandardCharsets.UTF_8));
        try {
            return Jwts.parser()
                    .setSigningKey(key)
                    .build()
                    .parseClaimsJws(token)
                    .getBody();
        } catch (Exception e) {
            throw new IllegalArgumentException("Invalid JWT", e);
        }
    }
}
