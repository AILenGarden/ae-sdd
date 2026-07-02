package ai.nubase.postgrest.auth;

import ai.nubase.postgrest.config.PostgRESTConfig;
import io.jsonwebtoken.Claims;
import io.jsonwebtoken.Jwts;
import io.jsonwebtoken.security.Keys;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang3.StringUtils;

import javax.crypto.SecretKey;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.Base64;

/**
 * JWT authentication service
 * Equivalent to PostgREST's Auth.Jwt module
 * <p>
 * NOTE: This class is NO LONGER a Spring singleton.
 * Each database has its own JwtService instance managed by JwtServiceManager.
 */
@Slf4j
public class JwtService {

  private final PostgRESTConfig config;
  private final String databaseKey;  // NEW: Track which database this service belongs to
  private SecretKey secretKey;

  /**
   * Constructor with database key for logging
   *
   * @param config      PostgREST configuration
   * @param databaseKey the database key (for logging)
   */
  public JwtService(PostgRESTConfig config, String databaseKey) {
    this.config = config;
    this.databaseKey = databaseKey;
    initializeSecret();
  }

  private void initializeSecret() {
    try {
      String secret = null;

      // Try loading from file first
      if (config.getJwtSecretFile() != null && !config.getJwtSecretFile().isEmpty()) {
        secret = Files.readString(Paths.get(config.getJwtSecretFile()), StandardCharsets.UTF_8).strip();
        log.info("Loaded JWT secret from file: {}", config.getJwtSecretFile());
      }
      // Fall back to direct secret
      else if (config.getJwtSecret() != null && !config.getJwtSecret().isEmpty()) {
        secret = config.getJwtSecret();
        log.info("Using JWT secret from configuration");
      }

      if (secret != null) {
        byte[] keyBytes;
        if (config.getJwtSecretIsBase64()) {
          keyBytes = Base64.getDecoder().decode(secret);
        } else {
          keyBytes = secret.getBytes(StandardCharsets.UTF_8);
        }

        this.secretKey = Keys.hmacShaKeyFor(keyBytes);
        log.info("JWT secret initialized successfully");
      } else {
        log.warn("No JWT secret configured - JWT authentication disabled");
      }
    } catch (Exception e) {
      log.error("Failed to initialize JWT secret", e);
      throw new RuntimeException("JWT secret initialization failed", e);
    }
  }

  public AuthResult authenticate(String token) {
    //todo
    if (StringUtils.isBlank(token)) {
      return AuthResult.builder()
        .role(config.getDbAnonRole())
        .authenticated(false)
        .build();
    }

    if (secretKey == null) {
      // No JWT configured, use anonymous role
      return AuthResult.builder()
        .role(config.getDbAnonRole())
        .authenticated(false)
        .build();
    }

    if (token == null || token.isEmpty()) {
      return AuthResult.builder()
        .role(config.getDbAnonRole())
        .authenticated(false)
        .build();
    }

    try {
      // Remove "Bearer " prefix if present
      if (token.startsWith("Bearer ")) {
        token = token.substring(7);
      }

      Claims claims = Jwts.parser()
        .verifyWith(secretKey)
        .build()
        .parseSignedClaims(token)
        .getPayload();

      // Validate audience if configured
      if (config.getJwtAudience() != null && !config.getJwtAudience().isEmpty()) {
        String audience = claims.getAudience().stream().findFirst().orElse(null);
        if (!config.getJwtAudience().equals(audience)) {
          log.warn("JWT audience mismatch. Expected: {}, Got: {}",
            config.getJwtAudience(), audience);
          return AuthResult.builder()
            .role(config.getDbAnonRole())
            .authenticated(false)
            .error("Invalid token audience")
            .build();
        }
      }

      // Extract role from claims
      String role = extractRole(claims);

      return AuthResult.builder()
        .role(role != null ? role : config.getDbAnonRole())
        .authenticated(true)
        .claims(claims)
        .build();

    } catch (Exception e) {
      log.warn("JWT validation failed: {}", e.getMessage());
      return AuthResult.builder()
        .role(config.getDbAnonRole())
        .authenticated(false)
        .error(e.getMessage())
        .build();
    }
  }

  private String extractRole(Claims claims) {
    // Parse role claim key (supports nested paths like ".role" or "user.role")
    String roleClaimKey = config.getJwtRoleClaimKey();

    if (roleClaimKey.startsWith(".")) {
      roleClaimKey = roleClaimKey.substring(1);
    }

    String[] parts = roleClaimKey.split("\\.");
    Object current = claims;

    for (String part : parts) {
      if (current instanceof Claims) {
        current = ((Claims) current).get(part);
      } else if (current instanceof java.util.Map) {
        current = ((java.util.Map<?, ?>) current).get(part);
      } else {
        return null;
      }

      if (current == null) {
        break;
      }
    }

    return current != null ? current.toString() : null;
  }
}
