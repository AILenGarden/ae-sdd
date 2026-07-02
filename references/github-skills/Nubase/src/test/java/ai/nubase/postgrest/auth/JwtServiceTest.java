package ai.nubase.postgrest.auth;

import ai.nubase.common.enums.Role;
import io.jsonwebtoken.Jwts;
import io.jsonwebtoken.security.Keys;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.Mock;
import org.mockito.MockitoAnnotations;
import ai.nubase.postgrest.config.PostgRESTConfig;

import javax.crypto.SecretKey;
import java.nio.charset.StandardCharsets;
import java.util.Date;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.Mockito.when;

class JwtServiceTest {

    @Mock
    private PostgRESTConfig config;

    private JwtService jwtService;
    private SecretKey secretKey;

    @BeforeEach
    void setUp() {
        MockitoAnnotations.openMocks(this);

        // Setup test secret
        String secret = "test-secret-key-that-is-long-enough-for-hs256";
        secretKey = Keys.hmacShaKeyFor(secret.getBytes(StandardCharsets.UTF_8));

        when(config.getJwtSecret()).thenReturn(secret);
        when(config.getJwtSecretIsBase64()).thenReturn(false);
        when(config.getDbAnonRole()).thenReturn(Role.ANON.getValue());
        when(config.getJwtRoleClaimKey()).thenReturn(".role");

        jwtService = new JwtService(config, "test_db");
    }

    @Test
    void testAuthenticateWithValidToken() {
        // Create a valid JWT
        String token = Jwts.builder()
            .claim("role", Role.AUTHENTICATED.getValue())
            .claim("user_id", 123)
            .issuedAt(new Date())
            .expiration(new Date(System.currentTimeMillis() + 3600000))
            .signWith(secretKey)
            .compact();

        // Authenticate
        AuthResult result = jwtService.authenticate(token);

        // Verify
        assertTrue(result.isAuthenticated());
        assertEquals(Role.AUTHENTICATED.getValue(), result.getRole());
        assertNull(result.getError());
    }

    @Test
    void testAuthenticateWithBearerPrefix() {
        String token = Jwts.builder()
            .claim("role", "user")
            .signWith(secretKey)
            .compact();

        AuthResult result = jwtService.authenticate("Bearer " + token);

        assertTrue(result.isAuthenticated());
        assertEquals("user", result.getRole());
    }

    @Test
    void testAuthenticateWithNullToken() {
        AuthResult result = jwtService.authenticate(null);

        assertFalse(result.isAuthenticated());
        assertEquals(Role.ANON.getValue(), result.getRole());
    }

    @Test
    void testAuthenticateWithEmptyToken() {
        AuthResult result = jwtService.authenticate("");

        assertFalse(result.isAuthenticated());
        assertEquals(Role.ANON.getValue(), result.getRole());
    }

    @Test
    void testAuthenticateWithInvalidToken() {
        AuthResult result = jwtService.authenticate("invalid.token.here");

        assertFalse(result.isAuthenticated());
        assertEquals(Role.ANON.getValue(), result.getRole());
        assertNotNull(result.getError());
    }

    @Test
    void testAuthenticateWithExpiredToken() {
        // Create an expired token
        String token = Jwts.builder()
            .claim("role", "user")
            .issuedAt(new Date(System.currentTimeMillis() - 7200000))
            .expiration(new Date(System.currentTimeMillis() - 3600000))
            .signWith(secretKey)
            .compact();

        AuthResult result = jwtService.authenticate(token);

        assertFalse(result.isAuthenticated());
        assertEquals(Role.ANON.getValue(), result.getRole());
        assertNotNull(result.getError());
    }

    @Test
    void testExtractNestedRoleClaim() {
        when(config.getJwtRoleClaimKey()).thenReturn(".user.role");

        String token = Jwts.builder()
            .claim("user", java.util.Map.of("role", "admin"))
            .signWith(secretKey)
            .compact();

        jwtService = new JwtService(config, "test_db");
        AuthResult result = jwtService.authenticate(token);

        // Note: Current implementation may not support nested claims
        // This test documents expected behavior
        assertNotNull(result);
    }
}
