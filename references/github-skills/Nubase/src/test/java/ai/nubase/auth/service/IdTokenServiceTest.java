package ai.nubase.auth.service;

import ai.nubase.auth.dto.oauth.OAuthUserInfo;
import ai.nubase.common.config.AuthConfig;
import io.jsonwebtoken.Jwts;
import io.jsonwebtoken.SignatureAlgorithm;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.PublicKey;
import java.time.Instant;
import java.util.Date;
import java.util.HashMap;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * Unit tests for {@link IdTokenService}: RS256 signature + issuer/nonce/expiry validation.
 * No network — a test subclass overrides {@code resolveKey} to return the locally-generated key.
 */
@DisplayName("IdTokenService (id_token verification)")
class IdTokenServiceTest {

    private KeyPair keyPair;
    private IdTokenService svc;

    @BeforeEach
    void setUp() throws Exception {
        KeyPairGenerator kpg = KeyPairGenerator.getInstance("RSA");
        kpg.initialize(2048);
        keyPair = kpg.generateKeyPair();

        final PublicKey pub = keyPair.getPublic();
        svc = new IdTokenService(new AuthConfig()) {
            @Override
            protected PublicKey resolveKey(String jwksUri, String kid) {
                return pub; // bypass JWKS network fetch
            }
        };
    }

    private String token(String issuer, String nonce, long expiresInSec) {
        Map<String, Object> claims = new HashMap<>();
        claims.put("sub", "google-sub-123");
        claims.put("email", "alice@example.com");
        claims.put("email_verified", true);
        claims.put("name", "Alice");
        claims.put("picture", "https://pic");
        if (nonce != null) claims.put("nonce", nonce);
        return Jwts.builder()
                .setClaims(claims)
                .setIssuer(issuer)
                .setIssuedAt(new Date())
                .setExpiration(Date.from(Instant.now().plusSeconds(expiresInSec)))
                .signWith(keyPair.getPrivate(), SignatureAlgorithm.RS256)
                .compact();
    }

    @Test
    @DisplayName("valid Google id_token yields normalized user info")
    void validToken() {
        OAuthUserInfo info = svc.verify("google", token("https://accounts.google.com", null, 300), null);
        assertThat(info.getProvider()).isEqualTo("google");
        assertThat(info.getProviderId()).isEqualTo("google-sub-123");
        assertThat(info.getEmail()).isEqualTo("alice@example.com");
        assertThat(info.isEmailVerified()).isTrue();
        assertThat(info.getName()).isEqualTo("Alice");
    }

    @Test
    @DisplayName("rejects a disallowed issuer")
    void wrongIssuer() {
        assertThatThrownBy(() -> svc.verify("google", token("https://evil.example", null, 300), null))
                .isInstanceOf(RuntimeException.class)
                .hasMessageContaining("issuer");
    }

    @Test
    @DisplayName("enforces nonce when supplied")
    void nonceMismatch() {
        String t = token("https://accounts.google.com", "expected-nonce", 300);
        assertThat(svc.verify("google", t, "expected-nonce").getEmail()).isEqualTo("alice@example.com");
        assertThatThrownBy(() -> svc.verify("google", t, "different-nonce"))
                .isInstanceOf(RuntimeException.class)
                .hasMessageContaining("nonce");
    }

    @Test
    @DisplayName("rejects an expired token")
    void expired() {
        assertThatThrownBy(() -> svc.verify("google", token("https://accounts.google.com", null, -10), null))
                .isInstanceOf(RuntimeException.class)
                .hasMessageContaining("Invalid id_token");
    }

    @Test
    @DisplayName("rejects an unknown provider")
    void unknownProvider() {
        assertThatThrownBy(() -> svc.verify("myspace", token("https://accounts.google.com", null, 300), null))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("Unsupported id_token provider");
    }
}
